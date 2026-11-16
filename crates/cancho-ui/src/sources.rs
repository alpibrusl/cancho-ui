//! The text of a program before anything reads it (docs/file-budget.md): the file budget (`file-too-long`) and `include "x.part";`.
//!
//! A source file is read here, once. Its size is checked; each include line is replaced by the lines of its part (recursively, with the checks of
//! section 2); and a table from each line of the expanded text to the file and line it came from is kept, so that a diagnostic about the expanded text
//! can be turned back into one about the file the text is in. Everything after this point (lexer, parser, checks, IR, emitter) sees one string.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::CliError;
use crate::pipeline::{
    Confidence, Diagnostic, PositionOut, Remediation, Severity, SpanOut, Subject, Tier,
};

/// The default limit of a source file, in lines (CONTRIBUTING.md section 2).
pub const DEFAULT_MAX_FILE_LINES: u64 = 2000;
/// The byte bound at the default limit: 160 KiB. It scales with the limit, see `max_file_bytes`.
const DEFAULT_MAX_FILE_BYTES: u128 = 160 * 1024;
/// The environment variable that sets the limit when the flag does not.
pub const LIMIT_ENV: &str = "CANCHO_UI_MAX_FILE_LINES";
/// How deep includes nest: the entry's own include is depth 1.
pub const MAX_INCLUDE_DEPTH: usize = 8;
/// The expanded text of one file may be this many lines, or the line limit if that is higher.
const MIN_EXPANDED_LINES: u64 = 100_000;

static LIMIT: AtomicU64 = AtomicU64::new(DEFAULT_MAX_FILE_LINES);

pub fn max_file_lines() -> u64 {
    LIMIT.load(Ordering::Relaxed)
}

/// The byte bound for the current line limit: 163,840 at 2,000 lines, in the same proportion for any other limit.
pub fn max_file_bytes() -> u64 {
    let b =
        DEFAULT_MAX_FILE_BYTES * u128::from(max_file_lines()) / u128::from(DEFAULT_MAX_FILE_LINES);
    u64::try_from(b).unwrap_or(u64::MAX)
}

/// A limit as text: a whole number of at least 1 (digits only). A number too large for 64 bits is "no limit".
pub fn parse_limit(text: &str) -> Result<u64, String> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "`{text}` is not a limit: a limit is a whole number of lines, 1 or more, in digits"
        ));
    }
    let n = text.parse::<u64>().unwrap_or(u64::MAX);
    if n == 0 {
        return Err(
            "a limit of 0 would refuse every file that has a line; use 1 or more".to_string(),
        );
    }
    Ok(n)
}

/// Sets the limit from `--max-file-lines N` (taken out of the arguments, and returned without it) or, when there is no flag, from the environment.
pub fn configure(args: &[String]) -> Result<Vec<String>, CliError> {
    let mut rest = Vec::with_capacity(args.len());
    let mut flag: Option<u64> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--max-file-lines" {
            let v = args.get(i + 1).ok_or_else(|| {
                CliError::Usage("--max-file-lines requires a number of lines".into())
            })?;
            flag = Some(
                parse_limit(v).map_err(|m| CliError::Usage(format!("--max-file-lines: {m}")))?,
            );
            i += 2;
        } else if let Some(v) = a.strip_prefix("--max-file-lines=") {
            flag = Some(
                parse_limit(v).map_err(|m| CliError::Usage(format!("--max-file-lines: {m}")))?,
            );
            i += 1;
        } else {
            rest.push(a.clone());
            i += 1;
        }
    }
    let n = match flag {
        Some(n) => n,
        None => match std::env::var(LIMIT_ENV) {
            Ok(v) => parse_limit(&v).map_err(|m| CliError::Env(format!("{LIMIT_ENV}: {m}")))?,
            Err(std::env::VarError::NotPresent) => DEFAULT_MAX_FILE_LINES,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(CliError::Env(format!("{LIMIT_ENV} is not valid UTF-8")));
            }
        },
    };
    LIMIT.store(n, Ordering::Relaxed);
    Ok(rest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Entry,
    Module,
    Part,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub role: Role,
    pub lines: usize,
    pub bytes: usize,
    pub included_by: Option<String>,
}

