//! Streams (`docs/streams.md`, issue #78): `subscribe Event "url" [status Event];`. What parses, what is refused with which tag,
//! what authority it costs, how replay plays the server, and that the page carries the numbers replay uses.
//! `tests/stream_corpus.json` is the file replay is run over here and Chromium is run over in `scripts/check-stream.js`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-stream-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component with the given members, a message event `Got(text)`, a status event `Link(word)` and a plain event.
fn comp(members: &str) -> String {
    format!(
        "component Feed {{\n    state last: String = \"\";\n    state link: String = \"\";\n{members}\n    on Got(text) {{ last = text; }}\n    on Link(word) {{ link = word; }}\n    on Plain {{ last = \"\"; }}\n    view {{ Column {{ Text {{ \"{{link}} {{last}}\" }} }} }}\n}}\n"
    )
}

const OK: &str = "    requires Network;\n    subscribe Got \"/api/stream\" status Link;";

fn codes(src: &str) -> Vec<String> {
    let file = scratch("f.cui", src);
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
    let file = scratch("f.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{src}\n{stdout}{stderr}");
}

fn replay(src: &str, trace: &Value) -> (i32, Value) {
    let file = scratch("f.cui", src);
    let trace = scratch("f.trace.json", &trace.to_string());
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    let report: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("a JSON report ({e}): {stdout}{stderr}"));
    (code, report)
}

fn trace(steps: Value) -> Value {
    json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps})
}

fn rules(report: &Value) -> Vec<String> {
    report["failures"]
        .as_array()
        .expect("failures")
        .iter()
        .map(|f| f["rule"].as_str().expect("a rule").to_string())
        .collect()
}

fn utf16_len(v: &Value) -> usize {
    v.as_str().expect("a string").encode_utf16().count()
}

// ---- the corpus -------------------------------------------------------------------------------------------------

fn corpus() -> Value {
    let text = include_str!("../../../tests/stream_corpus.json");
    serde_json::from_str(text).expect("the corpus is JSON")
}

/// The text a corpus step sends: a string, or a list of `[text, times]` pairs.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| {
                let t = p[0].as_str().expect("a text");
                t.repeat(p[1].as_u64().expect("a count") as usize)
            })
            .collect(),
        other => panic!("a corpus text is a string or pairs: {other}"),
    }
}

/// A corpus step as a trace step: what the server or the user does, and what replay must report after it.
fn trace_step(s: &Value) -> Value {
    let mut step = match s["do"].as_str().expect("a verb") {
        "mount" => json!({"advance": 0}),
        "open" => json!({"stream": "open"}),
        "drop" => json!({"stream": "drop"}),
        "refuse" => json!({"stream": "refuse"}),
        "message" => {
            let mut m = json!({"message": text_of(&s["arg"])});
            if let Some(id) = s.get("id") {
                m["id"] = id.clone();
            }
            m
        }
        "advance" => json!({"advance": s["ms"]}),
        "hide" => json!({"hide": true}),
        "show" => json!({"show": true}),
        "dispose" => json!({"dispose": true}),
        other => panic!("unknown corpus verb {other}"),
    };
    let mut state = serde_json::Map::new();
    for k in ["link", "log"] {
        if let Some(v) = s.get(k) {
            state.insert(k.to_string(), v.clone());
        }
    }
    if !state.is_empty() {
        step["state"] = Value::Object(state);
    }
    let mut stream = serde_json::Map::new();
    for k in ["retry_in_ms", "last_event_id", "attempt"] {
        if let Some(v) = s.get(k) {
            stream.insert(k.to_string(), v.clone());
        }
    }
    // what the next request carries is the id the stream holds when it is made
    if let Some(v) = s.get("request_id") {
        stream.insert("last_event_id".to_string(), v.clone());
    }
    if !stream.is_empty() {
        step["expect"] = json!({"stream": Value::Object(stream)});
    }
    step
}

