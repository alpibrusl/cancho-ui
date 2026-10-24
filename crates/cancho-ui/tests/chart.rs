//! `Chart` (`docs/charts.md`, #77): the shared corpus `tests/chart_corpus.json` replayed in-process (the file `scripts/check-chart.js` runs
//! in Chromium), what the report says about a chart, and every rule that refuses a misuse.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/chart_corpus.json");

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
    let dir = common::scratch_root().join(format!("cancho-ui-chart-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn replay(src: &str, steps: Vec<Value>) -> (i32, Value) {
    let file = scratch("c.cui", src);
    let trace = scratch(
        "c.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (
        code,
        serde_json::from_str(&stdout).unwrap_or(Value::String(stderr)),
    )
}

#[test]
fn replay_reports_the_text_alternative_of_every_case_of_the_corpus() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let src = c["program"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 15, "the corpus shrank");
    let steps: Vec<Value> = cases
        .iter()
        .map(|k| {
            let expect = json!({"charts": [
                {"label": c["labelLine"], "kind": "line", "summary": k["line_summary"], "count": k["values"].as_array().map_or(0, |v| v.len().min(1000)), "total": k["values"].as_array().map_or(0, Vec::len)},
                {"label": c["labelBar"], "kind": "bar", "summary": k["bar_summary"]},
            ]});
            match k["event"].as_str() {
                None => json!({"advance": 0, "expect": expect}),
                Some(e) => json!({"event": e, "expect": expect}),
            }
        })
        .collect();
    let (code, report) = replay(&src, steps);
    assert_eq!(
        code,
        0,
        "{}",
        report.to_string().chars().take(3000).collect::<String>()
    );
}

