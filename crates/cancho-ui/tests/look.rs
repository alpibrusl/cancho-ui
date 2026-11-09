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
    assert_eq!(html.matches("\n.cui-card {").count(), 1, "{html}");
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
    assert!(!html.contains("--cui-measure") && !html.contains("clamp(2.1rem"));
}

#[test]
fn a_sticky_header_has_its_end_group() {
    let (html, _) = page_of(&program(
        PAGE,
        "Header { sticky: true Link { to: \"#a\" \"cancho\" } Row { Link { to: \"https://a.example/\" \"GitHub\" } } } Main { id: \"main\" Section { id: \"a\" Text { \"x\" } } }",
    ));
    assert!(
        html.contains(".cui-sticky > .cui-row { margin-inline-start: auto; align-self: center; }")
    );
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
            "card on a nav",
            "Nav { card: true Link { to: \"#a\" \"a\" } }",
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
    assert!(html.contains("h1 { font-size: clamp(2.1rem, 4.6vw, 3.05rem)"));
    // the column is wider than a phone only when the window is: the gutter is never under 1.25 rem
    assert!(html.contains("max(1.25rem,"));
    // measure alone has no type scale, scale alone has no column
    let (m, _) = page_of(&program(
        "page { title: \"T\"; lang: \"en\"; measure: \"60\"; }",
        "Text { \"x\" }",
    ));
    assert!(m.contains("--cui-measure: 60rem;") && !m.contains("clamp(2.1rem"));
    let (s, _) = page_of(&program(
        "page { title: \"T\"; lang: \"en\"; scale: \"display\"; }",
        "Text { \"x\" }",
    ));
    assert!(!s.contains("--cui-measure") && s.contains("clamp(2.1rem"));
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
    assert!(theme_css.contains(".cui-card, .cui-cards > li { background: var(--cui-surface); border-color: transparent; box-shadow: var(--cui-shadow); }"));
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

#[test]
fn a_list_of_cards_is_a_grid_of_cards() {
    let (html, js) = page_of(&program(
        PAGE,
        "List { ordered: true card: true Item { Text { \"a\" } } Item { Text { \"b\" } } }",
    ));
    assert!(html.contains("<ol class=\"cui-cards\">") || js.contains("cui-cards"));
    assert_eq!(html.matches(".cui-cards {").count(), 1);
    assert_eq!(html.matches(".cui-cards > li {").count(), 1);
}

#[test]
fn round_two_roles_have_their_classes_and_rules() {
    let (html, js) = page_of(&program(
        PAGE,
        "Column { card: true Text { \"a\" } } Link { to: \"https://a.example/\" card: true \"c\" } Column { overlay: \"banner\" Text { \"f\" } Heading { level: 2 \"g\" } } Column { overlay: true Text { \"d\" } Text { \"e\" } } Grid { min: 9 weights: [1, 2] from: 60 Text { \"h\" } Text { \"i\" } }",
    ));
    for want in [
        "class=\"cui-col cui-card\"",
        "class=\"cui-card\">c",
        "cui-overlay cui-overlay-banner",
        "cui-grid-w cui-from-60\"",
        "\na.cui-card { display: block;",
        "@media (max-width: 59.99rem) { .cui-from-60.cui-grid-w > * { flex-basis: 100%; }",
        ".cui-overlay-banner { position: relative;",
        ".cui-overlay-banner > :nth-child(n+2) { position: relative;",
    ] {
        assert!(html.contains(want) || js.contains(want), "{want}");
    }
    // none of it for a page that uses none
    let (plain, _) = page_of(&program(PAGE, "Text { \"x\" }"));
    assert!(!plain.contains("cui-overlay") && !plain.contains("cui-from-"));
}

#[test]
fn a_sticky_header_has_a_mark_an_inline_nav_and_a_menu_that_swap() {
    let (html, _) = page_of(&program(
        PAGE,
        "Header { sticky: true Link { to: \"#a\" \"cancho\" } Nav { label: \"Page\" Link { to: \"#a\" \"A\" } } Row { Details { summary: \"Menu\" Nav { label: \"Menu\" Link { to: \"#a\" \"A\" } } } } } Main { id: \"main\" Section { id: \"a\" Text { \"x\" } } }",
    ));
    for want in [
        ".cui-sticky > nav { display: none; }",
        "@media (min-width: 62rem) { .cui-sticky > nav { display: flex;",
        ".cui-sticky details { display: none; }",
        "@media (max-width: 39.99rem) { .cui-sticky > :has(> .cui-chip) { display: none; } }",
        ".cui-sticky > img, .cui-sticky > picture img { display: block;",
    ] {
        assert!(html.contains(want), "{want}");
    }
}

#[test]
fn a_copy_button_is_a_small_dark_pill_with_a_44_px_target_above_the_text() {
    let src = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    requires Clipboard;\n    view { Code { copy: true \"$ cancho check\" } }\n}\n";
    let (html, _) = page_of(src);
    let (plain, _) = page_of(&program(PAGE, "Text { \"x\" }"));
    assert!(!plain.contains(".cui-copy { position: absolute; top: calc("));
    for want in [
        ".cui-code[data-copy] pre { padding-top: 2.5rem; }",
        ".cui-copy { position: absolute; top: calc(0.45rem - 7px); right: calc(0.45rem - 7px); box-sizing: border-box; min-height: 44px; padding: 0 0.6rem; border: 7px solid transparent;",
        "background: #1a212b; background-clip: padding-box; box-shadow: inset 0 0 0 1px #2d3644;",
    ] {
        assert!(html.contains(want), "{want}");
    }
}

#[test]
fn a_sticky_header_has_a_hamburger_and_no_github_button_on_a_phone() {
    let (html, _) = page_of(&program(
        PAGE,
        "Header { sticky: true Link { to: \"#a\" \"cancho\" } Row { Link { to: \"https://a.example/\" \"GitHub\" } Details { summary: \"Menu\" Nav { label: \"Menu\" Link { to: \"#a\" \"A\" } } } } } Main { id: \"main\" Section { id: \"a\" Text { \"x\" } } }",
    ));
    for want in [
        ".cui-sticky summary { display: grid; place-items: center; min-width: 44px; font-size: 0;",
        ".cui-sticky summary::before { content: \"\"; display: block; width: 1.25rem; height: 2px;",
        "@media (max-width: 29.99rem) { .cui-sticky .cui-row > a { display: none; } }",
    ] {
        assert!(html.contains(want), "{want}");
    }
}

#[test]
fn the_display_scale_sets_a_cards_title_and_text_as_the_page_does() {
    let page = "page { title: \"T\"; lang: \"en\"; scale: \"display\"; }";
    let (html, _) = page_of(&program(page, "Text { \"x\" }"));
    for want in [
        ".cui-card h3 { font-size: 1.35rem;",
        ".cui-card > p:not([data-style]), .cui-card .cui-col > p:not([data-style]) { margin: 0; font-size: 0.93rem; }",
    ] {
        assert!(html.contains(want), "{want}");
    }
}

#[test]
fn a_render_keeps_the_scroll_unless_a_new_screen_appeared() {
    // docs/screen-focus.md section 9: the page is rebuilt at every render; the window's scroll is read before and put back after, except when the render drew a screen that was not there
    let with_if = "component App {\n    state shown: Bool = false;\n    on Flip { shown = !shown; }\n    view {\n        Column {\n            Button { label: \"Flip\" on: Flip }\n            If { when: shown Section { Heading { level: 2 \"Next\" } Text { \"x\" } } }\n        }\n    }\n}\n";
    let (_, js) = page_of(with_if);
    assert!(js.contains("const __cui_sx = window.scrollX, __cui_sy = window.scrollY;"));
    assert!(js.contains("__cui_newscreen = __cui_screen_of.path !== null;"));
    assert!(js.contains("if (!__cui_newscreen && (__cui_sx > 0 || __cui_sy > 0)) { window.scrollTo(__cui_sx, __cui_sy); }"));
    // the restore comes after the page was built and after the focus was given back, so it is the last word on where the window is
    let build = js.find("__cui_ifs_prev = {};").expect("bookkeeping");
    let restore = js.find("window.scrollTo(__cui_sx, __cui_sy)").expect("restore");
    assert!(build < restore);
    // a page with no `If` has one screen: its scroll is always kept
    let (_, plain) = page_of(&program(PAGE, "Text { \"x\" }"));
    assert!(plain.contains("window.scrollTo(__cui_sx, __cui_sy)"));
    assert!(!plain.contains("__cui_newscreen = __cui_screen_of"));
}

#[test]
fn a_button_in_a_nav_is_a_tab_quiet_until_it_is_the_open_one() {
    // docs/structure.md section 12.0a: a Button inside a Nav is a tab; `pressed:` is the open one and is filled; only a page that uses a look role has the rule
    let tabs = "Nav { label: \"Main\" Row { Button { label: \"Home\" on: Go pressed: true } Button { label: \"Me\" on: Go pressed: false } } } Article { card: true Text { \"a\" } }";
    let text = format!("{PAGE}\ncomponent App {{\n    on Go {{ }}\n    view {{\n        Column {{ {tabs} }}\n    }}\n}}\n");
    let (html, js) = page_of(&text);
    let css = format!("{html}{js}");
    assert!(css.contains(":root nav button { background: transparent; color: var(--cui-fg, currentColor); }"));
    assert!(css.contains(":root nav button[aria-pressed=\"true\"] { background: var(--cui-accent, #185fa5); color: var(--cui-accent-fg, #ffffff); box-shadow: none; }"));
    let (plain_html, plain_js) = page_of(&program(PAGE, "Text { \"x\" }"));
    assert!(!format!("{plain_html}{plain_js}").contains(":root nav button"));
}

#[test]
fn a_docked_nav_is_a_tab_bar_fixed_at_the_foot_and_the_page_keeps_room_under_it() {
    // docs/structure.md 12.0b
    let (html, _) = page_of(&program(
        PAGE,
        "Main { id: \"main\" Text { \"x\" } } Nav { label: \"Main\" dock: true Link { to: \"#main\" \"Home\" } }",
    ));
    assert!(html.contains("<nav class=\"cui-dock\" aria-label=\"Main\""), "{html}");
    for want in [
        ".cui-dock { position: fixed; left: 50%; bottom: max(0.75rem, env(safe-area-inset-bottom));",
        "backdrop-filter: blur(16px);",
        "body:has(.cui-dock) { padding-bottom: calc(5.5rem + env(safe-area-inset-bottom)); }",
    ] {
        assert!(html.contains(want), "{want}");
    }
    // `dock: false` is an ordinary Nav
    let (plain, _) = page_of(&program(
        PAGE,
        "Main { id: \"main\" Text { \"x\" } } Nav { label: \"Main\" dock: false Link { to: \"#main\" \"Home\" } }",
    ));
    assert!(!plain.contains("class=\"cui-dock\""));
}

#[test]
fn a_docked_nav_is_refused_when_it_is_not_a_flag_or_not_alone_or_not_a_nav() {
    let one = "Nav { label: \"A\" dock: true Link { to: \"#a\" \"A\" } }";
    let got = codes(&program(PAGE, &format!("Main {{ id: \"a\" Text {{ \"x\" }} }} {one}")));
    assert!(got.is_empty(), "{got:?}");
    let got = codes(&program(PAGE, "Main { id: \"a\" Text { \"x\" } } Nav { label: \"A\" dock: \"bottom\" Link { to: \"#a\" \"A\" } }"));
    assert!(got.iter().any(|t| t == "nav-dock-invalid"), "{got:?}");
    let got = codes(&program(PAGE, "Main { id: \"a\" Text { \"x\" } } Nav { label: \"A\" dock: 1 Link { to: \"#a\" \"A\" } }"));
    assert!(got.iter().any(|t| t == "nav-dock-invalid"), "{got:?}");
    let two = "Nav { label: \"A\" dock: true Link { to: \"#a\" \"A\" } } Nav { label: \"B\" dock: true Link { to: \"#a\" \"B\" } }";
    let got = codes(&program(PAGE, &format!("Main {{ id: \"a\" Text {{ \"x\" }} }} {two}")));
    assert!(got.iter().any(|t| t == "nav-dock-duplicate"), "{got:?}");
    let got = codes(&program(PAGE, "Section { dock: true Text { \"x\" } }"));
    assert!(!got.is_empty(), "dock on a Section was accepted");
}

#[test]
fn an_avatar_is_a_round_disc_of_initials_that_overlaps_the_next_and_each_colour_reads() {
    let (html, _) = page_of(&program(
        PAGE,
        "Row { Text { style: \"avatar\" \"EA\" } Text { style: \"avatar\" \"MS\" } }",
    ));
    assert!(html.contains("data-style=\"avatar\""), "{html}");
    for want in [
        "[data-style=\"avatar\"] { display: inline-flex;",
        "border-radius: 50%;",
        "background: #b41d26; color: #fff; }",
        "[data-style=\"avatar\"] + [data-style=\"avatar\"] { margin-left: -0.6rem; }",
    ] {
        assert!(html.contains(want), "{want}");
    }
    // each fixed disc colour against white text: WCAG relative luminance, written out, not looked up
    fn lum(hex: &str) -> f64 {
        let c = |i: usize| {
            let v = f64::from(u8::from_str_radix(&hex[i..i + 2], 16).expect("hex")) / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * c(1) + 0.7152 * c(3) + 0.0722 * c(5)
    }
    for bg in ["#b41d26", "#6b5585", "#1e7e34", "#884e00"] {
        assert!(html.contains(bg), "{bg} is not in the page");
        let ratio = 1.05 / (lum(bg) + 0.05);
        assert!(ratio >= 4.5, "{bg}: {ratio:.2}");
    }
}

#[test]
fn a_cover_card_has_the_picture_on_top_the_title_over_its_foot_and_the_rest_as_a_panel() {
    // docs/structure.md 12.0d
    let (html, _) = page_of(&program(
        PAGE,
        "Column { overlay: \"cover\" Text { \"pic\" } Text { \"over\" } Text { \"panel\" } }",
    ));
    assert!(html.contains("class=\"cui-col cui-overlay cui-overlay-cover\"") || html.contains("cui-overlay cui-overlay-cover"), "{html}");
    for want in [
        ".cui-overlay-cover { position: relative; border-radius: 24px; overflow: hidden;",
        ".cui-overlay-cover > :nth-child(2) { grid-area: 1 / 1; align-self: end;",
        "linear-gradient(to top, rgba(0,0,0,0.7), rgba(0,0,0,0))",
        ".cui-overlay-cover > :nth-child(n+3) { grid-area: auto; grid-column: 1;",
        ".cui-overlay-cover > :first-child img { display: block; width: 100%;",
    ] {
        assert!(html.contains(want), "{want}");
    }
    let got = codes(&program(PAGE, "Column { overlay: \"hero\" Text { \"a\" } }"));
    assert!(got.iter().any(|t| t == "overlay-invalid"), "{got:?}");
    assert!(codes(&program(PAGE, "Column { overlay: \"banner\" Text { \"a\" } }")).is_empty());
}

#[test]
fn a_cover_card_in_a_themed_page_takes_the_surface_and_its_panel_is_not_a_floating_label() {
    let th = "theme {\n    light { bg: \"#faf7f2\"; fg: \"#1e1b18\"; muted: \"#6b6560\"; line: \"#e9e1dc\"; surface: \"#ffffff\"; soft: \"#f4ece7\"; ink: \"#2e1a47\"; accent: \"#b41d26\"; accent_fg: \"#ffffff\"; accent_soft: \"#ffdad7\"; good: \"#1e7e34\"; warn: \"#884e00\"; warn_bg: \"#ffdcbe\"; warn_line: \"#aa6402\"; bad: \"#ba1a1a\"; bad_bg: \"#ffdad6\"; code_bg: \"#12151a\"; code_fg: \"#d6dde8\"; }\n    dark { bg: \"#141118\"; fg: \"#f5f2eb\"; muted: \"#a8a199\"; line: \"#352d3d\"; surface: \"#1f1a24\"; soft: \"#2a2330\"; ink: \"#141118\"; accent: \"#ff6b6b\"; accent_fg: \"#2b0509\"; accent_soft: \"#3b1d26\"; good: \"#6fdba5\"; warn: \"#ffb366\"; warn_bg: \"#2a200d\"; warn_line: \"#7e6126\"; bad: \"#ff9187\"; bad_bg: \"#331816\"; code_bg: \"#12151a\"; code_fg: \"#d6dde8\"; }\n}\n";
    let src = format!("{th}{}", program(PAGE, "Column { overlay: \"cover\" Text { \"pic\" } Text { \"over\" } Text { \"panel\" } }"));
    let (html, _) = page_of(&src);
    assert!(html.contains(".cui-overlay:not(.cui-overlay-banner):not(.cui-overlay-cover) > :nth-child(n+2) {"), "the label rule must leave the cover card alone");
    assert!(html.contains(".cui-overlay-cover { background: var(--cui-surface); box-shadow: var(--cui-shadow); }"));
}

#[test]
fn a_render_keeps_an_open_disclosure_open() {
    // docs/screen-focus.md section 10: a `Details` is not program state, and the page is rebuilt at every render, so the open ones are read before and opened again after
    let src = "component App {\n    state v: String = \"\";\n    on Set(x) { v = x; }\n    view {\n        Column {\n            Details { summary: \"More\" TextField { label: \"Name\" value: v on: Set } }\n        }\n    }\n}\n";
    let (_, js) = page_of(src);
    let collect = "root.querySelectorAll('details').forEach((d) => { const s = d.querySelector(':scope > summary'); const t = s ? s.textContent : ''; __n[t] = (__n[t] || 0) + 1; __cui_open[t + '#' + __n[t]] = d.open; });";
    let reopen = "if (__cui_open[t + '#' + __n[t]] === true && !__cui_newscreen) { d.open = true; }";
    assert!(js.contains(collect), "{js}");
    assert!(js.contains(reopen), "{js}");
    assert!(js.contains("__n[t] = (__n[t] || 0) + 1; if (__cui_open["), "the second disclosure with the same text is told from the first: {js}");
    // the open flags are read before the page is rebuilt and put back after it
    let read = js.find("__cui_open[t + '#' + __n[t]] = d.open;").expect("read");
    let put = js.find("d.open = true;").expect("put back");
    let focus = js.find("__cui_focus_restore").unwrap_or(put);
    assert!(read < put, "{read} {put}");
    // it is put back before the focus is given back (a field inside a closed disclosure cannot take it) and before the scroll is moved (so the page has its height)
    let scroll = js.find("window.scrollTo(__cui_sx, __cui_sy)").expect("scroll");
    let focus_at = js.find("__cui_focus_restore(root, __focus)").expect("focus");
    assert!(put < focus_at && focus_at < scroll, "{put} {focus_at} {scroll}");
    let _ = focus;
}
