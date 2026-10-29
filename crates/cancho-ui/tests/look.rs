//! The roles that carry the look of a page (docs/www-port/look.md section 3): `card: true`, a `Grid`'s `align:`, `appearance: "pill"`, a sticky header's end
//! group, and a page's `measure:` and `scale:`. Each is accepted and refused with its tag, is in the DOM and the stylesheet only for a program that says so, and
//! leaves every other page as it was.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(tag: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        common::scratch_root().join(format!("cancho-ui-look-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn program(page: &str, view: &str) -> String {
    format!("{page}\ncomponent App {{\n    view {{\n        Column {{ {view} }}\n    }}\n}}\n")
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let f = dir.join(name);
    std::fs::write(&f, text).expect("write");
    f
}

fn codes(text: &str) -> Vec<String> {
    let dir = scratch("check");
    let f = write(&dir, "p.cui", text);
    let (code, out, err) = run(
        &dir,
        &["check", f.to_str().expect("utf8"), "--output", "json"],
    );
    assert!(code == 0 || code == 1, "{out}{err}");
    if code == 0 {
        return Vec::new();
    }
    let v: serde_json::Value = serde_json::from_str(&out).expect("json");
    v.as_array()
        .expect("diagnostics")
        .iter()
        .map(|d| d["code"].as_str().expect("code").to_string())
        .collect()
}

fn page_of(text: &str) -> (String, String) {
    let dir = scratch("build");
    let f = write(&dir, "p.cui", text);
    let out = dir.join("out");
    let (code, o, e) = run(
        &dir,
        &[
            "build",
            f.to_str().expect("utf8"),
            "--page",
            "-o",
            out.to_str().expect("utf8"),
        ],
    );
    assert_eq!(code, 0, "{o}{e}");
    (
        std::fs::read_to_string(out.join("index.html")).expect("page"),
        std::fs::read_to_string(out.join("app.js")).expect("script"),
    )
}

const PAGE: &str = "page { title: \"T\"; lang: \"en\"; }";
const ROLES: &str = "Article { card: true Text { \"a\" } } Grid { min: 17 align: \"center\" Text { \"a\" } Text { \"b\" } } Link { to: \"https://a.example/\" appearance: \"pill\" \"Pill\" }";

#[test]
fn a_program_that_uses_none_of_the_roles_is_the_page_it_was() {
    let (html, js) = page_of(&program(PAGE, "Text { \"x\" }"));
    for word in [
        "cui-look",
        ".cui-card",
        "cui-align-center",
        "cui-link-pill",
        "--cui-measure",
        "cui-row { margin-inline-start",
    ] {
        assert!(!html.contains(word) && !js.contains(word), "{word}");
    }
}

#[test]
fn the_roles_are_in_the_dom_and_their_rules_are_written_once() {
    let (html, js) = page_of(&program(PAGE, ROLES));
    assert_eq!(html.matches(".cui-card {").count(), 1, "{html}");
    assert_eq!(html.matches(".cui-align-center {").count(), 1);
    assert_eq!(html.matches("a.cui-link-pill {").count(), 1);
    // a pill is a 44 px target (the drawn pill is smaller: the transparent border is the target)
    assert!(html.contains("min-height: 44px; padding: 0 0.6rem; border: 7px solid transparent"));
    // the first render is the page's HTML (prerendered) and the script draws the same classes on a later one
    for class in [
        "class=\"cui-card\"",
        "cui-grid cui-align-center",
        "cui-link-pill",
    ] {
        assert!(
            html.contains(class) || js.contains(class),
            "{class}\n{html}"
        );
    }
    // none of the page-level rules: a page without `measure:` and `scale:` has no such block of them
    assert!(!html.contains("--cui-measure") && !html.contains("clamp(2.4rem"));
}

#[test]
fn a_sticky_header_has_its_end_group() {
    let (html, _) = page_of(&program(
        PAGE,
        "Header { sticky: true Link { to: \"#a\" \"cancho\" } Row { Link { to: \"https://a.example/\" \"GitHub\" } } } Main { id: \"main\" Section { id: \"a\" Text { \"x\" } } }",
    ));
    assert!(html.contains(".cui-sticky > .cui-row { margin-inline-start: auto; }"));
}

