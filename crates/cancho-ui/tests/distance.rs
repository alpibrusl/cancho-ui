//! `distance(a, b, default)` (docs/maps.md section 3.1): the corpus, the arity, the types and a replayed sort.
//! The page is checked in Chromium by `scripts/check-near.js`; `tests/distance_corpus.json` is run here and there.
mod common;

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
    let dir = common::scratch_root().join(format!("cancho-ui-sortlist-{}-{n}", std::process::id()));
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

const KM: &str = "state d: Float = 0.0;\n    state pos: String = \"40.42,-3.70\";";

fn comp(expr: &str) -> String {
    format!(
        "component T {{\n    {KM}\n    on Go {{ d = {expr}; }}\n    view {{ Column {{ Text {{ \"x\" }} }} }}\n}}\n"
    )
}

#[test]
fn distance_agrees_with_the_corpus() {
    let c: Value =
        serde_json::from_str(include_str!("../../../tests/distance_corpus.json")).expect("JSON");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 40);
    for case in cases {
        let got = cancho_ui_replay::distance_km(
            case["a"].as_str().expect("a"),
            case["b"].as_str().expect("b"),
            case["default"].as_f64().expect("default"),
        );
        match case["want"].as_f64() {
            Some(w) => assert!((got - w).abs() <= 1e-9, "{}: {got} vs {w}", case["why"]),
            None => assert_eq!(got, case["default"].as_f64().expect("d"), "{}", case["why"]),
        }
    }
}

#[test]
fn distance_is_a_builtin_with_three_arguments() {
    accepted(&comp("distance(pos, \"41.39,2.17\", -1)"));
    for call in [
        "distance(pos, pos)",
        "distance(pos)",
        "distance(pos, pos, 1, 2)",
        "distance()",
    ] {
        assert_eq!(codes(&comp(call)), ["builtin-shape-mismatch"], "{call}");
    }
}

#[test]
fn distance_in_a_sort_key_replays() {
    let rows = [
        "{\"name\":\"Barcelona\",\"pos\":\"41.39,2.17\"}",
        "{\"name\":\"Sol\",\"pos\":\"40.4169,-3.7035\"}",
        "{\"name\":\"Nowhere\",\"pos\":\"?\"}",
    ];
    // a brace in a string literal is written `\{` and `\}`
    let lit: Vec<String> = rows
        .iter()
        .map(|r| format!("{r:?}").replace('{', "\\{").replace('}', "\\}"))
        .collect();
    let src = format!(
        "component T {{\n    state places: List[String] = [{}];\n    state near: List[String] = [];\n    on Here(p) {{\n        near = sort(places, x => distance(p, json.get(x, \"pos\", \"\"), 20000));\n    }}\n    view {{ Column {{ Text {{ \"x\" }} }} }}\n}}\n",
        lit.join(", ")
    );
    accepted(&src);
    // from Madrid: Sol, then Barcelona, then the row whose position is unreadable (the default, 20000, sorts last)
    let (code, report) = replay(
        &src,
        json!([{"event": "Here", "params": ["40.42,-3.70"], "state": {"near": [rows[1], rows[0], rows[2]]}}]),
    );
    assert_eq!(code, 0, "{report:#}");
    // from Barcelona the order flips; the unreadable row is still last
    let (code, report) = replay(
        &src,
        json!([{"event": "Here", "params": ["41.39,2.17"], "state": {"near": [rows[0], rows[1], rows[2]]}}]),
    );
    assert_eq!(code, 0, "{report:#}");
    // a position the browser could not give (an empty text): every row is the default, so the list keeps its own order
    let (code, report) = replay(
        &src,
        json!([{"event": "Here", "params": [""], "state": {"near": [rows[0], rows[1], rows[2]]}}]),
    );
    assert_eq!(code, 0, "{report:#}");
}

#[test]
fn distance_needs_strings_and_a_number() {
    for call in [
        "distance(1, pos, 0)",
        "distance(pos, pos, \"x\")",
        "distance(pos, d, 0)",
    ] {
        let (code, report) = replay(&comp(call), json!([{"event": "Go", "state": {"d": 0}}]));
        assert_eq!(code, 4, "{call}: {report:#}");
        assert!(
            rules(&report).contains(&"replay-type-error".to_string()),
            "{call}: {report:#}"
        );
    }
}
