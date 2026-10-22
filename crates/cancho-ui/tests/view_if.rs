//! `If { when: cond ... }` in a view (docs/view-conditional.md): every case of `tests/view_if_corpus.json` replayed
//! in-process (the file `scripts/check-view-if.js` runs against the emitted page in Chromium), the rules that refuse a
//! misuse, and what is emitted.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/view_if_corpus.json");

fn source(case: &Value) -> String {
    case["source"]
        .as_array()
        .expect("source lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The trace of a case: each step names its event, the state it leaves and the outcome replay must give.
fn trace_of(case: &Value) -> String {
    let steps: Vec<Value> = case["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| {
            json!({
                "event": s["event"],
                "params": s.get("args").cloned().unwrap_or(json!([])),
                "state": s["state"],
                "expect": {"outcome": s["outcome"]},
            })
        })
        .collect();
    json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string()
}

#[test]
fn replay_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let cases = corpus.as_array().expect("a list");
    assert!(cases.len() >= 7, "the corpus shrank");
    let mut hidden = 0;
    let mut disabled = 0;
    for case in cases {
        let src = source(case);
        let ast = parse(lex(&src).expect("lexes")).expect("parses");
        check_file(&ast).expect("type-checks");
        let ir = lower_file(&ast).expect("lowers");
        let report = replay(&ir, &trace_of(case), "prog.cui", "t.json");
        let rules: Vec<&str> = report.failures.iter().map(|f| f.rule).collect();
        assert!(
            rules.is_empty() && report.result == "pass",
            "{}: {rules:?} {}\n{:#?}",
            case["why"],
            report.result,
            report.failures
        );
        for s in case["steps"].as_array().expect("steps") {
            match s["outcome"].as_str().expect("an outcome") {
                "hidden" => hidden += 1,
                "disabled" => disabled += 1,
                _ => {}
            }
        }
    }
    assert!(hidden >= 6, "the corpus must refuse a hidden control often");
    assert!(disabled >= 1, "the corpus must tell hidden from disabled");
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
    let dir = std::env::temp_dir().join(format!("cancho-ui-viewif-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(view: &str) -> String {
    format!(
        "component C {{\n    state a: Int = 1;\n    state t: Bool = true;\n    state s: String = \"x\";\n    state l: List[String] = [];\n    on Go {{ a = a + 1; }}\n    view {{ {view} }}\n}}\n"
    )
}

/// The rule tags `check` reports for `src` (it must be refused).
fn codes(src: &str) -> Vec<String> {
    let file = scratch("bad.cui", src);
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
fn a_condition_that_is_not_a_bool_is_refused() {
    for when in ["a", "s", "l", "a + 1", "len(s)", "\"yes\""] {
        let src = program(&format!(
            "Column {{ If {{ when: {when} Text {{ \"x\" }} }} }}"
        ));
        assert_eq!(codes(&src), ["condition-not-bool"], "{when}");
    }
}

#[test]
fn a_condition_that_names_nothing_is_refused() {
    let src = program("Column { If { when: missing Text { \"x\" } } }");
    assert_eq!(codes(&src), ["name-unknown"]);
}

#[test]
fn a_condition_that_reads_the_world_is_refused() {
    let src = "component C {\n    requires Storage;\n    state s: String = \"x\";\n    on Go { s = storage.get(\"k\", \"d\"); }\n    view { Column { If { when: storage.get(\"k\", \"d\") == \"d\" Text { \"x\" } } } }\n}\n";
    let found = codes(src);
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn an_if_without_a_condition_is_a_parse_error() {
    for view in [
        "Column { If { Text { \"x\" } } }",
        "Column { If { } }",
        "Column { If { label: \"x\" Text { \"x\" } } }",
    ] {
        assert_eq!(codes(&program(view)), ["parse-unexpected-token"], "{view}");
    }
}

#[test]
fn the_page_has_no_node_for_a_false_condition() {
    let src = program("Column { If { when: t Text { \"x\" } } }");
    let file = scratch("emit.cui", &src);
    let out = scratch("emit.html", "");
    let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    assert!(
        html.contains("if (__state_t) {"),
        "an if, not a hidden node"
    );
    assert!(
        !html.contains("display = 'none'") && !html.contains(".hidden"),
        "nothing is hidden with a style"
    );
}

#[test]
fn the_semantic_diff_names_the_condition() {
    let a = scratch(
        "a.cui",
        &program("Column { If { when: t Text { \"x\" } } }"),
    );
    let b = scratch(
        "b.cui",
        &program("Column { If { when: !t Text { \"x\" } } }"),
    );
    let (code, stdout, _) = run(&["diff", p(&a), p(&b)]);
    assert_eq!(code, 5, "{stdout}");
    assert!(
        stdout.contains("+ view If when=!t") && stdout.contains("- view If when=t"),
        "{stdout}"
    );
}