/// For each line of the expanded text of one file: the index in `names` of the file it came from, and its line there.
struct FileMap {
    names: Vec<String>,
    origin: Vec<(u32, u32)>,
}

impl FileMap {
    fn locate(&self, line: u32) -> (usize, u32) {
        match line
            .checked_sub(1)
            .and_then(|i| self.origin.get(i as usize))
        {
            Some(&(f, l)) => (f as usize, l),
            // after the last line (an end-of-file error): the last line's file, further down
            None => match self.origin.last() {
                Some(&(f, l)) => (
                    f as usize,
                    l + line.saturating_sub(self.origin.len() as u32),
                ),
                None => (0, line),
            },
        }
    }
}

pub enum LoadError {
    Io(std::io::Error),
    Refused(Box<Diagnostic>),
}

/// What the compile knows about the text it read: which files expanded, and the files it has seen.
pub struct Table {
    root: PathBuf,
    maps: HashMap<String, FileMap>,
    pub infos: Vec<FileInfo>,
}

impl Table {
    /// `root` is the directory of the entry file: includes are relative to it.
    pub fn new(root: &Path) -> Self {
        Table {
            root: root.to_path_buf(),
            maps: HashMap::new(),
            infos: Vec::new(),
        }
    }

    /// Turns a diagnostic about expanded text into one about the file the text is in.
    pub fn remap(&self, d: &mut Diagnostic) {
        if let Some(s) = d.span.as_mut() {
            self.remap_span(s);
        }
        for r in &mut d.related {
            if let Some(s) = r.span.as_mut() {
                self.remap_span(s);
            }
        }
    }

    fn remap_span(&self, s: &mut SpanOut) {
        let Some(m) = self.maps.get(&s.file) else {
            return;
        };
        if s.start.line == 0 {
            return;
        }
        let (sf, sl) = m.locate(s.start.line);
        let (ef, el) = m.locate(s.end.line.max(s.start.line));
        if let Some(n) = m.names.get(sf) {
            s.file = n.clone();
        }
        s.start.line = sl;
        if ef == sf {
            s.end.line = el;
        } else {
            // a span that runs from one file into another: it ends where it starts, on the same line
            s.end.line = sl;
            s.end.column = s.start.column.saturating_add(1);
        }
    }
}

/// Reads a source file, refuses it if it is over the budget, and returns its expanded text.
pub fn load(path: &Path, role: Role, table: &mut Table) -> Result<String, LoadError> {
    let text = std::fs::read_to_string(path).map_err(LoadError::Io)?;
    let shown = path.display().to_string();
    check_budget(&text, &shown).map_err(LoadError::Refused)?;
    table.infos.push(FileInfo {
        path: shown.clone(),
        role,
        lines: text.lines().count(),
        bytes: text.len(),
        included_by: None,
    });
    // a file that does not have the word cannot have an include line
    if !text.contains("include") {
        return Ok(text);
    }
    let mut ex = Expander {
        root: table.root.clone(),
        canon_root: table
            .root
            .canonicalize()
            .unwrap_or_else(|_| table.root.clone()),
        out: String::with_capacity(text.len()),
        origin: Vec::new(),
        names: vec![shown.clone()],
        name_ix: HashMap::new(),
        scan: Scan::default(),
        stack: Vec::new(),
        any: false,
        infos: Vec::new(),
        cap: MIN_EXPANDED_LINES.max(max_file_lines()),
        site: None,
    };
    ex.run(&text, 0, &shown, true).map_err(LoadError::Refused)?;
    if ex.any {
        table.infos.append(&mut ex.infos);
        table.maps.insert(
            shown,
            FileMap {
                names: ex.names,
                origin: ex.origin,
            },
        );
        Ok(ex.out)
    } else {
        Ok(text)
    }
}

