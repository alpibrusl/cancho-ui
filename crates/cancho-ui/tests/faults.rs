//! A handler that faults (docs/semantics.md, "A handler that faults"; issue #59): it is atomic between its commit points,
//! so a fault undoes the writes made since the last one. Every case of `tests/fault_corpus.json` replayed in-process (the
//! same file `scripts/check-faults.js` runs in the emitted page, in Chromium), the trace key `fault` and its two rules, and
//! what is emitted.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/fault_corpus.json");

fn replayed(src: &str, steps: &Value) -> (String, Vec<&'static str>) {
    let ast = parse(lex(src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    // `shows` is for the page (the text a step leaves on screen); a trace has no such key.
    let mut steps = steps.clone();
    for s in steps.as_array_mut().expect("steps") {
        s.as_object_mut().expect("a step").remove("shows");
    }
    let trace =
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string();
    let r = replay(&ir, &trace, "prog.cui", "t.json");
    (
        r.result.to_string(),
        r.failures.iter().map(|f| f.rule).collect(),
    )
}

#[test]
fn replay_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let programs = corpus["programs"].as_object().expect("programs");
    let cases = corpus["cases"].as_array().expect("cases");
    assert!(cases.len() >= 12, "the corpus shrank");
    let mut faulting = 0;
    for c in cases {
        let src = programs[c["program"].as_str().expect("a program")]
            .as_str()
            .expect("source");
        if c["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .any(|s| s.get("fault").is_some())
        {
            faulting += 1;
        }
        let (result, failures) = replayed(src, &c["steps"]);
        assert!(
            result == "pass" && failures.is_empty(),
            "[{}] {result} {failures:?}",
            c["why"]
        );
    }
    assert!(faulting >= 7, "the corpus must fault in every position");
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
    let dir = std::env::temp_dir().join(format!("cancho-ui-faults-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

const TWO: &str = "component C {\n    state a: Int = 0;\n    state b: Int = 0;\n    state m: Int = 0;\n    on Go { a = 1; b = 100 / m; }\n    view { Text { \"{a} {b}\" } }\n}\n";

/// A file of its own for each trace: the tests of a binary run in parallel.
fn trace_of(step: Value) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    scratch(
        &format!("t{n}.trace.json"),
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [step]}).to_string(),
    )
}

#[test]
fn a_step_that_faults_without_saying_so_is_still_a_failure_that_stops_the_replay() {
    let file = scratch("two.cui", TWO);
    let trace = trace_of(json!({"event": "Go", "state": {"a": 0}}));
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-non-finite"), "{stdout}");
}

#[test]
fn a_step_that_was_expected_to_fault_and_did_not_is_test_fault_missing() {
    let file = scratch("two-ok.cui", &TWO.replace("100 / m", "100 / 4"));
    let trace = trace_of(json!({"event": "Go", "fault": "replay-non-finite"}));
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("test-fault-missing"), "{stdout}");
}

#[test]
fn a_step_that_faulted_with_another_rule_is_test_fault_mismatch() {
    let file = scratch("two-m.cui", TWO);
    let trace = trace_of(json!({"event": "Go", "fault": "replay-type-error"}));
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("test-fault-mismatch"), "{stdout}");
    assert!(stdout.contains("replay-non-finite"), "{stdout}");
}

#[test]
fn the_fault_key_is_a_replay_rule_name() {
    let file = scratch("two-k.cui", TWO);
    for bad in [json!("if-else-required"), json!(5), json!(true)] {
        let trace = trace_of(json!({"event": "Go", "fault": bad}));
        let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
        assert_eq!(code, 4, "{stdout}");
        assert!(stdout.contains("trace-bad-step"), "{stdout}");
    }
}

#[test]
fn the_report_of_a_faulted_step_lists_no_write_of_the_undone_segment() {
    let file = scratch("two-r.cui", TWO);
    let trace = trace_of(json!({"event": "Go", "fault": "replay-non-finite", "state": {"a": 0}}));
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
    let r: Value = serde_json::from_str(&stdout).expect("a report");
    assert_eq!(r["steps"][0]["outcome"], "faulted", "{stdout}");
    assert_eq!(r["steps"][0]["writes"], json!([]), "{stdout}");
    assert_eq!(r["final_state"]["a"], 0, "{stdout}");
}

fn built(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

#[test]
fn a_handler_with_a_write_before_a_statement_snapshots_and_restores_what_it_writes() {
    let js = built(TWO, "emit.cui");
    assert!(js.contains("let __snap = [__state_a, __state_b];"), "{js}");
    assert!(
        js.contains("if (__snap) { [__state_a, __state_b] = __snap; }"),
        "{js}"
    );
}

#[test]
fn a_handler_of_one_statement_carries_no_snapshot() {
    let js = built(
        "component C {\n    state a: Int = 0;\n    on Go { a = a + 1; }\n    view { Text { \"{a}\" } }\n}\n",
        "one.cui",
    );
    assert!(!js.contains("__snap"), "{js}");
}

#[test]
fn an_awaiting_statement_clears_the_snapshot_and_takes_a_new_one_after_it() {
    let js = built(
        "component C {\n    state a: Int = 0;\n    state r: Result[Int, String] = Ok(0);\n    on Go { a = 1; r = await(sleep(5)); a = a + 1; }\n    view { Text { \"{a}\" } }\n}\n",
        "await.cui",
    );
    let at = |needle: &str| {
        js.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {js}"))
    };
    let clear = at("__snap = null;");
    let wait = at("await __cui_call");
    let retake = js
        .rfind("__snap = [__state_a, __state_r];")
        .expect("a snapshot after the await");
    assert!(clear < wait && wait < retake, "{js}");
}
