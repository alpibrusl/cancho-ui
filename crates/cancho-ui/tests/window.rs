//! A table that draws only the rows in view (`docs/window.md`): what the compiler writes, what it refuses, and that replay fires the event with an index. What a scroll box does with a
//! real scroll is `scripts/check-window.js`, outside the gate because it needs a browser.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const WINDOW: &str = include_str!("../../../examples/window/window.cui");

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
    let dir = common::scratch_root().join(format!("cancho-ui-keys-{}", std::process::id()));
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

/// A component with the events and the state a window names and a table around `bindings`.
fn with(bindings: &str) -> String {
    format!(
        "component A {{\n    state rows: List[String] = [\"a\", \"b\"];\n    state first: Int = 0;\n    state label: String = \"x\";\n    on Scrolled(at) {{ first = at; }}\n    on Plain {{ first = 0; }}\n    view {{ Table {{ label: \"T\" {bindings} Row {{ Text {{ role: columnheader \"h\" }} }} Row {{ Text {{ \"c\" }} }} }} }}\n}}\n"
    )
}

const FULL: &str = "max_height: 12 row_height: 2 total: len(rows) first: first on: Scrolled";

#[test]
fn the_example_is_a_listener_a_spacer_pair_and_row_numbers_and_a_plain_table_has_none() {
    let js = build_js(WINDOW, "window.cui");
    assert_eq!(js.matches("addEventListener('scroll'").count(), 1, "{js}");
    assert!(
        js.contains("aria-rowcount") && js.contains("aria-rowindex"),
        "{js}"
    );
    assert_eq!(js.matches("'aria-hidden', 'true'").count(), 1, "{js}");
    assert!(
        js.contains("data-cui-window") && js.contains("__cui_win"),
        "{js}"
    );
    let plain = build_js(&with("max_height: 12"), "plain.cui");
    assert!(
        !plain.contains("'scroll'")
            && !plain.contains("aria-rowcount")
            && !plain.contains("__cui_win"),
        "{plain}"
    );
}

#[test]
fn replay_fires_the_event_with_the_index() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (code, out, err) = run(&[
        "test",
        root.join("examples/window/window.cui")
            .to_str()
            .expect("path"),
        root.join("examples/window/window.trace.json")
            .to_str()
            .expect("path"),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("PASS  3 steps"), "{out}");
}

#[test]
fn a_window_is_the_five_bindings_together() {
    build_js(&with(FULL), "full.cui");
    for missing in [
        "max_height: 12",
        "row_height: 2",
        "total: len(rows)",
        "first: first",
        "on: Scrolled",
    ] {
        let b = FULL.replace(missing, "");
        assert_eq!(
            check_codes(&with(&b), "m.cui"),
            ["window-incomplete"],
            "{missing}"
        );
    }
}

#[test]
fn each_other_refusal_has_its_code() {
    for h in ["0", "21", "-1", "\"2\"", "first"] {
        let b = FULL.replace("row_height: 2", &format!("row_height: {h}"));
        assert_eq!(
            check_codes(&with(&b), "r.cui"),
            ["window-row-height-invalid"],
            "{h}"
        );
    }
    for h in ["1", "20"] {
        build_js(
            &with(&FULL.replace("row_height: 2", &format!("row_height: {h}"))),
            "rh.cui",
        );
    }
    for bad in ["label", "rows", "len(rows) > 1"] {
        let b = FULL.replace("total: len(rows)", &format!("total: {bad}"));
        assert_eq!(
            check_codes(&with(&b), "t.cui"),
            ["window-count-not-number"],
            "total {bad}"
        );
        let b = FULL.replace("first: first", &format!("first: {bad}"));
        assert_eq!(
            check_codes(&with(&b), "f.cui"),
            ["window-count-not-number"],
            "first {bad}"
        );
    }
    // the event takes the index and nothing else
    assert_eq!(
        check_codes(&with(&FULL.replace("on: Scrolled", "on: Plain")), "e.cui"),
        ["event-arity-mismatch"]
    );
    assert_eq!(
        check_codes(&with(&FULL.replace("on: Scrolled", "on: Nope")), "u.cui"),
        ["event-unknown"]
    );
    // a row height means nothing on anything but a table
    let button = "component A {\n    on Plain {}\n    view { Button { label: \"B\" row_height: 2 on: Plain } }\n}\n";
    assert_eq!(check_codes(button, "b.cui"), ["binding-unknown"]);
    // the numbers may be arithmetic of state
    build_js(
        &with(&FULL.replace("total: len(rows)", "total: len(rows) * 2 + first")),
        "arith.cui",
    );
}
