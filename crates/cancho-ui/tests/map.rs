//! `Map` (docs/maps.md, issue #81 step two): what it asks the tile host for (the corpus, checked here and in Chromium by `scripts/check-map.js`),
//! the tags it needs, and how replay reports it.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-map-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

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

fn replay(src: &str, steps: Value) -> (i32, Value) {
    let file = scratch("f.cui", src);
    let trace = scratch(
        "f.trace.json",
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    let report: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("a JSON report ({e}): {stdout}{stderr}"));
    (code, report)
}

const ROWS: &str = "[\"\\{\\\"name\\\":\\\"Sol\\\",\\\"pos\\\":\\\"40.41694,-3.70351\\\"\\}\", \"\\{\\\"name\\\":\\\"Retiro\\\",\\\"pos\\\":\\\"40.41531,-3.68446\\\"\\}\"]";

/// A component with a map; `map` is the body of the `Map { .. }`, `requires` its capability lines.
fn comp(requires: &str, map: &str) -> String {
    // a body of bindings is a `Map`'s; anything else is the view as written
    let map = if map.trim_start().starts_with("label: \"Places\"") {
        format!("Map {{ {map} }}")
    } else {
        map.to_string()
    };
    // a component that declares `Location` uses it
    let find = if requires.contains("Location") {
        "    state r: Result[String, String] = Ok(\"\");\n    on Find { r = await(location.current()); }\n"
    } else {
        ""
    };
    format!(
        "component M {{\n    {requires}\n    state pos: String = \"40.42,-3.70\";\n    state places: List[String] = {ROWS};\n{find}    on Go(p) {{ pos = p; }}\n    view {{ Column {{ {map} }} }}\n}}\n"
    )
}

const FULL: &str = "label: \"Places\" center: pos places: places zoom: 13 tiles: \"https://tiles.example/{z}/{x}/{y}.png\" attribution: \"Tiles: example\"";

fn full(requires: &str) -> String {
    comp(requires, &format!("Map {{ {FULL} }}"))
}

#[test]
fn a_map_with_a_declared_host_is_accepted() {
    accepted(&full("requires Network(\"tiles.example\");"));
    accepted(&full(
        "requires Network(\"TILES.example\", \"other.example\");",
    ));
    // a path on the page's own origin needs only the bare capability
    accepted(&comp(
        "requires Network;",
        &FULL.replace("https://tiles.example/{z}", "/tiles/{z}"),
    ));
    accepted(&comp(
        "requires Network(\"localhost:8099\");",
        &FULL.replace("https://tiles.example", "http://localhost:8099"),
    ));
    accepted(&comp(
        "requires Network;",
        &FULL
            .replace("center: pos", "center: \"40.42,-3.70\"")
            .replace("https://tiles.example/{z}", "./t/{z}"),
    ));
}

#[test]
fn the_host_is_the_programs_and_written_where_it_can_be_read() {
    // no Network at all, or a bare one for a host that is not this page: the host is not declared
    assert_eq!(codes(&full("")), ["map-host-undeclared"]);
    assert_eq!(codes(&full("requires Network;")), ["map-host-undeclared"]);
    assert_eq!(
        codes(&full("requires Network(\"other.example\");")),
        ["map-host-undeclared"]
    );
    // a port is part of the host
    assert_eq!(
        codes(&comp(
            "requires Network(\"localhost\");",
            &FULL.replace("https://tiles.example", "http://localhost:8099")
        )),
        ["map-host-undeclared"]
    );
    // a path on this page still needs the capability
    assert_eq!(
        codes(&comp(
            "",
            &FULL.replace("https://tiles.example/{z}", "/tiles/{z}")
        )),
        ["map-host-undeclared"]
    );
    // there is no default: the address is required and is a literal
    let no_tiles = FULL.replace(" tiles: \"https://tiles.example/{z}/{x}/{y}.png\"", "");
    assert_eq!(
        codes(&comp(
            "requires Network(\"tiles.example\");",
            &format!("Map {{ {no_tiles} }}")
        )),
        ["map-incomplete"]
    );
    let computed = FULL.replace("\"https://tiles.example/{z}/{x}/{y}.png\"", "pos");
    assert_eq!(
        codes(&comp(
            "requires Network(\"tiles.example\");",
            &format!("Map {{ {computed} }}")
        )),
        ["map-tiles-invalid"]
    );
}

