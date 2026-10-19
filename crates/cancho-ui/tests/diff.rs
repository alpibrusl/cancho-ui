//! `cancho-ui diff` (docs/semantic-diff.md). The engine tests run the
//! real front end on source strings and inspect the report as JSON; the
//! CLI tests run the binary for flags, exit codes and output channels.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_diff::{Authority, DiffReport, diff_ir, render_human};
use cancho_ui_ir::IrFile;
use cancho_ui_ir::lower_file;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::Value;

const CLOCK: &str = include_str!("../../../examples/clock/clock.cui");
const COUNTER: &str = include_str!("../../../examples/counter/counter.cui");
const SEARCH: &str = include_str!("../../../examples/search/search.cui");

fn ir(src: &str) -> IrFile {
    let ast = parse(lex(src).expect("lex")).expect("parse");
    check_file(&ast).expect("check");
    lower_file(&ast).expect("lower")
}

fn diff(base: &str, cand: &str) -> DiffReport {
    diff_ir(&ir(base), &ir(cand), "base", "cand")
}

fn json(base: &str, cand: &str) -> Value {
    serde_json::to_value(diff(base, cand)).expect("report serializes")
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_str().expect("string").to_string())
        .collect()
}

/// The clock example with every use of `Clock` removed: it must still
/// compile, so `requires Clock;` goes together with `clock.now()`.
fn clock_without_capability() -> String {
    CLOCK
        .replace("    requires Clock;\n", "")
        .replace("started_at = clock.now();", "started_at = 1;")
        .replace(
            "\"Elapsed: {(clock.now() - started_at) / 1000}s\"",
            "\"Elapsed: {started_at}s\"",
        )
}

/// The clock example plus `requires Network;` and a handler that uses it.
fn clock_with_network() -> String {
    CLOCK
        .replace("    requires Clock;\n", "    requires Clock;\n    requires Network;\n")
        .replace(
            "    state ticking: Bool = false;\n",
            "    state ticking: Bool = false;\n    state q: String = \"\";\n    state r: Result[List[String], String] = Ok([]);\n",
        )
        .replace(
            "    on Tick {",
            "    on Search {\n        r = await(net.search(q));\n    }\n\n    on Tick {",
        )
}

fn counter_with(extra: &str) -> String {
    COUNTER.replacen(
        "    on Increment {",
        &format!("{extra}\n    on Increment {{"),
        1,
    )
}

// ---------- no change ----------

#[test]
fn identical_programs_have_no_semantic_change() {
    let r = diff(CLOCK, CLOCK);
    assert!(!r.changed);
    assert_eq!(r.change_count, 0);
    assert_eq!(r.authority, Authority::Unchanged);
    assert!(render_human(&r).contains("no semantic change"));
}

#[test]
fn comments_blank_lines_and_line_shifts_are_not_a_change() {
    let shifted = format!("// a new header comment\n\n\n// and another\n{CLOCK}\n\n// trailing\n");
    let r = diff(CLOCK, &shifted);
    assert!(!r.changed, "{:?}", r.components);
}

#[test]
fn reordering_state_and_events_is_not_a_change() {
    let a = counter_with("    state a: Int = 1;\n    state b: Int = 2;\n");
    let b = counter_with("    state b: Int = 2;\n    state a: Int = 1;\n");
    assert!(!diff(&a, &b).changed);
    let reordered = COUNTER
        .replace("on Increment {\n        count = count + 1;\n    }\n\n", "")
        .replace(
            "    on Reset {\n        count = 0;\n    }\n",
            "    on Reset {\n        count = 0;\n    }\n\n    on Increment {\n        count = count + 1;\n    }\n",
        );
    assert!(!diff(COUNTER, &reordered).changed);
}

// ---------- authority ----------

