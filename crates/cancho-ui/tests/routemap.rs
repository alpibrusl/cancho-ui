//! `RouteMap` (`docs/routes.md`): the shared corpus `tests/routemap_corpus.json` replayed in-process (the file `scripts/check-routemap.js` runs against the
//! page's own functions), what the report says about a route map, every rule that refuses a misuse, the page's adapter, the colours of a pin, and the
//! example's trace.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/routemap_corpus.json");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root())
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
    let dir = common::scratch_root().join(format!("cancho-ui-routemap-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn replay(src: &str, steps: Vec<Value>) -> (i32, Value) {
    let file = scratch("r.cui", src);
    let trace = scratch(
        "r.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (
        code,
        serde_json::from_str(&stdout).unwrap_or(Value::String(stderr)),
    )
}

/// A cui string literal holding `text`: quotes, backslashes and braces escaped.
fn lit(text: &str) -> String {
    let mut s = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '{' => s.push_str("\\{"),
            '}' => s.push_str("\\}"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}

fn program_for(rows: &[String], route: bool) -> String {
    let list = rows.iter().map(|r| lit(r)).collect::<Vec<_>>().join(", ");
    format!(
        "component T {{\n    state rows: List[String] = [{list}];\n    state joined: Bool = {route};\n    view {{ Column {{ RouteMap {{ label: \"Trip\" points: rows route: joined }} }} }}\n}}\n"
    )
}

#[test]
fn replay_reports_what_the_page_says_for_every_case_of_the_corpus() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 17, "the corpus shrank");
    for k in cases {
        let rows: Vec<String> = k["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|r| r.as_str().expect("a row").to_string())
            .collect();
        let route = k["route"].as_bool().expect("route");
        let mut want = k["want"].clone();
        want["label"] = json!("Trip");
        let (code, report) = replay(
            &program_for(&rows, route),
            vec![json!({"advance": 0, "expect": {"routemaps": [want]}})],
        );
        assert_eq!(
            code,
            0,
            "{}: {}",
            k["why"],
            report.to_string().chars().take(3000).collect::<String>()
        );
    }
}

#[test]
fn the_antimeridian_is_not_handled_and_the_report_says_how_wide_the_diagram_is() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let k = c["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|k| {
            k["why"]
                .as_str()
                .is_some_and(|w| w.contains("antimeridian"))
        })
        .expect("the case");
    // two points 2 degrees apart across the line: 222 km by the list, and a diagram the width of the world
    assert_eq!(k["want"]["km"], json!(222.4));
    assert!(k["want"]["across_km"].as_f64().expect("a width") > 39000.0);
}

const COMP: &str = "component C {\n    state rows: List[String] = [\"\\{\\\"name\\\":\\\"A\\\",\\\"pos\\\":\\\"45,6\\\"\\}\"];\n    state flag: Bool = true;\n    state n: Int = 1;\n    state name: String = \"Trip\";\n    state show: Bool = true;\n    state nums: List[Int] = [1];\n    on Go { n = n + 1; }\n    view { Column { VIEW } }\n}\n";

fn comp(view: &str) -> String {
    COMP.replace("VIEW", view)
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("c.cui", src);
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
    let file = scratch("c.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{src}\n{stdout}{stderr}");
}

#[test]
fn a_route_map_is_accepted_without_a_capability() {
    accepted(&comp(
        "RouteMap { label: \"Trip\" points: rows route: true }",
    ));
    accepted(&comp(
        "RouteMap { label: \"Trip\" points: rows route: false }",
    ));
    accepted(&comp(
        "RouteMap { label: \"Trip\" points: rows route: flag }",
    ));
    accepted(&comp("RouteMap { label: \"Trip\" points: rows }"));
    accepted(&comp("RouteMap { label: name points: rows }"));
}

#[test]
fn a_route_map_needs_a_label_and_points() {
    for view in [
        "RouteMap { points: rows }",
        "RouteMap { label: \"Trip\" }",
        "RouteMap { route: true }",
        "RouteMap { }",
    ] {
        assert_eq!(codes(&comp(view)), ["routemap-incomplete"], "{view}");
    }
}

#[test]
fn points_and_route_are_checked() {
    for view in [
        "RouteMap { label: \"Trip\" points: name }",
        "RouteMap { label: \"Trip\" points: flag }",
        "RouteMap { label: \"Trip\" points: n }",
    ] {
        assert_eq!(codes(&comp(view)), ["routemap-binding-invalid"], "{view}");
    }
    for view in [
        "RouteMap { label: \"Trip\" points: rows route: name }",
        "RouteMap { label: \"Trip\" points: rows route: n }",
        "RouteMap { label: \"Trip\" points: rows route: 1 }",
        "RouteMap { label: \"Trip\" points: rows route: \"yes\" }",
    ] {
        let c = codes(&comp(view));
        assert!(
            c == ["routemap-binding-invalid"] || c == ["parse-unexpected-token"],
            "{view}: {c:?}"
        );
    }
    // a literal list of rows is not a binding: the rows are a state field
    let c = codes(&comp("RouteMap { label: \"Trip\" points: [\"a\"] }"));
    assert_eq!(c, ["parse-unexpected-token"], "{c:?}");
}