#[test]
fn a_tile_address_must_be_one() {
    for bad in [
        "https://tiles.example/{z}/{x}.png",
        "https://tiles.example/{z}/{x}/{y}/{y}.png",
        "https://tiles.example/{z}/{x}/{y}/{w}.png",
        "http://tiles.example/{z}/{x}/{y}.png",
        "ftp://tiles.example/{z}/{x}/{y}.png",
        "//tiles.example/{z}/{x}/{y}.png",
        "tiles.example/{z}/{x}/{y}.png",
        "https://user:pw@tiles.example/{z}/{x}/{y}.png",
        "https://tiles.example/{z}/{x}/{y}.png#frag",
        "https://tiles.example/{z}/{x}/ {y}.png",
        "https://tile s.example/{z}/{x}/{y}.png",
        "https:///{z}/{x}/{y}.png",
        "https://tiles.example:/{z}/{x}/{y}.png",
        "https://tiles.exa_mple/{z}/{x}/{y}.png",
        "/",
        "./",
    ] {
        let src = comp(
            "requires Network;",
            &FULL.replace("https://tiles.example/{z}/{x}/{y}.png", bad),
        );
        assert_eq!(codes(&src), ["map-tiles-invalid"], "{bad}");
    }
}

#[test]
fn the_zoom_is_bounded_by_the_precision_of_the_position() {
    let net = "requires Network(\"tiles.example\");";
    for z in ["0", "13"] {
        accepted(&comp(
            &format!("{net} requires Location;"),
            &format!(
                "Map {{ {} }}",
                FULL.replace("zoom: 13", &format!("zoom: {z}"))
            ),
        ));
    }
    // a bare Location is a position rounded to 1 km: a tile finer than that says more than it knows
    for z in ["14", "19", "20", "-1"] {
        let src = comp(
            &format!("{net} requires Location;"),
            &format!(
                "Map {{ {} }}",
                FULL.replace("zoom: 13", &format!("zoom: {z}"))
            ),
        );
        assert_eq!(codes(&src), ["map-zoom-invalid"], "zoom {z}");
    }
    accepted(&comp(
        &format!("{net} requires Location(\"fine\");"),
        &format!("Map {{ {} }}", FULL.replace("zoom: 13", "zoom: 19")),
    ));
    accepted(&comp(
        &format!("{net} requires Location(\"coarse\");"),
        &format!("Map {{ {} }}", FULL),
    ));
    assert_eq!(
        codes(&comp(
            &format!("{net} requires Location(\"coarse\");"),
            &format!("Map {{ {} }}", FULL.replace("zoom: 13", "zoom: 15"))
        )),
        ["map-zoom-invalid"]
    );
    // no position from the device: the program's own, up to 19
    accepted(&comp(
        net,
        &format!("Map {{ {} }}", FULL.replace("zoom: 13", "zoom: 19")),
    ));
    // not a number written out
    assert_eq!(
        codes(&comp(
            net,
            &format!("Map {{ {} }}", FULL.replace("zoom: 13", "zoom: \"13\""))
        )),
        ["map-zoom-invalid"]
    );
    assert_eq!(
        codes(&comp(
            net,
            &format!("Map {{ {} }}", FULL.replace("zoom: 13", "zoom: 13 + 1"))
        )),
        ["map-zoom-invalid"]
    );
}

#[test]
fn a_map_needs_everything_that_makes_it_usable() {
    let net = "requires Network(\"tiles.example\");";
    for key in [
        "label: \"Places\" ",
        "center: pos ",
        "places: places ",
        "zoom: 13 ",
        "attribution: \"Tiles: example\"",
    ] {
        let src = comp(net, &format!("Map {{ {} }}", FULL.replace(key, "")));
        assert_eq!(codes(&src), ["map-incomplete"], "without {key}");
    }
}

