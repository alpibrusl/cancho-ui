//! `Button { copy: field }` (`docs/clipboard.md`): what the compiler emits, what it refuses, and that the semantic diff sees the capability. The browser-side
//! checks (the text reaches the system clipboard; nothing is written at load or at a render; a refused write says so) are `scripts/check-clipboard.js`, outside
//! the gate because they need a browser.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_diff::diff_ir;
use cancho_ui_ir::lower_file;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::Value;

const COPIER: &str = include_str!("../../../examples/clipboard/clipboard.cui");

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = common::scratch_root().join(format!("cancho-ui-clipboard-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn build_js(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

fn check_codes(src: &str, name: &str) -> Vec<String> {
    let file = scratch(name, src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

/// A component with the declaration `requires`, one `String` field and a copy button that names `copy`.
fn with(requires: &str, copy: &str) -> String {
    format!(
        "component A {{\n    {requires}\n    state text: String = \"\";\n    state n: Int = 0;\n    view {{ Button {{ label: \"Copy\" copy: {copy} }} }}\n}}\n"
    )
}

#[test]
fn the_adapter_is_emitted_for_a_program_with_a_copy_button_and_for_no_other() {
    let js = build_js(COPIER, "copier.cui");
    assert!(
        js.contains("function __cui_copy_text(button, text)"),
        "{js}"
    );
    assert!(
        js.contains("addEventListener('click', function () { __cui_copy_text(")
            && js.contains("__state_draft"),
        "{js}"
    );
    assert_eq!(
        js.matches("writeText(").count(),
        1,
        "the adapter's one call, and no other"
    );
    assert!(!js.contains("readText"), "write only");
    assert!(js.contains("1000000"), "the limit is in the adapter");
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    let other = build_js(&counter, "counter.cui");
    assert!(
        !other.contains("__cui_copy_text") && !other.contains("writeText"),
        "a program without a copy button carries no adapter"
    );
    // a code box's copy button is the old adapter, not this one
    let code =
        "component A {\n    requires Clipboard;\n    view { Code { copy: true \"x\" } }\n}\n";
    let boxed = build_js(code, "box.cui");
    assert!(
        boxed.contains("__cui_copy(") && !boxed.contains("__cui_copy_text"),
        "{boxed}"
    );
    assert_eq!(boxed.matches("writeText(").count(), 1);
}

#[test]
fn a_button_without_the_declaration_is_clipboard_not_declared_and_a_declaration_without_use_is_unused()
 {
    assert_eq!(
        check_codes(&with("", "text"), "undeclared.cui"),
        ["clipboard-not-declared"]
    );
    let unused = "component A {\n    requires Clipboard;\n    view { Text { \"x\" } }\n}\n";
    assert_eq!(check_codes(unused, "unused.cui"), ["capability-unused"]);
}

#[test]
fn a_copy_that_is_not_a_string_field_is_copy_not_string() {
    for (name, copy) in [
        ("int.cui", "n"),
        ("lit.cui", "\"hello\""),
        ("none.cui", "nothing"),
    ] {
        let src = with("requires Clipboard;", copy);
        assert_eq!(check_codes(&src, name), ["copy-not-string"], "{copy}");
    }
}

#[test]
fn copy_on_a_column_is_binding_unknown_and_says_where_it_belongs() {
    let src = "component A {\n    requires Clipboard;\n    view { Column { copy: true Text { \"x\" } } }\n}\n";
    let file = scratch("text.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    let first = &diags.as_array().expect("a list")[0];
    assert_eq!(first["code"], "binding-unknown", "{stdout}");
    assert!(
        first["message"]
            .as_str()
            .expect("a message")
            .contains("Code and Button"),
        "{stdout}"
    );
}

#[test]
fn the_example_trace_replays_and_passes() {
    let (code, stdout, stderr) = run(&[
        "test",
        "examples/clipboard/clipboard.cui",
        "examples/clipboard/clipboard.trace.json",
    ]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("PASS"), "{stdout}");
}

#[test]
fn the_semantic_diff_reports_clipboard_as_a_new_capability() {
    let ir_of = |src: &str| {
        let ast = parse(lex(src).expect("lex")).expect("parse");
        check_file(&ast).expect("check");
        lower_file(&ast).expect("lower")
    };
    let without = COPIER
        .replace("    requires Clipboard;\n", "")
        .replace("Button { label: \"Copy\" copy: draft }", "Text { draft }");
    let report = serde_json::to_value(diff_ir(&ir_of(&without), &ir_of(COPIER), "base", "cand"))
        .expect("a report");
    let added = report["capabilities"]["added"].as_array().expect("a list");
    assert!(
        added.iter().any(|c| c["capability"] == "Clipboard"),
        "{report}"
    );
}
