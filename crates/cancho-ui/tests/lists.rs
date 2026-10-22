//! `sort`, `reverse`, `contains` and `lower` (`docs/lists.md`): the model in replay against the corpus
//! `tests/lists_corpus.json` (the same file `scripts/check-lists.js` runs against the emitted JavaScript), what is
//! emitted, what is refused, and traces that use them.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::{num_of_text, page_count, sort_key_order_json, text_builtin_json};
use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/lists_corpus.json");
const PAGING_CORPUS: &str = include_str!("../../../tests/paging_corpus.json");
const TEXT_CORPUS: &str = include_str!("../../../tests/text_corpus.json");
const NUM_CORPUS: &str = include_str!("../../../tests/num_corpus.json");
const CONCAT_CORPUS: &str = include_str!("../../../tests/concat_corpus.json");

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-lists-{}", std::process::id()));
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

const PROGRAM: &str = "component Rows {\n    state names: List[String] = [\"pear\", \"Apple\", \"fig\", \"apple\"];\n    state q: String = \"\";\n    state order: String = \"\";\n    on Go {\n        order = \"x\";\n    }\n    view {\n        Column {\n            For {\n                over: sort(filter(names, n => contains(lower(n), lower(q))), n => lower(n))\n                as: row\n                Text { \"{row}\" }\n            }\n        }\n    }\n}\n";

#[test]
fn the_model_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 15, "{} cases", entries.len());
    for e in entries {
        let keys = e["keys"].as_array().expect("keys");
        let mut idx: Vec<usize> = (0..keys.len()).collect();
        idx.sort_by(|&a, &b| sort_key_order_json(&keys[a], &keys[b]));
        let expect: Vec<usize> = e["expect"]
            .as_array()
            .expect("expect")
            .iter()
            .map(|n| n.as_u64().expect("an index") as usize)
            .collect();
        assert_eq!(idx, expect, "{}", e["why"]);
    }
}