#[test]
fn the_bindings_have_the_types_the_page_needs() {
    let net = "requires Network(\"tiles.example\");";
    let with = |from: &str, to: &str| comp(net, &format!("Map {{ {} }}", FULL.replace(from, to)));
    assert_eq!(
        codes(&with("center: pos", "center: places")),
        ["map-binding-invalid"]
    );
    assert_eq!(
        codes(&with("center: pos", "center: missing")),
        ["map-binding-invalid"]
    );
    assert_eq!(
        codes(&with("places: places", "places: pos")),
        ["map-binding-invalid"]
    );
    assert_eq!(codes(&with("places: places", "places: [\"a\"]")).len(), 1);
    assert_eq!(
        codes(&with(
            "attribution: \"Tiles: example\"",
            "attribution: \"\""
        )),
        ["map-binding-invalid"]
    );
    assert_eq!(
        codes(&with(
            "attribution: \"Tiles: example\"",
            &format!("attribution: \"{}\"", "x".repeat(201))
        )),
        ["map-binding-invalid"]
    );
    accepted(&with(
        "attribution: \"Tiles: example\"",
        &format!("attribution: \"{}\"", "x".repeat(200)),
    ));
    // a key a Map does not take, and a Map's keys on another element
    assert_eq!(
        codes(&with("zoom: 13", "zoom: 13 on: Go")),
        ["binding-unknown"]
    );
    for other in [
        "Button { label: \"b\" on: Go(\"x\") zoom: 3 }",
        "Button { label: \"b\" on: Go(\"x\") center: \"x\" }",
        "Column { places: \"x\" Text { \"x\" } }",
        "Button { label: \"b\" on: Go(\"x\") tiles: \"x\" }",
    ] {
        assert_eq!(codes(&comp(net, other)), ["binding-unknown"], "{other}");
    }
}

