//! A bearer token the program receives and sends (`docs/bearer-token.md`, #83): what is accepted and refused with which tag, what
//! replay keeps (the names of the credentials held, never a value), and what the emitted module carries. The browser side (the token
//! sent to the declared hosts only, gone after a reload) is `scripts/check-bearer.js`.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-bearer-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component that declares a credential, sends it, and has the given extra handler lines.
fn comp(requires: &str, handlers: &str) -> String {
    format!(
        "component B {{\n{requires}    requires Network;\n    state ok: Bool = false;\n    state t: String = \"\";\n    state r: Result[String, String] = Ok(\"\");\n    on Send {{ r = await(net.get(\"x\")); }}\n{handlers}\n    view {{ Text {{ \"{{ok}}\" }} }}\n}}\n"
    )
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("b.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "expected a refusal for:\n{src}\n{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

fn accepted(src: &str) {
    let file = scratch("b.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

const REQ: &str = "    requires Credential(\"api\");\n";
const SET: &str = "    on Use { ok = credential.set(\"api\", t); }\n    on Drop { ok = credential.clear(\"api\"); }";

#[test]
fn set_and_clear_of_the_declared_credential_are_accepted() {
    accepted(&comp(REQ, SET));
}

#[test]
fn the_name_is_the_declared_credential_as_a_literal() {
    // a name that is not the declared one, one that is not a literal, and a component that declares none
    for h in [
        "    on Use { ok = credential.set(\"other\", t); }",
        "    on Use { ok = credential.set(t, t); }",
        "    on Use { ok = credential.clear(\"other\"); }",
        "    on Use { ok = credential.clear(t); }",
    ] {
        assert_eq!(codes(&comp(REQ, h)), ["credential-name-invalid"], "{h}");
    }
}

#[test]
fn a_call_needs_the_credential_capability() {
    assert_eq!(
        codes(&comp("", "    on Use { ok = credential.set(\"api\", t); }")),
        ["capability-missing"]
    );
}

#[test]
fn the_calls_take_their_arguments() {
    for h in [
        "    on Use { ok = credential.set(\"api\"); }",
        "    on Use { ok = credential.set(\"api\", t, t); }",
        "    on Use { ok = credential.clear(); }",
        "    on Use { ok = credential.clear(\"api\", t); }",
    ] {
        assert_eq!(codes(&comp(REQ, h)), ["builtin-shape-mismatch"], "{h}");
    }
}

#[test]
fn a_credential_that_is_set_and_never_sent_is_unused() {
    let src = "component B {\n    requires Network;\n    requires Credential(\"api\");\n    state ok: Bool = false;\n    on Use { ok = credential.set(\"api\", \"tok\"); }\n    view { Text { \"{ok}\" } }\n}\n";
    assert_eq!(codes(src), ["capability-unused"]);
}

#[test]
fn a_component_that_sets_its_token_can_be_built_auto_mounted() {
    let file = scratch("b.cui", &comp(REQ, SET));
    let out = scratch("b.js", "");
    let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let js = std::fs::read_to_string(&out).expect("the module");
    assert!(js.contains("anon: true"), "{js}");
    assert!(js.contains("__cui_credential_set(__cui_cred, "), "{js}");
    assert!(js.contains("__cui_credential_clear(__cui_cred, "), "{js}");
    // and one that cannot is still refused, with its tag
    let file = scratch("c.cui", &comp(REQ, ""));
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("credential-needs-mount-options"),
        "{stderr}"
    );
    // built for a page that mounts it, a component that cannot set its token is blocked until it is given one
    let (code, _, _) = run(&["build", p(&file), "--no-auto-mount", "-o", p(&out)]);
    assert_eq!(code, 0);
    let js = std::fs::read_to_string(&out).expect("the module");
    assert!(js.contains("anon: false"), "{js}");
}

fn replay(src: &str, trace: &str) -> (i32, Value) {
    let file = scratch("r.cui", src);
    let trace = scratch("r.trace.json", trace);
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (
        code,
        serde_json::from_str(&stdout).unwrap_or(Value::String(stderr)),
    )
}

fn trace(steps: &str) -> String {
    format!("{{ \"schema\": \"cancho-ui-trace\", \"version\": \"0.1.0\", \"steps\": [{steps}] }}")
}

#[test]
fn replay_keeps_the_names_held_and_never_the_value() {
    let t = trace(
        r#"{ "event": "Tok", "params": ["secret-token-1"], "state": { "t": "secret-token-1" } },
           { "event": "Use", "state": { "ok": true }, "expect": { "credentials": ["api"] } },
           { "event": "Use", "state": { "ok": true }, "expect": { "credentials": ["api"] } },
           { "event": "Drop", "state": { "ok": true }, "expect": { "credentials": [] } },
           { "event": "Tok", "params": ["has space"] },
           { "event": "Use", "state": { "ok": false }, "expect": { "credentials": [] } },
           { "event": "Tok", "params": [""] },
           { "event": "Use", "state": { "ok": false }, "expect": { "credentials": [] } },
           { "event": "Send", "expect": { "outcome": "suspended", "issued": [{ "request": 1, "effect": "net.get" }] } }"#,
    );
    let src = comp(REQ, &format!("    on Tok(v) {{ t = v; }}\n{SET}"));
    let (code, report) = replay(&src, &t);
    assert_eq!(code, 0, "{report}");
    // the value is in the state the program wrote it to (it is the program's `t`), and in no credential field
    let steps = report["steps"].as_array().expect("steps");
    for s in steps {
        assert!(
            !s["credentials"].to_string().contains("secret-token"),
            "{s}"
        );
    }
    // a token longer than 4096 characters is not a token
    let long = "x".repeat(4097);
    let t = trace(&format!(
        r#"{{ "event": "Tok", "params": ["{long}"] }}, {{ "event": "Use", "state": {{ "ok": false }}, "expect": {{ "credentials": [] }} }}"#
    ));
    assert_eq!(replay(&src, &t).0, 0);
    let ok = "x".repeat(4096);
    let t = trace(&format!(
        r#"{{ "event": "Tok", "params": ["{ok}"] }}, {{ "event": "Use", "state": {{ "ok": true }}, "expect": {{ "credentials": ["api"] }} }}"#
    ));
    assert_eq!(replay(&src, &t).0, 0);
}

#[test]
fn a_component_without_a_credential_reports_none() {
    let src = "component B {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Text { \"{n}\" } }\n}\n";
    let (code, report) = replay(src, &trace(r#"{ "event": "Go", "state": { "n": 1 } }"#));
    assert_eq!(code, 0, "{report}");
    assert!(report["steps"][0].get("credentials").is_none(), "{report}");
}

#[test]
fn a_component_that_sets_its_token_is_anonymous_until_it_holds_one() {
    let src = comp(REQ, &format!("    on Tok(v) {{ t = v; }}\n{SET}"));
    let t = trace(
        r#"{ "event": "Send", "expect": { "outcome": "suspended" } },
           { "event": "Tok", "params": ["tok"] },
           { "event": "Use", "state": { "ok": true } },
           { "event": "Send", "expect": { "outcome": "suspended" } }"#,
    );
    let (code, report) = replay(&src, &t);
    assert_eq!(code, 0, "{report}");
    assert!(
        report["steps"][0]["issued"][0].get("credential").is_none(),
        "{report}"
    );
    assert_eq!(report["steps"][3]["issued"][0]["credential"], "api");
}

#[test]
fn the_example_replays_without_its_token_in_the_report() {
    let (code, out, err) = run(&[
        "test",
        "examples/bearer/bearer.cui",
        "examples/bearer/bearer.trace.json",
        "--output",
        "json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("t-8f3a"), "the token is in the report");
}

#[test]
fn the_diff_shows_the_credential_and_nothing_hides_a_set() {
    let before = comp("", "");
    let after = comp(REQ, SET);
    let a = scratch("a.cui", &before);
    let b = scratch("b.cui", &after);
    let (code, out, err) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{out}{err}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["authority"], "expanded");
    assert!(out.contains("Credential"), "{out}");
}
