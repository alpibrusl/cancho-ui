//! `RouteMap` with `tiles:` (`docs/routes.md` section 9): the rules that refuse a misuse (each reached, with the negative controls), what replay reports
//! about the tiles (the shared corpus `tests/routemap_tiles_corpus.json`, written by `scripts/gen-routemap-tiles-corpus.py`), the Content-Security-Policy
//! of the page, the module a program without tiles gets, and the example's trace. Nothing here asks a tile host for anything.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/routemap_tiles_corpus.json");

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
    let dir = common::scratch_root().join(format!(
        "cancho-ui-routemap-tiles-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

const ROWS: &str = "[\"\\{\\\"name\\\":\\\"A\\\",\\\"pos\\\":\\\"45.9237,6.8694\\\"\\}\", \"\\{\\\"name\\\":\\\"B\\\",\\\"pos\\\":\\\"45.7969,6.9689\\\"\\}\"]";
const TILES: &str =
    "tiles: \"https://tiles.example/{z}/{x}/{y}.png\" attribution: \"Map data: the provider\"";

fn comp(requires: &str, view: &str) -> String {
    format!(
        "component C {{\n    {requires}\n    state rows: List[String] = {ROWS};\n    state name: String = \"https://tiles.example/{{z}}/{{x}}/{{y}}.png\";\n    view {{ Column {{ {view} }} }}\n}}\n"
    )
}

fn tiled(requires: &str) -> String {
    comp(
        requires,
        &format!("RouteMap {{ label: \"Trip\" points: rows {TILES} }}"),
    )
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
fn tiles_are_accepted_when_the_host_is_declared_and_credited() {
    accepted(&tiled("requires Network(\"tiles.example\");"));
    accepted(&tiled(
        "requires Network(\"TILES.example\", \"other.example\");",
    ));
    // a path on the page's own origin needs the bare capability
    accepted(&comp(
        "requires Network;",
        "RouteMap { label: \"Trip\" points: rows tiles: \"./tiles/{z}/{x}/{y}.png\" attribution: \"Stand-in\" route: false }",
    ));
    accepted(&comp(
        "requires Network(\"localhost:8099\");",
        "RouteMap { label: \"Trip\" points: rows tiles: \"http://localhost:8099/{z}/{x}/{y}.png\" attribution: \"Stand-in\" }",
    ));
}

#[test]
fn tiles_without_attribution_are_refused() {
    let src = comp(
        "requires Network(\"tiles.example\");",
        "RouteMap { label: \"Trip\" points: rows tiles: \"https://tiles.example/{z}/{x}/{y}.png\" }",
    );
    assert_eq!(codes(&src), ["routemap-incomplete"]);
}

#[test]
fn an_attribution_that_credits_nothing_or_nothing_at_all_is_refused() {
    let none = comp(
        "",
        "RouteMap { label: \"Trip\" points: rows attribution: \"Map data\" }",
    );
    assert_eq!(codes(&none), ["routemap-binding-invalid"]);
    for attr in ["\"\"", "\"   \"", &format!("\"{}\"", "x".repeat(201))] {
        let src = comp(
            "requires Network(\"tiles.example\");",
            &format!(
                "RouteMap {{ label: \"Trip\" points: rows tiles: \"https://tiles.example/{{z}}/{{x}}/{{y}}.png\" attribution: {attr} }}"
            ),
        );
        assert_eq!(codes(&src), ["routemap-binding-invalid"], "{attr}");
    }
    // 200 characters is the limit and is allowed
    accepted(&comp(
        "requires Network(\"tiles.example\");",
        &format!(
            "RouteMap {{ label: \"Trip\" points: rows tiles: \"https://tiles.example/{{z}}/{{x}}/{{y}}.png\" attribution: \"{}\" }}",
            "x".repeat(200)
        ),
    ));
}

#[test]
fn a_host_the_component_did_not_declare_is_refused() {
    // no Network at all
    assert_eq!(codes(&tiled("")), ["routemap-host-undeclared"]);
    // another host
    assert_eq!(
        codes(&tiled("requires Network(\"other.example\");")),
        ["routemap-host-undeclared"]
    );
    // a bare capability covers this page's origin, not a host on the internet
    assert_eq!(
        codes(&tiled("requires Network;")),
        ["routemap-host-undeclared"]
    );
    // the port is part of the host
    let src = comp(
        "requires Network(\"localhost\");",
        "RouteMap { label: \"Trip\" points: rows tiles: \"http://localhost:8099/{z}/{x}/{y}.png\" attribution: \"c\" }",
    );
    assert_eq!(codes(&src), ["routemap-host-undeclared"]);
}

#[test]
fn a_malformed_tile_address_is_refused() {
    let bad = [
        "\"https://tiles.example/{x}/{y}.png\"",
        "\"https://tiles.example/{z}/{y}.png\"",
        "\"https://tiles.example/{z}/{x}.png\"",
        "\"https://tiles.example/{z}/{z}/{x}/{y}.png\"",
        "\"https://tiles.example/{z}/{x}/{y}/{x}.png\"",
        "\"https://tiles.example/{z}/{x}/{y}.png?key=\\{k\\}\"",
        "\"http://tiles.example/{z}/{x}/{y}.png\"",
        "\"ftp://tiles.example/{z}/{x}/{y}.png\"",
        "\"tiles.example/{z}/{x}/{y}.png\"",
        "\"https://user:pw@tiles.example/{z}/{x}/{y}.png\"",
        "\"https://tiles.example/a b/{z}/{x}/{y}.png\"",
        "\"https://tiles.example/a\\\"b/{z}/{x}/{y}.png\"",
        "\"https://tiles.example/{z}/{x}/{y}.png#frag\"",
        "\"https:///{z}/{x}/{y}.png\"",
        "\"//tiles.example/{z}/{x}/{y}.png\"",
        "\"/\"",
        // the address is a literal, not a field or a number
        "name",
        "3",
    ];
    for t in bad {
        let src = comp(
            "requires Network(\"tiles.example\");",
            &format!("RouteMap {{ label: \"Trip\" points: rows tiles: {t} attribution: \"c\" }}"),
        );
        let c = codes(&src);
        assert!(
            c == ["routemap-tiles-invalid"] || c == ["parse-unexpected-token"],
            "{t}: {c:?}"
        );
        if t == "name" {
            assert_eq!(c, ["routemap-tiles-invalid"], "a field is not an address");
        }
    }
}

#[test]
fn a_route_map_with_a_declared_host_that_it_does_not_use_is_a_capability_unused() {
    // negative control: the host is declared, no tiles are asked for
    let src = comp(
        "requires Network(\"tiles.example\");",
        "RouteMap { label: \"Trip\" points: rows }",
    );
    assert_eq!(codes(&src), ["capability-unused"]);
    // and a tiled route map is that use
    accepted(&tiled("requires Network(\"tiles.example\");"));
}

#[test]
fn the_other_keys_of_a_map_are_still_not_a_route_maps() {
    let src = comp(
        "requires Network(\"tiles.example\");",
        &format!("RouteMap {{ label: \"Trip\" points: rows zoom: 13 {TILES} }}"),
    );
    assert_eq!(codes(&src), ["binding-unknown"]);
    // a Map still takes its keys and refuses a RouteMap's
    let src = comp(
        "requires Network(\"tiles.example\");",
        &format!("Map {{ label: \"M\" points: rows {TILES} }}"),
    );
    let c = codes(&src);
    assert!(
        c == ["binding-unknown"] || c == ["parse-unexpected-token"],
        "{c:?}"
    );
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

fn program_for(points: &[String], template: &str) -> String {
    let rows: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(i, q)| lit(&format!("{{\"name\":\"P{}\",\"pos\":\"{q}\"}}", i + 1)))
        .collect();
    format!(
        "component T {{\n    requires Network(\"tiles.example\");\n    state rows: List[String] = [{}];\n    view {{ Column {{ RouteMap {{ label: \"Trip\" points: rows tiles: \"{template}\" attribution: \"Credit\" }} }} }}\n}}\n",
        rows.join(", ")
    )
}

#[test]
fn replay_reports_the_zoom_the_grid_and_the_exact_tiles_for_every_case_of_the_corpus() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let template = c["template"].as_str().expect("template");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 50, "the corpus shrank");
    for k in cases {
        let points: Vec<String> = k["points"]
            .as_array()
            .expect("points")
            .iter()
            .map(|q| q.as_str().expect("a point").to_string())
            .collect();
        let w = &k["want"];
        let (code, report) = replay(
            &program_for(&points, template),
            vec![json!({"advance": 0, "expect": {"routemaps": [{
                "label": "Trip", "zoom": w["zoom"], "grid": w["grid"], "tiles": w["tiles"], "attribution": "Credit"
            }]}})],
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
fn no_point_one_point_and_a_tile_it_would_not_ask_for() {
    // tiles with zero points: nothing is asked for
    let (code, report) = replay(
        &program_for(&[], "https://tiles.example/{z}/{x}/{y}.png"),
        vec![
            json!({"advance": 0, "expect": {"routemaps": [{"count": 0, "zoom": null, "grid": [0, 0], "tiles": [], "summary": "route map with no places"}]}}),
        ],
    );
    assert_eq!(code, 0, "{report}");
    // rows with no readable position are also no points
    let (code, report) = replay(
        &program_for(
            &["nowhere".to_string()],
            "https://tiles.example/{z}/{x}/{y}.png",
        ),
        vec![
            json!({"advance": 0, "expect": {"routemaps": [{"located": 0, "zoom": null, "tiles": []}]}}),
        ],
    );
    assert_eq!(code, 0, "{report}");
    // one point: zoom 13 and nine tiles around Chamonix's
    let (code, report) = replay(
        &program_for(
            &["45.9237,6.8694".to_string()],
            "https://tiles.example/{z}/{x}/{y}.png",
        ),
        vec![
            json!({"advance": 0, "expect": {"routemaps": [{"zoom": 13, "grid": [3, 3], "tiles": [
                "https://tiles.example/13/4251/2915.png", "https://tiles.example/13/4252/2915.png", "https://tiles.example/13/4253/2915.png",
                "https://tiles.example/13/4251/2916.png", "https://tiles.example/13/4252/2916.png", "https://tiles.example/13/4253/2916.png",
                "https://tiles.example/13/4251/2917.png", "https://tiles.example/13/4252/2917.png", "https://tiles.example/13/4253/2917.png"
            ]}]}}),
        ],
    );
    assert_eq!(code, 0, "{report}");
    // a tile address the page would not ask for is a failure of the trace
    let (code, _) = replay(
        &program_for(
            &["45.9237,6.8694".to_string()],
            "https://tiles.example/{z}/{x}/{y}.png",
        ),
        vec![
            json!({"advance": 0, "expect": {"routemaps": [{"tiles": ["https://tiles.example/13/1/1.png"]}]}}),
        ],
    );
    assert_ne!(code, 0);
    let (code, _) = replay(
        &program_for(
            &["45.9237,6.8694".to_string()],
            "https://tiles.example/{z}/{x}/{y}.png",
        ),
        vec![json!({"advance": 0, "expect": {"routemaps": [{"zoom": 12}]}})],
    );
    assert_ne!(code, 0);
}

#[test]
fn a_route_map_without_tiles_reports_nothing_about_tiles() {
    let src = "component T {\n    state rows: List[String] = [\"\\{\\\"name\\\":\\\"A\\\",\\\"pos\\\":\\\"45,6\\\"\\}\"];\n    view { Column { RouteMap { label: \"Trip\" points: rows } } }\n}\n";
    let file = scratch("r.cui", src);
    let trace = scratch(
        "r.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"advance": 0, "expect": {"routemaps": [{"count": 1}]}}]}).to_string(),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let report: Value = serde_json::from_str(&stdout).expect("JSON");
    let entry = &report["steps"][0]["routemaps"][0];
    for key in ["zoom", "grid", "tiles", "attribution"] {
        assert!(
            !stdout.contains(&format!("\"{key}\":")) || entry.get(key).is_none(),
            "{key}: {entry}"
        );
    }
}

fn built(src: &str) -> String {
    let file = scratch("b.cui", src);
    let out = scratch("b.js", "");
    let (code, stdout, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

fn built_index(src: &str) -> String {
    let file = scratch("f.cui", src);
    let out = file.with_extension("out");
    let (code, stdout, stderr) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

#[test]
fn a_program_without_tiles_gets_no_tile_code_and_one_with_tiles_gets_it() {
    let plain = built(&comp(
        "",
        "RouteMap { label: \"Trip\" points: rows route: true }",
    ));
    for absent in [
        "__cui_routemap_tiled",
        "__cui_routemap_tile_plan",
        "__CUI_RMT_",
        "referrerpolicy",
        "<img",
        "createElement('img')",
    ] {
        assert!(!plain.contains(absent), "{absent}");
    }
    assert!(plain.contains("__cui_routemap(\"Trip\""));
    let js = built(&tiled("requires Network(\"tiles.example\");"));
    assert!(
        js.contains("function __cui_routemap_tile_plan(pts, tpl)"),
        "{js}"
    );
    assert!(js.contains("__cui_routemap_tiled(root, "), "{js}");
    // the base adapter is the same text with and without tiles
    let base = |m: &str| {
        let from = m
            .find("// Platform adapter: RouteMap (docs")
            .expect("adapter");
        let f = from
            + m[from..]
                .find("function __cui_routemap(")
                .expect("the function");
        let to = f + m[f..].find("\n}\n").expect("its end");
        m[from..to].to_string()
    };
    assert_eq!(base(&plain), base(&js));
}

#[test]
fn the_tile_code_sets_no_markup_and_asks_with_no_referrer() {
    let js = built(&tiled("requires Network(\"tiles.example\");"));
    let from = js
        .find("// Platform adapter: RouteMap tiles")
        .expect("tile adapter");
    let adapter = &js[from..];
    let tiled = adapter
        .find("function __cui_routemap_tiled(")
        .expect("the function");
    let adapter = &adapter[..tiled + adapter[tiled..].find("\n}\n").expect("its end")];
    for banned in [
        "innerHTML",
        "insertAdjacentHTML",
        "fetch(",
        "XMLHttpRequest",
        "new Image",
        "crossorigin",
        "crossOrigin",
        "'lazy'",
        "setAttribute('alt'",
    ] {
        assert!(!adapter.contains(banned), "{banned}");
    }
    assert!(adapter.contains("img.setAttribute('referrerpolicy', 'no-referrer')"));
    assert!(adapter.contains("img.alt = ''"));
    assert!(adapter.contains("credit.textContent = attribution"));
    // the one `http` in the drawing is the SVG namespace; an address is never written by the compiler
    assert!(!adapter.contains("openstreetmap"));
}

#[test]
fn a_page_with_a_tiled_route_map_lets_images_in_from_its_host_and_no_other() {
    let html = built_index(&tiled("requires Network(\"tiles.example\");"));
    assert!(
        html.contains("<meta http-equiv=\"Content-Security-Policy\" content=\"img-src 'self' data: https://tiles.example\">"),
        "{html}"
    );
    assert!(html.find("Content-Security-Policy") < html.find("<style>"));
    // a path on the page's own origin: the policy is there and lists no host
    let own = comp(
        "requires Network;",
        "RouteMap { label: \"Trip\" points: rows tiles: \"./tiles/{z}/{x}/{y}.png\" attribution: \"c\" }",
    );
    assert!(built_index(&own).contains("content=\"img-src 'self' data:\""));
    // a route map without tiles needs no policy
    let plain = comp("", "RouteMap { label: \"Trip\" points: rows }");
    assert!(!built_index(&plain).contains("Content-Security-Policy"));
}

#[test]
fn the_tile_host_is_authority_the_diff_reports() {
    let a = scratch(
        "a.cui",
        &comp("", "RouteMap { label: \"Trip\" points: rows }"),
    );
    let b = scratch("b.cui", &tiled("requires Network(\"tiles.example\");"));
    let (code, out, err) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert!(code == 0 || code == 5, "{code} {out}{err}");
    assert!(out.contains("Network"), "{out}");
    assert!(out.contains("tiles.example"), "{out}");
}

#[test]
fn the_example_replays_its_trace() {
    let (code, out, err) = run(&[
        "test",
        "examples/route-tiles/route-tiles.cui",
        "examples/route-tiles/route-tiles.trace.json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
}
