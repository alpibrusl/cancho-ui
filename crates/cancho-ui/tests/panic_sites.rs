//! "No input may reach a panic" (#62), checked in the source: outside `#[cfg(test)]`, no `crates/*/src` file contains
//! `unreachable!`, `panic!`, `todo!`, `unimplemented!`, `.unwrap()` or `.expect("`. A construction that must hold is
//! written so that the invalid state cannot be represented (a total `match`, `try_from` into an array) or answers a
//! refusal; if one really cannot, it goes in `ALLOWED` below with the invariant in a sentence. The list is empty.
//!
//! Also: the paths that replaced the old panic sites are reached by real input (the CLI usage errors, the parser's
//! constructor-arity refusals, the lexer at the end of the source).
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// (file suffix, the exact trimmed line, the invariant that makes it safe). Empty on purpose.
const ALLOWED: &[(&str, &str, &str)] = &[];

const FORBIDDEN: &[&str] = &[
    "unreachable!",
    "panic!(",
    "todo!",
    "unimplemented!",
    ".unwrap()",
    ".expect(\"",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a readable source directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_panic_construct_outside_tests() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for c in std::fs::read_dir(&crates).expect("the crates directory") {
        let src = c.expect("a crate entry").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    assert!(files.len() > 20, "found only {} source files", files.len());
    let mut found = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).expect("a UTF-8 source file");
        for (n, line) in text.lines().enumerate() {
            let t = line.trim();
            if t == "#[cfg(test)]" {
                break; // the rest of the file is the test module
            }
            if t.starts_with("//") {
                continue;
            }
            if FORBIDDEN.iter().any(|p| t.contains(p))
                && !ALLOWED
                    .iter()
                    .any(|(suffix, l, _)| f.ends_with(suffix) && *l == t)
            {
                found.push(format!("{}:{}: {}", f.display(), n + 1, t));
            }
        }
    }
    assert!(
        found.is_empty(),
        "panic constructs outside tests:\n{}",
        found.join("\n")
    );
}

fn run(args: &[&str]) -> (i32, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn a_wrong_number_of_files_is_a_usage_error_not_a_panic() {
    for args in [
        vec!["diff"],
        vec!["diff", "a.cui"],
        vec!["diff", "a.cui", "b.cui", "c.cui"],
        vec!["test"],
        vec!["test", "a.cui"],
        vec!["test", "a.cui", "b.json", "c.json"],
    ] {
        let (code, text) = run(&args);
        assert_eq!(code, 2, "{args:?}: {text}");
        assert!(text.contains("takes exactly"), "{args:?}: {text}");
        assert!(!text.contains("panicked"), "{args:?}: {text}");
    }
}

#[test]
fn pwa_without_an_output_directory_is_a_usage_error_not_a_panic() {
    let (code, text) = run(&["build", "examples/counter/counter.cui", "--pwa"]);
    assert!(!text.contains("panicked"), "{text}");
    assert_eq!(code, 2, "{text}");
}

#[test]
fn constructors_with_the_wrong_arity_are_refused_by_the_parser() {
    for (name, call) in [
        ("Ok none", "Ok()"),
        ("Ok two", "Ok(1, 2)"),
        ("Err none", "Err()"),
        ("Err two", "Err(1, 2)"),
        ("Some none", "Some()"),
        ("Some two", "Some(1, 2)"),
        ("None one", "None(1)"),
    ] {
        let src = format!(
            "component C {{\n    state n: Int = 0;\n    on Go {{ n = {call}; }}\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n"
        );
        let dir = common::scratch_root().join(format!("cancho-ui-sites-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let path = dir.join("c.cui");
        std::fs::write(&path, src).expect("a scratch file");
        let (code, text) = run(&["check", path.to_str().expect("UTF-8 path")]);
        assert_eq!(code, 1, "{name}: {text}");
        assert!(!text.contains("panicked"), "{name}: {text}");
    }
}
