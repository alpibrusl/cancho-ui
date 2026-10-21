//! `json.get(text, path, default)` (`docs/language-design.md` decision 13): the model in replay against the corpus
//! `tests/json_get_corpus.json`, what is emitted, what is refused, and a trace that uses it. The same corpus is run
//! against the emitted JavaScript by `scripts/check-json-get.js` (outside the gate: it needs node).

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::{json_get, json_items};
use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/json_get_corpus.json");

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-json-get-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// The text, path and default of a corpus entry: `nest` n is `[[...7...]]` n deep read at `0.0...0`.
fn case(entry: &Value) -> (String, String, String) {
    if let Some(n) = entry["nest"].as_u64() {
        let n = n as usize;
        let text = format!("{}7{}", "[".repeat(n), "]".repeat(n));
        let path = vec!["0"; n].join(".");
        return (text, path, "?".to_string());
    }
    (
        entry["text"].as_str().expect("text").to_string(),
        entry["path"].as_str().expect("path").to_string(),
        entry["default"].as_str().expect("default").to_string(),
    )
}

#[test]
fn the_model_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 75, "{} cases", entries.len());
    let mut gets = 0;
    let mut lists = 0;
    for entry in entries {
        let why = entry["why"].as_str().unwrap_or("?");
        if let Some(n) = entry["items_n"].as_u64() {
            let text = format!("[{}]", vec!["1"; n as usize].join(","));
            assert_eq!(
                json_items(&text, "").len() as u64,
                entry["expect_len"].as_u64().expect("expect_len"),
                "{why}"
            );
            lists += 1;
        } else if entry["items"].as_bool() == Some(true) {
            let then = entry["then"].as_str().expect("then");
            let got: Vec<String> = json_items(
                entry["text"].as_str().expect("text"),
                entry["path"].as_str().expect("path"),
            )
            .iter()
            .map(|item| json_get(item, then, "?"))
            .collect();
            let expect: Vec<String> = entry["expect"]
                .as_array()
                .expect("a list")
                .iter()
                .map(|v| v.as_str().expect("a string").to_string())
                .collect();
            assert_eq!(got, expect, "{why}");
            lists += 1;
        } else {
            let (text, path, default) = case(entry);
            let expect = entry["expect"].as_str().expect("expect");
            assert_eq!(json_get(&text, &path, &default), expect, "{why}");
            gets += 1;
        }
    }
    assert!(gets >= 60 && lists >= 15, "{gets} gets, {lists} lists");
}

#[test]
fn nothing_in_a_text_reaches_a_panic() {
    // every prefix and every single-byte change of a document, and some depth: an answer each time
    let doc = r#"{"a":[1,{"b":"cé😀"},true,null,-0,1e3],"d":{"e":{"f":"g"}}}"#;
    for cut in 0..=doc.len() {
        if doc.is_char_boundary(cut) {
            for path in ["", "a", "a.1.b", "d.e.f", "a.9", "x.y.z"] {
                let _ = json_get(&doc[..cut], path, "?");
            }
        }
    }
    for at in 0..doc.len() {
        for byte in [b'"', b'{', b'}', b'[', b']', b':', b',', b'x', 0u8, 0xff] {
            let mut bytes = doc.as_bytes().to_vec();
            bytes[at] = byte;
            if let Ok(text) = String::from_utf8(bytes) {
                let _ = json_get(&text, "a.1.b", "?");
            }
        }
    }
    let deep = "[".repeat(100_000);
    assert_eq!(json_get(&deep, "0.0", "?"), "?");
}

const PLANT: &str = "component Plant {\n    requires Network;\n\n    state r: Result[String, String] = Ok(\"\");\n    state ready: String = \"unknown\";\n    state reqs: String = \"-\";\n\n    on Refresh {\n        r = await(net.get(\"metrics.json\"));\n        ready = match r {\n            Ok(text) => { json.get(text, \"ready\", \"unknown\") }\n            Err(why) => { \"error: {why}\" }\n            Cancelled => { \"cancelled\" }\n        };\n        reqs = match r {\n            Ok(text) => { json.get(text, \"routes.0.requests\", \"-\") }\n            Err(_) => { \"-\" }\n            Cancelled => { \"-\" }\n        };\n    }\n\n    view {\n        Column {\n            Button { label: \"Refresh\" on: Refresh }\n            Text { \"ready: {ready}\" }\n            Text { \"requests: {reqs}\" }\n        }\n    }\n}\n";

