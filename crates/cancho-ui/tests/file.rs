//! `FileField` (`docs/file-upload.md`, #79): the shared corpus `tests/file_corpus.json` replayed in-process (the file
//! `scripts/check-file.js` drives in Chromium), what replay refuses, and every rule that refuses a misuse.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/file_corpus.json");

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-file-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

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

fn replay(src: &str, steps: Vec<Value>) -> (i32, Value) {
    let file = scratch("f.cui", src);
    let trace = scratch(
        "f.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (
        code,
        serde_json::from_str(&stdout).unwrap_or(Value::String(stderr)),
    )
}

fn rules(report: &Value) -> Vec<String> {
    report["failures"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| f["rule"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn replay_agrees_with_every_step_of_the_corpus() {
    let c = corpus();
    let steps: Vec<Value> = c["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({"event": "Picked", "params": [s["value"]], "state": {"last": s["value"], "count": i + 1}})
        })
        .collect();
    assert!(steps.len() >= 16, "the corpus shrank");
    let (code, report) = replay(&source(&c), steps);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn replay_refuses_a_value_the_page_cannot_produce() {
    let c = corpus();
    let cases = c["notFiles"].as_array().expect("a list");
    assert!(cases.len() >= 12);
    for case in cases {
        let (code, report) = replay(
            &source(&c),
            vec![
                json!({"event": "Picked", "params": [case["value"]], "expect": {"outcome": "completed"}}),
            ],
        );
        assert_eq!(code, 4, "{}: {report}", case["why"]);
        assert_eq!(
            rules(&report),
            ["trace-value-not-a-file"],
            "{}",
            case["why"]
        );
    }
}

#[test]
fn a_button_that_can_give_the_event_any_string_lifts_the_check() {
    let src = "component F {\n    state s: String = \"\";\n    on Picked(v) { s = v; }\n    view { Column { FileField { label: \"F\" on: Picked } Button { label: \"Any\" on: Picked(\"zzz\") } } }\n}\n";
    let (code, report) = replay(
        src,
        vec![json!({"event": "Picked", "params": ["zzz"], "state": {"s": "zzz"}})],
    );
    assert_eq!(code, 0, "{report}");
}

#[test]
fn limits_default_to_the_cap_and_accept_to_anything() {
    let src = "component F {\n    state s: String = \"\";\n    on Picked(v) { s = v; }\n    view { FileField { label: \"F\" on: Picked } }\n}\n";
    let big = json!({"name": "a.bin", "size": 1048576, "type": "x/y", "text": "x"}).to_string();
    assert_eq!(
        replay(
            src,
            vec![json!({"event": "Picked", "params": [big], "state": {"s": big}})]
        )
        .0,
        0
    );
    let over = json!({"name": "a.bin", "size": 1048577, "type": "x/y", "text": "x"}).to_string();
    let (code, report) = replay(
        src,
        vec![json!({"event": "Picked", "params": [over], "expect": {"outcome": "completed"}})],
    );
    assert_eq!(code, 4);
    assert_eq!(rules(&report), ["trace-value-not-a-file"]);
}

fn comp(view: &str) -> String {
    format!(
        "component F {{\n    state s: String = \"\";\n    on Picked(v) {{ s = v; }}\n    on Two(a, b) {{ s = a; }}\n    on None {{ s = \"\"; }}\n    view {{ Column {{ {view} }} }}\n}}\n"
    )
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("c.cui", src);
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
    let file = scratch("c.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_file_field_is_accepted_without_a_capability() {
    accepted(&comp("FileField { label: \"Report\" on: Picked }"));
    accepted(&comp(
        "FileField { label: \"Report\" accept: \".txt,text/csv\" max_bytes: 100 on: Picked disabled: true }",
    ));
}

#[test]
fn a_file_field_needs_a_label_and_an_event() {
    for view in [
        "FileField { on: Picked }",
        "FileField { label: \"x\" }",
        "FileField { }",
    ] {
        assert_eq!(codes(&comp(view)), ["file-field-incomplete"], "{view}");
    }
}

#[test]
fn accept_and_max_bytes_are_checked() {
    for accept in [
        "",
        "txt",
        ".txt, .csv",
        "text/",
        "Text/Plain",
        ".t xt",
        "*/*",
        ".txt,",
    ] {
        let view = format!("FileField {{ label: \"x\" accept: \"{accept}\" on: Picked }}");
        assert_eq!(codes(&comp(&view)), ["file-accept-invalid"], "{accept:?}");
    }
    for limit in ["0", "1048577", "\"5\"", "true", "x"] {
        let view = format!("FileField {{ label: \"x\" max_bytes: {limit} on: Picked }}");
        let c = codes(&comp(&view));
        assert!(
            c == ["file-limit-invalid"] || c == ["parse-unexpected-token"],
            "{limit}: {c:?}"
        );
    }
    accepted(&comp("FileField { label: \"x\" max_bytes: 1 on: Picked }"));
    accepted(&comp(
        "FileField { label: \"x\" max_bytes: 1048576 on: Picked }",
    ));
}

#[test]
fn the_file_value_is_at_most_one_parameter() {
    assert_eq!(
        codes(&comp("FileField { label: \"x\" on: Two }")),
        ["event-arity-mismatch"]
    );
    // as for every field, an event without a parameter is allowed (it ignores the value)
    accepted(&comp("FileField { label: \"x\" on: None }"));
}

#[test]
fn bindings_of_other_elements_are_refused() {
    for view in [
        "Button { label: \"x\" accept: \".txt\" on: None }",
        "TextField { label: \"x\" value: s on: Picked max_bytes: 5 }",
        "FileField { label: \"x\" on: Picked value: s }",
        "FileField { label: \"x\" on: Picked error: s }",
        "FileField { label: \"x\" on: Picked placeholder: \"p\" }",
        "FileField { label: \"x\" on: Picked(\"a\") }",
    ] {
        let c = codes(&comp(view));
        assert!(
            c == ["binding-unknown"] || c == ["event-args-unsupported"],
            "{view}: {c:?}"
        );
    }
}

#[test]
fn a_file_field_is_not_drawn_in_a_for() {
    let src = "component F {\n    state s: String = \"\";\n    state rows: List[String] = [\"a\"];\n    on Picked(v) { s = v; }\n    view { For { over: rows as: r FileField { label: \"x\" on: Picked } } }\n}\n";
    assert_eq!(codes(src), ["file-field-in-loop"]);
    // an If inside a For is still in the For; an If outside it is fine
    let ok = "component F {\n    state s: String = \"\";\n    state show: Bool = true;\n    on Picked(v) { s = v; }\n    view { If { when: show FileField { label: \"x\" on: Picked } } }\n}\n";
    accepted(ok);
}

fn build(src: &str) -> String {
    let file = scratch("b.cui", src);
    let out = scratch("b.js", "");
    let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

#[test]
fn the_page_carries_the_file_adapter_only_for_a_program_that_has_a_file_field() {
    let plain = build(&comp("Button { label: \"x\" on: None }"));
    assert!(!plain.contains("__cui_file"), "{plain}");
    let js = build(&comp(
        "FileField { label: \"Report\" accept: \".txt\" max_bytes: 50 on: Picked }",
    ));
    assert!(js.contains("function __cui_file_pick("), "{js}");
    assert!(js.contains(".setAttribute('type', 'file');"), "{js}");
    assert!(js.contains(".setAttribute('accept', \".txt\");"), "{js}");
    assert!(js.contains("__cui_file_pick(root, \""), "{js}");
    assert!(js.contains(", 50, \".txt\", function"), "{js}");
    assert!(
        js.contains("new TextDecoder('utf-8', { fatal: true })"),
        "{js}"
    );
    assert!(!js.contains("readAsText"), "no lenient decoding");
    // no accept, no max_bytes: the cap and no filter
    let js = build(&comp("FileField { label: \"Report\" on: Picked }"));
    assert!(js.contains(", 1048576, null, function"), "{js}");
}

#[test]
fn the_diff_shows_a_file_field_added() {
    let a = scratch("a.cui", &comp("Button { label: \"x\" on: None }"));
    let b = scratch(
        "b.cui",
        &comp("Button { label: \"x\" on: None } FileField { label: \"Report\" on: Picked }"),
    );
    let (code, out, err) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert!(code == 0 || code == 5, "{code} {out}{err}");
    assert!(out.contains("FileField"), "{out}");
}

#[test]
fn the_example_replays() {
    let (code, out, err) = run(&[
        "test",
        "examples/upload/upload.cui",
        "examples/upload/upload.trace.json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
}
