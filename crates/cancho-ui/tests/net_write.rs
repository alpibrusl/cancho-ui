//! `net.put(url, body)` and `net.delete(url)` (`docs/adapters.md` section 21): the other writes, under the capability
//! `net.post` already has. What is emitted, what is refused, how a trace scripts them. The browser side is
//! `scripts/check-write.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const FETCH: &str = include_str!("../../../examples/fetch/fetch.cui");

const PUTTER: &str = "component Putter {\n    requires NetworkWrite;\n    state note: String = \"hello\";\n    state r: Result[String, String] = Ok(\"\");\n    state code: String = \"-\";\n    on Save {\n        r = await(net.put(\"/api/notes/1\", note));\n        code = match r { Ok(reply) => { json.get(reply, \"status\", \"-\") } Err(e) => { e } Cancelled => { \"cancelled\" } };\n    }\n    view { Column { Text { \"{code}\" } Button { label: \"Save\" on: Save } } }\n}\n";

const DELETER: &str = "component Deleter {\n    requires NetworkWrite;\n    state r: Result[String, String] = Ok(\"\");\n    state code: String = \"-\";\n    on Remove {\n        r = await(net.delete(\"/api/notes/1\"));\n        code = match r { Ok(reply) => { json.get(reply, \"status\", \"-\") } Err(e) => { e } Cancelled => { \"cancelled\" } };\n    }\n    view { Column { Text { \"{code}\" } Button { label: \"Remove\" on: Remove } } }\n}\n";

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
    let dir = common::scratch_root().join(format!("cancho-ui-net-write-{}", std::process::id()));
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

fn built(src: &str, name: &str) -> String {
    // a credential needs the page to be mounted with it, so no auto-mount (section 18)
    let file = scratch(&format!("{name}.cui"), src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "--no-auto-mount", "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

/// The text of one emitted adapter function, up to its closing brace.
fn adapter<'a>(js: &'a str, name: &str) -> &'a str {
    let text = js
        .split(&format!("function {name}"))
        .nth(1)
        .expect("the adapter is emitted");
    text.split("\n}\n").next().expect("its body")
}

#[test]
fn each_adapter_is_emitted_only_for_a_program_that_uses_it() {
    let js = built(PUTTER, "putter");
    assert!(
        js.contains("__cui_net_put(\"/api/notes/1\", __state_note, __cui_cred)"),
        "{js}"
    );
    assert!(js.contains("method: \"PUT\""));
    assert!(!js.contains("function __cui_net_delete"), "{js}");
    assert!(!js.contains("function __cui_net_post"), "{js}");
    assert!(!js.contains("method: \"DELETE\""));
    let js = built(DELETER, "deleter");
    assert!(
        js.contains("__cui_net_delete(\"/api/notes/1\", __cui_cred)"),
        "{js}"
    );
    assert!(js.contains("method: \"DELETE\""));
    assert!(!js.contains("function __cui_net_put"), "{js}");
    assert!(!js.contains("method: \"PUT\""));
    // a program that only reads, and one that only posts, carry neither
    let js = built(FETCH, "reader");
    assert!(!js.contains("__cui_net_put") && !js.contains("__cui_net_delete"));
    let post = PUTTER.replace(
        "net.put(\"/api/notes/1\", note)",
        "net.post(\"/api/notes\", note)",
    );
    let js = built(&post, "poster");
    assert!(js.contains("function __cui_net_post"));
    assert!(!js.contains("__cui_net_put") && !js.contains("method: \"PUT\""));
}

#[test]
fn the_guards_are_the_ones_of_net_post_and_neither_aborts() {
    for (src, name, method) in [
        (PUTTER, "__cui_net_put", "PUT"),
        (DELETER, "__cui_net_delete", "DELETE"),
    ] {
        let js = built(src, name);
        let a = adapter(&js, name);
        assert!(a.contains(&format!("method: \"{method}\"")), "{a}");
        assert!(
            a.contains("credentials: \"same-origin\", redirect: \"manual\""),
            "{a}"
        );
        assert!(
            a.contains("target.origin !== window.location.origin"),
            "{a}"
        );
        assert!(
            a.contains("a write reaches only this site's own origin"),
            "{a}"
        );
        assert!(a.contains("1048576") && a.contains("10000"), "{a}");
        assert!(a.contains("__cui_credential_check(cred, resolve)"), "{a}");
        assert!(a.contains("init.headers.Authorization = auth"), "{a}");
        assert!(!a.contains("owner"), "no abort wiring: {a}");
    }
}

