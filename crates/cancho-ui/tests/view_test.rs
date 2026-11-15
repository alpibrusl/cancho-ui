//! The outline of the view (`docs/view-test.md`): what the page shows for a state, written by hand here so that the evaluator is not its own oracle, the `view` and `view_has` of a trace, and what is
//! refused. That the outline is what the browser draws is `scripts/check-view.js`, outside the gate because it needs a browser.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const COUNTER: &str = include_str!("../../../examples/counter/counter.cui");
const SORT: &str = include_str!("../../../examples/table-sort/table-sort.cui");
const WINDOW: &str = include_str!("../../../examples/window/window.cui");
const KEYS: &str = include_str!("../../../examples/keys/keys.cui");

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

fn outline(src: &str, name: &str, state: &str) -> Vec<String> {
    let file = scratch(name, src);
    let (code, stdout, stderr) = run(&["inspect", "view", p(&file), "--state", state]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    serde_json::from_str::<Vec<String>>(&stdout).expect("a list of lines")
}

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Run a trace over a source: the exit code and the whole output.
fn test_trace(src: &str, trace: &str, name: &str) -> (i32, String) {
    let file = scratch(&format!("{name}.cui"), src);
    let tr = scratch(&format!("{name}.json"), trace);
    let (code, stdout, stderr) = run(&["test", p(&file), p(&tr), "--output", "json"]);
    (code, format!("{stdout}{stderr}"))
}

#[test]
fn the_outline_of_each_example_is_what_a_person_would_write() {
    assert_eq!(
        outline(COUNTER, "c.cui", "{}"),
        lines(&[
            "status \"Count: 0\"",
            "button \"Increment\"",
            "button \"Reset\""
        ])
    );
    assert_eq!(
        outline(COUNTER, "c2.cui", "{\"count\": 7}"),
        lines(&[
            "status \"Count: 7\"",
            "button \"Increment\"",
            "button \"Reset\""
        ])
    );
    // keys: a Key draws nothing, a field has its value, the empty log leaves one space between the words
    assert_eq!(
        outline(
            KEYS,
            "k.cui",
            "{\"query\": \"ab\", \"cursor\": 3, \"log\": \"move\"}"
        ),
        lines(&[
            "textbox \"Filter\" value=\"ab\"",
            "button \"Disable x\"",
            "text \"cursor 3 anchor 0 log move other 0\"",
            "text \"second container\"",
        ])
    );
    // the sortable header: aria-sort on the header of the sorted column, the rows in the order the program sorts them
    assert_eq!(
        outline(SORT, "s.cui", "{\"order\": \"-length\"}"),
        lines(&[
            "table \"Machines\"",
            "  row",
            "    columnheader \"name\"",
            "      button \"name\"",
            "    columnheader \"length\" sort=descending numeric",
            "      button \"length\"",
            "  row",
            "    cell \"compressor\"",
            "    cell \"10\" numeric",
            "  row",
            "    cell \"valve\"",
            "    cell \"5\" numeric",
            "  row",
            "    cell \"pipe\"",
            "    cell \"4\" numeric",
            "  row",
            "    cell \"pump\"",
            "    cell \"4\" numeric",
            "  row",
            "    cell \"fan\"",
            "    cell \"3\" numeric",
        ])
    );
    let name_asc = outline(SORT, "s2.cui", "{\"order\": \"name\"}");
    assert_eq!(name_asc[2], "    columnheader \"name\" sort=ascending");
    assert_eq!(name_asc[4], "    columnheader \"length\" numeric");
    assert_eq!(name_asc[7], "    cell \"compressor\"");
}

#[test]
fn a_window_says_how_many_rows_there_are_and_draws_the_ones_the_program_drew() {
    let w = outline(WINDOW, "w.cui", "{\"first\": 10}");
    assert_eq!(w[0], "table \"Machines\" rows=61");
    assert_eq!(w.len(), 1 + 3 + 16 * 3, "{w:?}");
    assert_eq!(w[4], "  row");
    assert_eq!(w[5], "    cell \"row-010\"");
    assert_eq!(w[6], "    cell \"7\" numeric");
    assert_eq!(w[w.len() - 2], "    cell \"row-025\"");
    // near the end the program draws what is left
    let end = outline(WINDOW, "w2.cui", "{\"first\": 55}");
    assert_eq!(end.len(), 1 + 3 + 5 * 3);
}

#[test]
fn a_step_that_asserts_the_view_passes_and_one_that_is_wrong_fails_with_both_outlines() {
    let ok = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "component": "Machines", "steps": [
        { "event": "__sort_order", "params": ["name"], "state": { "order": "name" },
          "view_has": ["columnheader \"name\" sort=ascending", "cell \"compressor\"", "cell \"valve\""] },
        { "event": "__sort_order", "params": ["name"], "state": { "order": "-name" },
          "view_has": ["columnheader \"name\" sort=descending", "cell \"valve\"", "cell \"compressor\""] } ] }"#;
    let (code, out) = test_trace(SORT, ok, "ok");
    assert_eq!(code, 0, "{out}");
    // exact: the whole page after a tap on the counter
    let exact = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        { "event": "Increment", "state": { "count": 1 },
          "view": ["status \"Count: 1\"", "button \"Increment\"", "button \"Reset\""] } ] }"#;
    let (code, out) = test_trace(COUNTER, exact, "exact");
    assert_eq!(code, 0, "{out}");
    // wrong: the order of the lines, a missing line, a wrong count
    for (bad, what) in [
        (
            r#""view": ["status \"Count: 2\"", "button \"Increment\"", "button \"Reset\""]"#,
            "a wrong count",
        ),
        (
            r#""view": ["button \"Increment\"", "status \"Count: 1\"", "button \"Reset\""]"#,
            "the order",
        ),
        (
            r#""view": ["status \"Count: 1\"", "button \"Increment\""]"#,
            "a missing line",
        ),
        (
            r#""view_has": ["button \"Reset\"", "button \"Increment\""]"#,
            "view_has in the wrong order",
        ),
        (
            r#""view": ["status \"Count: 1\"", "button \"Increment\"", "button \"Nope\""]"#,
            "only the last line wrong",
        ),
        (
            r#""view": ["status \"Count: 1\"", "button \"Increment\"", "button \"Reset\"", "button \"Extra\""]"#,
            "a line too many",
        ),
        (
            r#""view_has": ["button \"Nope\""]"#,
            "view_has with a line the page lacks",
        ),
    ] {
        let t = format!(
            r#"{{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [ {{ "event": "Increment", "state": {{ "count": 1 }}, {bad} }} ] }}"#
        );
        let (code, out) = test_trace(COUNTER, &t, "bad");
        assert_eq!(code, 4, "{what}: {out}");
        assert!(out.contains("view-mismatch"), "{what}: {out}");
        assert!(
            out.contains("Count: 1"),
            "{what}: the report carries the page: {out}"
        );
    }
}

