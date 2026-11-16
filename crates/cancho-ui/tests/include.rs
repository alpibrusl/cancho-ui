//! `include "x.part";` (docs/file-budget.md section 2): a compile-time splice of a part file, before anything reads the program. Every rule of the
//! section is run through the binary: the expansion is the program written in one file (byte for byte in the IR, the script and the page), a
//! refusal cites the part and the line in it, and each way an include can be wrong has its own tag.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_cancho-ui");

fn dir(name: &str) -> PathBuf {
    let d = common::scratch_root().join(format!("include-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch directory");
    d
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().expect("a parent")).expect("a directory");
    std::fs::write(p, text).expect("a file");
}

/// (exit code, stdout, stderr) of the binary run in `dir`, with the limit left at its default.
fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env_remove("CANCHO_UI_MAX_FILE_LINES")
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The first diagnostic of `check --output json` (the code, the file the span names, the line, the message).
fn refusal(dir: &Path, entry: &str) -> (String, String, u64, String) {
    let (code, out, err) = run(dir, &["check", entry, "--output", "json"]);
    assert_eq!(code, 1, "expected a refusal; stdout {out} stderr {err}");
    let v: Value = serde_json::from_str(&out).expect("JSON diagnostics");
    let d = &v[0];
    (
        d["code"].as_str().expect("code").to_string(),
        d["span"]["file"].as_str().unwrap_or("").to_string(),
        d["span"]["start"]["line"].as_u64().unwrap_or(0),
        d["message"].as_str().expect("message").to_string(),
    )
}

const ENTRY: &str = "component Tally {
    // state and handlers
    include \"parts/state.part\";
    include \"parts/handlers.part\";

    view {
        Column {
            include \"parts/view/header.part\";
            Row {
                include \"parts/view/buttons.part\";
            }
        }
    }
}
";
const STATE: &str = "state count: Int = 0;\nstate step: Int = 1;\n";
const HANDLERS: &str = "on Add {\n    count = count + step;\n}\n\non Reset {\n    count = 0;\n}\n";
const HEADER: &str = "Text {\n    role: status\n    \"Count: {count}\"\n}\n";
const BUTTONS: &str = "Button {\n    role: button\n    label: \"Add\"\n    on: Add\n}\nButton {\n    role: button\n    label: \"Reset\"\n    on: Reset\n}\n";

fn tally(d: &Path) {
    write(d, "app.cui", ENTRY);
    write(d, "parts/state.part", STATE);
    write(d, "parts/handlers.part", HANDLERS);
    write(d, "parts/view/header.part", HEADER);
    write(d, "parts/view/buttons.part", BUTTONS);
}

/// The model of the rule, written again here: each include line replaced by the lines of its part, and a newline added when a part lacks one.
fn expand_model(d: &Path, text: &str) -> String {
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("include \"") {
            let rel = rest.strip_suffix("\";").expect("an include line");
            let part = std::fs::read_to_string(d.join(rel)).expect("a part");
            let part = expand_model(d, &part);
            out.push_str(&part);
            if !part.is_empty() && !part.ends_with('\n') {
                out.push('\n');
            }
        } else {
            out.push_str(line);
        }
    }
    out
}

fn tree(d: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(base: &Path, d: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(d).expect("a directory") {
            let p = e.expect("an entry").path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push((
                    p.strip_prefix(base)
                        .expect("under base")
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(&p).expect("a file"),
                ));
            }
        }
    }
    walk(d, d, &mut out);
    out.sort();
    out
}

/// Everything a program makes: the IR, the module, the page (index.html and app.js).
fn outputs(d: &Path) -> (String, String, Vec<(String, Vec<u8>)>) {
    let (c1, ir, e1) = run(d, &["inspect", "ir", "app.cui"]);
    assert_eq!(c1, 0, "{e1}");
    let (c2, js, e2) = run(d, &["build", "app.cui"]);
    assert_eq!(c2, 0, "{e2}");
    let (c3, _, e3) = run(
        d,
        &[
            "build", "app.cui", "--page", "-o", "site", "--name", "Tally",
        ],
    );
    assert_eq!(c3, 0, "{e3}");
    (ir, js, tree(&d.join("site")))
}

