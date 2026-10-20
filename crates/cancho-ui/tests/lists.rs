//! `sort`, `reverse`, `contains` and `lower` (`docs/lists.md`): the model in replay against the corpus
//! `tests/lists_corpus.json` (the same file `scripts/check-lists.js` runs against the emitted JavaScript), what is
//! emitted, what is refused, and traces that use them.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::sort_key_order_json;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/lists_corpus.json");

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