#[test]
fn a_node_the_outline_cannot_say_is_refused_and_so_is_a_view_the_replay_cannot_read() {
    let rich = "component A {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Column { Rich { \"x\" } Button { label: \"Go\" on: Go } } }\n}\n";
    let t = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        { "event": "Go", "state": { "n": 1 }, "view_has": ["button \"Go\""] } ] }"#;
    let (code, out) = test_trace(rich, t, "rich");
    assert_eq!(code, 4, "{out}");
    assert!(
        out.contains("view-unsupported") && out.contains("? Rich"),
        "{out}"
    );
    // without a view, the same trace passes: the outline is asked for only when a step asserts it
    let plain = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [ { "event": "Go", "state": { "n": 1 } } ] }"#;
    assert_eq!(test_trace(rich, plain, "rich2").0, 0);
    // the page shows it, `inspect view` says so
    let lines = outline(rich, "rich3.cui", "{}");
    assert!(lines.contains(&"? Rich".to_string()), "{lines:?}");
    // a number the page writes differently from Rust is not guessed
    let tiny = "component A {\n    on Go {}\n    view { Text { \"{0.0000001}\" } }\n}\n";
    let t = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        { "event": "Go", "view": ["text \"0\""] } ] }"#;
    let (code, out) = test_trace(tiny, t, "tiny");
    assert_eq!(code, 4, "{out}");
    assert!(out.contains("view-expression-unsupported"), "{out}");
}

