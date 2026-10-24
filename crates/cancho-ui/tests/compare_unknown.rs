//! Comparing a value whose type the compiler cannot see (docs/comparisons.md section 6, issue #103): an event parameter, a lambda binding, a
//! `match` binding. Replay faults (`replay-type-error`) where the page used to coerce; the page is strict now, and
//! `scripts/check-compare-unknown.js` runs this same corpus (`tests/compare_unknown_corpus.json`) in Chromium.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-cmpu-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn corpus() -> Value {
    serde_json::from_str(include_str!("../../../tests/compare_unknown_corpus.json")).expect("JSON")
}

#[test]
fn replay_answers_the_corpus() {
    let c = corpus();
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 120, "{}", cases.len());
    let sources: std::collections::BTreeMap<String, PathBuf> = c["programs"]
        .as_object()
        .expect("programs")
        .iter()
        .map(|(k, v)| (k.clone(), scratch("p.cui", v.as_str().expect("source"))))
        .collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = cases
            .chunks(16)
            .map(|chunk| {
                let sources = &sources;
                s.spawn(move || {
                    for case in chunk {
                        let src = &sources[case["ctx"].as_str().expect("ctx")];
                        let want = case["want"].as_str().expect("want");
                        let mut step = json!({"event": case["event"], "params": case["params"]});
                        if want != "fault" {
                            step["state"] = json!({"out": want});
                        } else {
                            step["state"] = json!({"out": ""});
                        }
                        let trace = scratch(
                            "t.trace.json",
                            &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [step]})
                                .to_string(),
                        );
                        let (code, stdout, stderr) =
                            run(&["test", p(src), p(&trace), "--output", "json"]);
                        let report: Value = serde_json::from_str(&stdout)
                            .unwrap_or_else(|e| panic!("report ({e}): {stdout}{stderr}"));
                        if want == "fault" {
                            let rules: Vec<&str> = report["failures"]
                                .as_array()
                                .expect("failures")
                                .iter()
                                .filter_map(|f| f["rule"].as_str())
                                .collect();
                            assert_eq!(code, 4, "{case}: {report:#}");
                            assert_eq!(rules, ["replay-type-error"], "{case}");
                        } else {
                            assert_eq!(code, 0, "{case}: {report:#}");
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("a case panicked");
        }
    });
}

#[test]
fn the_probe_of_the_issue_faults_in_replay() {
    // a TextField given "7": `x <= 7` is a text against a number, `x == 7` is simply false
    let src = "component F {\n    state out: String = \"\";\n    on Param(x) { out = \"\" + (x <= 7) + \" \" + (x == 7); }\n    view { Column { TextField { label: \"v\" value: out on: Param } } }\n}\n";
    let file = scratch("f.cui", src);
    let trace = scratch(
        "t.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "Param", "params": ["7"], "state": {"out": ""}}]}).to_string(),
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
    // the number 7 is fine
    let trace = scratch(
        "t.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "Param", "params": [7], "state": {"out": "true true"}}]}).to_string(),
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
}

#[test]
fn the_page_is_strict_only_where_the_compiler_cannot_see_the_type() {
    let build = |src: &str| {
        let file = scratch("f.cui", src);
        let out = file.with_extension("js");
        let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 0, "{stdout}{stderr}");
        std::fs::read_to_string(out).expect("js")
    };
    // known operands: the host's own operator, and no helper in the module
    let known = build(
        "component K {\n    state a: Int = 1;\n    state s: String = \"x\";\n    state out: String = \"\";\n    on Go { out = \"\" + (a <= 7) + (s < \"y\") + (a == 1) + (s != \"z\"); }\n    view { Column { Text { \"{out}\" } } }\n}\n",
    );
    for helper in ["__cui_lt(", "__cui_le(", "__cui_eq(", "__cui_ord("] {
        assert!(!known.contains(helper), "known operands use {helper}");
    }
    assert!(known.contains("(__state_a <= 7)"), "{known}");
    // an operand the compiler cannot see: the strict helper, and only the helper that is used
    let unknown = build(
        "component U {\n    state out: String = \"\";\n    on Param(x) { out = \"\" + (x <= 7); }\n    view { Column { Text { \"{out}\" } } }\n}\n",
    );
    assert!(unknown.contains("function __cui_le("), "{unknown}");
    assert!(unknown.contains("replay-type-error"), "{unknown}");
    assert!(
        !unknown.contains("function __cui_eq("),
        "the equality helper is not emitted when no == is on an unknown operand"
    );
}