#[test]
fn a_new_capability_is_authority_expanded() {
    let r = json(CLOCK, &clock_with_network());
    assert_eq!(r["changed"], true);
    assert_eq!(r["authority"], "expanded");
    let added = &r["capabilities"]["added"];
    assert_eq!(added.as_array().unwrap().len(), 1);
    assert_eq!(added[0]["component"], "Clock");
    assert_eq!(added[0]["capability"], "Network");
    assert_eq!(r["capabilities"]["removed"].as_array().unwrap().len(), 0);
    let c = &r["components"]["changed"][0];
    assert_eq!(c["name"], "Clock");
    assert_eq!(c["authority"], "expanded");
    assert_eq!(strs(&c["requires"]["added"]), ["Network"]);
}

#[test]
fn a_removed_capability_is_authority_reduced() {
    let r = json(CLOCK, &clock_without_capability());
    assert_eq!(r["authority"], "reduced");
    assert_eq!(r["capabilities"]["removed"][0]["capability"], "Clock");
    assert_eq!(r["capabilities"]["added"].as_array().unwrap().len(), 0);
    let eff = &r["components"]["changed"][0]["effects"];
    assert_eq!(eff["removed"][0]["capability"], "Clock");
    assert_eq!(eff["removed"][0]["path"], "clock.now");
}

#[test]
fn the_same_capabilities_are_authority_unchanged_even_when_other_things_change() {
    let r = json(COUNTER, &counter_with("    state extra: Int = 0;\n"));
    assert_eq!(r["changed"], true);
    assert_eq!(r["authority"], "unchanged");
}

#[test]
fn expansion_is_not_masked_by_a_removal_elsewhere() {
    let two = |a: &str, b: &str| {
        format!(
            "component A {{\n    {a}\n    state x: Int = 0;\n    on Go {{ x = {ax}; }}\n    view {{ Column {{ Button {{ label: \"go\" on: Go }} }} }}\n}}\n\
             component B {{\n    {b}\n    state y: Int = 0;\n    on Go {{ y = {by}; }}\n    view {{ Column {{ Button {{ label: \"go\" on: Go }} }} }}\n}}\n",
            ax = if a.is_empty() { "1" } else { "clock.now()" },
            by = if b.is_empty() { "1" } else { "clock.now()" },
        )
    };
    let base = two("requires Clock;", "");
    let cand = two("", "requires Clock;");
    let r = json(&base, &cand);
    assert_eq!(r["authority"], "expanded");
    assert_eq!(r["capabilities"]["added"][0]["component"], "B");
    assert_eq!(r["capabilities"]["removed"][0]["component"], "A");
}

#[test]
fn diff_is_antisymmetric_for_authority() {
    let (a, b) = (CLOCK.to_string(), clock_with_network());
    assert_eq!(diff(&a, &b).authority, Authority::Expanded);
    assert_eq!(diff(&b, &a).authority, Authority::Reduced);
    let (fwd, back) = (json(&a, &b), json(&b, &a));
    assert_eq!(
        fwd["components"]["changed"][0]["state"]["added"],
        back["components"]["changed"][0]["state"]["removed"]
    );
    assert_eq!(
        fwd["components"]["changed"][0]["events"]["added"],
        back["components"]["changed"][0]["events"]["removed"]
    );
}

// ---------- components ----------

