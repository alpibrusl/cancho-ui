//! `==`, `!=`, `<`, `<=`, `>` and `>=` (docs/comparisons.md): every accepted case of `tests/compare_corpus.json` replayed
//! in-process (the file `scripts/check-comparisons.js` runs against the emitted JavaScript in Chromium), and every refused
//! case refused with `comparison-type-mismatch`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/compare_corpus.json");

/// The program one case is run in: fields of each type, one handler that writes the answer as text.
fn program(expr: &str) -> String {
    program_with(expr, "on Go {{ out = \"\" + ({expr}); }}")
}

/// The same, with the handler given (`{expr}` in it is the expression).
fn program_with(expr: &str, handler: &str) -> String {
    let handler = handler
        .replace("{{", "{")
        .replace("}}", "}")
        .replace("{expr}", expr);
    format!(
        "component Cmp {{\n    state a: Int = 7;\n    state f: Float = 7.5;\n    state s: String = \"7\";\n    state t: Bool = true;\n    state l: List[String] = [\"x\"];\n    state r: Result[String, String] = Ok(\"7\");\n    state reply: String = \"\\{{\\\"status\\\":200,\\\"body\\\":\\\"ok\\\"\\}}\";\n    state out: String = \"unset\";\n    {handler}\n    view {{ Column {{ Text {{ \"out={{out}}\" }} }} }}\n}}\n"
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
    let dir = common::scratch_root().join(format!("cancho-ui-cmp-{}-{n}", std::process::id()));
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
fn replay_agrees_with_every_accepted_case() {
    let c = corpus();
    let cases = c["accepted"].as_array().expect("a list");
    assert!(cases.len() >= 35, "the corpus shrank");
    let (mut trues, mut falses) = (0, 0);
    for case in cases {
        let expr = case["expr"].as_str().expect("an expr");
        let src = program(expr);
        let ast = parse(lex(&src).expect("lexes")).expect("parses");
        check_file(&ast).expect("type-checks");
        let ir = lower_file(&ast).expect("lowers");
        let want = case["expect"].as_bool().expect("a bool");
        if want {
            trues += 1;
        } else {
            falses += 1;
        }
        let trace = json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
            {"event": "Go", "state": {"out": want.to_string()}}
        ]})
        .to_string();
        let r = replay(&ir, &trace, "prog.cui", "t.json");
        assert!(
            r.failures.is_empty() && r.result == "pass",
            "{expr} [{}]: {:?}",
            case["why"],
            r.failures
        );
    }
    assert!(trues >= 15 && falses >= 7, "{trues} true, {falses} false");
}

#[test]
fn every_refused_case_has_its_rule() {
    let c = corpus();
    let cases = c["refused"].as_array().expect("a list");
    assert!(cases.len() >= 25, "the corpus shrank");
    for case in cases {
        let expr = case["expr"].as_str().expect("an expr");
        let file = scratch("r.cui", &program(expr));
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{expr}: {stdout}");
        let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
        let found: Vec<&str> = diags
            .as_array()
            .expect("a list")
            .iter()
            .map(|d| d["code"].as_str().expect("a code"))
            .collect();
        assert!(
            found.iter().all(|f| *f == "comparison-type-mismatch") && !found.is_empty(),
            "{expr} [{}]: {found:?}",
            case["why"]
        );
    }
}

#[test]
fn a_type_the_pass_cannot_see_is_not_refused() {
    let c = corpus();
    for case in c["unknown"].as_array().expect("a list") {
        let expr = case["expr"].as_str().expect("an expr");
        let file = scratch(
            "u.cui",
            &program_with(expr, "on Param(v) {{ out = \"\" + ({expr}); }}"),
        );
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 0, "{expr}: {stdout}");
    }
}

#[test]
fn the_remediation_names_num_and_a_string_literal() {
    let file = scratch(
        "h.cui",
        &program("json.get(reply, \"status\", \"\") == 200"),
    );
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains("num(text, default)"), "{stdout}");
    assert!(stdout.contains("string literal"), "{stdout}");
}