fn comp(view: &str) -> String {
    format!(
        "component C {{\n    state xs: List[Float] = [1, 2, 3];\n    state is: List[Int] = [1];\n    state ss: List[String] = [\"a\"];\n    state name: String = \"T\";\n    state show: Bool = true;\n    state rows: List[String] = [\"r\"];\n    on Go {{ xs = [4, 5]; }}\n    view {{ Column {{ {view} }} }}\n}}\n"
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
fn a_chart_is_accepted_without_a_capability() {
    accepted(&comp("Chart { label: \"T\" kind: \"line\" values: xs }"));
    accepted(&comp(
        "Chart { label: \"T\" kind: \"bar\" values: is labels: ss unit: \"°C\" }",
    ));
    accepted(&comp(
        "Chart { label: \"T\" kind: \"bar\" values: [1, -2.5, 3] labels: [\"a\", \"b\", \"c\"] unit: \"123456789012\" }",
    ));
    accepted(&comp("Chart { label: name kind: \"line\" values: [] }"));
}

#[test]
fn a_chart_needs_a_label_a_kind_and_values() {
    for view in [
        "Chart { kind: \"line\" values: xs }",
        "Chart { label: \"T\" values: xs }",
        "Chart { label: \"T\" kind: \"line\" }",
        "Chart { }",
    ] {
        assert_eq!(codes(&comp(view)), ["chart-incomplete"], "{view}");
    }
}

#[test]
fn kind_values_labels_and_unit_are_checked() {
    for view in [
        "Chart { label: \"T\" kind: \"pie\" values: xs }",
        "Chart { label: \"T\" kind: \"\" values: xs }",
    ] {
        assert_eq!(codes(&comp(view)), ["chart-kind-invalid"], "{view}");
    }
    for view in [
        "Chart { label: \"T\" kind: \"line\" values: ss }",
        "Chart { label: \"T\" kind: \"line\" values: name }",
        "Chart { label: \"T\" kind: \"line\" values: [\"a\"] }",
        "Chart { label: \"T\" kind: \"line\" values: [1, name] }",
    ] {
        assert!(
            codes(&comp(view)) == ["chart-values-invalid"],
            "{view}: {:?}",
            codes(&comp(view))
        );
    }
    for view in [
        "Chart { label: \"T\" kind: \"line\" values: xs labels: xs }",
        "Chart { label: \"T\" kind: \"line\" values: xs labels: [1, 2] }",
    ] {
        assert_eq!(codes(&comp(view)), ["chart-labels-invalid"], "{view}");
    }
    for view in [
        "Chart { label: \"T\" kind: \"line\" values: xs unit: \"1234567890123\" }",
        "Chart { label: \"T\" kind: \"line\" values: xs unit: 3 }",
    ] {
        let c = codes(&comp(view));
        assert!(c == ["chart-unit-invalid"], "{view}: {c:?}");
    }
}

#[test]
fn a_chart_is_not_drawn_in_a_for() {
    let src = comp("For { over: rows as: r Chart { label: \"T\" kind: \"line\" values: xs } }");
    assert_eq!(codes(&src), ["chart-in-loop"]);
    accepted(&comp(
        "If { when: show Chart { label: \"T\" kind: \"line\" values: xs } }",
    ));
}

#[test]
fn the_keys_of_a_chart_belong_to_it() {
    for view in [
        "Text { kind: \"line\" \"x\" }",
        "Button { label: \"b\" values: xs on: Go }",
        "TextField { label: \"b\" unit: \"x\" value: name on: Go }",
        "Chart { label: \"T\" kind: \"line\" values: xs on: Go }",
        "Chart { label: \"T\" kind: \"line\" values: xs tone: \"ok\" }",
    ] {
        let c = codes(&comp(view));
        assert!(
            c == ["binding-unknown"] || c == ["parse-unexpected-token"],
            "{view}: {c:?}"
        );
    }
}

#[test]
fn a_hidden_chart_is_not_in_the_report_and_a_label_can_be_an_expression() {
    let src = comp("If { when: show Chart { label: name kind: \"line\" values: xs unit: \"u\" } } Button { label: \"hide\" on: Hide }")
        .replace("    on Go {", "    on Hide { show = false; }\n    on Go {");
    let (code, report) = replay(
        &src,
        vec![
            json!({"advance": 0, "expect": {"charts": [{"label": "T", "kind": "line", "count": 3, "total": 3, "summary": "line chart of 3 values: minimum 1 u, maximum 3 u, last 3 u"}]}}),
            json!({"event": "Go", "expect": {"charts": [{"summary": "line chart of 2 values: minimum 4 u, maximum 5 u, last 5 u"}]}}),
            json!({"event": "Hide", "expect": {"charts": []}}),
        ],
    );
    assert_eq!(code, 0, "{report}");
}

fn build(src: &str) -> String {
    let file = scratch("b.cui", src);
    let out = scratch("b.js", "");
    let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

#[test]
fn the_page_carries_the_chart_adapter_only_for_a_program_that_draws_one() {
    let plain = build(&comp("Button { label: \"x\" on: Go }"));
    assert!(!plain.contains("__cui_chart"), "{plain}");
    let js = build(&comp(
        "Chart { label: \"T\" kind: \"bar\" values: xs labels: ss unit: \"u\" }",
    ));
    assert!(
        js.contains("function __cui_chart(fig, label, kind, values, labels, unit)"),
        "{js}"
    );
    assert!(js.contains("__cui_chart(__n"), "{js}");
    assert!(js.contains("\"bar\""), "{js}");
    // nothing from the program is ever parsed as markup, and the chart's name is on the svg, not also on the figure
    assert!(!js.contains("innerHTML"), "{js}");
    assert!(!js.contains("insertAdjacentHTML"), "{js}");
    assert!(
        !js.contains("setAttribute('aria-label', \"T\")"),
        "the figure carries no name of its own:\n{js}"
    );
}

#[test]
fn the_diff_shows_a_chart_added() {
    let a = scratch("a.cui", &comp("Button { label: \"x\" on: Go }"));
    let b = scratch(
        "b.cui",
        &comp("Button { label: \"x\" on: Go } Chart { label: \"T\" kind: \"line\" values: xs }"),
    );
    let (code, out, err) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert!(code == 0 || code == 5, "{code} {out}{err}");
    assert!(out.contains("Chart"), "{out}");
}