const TWO: &str = "\
component A {
    state x: Int = 0;
    on Go { x = 1; }
    view { Column { Button { label: \"a\" on: Go } } }
}
";
const WITH_B: &str = "\
component B {
    requires Clock;
    state t: Int = 0;
    on Go { t = clock.now(); }
    view { Column { Button { label: \"b\" on: Go } } }
}
";

#[test]
fn added_component_with_requires_is_expanded() {
    let r = json(TWO, &format!("{TWO}{WITH_B}"));
    assert_eq!(r["authority"], "expanded");
    let a = &r["components"]["added"][0];
    assert_eq!(a["name"], "B");
    assert_eq!(strs(&a["requires"]), ["Clock"]);
    assert_eq!(strs(&a["state"]), ["t"]);
    assert_eq!(strs(&a["events"]), ["Go"]);
    assert_eq!(r["capabilities"]["added"][0]["component"], "B");
}

#[test]
fn removed_component_with_requires_is_reduced() {
    let r = json(&format!("{TWO}{WITH_B}"), TWO);
    assert_eq!(r["authority"], "reduced");
    assert_eq!(r["components"]["removed"][0]["name"], "B");
    assert_eq!(r["capabilities"]["removed"][0]["capability"], "Clock");
}

#[test]
fn added_component_without_requires_is_a_change_but_not_expanded() {
    let extra = "component C {\n    state z: Int = 0;\n    on Go { z = 1; }\n    view { Column { Button { label: \"c\" on: Go } } }\n}\n";
    let r = json(TWO, &format!("{TWO}{extra}"));
    assert_eq!(r["changed"], true);
    assert_eq!(r["authority"], "unchanged");
    assert_eq!(r["components"]["added"][0]["name"], "C");
}

// ---------- state ----------

#[test]
fn state_added_removed_retyped_and_reinitialised() {
    let base = "component S {\n    state a: Int = 0;\n    state b: Int = 1;\n    state c: Int = 2;\n    state gone: Bool = false;\n    on Go { a = 1; b = 2; c = 3; }\n    view { Column { Text { \"{a}{b}{c}\" } } }\n}\n";
    let cand = "component S {\n    state a: Int = 5;\n    state b: String = \"x\";\n    state c: Int = 2;\n    state fresh: List[String] = [];\n    on Go { a = 1; b = \"y\"; c = 3; }\n    view { Column { Text { \"{a}{b}{c}\" } } }\n}\n";
    let r = json(base, cand);
    let s = &r["components"]["changed"][0]["state"];
    assert_eq!(s["added"][0]["name"], "fresh");
    assert_eq!(s["added"][0]["ty"], "List[String]");
    assert_eq!(s["added"][0]["initial"], "[]");
    assert_eq!(s["removed"][0]["name"], "gone");
    let changed = s["changed"].as_array().unwrap();
    assert_eq!(changed.len(), 2, "c is untouched: {changed:?}");
    assert_eq!(changed[0]["name"], "a");
    assert_eq!(changed[0]["initial"]["base"], "0");
    assert_eq!(changed[0]["initial"]["candidate"], "5");
    assert_eq!(changed[0]["ty"], Value::Null);
    assert_eq!(changed[1]["name"], "b");
    assert_eq!(changed[1]["ty"]["base"], "Int");
    assert_eq!(changed[1]["ty"]["candidate"], "String");
}

// ---------- events ----------

#[test]
fn events_added_removed_and_their_writes() {
    let r = json(
        COUNTER,
        &counter_with("    state n: Int = 0;\n    on Bump { n = n + 1; count = 0; }\n"),
    );
    let e = &r["components"]["changed"][0]["events"];
    assert_eq!(e["added"][0]["name"], "Bump");
    assert_eq!(strs(&e["added"][0]["writes"]), ["count", "n"]);
    let back = json(
        &counter_with("    state n: Int = 0;\n    on Bump { n = n + 1; count = 0; }\n"),
        COUNTER,
    );
    assert_eq!(
        back["components"]["changed"][0]["events"]["removed"][0]["name"],
        "Bump"
    );
}

#[test]
fn handler_writes_added_and_removed_are_named() {
    let base = "component W {\n    state a: Int = 0;\n    state b: Int = 0;\n    on Go { a = 1; }\n    view { Column { Text { \"{a}{b}\" } Button { label: \"g\" on: Go } } }\n}\n";
    let cand = base.replace("on Go { a = 1; }", "on Go { b = 1; }");
    let r = json(base, &cand);
    let ev = &r["components"]["changed"][0]["events"]["changed"][0];
    assert_eq!(ev["name"], "Go");
    assert_eq!(strs(&ev["writes_added"]), ["b"]);
    assert_eq!(strs(&ev["writes_removed"]), ["a"]);
}

#[test]
fn handler_body_change_is_reported_even_when_writes_are_the_same() {
    let r = json(
        SEARCH,
        &SEARCH.replace("on ShowLong { min = 12; }", "on ShowLong { min = 20; }"),
    );
    let ev = &r["components"]["changed"][0]["events"]["changed"][0];
    assert_eq!(ev["name"], "ShowLong");
    assert_eq!(ev["writes_added"].as_array().unwrap().len(), 0);
    assert_eq!(ev["writes_removed"].as_array().unwrap().len(), 0);
    assert_eq!(strs(&ev["body"]["base"]), ["min = 12;"]);
    assert_eq!(strs(&ev["body"]["candidate"]), ["min = 20;"]);
    assert_eq!(r["authority"], "unchanged");
}

#[test]
fn event_params_change_is_reported() {
    let r = json(
        SEARCH,
        &SEARCH.replace(
            "on Change(value) { query = value; }",
            "on Change(text) { query = text; }",
        ),
    );
    let ev = &r["components"]["changed"][0]["events"]["changed"][0];
    assert_eq!(strs(&ev["params"]["base"]), ["value"]);
    assert_eq!(strs(&ev["params"]["candidate"]), ["text"]);
}

// ---------- effects ----------

#[test]
fn added_effect_carries_a_candidate_span() {
    let cand = clock_with_network();
    let line = cand.lines().position(|l| l.contains("net.search")).unwrap() as u64 + 1;
    let r = json(CLOCK, &cand);
    let eff = &r["components"]["changed"][0]["effects"]["added"][0];
    assert_eq!(eff["capability"], "Network");
    assert_eq!(eff["path"], "net.search");
    assert_eq!(eff["spans"][0]["start"]["line"], line);
}

#[test]
fn effect_site_count_change_is_reported_without_a_new_capability() {
    let more = CLOCK.replace(
        "ticking = false;\n    }\n\n    on Tick",
        "ticking = false;\n        started_at = clock.now();\n    }\n\n    on Tick",
    );
    assert_ne!(more, CLOCK);
    let r = json(CLOCK, &more);
    let c = &r["components"]["changed"][0]["effects"]["changed"][0];
    assert_eq!(c["capability"], "Clock");
    assert_eq!(c["path"], "clock.now");
    assert_eq!(
        c["candidate_count"].as_u64().unwrap(),
        c["base_count"].as_u64().unwrap() + 1
    );
    assert_eq!(r["authority"], "unchanged");
}

// ---------- view ----------

#[test]
fn view_label_and_new_control_are_reported() {
    let relabelled = COUNTER.replace("label: \"Reset\"", "label: \"Clear\"");
    let r = json(COUNTER, &relabelled);
    let v = &r["components"]["changed"][0]["view"];
    assert_eq!(
        strs(&v["removed"]),
        ["Button role=button label=\"Reset\" on=Reset"]
    );
    assert!(strs(&v["added"])[0].contains("label=\"Clear\""));

    let second = COUNTER.replace(
        "            Button {\n                role: button\n                label: \"Reset\"",
        "            Button {\n                role: button\n                label: \"Reset\"\n                on: Reset\n            }\n            Button {\n                role: button\n                label: \"Reset\"",
    );
    let r = json(COUNTER, &second);
    assert_eq!(
        r["components"]["changed"][0]["view"]["added"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn rewiring_a_button_to_another_event_is_a_change() {
    let rewired = COUNTER.replace(
        "label: \"Increment\"\n                on: Increment",
        "label: \"Increment\"\n                on: Reset",
    );
    let r = json(COUNTER, &rewired);
    assert_eq!(r["changed"], true);
    assert!(strs(&r["components"]["changed"][0]["view"]["added"])[0].contains("on=Reset"));
}

// ---------- not compared ----------

#[test]
fn the_report_names_what_it_does_not_compare() {
    let r = json(COUNTER, COUNTER);
    let nc = strs(&r["not_compared"]);
    for k in [
        "foreign_boundaries",
        "properties",
        "view_structure_and_order",
    ] {
        assert!(nc.contains(&k.to_string()), "{nc:?}");
    }
}

// ---------- determinism ----------

#[test]
fn output_is_byte_identical_across_runs() {
    let a = serde_json::to_string_pretty(&diff(CLOCK, &clock_with_network())).unwrap();
    for _ in 0..5 {
        let b = serde_json::to_string_pretty(&diff(CLOCK, &clock_with_network())).unwrap();
        assert_eq!(a, b);
    }
}

#[test]
fn lists_are_sorted() {
    let base = "component S {\n    state z: Int = 0;\n    state a: Int = 0;\n    on Go { z = 1; }\n    view { Column { Text { \"{a}{z}\" } Button { label: \"g\" on: Go } } }\n}\n";
    let cand = "component S {\n    state z: Int = 9;\n    state a: Int = 9;\n    state m: Int = 0;\n    state b: Int = 0;\n    on Go { z = 1; }\n    view { Column { Text { \"{a}{z}{m}{b}\" } Button { label: \"g\" on: Go } } }\n}\n";
    let s = &json(base, cand)["components"]["changed"][0]["state"];
    let names = |k: &str| -> Vec<String> {
        s[k].as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names("added"), ["b", "m"]);
    assert_eq!(names("changed"), ["a", "z"]);
}

// ---------- the binary ----------

struct Tmp(PathBuf);
impl Tmp {
    fn new(tag: &str) -> Tmp {
        let p = std::env::temp_dir().join(format!("cui-diff-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
    fn file(&self, name: &str, body: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("binary runs");
    (
        out.status.code().expect("exited"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn every_example_diffed_against_itself_is_no_change() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut n = 0;
    for dir in std::fs::read_dir(&root).unwrap() {
        let dir = dir.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).unwrap() {
            let f = f.unwrap().path();
            if f.extension().is_some_and(|e| e == "cui") {
                let p = f.to_str().unwrap();
                let (code, out, err) = run(&["diff", p, p]);
                assert_eq!(code, 0, "{p}: {out}{err}");
                n += 1;
            }
        }
    }
    assert!(n >= 10, "only {n} examples found");
}

#[test]
fn exit_0_when_nothing_changed() {
    let (code, out, _) = run(&[
        "diff",
        "examples/clock/clock.cui",
        "examples/clock/clock.cui",
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("no semantic change"));
    let (code, out, _) = run(&[
        "diff",
        "examples/clock/clock.cui",
        "examples/clock/clock.cui",
        "--output",
        "json",
    ]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema"], "cancho-ui-diff");
    assert_eq!(v["changed"], false);
}

#[test]
fn exit_5_when_programs_differ_and_json_matches_the_library() {
    let t = Tmp::new("differs");
    let cand = t.file("c.cui", &clock_with_network());
    let c = cand.to_str().unwrap();
    let (code, out, _) = run(&["diff", "examples/clock/clock.cui", c, "--output", "json"]);
    assert_eq!(code, 5);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["authority"], "expanded");
    assert_eq!(v["base"], "examples/clock/clock.cui");
    // `--output` may come first.
    let (code2, out2, _) = run(&["diff", "--output", "json", "examples/clock/clock.cui", c]);
    assert_eq!((code2, &out2), (5, &out));
    let (code, human, _) = run(&["diff", "examples/clock/clock.cui", c]);
    assert_eq!(code, 5);
    assert!(human.contains("authority: expanded"), "{human}");
    assert!(human.contains("+ requires Network"), "{human}");
}

#[test]
fn removed_capability_exits_5_and_says_reduced() {
    let t = Tmp::new("reduced");
    let cand = t.file("c.cui", &clock_without_capability());
    let (code, out, _) = run(&[
        "diff",
        "examples/clock/clock.cui",
        cand.to_str().unwrap(),
        "--output",
        "json",
    ]);
    assert_eq!(code, 5);
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap()["authority"],
        "reduced"
    );
}

#[test]
fn a_file_that_does_not_compile_is_a_located_refusal_exit_1() {
    let t = Tmp::new("refused");
    // Clock use without `requires Clock;`.
    let bad = t.file("bad.cui", &CLOCK.replace("    requires Clock;\n", ""));
    let b = bad.to_str().unwrap();
    let (code, out, err) = run(&["diff", "examples/clock/clock.cui", b]);
    assert_eq!(code, 1);
    assert!(out.is_empty(), "no diff on stdout for a refusal: {out}");
    assert!(err.contains("capability-missing"), "{err}");
    assert!(err.contains("bad.cui:"), "located: {err}");

    let (code, out, _) = run(&["diff", b, "examples/clock/clock.cui", "--output", "json"]);
    assert_eq!(code, 1);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v[0]["code"], "capability-missing");
    assert!(v[0]["span"]["file"].as_str().unwrap().ends_with("bad.cui"));
}

#[test]
fn both_refusals_are_reported_in_one_run() {
    let t = Tmp::new("both");
    let a = t.file("a.cui", "component {");
    let b = t.file("b.cui", &CLOCK.replace("    requires Clock;\n", ""));
    let (code, out, _) = run(&[
        "diff",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--output",
        "json",
    ]);
    assert_eq!(code, 1);
    assert_eq!(
        serde_json::from_str::<Value>(&out)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn garbage_input_is_a_refusal_not_a_panic() {
    let t = Tmp::new("garbage");
    let g = t.file("g.cui", "\u{0}\u{1} component ((( {{{ \"unterminated");
    let (code, _, err) = run(&["diff", "examples/clock/clock.cui", g.to_str().unwrap()]);
    assert_eq!(code, 1, "{err}");
    assert!(!err.contains("panicked"), "{err}");
    let e = t.file("empty.cui", "");
    let (code, _, err) = run(&["diff", e.to_str().unwrap(), e.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "{code} {err}");
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn exit_3_for_a_missing_file() {
    let (code, _, err) = run(&["diff", "examples/clock/clock.cui", "/nonexistent/nope.cui"]);
    assert_eq!(code, 3, "{err}");
}

#[test]
fn exit_2_for_a_wrong_command_line() {
    for args in [
        vec!["diff"],
        vec!["diff", "examples/clock/clock.cui"],
        vec!["diff", "a.cui", "b.cui", "c.cui"],
        vec!["diff", "a.cui", "b.cui", "--bogus"],
        vec!["diff", "a.cui", "b.cui", "--output"],
        vec!["diff", "a.cui", "b.cui", "--output", "yaml"],
    ] {
        let (code, _, _) = run(&args);
        assert_eq!(code, 2, "{args:?}");
    }
}

#[test]
fn diff_is_on_the_introspect_and_skill_and_help_surfaces() {
    let (code, out, _) = run(&["introspect"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    let d = v["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "diff")
        .expect("diff in introspect");
    assert_eq!(d["exit_codes"]["5"], "the programs differ semantically");
    assert_eq!(d["exit_codes"]["1"], "a file was refused");
    assert!(v["exit_codes"]["5"].as_str().unwrap().contains("diff"));
    let (_, skill, _) = run(&["skill"]);
    assert!(skill.contains("cancho-ui diff <base.cui> <candidate.cui>"));
    assert!(skill.contains("`5`"));
    let (_, help, _) = run(&["--help"]);
    assert!(help.contains("cancho-ui diff <base.cui> <candidate.cui>"));
    assert!(help.contains("5  (diff only)"));
}

#[test]
fn existing_exit_codes_are_unchanged() {
    let (code, _, _) = run(&["check", "examples/clock/clock.cui"]);
    assert_eq!(code, 0);
    let t = Tmp::new("check");
    let bad = t.file("bad.cui", &CLOCK.replace("    requires Clock;\n", ""));
    let (code, _, _) = run(&["check", bad.to_str().unwrap()]);
    assert_eq!(code, 1);
}
