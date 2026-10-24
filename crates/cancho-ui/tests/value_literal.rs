//! A string literal `value:` on a text field (issue #140). It was emitted as it was written, `value: ""` as `.value = ;`, a syntax error that
//! stopped the whole module from loading. The corpus (`tests/value_literal_corpus.json`) is read back here from the IR, which is what replay
//! reads, and from the built module; `scripts/check-value-literal.js` loads the page and reads the field.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-vlit-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(source: &str) -> String {
    format!(
        "component F {{\n    state v: String = \"\";\n    on V(x) {{ v = x; }}\n    view {{ Column {{ TextField {{ label: \"F\" value: \"{source}\" on: V }} TextArea {{ label: \"T\" value: \"{source}\" on: V }} Text {{ \"{{v}}\" }} }} }}\n}}\n"
    )
}

/// Every `value` binding of an IR document, in order.
fn values(ir: &Value, out: &mut Vec<String>) {
    match ir {
        Value::Object(m) => {
            if m.get("key").and_then(Value::as_str) == Some("value")
                && let Some(Value::String(s)) = m.get("value")
            {
                out.push(s.clone());
            }
            for v in m.values() {
                values(v, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|v| values(v, out)),
        _ => {}
    }
}

#[test]
fn a_literal_value_reaches_the_ir_and_the_module() {
    let c: Value = serde_json::from_str(include_str!("../../../tests/value_literal_corpus.json"))
        .expect("JSON");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 15, "{}", cases.len());
    for case in cases {
        let (source, want, why) = (
            case["source"].as_str().expect("source"),
            case["value"].as_str().expect("value"),
            case["why"].as_str().expect("why"),
        );
        let file = scratch("f.cui", &program(source));
        let (code, stdout, stderr) = run(&["inspect", "ir", p(&file), "--output", "json"]);
        assert_eq!(code, 0, "{why}: {stdout}{stderr}");
        let ir: Value = serde_json::from_str(&stdout).expect("IR");
        let mut got = Vec::new();
        values(&ir, &mut got);
        assert_eq!(
            got,
            [want, want],
            "{why}: the IR holds the text the source says"
        );
        // the module: the value is written as a quoted text, never as a bare word or nothing
        let out = file.with_extension("js");
        let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 0, "{why}: {stdout}{stderr}");
        let js = std::fs::read_to_string(&out).expect("module");
        assert!(!js.contains(".value = ;"), "{why}");
        assert_eq!(js.matches(".value = \"").count(), 2, "{why}: {js}");
    }
}

#[test]
fn replay_runs_a_program_whose_fields_have_literal_values() {
    // replay does not draw a field; what it holds of one is the IR above. The program still runs: an event given a text sets the state.
    let file = scratch("f.cui", &program("say \\\"hi\\\""));
    let trace = scratch(
        "t.trace.json",
        r#"{"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "V", "params": ["typed"], "state": {"v": "typed"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_literal_value_with_an_unescaped_brace_is_still_refused() {
    // static text is not interpolated (a brace in it would be shown as a brace and read as a mistake): the literal rule is unchanged
    let file = scratch("f.cui", &program("a {b}"));
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
}
