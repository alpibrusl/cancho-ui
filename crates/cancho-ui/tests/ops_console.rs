//! `examples/ops-console` (`docs/ops-console.md`): the integrated page. The browser side is
//! `scripts/check-ops-console.js`; here, what the gate can see without one: the program checks, its trace passes and a
//! wrong expectation fails, it asks for exactly the two network capabilities, and it builds.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const SRC: &str = include_str!("../../../examples/ops-console/ops-console.cui");
const TRACE: &str = include_str!("../../../examples/ops-console/ops-console.trace.json");

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-ops-console-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

#[test]
fn the_trace_passes() {
    let src = scratch("ok.cui", SRC);
    let trace = scratch("ok.trace.json", TRACE);
    let (code, stdout, stderr) = run(&["test", p(&src), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("PASS"), "{stdout}");
}

#[test]
fn a_wrong_expectation_fails_it() {
    let mut trace: Value = serde_json::from_str(TRACE).expect("the trace is JSON");
    let steps = trace["steps"].as_array_mut().expect("steps");
    // the paging step: the second Next stays on page 1; claim it goes to page 2
    let next_twice = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s["event"] == "Next")
        .map(|(i, _)| i)
        .nth(1)
        .expect("a second Next");
    steps[next_twice]["state"]["page"] = Value::from(2);
    let src = scratch("bad.cui", SRC);
    let bad = scratch("bad.trace.json", &trace.to_string());
    let (code, stdout, _) = run(&["test", p(&src), p(&bad)]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("test-state-mismatch"), "{stdout}");
}

#[test]
fn it_needs_a_read_and_a_write_capability_and_neither_is_spare() {
    for (cut, what) in [
        ("    requires Network;\n", "Network"),
        ("    requires NetworkWrite;\n", "NetworkWrite"),
    ] {
        let src = scratch("cut.cui", &SRC.replace(cut, ""));
        let (code, stdout, _) = run(&["check", p(&src), "--output", "json"]);
        assert_eq!(code, 1, "{what}: {stdout}");
        assert!(stdout.contains("capability-missing"), "{what}: {stdout}");
    }
}

#[test]
fn it_builds_with_both_adapters_and_the_helpers_it_calls() {
    let src = scratch("build.cui", SRC);
    let out = scratch("build.js", "");
    let (code, _, stderr) = run(&["build", p(&src), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    for f in [
        "function __cui_net_get(",
        "function __cui_net_post(",
        "function __cui_sort(",
        "function __cui_take(",
        "function __cui_skip(",
        "function __cui_lines(",
        "function __cui_num(",
        "function __cui_contains(",
        "function __cui_json_get(",
    ] {
        assert!(js.contains(f), "{f}");
    }
    assert!(
        !js.contains("unsupported call"),
        "no call without an emission"
    );
}
