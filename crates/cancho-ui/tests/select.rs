//! `Select` (docs/controls.md section 10): the corpus `tests/select_corpus.json` replayed in-process (the file
//! `scripts/check-select.js` drives in Chromium), what replay refuses, and the rules that refuse a misuse.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/select_corpus.json");

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn source(c: &Value) -> String {
    c["program"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn report(src: &str, steps: Vec<Value>) -> cancho_ui_replay::TestReport {
    let ast = parse(lex(src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    let trace =
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string();
    replay(&ir, &trace, "prog.cui", "t.json")
}

#[test]
fn replay_agrees_with_every_step_of_the_corpus() {
    let c = corpus();
    let steps: Vec<Value> = c["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| json!({"event": s["event"], "params": s["params"], "state": s["state"]}))
        .collect();
    assert!(steps.len() >= 6, "the corpus shrank");
    let r = report(&source(&c), steps);
    assert!(
        r.failures.is_empty() && r.result == "pass",
        "{:?}",
        r.failures
    );
}

#[test]
fn replay_refuses_a_value_the_select_cannot_report() {
    let c = corpus();
    let cases = c["notOptions"].as_array().expect("a list");
    assert!(cases.len() >= 4);
    for case in cases {
        let r = report(
            &source(&c),
            vec![
                json!({"event": "Pick", "params": [case["value"]], "expect": {"outcome": "completed"}}),
            ],
        );
        let rules: Vec<&str> = r.failures.iter().map(|f| f.rule).collect();
        assert_eq!(rules, ["trace-value-not-an-option"], "{}", case["why"]);
    }
    // After `More` the list has `blocked`.
    let r = report(
        &source(&c),
        vec![
            json!({"event": "More"}),
            json!({"event": "Pick", "params": ["blocked"], "state": {"status": "blocked"}}),
        ],
    );
    assert!(r.failures.is_empty(), "{:?}", r.failures);
}

#[test]
fn a_button_that_can_give_the_event_any_string_lifts_the_limit() {
    let src = "component F {\n    state s: String = \"\";\n    on Pick(v) { s = v; }\n    view { Column { Select { label: \"S\" options: [\"a\", \"b\"] value: s on: Pick } Button { label: \"Any\" on: Pick(\"zzz\") } } }\n}\n";
    let r = report(
        src,
        vec![json!({"event": "Pick", "params": ["zzz"], "state": {"s": "zzz"}})],
    );
    assert!(r.failures.is_empty(), "{:?}", r.failures);
}

#[test]
fn two_selects_for_one_event_offer_the_union_of_their_options() {
    let src = "component F {\n    state s: String = \"\";\n    on Pick(v) { s = v; }\n    view { Column { Select { label: \"A\" options: [\"a\", \"b\"] value: s on: Pick } Select { label: \"B\" options: [\"c\"] value: s on: Pick } } }\n}\n";
    for ok in ["a", "b", "c"] {
        let r = report(
            src,
            vec![json!({"event": "Pick", "params": [ok], "state": {"s": ok}})],
        );
        assert!(r.failures.is_empty(), "{ok}: {:?}", r.failures);
    }
    let r = report(
        src,
        vec![json!({"event": "Pick", "params": ["d"], "expect": {"outcome": "completed"}})],
    );
    let rules: Vec<&str> = r.failures.iter().map(|f| f.rule).collect();
    assert_eq!(rules, ["trace-value-not-an-option"]);
}

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

/// A file of its own per call: the tests run in parallel, and a shared name would be a race.
fn scratch(name: &str, text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cancho-ui-sel-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(element: &str) -> String {
    format!(
        "component F {{\n    state s: String = \"\";\n    state n: Int = 0;\n    state l: List[String] = [\"a\"];\n    state msg: String = \"\";\n    state busy: Bool = false;\n    on Pick(v) {{ s = v; }}\n    on Go {{ n = n + 1; }}\n    view {{ Column {{ {element} }} }}\n}}\n"
    )
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("p.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{src}: {stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

#[test]
fn a_well_formed_select_is_accepted() {
    for element in [
        "Select { label: \"S\" options: [\"a\", \"b\"] value: s on: Pick }",
        "Select { label: \"S\" options: l value: s on: Pick }",
        "Select { label: \"S\" options: [] value: s on: Pick }",
        "Select { label: \"S\" options: l value: s on: Pick disabled: busy error: msg name: \"s\" }",
    ] {
        let file = scratch("ok.cui", &program(element));
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 0, "{element}: {stdout}");
    }
}

#[test]
fn a_select_needs_its_list_its_field_and_its_event() {
    for element in [
        "Select { label: \"S\" value: s on: Pick }",
        "Select { label: \"S\" options: l on: Pick }",
        "Select { label: \"S\" options: l value: s }",
    ] {
        assert_eq!(codes(&program(element)), ["select-incomplete"], "{element}");
    }
}

#[test]
fn options_and_value_are_checked() {
    for element in [
        "Select { label: \"S\" options: [\"a\", 1] value: s on: Pick }",
        "Select { label: \"S\" options: [s] value: s on: Pick }",
        "Select { label: \"S\" options: s value: s on: Pick }",
        "Select { label: \"S\" options: n value: s on: Pick }",
        "Select { label: \"S\" options: nope value: s on: Pick }",
        "Select { label: \"S\" options: l value: n on: Pick }",
        "Select { label: \"S\" options: l value: l on: Pick }",
        "Select { label: \"S\" options: l value: nope on: Pick }",
    ] {
        assert_eq!(
            codes(&program(element)),
            ["select-binding-invalid"],
            "{element}"
        );
    }
}

#[test]
fn options_belongs_to_a_select_and_a_select_takes_only_its_own_keys() {
    for element in [
        "TextField { label: \"F\" value: s on: Pick options: l }",
        "Button { label: \"B\" on: Go options: l }",
    ] {
        assert_eq!(codes(&program(element)), ["binding-unknown"], "{element}");
    }
    for binding in [
        "placeholder: \"x\"",
        "type: \"text\"",
        "submit: Go",
        "maxlength: 3",
        "checked: busy",
        "rows: 2",
        "tone: \"ok\"",
    ] {
        let found = codes(&program(&format!(
            "Select {{ label: \"S\" options: l value: s on: Pick {binding} }}"
        )));
        assert!(
            found
                .iter()
                .all(|c| c == "binding-unknown" || c == "submit-event-invalid"),
            "{binding}: {found:?}"
        );
    }
}

#[test]
fn the_page_draws_a_select_from_its_list_and_reports_the_choice() {
    let file = scratch(
        "emit.cui",
        &program("Select { label: \"S\" options: [\"a\", \"b\"] value: s on: Pick }"),
    );
    let out = scratch("emit.html", "");
    let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    for needle in [
        "document.createElement('select')",
        "document.createElement('option')",
        "for (const __o of [\"a\", \"b\"])",
        ".value = __state_s;",
        "addEventListener('change', function (e) { __on_Pick(e.target.value); })",
    ] {
        assert!(html.contains(needle), "{needle}");
    }
}

#[test]
fn the_semantic_diff_names_the_element() {
    let a = scratch(
        "a.cui",
        &program("Select { label: \"S\" options: [\"a\"] value: s on: Pick }"),
    );
    let b = scratch(
        "b.cui",
        &program("Select { label: \"S\" options: [\"a\", \"b\"] value: s on: Pick }"),
    );
    let (code, stdout, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("Select"), "{stdout}");
}
