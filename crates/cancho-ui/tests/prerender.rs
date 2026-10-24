//! Prerender (docs/prerender.md): the static part of the mounted component is written into `index.html`; what is left to the script is listed;
//! what a visitor without script is told depends on what is missing. The DOM itself is checked in Chromium against the structure corpus
//! (`scripts/check-structure.js` part G); this is the build, the page's text around it and the numbers.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

mod common;

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("prerender-{n}"));
    std::fs::create_dir_all(&dir).expect("a directory");
    let path = dir.join("p.cui");
    std::fs::write(&path, text).expect("a file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8")
}

/// Build with extra flags and return `index.html`.
fn page(source: &str, flags: &[&str]) -> String {
    let file = scratch(source);
    let out = file.parent().expect("a directory").join("out");
    let mut args = vec!["build", p(&file)];
    args.extend_from_slice(flags);
    args.extend_from_slice(&["-o", p(&out)]);
    let (code, _, err) = run(&args);
    assert_eq!(code, 0, "{err}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

fn root_of(html: &str) -> &str {
    let start = html.find("<div id=\"root\">").expect("a root") + "<div id=\"root\">".len();
    let end = html[start..].find("</div>\n").map(|i| start + i);
    // the root holds nested divs: take up to the line that ends it
    let end = html[start..]
        .find("</div>\n<")
        .map(|i| start + i)
        .or(end)
        .expect("the end of the root");
    &html[start..end]
}

const STATIC: &str = "component C {\n    view { Main { Section { Heading { level: 1 \"Title\" } Rich { \"a \" Strong { \"b\" } } } } }\n}\n";
const WITH_ID: &str = "component C {\n    view { Main { Section { id: \"top\" Heading { level: 1 \"Title\" } } } }\n}\n";
const WITH_HOLE: &str = "component C {\n    state n: Int = 0;\n    on Up { n = n + 1; }\n    view { Main { Heading { level: 1 \"Count\" } Button { label: \"Up\" on: Up } Text { \"{n}\" } } }\n}\n";
const ALL_HOLE: &str = "component C {\n    state n: Int = 0;\n    on Up { n = n + 1; }\n    view { Button { label: \"Up\" on: Up } }\n}\n";

#[test]
fn a_static_program_is_in_the_page_and_says_nothing_about_script() {
    let html = page(STATIC, &["--page"]);
    assert!(
        html.contains("<div id=\"root\"><main><section><h1>Title</h1><p>a <strong>b</strong></p></section></main></div>"),
        "{html}"
    );
    assert!(!html.contains("<noscript>"), "nothing is missing: {html}");
    assert!(
        !html.contains("scrollIntoView"),
        "no id, no fragment script"
    );
}

#[test]
fn a_program_with_an_id_goes_to_its_fragment_before_the_script_runs() {
    let html = page(WITH_ID, &["--page"]);
    assert_eq!(html.matches("scrollIntoView").count(), 1, "{html}");
    let root = html.find("<div id=\"root\">").expect("root");
    let hash = html.find("scrollIntoView").expect("hash");
    let module = html.find("type=\"module\"").expect("module");
    assert!(
        root < hash && hash < module,
        "after the root, before the module script"
    );
}

#[test]
fn holes_are_left_to_the_script_and_the_visitor_is_told_so() {
    let html = page(WITH_HOLE, &["--page"]);
    let root = root_of(&html);
    assert_eq!(root, "<main><h1>Count</h1></main>", "no button, no number");
    assert!(
        html.contains("<noscript>Some parts of this page need JavaScript.</noscript>"),
        "{html}"
    );
    assert!(!html.contains("This app needs JavaScript."));
}

#[test]
fn a_program_with_nothing_static_keeps_the_old_line() {
    for flags in [&["--page"][..], &["--pwa"][..]] {
        let html = page(ALL_HOLE, flags);
        assert!(html.contains("<div id=\"root\"></div>"), "{html}");
        assert!(html.contains("<noscript>This app needs JavaScript.</noscript>"));
    }
}

#[test]
fn a_page_without_the_module_mounting_itself_is_refused_so_the_root_is_always_the_components() {
    let file = scratch(STATIC);
    let out = file.parent().expect("a directory").join("out");
    let (code, _, err) = run(&[
        "build",
        p(&file),
        "--page",
        "--no-auto-mount",
        "-o",
        p(&out),
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("cannot be combined"), "{err}");
}

#[test]
fn an_app_gets_the_same_page_and_the_worker_still_caches_it() {
    let html = page(STATIC, &["--pwa"]);
    assert!(html.contains("<main><section><h1>Title</h1>"), "{html}");
    let file = scratch(STATIC);
    let out = file.parent().expect("a directory").join("app");
    let (code, _, err) = run(&["build", p(&file), "--pwa", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let sw = std::fs::read_to_string(out.join("sw.js")).expect("sw.js");
    assert!(sw.contains("\"./index.html\""), "{sw}");
}

#[test]
fn the_programs_text_is_text_and_never_a_token_of_the_page() {
    let source = "component C {\n    view { Rich { \"@CSP@ @TITLE@ @THEME_CSS@ @ROOT@ @APP_REGISTER@ </script> <!-- <noscript> & \\\"q\\\"\" } }\n}\n";
    let file = scratch(source);
    let out = file.parent().expect("a directory").join("out");
    // the name of the page is a token too
    let (code, _, err) = run(&[
        "build",
        p(&file),
        "--page",
        "--name",
        "x @ROOT@ <b>",
        "-o",
        p(&out),
    ]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(
        html.contains("<p>@CSP@ @TITLE@ @THEME_CSS@ @ROOT@ @APP_REGISTER@ &lt;/script&gt; &lt;!-- &lt;noscript&gt; &amp; \"q\"</p>"),
        "{html}"
    );
    assert_eq!(html.matches("<div id=\"root\">").count(), 1);
    assert!(html.contains("<title>x @ROOT@ &lt;b&gt;</title>"), "{html}");
    assert_eq!(html.matches("</html>").count(), 1);
}

#[test]
fn inspect_prerender_lists_what_is_left_to_the_script_and_why() {
    let file = scratch(WITH_HOLE);
    let (code, out, _) = run(&["inspect", "prerender", p(&file), "C"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["mounted"], true);
    assert_eq!(v["static_elements"], 2);
    let holes: Vec<&str> = v["holes"]
        .as_array()
        .expect("holes")
        .iter()
        .map(|h| h["what"].as_str().expect("what"))
        .collect();
    assert_eq!(holes, vec!["Button", "Text"]);
    assert!(
        v["holes"][1]["reason"]
            .as_str()
            .is_some_and(|r| r.contains("reads state"))
    );
    assert_eq!(v["html_bytes"], "<main><h1>Count</h1></main>".len());
    // a component that is not the last one is not mounted into the page
    let two = "component A {\n    view { Text { \"a\" } }\n}\n\ncomponent B {\n    view { Text { \"b\" } }\n}\n";
    let file = scratch(two);
    let (_, out, _) = run(&["inspect", "prerender", p(&file), "A"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["mounted"], false);
    let (code, _, _) = run(&["inspect", "prerender", p(&file), "Nope"]);
    assert_ne!(code, 0);
}

#[test]
fn a_static_element_costs_under_a_hundred_bytes_of_html() {
    // 60 blocks of 9 elements each: the measured cost is about 49 bytes an element (docs/prerender.md section 8)
    let mut blocks = String::new();
    for i in 0..60 {
        blocks.push_str(&format!("Section {{ id: \"s{i}\" Heading {{ level: 2 \"Section number {i}, a heading of ordinary length\" }} Rich {{ \"Every command here was run before it went on this page. \" Strong {{ \"cancho\" }} \" is pre-1.0, \" Link {{ to: \"https://github.com/alpibrusl/cancho\" \"the compiler\" }} \" needs Rust, and \" Code {{ \"cargo build\" }} \" builds it.\" }} Code {{ \"git clone https://github.com/alpibrusl/cancho\\ncargo build -p cancho --release\" }} }}\n"));
    }
    let source = format!("component C {{\n    view {{ Main {{\n{blocks} }} }}\n}}\n");
    let file = scratch(&source);
    let (code, out, err) = run(&["inspect", "prerender", p(&file), "C"]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    let (bytes, els) = (
        v["html_bytes"].as_f64().expect("bytes"),
        v["static_elements"].as_f64().expect("elements"),
    );
    assert!(els >= 540.0, "{els}");
    assert!(bytes / els < 100.0, "{} bytes an element", bytes / els);
    assert!(
        bytes / els > 30.0,
        "{} bytes an element: the markup is not all there",
        bytes / els
    );
    assert_eq!(v["holes"], serde_json::json!([]));
}

#[test]
fn text_the_html_parser_would_change_never_reaches_a_panic() {
    // every one of these builds; the browser-side equality is checked in part G
    for text in [
        "\u{0}",
        "a\u{0}b",
        "</script>",
        "<!--",
        "-->",
        "&amp; &#0; &",
        "\r",
        "a\r\nb",
        "\\n",
        "<pre>\\n</pre>",
        "\u{2028}\u{feff}\u{202e}",
        "\\\"'`",
    ] {
        for wrap in [
            "Rich { \"T\" }",
            "Heading { level: 2 \"T\" }",
            "Code { \"T\" }",
            "Text { \"T\" }",
            "Link { to: \"https://a.example/\" \"T\" }",
        ] {
            let source = format!(
                "component C {{\n    view {{ Column {{ {} }} }}\n}}\n",
                wrap.replace('T', text)
            );
            let file = scratch(&source);
            let (code, out, err) = run(&["inspect", "prerender", p(&file), "C"]);
            assert!(
                code == 0 || (code == 3 && err.contains("refused")),
                "{text:?} in {wrap}: {code} {err}"
            );
            if code == 0 {
                let v: Value = serde_json::from_str(&out).expect("JSON");
                assert!(v["html_bytes"].is_number());
            }
        }
    }
}

#[test]
fn the_gallery_demos_are_interactive_and_each_keeps_a_line_for_a_visitor_without_script() {
    // checked on 2026-10-24 with `inspect prerender` on each of the 13 examples, and the text of five of them read against it: every demo has a
    // control or a state-bound text, so every demo has a hole and keeps a line; five have some static text of their own (a label, a heading),
    // the other eight are controls and state only, so the page they get is the old one
    let some = ["notes", "ops-console", "upload", "charts", "reorder"];
    let demos = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/demo");
    let mut n = 0;
    for entry in std::fs::read_dir(&demos).expect("docs/demo").flatten() {
        let Ok(html) = std::fs::read_to_string(entry.path().join("index.html")) else {
            continue;
        };
        n += 1;
        let id = entry.file_name().to_string_lossy().into_owned();
        let want = if some.contains(&id.as_str()) {
            "<noscript>Some parts of this page need JavaScript.</noscript>"
        } else {
            "<noscript>This app needs JavaScript.</noscript>"
        };
        assert!(html.contains(want), "{id}: {want}");
        assert_eq!(
            html.contains("<div id=\"root\"></div>"),
            !some.contains(&id.as_str()),
            "{id}"
        );
    }
    assert_eq!(n, 13, "the gallery has 13 demos");
}
