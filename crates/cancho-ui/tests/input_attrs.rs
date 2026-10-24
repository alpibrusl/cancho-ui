//! `autocomplete:`, `inputmode:`, `maxlength:` and `name:` of a `TextField` (docs/controls.md section 6): every case of
//! `tests/input_attrs_corpus.json` through the compiler and the replay in-process (the file `scripts/check-input-attrs.js` checks in
//! Chromium), and the rules that refuse a misuse.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/input_attrs_corpus.json");

fn program(bindings: &str) -> String {
    format!(
        "component F {{\n    state v: String = \"\";\n    state u: String = \"\";\n    on V(x) {{ v = x; }}\n    view {{ Column {{ TextField {{ label: \"Field\" value: v on: V {bindings} }} Text {{ \"got=[{{v}}]\" }} }} }}\n}}\n"
    )
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
    let dir = common::scratch_root().join(format!("cancho-ui-attrs-{}-{n}", std::process::id()));
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

#[test]
fn every_accepted_binding_is_written_as_the_attribute() {
    let c = corpus();
    let cases = c["accepted"].as_array().expect("a list");
    assert!(cases.len() >= 13, "the corpus shrank");
    for case in cases {
        let file = scratch(
            "a.cui",
            &program(case["binding"].as_str().expect("binding")),
        );
        let out = scratch("a.html", "");
        let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 0, "{}: {err}", case["binding"]);
        let html = std::fs::read_to_string(&out).expect("the page");
        let attr = case["attr"].as_str().expect("attr");
        let value = case["value"].as_str().expect("value");
        let single = format!("setAttribute('{attr}', '{value}')");
        let double = format!("setAttribute(\"{attr}\", \"{value}\")");
        assert!(
            html.contains(&single) || html.contains(&double),
            "{}: neither {single} nor {double}",
            case["binding"]
        );
    }
}

#[test]
fn every_refused_binding_has_its_rule() {
    let c = corpus();
    let cases = c["refused"].as_array().expect("a list");
    assert!(cases.len() >= 16, "the corpus shrank");
    for case in cases {
        let file = scratch(
            "r.cui",
            &program(case["binding"].as_str().expect("binding")),
        );
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{}: {stdout}", case["binding"]);
        let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
        let found: Vec<&str> = diags
            .as_array()
            .expect("a list")
            .iter()
            .map(|d| d["code"].as_str().expect("a code"))
            .collect();
        assert_eq!(
            found,
            [case["rule"].as_str().expect("rule")],
            "{}",
            case["binding"]
        );
    }
}

#[test]
fn these_bindings_belong_to_a_text_field() {
    for binding in [
        "autocomplete: \"username\"",
        "inputmode: \"numeric\"",
        "maxlength: 20",
        "name: \"user\"",
    ] {
        for element in [
            format!("Button {{ label: \"Go\" on: V(\"x\") {binding} }}"),
            format!("Row {{ {binding} Text {{ \"x\" }} }}"),
        ] {
            let src = format!(
                "component F {{\n    state v: String = \"\";\n    on V(x) {{ v = x; }}\n    view {{ Column {{ {element} }} }}\n}}\n"
            );
            let file = scratch("e.cui", &src);
            let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
            assert_eq!(code, 1, "{element}: {stdout}");
            assert!(stdout.contains("binding-unknown"), "{element}: {stdout}");
        }
    }
}

fn trace(value: &str) -> String {
    json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        {"event": "V", "params": [value], "state": {"v": value}}
    ]})
    .to_string()
}

fn report_for(maxlength: i64, value: &str) -> cancho_ui_replay::TestReport {
    let src = program(&format!("maxlength: {maxlength}"));
    let ast = parse(lex(&src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    replay(&ir, &trace(value), "prog.cui", "t.json")
}

#[test]
fn replay_takes_what_the_browser_reports_and_refuses_what_it_cannot() {
    let c = corpus();
    let cases = c["typing"].as_array().expect("a list");
    assert!(cases.len() >= 6, "the corpus shrank");
    let mut cut = 0;
    for case in cases {
        let max = case["maxlength"].as_i64().expect("maxlength");
        let received = case["received"].as_str().expect("received");
        let typed = case["typed"].as_str().expect("typed");
        let ok = report_for(max, received);
        assert!(
            ok.failures.is_empty() && ok.result == "pass",
            "{}: {:?}",
            case["why"],
            ok.failures
        );
        if typed != received {
            cut += 1;
            let bad = report_for(max, typed);
            let rules: Vec<&str> = bad.failures.iter().map(|f| f.rule).collect();
            assert_eq!(rules, ["trace-value-too-long"], "{}", case["why"]);
        }
    }
    assert!(cut >= 4, "the corpus must show the browser cutting a value");
}

#[test]
fn a_limit_binds_only_when_every_field_that_fires_the_event_has_one() {
    // Two fields fire V; one has no limit, so a long value can come from it.
    let src = "component F {\n    state v: String = \"\";\n    on V(x) { v = x; }\n    view { Column { TextField { label: \"A\" value: v on: V maxlength: 3 } TextField { label: \"B\" value: v on: V } } }\n}\n";
    let ast = parse(lex(src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    let r = replay(&ir, &trace("abcdefgh"), "prog.cui", "t.json");
    assert!(r.failures.is_empty(), "{:?}", r.failures);
}

#[test]
fn a_button_that_fires_the_event_can_give_it_a_long_value() {
    // The field allows 3 characters, but `Fill` can hand `V` any string, so a long value in a trace is possible.
    let src = "component F {\n    state v: String = \"\";\n    on V(x) { v = x; }\n    view { Column { TextField { label: \"A\" value: v on: V maxlength: 3 } Button { label: \"Fill\" on: V(\"abcdefgh\") } } }\n}\n";
    let ast = parse(lex(src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    let r = replay(&ir, &trace("abcdefgh"), "prog.cui", "t.json");
    assert!(r.failures.is_empty(), "{:?}", r.failures);
}
