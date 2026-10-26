//! `Waffle` and `Chip` (docs/waffle.md): every refusal of `tests/waffle_corpus.json` is refused by its rule tag, the valid views check, the squares, the
//! caption, the text alternative and the table are in the page's HTML before any script, the squares are three paths and not a `<rect>` each, and
//! authority is unchanged. What the browser draws is checked by `scripts/check-waffle.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/waffle_corpus.json");
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
    let dir = common::scratch_root().join(format!("cancho-ui-waffle-{}-{n}", std::process::id()));
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
    "waffle-incomplete",
    "waffle-of-invalid",
    "waffle-counted-invalid",
    "waffle-columns-invalid",
    "waffle-rest-invalid",
    "waffle-has-children",
    "chip-tone-invalid",
    "chip-empty",
];
const OLD: &[&str] = &[
    "binding-unknown",
    "inline-outside-rich",
    "rich-child-invalid",
];

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
        assert!(
            README.contains(&format!("`{tag}`")),
            "`{tag}` is not documented"
        );
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
    let full = source(&c, &c["valid"][1]);
    let dir = scratch(&full).parent().expect("dir").to_path_buf();
    let mut cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).collect();
    cuts.push(full.len());
    for (n, at) in cuts.iter().enumerate() {
        let f = dir.join(format!("cut{n}.cui"));
        std::fs::write(&f, &full[..*at]).expect("cut");
        let (code, _, err) = run(&["check", p(&f)]);
        assert!(code == 0 || code == 1, "prefix {at}: exit {code}: {err}");
        if let Some(next) = cuts.get(n + 1) {
            let g = dir.join(format!("del{n}.cui"));
            std::fs::write(&g, format!("{}{}", &full[..*at], &full[*next..])).expect("cut");
            let (code, _, err) = run(&["check", p(&g)]);
            assert!(
                code == 0 || code == 1,
                "deleting at {at}: exit {code}: {err}"
            );
        }
    }
}

fn page(text: &str) -> String {
    let f = scratch(text);
    let out = f.parent().expect("dir").join("out");
    let (code, o, e) = run(&["build", p(&f), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

const WAFFLE: &str = "component C {\n    view { Column { Waffle { label: \"Tests pass\" of: 10 counted: 7 columns: 5 rest: \"known\" } Rich { \"a \" Chip { tone: \"warn\" \"alpha\" } } } }\n}\n";

#[test]
fn the_page_has_the_squares_the_caption_the_text_alternative_and_the_table() {
    let html = page(WAFFLE);
    let root = html
        .split("<body>")
        .nth(1)
        .expect("a body")
        .split("<script")
        .next()
        .expect("root");
    // 5 columns of 14 and a gap of 3: 5 * 17 - 3 = 82 wide, 2 rows: 31 high; seven squares counted, three outlined as known
    assert!(
        root.contains("<svg viewBox=\"0 0 82 31\" role=\"img\" aria-label=\"Tests pass: 7 of 10, 3 known differences\">"),
        "{root}"
    );
    assert!(root.contains("<path class=\"cui-wf-on\" d=\"M0 0h14v14h-14zM17 0h14v14h-14z"));
    assert_eq!(
        root.matches("<path ").count(),
        2,
        "one path a state, not an element a square"
    );
    assert!(root.contains("<path class=\"cui-wf-known\" d=\"M34 17h14v14h-14zM51 17h14v14h-14zM68 17h14v14h-14z\"></path>"), "{root}");
    assert!(root.contains("<figcaption>Tests pass</figcaption>"));
    assert!(root.contains("<details><summary>Data table</summary><table><tbody><tr><th scope=\"row\">Counted</th><td>7</td></tr><tr><th scope=\"row\">Known differences</th><td>3</td></tr><tr><th scope=\"row\">Squares</th><td>10</td></tr></tbody></table></details>"), "{root}");
    assert!(root.contains("<span class=\"cui-chip cui-chip-warn\">alpha</span>"));
    // nothing here needs script: no hole, no line about script
    assert!(!html.contains("need JavaScript"), "{html}");
}

#[test]
fn a_waffle_drawn_by_script_is_the_waffle_the_page_has() {
    // inside an If the module draws it: the same recipe, in the SVG namespace
    let f = scratch(
        "component C {\n    state shown: Bool = true;\n    view { If { when: shown Waffle { label: \"a\" of: 4 counted: 2 columns: 2 } } }\n}\n",
    );
    let (code, js, e) = run(&["build", p(&f)]);
    assert_eq!(code, 0, "{e}");
    assert!(
        js.contains("createElementNS('http://www.w3.org/2000/svg', 'svg')"),
        "{js}"
    );
    assert!(js.contains("createElementNS('http://www.w3.org/2000/svg', 'path')"));
    assert!(js.contains("M0 0h14v14h-14zM17 0h14v14h-14z"));
}

#[test]
fn the_largest_waffle_is_about_a_hundred_kilobytes_in_nine_elements_not_five_thousand() {
    let html = page(
        "component C {\n    view { Waffle { label: \"a\" of: 5000 counted: 2500 columns: 100 } }\n}\n",
    );
    assert!(html.len() < 140_000, "{}", html.len());
    assert_eq!(html.matches("<path ").count(), 2);
    assert!(!html.contains("<rect"));
}

#[test]
fn drawing_changes_authority_by_nothing() {
    let base = scratch(WAFFLE);
    let (_, out, _) = run(&["inspect", "effects", p(&base), "C"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["requires"], serde_json::json!([]));
    assert_eq!(v["reachable_effects"], serde_json::json!([]));
    let cand = base.parent().expect("dir").join("c.cui");
    std::fs::write(&cand, WAFFLE.replace("counted: 7", "counted: 8")).expect("candidate");
    let (code, human, _) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
}
