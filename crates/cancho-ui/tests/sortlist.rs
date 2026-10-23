//! Reordering (`docs/drag-and-drop.md`, issue #80): `SortList`, the `move` builtin, what is refused with which tag, and how replay reads a move.
//! The page is checked in Chromium by `scripts/check-sortlist.js`; `tests/move_corpus.json` is run here and there.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

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
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cancho-ui-sortlist-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("f.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "expected a refusal for:\n{src}\n{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

fn accepted(src: &str) {
    let file = scratch("f.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{src}\n{stdout}{stderr}");
}

fn replay(src: &str, steps: Value) -> (i32, Value) {
    let file = scratch("f.cui", src);
    let trace = scratch(
        "f.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    let report: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("a JSON report ({e}): {stdout}{stderr}"));
    (code, report)
}

fn rules(report: &Value) -> Vec<String> {
    report["failures"]
        .as_array()
        .expect("failures")
        .iter()
        .map(|f| f["rule"].as_str().expect("a rule").to_string())
        .collect()
}

/// A component with the given view body.
fn comp(view: &str) -> String {
    format!(
        "component T {{\n    state tasks: List[String] = [\"write\", \"test\", \"ship\"];\n    state n: Int = 0;\n    state shown: Bool = true;\n    on Moved(from, to) {{ tasks = move(tasks, from, to); }}\n    on Plain {{ n = n + 1; }}\n    on One(x) {{ n = n + 1; }}\n    view {{ Column {{ {view} }} }}\n}}\n"
    )
}

const OK: &str = "SortList { label: \"Tasks\" items: tasks on: Moved }";

// ---- move ------------------------------------------------------------------------------------------------------------

#[test]
fn move_agrees_with_the_corpus() {
    let c: Value =
        serde_json::from_str(include_str!("../../../tests/move_corpus.json")).expect("JSON");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 20);
    for case in cases {
        let list: Vec<Value> = case["list"].as_array().expect("list").clone();
        let got = cancho_ui_replay::move_item(
            list,
            case["from"].as_f64().expect("from"),
            case["to"].as_f64().expect("to"),
        );
        assert_eq!(Value::Array(got), case["want"], "{}", case["why"]);
    }
}

#[test]
fn move_is_a_builtin_with_three_arguments() {
    accepted(&comp(OK));
    for call in ["move(tasks, 0)", "move(tasks)", "move(tasks, 0, 1, 2)"] {
        let src = comp(OK).replace("move(tasks, from, to)", call);
        assert_eq!(codes(&src), ["builtin-shape-mismatch"], "{call}");
    }
    // a list is what it moves
    let (code, report) = replay(
        &comp(OK)
            .replace("move(tasks, from, to)", "move(n, from, to)")
            .replace("tasks = move", "n = move"),
        json!([{"event": "Moved", "params": [0, 1], "state": {"n": 0}}]),
    );
    assert_eq!(code, 4, "{report:#}");
}

// ---- what is refused ---------------------------------------------------------------------------------------------------

#[test]
fn each_sortlist_tag_is_reached() {
    // incomplete: no name, no list, no event
    for view in [
        "SortList { items: tasks on: Moved }",
        "SortList { label: \"T\" on: Moved }",
        "SortList { label: \"T\" items: tasks }",
    ] {
        assert_eq!(codes(&comp(view)), ["sortlist-incomplete"], "{view}");
    }
    // items: that is not a List field of the component
    assert_eq!(
        codes(&comp("SortList { label: \"T\" items: n on: Moved }")),
        ["sortlist-binding-invalid"]
    );
    assert_eq!(
        codes(&comp("SortList { label: \"T\" items: nothing on: Moved }")),
        ["sortlist-binding-invalid"]
    );
    // one list per place
    assert_eq!(
        codes(&comp(&format!("For {{ over: tasks as: t {OK} }}"))),
        ["sortlist-in-loop"]
    );
    // the keys it takes, and the key that is its alone
    assert_eq!(
        codes(&comp(
            "SortList { label: \"T\" items: tasks on: Moved disabled: shown }"
        )),
        ["binding-unknown"]
    );
    assert_eq!(
        codes(&comp("Button { label: \"a\" items: \"x\" on: Plain }")),
        ["binding-unknown"]
    );
    // the event: unknown, or not two parameters
    assert_eq!(
        codes(&comp("SortList { label: \"T\" items: tasks on: Nowhere }")),
        ["event-unknown"]
    );
    assert_eq!(
        codes(&comp("SortList { label: \"T\" items: tasks on: Plain }")),
        ["event-arity-mismatch"]
    );
    assert_eq!(
        codes(&comp("SortList { label: \"T\" items: tasks on: One }")),
        ["event-arity-mismatch"]
    );
    // a click gives arguments, a list does not
    assert_eq!(
        codes(&comp(
            "SortList { label: \"T\" items: tasks on: Moved(1, 2) }"
        )),
        ["event-args-unsupported"]
    );
}

// ---- replay ------------------------------------------------------------------------------------------------------------

#[test]
fn replay_reports_the_list_and_follows_a_move() {
    let (code, report) = replay(
        &comp(OK),
        json!([
            {"advance": 0, "expect": {"sortlists": [{"label": "Tasks", "count": 3, "items": ["write", "test", "ship"]}]}},
            {"event": "Moved", "params": [2, 0], "state": {"tasks": ["ship", "write", "test"]}, "expect": {"sortlists": [{"items": ["ship", "write", "test"]}]}},
            {"event": "Moved", "params": [0, 1], "state": {"tasks": ["write", "ship", "test"]}}
        ]),
    );
    assert_eq!(code, 0, "{report:#}");
    assert_eq!(
        report["steps"][2]["sortlists"][0]["items"],
        json!(["write", "ship", "test"])
    );
}

#[test]
fn replay_refuses_a_move_the_page_could_not_make() {
    let bad = [
        json!([1, 1]),
        json!([3, 0]),
        json!([0, 3]),
        json!([-1, 0]),
        json!([0, -1]),
        json!([0.5, 1]),
        json!(["0", 1]),
        json!([true, 1]),
    ];
    for params in bad {
        let (code, report) = replay(
            &comp(OK),
            json!([{"event": "Moved", "params": params, "state": {"n": 0}}]),
        );
        assert_eq!(code, 4, "{params}\n{report:#}");
        assert_eq!(rules(&report), ["trace-move-invalid"], "{params}");
    }
    // the list as it is now: after a pop the old last index is gone
    let src = comp(OK).replace(
        "on Plain { n = n + 1; }",
        "on Plain { tasks = take(tasks, 1); }",
    );
    let (code, report) = replay(
        &src,
        json!([{"event": "Plain"}, {"event": "Moved", "params": [0, 1], "state": {"n": 0}}]),
    );
    assert_eq!(code, 4, "{report:#}");
    assert_eq!(rules(&report), ["trace-move-invalid"]);
}

#[test]
fn a_move_is_not_checked_when_another_control_fires_the_event_too() {
    let view = format!("{OK} Button {{ label: \"Go\" on: One(1) }}");
    let src = comp(&view)
        .replace(
            "on One(x) { n = n + 1; }",
            "on One(x) { n = n + 1; }\n    on Both(a, b) { n = a; }",
        )
        .replace("on: One(1)", "on: Both(1, 2)");
    // `Both` is a different event: unrelated to the list
    let (code, _) = replay(
        &src,
        json!([{"event": "Both", "params": [5, 5], "state": {"n": 5}}]),
    );
    assert_eq!(code, 0);
    // the same event on a list and on a button: a button could give it anything
    let shared = comp(
        "SortList { label: \"T\" items: tasks on: Moved } Button { label: \"Go\" on: Moved(5, 5) }",
    );
    let (code, report) = replay(
        &shared,
        json!([{"event": "Moved", "params": [9, 9], "state": {"n": 0}}]),
    );
    assert_eq!(code, 0, "{report:#}");
}

#[test]
fn a_list_under_a_false_condition_is_not_in_the_page() {
    let src = comp("If { when: n == 1 SortList { label: \"T\" items: tasks on: Moved } }");
    let (code, report) = replay(
        &src,
        json!([
            {"advance": 0, "expect": {"sortlists": []}},
            {"event": "Plain", "expect": {"sortlists": [{"label": "T", "count": 3}]}}
        ]),
    );
    assert_eq!(code, 0, "{report:#}");
}

// ---- the page --------------------------------------------------------------------------------------------------------

fn built(src: &str) -> String {
    let file = scratch("f.cui", src);
    let out = scratch("out.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out), "--no-auto-mount"]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

#[test]
fn the_page_carries_the_list_and_does_not_rebuild_under_a_drag() {
    let js = built(&comp(OK));
    assert!(js.contains("function __cui_sortlist("), "the adapter");
    assert!(js.contains("function __cui_move("), "the move helper");
    assert!(
        js.contains("__cui_sortlist(root, \"__n"),
        "the call, with the component's root"
    );
    assert!(
        js.contains("if (__cui_drag.on) {"),
        "a render waits for a drag to end"
    );
    assert!(js.is_ascii(), "the module is ASCII");
    assert!(
        js.contains("touch-action: none"),
        "the handle gives the page's scroll back to the rest of the row"
    );
    let plain = built("component A {\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n");
    assert!(
        !plain.contains("__cui_sortlist"),
        "a program with no SortList carries none of it"
    );
    assert!(!plain.contains("__cui_drag"));
}