fn remedy(text: &str) -> Remediation {
    Remediation {
        description: text.to_string(),
        suggestion: None,
        confidence: Confidence::High,
    }
}

fn file_diag(
    code: &str,
    file: &str,
    at: (u32, u32, u32),
    message: String,
    remediation: &str,
) -> Box<Diagnostic> {
    Box::new(Diagnostic {
        code: code.into(),
        severity: Severity::Error,
        tier: Tier::Static,
        subject: Subject::File {
            path: file.to_string(),
        },
        span: Some(SpanOut {
            file: file.to_string(),
            start: PositionOut {
                line: at.0,
                column: at.1,
            },
            end: PositionOut {
                line: at.0,
                column: at.2,
            },
        }),
        message,
        expected: None,
        actual: None,
        remediation: Some(remedy(remediation)),
        related: Vec::new(),
        notes: Vec::new(),
    })
}

const SPLIT: &str = "Split the file: give each component of the program its own module file and `import` it, or cut a long component into parts with `include \"parts/name.part\";` (docs/file-budget.md).";

/// `file-too-long`: the lines first, then the bytes.
fn check_budget(text: &str, shown: &str) -> Result<(), Box<Diagnostic>> {
    let limit = max_file_lines();
    let lines = text.lines().count() as u64;
    if lines > limit {
        let line = u32::try_from(limit.saturating_add(1)).unwrap_or(u32::MAX);
        return Err(file_diag(
            "file-too-long",
            shown,
            (line, 0, 0),
            format!(
                "`{shown}` has {lines} lines, over the limit of {limit} lines a source file may have"
            ),
            SPLIT,
        ));
    }
    let bytes = text.len() as u64;
    let max = max_file_bytes();
    if bytes > max {
        return Err(file_diag(
            "file-too-long",
            shown,
            (1, 0, 0),
            format!(
                "`{shown}` is {bytes} bytes, over the limit of {max} bytes a source file may have (its {limit} lines are not the problem: the lines are too long)"
            ),
            SPLIT,
        ));
    }
    Ok(())
}

/// What the text of a line is, as far as the lexer's three kinds of text go: code, a string, a block comment (they nest).
#[derive(Default)]
struct Scan {
    block: u32,
    in_str: bool,
}