#[test]
fn split_program_is_the_inline_program_byte_for_byte() {
    let split = dir("identity-split");
    tally(&split);
    let model = expand_model(&split, ENTRY);
    // `inspect sources` prints exactly the model
    let (code, text, err) = run(&split, &["inspect", "sources", "app.cui"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(text, model, "the expanded text is the model's");
    // the same program written in one file
    let inline = dir("identity-inline");
    write(&inline, "app.cui", &model);
    let (a, b) = (outputs(&split), outputs(&inline));
    assert!(a.0.len() > 500, "the IR is not empty");
    assert_eq!(a.0, b.0, "inspect ir");
    assert_eq!(a.1, b.1, "build (app.js)");
    assert_eq!(a.2.len(), b.2.len());
    assert!(a.2.iter().any(|(n, _)| n == "index.html") && a.2.iter().any(|(n, _)| n == "app.js"));
    for (x, y) in a.2.iter().zip(&b.2) {
        assert_eq!(x, y, "page file {}", x.0);
    }
}

#[test]
fn the_example_is_its_expansion_byte_for_byte() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/parts");
    let (code, text, err) = run(&root, &["inspect", "sources", "parts.cui"]);
    assert_eq!(code, 0, "{err}");
    let inline = dir("example-inline");
    write(&inline, "app.cui", &text);
    let split = dir("example-split");
    write(
        &split,
        "app.cui",
        &std::fs::read_to_string(root.join("parts.cui")).expect("entry"),
    );
    for p in ["state", "handlers", "view/header", "view/buttons"] {
        let t = std::fs::read_to_string(root.join(format!("parts/{p}.part"))).expect("a part");
        write(&split, &format!("parts/{p}.part"), &t);
    }
    assert_eq!(outputs(&split), outputs(&inline));
}

#[test]
fn parts_without_a_final_newline_empty_parts_and_crlf_are_the_inline_text() {
    let d = dir("odd-parts");
    write(
        &d,
        "app.cui",
        "component C {\r\n    include \"a.part\";\r\n    include \"empty.part\";\r\n    include \"b.part\";\r\n    view { Text { \"{n}\" } }\r\n}\r\n",
    );
    write(&d, "a.part", "state n: Int = 1;");
    write(&d, "empty.part", "");
    write(&d, "b.part", "on Go {\r\n    n = n + 1;\r\n}\r\n");
    let model = expand_model(
        &d,
        &std::fs::read_to_string(d.join("app.cui")).expect("entry"),
    );
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
    let (_, text, _) = run(&d, &["inspect", "sources", "app.cui"]);
    assert_eq!(text, model);
    let i = dir("odd-parts-inline");
    write(&i, "app.cui", &model);
    assert_eq!(outputs_ir(&d), outputs_ir(&i));
}

fn outputs_ir(d: &Path) -> String {
    let (c, ir, e) = run(d, &["inspect", "ir", "app.cui"]);
    assert_eq!(c, 0, "{e}");
    ir
}

#[test]
fn a_refusal_in_a_part_cites_the_part_and_its_line() {
    let d = dir("map-part");
    tally(&d);
    // line 5 of the part names a state field that is not there
    write(
        &d,
        "parts/handlers.part",
        "on Add {\n    count = count + step;\n}\n\non Reset {\n    count = nope;\n}\n",
    );
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert!(file.ends_with("parts/handlers.part"), "{file}");
    assert_eq!(line, 6, "{code}: {msg}");
}

#[test]
fn a_refusal_in_a_nested_view_part_and_in_the_entry_after_the_parts() {
    let d = dir("map-view");
    tally(&d);
    // an event the component does not have, line 4 of a view part that is two levels down in the view
    write(
        &d,
        "parts/view/buttons.part",
        "Button {\n    role: button\n    label: \"Add\"\n    on: Nope\n}\n",
    );
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert_eq!(code, "event-unknown");
    assert!(file.ends_with("parts/view/buttons.part"), "{file}: {msg}");
    assert_eq!(line, 4);
    // a refusal in the entry, below 20 lines of parts: its own line, not the line of the expanded text
    tally(&d);
    write(
        &d,
        "app.cui",
        &ENTRY.replace("    view {", "    on Bad { count = nope; }\n    view {"),
    );
    let (_, file, line, msg) = refusal(&d, "app.cui");
    assert!(file.ends_with("app.cui"), "{file}: {msg}");
    assert_eq!(line, 6, "{msg}");
}

#[test]
fn a_lex_error_and_a_parse_error_in_a_part_cite_the_part() {
    let d = dir("map-lex");
    tally(&d);
    write(
        &d,
        "parts/state.part",
        "state count: Int = 0;\nstate step: Int = 1 ?;\n",
    );
    let (code, file, line, _) = refusal(&d, "app.cui");
    assert!(
        code.starts_with("lex-") || code.starts_with("parse-"),
        "{code}"
    );
    assert!(file.ends_with("parts/state.part"), "{file}");
    assert_eq!(line, 2);
    write(
        &d,
        "parts/state.part",
        "state count: Int = 0;\nstate : Int = 1;\n",
    );
    let (code, file, line, _) = refusal(&d, "app.cui");
    assert!(code.starts_with("parse-"), "{code}");
    assert!(file.ends_with("parts/state.part"), "{file}");
    assert_eq!(line, 2);
}

#[test]
fn a_part_that_includes_a_part_cites_the_inner_file() {
    let d = dir("map-nested");
    tally(&d);
    write(
        &d,
        "parts/handlers.part",
        "on Add {\n    count = count + step;\n}\ninclude \"parts/more.part\";\n",
    );
    write(
        &d,
        "parts/more.part",
        "// a comment\n\non Reset {\n    count = nope;\n}\n",
    );
    let (_, file, line, _) = refusal(&d, "app.cui");
    assert!(file.ends_with("parts/more.part"), "{file}");
    assert_eq!(line, 4);
}

#[test]
fn a_module_may_include_parts_and_its_refusals_cite_them() {
    let d = dir("module");
    write(
        &d,
        "cards.cui",
        "module cards;\n\ncomponent Card {\n    include \"parts/card-state.part\";\n    view { Text { \"{n}\" } }\n}\n",
    );
    write(
        &d,
        "parts/card-state.part",
        "state n: Int = 0;\non Go { n = nope; }\n",
    );
    write(
        &d,
        "app.cui",
        "module app;\nimport cards.Card;\n\ncomponent Page {\n    view { Card { } }\n}\n",
    );
    let (_, file, line, msg) = refusal(&d, "app.cui");
    assert!(file.ends_with("parts/card-state.part"), "{file}: {msg}");
    assert_eq!(line, 2);
    write(&d, "parts/card-state.part", "state n: Int = 0;\n");
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
}

#[test]
fn part_missing_for_a_file_that_is_not_there_a_directory_and_text_that_is_not_utf8() {
    let d = dir("missing");
    tally(&d);
    std::fs::remove_file(d.join("parts/state.part")).expect("remove");
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert_eq!((code.as_str(), line), ("part-missing", 3), "{msg}");
    assert!(file.ends_with("app.cui"));
    // a directory called x.part
    std::fs::create_dir_all(d.join("parts/state.part")).expect("a directory");
    assert_eq!(refusal(&d, "app.cui").0, "part-missing");
    std::fs::remove_dir_all(d.join("parts/state.part")).expect("remove");
    // bytes that are not UTF-8
    std::fs::write(d.join("parts/state.part"), [0xff, 0xfe, 0x00, b'x']).expect("write");
    assert_eq!(refusal(&d, "app.cui").0, "part-missing");
}

#[test]
fn part_path_invalid_for_every_way_out_of_the_directory_and_every_malformed_path() {
    let d = dir("paths");
    write(
        &d,
        "sub/app.cui",
        "component C {\n    include \"PATH\";\n    view { Text { \"x\" } }\n}\n",
    );
    write(&d, "sub/ok.part", "state n: Int = 0;\n");
    write(&d, "outside.part", "state n: Int = 0;\n");
    let cases = [
        "../outside.part",
        "parts/../ok.part",
        "./ok.part",
        "/etc/passwd.part",
        "a//b.part",
        "a\\\\b.part",
        "ok.txt",
        "ok",
        ".part",
        "dir/.part",
        "",
        "C:x.part",
    ];
    for c in cases {
        let t = std::fs::read_to_string(d.join("sub/app.cui")).expect("entry");
        let p = d.join("sub/prog.cui");
        std::fs::write(&p, t.replace("PATH", c)).expect("write");
        let (code, _, line, msg) = refusal(&d, "sub/prog.cui");
        assert_eq!(
            (code.as_str(), line),
            ("part-path-invalid", 2),
            "path {c:?}: {msg}"
        );
    }
    // the same program with a good path compiles
    write(
        &d,
        "sub/good.cui",
        &std::fs::read_to_string(d.join("sub/app.cui"))
            .expect("entry")
            .replace("PATH", "ok.part"),
    );
    assert_eq!(run(&d, &["check", "sub/good.cui"]).0, 0);
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_out_of_the_directory_is_refused_and_one_inside_is_followed() {
    use std::os::unix::fs::symlink;
    let d = dir("symlink");
    write(
        &d,
        "sub/app.cui",
        "component C {\n    include \"link.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    write(&d, "outside.part", "state n: Int = 0;\n");
    symlink(d.join("outside.part"), d.join("sub/link.part")).expect("a link");
    let (code, _, line, msg) = refusal(&d, "sub/app.cui");
    assert_eq!((code.as_str(), line), ("part-path-invalid", 2), "{msg}");
    assert!(msg.contains("outside"), "{msg}");
    // a link to a directory out of it, and a file below the link
    std::fs::remove_file(d.join("sub/link.part")).expect("rm");
    std::fs::create_dir_all(d.join("elsewhere")).expect("dir");
    write(&d, "elsewhere/x.part", "state n: Int = 0;\n");
    symlink(d.join("elsewhere"), d.join("sub/dirlink")).expect("a link");
    write(
        &d,
        "sub/app.cui",
        "component C {\n    include \"dirlink/x.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    assert_eq!(refusal(&d, "sub/app.cui").0, "part-path-invalid");
    // a link to a file inside the directory is the file
    std::fs::remove_file(d.join("sub/dirlink")).expect("rm");
    write(&d, "sub/real.part", "state n: Int = 0;\n");
    symlink(d.join("sub/real.part"), d.join("sub/inside.part")).expect("a link");
    write(
        &d,
        "sub/app.cui",
        "component C {\n    include \"inside.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    assert_eq!(run(&d, &["check", "sub/app.cui"]).0, 0);
    // a dangling link is a missing part
    std::fs::remove_file(d.join("sub/real.part")).expect("rm");
    assert_eq!(refusal(&d, "sub/app.cui").0, "part-missing");
}

#[test]
fn part_cycle_direct_through_two_parts_and_a_part_that_includes_itself() {
    let d = dir("cycle");
    write(
        &d,
        "app.cui",
        "component C {\n    include \"a.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    write(&d, "a.part", "include \"b.part\";\n");
    write(&d, "b.part", "state n: Int = 0;\ninclude \"a.part\";\n");
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert_eq!(code, "part-cycle", "{msg}");
    assert!(file.ends_with("b.part"), "{file}");
    assert_eq!(line, 2);
    assert!(msg.contains("a.part") && msg.contains("b.part"), "{msg}");
    write(&d, "a.part", "include \"a.part\";\n");
    assert_eq!(refusal(&d, "app.cui").0, "part-cycle");
}

#[test]
fn the_same_part_twice_is_not_a_cycle_it_is_the_text_twice() {
    let d = dir("twice");
    write(
        &d,
        "app.cui",
        "component C {\n    include \"a.part\";\n    include \"a.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    write(&d, "a.part", "state n: Int = 0;\n");
    // the text twice, as written in place twice (the compiler does not refuse a state field declared twice: that is the language's rule, not the include's)
    let (code, text, err) = run(&d, &["inspect", "sources", "app.cui"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(text.matches("state n: Int = 0;").count(), 2);
}

/// A chain of `n` parts, each including the next; the last declares a state field.
fn chain(d: &Path, n: usize) {
    write(
        d,
        "app.cui",
        "component C {\n    include \"p1.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    for i in 1..n {
        write(
            d,
            &format!("p{i}.part"),
            &format!("include \"p{}.part\";\n", i + 1),
        );
    }
    write(d, &format!("p{n}.part"), "state n: Int = 0;\n");
}

#[test]
fn includes_nest_eight_deep_and_not_nine() {
    let d = dir("depth");
    chain(&d, 8);
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
    chain(&d, 9);
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert_eq!(code, "part-nested-too-deep", "{msg}");
    assert!(file.ends_with("p8.part"), "{file}");
    assert_eq!(line, 1);
}

#[test]
fn an_include_that_is_not_a_whole_line_is_refused() {
    let d = dir("standalone");
    let cases = [
        (
            "    include \"a.part\"; state m: Int = 0;",
            "something after the semicolon",
        ),
        (
            "    state m: Int = 0; include \"a.part\";",
            "after other code",
        ),
        ("    include \"a.part\"", "no semicolon"),
        ("    include \"a.part", "no closing quote"),
        (
            "    include \"a.part\" x;",
            "text between the string and the semicolon",
        ),
    ];
    for (line, why) in cases {
        write(
            &d,
            "app.cui",
            &format!("component C {{\n{line}\n    view {{ Text {{ \"x\" }} }}\n}}\n"),
        );
        write(&d, "a.part", "state n: Int = 0;\n");
        let (code, _, l, msg) = refusal(&d, "app.cui");
        assert_eq!(
            (code.as_str(), l),
            ("part-not-standalone-line", 2),
            "{why}: {msg}"
        );
    }
    // in a view, after a node
    write(
        &d,
        "app.cui",
        "component C {\n    view { Column { Text { \"x\" } include \"a.part\"; } }\n}\n",
    );
    assert_eq!(refusal(&d, "app.cui").0, "part-not-standalone-line");
}

#[test]
fn the_word_is_text_in_a_string_in_a_comment_and_a_name() {
    let d = dir("inert");
    write(&d, "a.part", "this is not a program\n");
    let prog = "component C {\n    // include \"a.part\";\n    /* a block comment\n    include \"a.part\";\n    */\n    state s: String = \"\";\n    state include: Int = 0;\n    on Go {\n        include = include + 1;\n        s = \"line one\n        include \\\"a.part\\\";\";\n    }\n    view { Text { \"include \\\"a.part\\\";\" } }\n}\n";
    write(&d, "app.cui", prog);
    let (code, _, err) = run(&d, &["check", "app.cui"]);
    assert_eq!(code, 0, "{err}");
    let (_, text, _) = run(&d, &["inspect", "sources", "app.cui"]);
    assert_eq!(text, prog, "nothing was spliced");
}

#[test]
fn a_string_that_spans_lines_hides_an_include_line() {
    let d = dir("string-across");
    // the line `include "nope.part";` is inside a string that began on the line above (the lexer closes it at the first quote): nothing is spliced, and
    // the answer is the lexer's or the parser's and not `part-missing`
    write(
        &d,
        "app.cui",
        "component C {\n    state s: String = \"first\ninclude \"nope.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    let (code, _, line, msg) = refusal(&d, "app.cui");
    assert!(!code.starts_with("part-"), "{code}: {msg}");
    assert!(line >= 2, "{line}");
}

#[test]
fn a_part_that_opens_a_comment_continues_it_in_the_including_file() {
    let d = dir("comment-across");
    write(
        &d,
        "app.cui",
        "component C {\n    include \"open.part\";\n    include \"a.part\";\n    */\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n",
    );
    write(
        &d,
        "open.part",
        "/* a comment that is closed in the including file\n",
    );
    write(
        &d,
        "a.part",
        "this is inside the comment, and an include is text there\n",
    );
    // as written in one file: the comment swallows the two lines, so the program is valid
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
}

#[test]
fn an_expansion_that_is_too_big_is_refused_not_built() {
    let d = dir("bomb");
    write(
        &d,
        "app.cui",
        "component C {\n    include \"l0.part\";\n    view { Text { \"x\" } }\n}\n",
    );
    // each level includes the next ten times: 10^7 lines at the bottom
    for i in 0..7 {
        let body: String = (0..10)
            .map(|_| format!("include \"l{}.part\";\n", i + 1))
            .collect();
        write(&d, &format!("l{i}.part"), &body);
    }
    write(&d, "l7.part", "// text\n");
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert_eq!(
        (code.as_str(), line),
        ("part-expansion-too-large", 2),
        "{msg}"
    );
    assert!(file.ends_with("app.cui"));
}

#[test]
fn inspect_sources_lists_the_files_and_needs_no_type_check() {
    let d = dir("sources");
    tally(&d);
    // the program does not type-check; its text is still printed
    write(
        &d,
        "parts/handlers.part",
        "on Add {\n    count = nope;\n}\n",
    );
    assert_eq!(run(&d, &["check", "app.cui"]).0, 1);
    let (code, out, err) = run(&d, &["inspect", "sources", "app.cui", "--output", "json"]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    let files = v["files"].as_array().expect("files");
    let roles: Vec<(&str, &str)> = files
        .iter()
        .map(|f| {
            (
                f["role"].as_str().expect("role"),
                f["path"].as_str().expect("path"),
            )
        })
        .collect();
    assert_eq!(roles[0], ("entry", "app.cui"));
    let parts: Vec<&str> = roles
        .iter()
        .filter(|r| r.0 == "part")
        .map(|r| r.1)
        .collect();
    assert_eq!(
        parts,
        [
            "parts/state.part",
            "parts/handlers.part",
            "parts/view/header.part",
            "parts/view/buttons.part"
        ],
        "in include order"
    );
    assert!(v["text"].as_str().expect("text").contains("count = nope;"));
    assert_eq!(files[1]["included_by"], "app.cui");
    // a program that cannot be expanded is a refusal, exit 1
    write(&d, "parts/state.part", "");
    std::fs::remove_file(d.join("parts/state.part")).expect("rm");
    assert_eq!(run(&d, &["inspect", "sources", "app.cui"]).0, 1);
    // a file with no include is its own text
    write(&d, "plain.cui", "component C { view { Text { \"x\" } } }\n");
    assert_eq!(
        run(&d, &["inspect", "sources", "plain.cui"]).1,
        "component C { view { Text { \"x\" } } }\n"
    );
}

#[test]
fn every_command_that_compiles_expands_includes() {
    let d = dir("commands");
    tally(&d);
    write(
        &d,
        "trace.json",
        "{\"schema\":\"cancho-ui-trace\",\"version\":\"0.1.0\",\"steps\":[{\"event\":\"Add\",\"state\":{\"count\":1}}]}",
    );
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
    assert_eq!(run(&d, &["build", "app.cui"]).0, 0);
    assert_eq!(run(&d, &["inspect", "component", "app.cui", "Tally"]).0, 0);
    assert_eq!(run(&d, &["report", "app.cui"]).0, 0);
    assert_eq!(run(&d, &["test", "app.cui", "trace.json"]).0, 0);
    // the same program with the text moved is no change to `diff`
    let i = dir("commands-inline");
    write(&i, "app.cui", &expand_model(&d, ENTRY));
    let (c, out, err) = run(
        &d,
        &["diff", "app.cui", "../include-commands-inline/app.cui"],
    );
    assert_eq!(c, 0, "{out}{err}");
}

#[test]
fn the_example_check_and_the_trace_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(run(&root, &["check", "examples/parts/parts.cui"]).0, 0);
    let (c, out, err) = run(
        &root,
        &[
            "test",
            "examples/parts/parts.cui",
            "examples/parts/parts.trace.json",
        ],
    );
    assert_eq!(c, 0, "{out}{err}");
}

#[test]
fn a_part_over_the_limit_is_file_too_long_and_names_the_part() {
    let d = dir("part-long");
    tally(&d);
    let long: String = (0..2001).map(|i| format!("// line {i}\n")).collect();
    write(&d, "parts/handlers.part", &format!("{long}{HANDLERS}"));
    let (code, file, line, msg) = refusal(&d, "app.cui");
    assert_eq!(code, "file-too-long");
    assert!(file.ends_with("parts/handlers.part"), "{file}");
    assert_eq!(line, 2001);
    assert!(msg.contains("2008 lines") && msg.contains("2000"), "{msg}");
    // exactly at the limit is fine
    let ok: String = (0..1993).map(|i| format!("// line {i}\n")).collect();
    write(&d, "parts/handlers.part", &format!("{ok}{HANDLERS}"));
    assert_eq!(run(&d, &["check", "app.cui"]).0, 0);
}