#[test]
fn delete_sends_no_body_and_no_content_type_and_put_sends_json() {
    let js = built(DELETER, "deleter-wire");
    let a = adapter(&js, "__cui_net_delete");
    assert!(
        !a.contains("Content-Type") && !a.contains("body: String"),
        "{a}"
    );
    let js = built(PUTTER, "putter-wire");
    let a = adapter(&js, "__cui_net_put");
    assert!(
        a.contains("\"Content-Type\": \"application/json\"") && a.contains("body: String(body)"),
        "{a}"
    );
}

#[test]
fn a_write_needs_its_capability_and_a_read_does_not_grant_it() {
    for (src, name) in [(PUTTER, "put"), (DELETER, "delete")] {
        let none = src.replace("    requires NetworkWrite;\n", "");
        assert_eq!(
            codes(&none, &format!("{name}-none.cui")),
            ["capability-missing"]
        );
        let read_only = src.replace("NetworkWrite", "Network");
        assert_eq!(
            codes(&read_only, &format!("{name}-read.cui")),
            ["capability-missing"]
        );
        let unused = src.replace(
            "    requires NetworkWrite;\n",
            "    requires NetworkWrite;\n    requires Network;\n",
        );
        assert_eq!(
            codes(&unused, &format!("{name}-unused.cui")),
            ["capability-unused"]
        );
    }
}

