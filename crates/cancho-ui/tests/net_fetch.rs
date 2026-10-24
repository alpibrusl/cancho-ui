//! `net.fetch(url)` (`docs/adapters.md` section 15): `net.get` that keeps the answer of an HTTP error status. What is
//! emitted, what is refused, how a trace scripts it. The browser side is `scripts/check-plant.js` (a `503` with a body).
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const FETCH: &str = include_str!("../../../examples/fetch/fetch.cui");

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
    let dir = common::scratch_root().join(format!("cancho-ui-net-fetch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn fetching() -> String {
    let src = FETCH.replace("net.get(url)", "net.fetch(url)");
    assert_ne!(src, FETCH, "the call was replaced");
    src
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

#[test]
fn net_fetch_is_emitted_beside_the_shared_request_function() {
    let file = scratch("fetch.cui", &fetching());
    let out = scratch("fetch.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        js.contains("__cui_net_fetch(__state_url, __cui_net_hosts, __cui_cred)"),
        "{js}"
    );
    assert!(js.contains("function __cui_net_request(url, hosts, envelope, cred)"));
    assert!(js.contains("JSON.stringify({status: response.status, body: text})"));
    assert!(js.contains("if (!response.ok && !envelope)"));
    // docs/adapters.md 16: a superseded or disposed request is aborted
    assert!(js.contains("owner.abort = function () { stopped = true; controller.abort(); };"));
    assert!(js.contains("if (prev) { prev.cancelled = true; if (prev.abort) { prev.abort(); } }"));
    assert!(
        js.contains("t.disposed = true; __cui_inflight[g] = null; if (t.abort) { t.abort(); }")
    );
}

#[test]
fn net_fetch_takes_one_argument_and_needs_network() {
    let two = fetching().replace("net.fetch(url)", "net.fetch(url, url)");
    assert_eq!(
        codes(&two, "two.cui"),
        ["builtin-shape-mismatch"],
        "two arguments"
    );
    let none = FETCH
        .replace("    requires Network;\n", "")
        .replace("net.get(url)", "net.fetch(url)");
    assert_eq!(codes(&none, "none.cui"), ["capability-missing"]);
}

#[test]
fn a_literal_url_on_another_host_is_refused_for_fetch_as_for_get() {
    let src = fetching().replace(
        "await(net.fetch(url))",
        "await(net.fetch(\"https://other.example/x\"))",
    );
    assert_eq!(
        codes(&src, "bound.cui"),
        ["capability-bound-violated"],
        "no Network(\"host\") for it"
    );
    let allowed = src.replace("requires Network;", "requires Network(\"other.example\");");
    let file = scratch("allowed.cui", &allowed);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_trace_scripts_the_envelope_and_the_two_calls_are_separate_requests() {
    let src = "component Both {\n    requires Network;\n    state g: Result[String, String] = Ok(\"\");\n    state f: Result[String, String] = Ok(\"\");\n    state code: String = \"-\";\n    on Go { g = await(net.get(\"a\")); }\n    on Fetch { f = await(net.fetch(\"a\")); code = match f { Ok(reply) => { json.get(reply, \"status\", \"-\") } Err(e) => { e } Cancelled => { \"-\" } }; }\n    view { Text { \"{code}\" } }\n}\n";
    let file = scratch("both.cui", src);
    let trace = scratch(
        "both.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Both","steps":[
            {"event":"Go","expect":{"pending":[1]}},
            {"event":"Fetch","expect":{"pending":[1,2]}},
            {"resolve":2,"with":{"Ok":"{\"status\":503,\"body\":\"down\"}"},"state":{"code":"503"}},
            {"resolve":1,"with":{"Err":"HTTP 503"},"state":{"g":{"Err":"HTTP 503"}}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("issued #2 net.fetch(\"a\")"), "{stdout}");
}