#[test]
fn a_program_that_reads_a_fetched_document_builds_and_replays() {
    let src = scratch("plant.cui", PLANT);
    let trace = scratch(
        "plant.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Plant","steps":[
            {"event":"Refresh","expect":{"issued":[{"effect":"net.get"}]}},
            {"resolve":1,"with":{"Ok":"{\"ready\":true,\"routes\":[{\"name\":\"api\",\"requests\":4021}]}"},
             "state":{"ready":"true","reqs":"4021"}},
            {"event":"Refresh"},
            {"resolve":2,"with":{"Ok":"not json"},"state":{"ready":"unknown","reqs":"-"}},
            {"event":"Refresh"},
            {"resolve":3,"with":{"Err":"HTTP 503"},"state":{"ready":"error: HTTP 503","reqs":"-"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&src), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let out = scratch("plant.js", "");
    let (code, _, stderr) = run(&["build", p(&src), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(js.contains("function __cui_json_get(text, path, fallback)"));
    assert!(
        js.contains("__cui_json_get(text, \"ready\", \"unknown\")"),
        "{js}"
    );
}

#[test]
fn the_helper_is_emitted_only_for_a_program_that_calls_json_get() {
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    let file = scratch("counter.cui", &counter);
    let out = scratch("counter.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(!js.contains("__cui_json_get"));
}

#[test]
fn json_get_needs_no_capability_and_exactly_three_arguments() {
    // no `requires` for the call itself: it is pure
    let pure = "component P {\n    state v: String = \"\";\n    on Go { v = json.get(\"\\{\\\"a\\\":\\\"b\\\"\\}\", \"a\", \"?\"); }\n    view { Text { v } }\n}\n";
    let file = scratch("pure.cui", pure);
    let (code, stdout, stderr) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    for call in [
        "json.get()",
        "json.get(\"a\")",
        "json.get(\"a\", \"b\")",
        "json.get(\"a\", \"b\", \"c\", \"d\")",
    ] {
        let bad = pure.replace(
            "json.get(\"\\{\\\"a\\\":\\\"b\\\"\\}\", \"a\", \"?\")",
            call,
        );
        let file = scratch("bad.cui", &bad);
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{call}: {stdout}");
        let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
        assert_eq!(diags[0]["code"], "builtin-shape-mismatch", "{call}");
    }
}

#[test]
fn json_get_with_a_non_string_argument_is_a_replay_type_error() {
    let src = "component P {\n    state v: String = \"\";\n    on Go { v = json.get(1, \"a\", \"?\"); }\n    view { Text { v } }\n}\n";
    let file = scratch("typeerr.cui", src);
    let trace = scratch(
        "typeerr.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"P","steps":[{"event":"Go","state":{"v":"?"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_ne!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("replay-type-error") || stderr.contains("replay-type-error"),
        "{stdout}{stderr}"
    );
}

#[test]
fn the_plant_example_trace_passes() {
    let (code, stdout, stderr) = run(&[
        "test",
        "examples/plant/plant.cui",
        "examples/plant/plant.trace.json",
    ]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("PASS"), "{stdout}");
    assert!(
        stdout.contains("issued #1 net.get(\"admin/metrics\")"),
        "{stdout}"
    );
}

#[test]
fn json_items_takes_two_arguments_and_the_list_is_what_a_for_reads() {
    let src = "component L {\n    state xs: List[String] = [];\n    state n: Int = 0;\n    on Go { xs = json.items(\"x\", \"a\"); n = len(xs); }\n    view { Text { \"{n}\" } }\n}\n";
    let file = scratch("items.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    for call in [
        "json.items()",
        "json.items(\"a\")",
        "json.items(\"a\", \"b\", \"c\")",
    ] {
        let bad = src.replace("json.items(\"x\", \"a\")", call);
        let file = scratch("items-bad.cui", &bad);
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{call}: {stdout}");
        let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
        assert_eq!(diags[0]["code"], "builtin-shape-mismatch", "{call}");
    }
}

#[test]
fn a_text_body_may_be_a_concatenation_that_starts_with_a_string_literal() {
    let src = "component C {\n    state who: String = \"Ada\";\n    on Go { who = \"Lin\"; }\n    view { Column { Text { \"hello, \" + who } Text { \"{who}!\" } Button { label: \"Go\" on: Go } } }\n}\n";
    let file = scratch("concat.cui", src);
    let trace = scratch(
        "concat.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Go","state":{"who":"Lin"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let out = scratch("concat.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        js.contains("\"hello, \" + __state_who") || js.contains("hello, "),
        "{js}"
    );
}
