//! Routing (`docs/routing.md`, step 1 of #75): what parses, what is refused with which tag, how replay moves the URL and the
//! history, and that `build` says it does not emit a routing page yet. The page itself is the next step.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-routing-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component with the given extra members and view body.
fn comp(members: &str, view: &str) -> String {
    format!(
        "component R {{\n    state path: String = \"/\";\n{members}\n    on Go(p) {{ path = p; }}\n    on Plain {{ path = \"/\"; }}\n    view {{ Column {{ {view} }} }}\n}}\n"
    )
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("r.cui", src);
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
    let file = scratch("r.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

fn replay(src: &str, trace: &str) -> (i32, String) {
    let file = scratch("r.cui", src);
    let trace = scratch("r.trace.json", trace);
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (code, format!("{stdout}{stderr}"))
}

fn rules(report: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(report).expect("the report is JSON");
    v["failures"]
        .as_array()
        .expect("failures")
        .iter()
        .map(|f| f["rule"].as_str().expect("a rule").to_string())
        .collect()
}

const OK_MEMBERS: &str = "    requires Router;\n    route Go;";

#[test]
fn a_routing_component_is_accepted() {
    accepted(&comp(OK_MEMBERS, "NavLink { label: \"Home\" to: \"/\" }"));
}

#[test]
fn route_event_unknown_params_and_duplicate_are_refused_with_their_tags() {
    // no such event
    assert_eq!(
        codes(&comp("    requires Router;\n    route Nowhere;", "")),
        ["route-event-unknown"]
    );
    // no parameter, and two parameters
    assert_eq!(
        codes(&comp("    requires Router;\n    route Plain;", "")),
        ["route-event-params"]
    );
    let two = comp("    requires Router;\n    route Two;", "").replace(
        "    on Plain",
        "    on Two(a, b) { path = a; }\n    on Plain",
    );
    assert_eq!(codes(&two), ["route-event-params"]);
    // two routes: one event receives every path
    let dup = comp("    requires Router;\n    route Go;\n    route Other;", "").replace(
        "    on Plain",
        "    on Other(q) { path = q; }\n    on Plain",
    );
    assert_eq!(codes(&dup), ["route-duplicate"]);
}

#[test]
fn route_and_router_calls_need_the_router_capability() {
    assert_eq!(
        codes(&comp("    route Go;", "")),
        ["capability-missing"],
        "a route reads the URL"
    );
    let call = comp("    requires Router;", "").replace(
        "on Plain { path = \"/\"; }",
        "on Plain { ok = router.go(\"/x\"); }",
    );
    let call = call.replace(
        "state path: String = \"/\";",
        "state path: String = \"/\";\n    state ok: Bool = false;",
    );
    accepted(&call);
    let missing = call.replace("    requires Router;\n", "");
    assert_eq!(codes(&missing), ["capability-missing"]);
    // `Router` takes no bound
    let bounded = comp("    requires Router(\"x\");", "");
    assert_eq!(codes(&bounded), ["capability-narrowing-illegal"]);
}

#[test]
fn router_calls_take_exactly_one_argument() {
    for call in [
        "router.go()",
        "router.go(\"/a\", \"/b\")",
        "router.replace()",
    ] {
        let src = comp("    requires Router;\n    state ok: Bool = false;", "")
            .replace("{ path = \"/\"; }", &format!("{{ ok = {call}; }}"));
        assert_eq!(codes(&src), ["builtin-shape-mismatch"], "{call}");
    }
}

#[test]
fn a_navlink_needs_a_label_and_a_legal_path() {
    for view in [
        "NavLink { label: \"x\" }",
        "NavLink { to: \"/x\" }",
        "NavLink { }",
    ] {
        assert_eq!(codes(&comp("", view)), ["navlink-incomplete"], "{view}");
    }
    for bad in [
        "items",
        "",
        "/a b",
        "/a#b",
        "/caf\u{e9}",
        "/a<b",
        "/a>b",
        "/a`b",
    ] {
        let view = format!("NavLink {{ label: \"x\" to: \"{bad}\" }}");
        assert_eq!(
            codes(&comp("", &view)),
            ["navlink-target-invalid"],
            "{bad:?}"
        );
    }
    let long = "/".to_string() + &"a".repeat(2000);
    let view = format!("NavLink {{ label: \"x\" to: \"{long}\" }}");
    assert_eq!(codes(&comp("", &view)), ["navlink-target-invalid"]);
    // an expression is not checked until it runs; a legal literal and a computed path are accepted
    accepted(&comp("", "NavLink { label: \"x\" to: \"/\" + path }"));
    accepted(&comp("", "NavLink { label: path to: \"/ok\" }"));
}

#[test]
fn bindings_that_do_not_belong_to_a_navlink_are_refused() {
    assert_eq!(
        codes(&comp("", "Button { label: \"x\" to: \"/x\" on: Plain }")),
        ["binding-unknown"],
        "to: is a binding of NavLink"
    );
    assert_eq!(
        codes(&comp("", "NavLink { label: \"x\" to: \"/x\" on: Plain }")),
        ["binding-unknown"],
        "a link follows its path, it fires no event"
    );
    assert_eq!(
        codes(&comp(
            "",
            "NavLink { label: \"x\" to: \"/x\" disabled: true }"
        )),
        ["binding-unknown"]
    );
}

#[test]
fn the_page_carries_the_router_only_when_the_program_uses_it() {
    let build = |src: &str| -> String {
        let file = scratch("b.cui", src);
        let out = scratch("b.js", "");
        let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 0, "{stdout}{stderr}");
        std::fs::read_to_string(&out).expect("the built module")
    };
    let plain = build(&comp("", ""));
    assert!(!plain.contains("__cui_route"), "{plain}");
    assert!(!plain.contains("hashchange"), "{plain}");
    // a link alone needs no adapter: it is an anchor
    let link = build(&comp("", "NavLink { label: \"x\" to: \"/x\" }"));
    assert!(
        link.contains("setAttribute('href', '#' + String("),
        "{link}"
    );
    assert!(!link.contains("hashchange"), "{link}");
    let routed = build(&base());
    assert!(routed.contains("function __cui_route_of("), "{routed}");
    assert!(routed.contains("addEventListener('hashchange'"), "{routed}");
    assert!(
        routed.contains("removeEventListener('hashchange'"),
        "{routed}"
    );
    assert!(routed.contains("__cui_router_go("), "{routed}");
    // the page has no way to leave the document: the link writes '#' first, always
    assert!(!routed.contains("location.href ="), "{routed}");
}

fn base() -> String {
    comp(OK_MEMBERS, "NavLink { label: \"Home\" to: \"/\" }")
        .replace(
            "on Plain { path = \"/\"; }",
            "on Plain { ok = router.go(\"/p\"); }",
        )
        .replace(
            "state path: String = \"/\";",
            "state path: String = \"/\";\n    state ok: Bool = false;",
        )
}

fn trace(url: Option<&str>, steps: &str) -> String {
    let url = url.map_or(String::new(), |u| format!("\"url\": \"{u}\","));
    format!(
        "{{ \"schema\": \"cancho-ui-trace\", \"version\": \"0.1.0\", {url} \"steps\": [{steps}] }}"
    )
}

#[test]
fn the_route_event_hears_the_load_path_then_each_change() {
    let t = trace(
        Some("/start"),
        r#"{ "advance": 0, "state": { "path": "/start" }, "expect": { "routes": ["/start"] } },
           { "navigate": "/a", "state": { "path": "/a" }, "expect": { "routes": ["/a"], "outcome": "ok" } },
           { "navigate": "/a", "expect": { "outcome": "unchanged", "routes": [] } },
           { "back": true, "state": { "path": "/start" }, "expect": { "history": { "entries": ["/start", "/a"], "position": 0 } } },
           { "forward": true, "state": { "path": "/a" } },
           { "event": "Plain", "state": { "ok": true, "path": "/p" }, "expect": { "routes": ["/p"], "url": "/p" } }"#,
    );
    let (code, report) = replay(&base(), &t);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn the_default_load_path_is_the_root() {
    let t = trace(
        None,
        r#"{ "advance": 0, "expect": { "routes": ["/"], "url": "/" } }"#,
    );
    let (code, report) = replay(&base(), &t);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn a_new_entry_drops_the_forward_entries() {
    let t = trace(
        None,
        r#"{ "navigate": "/a" }, { "navigate": "/b" }, { "back": true }, { "navigate": "/c", "expect": { "history": { "entries": ["/", "/a", "/c"], "position": 2 } } }"#,
    );
    let (code, report) = replay(&base(), &t);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn a_path_the_fragment_cannot_carry_is_answered_false() {
    for bad in [
        "",
        "no-slash",
        "/a b",
        "/a#b",
        "/caf\u{e9}",
        "/a<b",
        "/a>b",
        "/a`b",
    ] {
        let src = base().replace("router.go(\"/p\")", &format!("router.go(\"{bad}\")"));
        let t = trace(
            None,
            r#"{ "event": "Plain", "state": { "ok": false }, "expect": { "routes": ["/"], "url": "/" } }"#,
        );
        let (code, report) = replay(&src, &t);
        assert_eq!(code, 0, "{bad:?}: {report}");
    }
    let long = format!("/{}", "a".repeat(2000));
    let src = base().replace("router.go(\"/p\")", &format!("router.go(\"{long}\")"));
    let t = trace(None, r#"{ "event": "Plain", "state": { "ok": false } }"#);
    assert_eq!(replay(&src, &t).0, 0);
}

#[test]
fn go_to_the_current_path_is_true_and_fires_nothing_but_the_mount() {
    let src = base().replace("router.go(\"/p\")", "router.go(\"/\")");
    let t = trace(
        None,
        r#"{ "event": "Plain", "state": { "ok": true }, "expect": { "routes": ["/"], "history": { "entries": ["/"], "position": 0 } } }"#,
    );
    let (code, report) = replay(&src, &t);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn replace_changes_the_entry_without_adding_one() {
    let src = base().replace("router.go(\"/p\")", "router.replace(\"/r\")");
    let t = trace(
        None,
        r#"{ "event": "Plain", "state": { "ok": true, "path": "/r" }, "expect": { "routes": ["/", "/r"], "history": { "entries": ["/r"], "position": 0 } } }"#,
    );
    let (code, report) = replay(&src, &t);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn the_trace_cannot_describe_a_page_that_does_not_exist() {
    // a path that is not a route
    for bad in ["main", "", "/a b", "/a#b"] {
        let t = trace(
            None,
            &format!(r#"{{ "navigate": "{bad}", "state": {{ "path": "/" }} }}"#),
        );
        let (code, report) = replay(&base(), &t);
        assert_eq!(code, 4, "{bad:?}: {report}");
        assert_eq!(rules(&report), ["trace-navigate-invalid"], "{bad:?}");
    }
    // a load url that is not a route
    let t = trace(
        Some("main"),
        r#"{ "advance": 0, "state": { "path": "/" } }"#,
    );
    assert_eq!(rules(&replay(&base(), &t).1), ["trace-navigate-invalid"]);
    // nowhere to go back or forward to
    let t = trace(None, r#"{ "back": true, "state": { "path": "/" } }"#);
    assert_eq!(rules(&replay(&base(), &t).1), ["trace-history-empty"]);
    let t = trace(None, r#"{ "forward": true, "state": { "path": "/" } }"#);
    assert_eq!(rules(&replay(&base(), &t).1), ["trace-history-empty"]);
    // back and forward are `true`
    let t = trace(None, r#"{ "back": 1, "state": { "path": "/" } }"#);
    assert_eq!(rules(&replay(&base(), &t).1), ["trace-bad-step"]);
}

#[test]
fn a_handler_that_always_moves_the_page_stops_replay_at_a_limit() {
    // each route event navigates to a path never seen before
    let src = "component Loop {\n    requires Router;\n    state ok: Bool = false;\n    route Heard;\n    on Heard(p) { ok = router.go(p + \"x\"); }\n    view { Text { \"x\" } }\n}\n";
    let t = trace(None, r#"{ "advance": 0, "state": { "ok": true } }"#);
    let (code, report) = replay(src, &t);
    assert_eq!(code, 4, "{report}");
    assert_eq!(rules(&report), ["replay-route-limit"]);
}

#[test]
fn the_example_replays_and_its_authority_is_reported() {
    let (code, out, err) = run(&[
        "test",
        "examples/routes/routes.cui",
        "examples/routes/routes.trace.json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, _) = run(&[
        "inspect",
        "ir",
        "examples/routes/routes.cui",
        "--output",
        "json",
    ]);
    assert_eq!(code, 0);
    let ir: Value = serde_json::from_str(&out).expect("JSON");
    let c = &ir["components"][0];
    assert_eq!(c["schedules"][0]["kind"], "route");
    assert_eq!(c["schedules"][0]["event"], "Navigated");
    assert_eq!(c["requires"][0]["name"], "Router");
}

#[test]
fn the_diff_shows_a_route_and_the_new_authority() {
    let before = "component A {\n    state path: String = \"/\";\n    on Go(p) { path = p; }\n    view { Text { \"{path}\" } }\n}\n";
    let after = "component A {\n    requires Router;\n    route Go;\n    state path: String = \"/\";\n    on Go(p) { path = p; }\n    view { Text { \"{path}\" } }\n}\n";
    let a = scratch("a.cui", before);
    let b = scratch("b.cui", after);
    let (code, out, err) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{out}{err}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["authority"], "expanded");
    assert_eq!(
        v["components"]["changed"][0]["schedules"]["added"][0],
        "route Go"
    );
}

const CORPUS: &str = include_str!("../../../tests/routing_corpus.json");

/// The corpus as a replay trace: each step the page takes becomes the trace steps that model it. A fragment that is not a
/// route (`replay: false`) is the page's alone; replay has no model of it, and the corpus keeps it from being followed by a
/// step that would depend on it.
fn corpus_trace(c: &Value) -> String {
    let mut steps: Vec<Value> = Vec::new();
    for s in c["steps"].as_array().expect("steps") {
        if s["replay"] == Value::Bool(false) {
            continue;
        }
        let mut want = serde_json::json!({"log": s["log"], "path": s["path"]});
        if let Some(ok) = s.get("ok") {
            want["ok"] = ok.clone();
        }
        let expect = serde_json::json!({"routes": s["heard"]});
        let arg = s["arg"].as_str().unwrap_or("");
        match s["do"].as_str().expect("do") {
            "load" => {
                steps.push(serde_json::json!({"advance": 0, "state": want, "expect": expect}))
            }
            "click" => {
                let to = c["links"][arg].as_str().expect("a link");
                steps.push(serde_json::json!({"navigate": to, "state": want, "expect": expect}));
            }
            "hash" => steps
                .push(serde_json::json!({"navigate": &arg[1..], "state": want, "expect": expect})),
            "go" | "replace" => {
                steps.push(serde_json::json!({"event": "SetTarget", "params": [arg]}));
                let event = if s["do"] == "go" { "Go" } else { "Replace" };
                steps.push(serde_json::json!({"event": event, "state": want, "expect": expect}));
            }
            "two" => {
                steps.push(serde_json::json!({"event": "Two", "state": want, "expect": expect}))
            }
            "back" => {
                steps.push(serde_json::json!({"back": true, "state": want, "expect": expect}))
            }
            "forward" => {
                steps.push(serde_json::json!({"forward": true, "state": want, "expect": expect}))
            }
            other => panic!("the corpus has a step this test does not know: {other}"),
        }
    }
    serde_json::json!({
        "schema": "cancho-ui-trace", "version": "0.1.0", "url": c["load"], "steps": steps
    })
    .to_string()
}

#[test]
fn replay_agrees_with_every_step_of_the_corpus() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let src = c["program"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(
        c["steps"].as_array().expect("steps").len() >= 30,
        "the corpus shrank"
    );
    let (code, report) = replay(&src, &corpus_trace(&c));
    assert_eq!(code, 0, "{}", &report[..report.len().min(4000)]);
}
