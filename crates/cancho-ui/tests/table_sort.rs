//! A sortable table header (`docs/table-sort.md`): what the compiler writes out, what it refuses, and that the replay of the taps gives the three states. What a screen
//! reader and a keyboard see is `scripts/check-table-sort.js`, outside the gate because it needs a browser.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const SORTER: &str = include_str!("../../../examples/table-sort/table-sort.cui");

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
    let dir = common::scratch_root().join(format!("cancho-ui-tsort-{}", std::process::id()));
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

fn page(header: &str, extra_state: &str) -> String {
    format!(
        "component A {{\n    state order: String = \"\";\n    state n: Int = 0;\n    {extra_state}\n    view {{ Table {{ label: \"T\" Row {{ {header} }} Row {{ Text {{ \"c\" }} }} }} }}\n}}\n"
    )
}

#[test]
fn the_example_is_an_event_a_button_and_an_aria_sort_for_each_header() {
    let js = build_js(SORTER, "sorter.cui");
    assert!(js.contains("async function __on___sort_order(key)"), "{js}");
    assert_eq!(js.matches("setAttribute('aria-sort'").count(), 2, "{js}");
    assert_eq!(js.matches("__on___sort_order(").count(), 2 + 1, "{js}");
    assert_eq!(js.matches("createElement('button')").count(), 2, "{js}");
    // the sort is the program's: the compiler wrote no comparator for the header
    assert_eq!(js.matches("async function __on_").count(), 1, "{js}");
}

#[test]
fn a_program_without_a_sortable_header_carries_none_of_it() {
    let js = build_js(&page("Text { role: columnheader \"h\" }", ""), "plain.cui");
    assert!(!js.contains("__sort_") && !js.contains("aria-sort"), "{js}");
}

#[test]
fn the_taps_move_the_field_through_three_states_and_a_new_column_starts_at_ascending() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (code, out, err) = run(&[
        "test",
        root.join("examples/table-sort/table-sort.cui")
            .to_str()
            .expect("path"),
        root.join("examples/table-sort/table-sort.trace.json")
            .to_str()
            .expect("path"),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("PASS  7 steps"), "{out}");
}

#[test]
fn each_refusal_has_its_code() {
    let h = |s: &str| page(s, "");
    assert_eq!(
        check_codes(&h("Text { role: columnheader sort: order \"h\" }"), "a.cui"),
        ["sort-key-missing"]
    );
    assert_eq!(
        check_codes(&h("Text { role: columnheader key: \"h\" \"h\" }"), "b.cui"),
        ["sort-key-missing"]
    );
    assert_eq!(
        check_codes(&h("Text { sort: order key: \"h\" \"h\" }"), "c.cui"),
        ["sort-outside-header"]
    );
    assert_eq!(
        check_codes(
            &h("Text { role: columnheader sort: n key: \"h\" \"h\" }"),
            "d.cui"
        ),
        ["sort-not-string"]
    );
    assert_eq!(
        check_codes(
            &h("Text { role: columnheader sort: nope key: \"h\" \"h\" }"),
            "e.cui"
        ),
        ["sort-not-string"]
    );
    for key in ["", "-h", "a b", "h/x"] {
        assert_eq!(
            check_codes(
                &h(&format!(
                    "Text {{ role: columnheader sort: order key: \"{key}\" \"h\" }}"
                )),
                "f.cui"
            ),
            ["sort-key-invalid"],
            "{key:?}"
        );
    }
    assert_eq!(
        check_codes(
            &h(
                "Text { role: columnheader sort: order key: \"h\" \"a\" } Text { role: columnheader sort: order key: \"h\" \"b\" }"
            ),
            "g.cui"
        ),
        ["sort-key-duplicate"]
    );
    assert_eq!(
        check_codes(
            &h("Text { role: columnheader sort: order key: \"h\" \"a {n}\" }"),
            "h.cui"
        ),
        ["sort-label-invalid"]
    );
    // outside a table
    let loose = "component A {\n    state order: String = \"\";\n    view { Row { Text { role: columnheader sort: order key: \"h\" \"h\" } } }\n}\n";
    assert_eq!(check_codes(loose, "i.cui"), ["sort-outside-header"]);
    // the same key in two tables is two tables
    let two = "component A {\n    state order: String = \"\";\n    view { Column { Table { label: \"T\" Row { Text { role: columnheader sort: order key: \"h\" \"h\" } } } Table { label: \"U\" Row { Text { role: columnheader sort: order key: \"h\" \"h\" } } } } }\n}\n";
    build_js(two, "two.cui");
}
