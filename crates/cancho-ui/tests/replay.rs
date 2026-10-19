//! `cancho-ui test`: deterministic replay (docs/replay.md). Engine tests run
//! in-process over small programs written for the purpose; the CLI tests run the
//! binary over the example traces.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::{IrFile, lower_file};
use cancho_ui_replay::{TestReport, replay};
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value as Json, json};

// ---------------------------------------------------------------- helpers

fn ir(src: &str) -> IrFile {
    let ast = parse(lex(src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    lower_file(&ast).expect("lowers")
}

fn trace(steps: Json) -> String {
    json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string()
}

fn run(src: &str, steps: Json) -> TestReport {
    replay(&ir(src), &trace(steps), "prog.cui", "t.json")
}

fn rules(r: &TestReport) -> Vec<&'static str> {
    r.failures.iter().map(|f| f.rule).collect()
}

fn assert_pass(r: &TestReport) {
    assert!(
        r.passed_for_test(),
        "expected pass, failures: {:#?}",
        r.failures
    );
}

trait Passed {
    fn passed_for_test(&self) -> bool;
}
impl Passed for TestReport {
    fn passed_for_test(&self) -> bool {
        self.failures.is_empty() && self.result == "pass"
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const SEARCH: &str = include_str!("../../../examples/search/search.cui");

/// Two components' worth of ordering questions in one place: a counter-like
/// component whose handlers are sequences of assignments.
const SEQ: &str = r#"
component Seq {
    state a: Int = 1;
    state b: Int = 0;
    state c: Int = 0;
    on Go { a = a + 1; b = a * 10; c = b - a; }
    on Swap { b = 5; a = b; }
    view { Text { "{a}" } }
}
"#;

// ------------------------------------------------- sequential semantics (Q3)

#[test]
fn statements_run_in_order_and_each_sees_the_earlier_writes() {
    let r = run(
        SEQ,
        json!([
            {"event": "Go", "state": {"a": 2, "b": 20, "c": 18}},
            {"event": "Go", "state": {"a": 3, "b": 30, "c": 27}},
            {"event": "Swap", "state": {"b": 5, "a": 5}}
        ]),
    );
    assert_pass(&r);
    assert_eq!(r.steps_run, 3);
}

#[test]
fn events_are_applied_in_trace_order_and_replay_is_deterministic() {
    let steps = json!([
        {"event": "Go", "state": {"a": 2}},
        {"event": "Swap", "state": {"a": 5}},
        {"event": "Go", "state": {"a": 6, "b": 60}}
    ]);
    let first = run(SEQ, steps.clone());
    let second = run(SEQ, steps);
    assert_pass(&first);
    assert_eq!(
        serde_json::to_string(&first).expect("serializes"),
        serde_json::to_string(&second).expect("serializes"),
        "the same input must give the same report, byte for byte"
    );
}

#[test]
fn a_wrong_expectation_fails_with_the_assignment_that_wrote_the_field() {
    let r = run(SEQ, json!([{"event": "Go", "state": {"c": 19}}]));
    assert_eq!(rules(&r), ["test-state-mismatch"]);
    let f = &r.failures[0];
    assert_eq!(f.path, "steps[0].state.c");
    assert_eq!(f.expected, Some(json!(19)));
    assert_eq!(f.actual, Some(json!(18)));
    // `c = b - a;` is on line 6 of SEQ (the source starts with a newline).
    let span = f.span.as_ref().expect("the assignment is located");
    assert_eq!(span.start.line, 6);
}

#[test]
fn a_wrong_expect_fails_with_the_handler_span() {
    let r = run(
        SEQ,
        json!([{"event": "Go", "expect": {"outcome": "suspended"}}]),
    );
    assert_eq!(rules(&r), ["test-expect-mismatch"]);
    assert_eq!(r.failures[0].actual, Some(json!("completed")));
    assert!(r.failures[0].span.is_some());
}

#[test]
fn expectation_failures_do_not_stop_the_replay() {
    let r = run(
        SEQ,
        json!([
            {"event": "Go", "state": {"a": 99}},
            {"event": "Go", "state": {"a": 99}},
            {"event": "Go", "state": {"a": 4}}
        ]),
    );
    assert_eq!(rules(&r), ["test-state-mismatch", "test-state-mismatch"]);
    assert_eq!(r.steps_run, 3);
}

// ------------------------------------------------------------ expressions

const EXPR: &str = r#"
component Expr {
    state half: Float = 0.0;
    state s: String = "";
    state n: Int = 0;
    state ok: Bool = false;
    on Div(x) { half = x / 2; }
    on Cat(x) { s = "n=" + x; n = len(s); ok = n > 2; }
    on Wide { n = len("😀"); }
    on Pair(x, y) { n = x - y; }
    view { Text { "{s}" } }
}
"#;

#[test]
fn division_is_real_division_as_in_the_emitted_javascript() {
    let r = run(
        EXPR,
        json!([{"event": "Div", "params": [7], "state": {"half": 3.5}}]),
    );
    assert_pass(&r);
}

#[test]
fn string_concatenation_len_and_comparison() {
    let r = run(
        EXPR,
        json!([
            {"event": "Cat", "params": [42], "state": {"s": "n=42", "n": 4, "ok": true}},
            {"event": "Cat", "params": [""], "state": {"s": "n=", "n": 2, "ok": false}}
        ]),
    );
    assert_pass(&r);
}

#[test]
fn parameters_bind_in_declaration_order() {
    let r = run(
        EXPR,
        json!([{"event": "Pair", "params": [10, 3], "state": {"n": 7}}]),
    );
    assert_pass(&r);
}

#[test]
fn expect_compares_lists_by_length_and_elements() {
    // A request is pending, so `pending: []` and `pending: [2]` must both fail;
    // only `[1]` holds.
    let base = submit("a").as_array().expect("array").clone();
    for (expected, ok) in [
        (json!([1]), true),
        (json!([]), false),
        (json!([2]), false),
        (json!([1, 2]), false),
    ] {
        let mut steps = base.clone();
        steps.push(json!({"event": "ShowAll", "expect": {"pending": expected}}));
        let r = run(SEARCH, Json::Array(steps));
        assert_eq!(
            r.failures.is_empty(),
            ok,
            "expected {expected}: {:?}",
            r.failures
        );
    }
}

#[test]
fn len_counts_utf16_code_units_like_javascript() {
    let r = run(EXPR, json!([{"event": "Wide", "state": {"n": 2}}]));
    assert_pass(&r);
}

const FAULTY: &str = r#"
component Faulty {
    state n: Int = 0;
    state s: String = "x";
    on Zero { n = 1 / 0; }
    on Bad { n = s - 1; }
    on Eq { n = [1] == [1]; }
    on Huge { n = 1000000000000000000000000000000.0 * 1000000000000000000000000000000.0; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; n = n * n; }
    view { Text { "{n}" } }
}
"#;

#[test]
fn operators_outside_the_modelled_cases_are_refused_with_a_rule_and_a_span() {
    for (event, rule) in [
        ("Zero", "replay-non-finite"),
        ("Bad", "replay-type-error"),
        ("Eq", "replay-type-error"),
        ("Huge", "replay-non-finite"),
    ] {
        let r = run(FAULTY, json!([{"event": event, "state": {"n": 0}}]));
        assert_eq!(rules(&r), [rule], "{event}");
        assert!(r.failures[0].span.is_some(), "{event} is located");
        assert_eq!(r.result, "fail");
    }
}

const FAULTY2: &str = r#"
component Faulty2 {
    state n: Int = 0;
    state s: String = "";
    on NoArm { n = match 5 { Ok(v) => { 1 } }; }
    on Unknown { n = zzz; }
    on Exp { s = "x" + 1000000000000000000000.0; }
    on Call { n = nope(1); }
    on Await { n = await(5); }
    view { Text { "{n}" } }
}
"#;

#[test]
fn the_remaining_runtime_refusals_have_rules_and_spans() {
    for (event, rule) in [
        ("NoArm", "replay-no-match-arm"),
        ("Unknown", "replay-unknown-identifier"),
        ("Exp", "replay-number-format"),
        ("Call", "replay-unsupported"),
        ("Await", "replay-unsupported"),
    ] {
        let r = run(FAULTY2, json!([{"event": event, "state": {"n": 0}}]));
        assert_eq!(rules(&r), [rule], "{event}: {:?}", r.failures);
        assert!(r.failures[0].span.is_some(), "{event} is located");
    }
}

#[test]
fn a_state_initial_value_that_javascript_leaves_undefined_is_refused() {
    let src = "component Init { state a: Int = 1; state b: Int = a; on Go { a = 2; } view { Text { \"{b}\" } } }";
    let r = run(src, json!([{"event": "Go", "state": {"a": 2}}]));
    assert_eq!(rules(&r), ["replay-unsupported"]);
    assert!(r.failures[0].span.is_some());
}

// ------------------------------------------------ async: scripted network (Q4)

fn submit(query: &str) -> Json {
    json!([
        {"event": "Change", "params": [query]},
        {"event": "Submit", "expect": {"outcome": "suspended"}}
    ])
}

#[test]
fn the_stale_response_is_dropped_and_the_state_is_the_newer_requests() {
    let r = replay(
        &ir(SEARCH),
        &std::fs::read_to_string(root().join("examples/search/stale.trace.json")).expect("trace"),
        "search.cui",
        "stale.trace.json",
    );
    assert_pass(&r);
    let last = r.steps.last().expect("steps");
    assert_eq!(last.outcome, "dropped");
    assert_eq!(last.fired[0].delivered, "dropped");
    assert_eq!(
        r.final_state["results"],
        json!(["Ada Lovelace", "Ada Marrero"])
    );
}

#[test]
fn the_same_requests_answered_in_the_other_order_also_leave_the_newer_data() {
    // Request 1 is superseded the moment request 2 starts, so it is dropped
    // whenever it answers: before or after the newer one.
    let mut steps = submit("a").as_array().expect("array").clone();
    steps.extend(submit("ad").as_array().expect("array").clone());
    steps.push(json!({"resolve": 1, "with": {"Ok": ["old"]},
        "state": {"status": "loading", "results": []},
        "expect": {"outcome": "dropped", "pending": [2]}}));
    steps.push(json!({"resolve": 2, "with": {"Ok": ["new"]},
        "state": {"status": "done", "results": ["new"]}}));
    assert_pass(&run(SEARCH, Json::Array(steps)));
}

#[test]
fn without_a_newer_request_a_late_answer_is_delivered() {
    let mut steps = submit("a").as_array().expect("array").clone();
    steps.push(json!({"resolve": 1, "with": {"Ok": ["only"]},
        "state": {"status": "done", "results": ["only"]},
        "expect": {"outcome": "completed", "pending": []}}));
    assert_pass(&run(SEARCH, Json::Array(steps)));
}

#[test]
fn an_err_answer_reaches_the_err_arm() {
    let mut steps = submit("a").as_array().expect("array").clone();
    steps.push(json!({"resolve": 1, "with": {"Err": "boom"},
        "state": {"status": "error: search failed", "results": [], "r": {"Err": "boom"}}}));
    assert_pass(&run(SEARCH, Json::Array(steps)));
}

#[test]
fn dispose_delivers_cancelled_when_the_request_completes_not_before() {
    let mut steps = submit("a").as_array().expect("array").clone();
    steps.push(json!({"dispose": true, "state": {"status": "loading"}}));
    steps.push(json!({"resolve": 1, "with": {"Ok": ["ignored"]},
        "state": {"status": "cancelled", "results": [], "r": {"Cancelled": null}},
        "expect": {"fired": [{"request": 1, "delivered": "Cancelled", "outcome": "completed"}]}}));
    assert_pass(&run(SEARCH, Json::Array(steps)));
}

#[test]
fn a_superseded_request_that_is_then_disposed_is_dropped_not_cancelled() {
    let mut steps = submit("a").as_array().expect("array").clone();
    steps.extend(submit("ab").as_array().expect("array").clone());
    steps.push(json!({"dispose": true}));
    steps.push(json!({"resolve": 1, "with": {"Ok": ["x"]},
        "expect": {"fired": [{"request": 1, "delivered": "dropped"}]}}));
    steps.push(json!({"resolve": 2, "with": {"Ok": ["y"]},
        "state": {"status": "cancelled"},
        "expect": {"fired": [{"request": 2, "delivered": "Cancelled"}]}}));
    assert_pass(&run(SEARCH, Json::Array(steps)));
}

// ------------------------------------------------- async: the fake clock

const TIMERS: &str = r#"
component Timers {
    requires Clock;
    state t: Int = 0;
    state u: Int = 0;
    state r: Result[Int, String] = Ok(0);
    state q: Result[Int, String] = Ok(0);
    state log: String = "";
    on Slow {
        r = await(sleep(100));
        t = clock.now();
        log = log + "s";
    }
    on Fast {
        q = await(sleep(10));
        u = clock.now();
        log = log + "f";
    }
    on Again {
        r = await(sleep(10));
        r = await(sleep(10));
        t = clock.now();
    }
    view { Text { "{t}" } }
}
"#;

#[test]
fn a_timer_reads_its_own_due_time_not_the_end_of_the_advance() {
    let r = run(
        TIMERS,
        json!([
            {"event": "Slow", "expect": {"issued": [{"request": 1, "effect": "sleep", "due": 100}]}},
            {"advance": 99, "state": {"t": 0}, "expect": {"fired": [], "pending": [1]}},
            {"advance": 901,
             "state": {"t": 100, "log": "s"},
             "expect": {"fired": [{"request": 1, "delivered": "Ok", "outcome": "completed"}], "pending": []}}
        ]),
    );
    assert_pass(&r);
    assert_eq!(r.steps[2].clock, json!(1000));
}

#[test]
fn sleeps_in_one_component_share_a_group_so_the_newer_supersedes_the_older() {
    // Slow (due 100) then Fast (due 10): both are `sleep`, so Fast supersedes
    // Slow. On advance Fast fires first and completes; Slow fires and is
    // dropped, so `t` is never written.
    let r = run(
        TIMERS,
        json!([
            {"event": "Slow"},
            {"event": "Fast", "expect": {"pending": [1, 2]}},
            {"advance": 1000,
             "state": {"t": 0, "u": 10, "log": "f"},
             "expect": {"fired": [{"request": 2, "delivered": "Ok", "outcome": "completed"},
                                  {"request": 1, "delivered": "dropped"}]}}
        ]),
    );
    assert_pass(&r);
}

#[test]
fn a_sleep_started_by_a_resumed_handler_inside_the_same_advance_fires_too() {
    let r = run(
        TIMERS,
        json!([
            {"event": "Again"},
            {"advance": 25, "state": {"t": 20}, "expect": {"pending": []}}
        ]),
    );
    assert_pass(&r);
}

#[test]
fn the_clock_starts_where_the_header_says_and_only_advance_moves_it() {
    let src = "component C { requires Clock; state t: Int = 0; on Read { t = clock.now(); } view { Text { \"{t}\" } } }";
    let t = json!({"schema": "cancho-ui-trace", "version": "0.1.0", "clock": 5000, "steps": [
        {"event": "Read", "state": {"t": 5000}},
        {"event": "Read", "state": {"t": 5000}},
        {"advance": 250},
        {"event": "Read", "state": {"t": 5250}}
    ]});
    assert_pass(&replay(&ir(src), &t.to_string(), "p.cui", "t.json"));
}

// -------------------------------------------- the journal (await continuation)

const JOURNAL: &str = r#"
component Journal {
    requires Network;
    requires Clock;
    state a: Int = 1;
    state out: Int = 0;
    state r: Result[List[String], String] = Ok([]);
    on Bump { a = a + 1; }
    on Go {
        out = a * 100 + match await(net.search("q")) {
            Ok(l) => { len(l) }
            Err(_) => { 0 }
            Cancelled => { 0 }
        };
    }
    on Stamp {
        out = clock.now() * 1000 + match await(net.search("q")) {
            Ok(l) => { len(l) }
            Err(_) => { 0 }
            Cancelled => { 0 }
        };
    }
    view { Text { "{out}" } }
}
"#;

#[test]
fn clock_read_before_an_await_keeps_the_instant_it_had() {
    let t = json!({"schema": "cancho-ui-trace", "version": "0.1.0", "clock": 5, "steps": [
        {"event": "Stamp", "expect": {"outcome": "suspended"}},
        {"advance": 7},
        {"resolve": 1, "with": {"Ok": ["x"]}, "state": {"out": 5001}}
    ]});
    assert_pass(&replay(&ir(JOURNAL), &t.to_string(), "p.cui", "t.json"));
}

#[test]
fn state_read_before_an_await_keeps_the_value_it_had() {
    // JavaScript evaluates `a * 100` before it awaits: 1 * 100, even though
    // `a` is 2 by the time the answer arrives.
    let r = run(
        JOURNAL,
        json!([
            {"event": "Go", "expect": {"outcome": "suspended"}},
            {"event": "Bump", "state": {"a": 2}},
            {"resolve": 1, "with": {"Ok": ["x", "y"]}, "state": {"out": 102}}
        ]),
    );
    assert_pass(&r);
}

// ------------------------------------------------------------ trace errors

fn trace_rule(body: Json, src: &str) -> (&'static str, String) {
    let t = json!({"schema": "cancho-ui-trace", "version": "0.1.0"})
        .as_object()
        .expect("object")
        .iter()
        .chain(body.as_object().expect("object").iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<serde_json::Map<_, _>>();
    let r = replay(&ir(src), &Json::Object(t).to_string(), "p.cui", "t.json");
    assert_eq!(r.failures.len(), 1, "{:#?}", r.failures);
    (r.failures[0].rule, r.failures[0].path.clone())
}

#[test]
fn every_trace_error_has_its_rule_and_path() {
    let ok_step = json!({"event": "Go", "state": {"a": 2}});
    let cases: Vec<(Json, &str, &str)> = vec![
        (json!({"steps": []}), "trace-malformed", "steps"),
        (json!({}), "trace-malformed", "steps"),
        (
            json!({"steps": [ok_step], "bogus": 1}),
            "trace-unknown-key",
            "bogus",
        ),
        (
            json!({"steps": [{"event": "Go", "state": {"a": 1}, "bogus": 1}]}),
            "trace-unknown-key",
            "steps[0].bogus",
        ),
        (
            json!({"steps": [{"event": "Go", "expect": {"bogus": 1}}]}),
            "trace-unknown-key",
            "steps[0].expect.bogus",
        ),
        (
            json!({"steps": [{"state": {"a": 1}}]}),
            "trace-bad-step",
            "steps[0]",
        ),
        (
            json!({"steps": [{"event": "Go", "advance": 1, "state": {"a": 1}}]}),
            "trace-bad-step",
            "steps[0]",
        ),
        (
            json!({"steps": [{"advance": 1, "params": [], "state": {"a": 1}}]}),
            "trace-bad-step",
            "steps[0].params",
        ),
        (
            json!({"steps": [{"event": "Go", "params": [{"x": 1, "y": 2}], "state": {"a": 1}}]}),
            "trace-bad-value",
            "steps[0].params[0]",
        ),
        (
            json!({"steps": [{"advance": -1, "state": {"a": 1}}]}),
            "trace-bad-value",
            "steps[0].advance",
        ),
        (
            json!({"clock": -5, "steps": [ok_step]}),
            "trace-bad-value",
            "clock",
        ),
        (
            json!({"component": "Nope", "steps": [ok_step]}),
            "trace-unknown-component",
            "component",
        ),
        (
            json!({"steps": [{"event": "Nope", "state": {"a": 1}}]}),
            "trace-unknown-event",
            "steps[0].event",
        ),
        (
            json!({"steps": [{"event": "Go", "params": [1], "state": {"a": 1}}]}),
            "trace-params-arity",
            "steps[0].params",
        ),
        (
            json!({"steps": [{"event": "Go", "state": {"zzz": 1}}]}),
            "trace-unknown-state-field",
            "steps[0].state.zzz",
        ),
        (
            json!({"steps": [{"resolve": 1, "with": {"Ok": 1}, "state": {"a": 1}}]}),
            "trace-unknown-request",
            "steps[0].resolve",
        ),
        (
            json!({"steps": [{"event": "Go"}]}),
            "trace-no-assertions",
            "steps",
        ),
        (
            json!({"steps": [{"resolve": 1, "with": {"Cancelled": null}, "state": {"a": 1}}]}),
            "trace-bad-response",
            "steps[0].with",
        ),
        (
            json!({"steps": [{"resolve": 1, "state": {"a": 1}}]}),
            "trace-bad-response",
            "steps[0].with",
        ),
        (
            json!({"steps": [{"dispose": 1, "state": {"a": 1}}]}),
            "trace-bad-step",
            "steps[0].dispose",
        ),
    ];
    let src = "component P { state a: Int = 1; on Go { a = 2; } view { Text { \"{a}\" } } }";
    for (body, rule, path) in cases {
        let got = trace_rule(body.clone(), src);
        assert_eq!(got, (rule, path.to_string()), "{body}");
    }
}

#[test]
fn schema_and_version_must_match() {
    let r = replay(
        &ir(SEQ),
        r#"{"schema":"cancho-ui-trace","version":"9.0.0","steps":[{"event":"Go","state":{"a":2}}]}"#,
        "p.cui",
        "t.json",
    );
    assert_eq!(rules(&r), ["trace-schema-unsupported"]);
    let r = replay(
        &ir(SEQ),
        r#"{"version":"0.1.0","steps":[]}"#,
        "p.cui",
        "t.json",
    );
    assert_eq!(rules(&r), ["trace-schema-unsupported"]);
}

#[test]
fn json_syntax_errors_carry_the_trace_line_and_column() {
    let r = replay(&ir(SEQ), "{\n  \"schema\": ,\n}", "p.cui", "t.json");
    assert_eq!(rules(&r), ["trace-malformed"]);
    assert_eq!(r.failures[0].line, Some(2));
    assert!(r.failures[0].column.is_some());
    for text in ["", "[]", "null", "42", "\"x\"", "{"] {
        let r = replay(&ir(SEQ), text, "p.cui", "t.json");
        assert_eq!(rules(&r), ["trace-malformed"], "{text:?}");
    }
}

#[test]
fn request_errors_name_the_request() {
    let src = TIMERS;
    let t = |steps: Json| run(src, steps);
    // Resolving a sleep: timers belong to the clock.
    let r = t(json!([{"event": "Slow"}, {"resolve": 1, "with": {"Ok": null}, "state": {"t": 0}}]));
    assert_eq!(rules(&r), ["trace-request-not-scriptable"]);
    // Resolving twice.
    let mut steps = submit("a").as_array().expect("array").clone();
    steps.push(json!({"resolve": 1, "with": {"Ok": []}, "state": {"status": "done"}}));
    steps.push(json!({"resolve": 1, "with": {"Ok": []}, "state": {"status": "done"}}));
    let r = run(SEARCH, Json::Array(steps));
    assert_eq!(rules(&r), ["trace-request-not-pending"]);
    // An event after dispose.
    let r = run(
        SEARCH,
        json!([{"dispose": true, "state": {"query": ""}}, {"event": "ShowAll", "state": {"min": 0}}]),
    );
    assert_eq!(rules(&r), ["trace-event-after-dispose"]);
}

#[test]
fn a_trace_error_stops_the_replay_and_reports_the_step() {
    let r = run(
        SEQ,
        json!([
            {"event": "Go", "state": {"a": 2}},
            {"event": "Nope", "state": {"a": 2}},
            {"event": "Go", "state": {"a": 3}}
        ]),
    );
    assert_eq!(rules(&r), ["trace-unknown-event"]);
    assert_eq!(r.failures[0].step, Some(1));
    assert_eq!(r.steps_run, 1);
    assert_eq!(r.steps_total, 3);
}

#[test]
fn the_report_always_says_what_it_does_not_claim() {
    let r = run(SEQ, json!([{"event": "Go", "state": {"a": 2}}]));
    for k in [
        "view_rendering",
        "timing_and_latency",
        "emitted_javascript_behaviour",
    ] {
        assert!(r.not_claimed.contains(&k), "{k}");
    }
}

// ------------------------------------------------------------ the CLI

fn example_traces() -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    for dir in std::fs::read_dir(root().join("examples")).expect("examples") {
        let dir = dir.expect("entry").path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        for f in std::fs::read_dir(&dir).expect("example dir") {
            let f = f.expect("entry").path();
            if f.to_string_lossy().ends_with(".trace.json") {
                out.push((dir.join(format!("{name}.cui")), f));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_example_trace_passes_through_the_binary() {
    let traces = example_traces();
    assert!(traces.len() >= 9, "only {} example traces", traces.len());
    for (cui, tr) in traces {
        let rel = |p: &Path| {
            p.strip_prefix(root().join("examples"))
                .expect("under examples")
                .to_string_lossy()
                .into_owned()
        };
        let cui_arg = format!("examples/{}", rel(&cui));
        let tr_arg = format!("examples/{}", rel(&tr));
        let (code, stdout, stderr) = cli(&["test", &cui_arg, &tr_arg, "--output", "json"]);
        assert_eq!(code, 0, "{tr_arg}\n{stdout}\n{stderr}");
        let v: Json = serde_json::from_str(&stdout).expect("JSON report");
        assert_eq!(v["result"], "pass");
        assert!(v["checks"].as_u64().expect("checks") > 0);
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cancho-ui-replay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn exit_codes_0_1_2_3_4() {
    let dir = scratch("exit");
    let cui = "examples/counter/counter.cui";
    let good = "examples/counter/counter.trace.json";
    assert_eq!(cli(&["test", cui, good]).0, 0);

    // 4: a state expectation that does not hold.
    let bad = dir.join("bad.json");
    std::fs::write(
        &bad,
        trace(json!([{"event": "Increment", "state": {"count": 7}}])),
    )
    .expect("write");
    let (code, stdout, _) = cli(&["test", cui, bad.to_str().expect("utf8")]);
    assert_eq!(code, 4, "{stdout}");
    assert!(stdout.contains("test-state-mismatch"), "{stdout}");
    assert!(stdout.contains("expected 7"), "{stdout}");
    assert!(
        stdout.contains("examples/counter/counter.cui:"),
        "located: {stdout}"
    );

    // 4: a trace that is not JSON.
    let junk = dir.join("junk.json");
    std::fs::write(&junk, "not json").expect("write");
    assert_eq!(cli(&["test", cui, junk.to_str().expect("utf8")]).0, 4);

    // 4: a trace that is not UTF-8 is malformed, not an environment error.
    let bin = dir.join("bin.json");
    std::fs::write(&bin, [0xff, 0xfe, 0x00]).expect("write");
    assert_eq!(cli(&["test", cui, bin.to_str().expect("utf8")]).0, 4);

    // 1: the program does not compile.
    let broken = dir.join("broken.cui");
    std::fs::write(&broken, "component {").expect("write");
    let (code, _, stderr) = cli(&["test", broken.to_str().expect("utf8"), good]);
    assert_eq!(code, 1);
    assert!(stderr.contains("error["), "{stderr}");

    // 2: command line wrong.
    assert_eq!(cli(&["test"]).0, 2);
    assert_eq!(cli(&["test", cui]).0, 2);
    assert_eq!(cli(&["test", cui, good, good]).0, 2);
    assert_eq!(cli(&["test", cui, good, "--bogus"]).0, 2);
    assert_eq!(cli(&["test", cui, good, "--output"]).0, 2);
    assert_eq!(cli(&["test", cui, good, "--output", "xml"]).0, 2);

    // 3: a file is missing.
    assert_eq!(cli(&["test", cui, "no/such/trace.json"]).0, 3);
    assert_eq!(cli(&["test", "no/such/file.cui", good]).0, 3);
}

#[test]
fn json_output_is_one_object_with_a_located_failure() {
    let dir = scratch("json");
    let bad = dir.join("bad.json");
    std::fs::write(
        &bad,
        trace(json!([{"event": "Increment", "state": {"count": 7}}])),
    )
    .expect("write");
    let (code, stdout, _) = cli(&[
        "test",
        "examples/counter/counter.cui",
        bad.to_str().expect("utf8"),
        "--output",
        "json",
    ]);
    assert_eq!(code, 4);
    let v: Json = serde_json::from_str(&stdout).expect("stdout is one JSON object");
    assert_eq!(v["schema"], "cancho-ui-test");
    assert_eq!(v["result"], "fail");
    let f = &v["failures"][0];
    assert_eq!(f["rule"], "test-state-mismatch");
    assert_eq!(f["tier"], "tested");
    assert_eq!(f["step"], 0);
    assert_eq!(f["path"], "steps[0].state.count");
    assert_eq!(f["expected"], 7);
    assert_eq!(f["actual"], 1);
    assert_eq!(f["span"]["file"], "examples/counter/counter.cui");
    assert_eq!(
        f["span"]["start"]["line"], 9,
        "the `count = count + 1` assignment"
    );
    assert!(v["not_claimed"].as_array().expect("array").len() >= 5);
}

#[test]
fn test_is_in_help_introspect_and_skill() {
    let (_, help, _) = cli(&["--help"]);
    assert!(
        help.contains("cancho-ui test <file.cui> <trace.json>"),
        "{help}"
    );
    let (code, out, _) = cli(&["introspect", "--output", "json"]);
    assert_eq!(code, 0);
    let v: Json = serde_json::from_str(&out).expect("JSON");
    let test = v["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .find(|c| c["name"] == "test")
        .expect("test is listed");
    assert_eq!(
        test["exit_codes"]["4"]
            .as_str()
            .map(|s| s.starts_with("a test failed")),
        Some(true)
    );
    assert_eq!(v["exit_codes"]["4"], "test only: a test failed");
    let (_, skill, _) = cli(&["skill"]);
    assert!(skill.contains("cancho-ui test <file.cui> <trace.json>"));
}
