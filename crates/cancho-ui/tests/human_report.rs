//! The human report of `cancho-ui test` bounds a value on a step line and `--full` lifts the bound (issue #45).
//! The JSON report and the expected/actual of a failure are never bounded (`docs/replay.md` section 7).
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::HUMAN_VALUE_BOUND;

fn run(args: &[&str]) -> (i32, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = common::scratch_root().join(format!("cancho-ui-human-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component whose handler writes `value` (a string literal body) to `s`; the trace expects `expect`.
fn replay(name: &str, value: &str, expect: &str, extra: &[&str]) -> (i32, String) {
    let src = format!(
        "component C {{\n    state s: String = \"\";\n    on Go {{ s = \"{value}\"; }}\n    view {{ Text {{ \"{{s}}\" }} }}\n}}\n"
    );
    let file = scratch(&format!("{name}.cui"), &src);
    let trace = scratch(
        &format!("{name}.trace.json"),
        &format!(
            r#"{{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{{"event":"Go","state":{{"s":"{expect}"}}}}]}}"#
        ),
    );
    let mut args = vec!["test", p(&file), p(&trace)];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn a_short_value_is_printed_whole() {
    let (code, out) = replay("short", "abc", "abc", &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(r#"s="abc""#), "{out}");
    assert!(!out.contains("chars)"), "{out}");
}

#[test]
fn a_long_value_is_cut_and_its_length_said() {
    let long = "x".repeat(300);
    let (code, out) = replay("long", &long, &long, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(
        !out.contains(&long),
        "the whole value must not be printed: {out}"
    );
    // The value is 300 x between quotes: 302 characters of JSON text, of which the bound is kept
    // (the opening quote and the first bound - 1 x).
    let head = format!("s=\"{}", "x".repeat(HUMAN_VALUE_BOUND - 1));
    assert!(out.contains(&format!("{head}\u{2026}(302 chars)")), "{out}");
}

#[test]
fn full_prints_the_whole_value() {
    let long = "x".repeat(300);
    let (code, out) = replay("full", &long, &long, &["--full"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&format!("s=\"{long}\"")), "{out}");
    assert!(!out.contains("chars)"), "{out}");
}

#[test]
fn a_multibyte_value_is_cut_on_a_character_boundary() {
    let long = "\u{e9}".repeat(300);
    let (code, out) = replay("multibyte", &long, &long, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("(302 chars)"), "{out}");
}

#[test]
fn a_failure_prints_expected_and_actual_in_full() {
    let long = "x".repeat(300);
    let (code, out) = replay("failure", &long, "different", &[]);
    assert_eq!(code, 4, "{out}");
    assert!(out.contains(&format!("actual   \"{long}\"")), "{out}");
    assert!(out.contains("expected \"different\""), "{out}");
}

#[test]
fn the_json_report_is_never_cut() {
    let long = "x".repeat(300);
    let (code, out) = replay("json", &long, &long, &["--output", "json"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&long), "{out}");
    assert!(!out.contains("chars)"), "{out}");
}

#[test]
fn introspect_lists_the_flag() {
    let (code, out) = run(&["introspect", "--output", "json"]);
    assert_eq!(code, 0);
    assert!(out.contains("--full"), "introspect lists the flag");
}
