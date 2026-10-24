//! String interpolation outside a `Text` body (`docs/semantics.md` Q9, `docs/adapters.md` 9.4): a `{expr}` in a
//! string expression (a handler, a `match` arm) is interpolated like one in a `Text`, and a `{` in static text (a
//! label, an attribute, a property) is refused instead of being shown as a brace.
mod common;

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
    let dir = common::scratch_root().join(format!("cancho-ui-interp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component with two Int fields, a String field `s`, and a handler `Go` whose body is `stmt`.
fn program(stmt: &str) -> String {
    format!(
        "component C {{\n    state a: Int = 1;\n    state b: Int = 2;\n    state s: String = \"\";\n    state ok: Bool = true;\n    on Go {{ {stmt} }}\n    view {{ Text {{ \"x\" }} }}\n}}\n"
    )
}

/// Replay `Go` once on `stmt` and answer the final `s`.
fn replayed(stmt: &str, name: &str) -> String {
    let file = scratch(&format!("{name}.cui"), &program(stmt));
    let trace = scratch(
        &format!("{name}.trace.json"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[{"event":"Go","expect":{"outcome":"completed"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let report: Value = serde_json::from_str(&stdout).expect("a JSON report");
    report["final_state"]["s"]
        .as_str()
        .expect("s is a string")
        .to_string()
}

fn js_of(stmt: &str, name: &str) -> String {
    let file = scratch(&format!("{name}.cui"), &program(stmt));
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

#[test]
fn an_interpolation_in_a_handler_string_is_the_concatenation_a_text_would_show() {
    assert_eq!(replayed("s = \"a={a} b={b}\";", "pair"), "a=1 b=2");
    assert_eq!(
        replayed("s = \"{a}{b}\";", "adjacent"),
        "12",
        "two numbers concatenate, they do not add"
    );
    assert_eq!(replayed("s = \"{a + b}\";", "leading"), "3");
    assert_eq!(replayed("s = \"{(a + b) * 2}!\";", "grouped"), "6!");
    assert_eq!(replayed("s = \"ok={ok}\";", "boolean"), "ok=true");
}

#[test]
fn the_emitted_code_concatenates_from_a_string_and_a_plain_string_is_unchanged() {
    let js = js_of("s = \"{a}{b}\";", "emit");
    assert!(
        js.contains("__cui_add(__cui_add(\"\", __state_a), __state_b)"),
        "{js}"
    );
    let plain = js_of("s = \"no braces\";", "plain");
    assert!(plain.contains("__state_s = \"no braces\""), "{plain}");
}

#[test]
fn an_interpolation_in_a_match_arm_sees_the_arms_binding() {
    let src = "component M {\n    requires Network;\n    state r: Result[String, String] = Ok(\"\");\n    state s: String = \"\";\n    on Go {\n        r = await(net.get(\"u\"));\n        s = match r {\n            Ok(t) => { \"got {t}\" }\n            Err(why) => { \"error: {why}\" }\n            Cancelled => { \"cancelled\" }\n        };\n    }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("arm.cui", src);
    let trace = scratch(
        "arm.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Go"},
            {"resolve":1,"with":{"Err":"HTTP 404"},"state":{"s":"error: HTTP 404"}},
            {"event":"Go"},
            {"resolve":2,"with":{"Ok":"hello"},"state":{"s":"got hello"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn an_unclosed_brace_in_a_string_expression_is_refused() {
    let file = scratch("unclosed.cui", &program("s = \"a {b\";"));
    let (code, _, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("parse-unexpected-token"), "{stderr}");
}

#[test]
fn a_brace_in_static_text_is_refused_wherever_static_text_is_written() {
    let cases = [
        ("a button label", "Button { label: \"Item {a}\" on: Go }"),
        (
            "an accessibility label",
            "Text { role: status label: \"n is {a}\" \"x\" }",
        ),
        (
            "a text field value",
            "TextField { label: \"q\" value: \"{a}\" on: Go }",
        ),
    ];
    for (what, element) in cases {
        let src = format!(
            "component C {{\n    state a: Int = 1;\n    on Go {{ a = a + 1; }}\n    view {{ {element} }}\n}}\n"
        );
        let file = scratch("static.cui", &src);
        let (code, _, stderr) = run(&["check", p(&file)]);
        assert_eq!(code, 1, "{what}: {stderr}");
        assert!(
            stderr.contains("static text without `{`"),
            "{what}: {stderr}"
        );
    }
    let prop = "component Child {\n    state title: String = \"\";\n    view { Text { \"{title}\" } }\n}\ncomponent Parent {\n    state a: Int = 1;\n    view { Child { title: \"x {a}\" } }\n}\n";
    let file = scratch("prop.cui", prop);
    let (code, _, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("static text without `{`"), "{stderr}");
}

#[test]
fn static_text_without_a_brace_and_text_bodies_are_unchanged() {
    let src = "component C {\n    state a: Int = 1;\n    on Go { a = a + 1; }\n    view { Column { Button { label: \"Add\" on: Go } Text { \"a is {a}\" } } }\n}\n";
    let file = scratch("ok.cui", src);
    let (code, _, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stderr}");
}