#[test]
fn a_route_map_is_not_drawn_in_a_for() {
    let src = comp("For { over: rows as: r RouteMap { label: \"Trip\" points: rows } }");
    assert_eq!(codes(&src), ["routemap-in-loop"]);
    accepted(&comp(
        "If { when: show RouteMap { label: \"Trip\" points: rows } }",
    ));
}

#[test]
fn the_keys_of_a_route_map_belong_to_it() {
    for view in [
        "Text { points: rows \"x\" }",
        "Button { label: \"b\" route: true on: Go }",
        "Chart { label: \"T\" kind: \"line\" values: nums points: rows }",
        "Map { label: \"T\" route: true }",
        "RouteMap { label: \"Trip\" points: rows on: Go }",
        "RouteMap { label: \"Trip\" points: rows zoom: 3 }",
        "RouteMap { label: \"Trip\" points: rows tiles: \"https://tiles.example/{z}/{x}/{y}.png\" }",
        "RouteMap { label: \"Trip\" points: rows tone: \"ok\" }",
    ] {
        let c = codes(&comp(view));
        assert!(
            c == ["binding-unknown"] || c == ["parse-unexpected-token"],
            "{view}: {c:?}"
        );
    }
}

#[test]
fn a_hidden_route_map_is_not_in_the_report_and_the_label_and_route_follow_the_state() {
    let src = "component C {\n    state rows: List[String] = [\"\\{\\\"name\\\":\\\"A\\\",\\\"pos\\\":\\\"45,6\\\"\\}\", \"\\{\\\"name\\\":\\\"B\\\",\\\"pos\\\":\\\"45,7\\\"\\}\"];\n    state flag: Bool = true;\n    state show: Bool = true;\n    state name: String = \"T\";\n    on Join { flag = !flag; }\n    on Rename { name = \"U\"; }\n    on Hide { show = false; }\n    on Add { rows = concat(rows, [\"\\{\\\"name\\\":\\\"C\\\"\\}\"]); }\n    view { Column { If { when: show RouteMap { label: name points: rows route: flag } } } }\n}\n";
    let (code, report) = replay(
        src,
        vec![
            json!({"advance": 0, "expect": {"routemaps": [{"label": "T", "count": 2, "located": 2, "on_route": 2, "summary": "route map of 2 places, 2 on the route, about 79 km across"}]}}),
            json!({"event": "Join", "expect": {"routemaps": [{"on_route": 0, "km": 0, "summary": "route map of 2 places, route not drawn, about 79 km across"}]}}),
            json!({"event": "Rename", "expect": {"routemaps": [{"label": "U"}]}}),
            json!({"event": "Add", "expect": {"routemaps": [{"count": 3, "located": 2, "list": ["1. A, start", "2. B, 78.6 km from the previous point", "C, no position"]}]}}),
            json!({"event": "Hide", "expect": {"routemaps": []}}),
        ],
    );
    assert_eq!(code, 0, "{report}");
}

#[test]
fn a_wrong_expectation_about_the_summary_fails() {
    let rows = vec!["{\"name\":\"A\",\"pos\":\"45,6\"}".to_string()];
    let (code, _) = replay(
        &program_for(&rows, true),
        vec![
            json!({"advance": 0, "expect": {"routemaps": [{"summary": "route map of 2 places"}]}}),
        ],
    );
    assert_ne!(code, 0, "a summary that is not the page's must fail");
}