#[test]
fn the_corpus_is_what_replay_says() {
    let c: Value =
        serde_json::from_str(include_str!("../../../tests/map_corpus.json")).expect("JSON");
    let cases = c["cases"].as_array().expect("cases");
    assert!(cases.len() >= 25, "{}", cases.len());
    for case in cases {
        let rows: Vec<String> = case["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|r| r.as_str().expect("row").to_string())
            .collect();
        let got = cancho_ui_replay::map_plan(
            case["center"].as_str().expect("center"),
            &rows,
            case["template"].as_str().expect("template"),
            case["zoom"].as_i64().expect("zoom"),
            case["coarse"].as_bool().expect("coarse"),
        );
        let want = &case["want"];
        let why = case["why"].as_str().expect("why");
        assert_eq!(
            got.zoom,
            want["zoom"].as_i64().expect("zoom"),
            "{why}: zoom"
        );
        let tiles: Vec<&str> = want["tiles"]
            .as_array()
            .expect("tiles")
            .iter()
            .map(|t| t.as_str().expect("tile"))
            .collect();
        assert_eq!(got.tiles, tiles, "{why}: tiles");
        let list: Vec<(String, String)> = want["list"]
            .as_array()
            .expect("list")
            .iter()
            .map(|e| {
                (
                    e["name"].as_str().expect("name").to_string(),
                    e["km"].as_str().expect("km").to_string(),
                )
            })
            .collect();
        assert_eq!(got.list, list, "{why}: list");
        assert_eq!(
            got.listed as u64,
            want["listed"].as_u64().expect("listed"),
            "{why}: listed"
        );
        assert_eq!(
            got.total as u64,
            want["total"].as_u64().expect("total"),
            "{why}: total"
        );
        assert_eq!(
            got.pinned as u64,
            want["pinned"].as_u64().expect("pinned"),
            "{why}: pinned"
        );
    }
}

#[test]
fn replay_reports_the_exact_tiles_and_the_list() {
    let src = full("requires Network(\"tiles.example\");");
    let (code, report) = replay(
        &src,
        json!([{
            "event": "Go", "params": ["40.41694,-3.70351"],
            "state": {"pos": "40.41694,-3.70351"},
            "expect": {"maps": [{
                "label": "Places", "zoom": 13,
                "tiles": [
                    "https://tiles.example/13/4010/3087.png", "https://tiles.example/13/4011/3087.png", "https://tiles.example/13/4012/3087.png",
                    "https://tiles.example/13/4010/3088.png", "https://tiles.example/13/4011/3088.png", "https://tiles.example/13/4012/3088.png",
                    "https://tiles.example/13/4010/3089.png", "https://tiles.example/13/4011/3089.png", "https://tiles.example/13/4012/3089.png"
                ],
                "list": [{"name": "Sol", "km": "0"}, {"name": "Retiro", "km": "1.6"}],
                "listed": 2, "total": 2, "pinned": 2
            }]}
        }]),
    );
    assert_eq!(code, 0, "{report:#}");
    // a tile the page would not ask for is a failure of the trace, not of the page
    let (code, _) = replay(
        &src,
        json!([{"event": "Go", "params": ["40.41694,-3.70351"], "state": {"pos": "40.41694,-3.70351"},
                "expect": {"maps": [{"tiles": ["https://tiles.example/13/1/1.png"]}]}}]),
    );
    assert_eq!(code, 4);
}

fn built_index(src: &str, mode: &str) -> String {
    let file = scratch("f.cui", src);
    let out = file.with_extension("out");
    let (code, stdout, stderr) = run(&["build", p(&file), mode, "-o", p(&out)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

#[test]
fn a_page_with_a_map_lets_images_in_from_the_hosts_it_declared_and_no_others() {
    let src = full("requires Network(\"tiles.example\");");
    for mode in ["--page", "--pwa"] {
        let html = built_index(&src, mode);
        let want = "<meta http-equiv=\"Content-Security-Policy\" content=\"img-src 'self' data: https://tiles.example\">";
        assert!(html.contains(want), "{mode}: {html}");
        // before anything in the head that could ask for an image
        assert!(
            html.find("Content-Security-Policy") < html.find("<style>"),
            "{mode}"
        );
        assert!(
            html.find("Content-Security-Policy") < html.find("<title>"),
            "{mode}"
        );
    }
    // a path on the page's own origin: the policy is there and lists no host
    let own = comp(
        "requires Network;",
        &FULL.replace("https://tiles.example/{z}", "/tiles/{z}"),
    );
    assert!(built_index(&own, "--page").contains("content=\"img-src 'self' data:\""));
    // a localhost stand-in keeps its scheme and port
    let local = comp(
        "requires Network(\"localhost:8099\");",
        &FULL.replace("https://tiles.example", "http://localhost:8099"),
    );
    assert!(built_index(&local, "--page").contains("img-src 'self' data: http://localhost:8099\""));
}

#[test]
fn two_maps_on_one_host_name_it_once_and_a_page_without_a_map_has_no_policy() {
    let two = format!(
        "component M {{\n    requires Network(\"tiles.example\", \"other.example\");\n    state pos: String = \"40.42,-3.70\";\n    state places: List[String] = {ROWS};\n    view {{ Column {{ Map {{ {FULL} }} Map {{ {} }} Map {{ {} }} }} }}\n}}\n",
        FULL.replace("Places", "Second"),
        FULL.replace("tiles.example", "other.example"),
    );
    let html = built_index(&two, "--page");
    assert!(
        html.contains("img-src 'self' data: https://tiles.example https://other.example\""),
        "{html}"
    );
    let plain = "component C {\n    state n: Int = 0;\n    on Inc { n = n + 1; }\n    view { Column { Button { label: \"Add\" on: Inc } } }\n}\n";
    assert!(!built_index(plain, "--page").contains("Content-Security-Policy"));
}

#[test]
fn the_compiler_names_no_tile_provider() {
    // no default host: nothing the compiler writes names a map provider, and a map with no address is refused
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
    for dir in [
        "cancho-ui-ir",
        "cancho-ui-codegen-web",
        "cancho-ui-replay",
        "cancho-ui",
    ] {
        for entry in walk(&root.join(dir).join("src")) {
            let text = std::fs::read_to_string(&entry).expect("source");
            for provider in [
                "openstreetmap",
                "tile.osm",
                "mapbox",
                "maptiler",
                "stadiamaps",
                "googleapis",
                "arcgis",
                "carto",
            ] {
                assert!(
                    !text.to_ascii_lowercase().contains(provider),
                    "{} names {provider}",
                    entry.display()
                );
            }
        }
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).expect("dir") {
        let path = e.expect("entry").path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    out
}
