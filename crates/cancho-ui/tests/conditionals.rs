//! `if`, `&&`, `||` and `!` (docs/conditionals.md): every case of `tests/cond_corpus.json` replayed in-process (the file
//! `scripts/check-conditionals.js` runs against the emitted JavaScript in Chromium), the rules that refuse a misuse, and
//! what is emitted.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/cond_corpus.json");

/// The program one corpus case is run in: five fields the expressions read, and one handler that writes the answer.
fn program(expr: &str, state: &Value) -> String {
    let get = |k: &str, d: Value| state.get(k).cloned().unwrap_or(d);
    format!(
        "component Cond {{\n    state a: Int = {};\n    state b: Int = {};\n    state t: Bool = {};\n    state f: Bool = {};\n    state s: String = {};\n    state out: String = \"unset\";\n    on Go {{ out = {expr}; }}\n    view {{ Column {{ Button {{ label: \"Go\" on: Go }} Text {{ \"out={{out}}\" }} }} }}\n}}\n",
        get("a", json!(7)),
        get("b", json!(0)),
        get("t", json!(true)),
        get("f", json!(false)),
        get("s", json!("x")),
    )
}

fn replayed(src: &str, expect: &Value) -> (Vec<&'static str>, String) {
    let ast = parse(lex(src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    let step = if expect == "fault" {
        json!({"event": "Go", "state": {"out": "unset"}})
    } else {
        json!({"event": "Go", "state": {"out": expect}})
    };
    let trace =
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [step]}).to_string();
    let r = replay(&ir, &trace, "prog.cui", "t.json");
    (
        r.failures.iter().map(|f| f.rule).collect(),
        r.result.to_string(),
    )
}

