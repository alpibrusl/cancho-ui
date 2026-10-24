//! A number field (`TextField { type: "number" }`, docs/number-input.md, issue #111): what it gives its event (the corpus, checked here and in
//! Chromium by `scripts/check-number.js`), the tags it needs, and how replay reads a trace of one.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-number-{}-{n}", std::process::id()));
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

/// A component with one number field and the given extra bindings.
fn field(extra: &str) -> String {
    format!(
        "component F {{\n    state v: String = \"\";\n    on V(x) {{ v = x; }}\n    view {{ Column {{ TextField {{ label: \"Amount\" value: v on: V type: \"number\" {extra} }} Text {{ \"got=[{{v}}]\" }} }} }}\n}}\n"
    )
}

/// The same with a Spanish and an English catalog and the `Locale` capability.
fn localized() -> String {
    "component F {\n    requires Locale;\n    state v: String = \"\";\n    messages {\n        es { title: \"Cantidad\"; }\n        en { title: \"Amount\"; }\n    }\n    on V(x) { v = x; }\n    on English { locale.set(\"en\"); }\n    view { Column { TextField { label: t(\"title\") value: v on: V type: \"number\" } Button { label: \"English\" on: English } } }\n}\n".to_string()
}

#[test]
fn a_number_field_is_accepted() {
    accepted(&field(""));
    accepted(&field("inputmode: \"numeric\""));
    accepted(&field("maxlength: 12"));
    accepted(&field("autocomplete: \"off\""));
}

#[test]
fn the_other_types_that_are_not_text_stay_refused() {
    for ty in ["checkbox", "date", "range", "Number", "number "] {
        let src = field("").replace("type: \"number\"", &format!("type: \"{ty}\""));
        assert_eq!(codes(&src), ["input-type-invalid"], "{ty}");
    }
}

#[test]
fn the_corpus_is_what_replay_says() {
    let c: Value =
        serde_json::from_str(include_str!("../../../tests/number_corpus.json")).expect("JSON");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 100, "{}", cases.len());
    for case in cases {
        let lang = case["lang"].as_str().and_then(cancho_ui_ir::i18n::lang);
        if !case["lang"].is_null() {
            assert!(
                lang.is_some(),
                "{}: a language the tables have",
                case["why"]
            );
        }
        let got = cancho_ui_replay::number_delivered(case["typed"].as_str().expect("typed"), lang);
        assert_eq!(
            got,
            case["delivered"].as_str().expect("delivered"),
            "{}",
            case["why"]
        );
    }
}

#[test]
fn a_trace_gives_the_number_as_the_field_would() {
    // English: 1.5 is the number; "1,5" is not one, so it is the text itself and a legal thing to type
    let (code, report) = replay(
        &field(""),
        json!([
            {"event": "V", "params": ["1.5"], "state": {"v": "1.5"}},
            {"event": "V", "params": ["1,5"], "state": {"v": "1,5"}},
            {"event": "V", "params": [""], "state": {"v": ""}},
            {"event": "V", "params": ["-"], "state": {"v": "-"}}
        ]),
    );
    assert_eq!(code, 0, "{report:#}");
    // spelled another way, a number is a page that does not exist
    for given in ["1.50", ".5", "1e5", "+1", "007", " 1", "-0"] {
        let (code, report) = replay(
            &field(""),
            json!([{"event": "V", "params": [given], "state": {"v": given}}]),
        );
        assert_eq!(code, 4, "{given}: {report:#}");
        assert_eq!(rules(&report), ["trace-number-invalid"], "{given}");
    }
}

#[test]
fn the_language_in_use_decides_what_is_a_number() {
    // Spanish first: "1,5" is the number 1.5 and a trace must say "1.5"; "1.5" is not a number there (a grouping mark), so it is text
    let src = localized();
    let (code, report) = replay(
        &src,
        json!([
            {"event": "V", "params": ["1.5"], "state": {"v": "1.5"}},
            {"event": "V", "params": ["2.5e3"], "state": {"v": "2.5e3"}}
        ]),
    );
    assert_eq!(code, 0, "{report:#}");
    let (code, report) = replay(
        &src,
        json!([{"event": "V", "params": ["1,5"], "state": {"v": "1,5"}}]),
    );
    assert_eq!(code, 4, "{report:#}");
    assert_eq!(rules(&report), ["trace-number-invalid"]);
}

#[test]
fn a_trace_for_a_text_field_is_not_checked_like_a_number_field() {
    let src = field("").replace("type: \"number\"", "type: \"text\"");
    let (code, report) = replay(
        &src,
        json!([{"event": "V", "params": ["1.50"], "state": {"v": "1.50"}}]),
    );
    assert_eq!(code, 0, "{report:#}");
}
