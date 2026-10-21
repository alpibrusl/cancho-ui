//! The bindings of the built-in elements (`docs/controls.md`): `disabled:`, and the refusal of a key that does
//! nothing. What is emitted, what is refused, and how a trace sees a disabled control. The browser side is
//! `scripts/check-controls.js`.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-controls-{}", std::process::id()));
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

const WRITER: &str = "component Writer {\n    requires NetworkWrite;\n    state sending: Bool = false;\n    state sent: Int = 0;\n    state r: Result[String, String] = Ok(\"\");\n    on Save {\n        sending = true;\n        r = await(net.post(\"/api/x\", \"[1]\"));\n        sending = false;\n        sent = sent + 1;\n    }\n    view {\n        Column {\n            Button { label: \"Save\" on: Save disabled: sending }\n            Text { \"{sent}\" }\n        }\n    }\n}\n";

fn built(src: &str, name: &str) -> String {
    let file = scratch(&format!("{name}.cui"), src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

#[test]
fn disabled_is_emitted_from_a_bool_state_field_and_from_a_literal() {
    let js = built(WRITER, "state");
    assert!(js.contains(".disabled = !!__state_sending;"), "{js}");
    let lit = WRITER.replace("disabled: sending", "disabled: true");
    let js = built(&lit, "lit");
    assert!(js.contains(".disabled = true;"), "{js}");
    let field = WRITER.replace(
        "Button { label: \"Save\" on: Save disabled: sending }",
        "TextField { label: \"Note\" value: note on: Save disabled: sending }",
    );
    let field = field.replace(
        "state sent: Int = 0;",
        "state sent: Int = 0;\n    state note: String = \"\";",
    );
    let js = built(&field, "field");
    assert!(js.contains(".disabled = !!__state_sending;"), "{js}");
}

#[test]
fn disabled_needs_a_bool() {
    for (what, from, to) in [
        ("a number", "disabled: sending", "disabled: 5"),
        ("a string", "disabled: sending", "disabled: \"yes\""),
        (
            "a field that is not Bool",
            "disabled: sending",
            "disabled: sent",
        ),
        (
            "a name that is no field",
            "disabled: sending",
            "disabled: nothere",
        ),
    ] {
        let src = WRITER.replace(from, to);
        assert_eq!(codes(&src, "notbool.cui"), ["disabled-not-bool"], "{what}");
    }
}

#[test]
fn a_key_that_does_nothing_is_refused() {
    let unknown = WRITER.replace("disabled: sending", "title: \"x\"");
    assert_eq!(codes(&unknown, "title.cui"), ["binding-unknown"]);
    let on_column = WRITER.replace("Column {", "Column { disabled: true");
    assert_eq!(codes(&on_column, "oncolumn.cui"), ["binding-unknown"]);
    // the keys that do something stay legal, `placeholder` among them
    let ok = WRITER.replace(
        "Button { label: \"Save\" on: Save disabled: sending }",
        "Button { label: \"Save\" on: Save disabled: sending }\n            TextField { label: \"Note\" placeholder: \"a note\" value: note on: Edit }",
    );
    let ok = ok.replace(
        "state sent: Int = 0;",
        "state sent: Int = 0;\n    state note: String = \"\";",
    );
    let ok = ok.replace("    view {", "    on Edit(v) { note = v; }\n    view {");
    let file = scratch("ok.cui", &ok);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_trace_refuses_an_event_whose_every_control_is_disabled() {
    let file = scratch("writer.cui", WRITER);
    let trace = scratch(
        "writer.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Writer","steps":[
            {"event":"Save","expect":{"outcome":"suspended","pending":[1]},"state":{"sending":true,"sent":0}},
            {"event":"Save","expect":{"outcome":"disabled","pending":[1]},"state":{"sending":true,"sent":0}},
            {"event":"Save","expect":{"outcome":"disabled","pending":[1]}},
            {"resolve":1,"with":{"Ok":"{\"status\":201,\"body\":\"\"}"},"state":{"sending":false,"sent":1}},
            {"event":"Save","expect":{"outcome":"suspended","pending":[2]},"state":{"sending":true}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("disabled"), "{stdout}");
}

#[test]
fn an_event_no_control_fires_is_never_disabled() {
    // `Tick` is fired by a schedule, not by a control: `disabled` has no say.
    let src = "component T {\n    state n: Int = 0;\n    state off: Bool = true;\n    every 1000 Tick;\n    on Tick { n = n + 1; }\n    on Go { n = n + 10; }\n    view { Column { Button { label: \"Go\" on: Go disabled: off } Text { \"{n}\" } } }\n}\n";
    let file = scratch("tick.cui", src);
    let trace = scratch(
        "tick.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"T","steps":[
            {"event":"Tick","state":{"n":1}},
            {"event":"Go","expect":{"outcome":"disabled"},"state":{"n":1}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}