#[test]
fn replay_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 50, "the corpus shrank");
    let (mut faults, mut shorts) = (0, 0);
    for e in entries {
        let expr = e["expr"].as_str().expect("an expr");
        let state = e.get("state").cloned().unwrap_or(json!({}));
        let (rules, result) = replayed(&program(expr, &state), &e["expect"]);
        if e["expect"] == "fault" {
            faults += 1;
            assert_eq!(rules, ["replay-non-finite"], "{expr} [{}]", e["why"]);
        } else {
            assert!(
                rules.is_empty() && result == "pass",
                "{expr} [{}]: {rules:?} {result}",
                e["why"]
            );
        }
        if e["why"]
            .as_str()
            .expect("why")
            .contains("does not evaluate")
        {
            shorts += 1;
        }
    }
    assert!(
        faults >= 6,
        "the corpus must refuse a division by zero in each position"
    );
    assert!(shorts >= 2, "the corpus must show short-circuit");
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

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = common::scratch_root().join(format!("cancho-ui-cond-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn handler(stmt: &str) -> String {
    handler_requiring("", stmt)
}

fn handler_requiring(requires: &str, stmt: &str) -> String {
    format!(
        "component C {{\n    {requires}\n    state a: Int = 1;\n    state t: Bool = true;\n    state s: String = \"x\";\n    state out: Int = 0;\n    on Go {{ {stmt} }}\n    view {{ Text {{ \"{{out}}\" }} }}\n}}\n"
    )
}

/// The rule tags `check` reports for `src` (it must be refused).
fn codes(src: &str, name: &str) -> Vec<String> {
    let file = scratch(name, src);
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
fn a_misused_condition_is_refused_with_its_rule() {
    for (stmt, rule) in [
        ("out = if t { 1 };", "if-else-required"),
        ("out = if t { 1 } else if a > 3 { 2 };", "if-else-required"),
        ("out = if a { 1 } else { 2 };", "condition-not-bool"),
        ("out = if s { 1 } else { 2 };", "condition-not-bool"),
        ("out = if [1] { 1 } else { 2 };", "condition-not-bool"),
        ("out = if len(s) { 1 } else { 2 };", "condition-not-bool"),
        ("out = if 1 + 1 { 1 } else { 2 };", "condition-not-bool"),
        ("t = !a;", "condition-not-bool"),
        ("t = a && t;", "condition-not-bool"),
        ("t = t || s;", "condition-not-bool"),
        ("t = t && 5;", "condition-not-bool"),
        (
            "out = if t { 1 } else { \"x\" };",
            "if-branch-type-mismatch",
        ),
        ("out = if t { t } else { 3 };", "if-branch-type-mismatch"),
        ("out = if t { [1] } else { 3 };", "if-branch-type-mismatch"),
        (
            "out = if t { 1 } else if a > 3 { \"no\" } else { 2 };",
            "if-branch-type-mismatch",
        ),
    ] {
        assert_eq!(codes(&handler(stmt), "bad.cui"), [rule], "{stmt}");
    }
}

#[test]
fn an_effect_in_a_condition_or_a_branch_is_refused() {
    for stmt in [
        "out = if storage.get(\"k\", \"d\") == \"d\" { 1 } else { 2 };",
        "out = if t { storage.get(\"k\", \"0\") } else { 2 };",
        "out = if t { 1 } else { await(sleep(5)) };",
    ] {
        let src = handler_requiring("requires Storage;", stmt);
        assert_eq!(
            codes(&src, "effect.cui"),
            ["if-effect-in-expression"],
            "{stmt}"
        );
    }
}

#[test]
fn a_misused_condition_in_the_view_is_refused_too() {
    let src = "component C {\n    state a: Int = 1;\n    view { Text { if a { \"x\" } else { \"y\" } } }\n}\n";
    assert_eq!(codes(src, "view.cui"), ["condition-not-bool"]);
}

#[test]
fn what_the_pass_cannot_see_it_does_not_refuse() {
    // An event parameter, a `For` binding and a `match` arm have no static type here; replay refuses the wrong one when it
    // runs (replay-type-error: no JavaScript truthiness) and the page would answer an operand, not a Bool.
    for stmt in [
        "out = if v { 1 } else { 2 };",
        "t = !v;",
        "t = v && t;",
        "t = !t || v;",
        "t = t && v;",
    ] {
        let src = format!(
            "component C {{\n    state out: Int = 0;\n    state t: Bool = true;\n    on Go(v) {{ {stmt} }}\n    view {{ Text {{ \"{{out}}\" }} }}\n}}\n"
        );
        let file = scratch("param.cui", &src);
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 0, "{stmt}: {stdout}");
        let trace = scratch(
            "param.trace.json",
            r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","params":[5],"state":{"out":1}}]}"#,
        );
        let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
        assert_eq!(code, 4, "{stmt}: {stdout}");
        assert!(stdout.contains("replay-type-error"), "{stmt}: {stdout}");
        assert!(stdout.contains("needs a bool"), "{stmt}: {stdout}");
    }
}

#[test]
fn a_single_ampersand_or_bar_is_still_a_lexical_error() {
    for stmt in ["t = t & t;", "t = t | t;"] {
        assert_eq!(
            codes(&handler(stmt), "amp.cui"),
            ["lex-unexpected-char"],
            "{stmt}"
        );
    }
}

#[test]
fn bang_equals_is_still_one_token() {
    let file = scratch("ne.cui", &handler("t = a != 2 && !t == false;"));
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
}

#[test]
fn the_emitted_code_uses_the_hosts_operators_with_explicit_parentheses() {
    let file = scratch(
        "emit.cui",
        &handler("t = !t || a > 1 && t; out = if t { 1 } else { 2 };"),
    );
    let out = scratch("emit.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        js.contains("((!__state_t) || ((__state_a > 1) && __state_t))"),
        "{js}"
    );
    assert!(js.contains("(__state_t ? 1 : 2)"), "{js}");
}

#[test]
fn the_literal_pattern_refusal_now_names_the_way_out() {
    let src = "component C {\n    state b: Bool = true;\n    state s: String = \"\";\n    on Go { s = match b { true => { \"y\" } false => { \"n\" } }; }\n    view { Text { \"{s}\" } }\n}\n";
    let file = scratch("lit.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains("pattern-literal-unsupported"), "{stdout}");
    assert!(stdout.contains("if cond { a } else { b }"), "{stdout}");
}

#[test]
fn the_semantic_diff_sees_an_if_and_a_not() {
    let base = scratch("d0.cui", &handler("out = 1;"));
    let cand = scratch("d1.cui", &handler("out = if !t { 1 } else { 2 };"));
    let (code, stdout, _) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("event Go: body changed"), "{stdout}");
    // And the same program again is no change: the diff reads the IR, which now carries the `if`.
    let (code, stdout, _) = run(&["diff", p(&cand), p(&cand)]);
    assert_eq!(code, 0, "{stdout}");
}
