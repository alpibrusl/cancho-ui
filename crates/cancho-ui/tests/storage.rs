//! The `Storage` adapter (`docs/adapters.md`): what the compiler emits, what it refuses, how a trace replays
//! it, and that the semantic diff sees the capability. The browser-side checks (a note survives a reload;
//! storage that throws) are `scripts/check-storage.js`, outside the gate because they need a browser.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_diff::diff_ir;
use cancho_ui_ir::lower_file;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::Value;

const NOTES: &str = include_str!("../../../examples/notes/notes.cui");

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

/// A scratch file holding `text`, named for the test.
fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cancho-ui-storage-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn build_js(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

fn check_codes(src: &str, name: &str) -> Vec<String> {
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
fn the_adapter_is_emitted_for_a_program_that_requires_storage_and_for_no_other() {
    let js = build_js(NOTES, "notes.cui");
    assert!(js.contains("function __cui_storage_get(key, fallback, ns)"));
    assert!(js.contains("function __cui_storage_set(key, value, ns)"));
    assert_eq!(
        js.matches("'cui:' + (ns ? ns + ':' : '') + String(key)")
            .count(),
        2,
        "get and set both prefix the key"
    );
    assert!(
        js.contains("__cui_storage_set(\"note\", __state_draft, __cui_storage_ns)"),
        "{js}"
    );
    assert!(js.contains("__cui_storage_get(\"note\", \"nothing yet\", __cui_storage_ns)"));
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    let other = build_js(&counter, "counter.cui");
    assert!(
        !other.contains("__cui_storage"),
        "a program without Storage carries no adapter"
    );
    assert!(!other.contains("localStorage"));
}

#[test]
fn the_adapter_never_touches_a_key_outside_its_prefix_and_never_throws() {
    let js = build_js(NOTES, "notes-adapter.cui");
    // Every localStorage call is inside a try, and takes the prefixed key.
    for call in ["getItem(k)", "setItem(k, String(value))"] {
        assert!(js.contains(call), "the adapter calls {call}");
    }
    assert!(!js.contains("getItem(key)") && !js.contains("setItem(key"));
    assert_eq!(
        js.matches("window.localStorage.").count(),
        2,
        "only get and set reach localStorage"
    );
    for call in [
        "window.localStorage.getItem(k)",
        "window.localStorage.setItem(k, String(value))",
    ] {
        let at = js.find(call).expect("the call");
        assert!(
            js[..at]
                .rsplit("try {")
                .next()
                .is_some_and(|after| after.len() < 80),
            "{call} is inside a try"
        );
    }
}

#[test]
fn storage_without_a_declaration_is_capability_missing_and_a_declaration_without_use_is_unused() {
    let missing =
        "component A {\n    on Go { storage.set(\"a\", \"b\"); }\n    view { Text { \"x\" } }\n}\n";
    assert_eq!(check_codes(missing, "missing.cui"), ["capability-missing"]);
    let unused =
        "component A {\n    requires Storage;\n    on Go { }\n    view { Text { \"x\" } }\n}\n";
    assert_eq!(check_codes(unused, "unused.cui"), ["capability-unused"]);
}

#[test]
fn a_storage_call_with_the_wrong_number_of_arguments_is_builtin_shape_mismatch() {
    let set = "component A {\n    requires Storage;\n    on Go { storage.set(\"a\"); }\n    view { Text { storage.get(\"a\", \"d\") } }\n}\n";
    assert_eq!(check_codes(set, "set1.cui"), ["builtin-shape-mismatch"]);
    let get = "component A {\n    requires Storage;\n    on Go { storage.set(\"a\", \"b\"); }\n    view { Text { storage.get(\"a\") } }\n}\n";
    assert_eq!(check_codes(get, "get1.cui"), ["builtin-shape-mismatch"]);
}

#[test]
fn the_example_traces_replay_and_pass() {
    for trace in ["notes.trace.json", "seeded.trace.json"] {
        let (code, stdout, stderr) = run(&[
            "test",
            "examples/notes/notes.cui",
            &format!("examples/notes/{trace}"),
        ]);
        assert_eq!(code, 0, "{trace}: {stdout}{stderr}");
        assert!(stdout.contains("PASS"), "{trace}: {stdout}");
    }
}

#[test]
fn a_trace_that_expects_the_wrong_store_fails_with_the_store_in_the_report() {
    let trace = scratch(
        "wrong.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Notes","steps":[
            {"event":"Change","params":["a"],"state":{"draft":"a"}},
            {"event":"Save","expect":{"storage":{"note":"not a"}}}]}"#,
    );
    let (code, stdout, _) = run(&[
        "test",
        "examples/notes/notes.cui",
        p(&trace),
        "--output",
        "json",
    ]);
    assert_eq!(code, 4, "{stdout}");
    let report: Value = serde_json::from_str(&stdout).expect("a JSON report");
    assert_eq!(report["failures"][0]["rule"], "test-expect-mismatch");
    assert_eq!(report["failures"][0]["actual"]["note"], "a");
    assert_eq!(report["steps"][1]["storage"]["note"], "a");
}

#[test]
fn the_store_written_before_an_await_is_still_there_when_the_request_resolves() {
    let src = "component A {\n    requires Storage;\n    requires Network;\n    state n: Int = 0;\n    state r: Result[List[String], String] = Ok([]);\n    on Go {\n        storage.set(\"k\", \"v\");\n        r = await(net.search(\"q\"));\n        n = 1;\n    }\n    view { Text { storage.get(\"k\", \"d\") } }\n}\n";
    let file = scratch("resume.cui", src);
    let trace = scratch(
        "resume.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Go","expect":{"outcome":"suspended","storage":{"k":"v"}}},
            {"resolve":1,"with":{"Ok":["x"]},"state":{"n":1}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_store_that_is_not_an_object_of_strings_is_trace_malformed() {
    for storage in [r#""x""#, r#"{"k": 1}"#, "[]"] {
        let trace = scratch(
            "badstore.trace.json",
            &format!(
                r#"{{"schema":"cancho-ui-trace","version":"0.1.0","storage":{storage},"steps":[{{"event":"Restore","state":{{"draft":""}}}}]}}"#
            ),
        );
        let (code, stdout, stderr) = run(&["test", "examples/notes/notes.cui", p(&trace)]);
        assert_eq!(code, 4, "{storage}: {stdout}{stderr}");
        assert!(
            stdout.contains("trace-malformed") || stderr.contains("trace-malformed"),
            "{storage}: {stdout}{stderr}"
        );
    }
}

#[test]
fn the_semantic_diff_reports_storage_as_a_new_capability() {
    let ir_of = |src: &str| {
        let ast = parse(lex(src).expect("lex")).expect("parse");
        check_file(&ast).expect("check");
        lower_file(&ast).expect("lower")
    };
    let without = NOTES
        .replace("    requires Storage;\n", "")
        .replace("storage.set(\"note\", draft);", "draft = \"x\";")
        .replace("draft = storage.get(\"note\", \"\");", "draft = \"y\";")
        .replace(
            "Text { storage.get(\"note\", \"nothing yet\") }",
            "Text { \"none\" }",
        );
    let report = serde_json::to_value(diff_ir(&ir_of(&without), &ir_of(NOTES), "base", "cand"))
        .expect("a report");
    let added = report["capabilities"]["added"].as_array().expect("a list");
    assert!(
        added.iter().any(|c| c["capability"] == "Storage"),
        "{report}"
    );
}

/// The notes example with its declaration replaced.
fn notes_with(requires: &str) -> String {
    let src = NOTES.replace("requires Storage;", requires);
    assert_ne!(src, NOTES, "the declaration was replaced");
    src
}

#[test]
fn a_namespace_prefixes_every_key_and_each_component_keeps_its_own() {
    let bare = build_js(NOTES, "bare.cui");
    assert!(bare.contains("const __cui_storage_ns = \"\";"), "{bare}");
    let ns = build_js(&notes_with("requires Storage(\"notes\");"), "ns.cui");
    assert!(ns.contains("const __cui_storage_ns = \"notes\";"), "{ns}");
    assert!(
        ns.contains("__cui_storage_get(\"note\", \"nothing yet\", __cui_storage_ns)"),
        "{ns}"
    );
    assert!(
        ns.contains("'cui:' + (ns ? ns + ':' : '') + String(key)"),
        "{ns}"
    );
    let two = format!(
        "{}\n{}",
        notes_with("requires Storage(\"a\");"),
        notes_with("requires Storage(\"b\");").replace("component Notes", "component Other")
    );
    let js = build_js(&two, "two.cui");
    assert_eq!(
        js.matches("const __cui_storage_ns = \"a\";").count(),
        1,
        "{js}"
    );
    assert_eq!(
        js.matches("const __cui_storage_ns = \"b\";").count(),
        1,
        "{js}"
    );
}

#[test]
fn a_namespace_that_could_spell_another_key_is_narrowing_illegal() {
    for requires in [
        "requires Storage(\"\");",
        "requires Storage(\"a:b\");",
        "requires Storage(\"Notes\");",
        "requires Storage(\"-a\");",
        "requires Storage(\"a b\");",
        "requires Storage(\"a\", \"b\");",
        "requires Storage(\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\");",
    ] {
        assert_eq!(
            check_codes(&notes_with(requires), "ns-bad.cui"),
            ["capability-narrowing-illegal"],
            "{requires}"
        );
    }
}

#[test]
fn a_namespaced_trace_reads_and_writes_namespaced_keys() {
    let src = scratch("ns-replay.cui", &notes_with("requires Storage(\"notes\");"));
    let trace = scratch(
        "ns-replay.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Notes","steps":[
            {"event":"Change","params":["buy milk"],"state":{"draft":"buy milk"}},
            {"event":"Save","expect":{"storage":{"notes:note":"buy milk"}}},
            {"event":"Change","params":["x"],"state":{"draft":"x"}},
            {"event":"Restore","state":{"draft":"buy milk"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&src), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    // The same trace seeded under the bare key finds nothing: the namespace is part of the key.
    let seeded = scratch(
        "ns-seeded.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Notes","storage":{"note":"not mine"},"steps":[
            {"event":"Restore","state":{"draft":""}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&src), p(&seeded)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

const QUOTA_CORPUS: &str = include_str!("../../../tests/storage_quota_corpus.json");

/// A full `localStorage` (docs/adapters.md section 20) through the real replay: one trace per case of
/// `tests/storage_quota_corpus.json`, the file `scripts/check-storage-quota.js` runs against the emitted adapter. The
/// report's store after the last step is the persisted store; a refused write is not in it.
#[test]
fn a_full_store_refuses_writes_as_the_corpus_says() {
    let corpus: Value = serde_json::from_str(QUOTA_CORPUS).expect("the corpus is JSON");
    let cases = corpus["cases"].as_array().expect("cases");
    assert!(cases.len() >= 10, "{} cases", cases.len());
    let src = "component Keep {\n    requires Storage;\n    state ok: Bool = true;\n    state got: String = \"\";\n    on Set(k, v) { ok = storage.set(k, v); }\n    on Get(k) { got = storage.get(k, \"none\"); }\n    view { Text { \"{got}\" } }\n}\n";
    let file = scratch("keep.cui", src);
    for c in cases {
        let mut steps = Vec::new();
        for op in c["ops"].as_array().expect("ops") {
            if let Some(set) = op.get("set") {
                steps.push(serde_json::json!({
                    "event": "Set", "params": [set[0], set[1]], "state": {"ok": op["ok"]}
                }));
            } else {
                let get = &op["get"];
                steps.push(serde_json::json!({
                    "event": "Get", "params": [get[0]], "state": {"got": op["is"]}
                }));
            }
        }
        let mut trace = serde_json::json!({
            "schema": "cancho-ui-trace", "version": "0.1.0", "component": "Keep",
            "storage": c["seed"], "steps": steps
        });
        if !c["quota"].is_null() {
            trace["storage_quota"] = c["quota"].clone();
        }
        let trace_file = scratch("keep.trace.json", &trace.to_string());
        let (code, stdout, stderr) = run(&["test", p(&file), p(&trace_file), "--output", "json"]);
        assert_eq!(code, 0, "{}: {stdout}{stderr}", c["why"]);
        let report: Value = serde_json::from_str(&stdout).expect("a JSON report");
        let last = report["steps"]
            .as_array()
            .and_then(|s| s.last())
            .expect("a step");
        let stored = last
            .get("storage")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        assert_eq!(stored, c["stored"], "{}: the persisted store", c["why"]);
    }
}

#[test]
fn a_bad_storage_quota_is_a_trace_error() {
    for bad in ["-1", "1.5", "\"big\"", "null"] {
        let trace = scratch(
            "badquota.trace.json",
            &format!(
                r#"{{"schema":"cancho-ui-trace","version":"0.1.0","component":"Notes","storage_quota":{bad},"steps":[{{"event":"Save"}}]}}"#
            ),
        );
        let (code, stdout, _) = run(&[
            "test",
            "examples/notes/notes.cui",
            p(&trace),
            "--output",
            "json",
        ]);
        assert_eq!(code, 4, "{bad}: {stdout}");
        assert!(stdout.contains("trace-bad-value"), "{bad}: {stdout}");
    }
}