enum Directive {
    No,
    Yes { path: String },
    Bad { column: u32, why: &'static str },
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether a line of code is an include line (first non-blank text `include "path";`, nothing after).
fn directive(line: &str) -> Directive {
    let t = line.trim_start();
    let lead = (line.chars().count() - t.chars().count()) as u32;
    let Some(rest) = t.strip_prefix("include") else {
        return Directive::No;
    };
    if !rest.starts_with(|c: char| c == '"' || (c.is_whitespace() && c != '\n')) {
        return Directive::No;
    }
    let after = rest.trim_start_matches([' ', '\t', '\r']);
    let Some(body) = after.strip_prefix('"') else {
        return Directive::No;
    };
    let Some((path, tail)) = body.split_once('"') else {
        return Directive::Bad {
            column: lead,
            why: "the path is not closed with a `\"` on the line",
        };
    };
    let tail = tail.trim_start_matches([' ', '\t']);
    let Some(tail) = tail.strip_prefix(';') else {
        return Directive::Bad {
            column: lead,
            why: "the path is not followed by `;`",
        };
    };
    if tail.trim().is_empty() {
        Directive::Yes {
            path: path.to_string(),
        }
    } else {
        Directive::Bad {
            column: lead,
            why: "something follows the `;` on the line",
        }
    }
}

struct Expander {
    root: PathBuf,
    canon_root: PathBuf,
    out: String,
    origin: Vec<(u32, u32)>,
    names: Vec<String>,
    name_ix: HashMap<String, u32>,
    scan: Scan,
    /// The real paths of the parts being expanded, outermost first.
    stack: Vec<PathBuf>,
    any: bool,
    infos: Vec<FileInfo>,
    cap: u64,
    /// The include line of the entry's own file that is being expanded (for `part-expansion-too-large`).
    site: Option<(String, u32, u32)>,
}

impl Expander {
    fn run(
        &mut self,
        text: &str,
        file_ix: u32,
        shown: &str,
        top: bool,
    ) -> Result<(), Box<Diagnostic>> {
        for (n, line) in text.split_inclusive('\n').enumerate() {
            let line_no = (n + 1) as u32;
            if self.scan.block == 0 && !self.scan.in_str {
                match directive(line) {
                    Directive::No => {}
                    Directive::Bad { column, why } => {
                        return Err(file_diag(
                            "part-not-standalone-line",
                            shown,
                            (line_no, column, column + 7),
                            format!(
                                "an `include` must be a line of its own, `include \"path.part\";`: {why}"
                            ),
                            "Write the include on a line by itself: `include \"parts/name.part\";`.",
                        ));
                    }
                    Directive::Yes { path } => {
                        let col = (line.chars().count() - line.trim_start().chars().count()) as u32;
                        self.include(&path, shown, line_no, col, top)?;
                        continue;
                    }
                }
            }
            self.scan_line(line, shown, line_no)?;
            self.emit(line, file_ix, line_no, top)?;
        }
        Ok(())
    }

    fn emit(
        &mut self,
        line: &str,
        file_ix: u32,
        line_no: u32,
        top: bool,
    ) -> Result<(), Box<Diagnostic>> {
        if self.origin.len() as u64 >= self.cap {
            let (f, l, c) = self
                .site
                .clone()
                .unwrap_or((self.names[0].clone(), line_no, 0));
            return Err(file_diag(
                "part-expansion-too-large",
                &f,
                (l, c, c + 7),
                format!(
                    "the expanded text of `{}` is over {} lines: the includes expand to more than a file may hold",
                    self.names[0], self.cap
                ),
                "Include fewer parts, or each part once; a part is the text of a file, written where the include line is.",
            ));
        }
        self.out.push_str(line);
        if !line.ends_with('\n') && !top {
            self.out.push('\n');
        }
        self.origin.push((file_ix, line_no));
        Ok(())
    }

    /// Updates what the text is (string, comment) over one line of code, and refuses the word `include` before a string in the middle of a line.
    fn scan_line(&mut self, line: &str, shown: &str, line_no: u32) -> Result<(), Box<Diagnostic>> {
        let cs: Vec<char> = line.chars().collect();
        let at = |i: usize| cs.get(i).copied();
        let mut i = 0;
        while i < cs.len() {
            let c = cs[i];
            if self.scan.in_str {
                if c == '\\' {
                    i += 2;
                    continue;
                }
                if c == '"' {
                    self.scan.in_str = false;
                }
                i += 1;
            } else if self.scan.block > 0 {
                if c == '/' && at(i + 1) == Some('*') {
                    self.scan.block += 1;
                    i += 2;
                } else if c == '*' && at(i + 1) == Some('/') {
                    self.scan.block -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            } else if c == '/' && at(i + 1) == Some('/') {
                break;
            } else if c == '/' && at(i + 1) == Some('*') {
                self.scan.block = 1;
                i += 2;
            } else if c == '"' {
                self.scan.in_str = true;
                i += 1;
            } else if is_word(c) {
                let start = i;
                while i < cs.len() && is_word(cs[i]) {
                    i += 1;
                }
                if cs[start..i].iter().collect::<String>() == "include" {
                    let mut k = i;
                    while k < cs.len() && (cs[k] == ' ' || cs[k] == '\t') {
                        k += 1;
                    }
                    if at(k) == Some('"') {
                        return Err(file_diag(
                            "part-not-standalone-line",
                            shown,
                            (line_no, start as u32, i as u32),
                            "an `include` must be a line of its own, `include \"path.part\";`: it follows other text on its line".to_string(),
                            "Write the include on a line by itself: `include \"parts/name.part\";`.",
                        ));
                    }
                }
            } else {
                i += 1;
            }
        }
        Ok(())
    }

    fn include(
        &mut self,
        rel: &str,
        shown: &str,
        line_no: u32,
        col: u32,
        top: bool,
    ) -> Result<(), Box<Diagnostic>> {
        let here = (line_no, col, col + 7);
        let bad = |why: String| {
            file_diag(
                "part-path-invalid",
                shown,
                here,
                format!("`include \"{rel}\";`: {why}"),
                "Name a file below the entry file's directory, relative to it, ending `.part`: `parts/name.part` (no `..`, no leading `/`).",
            )
        };
        if let Some(why) = path_problem(rel) {
            return Err(bad(why));
        }
        if top {
            self.site = Some((shown.to_string(), line_no, col));
        }
        if self.stack.len() >= MAX_INCLUDE_DEPTH {
            return Err(file_diag(
                "part-nested-too-deep",
                shown,
                here,
                format!(
                    "`include \"{rel}\";` is more than {MAX_INCLUDE_DEPTH} levels of include deep (a part that includes a part that includes a part ...)"
                ),
                "Flatten the parts: include them side by side from one file.",
            ));
        }
        let full = self.root.join(rel);
        let missing = |why: String| {
            file_diag(
                "part-missing",
                shown,
                here,
                format!(
                    "`include \"{rel}\";` names `{}`, which {why}",
                    full.display()
                ),
                "Create the part file at that path (relative to the entry file's directory) or correct the path.",
            )
        };
        let Ok(canon) = full.canonicalize() else {
            return Err(missing("does not exist".to_string()));
        };
        if !canon.is_file() {
            return Err(missing("is not a file".to_string()));
        }
        if !canon.starts_with(&self.canon_root) {
            return Err(bad(format!(
                "`{}` is, by its real path `{}`, outside the entry file's directory",
                full.display(),
                canon.display()
            )));
        }
        if let Some(pos) = self.stack.iter().position(|p| *p == canon) {
            let mut chain: Vec<String> = self.stack[pos..]
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            chain.push(canon.display().to_string());
            return Err(file_diag(
                "part-cycle",
                shown,
                here,
                format!(
                    "the includes cycle: {} — a part cannot contain itself",
                    chain.join(" -> ")
                ),
                "Remove the include that closes the cycle.",
            ));
        }
        let text = std::fs::read_to_string(&canon)
            .map_err(|e| missing(format!("cannot be read as UTF-8 text: {e}")))?;
        let part = if self.root == Path::new(".") {
            rel.to_string()
        } else {
            full.display().to_string()
        };
        check_budget(&text, &part)?;
        self.any = true;
        self.infos.push(FileInfo {
            path: part.clone(),
            role: Role::Part,
            lines: text.lines().count(),
            bytes: text.len(),
            included_by: Some(shown.to_string()),
        });
        let ix = match self.name_ix.get(&part) {
            Some(&i) => i,
            None => {
                let i = self.names.len() as u32;
                self.names.push(part.clone());
                self.name_ix.insert(part.clone(), i);
                i
            }
        };
        self.stack.push(canon);
        let r = self.run(&text, ix, &part, false);
        self.stack.pop();
        r
    }
}

/// Why an include path is not one (docs/file-budget.md section 2.2), or nothing.
fn path_problem(rel: &str) -> Option<String> {
    if rel.is_empty() {
        return Some("the path is empty".to_string());
    }
    if rel.starts_with('/') {
        return Some("the path is absolute".to_string());
    }
    if let Some(c) = rel
        .chars()
        .find(|c| *c == '\\' || *c == ':' || c.is_control())
    {
        return Some(format!(
            "the path has {c:?}, which a path of a part does not"
        ));
    }
    for seg in rel.split('/') {
        match seg {
            "" => return Some("the path has an empty segment".to_string()),
            "." | ".." => return Some(format!("the path has a `{seg}` segment")),
            _ => {}
        }
    }
    match rel.strip_suffix(".part") {
        Some(stem) if !stem.is_empty() && !stem.ends_with('/') => None,
        _ => Some("the path does not end in `.part` with a name before it".to_string()),
    }
}

#[derive(Serialize)]
struct SourcesOut<'a> {
    entry: String,
    files: &'a [FileInfo],
    text: &'a str,
}

/// `cancho-ui inspect sources <entry.cui> [--output json]`: the expanded text of the entry, exactly what the lexer is given.
pub fn inspect(file: &Path, json: bool, out: Option<&Path>) -> Result<ExitCode, CliError> {
    let mut table = Table::new(crate::pipeline::program_dir(file));
    let text = match load(file, Role::Entry, &mut table) {
        Ok(t) => t,
        Err(LoadError::Io(e)) => {
            return Err(CliError::Env(format!(
                "reading `{}`: {}",
                file.display(),
                e
            )));
        }
        Err(LoadError::Refused(d)) => {
            if json {
                println!("{}", cancho_ui_ir::pretty_json(&[&*d]));
            } else {
                eprintln!("{}", crate::pipeline::format_diagnostic(&d, file));
            }
            return Ok(ExitCode::from(1));
        }
    };
    // the modules the entry imports (best effort: a file that does not parse lists no imports), each with its own parts
    let mut seen: Vec<PathBuf> = vec![file.to_path_buf()];
    let mut queue: Vec<(PathBuf, String)> = vec![(file.to_path_buf(), text.clone())];
    while let Some((path, src)) = queue.pop() {
        let Ok(toks) = cancho_ui_syntax::lexer::lex(&src) else {
            continue;
        };
        let Ok(ast) = cancho_ui_syntax::parser::parse(toks) else {
            continue;
        };
        for imp in &ast.imports {
            let m = crate::pipeline::program_dir(&path).join(format!("{}.cui", imp.module));
            if seen.contains(&m) || seen.len() > 256 {
                continue;
            }
            seen.push(m.clone());
            match load(&m, Role::Module, &mut table) {
                Ok(t) => queue.push((m, t)),
                Err(LoadError::Io(_)) => {}
                Err(LoadError::Refused(d)) => {
                    eprintln!("{}", crate::pipeline::format_diagnostic(&d, &m));
                    return Ok(ExitCode::from(1));
                }
            }
        }
    }
    let body = if json {
        cancho_ui_ir::pretty_json(&SourcesOut {
            entry: file.display().to_string(),
            files: &table.infos,
            text: &text,
        })
    } else {
        text
    };
    match out {
        Some(p) => std::fs::write(p, &body)
            .map_err(|e| CliError::Env(format!("writing `{}`: {}", p.display(), e)))?,
        None => print!("{body}"),
    }
    Ok(ExitCode::from(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_parse() {
        assert_eq!(parse_limit("2000"), Ok(2000));
        assert_eq!(parse_limit("007"), Ok(7));
        assert_eq!(parse_limit("99999999999999999999999"), Ok(u64::MAX));
        for bad in ["", "0", "-1", "+5", "5 ", "abc", "1e3", "0000"] {
            assert!(parse_limit(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn directives() {
        assert!(matches!(
            directive("  include \"a.part\";\n"),
            Directive::Yes { .. }
        ));
        assert!(matches!(
            directive("include \"a.part\" ;\r\n"),
            Directive::Yes { .. }
        ));
        assert!(matches!(directive("include = 3;\n"), Directive::No));
        assert!(matches!(directive("includes \"a\";\n"), Directive::No));
        assert!(matches!(
            directive("include \"a.part\"; x\n"),
            Directive::Bad { .. }
        ));
        assert!(matches!(
            directive("include \"a.part\"\n"),
            Directive::Bad { .. }
        ));
    }

    #[test]
    fn bytes_scale() {
        assert_eq!(DEFAULT_MAX_FILE_BYTES, 163_840);
    }

    /// CONTRIBUTING.md section 6.1 for `include`: the splice runs before the lexer, so every prefix and every one-character deletion of the example
    /// program and of each of its parts goes through the expansion and the front end, and none of them is a panic. (The same mutants of the entry
    /// without its parts are run by `tests/no_panic.rs`, where the include lines are text the parser refuses.)
    #[test]
    fn mutated_include_programs_never_panic() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/parts");
        let names = [
            "parts.cui",
            "parts/state.part",
            "parts/handlers.part",
            "parts/view/header.part",
            "parts/view/buttons.part",
        ];
        let tmp =
            std::env::temp_dir().join(format!("cancho-ui-include-mut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut texts = Vec::new();
        for n in names {
            let t = std::fs::read_to_string(root.join(n)).expect("an example file");
            let p = tmp.join(n);
            std::fs::create_dir_all(p.parent().expect("a parent")).expect("a directory");
            std::fs::write(&p, &t).expect("write");
            texts.push(t);
        }
        let entry = tmp.join("parts.cui");
        let (mut runs, mut expanded, mut failures) = (0usize, 0usize, Vec::new());
        for (ix, n) in names.iter().enumerate() {
            let target = tmp.join(n);
            let src = &texts[ix];
            for (i, _) in src.char_indices() {
                let mut del = src.clone();
                del.remove(i);
                for (kind, text) in [("prefix", src[..i].to_string()), ("deletion", del)] {
                    std::fs::write(&target, &text).expect("write");
                    runs += 1;
                    let r = catch_unwind(AssertUnwindSafe(|| {
                        let mut table = Table::new(&tmp);
                        let Ok(t) = load(&entry, Role::Entry, &mut table) else {
                            return false;
                        };
                        let Ok(toks) = cancho_ui_syntax::lexer::lex(&t) else {
                            return true;
                        };
                        let Ok(ast) = cancho_ui_syntax::parser::parse(toks) else {
                            return true;
                        };
                        if cancho_ui_types::check_file(&ast).is_ok() {
                            let _ = cancho_ui_ir::lower_file(&ast);
                        }
                        true
                    }));
                    match r {
                        Ok(true) => expanded += 1,
                        Ok(false) => {}
                        Err(_) => failures.push(format!("{n} {kind} at byte {i}")),
                    }
                }
            }
            std::fs::write(&target, src).expect("restore");
        }
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(failures.is_empty(), "panics:\n{}", failures.join("\n"));
        // the claim is worth something only if most mutants got through the expansion to the front end
        assert!(
            runs > 2000 && expanded * 2 > runs,
            "{expanded} of {runs} mutants expanded"
        );
    }

    /// Text that is hard for the scanner: strings and comments that open and do not close, an include line at the end of a file with no newline,
    /// a part that is a lone quote, bare `\r`, and a directive at the limits of its grammar.
    #[test]
    fn awkward_text_is_a_refusal_or_a_result_not_a_panic() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let tmp =
            std::env::temp_dir().join(format!("cancho-ui-include-awkward-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("a directory");
        std::fs::write(tmp.join("q.part"), "\"").expect("write");
        std::fs::write(tmp.join("c.part"), "/*").expect("write");
        std::fs::write(tmp.join("b.part"), "\\").expect("write");
        let entries = [
            "include",
            "include \"",
            "include \"q.part",
            "include \"q.part\"",
            "include \"q.part\";",
            "include \"c.part\";\ninclude \"q.part\";",
            "\"\\\ninclude \"q.part\";",
            "/*/ include \"q.part\"; /*",
            "x include",
            "x include \"",
            "\r\rinclude \"b.part\";\r",
            "include \"b.part\";\ninclude \"b.part\";",
            "  \t include\t\"q.part\"\t;\t",
            "include \"\u{e9}\u{4e2d}.part\";",
            "include \"\";",
            "include \"\\\";",
        ];
        for e in entries {
            let entry = tmp.join("e.cui");
            std::fs::write(&entry, e).expect("write");
            let r = catch_unwind(AssertUnwindSafe(|| {
                let mut table = Table::new(&tmp);
                let _ = load(&entry, Role::Entry, &mut table);
            }));
            assert!(r.is_ok(), "panicked on {e:?}");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