fn build(src: &str) -> String {
    let file = scratch("b.cui", src);
    let out = scratch("b.js", "");
    let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

#[test]
fn the_page_carries_the_adapter_only_for_a_program_that_draws_a_route_map() {
    let plain = build(&comp("Button { label: \"x\" on: Go }"));
    assert!(!plain.contains("__cui_routemap"), "{plain}");
    let js = build(&comp(
        "RouteMap { label: \"Trip\" points: rows route: flag }",
    ));
    assert!(
        js.contains("function __cui_routemap(label, rows, route, getj, dist)"),
        "{js}"
    );
    assert!(js.contains("__cui_routemap(\"Trip\""), "{js}");
    // it helps itself to the position reader and the distance, and to nothing else
    assert!(js.contains("function __cui_position("), "{js}");
    assert!(js.contains("function __cui_distance("), "{js}");
    // nothing is fetched and nothing from the program is parsed as markup
    for banned in [
        "innerHTML",
        "insertAdjacentHTML",
        "fetch(",
        "XMLHttpRequest",
        "new Image",
        "createElement('img')",
        "http://",
    ] {
        let adapter = &js[js
            .find("// Platform adapter: RouteMap")
            .expect("the adapter")..];
        let adapter = &adapter[..adapter
            .find("function __cui_routemap(")
            .expect("the function")
            + adapter[adapter.find("function __cui_routemap(").unwrap()..]
                .find("\n}\n")
                .expect("its end")];
        // the one `http://` is the SVG namespace
        if banned == "http://" {
            assert_eq!(adapter.matches("http://").count(), 1, "{adapter}");
            assert!(adapter.contains("http://www.w3.org/2000/svg"));
        } else {
            assert!(!adapter.contains(banned), "{banned}");
        }
    }
    // the chart's name is on the svg, not also on the figure
    assert!(
        !js.contains("setAttribute('aria-label', \"Trip\")"),
        "the figure carries no name of its own:\n{js}"
    );
}

#[test]
fn a_route_map_is_a_hole_for_the_prerenderer_and_the_diff_shows_it_added() {
    let file = scratch(
        "p.cui",
        &comp("Text { \"x\" } RouteMap { label: \"Trip\" points: rows }"),
    );
    let (code, out, _) = run(&["inspect", "prerender", p(&file), "C"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    let holes: Vec<&str> = v["holes"]
        .as_array()
        .expect("holes")
        .iter()
        .map(|h| h["what"].as_str().expect("what"))
        .collect();
    assert!(holes.iter().any(|h| h.contains("RouteMap")), "{holes:?}");

    let a = scratch("a.cui", &comp("Button { label: \"x\" on: Go }"));
    let b = scratch(
        "b.cui",
        &comp("Button { label: \"x\" on: Go } RouteMap { label: \"Trip\" points: rows }"),
    );
    let (code, out, err) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert!(code == 0 || code == 5, "{code} {out}{err}");
    assert!(out.contains("RouteMap"), "{out}");
    // and it is no authority: a route map adds no capability to the diff
    assert!(!out.contains("Network"), "{out}");
}

fn hex(h: &str) -> [u8; 3] {
    let h = h.trim_start_matches('#');
    [0, 2, 4].map(|i| u8::from_str_radix(&h[i..i + 2], 16).expect("a hex colour"))
}

/// The value of `--cui-route-<name>` in the first `:root` rule after `from`.
fn token(css: &str, from: usize, name: &str) -> String {
    let key = format!("--cui-route-{name}: ");
    let at = css[from..].find(&key).expect(&key) + from + key.len();
    css[at..]
        .split([';', ' '])
        .next()
        .expect("a value")
        .to_string()
}

#[test]
fn the_pin_colours_are_checked_in_light_and_dark() {
    let dir =
        common::scratch_root().join(format!("cancho-ui-routemap-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (code, out, err) = run(&["build", "examples/route/route.cui", "--page", "-o", p(&dir)]);
    assert_eq!(code, 0, "{out}{err}");
    let html = std::fs::read_to_string(dir.join("index.html")).expect("the page");
    let light_at = html
        .find("--cui-route-lodging: ")
        .expect("the light tokens");
    let dark_at = html[light_at + 10..]
        .find("--cui-route-lodging: ")
        .expect("the dark tokens")
        + light_at
        + 10;
    assert!(
        html[..dark_at]
            .rfind("prefers-color-scheme: dark")
            .is_some_and(|m| m > light_at),
        "the second set is inside the dark media query"
    );
    // the page: white in light, Canvas rgb(18,18,18) in dark (what docs/charts.md measured); a pin is a fill with the page colour as its number and
    // outline, so the same ratio is its number's contrast and its edge's. 4.5:1 for the number (WCAG 1.4.3), and 3:1 for the shape (1.4.11).
    for (scheme, at, page) in [
        ("light", light_at, [255u8, 255, 255]),
        ("dark", dark_at, [18, 18, 18]),
    ] {
        for kind in ["lodging", "activity", "transport"] {
            let fill = hex(&token(&html, at, kind));
            let ratio = cancho_ui_ir::theme::contrast_ratio(fill, page);
            assert!(ratio >= 4.5, "{scheme} {kind}: {ratio:.2}:1");
        }
    }
    // `other` and the ink are the page's own colours (CanvasText on Canvas), so they cannot fail
    assert!(html.contains("--cui-route-other: CanvasText"));
    assert!(html.contains("--cui-route-ink: Canvas"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_theme_gives_the_pins_its_own_checked_tokens() {
    // the theme's stylesheet maps the pin colours to tokens whose pairs on `bg` the compiler already checks (accent, good, muted, fg)
    let dir =
        common::scratch_root().join(format!("cancho-ui-routemap-theme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (code, out, err) = run(&["build", "examples/theme/theme.cui", "--page", "-o", p(&dir)]);
    assert_eq!(code, 0, "{out}{err}");
    let html = std::fs::read_to_string(dir.join("index.html")).expect("the page");
    for pair in [
        "--cui-route-lodging: var(--cui-accent)",
        "--cui-route-activity: var(--cui-good)",
        "--cui-route-transport: var(--cui-muted)",
        "--cui-route-other: var(--cui-fg)",
        "--cui-route-ink: var(--cui-bg)",
    ] {
        assert!(html.contains(pair), "{pair}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_example_replays_its_trace() {
    let (code, out, err) = run(&[
        "test",
        "examples/route/route.cui",
        "examples/route/route.trace.json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
}
