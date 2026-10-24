//! `min`, `max`, `abs`, `floor`, `ceil` and `round` (`docs/lists.md` section 8, issue #43): the model in replay against
//! `tests/math_corpus.json` (the file `scripts/check-lists.js` runs against the emitted JavaScript, comparing with
//! `Object.is`), what is emitted, what is refused, and a trace that counts pages with `ceil`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::math_builtin;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/math_corpus.json");

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
    let dir = common::scratch_root().join(format!("cancho-ui-math-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn number(v: &Value) -> f64 {
    match v {
        Value::String(s) if s == "NaN" => f64::NAN,
        Value::String(s) if s == "Infinity" => f64::INFINITY,
        Value::String(s) if s == "-Infinity" => f64::NEG_INFINITY,
        other => other.as_f64().expect("a number"),
    }
}

/// The same relation as `Object.is`: NaN is NaN, and zero has a sign.
fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

#[test]
fn the_model_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 60, "the corpus shrank");
    for e in entries {
        let op = e["op"].as_str().expect("an op");
        let args: Vec<f64> = e["args"]
            .as_array()
            .expect("args")
            .iter()
            .map(number)
            .collect();
        let want = number(&e["expect"]);
        let got = math_builtin(op, &args).expect("a known op with the right arity");
        assert!(
            same(got, want),
            "{op}({args:?}) [{}]: got {got:?}, expected {want:?}",
            e["why"]
        );
    }
}

#[test]
fn the_model_answers_nothing_for_an_unknown_name_or_the_wrong_arity() {
    assert!(math_builtin("sqrt", &[4.0]).is_none());
    assert!(math_builtin("min", &[1.0]).is_none());
    assert!(math_builtin("abs", &[1.0, 2.0]).is_none());
    assert!(math_builtin("round", &[]).is_none());
}

fn program(stmt: &str) -> String {
    format!(
        "component C {{\n    state n: Int = 0;\n    state m: Int = 0;\n    on Go {{ {stmt} }}\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n"
    )
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

#[test]
fn a_misused_builtin_is_refused_with_its_form() {
    for (stmt, form) in [
        ("n = min(m);", "min(a, b)"),
        ("n = min(m, 1, 2);", "min(a, b)"),
        ("n = max();", "max(a, b)"),
        ("n = abs(m, 1);", "abs(n)"),
        ("n = floor();", "floor(n)"),
        ("n = ceil(m, 2);", "ceil(n)"),
        ("n = round();", "round(n)"),
    ] {
        let src = program(stmt);
        assert_eq!(codes(&src, "bad.cui"), ["builtin-shape-mismatch"], "{stmt}");
        let file = scratch("bad.cui", &src);
        let (_, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert!(stdout.contains(form), "{stmt}: {stdout}");
    }
}

#[test]
fn the_helpers_are_emitted_once_and_only_for_a_program_that_calls_them() {
    let file = scratch(
        "pages.cui",
        &program("n = ceil(m / 5); m = max(n, 1) + abs(m);"),
    );
    let out = scratch("pages.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    for f in ["__cui_num_arg", "__cui_ceil", "__cui_max", "__cui_abs"] {
        assert_eq!(js.matches(&format!("function {f}(")).count(), 1, "{f}");
    }
    let file = scratch("plain.cui", &program("n = n + 1;"));
    let out = scratch("plain.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        !js.contains("__cui_num_arg"),
        "a program that calls none carries none"
    );
}

#[test]
fn a_trace_counts_pages_with_ceil_and_clamps_with_min_and_max() {
    let src = "component Pager {\n    state rows: Int = 23;\n    state page: Int = 0;\n    state pages: Int = 0;\n    state label: String = \"\";\n    on Open { pages = max(ceil(rows / 5), 1); label = \"page {page + 1} of {pages}\"; }\n    on Next { page = min(page + 1, pages - 1); label = \"page {page + 1} of {pages}\"; }\n    on Rows(n) { rows = n; pages = max(ceil(n / 5), 1); page = min(page, pages - 1); }\n    view { Text { \"{label}\" } }\n}\n";
    let file = scratch("pager.cui", src);
    let trace = scratch(
        "pager.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Pager","steps":[
            {"event":"Open","state":{"pages":5,"label":"page 1 of 5"}},
            {"event":"Next","state":{"page":1}},
            {"event":"Next","state":{"page":2}},
            {"event":"Next","state":{"page":3}},
            {"event":"Next","state":{"page":4,"label":"page 5 of 5"}},
            {"event":"Next","state":{"page":4,"label":"page 5 of 5"}},
            {"event":"Rows","params":[0],"state":{"pages":1,"page":0}},
            {"event":"Rows","params":[25],"state":{"pages":5}},
            {"event":"Rows","params":[26],"state":{"pages":6}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_trace_rounds_as_the_browser_does() {
    let src = "component R {\n    state a: Int = 0;\n    state b: Int = 0;\n    state c: Int = 0;\n    state d: Int = 0;\n    on Go(x) { a = round(x); b = floor(x); c = ceil(x); d = abs(x); }\n    view { Text { \"{a}\" } }\n}\n";
    let file = scratch("rounds.cui", src);
    let trace = scratch(
        "rounds.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"R","steps":[
            {"event":"Go","params":[2.5],"state":{"a":3,"b":2,"c":3,"d":2.5}},
            {"event":"Go","params":[-2.5],"state":{"a":-2,"b":-3,"c":-2,"d":2.5}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_non_number_argument_is_a_replay_type_error() {
    for stmt in ["n = min(\"a\", 1);", "n = abs(\"1\");", "n = ceil(true);"] {
        let file = scratch("nonnum.cui", &program(stmt));
        let trace = scratch(
            "nonnum.trace.json",
            r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"n":0}}]}"#,
        );
        let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
        assert_eq!(code, 4, "{stmt}: {stdout}");
        assert!(stdout.contains("replay-type-error"), "{stmt}: {stdout}");
    }
}

#[test]
fn a_zero_divided_by_zero_is_still_refused_not_hidden_by_a_builtin() {
    // `ceil` of a non-finite result is not a way round replay-non-finite (docs/conditionals.md section 5).
    let file = scratch("nan.cui", &program("n = ceil(m / 0);"));
    let trace = scratch(
        "nan.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"n":0}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-non-finite"), "{stdout}");
}
