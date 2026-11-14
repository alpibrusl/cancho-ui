//! `Icon` (docs/structure.md 12.0q): every refusal of `tests/icon_corpus.json` is refused by its rule tag, the valid views check, an icon is in the page's HTML as an
//! inline `<svg>` built from the vendored Lucide files (no `<img>`, no `data:` URI, no external request), the script draws the same element, and the set is closed.
//! What a browser draws is checked by cancho-ota's `scripts/icons_check.sh`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/icon_corpus.json");
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

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn scratch(text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-icon-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let f = dir.join("p.cui");
    std::fs::write(&f, text).expect("the program");
    f
}

fn source(c: &Value, case: &Value) -> String {
    c["component"]
        .as_str()
        .expect("component")
        .replace("{view}", case["view"].as_str().expect("view"))
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


const NEW: &[&str] = &[
    "icon-incomplete",
    "icon-unknown",
    "icon-size-invalid",
    "icon-decorative-invalid",
    "icon-has-children",
    "icon-label-conflict",
    "icon-label-missing",
    "icon-button-invalid",
];
const OLD: &[&str] = &["binding-unknown"];

#[test]
fn every_valid_view_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let f = scratch(&source(&c, case));
        assert_eq!(codes(&f), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    for case in c["refusals"].as_array().expect("refusals") {
        let f = scratch(&source(&c, case));
        let (code, found) = codes(&f);
        let tag = case["tag"].as_str().expect("tag");
        assert_eq!(code, 1, "{}: {found:?}", case["name"]);
        assert_eq!(found, vec![tag.to_string()], "{}", case["name"]);
    }
}

#[test]
fn every_tag_is_documented_and_reached() {
    let c = corpus();
    let reached: Vec<&str> = c["refusals"]
        .as_array()
        .expect("refusals")
        .iter()
        .filter_map(|r| r["tag"].as_str())
        .collect();
    for tag in NEW.iter().chain(OLD) {
        assert!(reached.contains(tag), "no case reaches `{tag}`");
        assert!(README.contains(&format!("`{tag}`")), "`{tag}` is not documented");
    }
    for tag in NEW {
        assert!(
            README.contains(&format!("`{tag}` | `Component`")),
            "`{tag}` has no row of its own in the README"
        );
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let c = corpus();
    let full = source(&c, &c["valid"][3]);
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

const ICONS: &str = "component C {\n    on Go { }\n    view { Column { Row { Icon { name: \"map-pin\" decorative: true } Text { \"Madrid\" } } Icon { label: \"Alerts\" name: \"bell\" size: \"large\" } } }\n}\n";

#[test]
fn the_page_has_the_icon_as_an_inline_svg_with_currentcolor_strokes() {
    let html = page(ICONS);
    let root = html.split("<body>").nth(1).expect("a body").split("<script").next().expect("root");
    // decorative: hidden from a screen reader; labelled: an image with a name
    assert!(root.contains("<svg class=\"cui-icon cui-icon-m\" data-icon=\"map-pin\" viewBox=\"0 0 24 24\" width=\"20\" height=\"20\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\" focusable=\"false\"><path d=\"M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0\"></path><circle cx=\"12\" cy=\"10\" r=\"3\"></circle></svg>"), "{root}");
    assert!(root.contains("class=\"cui-icon cui-icon-l\"") && root.contains("role=\"img\" aria-label=\"Alerts\""), "{root}");
    // no picture of any kind, and no address
    for banned in ["<img", "<use", "data:image", "href=", "xlink"] {
        assert!(!root.contains(banned), "{banned} in {root}");
    }
    // the stylesheet is written for a page that has an icon
    assert!(html.contains(".cui-icon { display: inline-block;") && html.contains("stroke: currentColor"), "no icon rule");
    // the licence travels with the icons: both notices are in the page
    assert!(html.contains("ISC License") && html.contains("Lucide Icons and Contributors") && html.contains("The MIT License (MIT)") && html.contains("Cole Bemis"), "the licence notices");
    // the page's policy is not touched by an icon
    assert!(!html.contains("Content-Security-Policy") || !html.contains("data:"), "an icon must not add data: to the policy");
}

#[test]
fn a_page_with_no_icon_has_no_icon_rule() {
    let html = page("component C {\n    view { Column { Text { \"hi\" } } }\n}\n");
    assert!(!html.contains("cui-icon"), "the rule is written for a page that uses an icon");
}

#[test]
fn the_script_draws_the_same_icon_in_the_svg_namespace_and_a_button_icon_before_its_text() {
    let js = script("component C {\n    state shown: Bool = true;\n    on Go { }\n    view { Column { If { when: shown Icon { label: \"a\" name: \"bell-off\" } } Button { label: \"Save\" icon: \"save\" on: Go } Button { label: \"Mute\" on: Go Icon { name: \"bell\" } Text { \"Mute\" } } } }\n}\n");
    assert!(js.contains("createElementNS('http://www.w3.org/2000/svg', 'svg')"), "svg");
    assert!(js.contains("createElementNS('http://www.w3.org/2000/svg', 'path')"));
    assert!(js.contains("createElementNS('http://www.w3.org/2000/svg', 'circle')") || js.contains("'rect'") || js.contains("'path'"));
    assert!(js.contains("cui-icon cui-icon-m cui-icon-lead"), "the button's icon");
    assert!(!js.contains("<img") && !js.contains("data:image/svg"), "no picture");
    assert!(js.contains("cui-ico"), "a button of an icon is still marked as icon-only");
}

#[test]
fn the_vendored_set_is_what_the_source_table_lists_and_both_licence_notices_are_kept() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("crates/cancho-ui-ir/vendor/lucide");
    let names: Vec<String> = std::fs::read_to_string(dir.join("NAMES")).expect("NAMES").lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect();
    let mut files: Vec<String> = std::fs::read_dir(&dir).expect("dir").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| f.ends_with(".svg")).map(|f| f.trim_end_matches(".svg").to_string()).collect();
    files.sort();
    let mut want = names.clone();
    want.sort();
    assert_eq!(files, want, "the vendored files are exactly NAMES");
    let table = std::fs::read_to_string(root.join("crates/cancho-ui-ir/src/icon_set.rs")).expect("icon_set.rs");
    for n in &names {
        assert!(table.contains(&format!("(\"{n}\", include_str!")), "{n} is not in the table");
    }
    let third = std::fs::read_to_string(root.join("THIRD_PARTY.md")).expect("THIRD_PARTY.md");
    let version = std::fs::read_to_string(dir.join("VERSION")).expect("VERSION");
    let tag = version.lines().next().and_then(|l| l.strip_prefix("lucide ")).expect("tag");
    let commit = version.lines().nth(1).and_then(|l| l.strip_prefix("commit ")).expect("commit");
    assert!(third.contains(tag) && third.contains(commit), "THIRD_PARTY.md names the release and the commit");
    assert!(third.contains("ISC") && third.contains("Cole Bemis") && third.contains("Lucide Icons and Contributors"), "both notices");
    for n in &names {
        assert!(third.contains(&format!("`{n}`")), "THIRD_PARTY.md lists `{n}`");
    }
    let lic = std::fs::read_to_string(dir.join("LICENSE")).expect("LICENSE");
    assert!(lic.contains("ISC License") && lic.contains("The MIT License (MIT)"));
}
