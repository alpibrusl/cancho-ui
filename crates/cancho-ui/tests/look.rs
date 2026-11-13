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

#[test]
fn a_field_is_a_pill_with_a_soft_fill_and_a_ring_in_the_accent_on_the_focus() {
    // docs/structure.md 12.0e
    let th = "theme {\n    light { bg: \"#faf7f2\"; fg: \"#1e1b18\"; muted: \"#6b6560\"; line: \"#e9e1dc\"; surface: \"#ffffff\"; soft: \"#f4ece7\"; ink: \"#2e1a47\"; accent: \"#b41d26\"; accent_fg: \"#ffffff\"; accent_soft: \"#ffdad7\"; good: \"#1e7e34\"; warn: \"#884e00\"; warn_bg: \"#ffdcbe\"; warn_line: \"#aa6402\"; bad: \"#ba1a1a\"; bad_bg: \"#ffdad6\"; code_bg: \"#12151a\"; code_fg: \"#d6dde8\"; }\n    dark { bg: \"#141118\"; fg: \"#f5f2eb\"; muted: \"#a8a199\"; line: \"#352d3d\"; surface: \"#1f1a24\"; soft: \"#2a2330\"; ink: \"#141118\"; accent: \"#ff6b6b\"; accent_fg: \"#2b0509\"; accent_soft: \"#3b1d26\"; good: \"#6fdba5\"; warn: \"#ffb366\"; warn_bg: \"#2a200d\"; warn_line: \"#7e6126\"; bad: \"#ff9187\"; bad_bg: \"#331816\"; code_bg: \"#12151a\"; code_fg: \"#d6dde8\"; }\n}\n";
    let app = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    state v: String = \"\";\n    on Set(x) { v = x; }\n    view { Column { Article { card: true Text { \"x\" } } TextField { label: \"Name\" value: v on: Set } } }\n}\n";
    let (html, _) = page_of(&format!("{th}{app}"));
    for want in [
        ":root input:not([type=\"checkbox\"]):not([type=\"file\"]):not([type=\"range\"]), :root select { border-radius: 999px; border-style: solid; border-width: 1px; padding-inline: 1.1rem; min-height: 2.75rem; }",
        ":root textarea { border-radius: 20px; border-style: solid; border-width: 1px; padding: 0.7rem 1.1rem; }",
        ":root input:focus-visible, :root textarea:focus-visible, :root select:focus-visible { outline: 3px solid color-mix(in srgb, var(--cui-accent, #185fa5) 55%, transparent);",
        "outline-offset: 1px; border-color: var(--cui-accent, #185fa5); }",
        "input:not([type=\"checkbox\"]):not([type=\"file\"]), textarea, select { background: var(--cui-soft); }",
        "border-color: var(--cui-muted); }",
    ] {
        assert!(html.contains(want), "{want}");
    }
}

