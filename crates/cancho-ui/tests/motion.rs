//! `Reveal`, `CountUp` and `visible:` / `hidden:` (docs/motion.md): every refusal of `tests/motion_corpus.json` is refused by its rule tag, the valid
//! views check, the page's HTML holds every block and every final number before any script, the script is in the page only for a program that uses
//! it, and the module tells the program by the name written on the element. What the browser does is checked by `scripts/check-motion.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/motion_corpus.json");
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
    let dir = common::scratch_root().join(format!("cancho-ui-motion-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let f = dir.join("p.cui");
    std::fs::write(&f, text).expect("the program");
    f
}

fn source(c: &Value, case: &Value) -> String {
    if let Some(whole) = case["program"].as_str() {
        return whole.to_string();
    }
    c["component"]
        .as_str()
        .expect("component")
        .replace("{state}", case["extra_state"].as_str().unwrap_or(""))
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

const TAGS: &[&str] = &[
    "countup-not-number",
    "countup-out-of-range",
    "countup-has-children",
    "visible-event-invalid",
    "visible-dynamic",
    "inline-outside-rich",
    "rich-child-invalid",
    "binding-unknown",
];
const NEW: &[&str] = &[
    "countup-not-number",
    "countup-out-of-range",
    "countup-has-children",
    "visible-event-invalid",
    "visible-dynamic",
];

#[test]
fn every_valid_view_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        // the view must hold a `shown` for the If of a refusal; a valid view needs only the events
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
    for tag in TAGS {
        assert!(
            README.contains(&format!("`{tag}`")),
            "`{tag}` is not documented"
        );
        assert!(reached.contains(tag), "no case reaches `{tag}`");
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
    let full = source(&c, &c["valid"][5]);
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

const MOTION: &str = "component C {\n    state n: Int = 0;\n    on Seen { n = n + 1; }\n    view { Column { Main { id: \"m\" Reveal { Section { visible: Seen Rich { \"Total \" CountUp { to: 1380 } \".\" } } } } } }\n}\n";

#[test]
fn the_html_has_every_block_and_every_final_number_before_any_script() {
    let html = page(MOTION);
    let body = html.split("<body>").nth(1).expect("a body");
    let root = body.split("<script").next().expect("root");
    assert!(
        root.contains("<div class=\"cui-reveal\"><section data-cui-visible=\"Seen\"><p>Total <span data-to=\"1380\" class=\"cui-countup\">1380</span>.</p></section></div>"),
        "{root}"
    );
    // nothing of the script's own state is in what the page was written with
    for state in ["cui-armed", "cui-in", "data-frame", "style="] {
        assert!(!root.contains(state), "{state}: {root}");
    }
    // the section is static: not a hole, so a visitor with no script has it
    assert!(
        !root.contains("<!--cui:") || !root.contains("Total <!--"),
        "{root}"
    );
}

#[test]
fn the_script_is_in_the_page_only_for_a_program_that_uses_it() {
    let plain = page("component C {\n    view { Text { \"x\" } }\n}\n");
    assert!(
        !plain.contains("IntersectionObserver"),
        "no script for a page that does not ask"
    );
    let html = page(MOTION);
    assert!(html.contains("IntersectionObserver"));
    // the stylesheet hides only an element the script armed, and never under reduced motion or in print
    assert!(html.contains(".cui-reveal.cui-armed:not(.cui-in) { opacity: 0;"));
    assert!(html.contains("@media (prefers-reduced-motion: reduce), print {"));
    assert!(html.contains("content: attr(data-frame) / \"\""));
    let reveal_only = page("component C {\n    view { Reveal { Text { \"x\" } } }\n}\n");
    assert!(
        reveal_only.contains("cui-armed")
            && !reveal_only.contains("data-cui-visible=\"")
            && !reveal_only.contains("'.cui-countup[data-to]'")
    );
}

#[test]
fn the_module_tells_the_program_by_the_name_written_on_the_element() {
    let f = scratch(MOTION);
    let (code, js, e) = run(&["build", p(&f)]);
    assert_eq!(code, 0, "{e}");
    assert!(
        js.contains("__cui_visible_hook({ Seen: __on_Seen })"),
        "{js}"
    );
    assert!(js.contains("function __cui_visible_hook("));
    let (_, none, _) = run(&[
        "build",
        p(&scratch("component C {\n    view { Text { \"x\" } }\n}\n")),
    ]);
    assert!(!none.contains("__cui_visible_hook"));
}

#[test]
fn motion_changes_authority_and_the_network_by_nothing() {
    let base = scratch(MOTION);
    let (_, out, _) = run(&["inspect", "effects", p(&base), "C"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["requires"], serde_json::json!([]));
    assert_eq!(v["reachable_effects"], serde_json::json!([]));
    let cand = base.parent().expect("dir").join("c.cui");
    std::fs::write(&cand, MOTION.replace("to: 1380", "to: 1381")).expect("candidate");
    let (code, human, _) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
}
