//! Braces in strings and `json.quote` (`docs/language-design.md` decision 15, issue #38): `\{` and `\}` write a literal
//! brace, anything left in an interpolation after its expression is a parse error (it used to be dropped without a
//! diagnostic), and `json.quote` is the JSON string literal of a text, the same in replay and in the emitted JavaScript
//! (`tests/json_quote_corpus.json`, run against the JavaScript by `scripts/check-lists.js`).
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::json_quote;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/json_quote_corpus.json");

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
    let dir = common::scratch_root().join(format!("cancho-ui-braces-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn codes(src: &str, name: &str) -> Vec<String> {
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

/// A program with one string `$S` in a handler, a label and a `Text`.
fn with(handler_string: &str) -> String {
    format!(
        "component C {{\n    state s: String = \"\";\n    on Go {{ s = {handler_string}; }}\n    view {{ Text {{ \"{{s}}\" }} }}\n}}\n"
    )
}

fn trace_state(src: &str, name: &str, expect: &str) -> (i32, String) {
    let file = scratch(&format!("{name}.cui"), src);
    let trace = scratch(
        &format!("{name}.trace.json"),
        &format!(
            r#"{{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{{"event":"Go","state":{{"s":{expect}}}}}]}}"#
        ),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    (code, format!("{stdout}{stderr}"))
}

#[test]
fn json_quote_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 20, "{} cases", entries.len());
    for e in entries {
        let text = e["text"].as_str().expect("text");
        assert_eq!(
            json_quote(text),
            e["expect"].as_str().expect("expect"),
            "{}",
            e["why"]
        );
    }
}

#[test]
fn a_backslash_brace_is_a_literal_brace_in_every_kind_of_string() {
    // a string expression
    let (code, out) = trace_state(&with(r#""\{\"a\":1\}""#), "expr", r#""{\"a\":1}""#);
    assert_eq!(code, 0, "{out}");
    // next to a real interpolation
    let (code, out) = trace_state(&with(r#""n={1 + 1}\{x\}""#), "mixed", r#""n=2{x}""#);
    assert_eq!(code, 0, "{out}");
    // in a Text body and in static text (a label), where a real brace is still refused
    let src = "component C {\n    state s: String = \"\";\n    on Go { s = \"x\"; }\n    view { Column { Text { \"\\{literal\\} {s}\" } Button { label: \"press \\{me\\}\" on: Go } } }\n}\n";
    let file = scratch("view.cui", src);
    let out = scratch("view.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(js.contains("\"{literal} \""), "{js}");
    assert!(js.contains("press {me}"), "{js}");
    assert!(
        !js.contains('\u{e000}') && !js.contains('\u{e001}'),
        "a mark leaked into the output"
    );
    let real = src.replace("press \\{me\\}", "press {me}");
    assert_eq!(codes(&real, "real.cui"), ["parse-unexpected-token"]);
}

#[test]
fn what_is_left_in_an_interpolation_is_an_error_not_dropped() {
    // issue #38: `"{\"a\":1}"` was `"" + "a"` with `:1` dropped, without a diagnostic
    assert_eq!(
        codes(&with(r#""{\"a\":1}""#), "stray.cui"),
        ["parse-unexpected-token"]
    );
    assert_eq!(
        codes(&with(r#""{1 2}""#), "two.cui"),
        ["parse-unexpected-token"]
    );
    let file = scratch("stray2.cui", &with(r#""{\"a\":1}""#));
    let (_, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert!(
        stdout.contains("\\{"),
        "the remediation names the escape: {stdout}"
    );
}

#[test]
fn json_quote_builds_an_object_body_that_typed_text_cannot_change() {
    // the payload of issue #38: an id that tries to add a field
    let src = "component C {\n    state id: String = \"\";\n    state s: String = \"\";\n    on SetId(v) { id = v; }\n    on Go { s = \"\\{\\\"id\\\":\" + json.quote(id) + \",\\\"setpoint\\\":55\\}\"; }\n    view { Text { \"{s}\" } }\n}\n";
    let file = scratch("body.cui", src);
    let trace = scratch(
        "body.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[
            {"event":"SetId","params":["M-102\", 99, \"x"]},
            {"event":"Go","state":{"s":"{\"id\":\"M-102\\\", 99, \\\"x\",\"setpoint\":55}"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn json_quote_is_emitted_only_when_called_and_takes_one_text() {
    let src = "component C {\n    state s: String = \"\";\n    on Go { s = json.quote(\"a\"); }\n    view { Text { \"{s}\" } }\n}\n";
    let file = scratch("q.cui", src);
    let out = scratch("q.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(js.contains("function __cui_json_quote("), "{js}");
    assert!(js.contains("__cui_json_quote(\"a\")"), "{js}");
    let plain = with("\"a\"");
    let file = scratch("noq.cui", &plain);
    let out = scratch("noq.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        !std::fs::read_to_string(out)
            .expect("js")
            .contains("__cui_json_quote")
    );
    for call in ["json.quote()", "json.quote(\"a\", \"b\")"] {
        let bad = with(call);
        assert_eq!(
            codes(&bad, "arity.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
    // a non-text argument is a replay type error
    let bad = "component C {\n    state s: String = \"\";\n    on Go { s = json.quote(1); }\n    view { Text { \"{s}\" } }\n}\n";
    let file = scratch("num.cui", bad);
    let trace = scratch(
        "num.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"s":""}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}