#[test]
fn a_line_that_is_not_an_outline_line_is_refused_before_the_trace_runs() {
    for bad in [
        "",
        "  ",
        "Button",
        "button name",
        "button \"open",
        "button \"a\" =x",
        "   button \"a\"",
        "BUTTON \"a\"",
        "button \"a\" sort=",
    ] {
        let t = format!(
            r#"{{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [ {{ "event": "Increment", "state": {{ "count": 1 }}, "view": [{}] }} ] }}"#,
            serde_json::to_string(bad).expect("json")
        );
        let (code, out) = test_trace(COUNTER, &t, "line");
        assert_ne!(code, 0, "{bad:?}: {out}");
        assert!(out.contains("view-bad-line"), "{bad:?}: {out}");
    }
    // a good line is not refused
    for good in [
        "button \"a\"",
        "  row",
        "    cell \"x\" numeric",
        "table \"T\" rows=61",
        "textbox \"F\" value=\"a b\" disabled",
        "? Chart",
    ] {
        let t = format!(
            r#"{{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [ {{ "event": "Increment", "state": {{ "count": 1 }}, "view_has": [{}] }} ] }}"#,
            serde_json::to_string(good).expect("json")
        );
        let (_, out) = test_trace(COUNTER, &t, "line2");
        assert!(!out.contains("view-bad-line"), "{good:?}: {out}");
    }
}

#[test]
fn an_if_that_is_false_is_not_in_the_page_and_a_for_is_drawn_for_each_element() {
    let src = "component A {\n    state show: Bool = false;\n    state names: List[String] = [\"a\", \"b\", \"c\"];\n    on Go { show = true; }\n    view { Column { Text { \"top\" } If { when: show Text { \"shown\" } } For { over: names as: n Text { \"{n}\" } } } }\n}\n";
    assert_eq!(
        outline(src, "if.cui", "{}"),
        lines(&["text \"top\"", "text \"a\"", "text \"b\"", "text \"c\""])
    );
    assert_eq!(
        outline(src, "if2.cui", "{\"show\": true, \"names\": [\"x\"]}"),
        lines(&["text \"top\"", "text \"shown\"", "text \"x\""])
    );
    // the same through a trace: the event shows the part
    let t = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        { "event": "Go", "state": { "show": true }, "view_has": ["text \"top\"", "text \"shown\"", "text \"c\""] } ] }"#;
    let (code, out) = test_trace(src, t, "if3");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_button_says_whether_it_is_pressed_or_disabled_and_a_box_whether_it_is_checked() {
    let src = "component A {\n    state bold: Bool = false;\n    state locked: Bool = false;\n    state agree: Bool = false;\n    on Tog { bold = !bold; }\n    on Agree(v) { agree = v; }\n    view { Column { Button { label: \"Bold\" pressed: bold on: Tog } Button { label: \"Lock\" disabled: locked on: Tog } Checkbox { label: \"I agree\" checked: agree on: Agree } } }\n}\n";
    assert_eq!(
        outline(src, "p.cui", "{}"),
        lines(&["button \"Bold\"", "button \"Lock\"", "checkbox \"I agree\""])
    );
    assert_eq!(
        outline(src, "p2.cui", "{\"bold\": true}"),
        lines(&[
            "button \"Bold\" pressed",
            "button \"Lock\"",
            "checkbox \"I agree\""
        ])
    );
    assert_eq!(
        outline(src, "p3.cui", "{\"locked\": true, \"agree\": true}"),
        lines(&[
            "button \"Bold\"",
            "button \"Lock\" disabled",
            "checkbox \"I agree\" checked"
        ])
    );
}

#[test]
fn a_component_whose_view_may_read_the_browser_is_refused_whole() {
    let src = "component A {\n    requires Storage;\n    state n: Int = 0;\n    on Go { n = n + 1; storage.set(\"k\", \"v\"); }\n    view { Text { \"{n}\" } }\n}\n";
    let t = r#"{ "schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        { "event": "Go", "state": { "n": 1 }, "view": ["text \"1\""] } ] }"#;
    let (code, out) = test_trace(src, t, "storage");
    assert_eq!(code, 4, "{out}");
    assert!(
        out.contains("view-expression-unsupported") && out.contains("requires Storage"),
        "{out}"
    );
    // `inspect view` says it too
    let file = scratch("storage2.cui", src);
    let (code, _, stderr) = run(&["inspect", "view", p(&file)]);
    assert_ne!(code, 0);
    assert!(stderr.contains("requires Storage"), "{stderr}");
}
