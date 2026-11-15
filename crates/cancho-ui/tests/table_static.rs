//! `Table { sticky: .. max_height: .. }` and `Text { numeric: true }` (`docs/table-static.md`): what the compiler emits and what it refuses. Where the header and the
//! first column sit after a scroll is `scripts/check-table-static.js`, outside the gate because it needs a browser.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const LEDGER: &str = include_str!("../../../examples/table-static/table-static.cui");

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
    let dir = common::scratch_root().join(format!("cancho-ui-tstatic-{}", std::process::id()));
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

fn table(bindings: &str) -> String {
    format!(
        "component A {{\n    view {{ Table {{ label: \"T\" {bindings} Row {{ Text {{ role: columnheader \"h\" }} }} Row {{ Text {{ \"c\" }} }} }} }}\n}}\n"
    )
}

#[test]
fn the_example_builds_with_the_classes_the_height_and_the_numeric_cells() {
    let js = build_js(LEDGER, "ledger.cui");
    for class in ["cui-sticky-head", "cui-sticky-col", "cui-has-height"] {
        assert!(
            js.contains(&format!("classList.add('{class}')")),
            "{class}: {js}"
        );
    }
    assert!(js.contains("max-height: 12rem;"), "{js}");
    assert!(js.matches("\"cui-num\"").count() == 8, "{js}");
    let css = include_str!("../src/pwa.rs");
    for rule in [
        ".cui-sticky-head th",
        ".cui-sticky-col th:first-child",
        "tabular-nums",
    ] {
        assert!(css.contains(rule), "the baseline css has {rule}");
    }
}

#[test]
fn a_table_without_the_bindings_carries_none_of_it() {
    let js = build_js(&table(""), "plain.cui");
    assert!(
        !js.contains("cui-sticky-head") && !js.contains("max-height: ") && !js.contains("cui-num"),
        "{js}"
    );
}

#[test]
fn the_header_only_sticks_with_a_height_and_the_column_alone_needs_none() {
    let head = build_js(&table("sticky: \"header\" max_height: 8"), "head.cui");
    assert!(head.contains("cui-sticky-head") && !head.contains("cui-sticky-col"));
    let col = build_js(&table("sticky: \"column\""), "col.cui");
    assert!(col.contains("cui-sticky-col") && !col.contains("cui-has-height"));
    assert_eq!(
        check_codes(&table("sticky: \"header\""), "needs.cui"),
        ["table-sticky-needs-height"]
    );
    assert_eq!(
        check_codes(&table("sticky: \"both\""), "needs2.cui"),
        ["table-sticky-needs-height"]
    );
}

#[test]
fn bad_values_are_refused_with_their_codes() {
    assert_eq!(
        check_codes(&table("sticky: \"row\""), "s.cui"),
        ["table-sticky-invalid"]
    );
    for h in ["3", "101", "\"8\""] {
        assert_eq!(
            check_codes(&table(&format!("max_height: {h}")), "h.cui"),
            ["table-max-height-invalid"],
            "{h}"
        );
    }
    let bad = "component A {\n    state f: Bool = true;\n    view { Table { label: \"T\" Row { Text { numeric: f \"1\" } } } }\n}\n";
    assert_eq!(check_codes(bad, "n.cui"), ["numeric-invalid"]);
    let out = "component A {\n    view { Text { numeric: true \"1\" } }\n}\n";
    assert_eq!(check_codes(out, "o.cui"), ["numeric-outside-table"]);
}