#[test]
fn the_helpers_are_emitted_only_for_a_program_that_calls_them() {
    let file = scratch("rows.cui", PROGRAM);
    let out = scratch("rows.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    for f in ["__cui_sort", "__cui_contains", "__cui_lower"] {
        assert!(js.contains(&format!("function {f}(")), "{f}");
    }
    assert!(
        !js.contains("function __cui_reverse("),
        "reverse is not used"
    );
    // a program that uses none carries none
    let plain = "component C {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Text { \"{n}\" } }\n}\n";
    let file = scratch("plain.cui", plain);
    let out = scratch("plain.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(!js.contains("__cui_sort") && !js.contains("__cui_lower"));
}

#[test]
fn a_misused_builtin_is_refused_with_its_form() {
    for (call, form) in [
        ("sort(names)", "sort(list, x => key)"),
        ("sort(names, \"k\")", "x => key"),
        ("sort(names, n => n, n => n)", "sort(list, x => key)"),
        ("reverse()", "reverse(list)"),
        ("reverse(names, names)", "reverse(list)"),
        ("contains(q)", "contains(text, needle)"),
        ("lower()", "lower(text)"),
    ] {
        let src = PROGRAM.replace(
            "sort(filter(names, n => contains(lower(n), lower(q))), n => lower(n))",
            call,
        );
        assert_eq!(codes(&src, "bad.cui"), ["builtin-shape-mismatch"], "{call}");
        let file = scratch("bad.cui", &src);
        let (_, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert!(stdout.contains(form), "{call}: {stdout}");
    }
}

#[test]
fn a_sort_key_must_be_pure() {
    let src = PROGRAM.replace("n => lower(n))\n", "n => await(sleep(1)))\n");
    assert_eq!(codes(&src, "impure.cui"), ["pure-lambda-required"]);
}

#[test]
fn a_trace_sees_the_sorted_filtered_list() {
    let src = "component Rows {\n    state names: List[String] = [\"pear\", \"Apple\", \"fig\", \"apple\", \"Fig\"];\n    state q: String = \"\";\n    state shown: List[String] = [];\n    state n: Int = 0;\n    on Query(v) {\n        q = v;\n        shown = reverse(sort(filter(names, s => contains(lower(s), lower(v))), s => lower(s)));\n        n = len(shown);\n    }\n    view { Text { \"{n}\" } }\n}\n";
    let file = scratch("trace.cui", src);
    let trace = scratch(
        "trace.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Rows","steps":[
            {"event":"Query","params":["F"],"state":{"shown":["Fig","fig"],"n":2}},
            {"event":"Query","params":["p"],"state":{"shown":["pear","apple","Apple"]}},
            {"event":"Query","params":[""],"state":{"shown":["pear","Fig","fig","apple","Apple"],"n":5}},
            {"event":"Query","params":["zzz"],"state":{"shown":[],"n":0}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn sorting_keys_of_mixed_kinds_and_non_text_arguments_do_not_panic() {
    let src = "component C {\n    state a: List[String] = [\"b\", \"a\"];\n    state r: List[String] = [];\n    state t: Bool = false;\n    on Go { r = sort(a, x => 1); t = contains(1, \"a\"); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("mixed.cui", src);
    let trace = scratch(
        "mixed.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"r":["b","a"]}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}

#[test]
fn num_agrees_with_every_case_of_its_corpus() {
    let corpus: Value = serde_json::from_str(NUM_CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 40, "{} cases", entries.len());
    for e in entries {
        let text = e["text"].as_str().expect("text");
        let default = e["default"].as_f64().expect("default");
        let got = num_of_text(text, default);
        let want = if e["negzero"].as_bool() == Some(true) {
            -0.0
        } else {
            e["expect"].as_f64().expect("expect")
        };
        assert_eq!(got.to_bits(), want.to_bits(), "{text:?}: {got} vs {want}");
        assert!(!got.is_nan());
    }
}

#[test]
fn num_sorts_rows_by_a_number_held_in_text() {
    let src = "component Rows {\n    state out: List[String] = [];\n    on Go { out = reverse(sort([\"9\", \"10\", \"x\", \"2.5\", \"-1\"], s => num(s, 1000))); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("num.cui", src);
    let trace = scratch(
        "num.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Rows","steps":[
            {"event":"Go","state":{"out":["x","10","9","2.5","-1"]}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn num_is_refused_with_the_wrong_arguments_and_rejects_a_non_number_default() {
    for call in ["num(\"1\")", "num(\"1\", 2, 3)", "num()"] {
        let src = PROGRAM.replace(
            "sort(filter(names, n => contains(lower(n), lower(q))), n => lower(n))",
            call,
        );
        assert_eq!(
            codes(&src, "numbad.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
    let src = "component C {\n    state n: Int = 0;\n    on Go { n = num(\"1\", \"2\"); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("baddefault.cui", src);
    let trace = scratch(
        "baddefault.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"n":0}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}

fn paging_n(e: &Value) -> f64 {
    match &e["n"] {
        Value::String(s) if s == "NaN" => f64::NAN,
        Value::String(s) if s == "Infinity" => f64::INFINITY,
        Value::String(s) if s == "-Infinity" => f64::NEG_INFINITY,
        other => other.as_f64().expect("a number"),
    }
}

#[test]
fn take_and_skip_agree_with_every_case_of_their_corpus() {
    let corpus: Value = serde_json::from_str(PAGING_CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 15, "{} cases", entries.len());
    for e in entries {
        let list = e["list"].as_array().expect("list");
        let k = page_count(paging_n(e), list.len());
        assert_eq!(
            Value::Array(list[..k].to_vec()),
            e["take"],
            "take: {}",
            e["why"]
        );
        assert_eq!(
            Value::Array(list[k..].to_vec()),
            e["skip"],
            "skip: {}",
            e["why"]
        );
    }
}

#[test]
fn a_trace_pages_a_filtered_list() {
    let src = "component Rows {\n    state names: List[String] = [\"a\", \"b\", \"c\", \"d\", \"e\"];\n    state page: Int = 0;\n    state shown: List[String] = [];\n    on Page(p) {\n        page = p;\n        shown = take(skip(names, p * 2), 2);\n    }\n    view { Text { \"{page}\" } }\n}\n";
    let file = scratch("paging.cui", src);
    let trace = scratch(
        "paging.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Rows","steps":[
            {"event":"Page","params":[0],"state":{"shown":["a","b"]}},
            {"event":"Page","params":[1],"state":{"shown":["c","d"]}},
            {"event":"Page","params":[2],"state":{"shown":["e"]}},
            {"event":"Page","params":[3],"state":{"shown":[]}},
            {"event":"Page","params":[-1],"state":{"shown":["a","b"]}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn take_and_skip_are_emitted_once_even_when_both_are_used() {
    let src = "component C {\n    state names: List[String] = [\"a\", \"b\", \"c\"];\n    state r: List[String] = [];\n    on Go { r = take(skip(names, 1), 1); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("both.cui", src);
    let out = scratch("both.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    for f in ["__cui_page_count", "__cui_take", "__cui_skip"] {
        assert_eq!(js.matches(&format!("function {f}(")).count(), 1, "{f}");
    }
    // a program that pages nothing carries none of it
    let plain = "component C {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Text { \"{n}\" } }\n}\n";
    let file = scratch("nopage.cui", plain);
    let out = scratch("nopage.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        !std::fs::read_to_string(out)
            .expect("the built program")
            .contains("__cui_take")
    );
}

#[test]
fn take_and_skip_refuse_the_wrong_arguments_and_a_non_number_count() {
    for call in ["take(names)", "take(names, 1, 2)", "skip()", "skip(names)"] {
        let src = PROGRAM.replace(
            "sort(filter(names, n => contains(lower(n), lower(q))), n => lower(n))",
            call,
        );
        assert_eq!(
            codes(&src, "pagebad.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
    let src = "component C {\n    state names: List[String] = [\"a\"];\n    state r: List[String] = [];\n    on Go { r = take(names, \"1\"); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("pagetype.cui", src);
    let trace = scratch(
        "pagetype.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"r":[]}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}

/// An argument of the text corpus: a JSON value, or `{"repeat": [unit, n]}` for a long text.
fn text_arg(a: &Value) -> Value {
    match a.get("repeat") {
        Some(r) => Value::String(
            r[0].as_str()
                .expect("a unit")
                .repeat(r[1].as_u64().expect("a count") as usize),
        ),
        None => a.clone(),
    }
}

#[test]
fn the_text_builtins_agree_with_every_case_of_their_corpus() {
    let corpus: Value = serde_json::from_str(TEXT_CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 55, "{} cases", entries.len());
    for e in entries {
        let args: Vec<Value> = e["args"]
            .as_array()
            .expect("args")
            .iter()
            .map(text_arg)
            .collect();
        let got = text_builtin_json(e["op"].as_str().expect("op"), &args)
            .unwrap_or_else(|| panic!("{}: no answer", e["why"]));
        if let Some(n) = e["expect_len"].as_u64() {
            assert_eq!(
                got.as_array().expect("a list").len() as u64,
                n,
                "{}",
                e["why"]
            );
        } else {
            assert_eq!(got, e["expect"], "{}", e["why"]);
        }
    }
}

#[test]
fn a_text_builtin_with_the_wrong_arity_or_a_non_text_argument_is_refused() {
    for (call, form) in [
        ("lines()", "lines(text)"),
        ("lines(q, q)", "lines(text)"),
        ("split(q)", "split(text, sep)"),
        ("join(names)", "join(list, sep)"),
        ("trim(q, q)", "trim(text)"),
        ("upper()", "upper(text)"),
        ("starts_with(q)", "starts_with(text, prefix)"),
        ("ends_with(q, q, q)", "ends_with(text, suffix)"),
    ] {
        let src = PROGRAM.replace(
            "sort(filter(names, n => contains(lower(n), lower(q))), n => lower(n))",
            call,
        );
        assert_eq!(
            codes(&src, "textarity.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
        let file = scratch("textarity.cui", &src);
        let (_, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert!(stdout.contains(form), "{call}: {stdout}");
    }
    let src = "component C {\n    state r: List[String] = [];\n    on Go { r = lines(1); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("textnotext.cui", src);
    let trace = scratch(
        "textnotext.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"r":[]}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}

#[test]
fn a_trace_turns_a_text_answer_into_rows() {
    // The scenario of cancho-ui#18: an endpoint answers one reading per line; the page renders a row for each.
    let src = "component Readings {\n    requires Network;\n    state rows: List[String] = [];\n    state n: Int = 0;\n    state first: String = \"\";\n    state r: Result[String, String] = Ok(\"\");\n    on Refresh {\n        r = await(net.get(\"readings.txt\"));\n        rows = match r { Ok(t) => { map(lines(t), l => trim(upper(l))) } Err(e) => { [] } Cancelled => { [] } };\n        n = len(rows);\n        first = join(take(rows, 2), \"|\");\n    }\n    view { For { over: rows as: row Text { \"{row}\" } } }\n}\n";
    let file = scratch("rows.cui", src);
    let trace = scratch(
        "rows.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Readings","steps":[
            {"event":"Refresh","expect":{"pending":[1]}},
            {"resolve":1,"with":{"Ok":"temp 21.5 \r\n  hum 40\n\npress 1013\n"},"state":{"rows":["TEMP 21.5","HUM 40","","PRESS 1013"],"n":4,"first":"TEMP 21.5|HUM 40"}},
            {"event":"Refresh","expect":{"pending":[2]}},
            {"resolve":2,"with":{"Ok":""},"state":{"rows":[],"n":0,"first":""}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

/// `concat(a, b)` (docs/lists.md section 9) through the real replay: one trace step per case of
/// `tests/concat_corpus.json`, the same file `scripts/check-lists.js` runs against the emitted `__cui_concat`.
#[test]
fn concat_agrees_with_every_case_of_its_corpus() {
    let corpus: Value = serde_json::from_str(CONCAT_CORPUS).expect("the corpus is JSON");
    let ok = corpus["ok"].as_array().expect("ok cases");
    let refused = corpus["refused"].as_array().expect("refused cases");
    assert!(
        ok.len() >= 10 && refused.len() >= 6,
        "{} + {}",
        ok.len(),
        refused.len()
    );
    let src = "component Cat {\n    state out: List[String] = [];\n    on Go(a, b) { out = concat(a, b); }\n    view { Text { \"{len(out)}\" } }\n}\n";
    let mut steps = Vec::new();
    let mut last = serde_json::json!([]);
    for e in ok {
        steps.push(serde_json::json!({
            "event": "Go", "params": [e["a"], e["b"]], "state": {"out": e["concat"]}
        }));
        last = e["concat"].clone();
    }
    for e in refused {
        // A fault leaves the state as it was (docs/semantics.md Q3), so the list from the last good step is still there.
        steps.push(serde_json::json!({
            "event": "Go", "params": [e["a"], e["b"]], "fault": e["rule"], "state": {"out": last}
        }));
    }
    let trace = serde_json::json!({
        "schema": "cancho-ui-trace", "version": "0.1.0", "component": "Cat", "steps": steps
    });
    let file = scratch("concat.cui", src);
    let trace = scratch("concat.trace.json", &trace.to_string());
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn plus_on_lists_is_refused_and_concat_is_the_way() {
    let wrap = |stmt: &str| {
        format!(
            "component C {{\n    state l: List[String] = [\"a\"];\n    state m: List[String] = [\"b\"];\n    state n: Int = 0;\n    state b: Bool = true;\n    state s: String = \"\";\n    state r: Result[Int, String] = Ok(0);\n    on Go {{ {stmt} }}\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n"
        )
    };
    for stmt in [
        "l = l + m;",
        "l = l + [\"x\"];",
        "s = \"x\" + l;",
        "s = l + \"x\";",
        "n = n + b;",
        "n = b + n;",
        "n = b + b;",
        "n = n + r;",
    ] {
        assert_eq!(
            codes(&wrap(stmt), "plus.cui"),
            ["operand-type-mismatch"],
            "{stmt}"
        );
    }
    for stmt in [
        "l = concat(l, m);",
        "s = \"x\" + b;",
        "s = \"x\" + n;",
        "s = n + \"x\";",
        "n = n + 1;",
    ] {
        let file = scratch("plus-ok.cui", &wrap(stmt));
        let (code, stdout, stderr) = run(&["check", p(&file)]);
        assert_eq!(code, 0, "{stmt}: {stdout}{stderr}");
    }
}

#[test]
fn concat_with_the_wrong_number_of_arguments_names_its_form() {
    for call in ["concat()", "concat(l)", "concat(l, l, l)"] {
        let src = format!(
            "component C {{\n    state l: List[String] = [\"a\"];\n    on Go {{ l = {call}; }}\n    view {{ Text {{ \"x\" }} }}\n}}\n"
        );
        let file = scratch("concat-arity.cui", &src);
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{call}: {stdout}");
        assert!(
            stdout.contains("builtin-shape-mismatch"),
            "{call}: {stdout}"
        );
        assert!(stdout.contains("concat(a, b)"), "{call}: {stdout}");
    }
}

#[test]
fn the_concat_helper_is_emitted_only_when_used() {
    let used = "component C {\n    state l: List[String] = [\"a\"];\n    on Go { l = concat(l, l); }\n    view { Text { \"x\" } }\n}\n";
    let unused = "component C {\n    state l: List[String] = [\"a\"];\n    on Go { l = reverse(l); }\n    view { Text { \"x\" } }\n}\n";
    for (src, has) in [(used, true), (unused, false)] {
        let file = scratch("concat-emit.cui", src);
        let out = std::env::temp_dir().join(format!("cancho-ui-lists-{}", std::process::id()));
        let js = out.join("concat-emit.js");
        let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&js)]);
        assert_eq!(code, 0, "{stdout}{stderr}");
        let text = std::fs::read_to_string(&js).expect("the built module");
        assert_eq!(text.contains("function __cui_concat("), has);
    }
}