#[test]
fn the_corpus_passes_in_replay() {
    let c = corpus();
    let program = c["program"]
        .as_array()
        .expect("program lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 12, "the corpus has {} cases", cases.len());
    for case in cases {
        let why = case["why"].as_str().expect("a why");
        let steps: Vec<Value> = case["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .map(trace_step)
            .collect();
        let mut t = trace(Value::Array(steps));
        t["component"] = json!("Feed");
        let (code, report) = replay(&program, &t);
        assert_eq!(code, 0, "{why}\n{report:#}");
        // the length of the log is checked here, not in a state expectation: it is up to 65,537 characters long
        for (i, s) in case["steps"].as_array().expect("steps").iter().enumerate() {
            if let Some(n) = s.get("log_len") {
                let got = utf16_len(&report["steps"][i]["state"]["log"]);
                assert_eq!(Some(got as u64), n.as_u64(), "{why}, step {i}: log length");
            }
        }
    }
}

#[test]
fn the_corpus_has_what_the_note_promises() {
    let c = corpus();
    let text = serde_json::to_string(&c["cases"]).expect("cases");
    // the whole backoff list, both bounds of the message, every failed case, hidden and shown, disposal, an id sent back
    for needle in [
        "\"retry_in_ms\":30000",
        "\"retry_in_ms\":16000",
        "\"retry_in_ms\":1000",
        "65536",
        "65537",
        "\"how\":\"500\"",
        "\"how\":\"type\"",
        "\"how\":\"redirect\"",
        "\"do\":\"hide\"",
        "\"do\":\"show\"",
        "\"do\":\"dispose\"",
        "\"request_id\":\"7\"",
    ] {
        assert!(text.contains(needle), "the corpus lacks {needle}");
    }
    assert!(c["wire"].as_array().expect("wire").len() >= 20);
}

// ---- what parses and what is refused -----------------------------------------------------------------------------

#[test]
fn a_subscribing_component_is_accepted() {
    accepted(&comp(OK));
    // no status event, a full URL on an allowed host, a relative path
    accepted(&comp(
        "    requires Network(\"api.example.com\");\n    subscribe Got \"https://api.example.com/s?x=1\";",
    ));
    accepted(&comp(
        "    requires Network;\n    subscribe Got \"stream\";",
    ));
}

#[test]
fn subscribe_event_unknown_params_and_duplicate_are_refused_with_their_tags() {
    assert_eq!(
        codes(&comp(
            "    requires Network;\n    subscribe Nowhere \"/s\";"
        )),
        ["subscribe-event-unknown"]
    );
    assert_eq!(
        codes(&comp(
            "    requires Network;\n    subscribe Got \"/s\" status Nowhere;"
        )),
        ["subscribe-event-unknown"]
    );
    // no parameter, and two parameters, for the message event and for the status event
    assert_eq!(
        codes(&comp("    requires Network;\n    subscribe Plain \"/s\";")),
        ["subscribe-event-params"]
    );
    assert_eq!(
        codes(&comp(
            "    requires Network;\n    subscribe Got \"/s\" status Plain;"
        )),
        ["subscribe-event-params"]
    );
    let two = comp("    requires Network;\n    subscribe Two \"/s\";").replace(
        "    on Plain",
        "    on Two(a, b) { last = a; }\n    on Plain",
    );
    assert_eq!(codes(&two), ["subscribe-event-params"]);
    // one stream per component
    assert_eq!(
        codes(&comp(
            "    requires Network;\n    subscribe Got \"/a\";\n    subscribe Got \"/b\";"
        )),
        ["subscribe-duplicate"]
    );
}

#[test]
fn subscribe_url_invalid_is_refused_for_each_way_an_address_can_be_wrong() {
    let long = format!("/{}", "a".repeat(2000));
    let bad = [
        "".to_string(),
        "a b".to_string(),
        "/x#frag".to_string(),
        "//other.example/s".to_string(),
        "javascript:alert(1)".to_string(),
        "ftp://x.example/s".to_string(),
        "wss://x.example/s".to_string(),
        "data:text/plain,x".to_string(),
        "https://user:pw@api.example.com/s".to_string(),
        "https://".to_string(),
        "/a\\b".to_string(),
        "/caf\u{e9}".to_string(),
        long,
    ];
    for url in &bad {
        let src = comp(&format!(
            "    requires Network(\"api.example.com\");\n    subscribe Got {url:?};"
        ));
        let src = src.replace("\\u{e9}", "\u{e9}");
        assert_eq!(codes(&src), ["subscribe-url-invalid"], "address {url:?}");
    }
    // and an address of exactly 2000 characters is accepted
    let edge = format!("/{}", "a".repeat(1999));
    accepted(&comp(&format!(
        "    requires Network;\n    subscribe Got {edge:?};"
    )));
}

#[test]
fn a_subscribe_without_an_address_is_a_parse_error_not_a_panic() {
    let file = scratch("f.cui", &comp("    requires Network;\n    subscribe Got;"));
    let (code, _, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("string literal"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn a_stream_is_an_effect_of_network_with_the_hosts_checked() {
    // no Network: capability-missing
    assert_eq!(
        codes(&comp("    subscribe Got \"/s\";")),
        ["capability-missing"]
    );
    // a host the component's Network does not allow, and the right one
    assert_eq!(
        codes(&comp(
            "    requires Network(\"api.example.com\");\n    subscribe Got \"https://evil.example/s\";"
        )),
        ["capability-bound-violated"]
    );
    assert_eq!(
        codes(&comp(
            "    requires Network;\n    subscribe Got \"https://api.example.com/s\";"
        )),
        ["capability-bound-violated"]
    );
    accepted(&comp(
        "    requires Network(\"API.example.com\");\n    subscribe Got \"https://api.example.com:443/s\";",
    ));
    // a Network that nothing uses is still unused
    assert_eq!(codes(&comp("    requires Network;")), ["capability-unused"]);
}

#[test]
fn inspect_effects_and_diff_say_where_the_page_listens() {
    let file = scratch("f.cui", &comp(OK));
    let (code, out, err) = run(&["inspect", "effects", p(&file), "Feed", "--output", "json"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("\"subscribe\""), "{out}");
    assert!(out.contains("Network"), "{out}");

    let base = scratch(
        "base.cui",
        &comp("    requires Network;\n    subscribe Got \"/a\";"),
    );
    let cand = scratch(
        "cand.cui",
        &comp("    requires Network;\n    subscribe Got \"/b\";"),
    );
    let (code, out, err) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{out}{err}");
    assert!(out.contains("subscribe Got \"/a\""), "{out}");
    assert!(out.contains("subscribe Got \"/b\""), "{out}");
    let (code, json_out, _) = run(&["diff", p(&base), p(&cand), "--output", "json"]);
    assert_eq!(code, 5);
    assert!(json_out.contains("subscribe Got"), "{json_out}");
    // the same stream, the same program: no difference
    let (code, _, _) = run(&["diff", p(&base), p(&base)]);
    assert_eq!(code, 0);
}

// ---- replay -----------------------------------------------------------------------------------------------------

#[test]
fn a_status_event_is_optional_and_the_report_carries_the_stream() {
    let src = comp("    requires Network;\n    subscribe Got \"/s\";");
    let (code, report) = replay(
        &src,
        &trace(json!([
            {"stream": "open", "expect": {"stream": {"status": "open", "attempt": 0, "retry_in_ms": null, "last_event_id": null}}},
            {"message": "hi", "id": "5", "state": {"last": "hi"}, "expect": {"stream": {"last_event_id": "5"}}}
        ])),
    );
    assert_eq!(code, 0, "{report:#}");
    let s = &report["steps"][1]["stream"];
    assert_eq!(s["status"], "open");
    assert_eq!(s["last_event_id"], "5");
    assert_eq!(report["final_state"]["link"], "", "no status event ran");
}

#[test]
fn replay_refuses_a_stream_step_the_page_could_not_have_seen() {
    let src = comp(OK);
    let cases: Vec<(Value, &str)> = vec![
        // an answer nobody is waiting for
        (
            json!([{"stream": "open"}, {"stream": "open", "state": {"link": "open"}}]),
            "steps[1].stream",
        ),
        (
            json!([{"stream": "open"}, {"stream": "refuse", "state": {"link": "open"}}]),
            "steps[1].stream",
        ),
        // a drop with no connection: retrying, paused and failed have none
        (
            json!([{"stream": "drop"}, {"stream": "drop", "state": {"link": "retrying"}}]),
            "steps[1].stream",
        ),
        (
            json!([{"stream": "refuse"}, {"stream": "drop", "state": {"link": "failed"}}]),
            "steps[1].stream",
        ),
        // a message on a stream that is not open
        (
            json!([{"message": "early", "state": {"last": "early"}}]),
            "steps[0].message",
        ),
        (
            json!([{"stream": "drop"}, {"message": "late", "state": {"last": "late"}}]),
            "steps[1].message",
        ),
        (
            json!([{"stream": "refuse"}, {"message": "late", "state": {"last": "late"}}]),
            "steps[1].message",
        ),
        (
            json!([{"stream": "open"}, {"hide": true}, {"message": "hidden", "state": {"last": "hidden"}}]),
            "steps[2].message",
        ),
    ];
    for (steps, path) in cases {
        let (code, report) = replay(&src, &trace(steps.clone()));
        assert_eq!(code, 4, "{steps}\n{report:#}");
        assert_eq!(rules(&report), ["trace-stream-state"], "{steps}");
        assert_eq!(report["failures"][0]["path"], path, "{steps}");
    }
    // a component with no subscribe has no server to answer
    let plain = "component A {\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n";
    for step in [
        json!({"stream": "open", "state": {"n": 0}}),
        json!({"message": "x", "state": {"n": 0}}),
    ] {
        let (_, report) = replay(plain, &trace(json!([step])));
        assert_eq!(rules(&report), ["trace-stream-state"]);
    }
    // after disposal the connection is closed
    for step in [json!({"stream": "drop"}), json!({"message": "x"})] {
        let (_, report) = replay(
            &src,
            &trace(json!([{"stream": "open"}, {"dispose": true, "state": {"link": "open"}}, step])),
        );
        assert_eq!(rules(&report), ["trace-event-after-dispose"]);
    }
}

#[test]
fn a_malformed_stream_step_is_a_trace_error_not_a_panic() {
    let src = comp(OK);
    let cases: Vec<(Value, &str)> = vec![
        (
            json!({"stream": "close", "state": {"link": ""}}),
            "trace-bad-step",
        ),
        (
            json!({"stream": 1, "state": {"link": ""}}),
            "trace-bad-step",
        ),
        (
            json!({"message": 1, "state": {"link": ""}}),
            "trace-bad-step",
        ),
        (
            json!({"stream": "open", "id": "1", "state": {"link": ""}}),
            "trace-bad-step",
        ),
        (
            json!({"message": "m", "id": 5, "state": {"link": ""}}),
            "trace-bad-value",
        ),
        (
            json!({"message": "m", "id": "a\nb", "state": {"link": ""}}),
            "trace-bad-value",
        ),
        (
            json!({"message": "m", "id": "a\u{0}b", "state": {"link": ""}}),
            "trace-bad-value",
        ),
        (
            json!({"message": "m", "stream": "open", "state": {"link": ""}}),
            "trace-bad-step",
        ),
    ];
    for (step, rule) in cases {
        let (code, report) = replay(&src, &trace(json!([step.clone()])));
        assert_eq!(code, 4, "{step}");
        assert_eq!(rules(&report), [rule], "{step}");
    }
}

#[test]
fn a_message_over_the_bound_fails_the_stream_and_is_not_delivered() {
    let src = comp(OK);
    let ok = "a".repeat(65_536);
    let over = "a".repeat(65_537);
    let (code, report) = replay(
        &src,
        &trace(json!([
            {"stream": "open"},
            {"message": ok, "expect": {"stream": {"status": "open"}}},
            {"message": over, "state": {"link": "failed"}, "expect": {"stream": {"status": "failed"}}}
        ])),
    );
    assert_eq!(code, 0, "{report:#}");
    assert_eq!(utf16_len(&report["final_state"]["last"]), 65_536);
}

#[test]
fn hide_show_and_the_clock_move_the_stream_and_every_together() {
    let src =
        comp("    requires Network;\n    subscribe Got \"/s\" status Link;\n    every 1000 Plain;");
    let (code, report) = replay(
        &src,
        &trace(json!([
            {"stream": "open", "state": {"link": "open"}},
            {"message": "x", "state": {"last": "x"}},
            {"advance": 1000, "state": {"last": ""}},
            {"message": "y", "state": {"last": "y"}},
            {"hide": true, "state": {"link": "paused"}, "expect": {"stream": {"status": "paused"}}},
            {"advance": 5000, "state": {"link": "paused", "last": "y"}},
            {"show": true, "state": {"link": "connecting", "last": ""}},
            {"stream": "drop", "state": {"link": "retrying"}, "expect": {"stream": {"retry_in_ms": 1000, "attempt": 1}}},
            {"advance": 1000, "state": {"link": "connecting"}},
            {"stream": "open"},
            {"dispose": true},
            {"advance": 60000, "state": {"link": "open"}}
        ])),
    );
    assert_eq!(code, 0, "{report:#}");
}

#[test]
fn a_retry_due_at_the_same_instant_as_a_tick_runs_after_it() {
    let src =
        comp("    requires Network;\n    subscribe Got \"/s\" status Link;\n    every 1000 Plain;");
    // the tick clears `last`; the retry's `connecting` is written after: both are in the step
    let (code, report) = replay(
        &src,
        &trace(json!([
            {"stream": "open"},
            {"message": "x", "state": {"last": "x"}},
            {"stream": "drop", "expect": {"stream": {"retry_in_ms": 1000}}},
            {"advance": 1000, "state": {"link": "connecting", "last": ""}, "expect": {"ticks": [{"event": "Plain", "at": 1000}]}}
        ])),
    );
    assert_eq!(code, 0, "{report:#}");
    let writes: Vec<&str> = report["steps"][3]["writes"]
        .as_array()
        .expect("writes")
        .iter()
        .map(|w| w["field"].as_str().expect("a field"))
        .collect();
    assert_eq!(writes, ["last", "link"], "the tick, then the retry");
}

#[test]
fn a_message_runs_its_handler_while_an_earlier_one_waits_on_a_request() {
    let src = "component Slow {\n    requires Network;\n    state log: String = \"\";\n    subscribe Got \"/s\";\n    on Got(text) {\n        log = await(net.get(\"/slow\"));\n    }\n    view { Text { \"{log}\" } }\n}\n";
    // `net.get` here is in a Result; keep the program well typed
    let src = src
        .replace(
            "state log: String = \"\";",
            "state log: Result[String, String] = Ok(\"\");",
        )
        .replace("{log}", "x");
    let (code, report) = replay(
        &src,
        &trace(json!([
            {"stream": "open"},
            {"message": "a", "expect": {"pending": [1], "outcome": "suspended"}},
            {"message": "b", "expect": {"outcome": "suspended"}}
        ])),
    );
    assert_eq!(code, 0, "{report:#}");
    let issued = report["steps"][2]["issued"].as_array().expect("issued");
    assert_eq!(
        issued.len(),
        1,
        "the second message started its own request: {report:#}"
    );
    assert_eq!(report["requests"].as_array().expect("requests").len(), 2);
    assert_eq!(
        report["requests"][0]["superseded"], true,
        "the same effect supersedes the first, as two clicks would (docs/semantics.md)"
    );
}

// ---- the page ----------------------------------------------------------------------------------------------------

fn built(src: &str) -> String {
    let file = scratch("f.cui", src);
    let out = scratch("out.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out), "--no-auto-mount"]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

#[test]
fn the_page_uses_the_numbers_replay_uses() {
    let js = built(&comp(OK));
    assert!(
        js.contains("function __cui_stream_new("),
        "the stream adapter is emitted"
    );
    assert!(js.contains("function __cui_sse("), "the parser is emitted");
    assert!(
        js.contains("[1000, 2000, 4000, 8000, 16000, 30000]"),
        "the backoff list"
    );
    assert!(js.contains("65536"), "the message bound");
    assert!(js.contains("65542"), "the line bound");
    assert!(
        js.contains("url: \"/api/stream\""),
        "the address is the program's literal"
    );
    assert!(
        js.contains("__cui_stream.dispose();"),
        "disposal ends the connection"
    );
    // a stream is not net.search: the offline stand-in corpus is not emitted for it
    assert!(
        !js.contains("__CUI_CORPUS"),
        "the search stub is not emitted"
    );
    // a program with no stream carries none of it
    let plain = built("component A {\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n");
    assert!(!plain.contains("__cui_stream_new"));
    assert!(!plain.contains("__cui_sse"));
}

#[test]
fn the_page_sends_the_credential_the_component_declares() {
    let js = built(&comp(
        "    requires Network(\"127.0.0.1:9\");\n    requires Credential(\"api\", \"127.0.0.1:9\");\n    subscribe Got \"/s\";",
    ));
    assert!(js.contains("function __cui_credential_check("));
    assert!(js.contains("cred: __cui_cred"));
}
