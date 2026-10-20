//! `net.get`, the real `Network` adapter (`docs/adapters.md` section 9): what the compiler emits and refuses, and
//! how a trace scripts it. The browser-side checks (same-origin only, redirects, size, timeout, offline) are
//! `scripts/check-network.js`, outside the gate because they need a browser.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-network-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn build_js(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

fn check_codes(src: &str, name: &str) -> Vec<String> {
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
fn only_the_adapter_a_program_uses_is_emitted() {
    let js = build_js(FETCH, "fetch.cui");
    assert!(js.contains("function __cui_net_get(url, hosts)"));
    assert!(
        js.contains("__cui_net_get(__state_url, __cui_net_hosts)"),
        "{js}"
    );
    assert!(
        !js.contains("__CUI_CORPUS") && !js.contains("__cui_net_search"),
        "a program that only fetches carries no simulated search"
    );
    let search = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/search/search.cui"),
    )
    .expect("the search example");
    let other = build_js(&search, "search.cui");
    assert!(other.contains("__cui_net_search") && !other.contains("__cui_net_get"));
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    assert!(!build_js(&counter, "counter.cui").contains("fetch("));
}

#[test]
fn the_adapter_keeps_its_four_guards() {
    let js = build_js(FETCH, "fetch-guards.cui");
    for needle in [
        "target.origin !== window.location.origin && hosts.indexOf(target.host.toLowerCase()) === -1",
        "redirect: \"manual\"",
        "text.length > 1048576",
        "setTimeout(function () { controller.abort(); }, 10000)",
    ] {
        assert!(js.contains(needle), "the adapter lacks {needle:?}");
    }
    assert_eq!(
        js.matches(" fetch(target.href").count(),
        1,
        "one fetch, behind the guards"
    );
}

#[test]
fn net_get_without_a_declaration_is_capability_missing_and_the_wrong_arity_is_a_shape_mismatch() {
    let missing = "component A {\n    state r: Result[String, String] = Ok(\"\");\n    on Go { r = await(net.get(\"a\")); }\n    view { Text { \"x\" } }\n}\n";
    assert_eq!(check_codes(missing, "missing.cui"), ["capability-missing"]);
    for call in ["net.get()", "net.get(\"a\", \"b\")"] {
        let src = format!(
            "component A {{\n    requires Network;\n    state r: Result[String, String] = Ok(\"\");\n    on Go {{ r = await({call}); }}\n    view {{ Text {{ \"x\" }} }}\n}}\n"
        );
        assert_eq!(
            check_codes(&src, "arity.cui"),
            ["builtin-shape-mismatch"],
            "{call}"
        );
    }
}

#[test]
fn the_example_trace_passes_and_a_wrong_answer_fails() {
    let (code, stdout, stderr) = run(&[
        "test",
        "examples/fetch/fetch.cui",
        "examples/fetch/fetch.trace.json",
    ]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("net.get(\"data.txt\")"), "{stdout}");
    let wrong = scratch(
        "wrong.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Load"},
            {"resolve":1,"with":{"Err":"HTTP 500"},"state":{"status":"done"}}]}"#,
    );
    let (code, stdout, _) = run(&["test", "examples/fetch/fetch.cui", p(&wrong)]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("test-state-mismatch"), "{stdout}");
}

#[test]
fn a_url_that_is_not_a_string_is_a_replay_type_error() {
    let src = FETCH.replace("net.get(url)", "net.get(1)");
    let file = scratch("typeerr.cui", &src);
    let trace = scratch(
        "typeerr.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[{"event":"Load","state":{"status":"loading"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 4, "{stdout}{stderr}");
    assert!(stdout.contains("replay-type-error"), "{stdout}");
}

#[test]
fn net_get_requests_are_their_own_cancellation_group() {
    // A `net.search` in flight is not superseded by a `net.get`: the groups are the effect names.
    let src = "component A {\n    requires Network;\n    state a: Result[List[String], String] = Ok([]);\n    state b: Result[String, String] = Ok(\"\");\n    on Search { a = await(net.search(\"q\")); }\n    on Get { b = await(net.get(\"u\")); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("groups.cui", src);
    let trace = scratch(
        "groups.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Search","expect":{"pending":[1]}},
            {"event":"Get","expect":{"pending":[1,2]}},
            {"resolve":1,"with":{"Ok":["x"]},"expect":{"fired":[{"request":1,"delivered":"Ok","outcome":"completed"}]}},
            {"resolve":2,"with":{"Ok":"y"},"expect":{"fired":[{"request":2,"delivered":"Ok","outcome":"completed"}]}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}