#[test]
fn the_roles_are_refused_with_their_tags() {
    for (what, view, tag) in [
        (
            "card: a text",
            "Article { card: \"yes\" Text { \"a\" } }",
            "card-invalid",
        ),
        (
            "card: a number",
            "Section { card: 1 Text { \"a\" } }",
            "card-invalid",
        ),
        (
            "card on a column",
            "Column { card: true Text { \"a\" } }",
            "binding-unknown",
        ),
        (
            "card on a heading",
            "Heading { level: 2 card: true \"h\" }",
            "binding-unknown",
        ),
        (
            "an alignment that is not one",
            "Grid { min: 17 align: \"left\" Text { \"a\" } }",
            "grid-align-invalid",
        ),
        (
            "an alignment that is empty",
            "Grid { min: 17 align: \"\" Text { \"a\" } }",
            "grid-align-invalid",
        ),
        (
            "align on a row",
            "Row { align: \"center\" Text { \"a\" } }",
            "binding-unknown",
        ),
        (
            "an appearance that is not one",
            "Link { to: \"https://a.example/\" appearance: \"badge\" \"x\" }",
            "link-appearance-invalid",
        ),
    ] {
        assert_eq!(codes(&program(PAGE, view)), vec![tag.to_string()], "{what}");
    }
    // the accepted spellings
    for view in [
        "Article { card: false Text { \"a\" } }",
        "Aside { card: true Text { \"a\" } }",
        "Figure { card: true Text { \"a\" } }",
        "Grid { min: 17 align: \"stretch\" Text { \"a\" } }",
        "Grid { min: 17 weights: [1, 2] align: \"center\" gap: 2 Text { \"a\" } Text { \"b\" } }",
    ] {
        assert_eq!(codes(&program(PAGE, view)), Vec::<String>::new(), "{view}");
    }
}

#[test]
fn a_page_measure_and_scale_are_checked() {
    for (entry, tag) in [
        ("measure: \"29\";", "page-measure-invalid"),
        ("measure: \"101\";", "page-measure-invalid"),
        ("measure: \"\";", "page-measure-invalid"),
        ("measure: \"72rem\";", "page-measure-invalid"),
        ("measure: \"+72\";", "page-measure-invalid"),
        ("measure: \"7 2\";", "page-measure-invalid"),
        ("measure: \"-72\";", "page-measure-invalid"),
        ("scale: \"big\";", "page-scale-invalid"),
        ("scale: \"\";", "page-scale-invalid"),
        ("measure: \"72\"; measure: \"80\";", "page-entry-duplicate"),
        (
            "scale: \"display\"; scale: \"display\";",
            "page-entry-duplicate",
        ),
    ] {
        let page = format!("page {{ title: \"T\"; lang: \"en\"; {entry} }}");
        assert_eq!(
            codes(&program(&page, "Text { \"x\" }")),
            vec![tag.to_string()],
            "{entry}"
        );
    }
    for entry in [
        "measure: \"30\";",
        "measure: \"100\";",
        "scale: \"display\";",
    ] {
        let page = format!("page {{ title: \"T\"; lang: \"en\"; {entry} }}");
        assert!(
            codes(&program(&page, "Text { \"x\" }")).is_empty(),
            "{entry}"
        );
    }
}

