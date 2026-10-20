//! `start` and `every` (`docs/schedules.md`): what parses, what is refused, what is emitted, how replay moves the fake
//! clock through them, and that the diff sees a change of rate. The browser side (a tick per period, a run in flight, a
//! disposed component) is `scripts/check-schedules.js` and `scripts/check-plant.js`, outside the gate.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-schedules-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A counter with the given schedule lines.
fn counter(schedules: &str) -> String {
    format!(
        "component Counter {{\n    state n: Int = 0;\n{schedules}\n    on Tick {{ n = n + 1; }}\n    on Bump(by) {{ n = n + by; }}\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n"
    )
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

fn accepted(src: &str, name: &str) {
    let file = scratch(name, src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

fn replay(src: &str, trace: &str) -> (i32, String) {
    let file = scratch("r.cui", src);
    let trace = scratch("r.trace.json", trace);
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    (code, format!("{stdout}{stderr}"))
}

#[test]
fn both_forms_parse_and_reach_the_ir_in_the_order_written() {
    let src = counter("    every 2000 Tick;\n    start Tick;\n    every 100 Tick2;\n")
        .replace("    on Bump", "    on Tick2 { n = n + 2; }\n    on Bump");
    let file = scratch("two.cui", &src);
    let (code, stdout, stderr) = run(&["inspect", "ir", p(&file)]);
    assert_eq!(code, 0, "{stderr}");
    let ir: Value = serde_json::from_str(&stdout).expect("the IR is JSON");
    let s = &ir["components"][0]["schedules"];
    assert_eq!(s[0]["kind"], "every");
    assert_eq!(s[0]["event"], "Tick");
    assert_eq!(s[0]["period_ms"], 2000);
    assert_eq!(s[1]["kind"], "start");
    assert!(s[1].get("period_ms").is_none(), "{s}");
    assert_eq!(s[2]["period_ms"], 100);
    // and `inspect component` shows them
    let (code, stdout, _) = run(&["inspect", "component", p(&file), "Counter"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"schedules\""), "{stdout}");
}

#[test]
fn start_and_every_are_ordinary_words_everywhere_but_the_start_of_a_member() {
    let src = "component W {\n    state start: Int = 1;\n    state every: Int = 2;\n    on Go { start = every + start; }\n    view { Text { \"{start}\" } }\n}\n";
    accepted(src, "words.cui");
}

#[test]
fn a_component_without_schedules_has_an_empty_list_and_no_timer() {
    let src = counter("");
    let file = scratch("none.cui", &src);
    let (_, stdout, _) = run(&["inspect", "ir", p(&file)]);
    let ir: Value = serde_json::from_str(&stdout).expect("the IR is JSON");
    assert_eq!(ir["components"][0]["schedules"], serde_json::json!([]));
    let out = scratch("none.js", "");
    let (code, _, _) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0);
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(
        !js.contains("setInterval")
            && !js.contains("__cui_every")
            && !js.contains("visibilitychange")
    );
}

#[test]
fn each_refusal_has_its_code_and_a_remediation() {
    assert_eq!(
        codes(&counter("    start Nope;"), "unknown.cui"),
        ["schedule-event-unknown"]
    );
    assert_eq!(
        codes(&counter("    every 1000 Nope;"), "unknown2.cui"),
        ["schedule-event-unknown"]
    );
    assert_eq!(
        codes(&counter("    start Bump;"), "params.cui"),
        ["schedule-event-params"]
    );
    assert_eq!(
        codes(&counter("    every 1000 Bump;"), "params2.cui"),
        ["schedule-event-params"]
    );
    for period in ["0", "99", "86400001", "1000000000000"] {
        assert_eq!(
            codes(&counter(&format!("    every {period} Tick;")), "period.cui"),
            ["schedule-period-invalid"],
            "every {period}"
        );
    }
    assert_eq!(
        codes(&counter("    start Tick;\n    start Tick;"), "dup.cui"),
        ["schedule-duplicate"]
    );
    assert_eq!(
        codes(
            &counter("    every 1000 Tick;\n    every 2000 Tick;"),
            "dup2.cui"
        ),
        ["schedule-duplicate"],
        "two periods for one event"
    );
    let file = scratch("rem.cui", &counter("    every 50 Tick;"));
    let (_, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    assert!(
        diags[0]["remediation"]["description"]
            .as_str()
            .is_some_and(|d| d.contains("100 to 86400000")),
        "{stdout}"
    );
}

#[test]
fn the_edges_of_the_period_are_accepted_and_a_start_and_an_every_may_share_an_event() {
    accepted(&counter("    every 100 Tick;"), "min.cui");
    accepted(&counter("    every 86400000 Tick;"), "max.cui");
    accepted(
        &counter("    start Tick;\n    every 1000 Tick;"),
        "both.cui",
    );
}

#[test]
fn a_period_that_is_not_an_integer_literal_is_a_syntax_error() {
    for line in [
        "    every 1.5 Tick;",
        "    every -5 Tick;",
        "    every Tick;",
        "    every 1000;",
        "    start;",
        "    every 1000 Tick",
    ] {
        let codes = codes(&counter(line), "syntax.cui");
        assert!(
            codes.iter().all(|c| c.starts_with("parse-")),
            "{line}: {codes:?}"
        );
    }
}

#[test]
fn the_emitted_page_starts_after_the_first_render_ticks_with_a_guard_and_clears_on_disposal() {
    let src = counter("    start Tick;\n    every 1500 Tick;");
    let file = scratch("emit.cui", &src);
    let out = scratch("emit.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(out).expect("the built program");
    assert!(js.contains("const __cui_busy = { Tick: 0 };"), "{js}");
    assert!(js.contains("__cui_busy.Tick++;") && js.contains("__cui_busy.Tick--;"));
    let render = js
        .find("        __render();\n        __on_Tick();")
        .expect("start after the first render");
    let timer = js
        .find("__cui_every.push({ ms: 1500, next: Date.now() + 1500, o: null, run: function () { if (__cui_busy.Tick === 0) { __on_Tick(); } } });")
        .expect("the guarded timer");
    assert!(render < timer, "start before the timer is armed");
    assert!(js.contains("__cui_every.forEach(function (s) { if (s.o !== null) { clearTimeout(s.o); s.o = null; } });"));
    // section 11: the page's visibility is listened to, and the listeners go with the component.
    assert!(js.contains("document.addEventListener('visibilitychange', __cui_visibility);"));
    assert!(js.contains("window.addEventListener('pagehide', __cui_pagehide);"));
    assert!(js.contains("document.removeEventListener('visibilitychange', __cui_visibility);"));
    assert!(
        js.contains("__cui_dispose"),
        "a scheduled component can be disposed"
    );
}

#[test]
fn replay_runs_start_before_the_first_step_and_reports_it_there() {
    let (code, out) = replay(
        &counter("    start Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Bump","params":[10],"state":{"n":11}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn replay_fires_ticks_in_time_order_and_reports_them() {
    let src = counter("    every 1000 Tick;\n    every 1500 Tick2;")
        .replace("    on Bump", "    on Tick2 { n = n + 100; }\n    on Bump");
    let (code, out) = replay(
        &src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":999,"state":{"n":0},"expect":{"ticks":[]}},
            {"advance":1,"state":{"n":1},"expect":{"ticks":[{"event":"Tick","at":1000,"ran":true}]}},
            {"advance":500,"state":{"n":101},"expect":{"ticks":[{"event":"Tick2","at":1500,"ran":true}]}},
            {"advance":1500,"state":{"n":203},"expect":{"ticks":[{"event":"Tick","at":2000},{"event":"Tick","at":3000},{"event":"Tick2","at":3000}]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn ticks_due_together_run_in_the_order_the_every_lines_are_written() {
    let src = counter("    every 1000 Tick2;\n    every 1000 Tick;")
        .replace("    on Bump", "    on Tick2 { n = n * 10; }\n    on Bump");
    // Tick2 first (n = 0 * 10 = 0), then Tick (n = 1): 1. The other order would be (0 + 1) * 10 = 10.
    let (code, out) = replay(
        &src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":1000,"state":{"n":1},"expect":{"ticks":[{"event":"Tick2"},{"event":"Tick"}]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_tick_while_a_run_is_in_flight_is_skipped_not_started_on_top() {
    let src = "component Poll {\n    requires Network;\n    state r: Result[String, String] = Ok(\"\");\n    state n: Int = 0;\n    every 1000 Refresh;\n    on Refresh { r = await(net.get(\"x\")); n = n + 1; }\n    view { Text { \"{n}\" } }\n}\n";
    let (code, out) = replay(
        src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":1000,"expect":{"ticks":[{"at":1000,"ran":true}],"issued":[{"request":1}],"pending":[1]}},
            {"advance":2000,"expect":{"ticks":[{"at":2000,"ran":false},{"at":3000,"ran":false}],"issued":[],"pending":[1]}},
            {"resolve":1,"with":{"Ok":"a"},"state":{"n":1}},
            {"advance":1000,"expect":{"ticks":[{"at":4000,"ran":true}],"issued":[{"request":2}],"pending":[2]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_run_started_by_an_event_counts_as_in_flight_too() {
    let src = "component Poll {\n    requires Network;\n    state r: Result[String, String] = Ok(\"\");\n    every 1000 Refresh;\n    on Refresh { r = await(net.get(\"x\")); }\n    view { Text { \"x\" } }\n}\n";
    let (code, out) = replay(
        src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Refresh","expect":{"pending":[1]}},
            {"advance":1000,"expect":{"ticks":[{"at":1000,"ran":false}],"pending":[1]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_tick_runs_before_a_sleep_due_at_the_same_instant() {
    let src = "component T {\n    state log: String = \"\";\n    every 1000 Mark;\n    on Mark { log = log + \"t\"; }\n    on Nap { log = log + \"n\"; r = await(sleep(1000)); log = log + \"s\"; }\n    state r: Result[Int, String] = Ok(0);\n    view { Text { log } }\n}\n";
    let (code, out) = replay(
        src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Nap","state":{"log":"n"}},
            {"advance":1000,"state":{"log":"nts"}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn dispose_stops_the_ticks_and_the_header_clock_sets_the_first_one() {
    let (code, out) = replay(
        &counter("    every 1000 Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","clock":5000,"steps":[
            {"advance":999,"state":{"n":0},"expect":{"ticks":[]}},
            {"advance":1,"state":{"n":1},"expect":{"ticks":[{"at":6000}]}},
            {"dispose":true},
            {"advance":10000,"state":{"n":1},"expect":{"ticks":[]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_trace_that_expects_a_tick_that_does_not_come_fails_with_the_ticks_in_the_report() {
    let (code, out) = replay(
        &counter("    every 1000 Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":999,"expect":{"ticks":[{"event":"Tick"}]}}]}"#,
    );
    assert_eq!(code, 4, "{out}");
    assert!(out.contains("test-expect-mismatch"), "{out}");
}

#[test]
fn the_diff_reports_a_schedule_added_removed_or_changed_in_period() {
    let base = scratch("base.cui", &counter("    every 5000 Tick;"));
    let fast = scratch("fast.cui", &counter("    every 100 Tick;"));
    let same = scratch("same.cui", &counter("    every 5000 Tick;"));
    let none = scratch("nosched.cui", &counter(""));
    let (code, stdout, _) = run(&["diff", p(&base), p(&same)]);
    assert_eq!(code, 0, "{stdout}");
    let (code, stdout, _) = run(&["diff", p(&base), p(&fast), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    let d: Value = serde_json::from_str(&stdout).expect("the diff is JSON");
    let c = &d["components"]["changed"][0];
    assert_eq!(
        c["schedules"]["added"],
        serde_json::json!(["every 100 Tick"])
    );
    assert_eq!(
        c["schedules"]["removed"],
        serde_json::json!(["every 5000 Tick"])
    );
    assert_eq!(c["authority"], "unchanged", "a rate is not authority");
    let (_, text, _) = run(&["diff", p(&base), p(&fast)]);
    assert!(
        text.contains("+ schedule every 100 Tick") && text.contains("- schedule every 5000 Tick"),
        "{text}"
    );
    let (code, stdout, _) = run(&["diff", p(&none), p(&base), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    let d: Value = serde_json::from_str(&stdout).expect("the diff is JSON");
    assert_eq!(
        d["components"]["changed"][0]["schedules"]["added"],
        serde_json::json!(["every 5000 Tick"])
    );
}

#[test]
fn an_advance_that_would_fire_too_many_ticks_is_refused_with_a_rule_tag() {
    let (code, out) = replay(
        &counter("    every 100 Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":1000000000000,"state":{"n":0}}]}"#,
    );
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("replay-tick-limit"), "{out}");
}

// ---- hidden pages (docs/schedules.md section 11) -------------------------------------------------------------------

#[test]
fn a_hidden_page_runs_no_tick_and_one_catch_up_tick_when_it_is_shown() {
    let (code, out) = replay(
        &counter("    every 1000 Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"hide":true},
            {"advance":3500,"state":{"n":0},"expect":{"ticks":[]}},
            {"show":true,"state":{"n":1},"expect":{"ticks":[{"event":"Tick","at":3500,"ran":true,"catch_up":true}]}},
            {"advance":999,"state":{"n":1},"expect":{"ticks":[]}},
            {"advance":1,"state":{"n":2},"expect":{"ticks":[{"event":"Tick","at":4500,"ran":true}]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
    // the human report names the two steps for what they are
    assert!(
        out.contains("step 0  hide") && out.contains("step 2  show"),
        "{out}"
    );
}

#[test]
fn a_schedule_that_was_not_due_keeps_the_time_it_had_left() {
    let (code, out) = replay(
        &counter("    every 1000 Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":600,"state":{"n":0}},
            {"hide":true},
            {"advance":100,"state":{"n":0}},
            {"show":true,"state":{"n":0},"expect":{"ticks":[]}},
            {"advance":299,"state":{"n":0}},
            {"advance":1,"state":{"n":1},"expect":{"ticks":[{"event":"Tick","at":1000}]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_catch_up_tick_is_skipped_while_a_run_is_in_flight() {
    let src = "component Poll {\n    requires Network;\n    state r: Result[String, String] = Ok(\"\");\n    every 1000 Refresh;\n    on Refresh { r = await(net.get(\"x\")); }\n    view { Text { \"x\" } }\n}\n";
    let (code, out) = replay(
        src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"advance":1000,"expect":{"pending":[1]}},
            {"hide":true},
            {"advance":5000},
            {"show":true,"expect":{"ticks":[{"at":6000,"ran":false,"catch_up":true}],"pending":[1]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_request_in_flight_completes_and_a_sleep_fires_while_the_page_is_hidden() {
    let src = "component T {\n    state log: String = \"\";\n    every 1000 Mark;\n    on Mark { log = log + \"t\"; }\n    on Nap { r = await(sleep(1500)); log = log + \"s\"; }\n    state r: Result[Int, String] = Ok(0);\n    view { Text { log } }\n}\n";
    let (code, out) = replay(
        src,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Nap","state":{"log":""}},
            {"hide":true},
            {"advance":2000,"state":{"log":"s"}},
            {"show":true,"state":{"log":"st"}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}

#[test]
fn hiding_a_hidden_page_or_showing_a_visible_one_is_a_bad_step() {
    for trace in [
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[{"hide":true,"state":{"n":0}},{"hide":true,"state":{"n":0}}]}"#,
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[{"show":true,"state":{"n":0}}]}"#,
    ] {
        let (code, out) = replay(&counter("    every 1000 Tick;"), trace);
        assert_ne!(code, 0, "{out}");
        assert!(out.contains("trace-bad-step"), "{out}");
    }
}

#[test]
fn showing_a_disposed_component_runs_no_tick() {
    let (code, out) = replay(
        &counter("    every 1000 Tick;"),
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"hide":true},
            {"advance":5000},
            {"dispose":true},
            {"show":true,"state":{"n":0},"expect":{"ticks":[]}}]}"#,
    );
    assert_eq!(code, 0, "{out}");
}