#[test]
fn a_disclosure_summary_is_a_pill_with_a_plus_that_turns_into_a_minus() {
    // docs/structure.md 12.0f
    let src = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    view { Column { Article { card: true Text { \"x\" } } Details { summary: \"More\" Text { \"inside\" } } } }\n}\n";
    let (html, _) = page_of(src);
    for want in [
        ":root details:not(.cui-chart) > summary { display: inline-flex; box-sizing: border-box; padding: 0 1.1rem; border: 1px solid var(--cui-muted, currentColor); border-radius: 999px;",
        ":root details:not(.cui-chart) > summary::-webkit-details-marker { display: none; }",
        ":root details:not(.cui-chart) > summary::before { content: \"+\";",
        ":root details[open]:not(.cui-chart) > summary::before { content: \"\\2212\"; }",
        ":root details[open]:not(.cui-chart) > summary { margin-bottom: 0.75rem; }",
    ] {
        assert!(html.contains(want), "{want}");
    }
}

#[test]
fn a_floating_button_and_a_sheet_are_fixed_and_shown_only_while_their_marker_is_on_the_page() {
    // docs/structure.md 12.0g
    let src = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    state open: Bool = false;\n    on Toggle { open = !open; }\n    view {\n        Column {\n            Main { id: \"main\" Text { \"x\" } }\n            If { when: true Aside { marks: \"fab\" } }\n            If { when: open Aside { marks: \"sheet\" } }\n            Aside { label: \"Chat button\" fab: true Button { label: \"Chat\" on: Toggle } }\n            Aside { label: \"Chat\" sheet: true Text { \"hello\" } }\n        }\n    }\n}\n";
    let (html, _) = page_of(src);
    assert!(html.contains("class=\"cui-fab\""), "{html}");
    assert!(html.contains("class=\"cui-sheet\""), "{html}");
    for want in [
        ".cui-fab, .cui-sheet { position: fixed; z-index: 11; box-sizing: border-box; margin: 0; }",
        ".cui-fab { right: 1rem; bottom: calc(5.25rem + env(safe-area-inset-bottom)); }",
        ".cui-sheet { left: 50%; bottom: calc(5.25rem + env(safe-area-inset-bottom)); transform: translateX(-50%);",
        "max-height: calc(100dvh - 9.5rem); overflow-y: auto;",
        "[data-mark] { display: none; }",
        ":root:not(:has([data-mark=\"fab\"])) .cui-fab { display: none; }",
        ":root:not(:has([data-mark=\"sheet\"])) .cui-sheet { display: none; }",
    ] {
        assert!(html.contains(want), "{want}");
    }
    // the marker is the empty element the `If` draws: its attribute is in the script that draws it
    let (_, js) = page_of(src);
    assert!(js.contains("data-mark") && js.contains("\"sheet\"") && js.contains("\"fab\""), "{js}");
}

#[test]
fn a_floating_button_and_a_sheet_are_refused_when_badly_written_twice_or_never_marked() {
    let base = |aside: &str, extra: &str| format!("{PAGE}\ncomponent App {{\n    view {{\n        Column {{\n            Main {{ id: \"main\" Text {{ \"x\" }} }}\n            {extra}\n            {aside}\n        }}\n    }}\n}}\n");
    let marks = "If { when: true Aside { marks: \"fab\" } } If { when: true Aside { marks: \"sheet\" } }";
    let ok = base("Aside { label: \"F\" fab: true Button { label: \"B\" on: Go } }", marks).replace("view {", "on Go { }\n    view {");
    let got = codes(&ok);
    assert!(got.is_empty(), "{got:?}");
    let bad_flag = base("Aside { label: \"F\" fab: \"yes\" Text { \"x\" } }", marks);
    assert!(codes(&bad_flag).iter().any(|t| t == "aside-flag-invalid"));
    let bad_mark = base("Aside { label: \"F\" fab: true Text { \"x\" } }", "If { when: true Aside { marks: \"menu\" } }");
    assert!(codes(&bad_mark).iter().any(|t| t == "aside-mark-invalid"));
    let two = base("Aside { label: \"F\" fab: true Text { \"x\" } } Aside { label: \"G\" fab: true Text { \"y\" } }", marks);
    assert!(codes(&two).iter().any(|t| t == "aside-fab-duplicate"));
    let two_sheets = base("Aside { label: \"F\" sheet: true Text { \"x\" } } Aside { label: \"G\" sheet: true Text { \"y\" } }", marks);
    assert!(codes(&two_sheets).iter().any(|t| t == "aside-sheet-duplicate"));
    let unmarked_fab = base("Aside { label: \"F\" fab: true Text { \"x\" } }", "If { when: true Aside { marks: \"sheet\" } }");
    assert!(codes(&unmarked_fab).iter().any(|t| t == "aside-unmarked"));
    let unmarked_sheet = base("Aside { label: \"F\" sheet: true Text { \"x\" } }", "If { when: true Aside { marks: \"fab\" } }");
    assert!(codes(&unmarked_sheet).iter().any(|t| t == "aside-unmarked"));
    // the keys belong to Aside only
    let on_section = base("Section { fab: true Text { \"x\" } }", marks);
    assert!(!codes(&on_section).is_empty());
}

const CHAT: &str = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    state v: String = \"\";\n    on Set(x) { v = x; }\n    on Go { }\n    view {\n        Column {\n            Column { chatlog: true Column { bubble: \"mine\" Text { \"hi\" } } Column { bubble: \"theirs\" Text { \"yo\" } } Column { bubble: \"system\" Text { \"Ana joined\" } } }\n            Row { composer: true TextField { label: \"Write\" value: v on: Set placeholder: \"Message\" } Button { label: \"Send\" on: Go } }\n        }\n    }\n}\n";

#[test]
fn a_chat_is_a_log_with_bubbles_that_lean_and_a_composer_with_a_round_send_button() {
    // docs/structure.md 12.0h
    let (html, js) = page_of(CHAT);
    for want in [
        "class=\"cui-col cui-chatlog\"",
        "class=\"cui-col cui-bubble cui-bubble-mine\"",
        "class=\"cui-col cui-bubble cui-bubble-theirs\"",
        "class=\"cui-col cui-bubble cui-bubble-system\"",
        "class=\"cui-row cui-composer\"",
        ".cui-chatlog { display: flex; flex-direction: column; gap: 0; flex: 1 1 auto; min-height: 9rem; overflow-y: auto;",
        ".cui-bubble-theirs { align-self: flex-start; margin-left: 2.4rem; }",
        ".cui-bubble-mine { --b: var(--cui-accent-soft, #dcf8c6); align-self: flex-end; margin-right: 0.1rem; }",
        ".cui-bubble-system { align-self: center; max-width: 92%; background: transparent;",
        ".cui-bubble [data-style=\"eyebrow\"] { text-transform: none;",
        ".cui-bubble [data-style=\"note\"] { align-self: flex-end;",
        ".cui-composer > .cui-field { flex: 1 1 0; min-width: 0; font-size: 0; }",
        ".cui-composer > button:last-child { flex: none; width: 46px; min-width: 46px; height: 46px; margin-left: 0.3rem; padding: 0; border-radius: 50%; font-size: 0;",
        ".cui-composer > button:last-child::before { content: \"\\27A4\";",
        ".cui-sheet:has(> .cui-chatlog) { overflow: hidden;",
        ":root .cui-bubble button { min-height: 28px; min-width: 0; padding: 0 0.7rem; font-size: 0.78rem; background: transparent;",
        ".cui-sheet > .cui-row:first-child { justify-content: space-between; align-items: center; }",
    ] {
        assert!(html.contains(want), "{want}");
    }
    // the log that was at its foot stays at its foot; one scrolled up keeps its place
    assert!(js.contains("const __cui_log = [];"), "{js}");
    assert!(js.contains("foot: e.scrollHeight - e.scrollTop - e.clientHeight < 48, top: e.scrollTop"), "{js}");
    assert!(js.contains("if (!k || k.foot) { e.scrollTop = e.scrollHeight; } else { e.scrollTop = k.top; }"), "{js}");
    let read = js.find("const __cui_log = [];").expect("read");
    let put = js.find("e.scrollTop = e.scrollHeight;").expect("put");
    let focus = js.find("__cui_focus_restore(root, __focus)").expect("focus");
    assert!(read < put && put < focus, "{read} {put} {focus}");
    // a page with no chat log has none of that script
    let (_, plain) = page_of(&program(PAGE, "Text { \"x\" }"));
    assert!(!plain.contains("__cui_log"));
}

#[test]
fn a_bubble_a_chat_log_and_a_composer_are_refused_when_badly_written() {
    let bad_bubble = CHAT.replace("bubble: \"mine\"", "bubble: \"loud\"");
    assert!(codes(&bad_bubble).iter().any(|t| t == "bubble-invalid"));
    let bad_log = CHAT.replace("chatlog: true", "chatlog: \"yes\"");
    assert!(codes(&bad_log).iter().any(|t| t == "chat-flag-invalid"));
    let bad_composer = CHAT.replace("composer: true", "composer: 1");
    assert!(codes(&bad_composer).iter().any(|t| t == "chat-flag-invalid"));
    assert!(codes(CHAT).is_empty());
    // the keys belong to Column (bubble, chatlog) and Row (composer)
    let on_row = CHAT.replace("Row { composer: true", "Row { bubble: \"mine\"");
    assert!(!codes(&on_row).is_empty());
    let on_column = CHAT.replace("Row { composer: true", "Column { composer: true");
    assert!(!codes(&on_column).is_empty());
}


#[test]
fn an_icon_only_button_keeps_its_own_size_in_a_column() {
    let th = "theme {\n    light { bg: \"#faf7f2\"; fg: \"#1e1b18\"; muted: \"#6b6560\"; line: \"#e9e1dc\"; surface: \"#ffffff\"; soft: \"#f4ece7\"; ink: \"#2e1a47\"; accent: \"#b41d26\"; accent_fg: \"#ffffff\"; accent_soft: \"#ffdad7\"; good: \"#1e7e34\"; warn: \"#884e00\"; warn_bg: \"#ffdcbe\"; warn_line: \"#aa6402\"; bad: \"#ba1a1a\"; bad_bg: \"#ffdad6\"; code_bg: \"#12151a\"; code_fg: \"#d6dde8\"; }\n    dark { bg: \"#141118\"; fg: \"#f5f2eb\"; muted: \"#a8a1b2\"; line: \"#352d3d\"; surface: \"#1f1a24\"; soft: \"#2a2330\"; ink: \"#141118\"; accent: \"#ff6b6b\"; accent_fg: \"#2b0509\"; accent_soft: \"#3b1d26\"; good: \"#6fdba5\"; warn: \"#ffb366\"; warn_bg: \"#2a200d\"; warn_line: \"#7e6126\"; bad: \"#ff9187\"; bad_bg: \"#331816\"; code_bg: \"#12151a\"; code_fg: \"#d6dde8\"; }\n}\n";
    let app = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    state v: Int = 0;\n    on Tap { v = v + 1; }\n    view { Column { Article { card: true Text { \"x\" } } Button { label: \"Save\" on: Tap Text { \"S\" } } } }\n}\n";
    let (html, _) = page_of(&format!("{th}{app}"));
    assert!(html.contains(":root .cui-col > button:has(> *) { align-self: flex-start; }"), "an icon button does not stretch");
    assert!(html.contains(":root a[title]:has(> *) { text-decoration: none;"), "an icon link is a round target");
    assert!(html.contains(":root button:has(> *) { font-size: 1.25rem; line-height: 1; }"), "the icon is larger");
}


const WA: &str = "page { title: \"T\"; lang: \"en\"; }\ncomponent App {\n    state v: String = \"\";\n    state shown: String = \"1\";\n    state open: Bool = true;\n    on Set(x) { v = x; }\n    on Go { }\n    on Pick(n) { v = n; }\n    view {\n        Column {\n            If { when: open Aside { marks: \"sheet\" } }\n            Section { id: \"media\" Text { \"the media\" } }\n            Aside { label: \"Chat\" sheet: true\n                Row { head: true Text { style: \"avatar\" initials(\"lisbon trip\") } Column { Heading { level: 2 \"lisbon\" } Text { style: \"note\" \"Ana, Ben\" } } Button { label: \"Close\" on: Go Text { \"x\" } } }\n                Column { chatlog: true\n                    Column { bubble: \"day\" Text { \"26 July 2026\" } }\n                    Column { bubble: \"theirs\" Text { style: \"mark\" \".\" } Text { style: \"disc3\" \"AR\" } Text { style: \"hue3\" \"Ana\" } If { when: emoji_only(v) Text { style: \"big\" \"x\" } } Row { chips: true Button { label: \"Laugh\" on: Pick(\"1\") Text { \"x\" } } } Button { label: \"React\" on: Pick(\"2\") Text { \"x\" } } }\n                }\n                Row { composer: true Button { label: \"Emoji\" on: Go Text { \"x\" } } TextField { label: \"Write\" value: v on: Set placeholder: \"Message\" } Button { label: \"Attach\" on: Go scroll: \"media\" Text { \"x\" } } Button { label: \"Send\" on: Go } }\n            }\n        }\n    }\n}\n";

#[test]
fn a_whatsapp_style_chat_has_runs_day_pills_chips_a_header_and_a_composer_pill() {
    // docs/structure.md 12.0i
    let (html, js) = page_of(WA);
    for want in [
        "class=\"cui-col cui-bubble cui-bubble-day\"",
        "class=\"cui-row cui-chips\"",
        "class=\"cui-row cui-chathead\"",
        "data-style=\"mark\"",
        "data-style=\"disc3\"",
        "data-style=\"hue3\"",
        ".cui-bubble:has(> [data-style=\"mark\"])::before { content: \"\"; position: absolute; top: 0;",
        ".cui-bubble:has(> [data-style=\"big\"]) { --b: transparent; background: transparent; box-shadow: none;",
        ".cui-bubble [data-style=\"hue0\"] { color: #b41d26; }",
        ".cui-bubble [data-style=\"hue7\"] { color: #c9d65a; }",
        ".cui-bubble:has(> .cui-chips) { margin-bottom: 0.95rem; }",
        ".cui-chathead > .cui-col > * { margin: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }",
        ".cui-composer::before { content: \"\"; position: absolute;",
        "background-image: url(\"data:image/svg+xml,",
    ] {
        assert!(html.contains(want), "{want}");
    }
    // a Button's `scroll:` scrolls to the element with that id after the click
    assert!(js.contains("document.getElementById(\"media\")"), "{js}");
    assert!(js.contains("scrollIntoView({ block: 'start' })"), "{js}");
    assert!(html.contains("id=\"media\""), "{html}");
    // the two builtins are in the page, and only because it calls them
    assert!(js.contains("function __cui_emoji_only(text)") && js.contains("function __cui_initials(text)"));
    let (_, plain) = page_of(&program(PAGE, "Text { \"x\" }"));
    assert!(!plain.contains("__cui_emoji_only") && !plain.contains("scrollIntoView"));
    assert!(codes(WA).is_empty(), "{:?}", codes(WA));
}

#[test]
fn the_new_chat_keys_are_refused_when_badly_written() {
    assert!(codes(&WA.replace("bubble: \"day\"", "bubble: \"week\"")).iter().any(|t| t == "bubble-invalid"));
    assert!(codes(&WA.replace("style: \"hue3\"", "style: \"hue9\"")).iter().any(|t| t == "text-style-invalid"));
    assert!(codes(&WA.replace("chips: true", "chips: 1")).iter().any(|t| t == "chat-flag-invalid"));
    assert!(codes(&WA.replace("head: true", "head: \"yes\"")).iter().any(|t| t == "chat-flag-invalid"));
    // `chips` and `head` belong to Row
    assert!(!codes(&WA.replace("Row { chips: true", "Column { chips: true")).is_empty());
    // `scroll:` is a Button's, a literal name of an anchor
    assert!(!codes(&WA.replace("scroll: \"media\"", "scroll: v")).is_empty());
    assert!(codes(&WA.replace("scroll: \"media\"", "scroll: \"a b\"")).iter().any(|t| t == "scroll-invalid"));
    assert!(codes(&WA.replace("TextField { label: \"Write\" value: v on: Set", "TextField { label: \"Write\" scroll: \"media\" value: v on: Set")).iter().any(|t| t == "scroll-invalid"));
    // a builtin takes one argument
    assert!(codes(&WA.replace("emoji_only(v)", "emoji_only(v, v)")).iter().any(|t| t == "builtin-shape-mismatch"), "{:?}", codes(&WA.replace("emoji_only(v)", "emoji_only(v, v)")));
}

#[test]
fn a_quote_a_mention_and_the_ticks_have_their_tags_classes_and_rules() {
    let (html, js) = page_of(&program(
        PAGE,
        "Column { bubble: \"mine\" Column { quote: true Text { style: \"hue3\" \"Ana\" } Text { style: \"note\" \"quoted\" } } Text { style: \"marked\" mark_text(\"hi @ana\", \"[]\") } Row { meta: true Text { style: \"note\" \"20:42\" } Text { style: \"tickblue\" \"x\" } Text { style: \"tick\" \"y\" } } }",
    ));
    assert!(html.contains("cui-quote"), "{html}");
    for want in [".cui-quote { display: flex;", ".cui-quote:has([data-style=\"hue3\"]) { --q: #1e7e34; }", ".cui-mention {", ".cui-meta {", "[data-style=\"tickblue\"] { color: #1d5fa8; }"] {
        assert!(html.contains(want), "{want}");
    }
    // the spans are made with text nodes and textContent, never as markup
    assert!(js.contains("function __cui_marked(el)") && js.contains("sp.textContent = parts[i].slice(0, j);"), "{js}");
    assert!(!js.contains("innerHTML"), "innerHTML in the page script");
    assert!(js.contains("__cui_mark_text("), "{js}");
}

#[test]
fn the_chat_flags_and_styles_are_refused_where_they_do_not_belong() {
    let c = codes(&program(PAGE, "Row { quote: true Text { \"a\" } }"));
    assert!(c.iter().any(|x| x == "binding-unknown"), "{c:?}");
    let c = codes(&program(PAGE, "Column { meta: true Text { \"a\" } }"));
    assert!(c.iter().any(|x| x == "binding-unknown"), "{c:?}");
    let c = codes(&program(PAGE, "Column { quote: \"yes\" Text { \"a\" } }"));
    assert!(c.iter().any(|x| x == "chat-flag-invalid"), "{c:?}");
    let c = codes(&program(PAGE, "Text { style: \"marker\" \"a\" }"));
    assert!(c.iter().any(|x| x == "text-style-invalid"), "{c:?}");
    assert!(codes(&program(PAGE, "Text { style: \"marked\" \"a\" }")).is_empty());
    // mark_text takes two arguments
    assert!(!codes(&program(PAGE, "Text { mark_text(\"a\") }")).is_empty());
}

/// The emitted `__cui_mark_text` and `__cui_marked` against the same cases as replay's `mark_text` (docs/structure.md 12.0j), run by node when there is one.
#[test]
fn the_emitted_mark_text_agrees_with_replays() {
    let Ok(v) = Command::new("node").arg("--version").output() else {
        eprintln!("node is not installed: not run");
        return;
    };
    assert!(v.status.success());
    let (_, js) = page_of(&program(
        PAGE,
        "Text { style: \"marked\" mark_text(\"hi\", \"[]\") }",
    ));
    let start = js.find("function __cui_mark_text").expect("the helper is in the page");
    let end = js.find("function __cui_marked").expect("marked");
    let helper = &js[start..end];
    let case = |t: &str, m: &str| format!("console.log(JSON.stringify(__cui_mark_text({t}, {m})));");
    let cases = [
        case("\"hi @ana!\"", "'[{\"start\":3,\"end\":7}]'"),
        case("\"\\u00e9 \\ud83d\\ude02 @ben\"", "'[{\"start\":8,\"end\":12}]'"),
        case("\"abc\"", "'[{\"start\":1,\"end\":9}]'"),
        case("\"\\u00e9\"", "'[{\"start\":1,\"end\":2}]'"),
        case("\"abcd\"", "'[{\"start\":0,\"end\":3},{\"start\":2,\"end\":4}]'"),
        case("\"plain\"", "'not json'"),
        case("\"a\\ue000b\\ue001c\"", "'[]'"),
        case("\"abc\"", "'[{\"start\":\"1\",\"end\":2}]'"),
    ];
    let want = [
        "\"hi \u{e000}@ana\u{e001}!\"",
        "\"\u{e9} \u{1f602} \u{e000}@ben\u{e001}\"",
        "\"abc\"",
        "\"\u{e9}\"",
        "\"\u{e000}abc\u{e001}d\"",
        "\"plain\"",
        "\"abc\"",
        "\"abc\"",
    ];
    let script = format!("{helper}\n{}", cases.join("\n"));
    let out = Command::new("node").arg("-e").arg(&script).output().expect("node runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let got: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want.iter()) {
        assert_eq!(g, w);
    }
}