#[test]
fn a_page_that_declares_measure_and_scale_has_their_rules_and_only_theirs() {
    let page = "page { title: \"T\"; lang: \"en\"; measure: \"72\"; scale: \"display\"; }";
    let (html, _) = page_of(&program(page, "Text { \"x\" }"));
    assert!(html.contains("<style id=\"cui-look\">"), "{html}");
    assert!(html.contains("--cui-measure: 72rem;"));
    assert!(html.contains("h1 { font-size: clamp(2.4rem, 6.4vw, 3.5rem)"));
    // the column is wider than a phone only when the window is: the gutter is never under 1.25 rem
    assert!(html.contains("max(1.25rem,"));
    // measure alone has no type scale, scale alone has no column
    let (m, _) = page_of(&program(
        "page { title: \"T\"; lang: \"en\"; measure: \"60\"; }",
        "Text { \"x\" }",
    ));
    assert!(m.contains("--cui-measure: 60rem;") && !m.contains("clamp(2.4rem"));
    let (s, _) = page_of(&program(
        "page { title: \"T\"; lang: \"en\"; scale: \"display\"; }",
        "Text { \"x\" }",
    ));
    assert!(!s.contains("--cui-measure") && s.contains("clamp(2.4rem"));
    // and it comes before the theme (the theme's colours win), and after the baseline
    assert!(html.find("cui-look").expect("look") > html.find("</style>").expect("baseline"));
}

#[test]
fn a_changed_measure_or_scale_is_a_change_of_the_page() {
    let dir = scratch("diff");
    let a = write(
        &dir,
        "a.cui",
        &program(
            "page { title: \"T\"; lang: \"en\"; measure: \"72\"; }",
            "Text { \"x\" }",
        ),
    );
    let b = write(
        &dir,
        "b.cui",
        &program(
            "page { title: \"T\"; lang: \"en\"; measure: \"80\"; scale: \"display\"; }",
            "Text { \"x\" }",
        ),
    );
    let (code, out, err) = run(
        &dir,
        &["diff", a.to_str().expect("utf8"), b.to_str().expect("utf8")],
    );
    assert_eq!(code, 5, "{out}{err}");
    assert!(out.contains("~ page measure: 72 -> 80"), "{out}");
    assert!(out.contains("+ page scale = display"), "{out}");
    assert!(out.contains("authority: unchanged"), "{out}");
}

#[test]
fn the_theme_draws_the_roles_with_colours_and_shadows_only() {
    let c: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/theme_corpus.json")).expect("json");
    let _ = c; // the palette of the example's own theme is used below
    let theme = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/theme/theme.cui"),
    )
    .expect("the example");
    let dir = scratch("themed");
    std::fs::create_dir_all(dir.join("fonts")).expect("fonts");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/theme/fonts/jetbrains-mono-latin-wght-normal.woff2"),
        dir.join("fonts/jetbrains-mono-latin-wght-normal.woff2"),
    )
    .expect("font");
    // the example's theme block, with a view that uses every role
    let start = theme.find("theme {").expect("theme");
    let mut depth = 0;
    let mut end = start;
    for (i, ch) in theme[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let text = program(
        &format!("{PAGE}\n{}", &theme[start..end]),
        &format!(
            "Header {{ sticky: true Link {{ to: \"#a\" \"cancho\" }} }} {ROLES} Main {{ id: \"main\" Section {{ id: \"a\" Text {{ \"x\" }} }} }}"
        ),
    );
    let f = write(&dir, "t.cui", &text);
    let out = dir.join("out");
    let (code, o, e) = run(
        &dir,
        &[
            "build",
            f.to_str().expect("utf8"),
            "--page",
            "-o",
            out.to_str().expect("utf8"),
        ],
    );
    assert_eq!(code, 0, "{o}{e}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("page");
    let at = html.find("<style id=\"cui-theme\">").expect("theme");
    let theme_css = &html[at..];
    assert!(theme_css.contains(".cui-card { background: var(--cui-surface); border-color: transparent; box-shadow: var(--cui-shadow); }"));
    assert!(theme_css.contains("backdrop-filter: blur(10px)"));
    assert!(theme_css.contains(
        "a.cui-link-pill { color: var(--cui-warn); background-color: var(--cui-warn-bg);"
    ));
    // the baseline's padding, radius and 44 px are not restated, and so cannot be taken away, by the theme
    for forbidden in [
        "padding",
        "border-radius",
        "min-height",
        "{ display",
        "; display",
        "width:",
        "height:",
    ] {
        assert!(!theme_css.contains(forbidden), "{forbidden}");
    }
    // the baseline's own card rule comes first
    assert!(html.find(".cui-card { box-sizing").expect("baseline") < at);
}