#[test]
fn put_takes_two_arguments_and_delete_one() {
    for call in [
        "net.put(\"/a\")",
        "net.put(\"/a\", note, note)",
        "net.put()",
    ] {
        let src = PUTTER.replace("net.put(\"/api/notes/1\", note)", call);
        assert_eq!(
            codes(&src, "put-arity.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
    for call in ["net.delete()", "net.delete(\"/a\", \"/b\")"] {
        let src = DELETER.replace("net.delete(\"/api/notes/1\")", call);
        assert_eq!(
            codes(&src, "delete-arity.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
}

#[test]
fn a_write_takes_no_host_bound_and_a_literal_on_another_host_is_refused() {
    for (src, name) in [(PUTTER, "put"), (DELETER, "delete")] {
        let bound = src.replace("NetworkWrite;", "NetworkWrite(\"other.example\");");
        assert_eq!(
            codes(&bound, &format!("{name}-bound.cui")),
            ["capability-narrowing-illegal"]
        );
        for url in [
            "https://other.example/x",
            "//other.example/x",
            "http://localhost:9/x",
        ] {
            let s = src.replace("\"/api/notes/1\"", &format!("\"{url}\""));
            assert_eq!(
                codes(&s, &format!("{name}-host.cui")),
                ["capability-bound-violated"],
                "{name} {url}"
            );
        }
        let same = src.replace("\"/api/notes/1\"", "\"api/notes/1?x=1\"");
        let file = scratch(&format!("{name}-same.cui"), &same);
        let (code, stdout, stderr) = run(&["check", p(&file)]);
        assert_eq!(code, 0, "{stdout}{stderr}");
    }
}

#[test]
fn a_credential_may_be_used_by_a_put_or_a_delete_alone() {
    for (src, name) in [(PUTTER, "put"), (DELETER, "delete")] {
        let with = src.replace(
            "    requires NetworkWrite;\n",
            "    requires NetworkWrite;\n    requires Credential(\"api\");\n",
        );
        let file = scratch(&format!("{name}-cred.cui"), &with);
        let (code, stdout, stderr) = run(&["check", p(&file)]);
        assert_eq!(code, 0, "{stdout}{stderr}");
        // and the credential is handed to the adapter
        let js = built(&with, &format!("{name}-cred"));
        let call = if name == "put" {
            "return __cui_net_put("
        } else {
            "return __cui_net_delete("
        };
        let at = js.find(call).expect("the call is emitted");
        assert!(
            js[at..]
                .lines()
                .next()
                .expect("a line")
                .contains("__cui_cred)"),
            "{js}"
        );
    }
}

#[test]
fn a_trace_scripts_both_and_a_newer_answer_of_the_same_call_wins() {
    let file = scratch("put.cui", PUTTER);
    let trace = scratch(
        "put.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Putter","steps":[
            {"event":"Save","expect":{"pending":[1]}},
            {"event":"Save","expect":{"pending":[1,2]}},
            {"resolve":1,"with":{"Ok":"{\"status\":200,\"body\":\"\"}"},"state":{"code":"-"}},
            {"resolve":2,"with":{"Ok":"{\"status\":409,\"body\":\"stale\"}"},"state":{"code":"409"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("issued #2 net.put(\"/api/notes/1\", \"hello\")"),
        "{stdout}"
    );

    let file = scratch("delete.cui", DELETER);
    let trace = scratch(
        "delete.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Deleter","steps":[
            {"event":"Remove","expect":{"pending":[1]}},
            {"event":"Remove","expect":{"pending":[1,2]}},
            {"resolve":1,"with":{"Ok":"{\"status\":204,\"body\":\"\"}"},"state":{"code":"-"}},
            {"resolve":2,"with":{"Ok":"{\"status\":404,\"body\":\"\"}"},"state":{"code":"404"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("issued #2 net.delete(\"/api/notes/1\")"),
        "{stdout}"
    );
}

#[test]
fn a_put_and_a_delete_do_not_supersede_each_other() {
    // two groups: a put in flight and a delete issued after it are both answered; had they shared a group, the
    // put's answer would be dropped and `a` would stay "-"
    let src = "component Both {\n    requires NetworkWrite;\n    state r: Result[String, String] = Ok(\"\");\n    state s: Result[String, String] = Ok(\"\");\n    state a: String = \"-\";\n    state b: String = \"-\";\n    on A {\n        r = await(net.put(\"/a\", \"1\"));\n        a = match r { Ok(reply) => { json.get(reply, \"status\", \"?\") } Err(e) => { e } Cancelled => { \"cancelled\" } };\n    }\n    on B {\n        s = await(net.delete(\"/a\"));\n        b = match s { Ok(reply) => { json.get(reply, \"status\", \"?\") } Err(e) => { e } Cancelled => { \"cancelled\" } };\n    }\n    view { Column { Text { \"{a}{b}\" } Button { label: \"A\" on: A } Button { label: \"B\" on: B } } }\n}\n";
    let file = scratch("both.cui", src);
    let trace = scratch(
        "both.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Both","steps":[
            {"event":"A","expect":{"pending":[1]}},
            {"event":"B","expect":{"pending":[1,2]}},
            {"resolve":1,"with":{"Ok":"{\"status\":200,\"body\":\"\"}"},"state":{"a":"200","b":"-"}},
            {"resolve":2,"with":{"Ok":"{\"status\":204,\"body\":\"\"}"},"state":{"a":"200","b":"204"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_non_string_argument_is_a_replay_type_error() {
    let cases = [
        (PUTTER.replace("note));", "1));"), "Putter", "Save"),
        (
            DELETER.replace("net.delete(\"/api/notes/1\")", "net.delete(1)"),
            "Deleter",
            "Remove",
        ),
    ];
    for (i, (src, component, event)) in cases.iter().enumerate() {
        let file = scratch(&format!("bad{i}.cui"), src);
        let trace = scratch(
            &format!("bad{i}.trace.json"),
            &format!(
                r#"{{"schema":"cancho-ui-trace","version":"0.1.0","component":"{component}","steps":[{{"event":"{event}","expect":{{"pending":[1]}}}}]}}"#
            ),
        );
        let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
        assert_ne!(code, 0, "{src}");
        assert!(stdout.contains("replay-type-error"), "{stdout}");
    }
}
