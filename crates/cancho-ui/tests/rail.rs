//! `Rail` (docs/structure.md 12.0s): every refusal is refused by its rule tag and only by it, the valid views check, the script and the stylesheet of the rows are in the page only of a program that has
//! one (every other page is the bytes it was), the place of a row is kept across a render, replay refuses a value a rail could not give, and the lists of the docs name every tag.
//! What a browser does with the row (scrolling, snapping, a finger, a key) is `scripts/check-rail.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const README: &str = include_str!("../../../spec/diagnostics/README.md");

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn scratch(text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-rail-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let f = dir.join("r.cui");
    std::fs::write(&f, text).expect("the program");
    f
}

fn codes(file: &Path) -> (i32, Vec<String>) {
    let (code, out, _) = run(&["check", p(file), "--output", "json"]);
    let diags: Vec<Value> = if out.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&out).expect("diagnostics are JSON")
    };
    (
        code,
        diags
            .iter()
            .map(|d| d["code"].as_str().expect("code").to_string())
            .collect(),
    )
}

fn page(text: &str) -> String {
    let f = scratch(text);
    let out = f.parent().expect("dir").join("out");
    let (code, o, e) = run(&["build", p(&f), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

fn script(text: &str) -> String {
    let f = scratch(text);
    let (code, js, e) = run(&["build", p(&f)]);
    assert_eq!(code, 0, "{e}");
    js
}

/// A component with a rail whose attributes are `attrs` and whose children are `kids`.
fn program(attrs: &str, kids: &str) -> String {
    format!(
        "component C {{\n    state cur: Int = 0;\n    state name: String = \"x\";\n    state flag: Bool = false;\n    state rows: List[Int] = [1, 2, 3];\n    on Pick(i) {{ cur = i; }}\n    on Plain {{ cur = 0; }}\n    on Two(a, b) {{ cur = a; }}\n    view {{ Column {{ Rail {{ {attrs} {kids} }} }} }}\n}}\n"
    )
}

const KIDS: &str = "Column { Text { \"a\" } } Column { Text { \"b\" } }";

const VALID: &[(&str, &str, &str)] = &[
    ("a named row", "label: \"Legs\"", KIDS),
    (
        "a row to choose from",
        "label: \"Legs\" on: Pick selected: cur",
        KIDS,
    ),
    (
        "every key",
        "label: \"Legs\" on: Pick selected: cur follow: true snap: \"center\" gap: 2 item_width: 74 dots: true orientation: \"horizontal\"",
        KIDS,
    ),
    (
        "a column",
        "label: \"Legs\" on: Pick orientation: \"vertical\" snap: \"end\"",
        KIDS,
    ),
    ("snapping off", "label: \"Legs\" snap: \"none\"", KIDS),
    (
        "an expression for the chosen place",
        "label: \"Legs\" on: Pick selected: cur + 1",
        KIDS,
    ),
    (
        "a minus one: none chosen",
        "label: \"Legs\" on: Pick selected: 0 - 1",
        KIDS,
    ),
    (
        "children from a For",
        "label: \"Legs\" on: Pick selected: cur",
        "For { over: rows as: n Column { Text { \"{n}\" } } }",
    ),
    (
        "the narrowest and the widest child",
        "label: \"Legs\" item_width: 20",
        KIDS,
    ),
    ("a label read from state", "label: name", KIDS),
    (
        "an event that ignores the place",
        "label: \"Legs\" on: Plain",
        KIDS,
    ),
];

#[test]
fn every_valid_rail_checks() {
    for (name, attrs, kids) in VALID {
        let f = scratch(&program(attrs, kids));
        assert_eq!(codes(&f), (0, vec![]), "{name}");
    }
    let f = scratch(&program("label: \"Legs\" item_width: 100", KIDS));
    assert_eq!(codes(&f), (0, vec![]), "a child as wide as the row");
}

/// (what is wrong, attributes, children, the one tag that refuses it)
const REFUSED: &[(&str, &str, &str, &str)] = &[
    ("no label", "on: Pick", KIDS, "rail-incomplete"),
    (
        "a blank label",
        "label: \"  \" on: Pick",
        KIDS,
        "rail-incomplete",
    ),
    ("nothing to hold", "label: \"Legs\"", "", "rail-incomplete"),
    (
        "an unknown orientation",
        "label: \"Legs\" orientation: \"diagonal\"",
        KIDS,
        "rail-orientation-invalid",
    ),
    (
        "an unknown snap",
        "label: \"Legs\" snap: \"middle\"",
        KIDS,
        "rail-snap-invalid",
    ),
    (
        "a snap that is a number",
        "label: \"Legs\" snap: 3",
        KIDS,
        "rail-snap-invalid",
    ),
    (
        "an item_width under the least",
        "label: \"Legs\" item_width: 19",
        KIDS,
        "rail-item-width-invalid",
    ),
    (
        "an item_width over the most",
        "label: \"Legs\" item_width: 101",
        KIDS,
        "rail-item-width-invalid",
    ),
    (
        "dots that are text",
        "label: \"Legs\" dots: \"yes\"",
        KIDS,
        "rail-flag-invalid",
    ),
    (
        "a selected that is text",
        "label: \"Legs\" on: Pick selected: \"1\"",
        KIDS,
        "rail-selected-invalid",
    ),
    (
        "a selected that is a Bool",
        "label: \"Legs\" on: Pick selected: flag",
        KIDS,
        "rail-selected-invalid",
    ),
    (
        "a selected that is a String field",
        "label: \"Legs\" on: Pick selected: name",
        KIDS,
        "rail-selected-invalid",
    ),
    (
        "a selected with nothing to choose",
        "label: \"Legs\" selected: cur",
        KIDS,
        "rail-selected-needs-on",
    ),
    (
        "a follow with nothing to choose",
        "label: \"Legs\" follow: true",
        KIDS,
        "rail-follow-needs-on",
    ),
    (
        "a gap off the scale",
        "label: \"Legs\" gap: 9",
        KIDS,
        "layout-value-invalid",
    ),
    (
        "a key that is not the rail's",
        "label: \"Legs\" colour: \"red\"",
        KIDS,
        "binding-unknown",
    ),
    (
        "max_width, which is a column's",
        "label: \"Legs\" max_width: 40",
        KIDS,
        "binding-unknown",
    ),
    (
        "an event that does not exist",
        "label: \"Legs\" on: Nowhere",
        KIDS,
        "event-unknown",
    ),
    (
        "an event with two parameters",
        "label: \"Legs\" on: Two",
        KIDS,
        "event-arity-mismatch",
    ),
    (
        "arguments of its own",
        "label: \"Legs\" on: Pick(1)",
        KIDS,
        "event-args-unsupported",
    ),
];

#[test]
fn every_refusal_is_refused_by_its_tag() {
    for (why, attrs, kids, tag) in REFUSED {
        let f = scratch(&program(attrs, kids));
        let (code, found) = codes(&f);
        assert_eq!(code, 1, "{why}: {found:?}");
        assert_eq!(found, vec![tag.to_string()], "{why}");
    }
}

#[test]
fn the_rails_keys_are_refused_on_anything_else() {
    for key in [
        "selected: cur",
        "follow: true",
        "orientation: \"vertical\"",
        "snap: \"center\"",
        "item_width: 50",
        "dots: true",
    ] {
        let src = format!(
            "component C {{\n    state cur: Int = 0;\n    view {{ Column {{ {key} Text {{ \"a\" }} }} }}\n}}\n"
        );
        let f = scratch(&src);
        let (code, found) = codes(&f);
        assert_eq!(code, 1, "{key}: {found:?}");
        assert_eq!(found, vec!["binding-unknown".to_string()], "{key}");
    }
}

#[test]
fn every_new_tag_is_documented_with_a_row_of_its_own() {
    for tag in [
        "rail-incomplete",
        "rail-orientation-invalid",
        "rail-snap-invalid",
        "rail-item-width-invalid",
        "rail-flag-invalid",
        "rail-selected-invalid",
        "rail-selected-needs-on",
        "rail-follow-needs-on",
        "trace-value-not-a-rail-index",
    ] {
        assert!(
            README.contains(&format!("`{tag}`")),
            "`{tag}` is not documented"
        );
        if !tag.starts_with("trace-") {
            assert!(
                README.contains(&format!("`{tag}` | ")),
                "`{tag}` has no row of its own in the README"
            );
            assert!(
                REFUSED.iter().any(|r| r.3 == tag),
                "no case reaches `{tag}`"
            );
        }
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let full = program(
        "label: \"Legs\" on: Pick selected: cur follow: true snap: \"center\" gap: 2 item_width: 74 dots: true",
        KIDS,
    );
    let dir = scratch(&full).parent().expect("dir").to_path_buf();
    let mut cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).collect();
    cuts.push(full.len());
    for (n, at) in cuts.iter().enumerate() {
        let f = dir.join(format!("cut{n}.cui"));
        std::fs::write(&f, &full[..*at]).expect("cut");
        let (code, _, err) = run(&["check", p(&f)]);
        assert!(code == 0 || code == 1, "prefix {at}: exit {code}: {err}");
    }
}

const FULL: &str = "component C {\n    state cur: Int = 1;\n    state rows: List[String] = [\"a\", \"b\", \"c\"];\n    on Pick(i) { cur = i; }\n    view { Column { Rail { label: \"Legs\" on: Pick selected: cur follow: true snap: \"center\" gap: 2 item_width: 74 dots: true orientation: \"vertical\" For { over: rows as: r Column { Text { \"{r}\" } } } } } }\n}\n";

#[test]
fn the_script_makes_a_box_a_row_inside_it_the_children_and_one_wiring() {
    let js = script(FULL);
    for want in [
        "className = 'cui-rail-box'",
        "className = \"cui-rail cui-rail-center cui-rail-v cui-gap-2\"",
        "setAttribute('data-cui-rail', \"",
        "setAttribute('aria-label', \"Legs\")",
        "style.setProperty('--cui-rail-item', '74%')",
        "classList.add('cui-rail-sized')",
        "__cui_rail_wire(",
        "sel: __state_cur, on: function (i) { __on_Pick(i); }, follow: true, dots: true, vertical: true, snap: \"center\"",
        "const __cui_rails = __cui_rail_save(root);",
        "__cui_rail_after(root, __cui_rails);",
        "function __cui_rail_wire(",
        "function __cui_rail_after(",
    ] {
        assert!(js.contains(want), "no `{want}` in\n{js}");
    }
    // the children are put in the row, the row in the box, the box in the page: in that order
    let row = js.find("__n1.appendChild(").expect("a child in the row");
    let boxed = js
        .find("__n1_box.appendChild(__n1)")
        .expect("the row in the box");
    assert!(row < boxed);
    // nothing the program writes is read as markup, and nothing is fetched
    for banned in [
        "innerHTML",
        "insertAdjacentHTML",
        "<img",
        "fetch(",
        "new Image",
        "eval(",
    ] {
        assert!(!js.contains(banned), "`{banned}` in the rail code");
    }
}

#[test]
fn a_rail_with_nothing_to_choose_has_no_listener_of_its_own_in_the_page_it_asks_for() {
    let js = script(&program("label: \"Legs\"", KIDS));
    assert!(js.contains("on: null"), "{js}");
    assert!(
        js.contains("follow: false, dots: false, vertical: false, snap: \"start\""),
        "{js}"
    );
}

#[test]
fn the_page_has_the_rules_of_the_rows_and_no_other_page_does() {
    let html = page(FULL);
    for want in [
        ".cui-rail-box {",
        ".cui-rail-h { flex-direction: row; overflow-x: auto; overflow-y: hidden; scroll-snap-type: x mandatory; }",
        ".cui-rail-v { flex-direction: column; overflow-y: auto; overflow-x: hidden; max-height: 24rem; scroll-snap-type: y mandatory; }",
        ".cui-rail-none { scroll-snap-type: none; }",
        ".cui-rail-center > * { scroll-snap-align: center; }",
        ".cui-rail > *:focus-visible {",
        ".cui-rail > .cui-rail-on {",
        ".cui-rail-dots {",
    ] {
        assert!(html.contains(want), "no `{want}`");
    }
    // the policy is the baseline: a row draws no picture and fetches nothing
    assert_eq!(
        common::directive(&html, "img-src").as_deref(),
        Some("'self' data:")
    );
    assert_eq!(
        common::directive(&html, "connect-src").as_deref(),
        Some("'self'")
    );
    let plain = "component C {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Column { Button { label: \"Add\" on: Go } Text { \"{n}\" } } }\n}\n";
    let js = script(plain);
    let html = page(plain);
    assert!(
        !js.contains("__cui_rail") && !js.contains("cui-rail"),
        "{js}"
    );
    assert!(!html.contains("cui-rail"), "no rule of the rows");
}

#[test]
fn the_row_is_not_scrolled_by_script_and_nothing_makes_a_timer_without_follow() {
    // the snapping is the stylesheet's; the script only puts the row back where a render found it and brings a chosen child into place
    let js = script(&program("label: \"Legs\" on: Pick selected: cur", KIDS));
    assert!(js.contains("function __cui_rail_after("));
    // the scroll listener (and its timer) is wired only for a row that follows
    let wire = js
        .split("function __cui_rail_wire(")
        .nth(1)
        .expect("the wiring")
        .split("// The place of every rail")
        .next()
        .expect("its end");
    assert!(wire.contains("if (on && o.follow) {"), "{wire}");
}

fn replay(src: &str, steps: Vec<Value>) -> (i32, Value) {
    let file = scratch(src);
    let trace = file.with_file_name("t.trace.json");
    std::fs::write(
        &trace,
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    )
    .expect("the trace");
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (
        code,
        serde_json::from_str(&stdout).unwrap_or(Value::String(stderr)),
    )
}

fn rules(report: &Value) -> Vec<String> {
    report["failures"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| f["rule"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default()
}

const ROW: &str = "component C {\n    state cur: Int = 0;\n    on Pick(i) { cur = i; }\n    view { Rail { label: \"Legs\" on: Pick selected: cur Text { \"a\" } Text { \"b\" } } }\n}\n";

#[test]
fn replay_takes_a_place_and_refuses_what_a_rail_cannot_give() {
    for ok in [0, 1, 7, 1000] {
        let (code, report) = replay(
            ROW,
            vec![json!({"event": "Pick", "params": [ok], "state": {"cur": ok}})],
        );
        assert_eq!(code, 0, "{ok}: {report}");
    }
    for (why, value) in [
        ("below the first", json!(-1)),
        ("between two places", json!(1.5)),
        ("text", json!("1")),
        ("a Bool", json!(true)),
    ] {
        let (code, report) = replay(
            ROW,
            vec![json!({"event": "Pick", "params": [value], "expect": {"outcome": "completed"}})],
        );
        assert_eq!(code, 4, "{why}: {report}");
        assert_eq!(rules(&report), ["trace-value-not-a-rail-index"], "{why}");
    }
}

#[test]
fn a_button_that_can_give_the_event_any_number_lifts_the_check() {
    let src = "component C {\n    state cur: Int = 0;\n    on Pick(i) { cur = i; }\n    view { Column { Rail { label: \"Legs\" on: Pick Text { \"a\" } } Button { label: \"Any\" on: Pick(-5) } } }\n}\n";
    let (code, report) = replay(
        src,
        vec![json!({"event": "Pick", "params": [-5], "state": {"cur": -5}})],
    );
    assert_eq!(code, 0, "{report}");
}

#[test]
fn the_example_checks_builds_and_has_three_rails() {
    let (code, _, err) = run(&["check", "examples/rail/rail.cui"]);
    assert_eq!(code, 0, "{err}");
    let out = scratch("").parent().expect("dir").join("demo");
    let (code, _, err) = run(&["build", "examples/rail/rail.cui", "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    assert_eq!(js.matches("__cui_rail_wire(__n").count(), 3);
}

#[test]
fn the_diff_names_a_rail_and_the_prerender_says_the_page_draws_it() {
    let a = scratch(&program("label: \"Legs\" on: Pick", KIDS));
    let b = scratch(&program(
        "label: \"Legs\" on: Pick selected: cur snap: \"center\"",
        KIDS,
    ));
    let (code, out, err) = run(&["diff", p(&a), p(&b)]);
    assert_eq!(code, 5, "{out}{err}");
    assert!(out.contains("Rail"), "{out}");
    let (code, out, err) = run(&["inspect", "prerender", p(&a), "C"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Rail"), "the hole is named: {out}");
}
