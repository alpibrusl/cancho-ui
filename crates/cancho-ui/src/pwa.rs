//! `cancho-ui build --pwa`: the files that make a compiled program an installable web app.
//!
//! Designed in `docs/mobile-backend.md` first. Everything here is deterministic text or bytes with no clock, no
//! network and no image or hashing library: the PNG icons are written by hand (a palette image in a stored-deflate
//! zlib stream) and the cache version is FNV-1a 64, which is a version label and not a security measure.

use std::path::Path;

use cancho_ui_ir::recipe::Prerender;

/// The files of one PWA, in the order they are written: `(file name, bytes)`.
pub type PwaFiles = Vec<(String, Vec<u8>)>;

/// What a theme adds to a page (docs/theme.md): its stylesheet, and the font files it declared, as `(path, bytes)`.
pub struct ThemeAssets {
    pub css: String,
    pub fonts: Vec<(String, Vec<u8>)>,
}

/// What a `page { .. }` and a sticky `Header` add to the document (docs/page.md); the default is a program with neither.
#[derive(Default)]
pub struct PageParts {
    /// replaces the `--name` as the `<title>`
    pub title: Option<String>,
    /// replaces `en` as `<html lang>`
    pub lang: Option<String>,
    /// the `<meta>` and `<link>` elements, already escaped (`cancho_ui_ir::page::head_html`)
    pub head: String,
    /// the skip link, the first element of `<body>`
    pub skip: String,
    /// the page declares an icon / a touch icon, so `--pwa` does not add its own
    pub icon: bool,
    pub touch_icon: bool,
    /// the program has a sticky `Header`: the page reserves room for it above an anchor
    pub sticky: bool,
    /// what the program uses of the motion script (docs/motion.md)
    pub motion: crate::motion::MotionUse,
    /// `page { measure: "72"; }`: the width in rem of the centred column
    pub measure: Option<u32>,
    /// `page { scale: "display"; }`
    pub display_scale: bool,
    /// the program uses `card:`, `align: "center"` or a pill link
    pub roles: bool,
    /// the program has a `Code { copy: true }`: its button is the small dark pill (docs/www-port/look3.md)
    pub copy: bool,
}

const BACKGROUND: [u8; 3] = [0x1f, 0x6f, 0xeb];
const MARK: [u8; 3] = [0xff, 0xff, 0xff];

/// Everything `--pwa` writes for a program compiled to `js`, named `name`; `site` is the files its views name (`Image`, `Media`).
pub fn generate(
    js: &str,
    name: &str,
    csp: Option<&str>,
    theme: Option<&ThemeAssets>,
    site: &[(String, Vec<u8>)],
    prerender: Option<&Prerender>,
    page: &PageParts,
) -> PwaFiles {
    let mut files: PwaFiles = vec![
        ("app.js".into(), js.as_bytes().to_vec()),
        (
            "index.html".into(),
            index_html(name, true, csp, theme, prerender, page).into_bytes(),
        ),
        ("manifest.webmanifest".into(), manifest(name).into_bytes()),
        ("icon-192.png".into(), icon_png(192)),
        ("icon-512.png".into(), icon_png(512)),
    ];
    files.extend(theme.iter().flat_map(|t| t.fonts.iter().cloned()));
    files.extend(site.iter().cloned());
    let sw = service_worker(&files);
    files.push(("sw.js".into(), sw.into_bytes()));
    files
}

/// What `--page` writes for a program compiled to `js`, named `name`: a web page and its script, and nothing that makes it an app
/// (no manifest, no icons, no service worker, no `theme-color`), so a browser has nothing to offer to install and nothing
/// caches it behind the author's back (docs/mobile-backend.md section 5.2).
pub fn generate_page(
    js: &str,
    name: &str,
    csp: Option<&str>,
    theme: Option<&ThemeAssets>,
    site: &[(String, Vec<u8>)],
    prerender: Option<&Prerender>,
    page: &PageParts,
) -> PwaFiles {
    let mut files: PwaFiles = vec![
        ("app.js".into(), js.as_bytes().to_vec()),
        (
            "index.html".into(),
            index_html(name, false, csp, theme, prerender, page).into_bytes(),
        ),
    ];
    files.extend(theme.iter().flat_map(|t| t.fonts.iter().cloned()));
    files.extend(site.iter().cloned());
    files
}

/// Write `files` into `dir`, creating it. Nothing is written before the caller has a program that compiled.
pub fn write_all(dir: &Path, files: &PwaFiles) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creating `{}`: {}", dir.display(), e))?;
    for (file, bytes) in files {
        let path = dir.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating `{}`: {}", parent.display(), e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("writing `{}`: {}", path.display(), e))?;
    }
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The page; `installable` adds what makes it an app (`--pwa`), and leaves it out for a plain page (`--page`).
fn index_html(
    name: &str,
    installable: bool,
    csp: Option<&str>,
    theme: Option<&ThemeAssets>,
    prerender: Option<&Prerender>,
    page: &PageParts,
) -> String {
    let title = html_escape(page.title.as_deref().unwrap_or(name));
    let html = String::from(
        r##"<!DOCTYPE html>
<html lang="@LANG@">
<head>
<meta charset="UTF-8">
@CSP@<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
@APP_THEME@<title>@TITLE@</title>
@PAGE_HEAD@@APP_LINKS@<style>
/* A default so a program is usable on a phone; the page owns presentation, replace it freely. */
:root { color-scheme: light dark; }
body {
    margin: 0;
    padding: max(1rem, env(safe-area-inset-top)) max(1rem, env(safe-area-inset-right))
        max(1rem, env(safe-area-inset-bottom)) max(1rem, env(safe-area-inset-left));
    font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif;
}
.cui-col { display: flex; flex-direction: column; gap: 0.5rem; }
.cui-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; }
/* Grid (docs/structure.md section 10): as many columns as fit at `min`, equal; with weights, one flex line whose cells grow in proportion and wrap, one under another, when the line cannot hold them */
.cui-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--cui-min)), 1fr)); gap: 0.5rem; }
.cui-grid.cui-grid-w { display: flex; flex-wrap: wrap; }
.cui-grid-w > * { min-width: 0; }
.cui-grid-w > :nth-child(1) { flex: var(--cui-w1) 1 min(100%, calc(var(--cui-min) * var(--cui-w1))); }
.cui-grid-w > :nth-child(2) { flex: var(--cui-w2) 1 min(100%, calc(var(--cui-min) * var(--cui-w2))); }
.cui-grid-w > :nth-child(3) { flex: var(--cui-w3) 1 min(100%, calc(var(--cui-min) * var(--cui-w3))); }
.cui-grid-w > :nth-child(4) { flex: var(--cui-w4) 1 min(100%, calc(var(--cui-min) * var(--cui-w4))); }
/* gap: 0..4 on a Row or Column (docs/presentation.md section 2) */
.cui-gap-0 { gap: 0; } .cui-gap-1 { gap: 0.25rem; } .cui-gap-2 { gap: 0.5rem; } .cui-gap-3 { gap: 1rem; } .cui-gap-4 { gap: 2rem; }
/* a TextField with a label: the label is shown above the field */
.cui-field { display: flex; flex-direction: column; gap: 0.25rem; }
/* a Checkbox: the box and its text in one label that is the touch target (docs/controls.md section 9) */
.cui-check { display: flex; align-items: center; gap: 0.5rem; min-height: 44px; min-width: 44px; }
.cui-check input { min-height: 0; min-width: 0; width: 1.25rem; height: 1.25rem; margin: 0; }
/* Table */
.cui-table { border-collapse: collapse; width: 100%; }
/* a table wider than the page scrolls inside this box, not the page (#144) */
.cui-table-scroll { overflow-x: auto; max-width: 100%; }
.cui-table th, .cui-table td { text-align: start; padding: 0.25rem 0.5rem; border-bottom: 1px solid color-mix(in srgb, currentColor 25%, transparent); }
.cui-table th { font-weight: 600; }
/* Chart (docs/charts.md): the series colour is 7.59:1 on white and 9.63:1 on the dark page (WCAG 1.4.11 asks 3:1) */
:root { --cui-chart: #0550ae; --cui-chart-open: #57606a; }
.cui-chart { margin: 0; }
.cui-chart figcaption { font-weight: 600; }
.cui-chart summary { min-height: 44px; display: flex; align-items: center; cursor: pointer; }
/* tone: a status text says what kind it is by a prefix as well as by colour, so colour is never the only signal */
[data-tone] { font-weight: 600; }
[data-tone="ok"] { color: #1a7f37; } [data-tone="ok"]::before { content: "\2713  "; }
[data-tone="warn"] { color: #9a6700; } [data-tone="warn"]::before { content: "\26A0  "; }
[data-tone="error"] { color: #cf222e; } [data-tone="error"]::before { content: "\2717  "; }
@media (prefers-color-scheme: dark) {
    :root { --cui-chart: #79c0ff; --cui-chart-open: #9198a1; }
    [data-tone="ok"] { color: #3fb950; } [data-tone="warn"] { color: #d29922; } [data-tone="error"] { color: #ff7b72; }
}
button, input, textarea { min-height: 44px; min-width: 44px; font: inherit; }
/* structure (docs/structure.md): a link in a header, a nav or a footer is a target of its own; the appearances of a Link; the roles of a Text */
header a, nav a, footer a { display: inline-flex; align-items: center; min-height: 44px; }
a.cui-link-primary, a.cui-link-secondary, a.cui-link-quiet { display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; min-height: 44px; min-width: 44px; padding: 0 1rem; border-radius: 8px; font-weight: 600; text-decoration: none; }
a.cui-link-primary { background: LinkText; color: Canvas; }
a.cui-link-secondary { border: 1px solid LinkText; }
details > summary { display: flex; align-items: center; min-height: 44px; cursor: pointer; }
figure { margin: 0; } figcaption { font-size: 0.88rem; }
[data-style] { margin: 0; }
[data-style="eyebrow"], [data-style="label"] { font-size: 0.75rem; font-weight: 600; letter-spacing: 0.1em; text-transform: uppercase; }
[data-style="eyebrow"] { color: LinkText; }
[data-style="lead"] { font-size: 1.1rem; max-width: 40rem; }
[data-style="note"] { font-size: 0.88rem; }
/* inline content and code (docs/structure.md sections 7 and 8): a code box is dark in both schemes, as the cancho-www page draws it */
code { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 0.9em; }
.cui-code { position: relative; border-radius: 12px; background: #12151a; color: #d6dde8; }
.cui-code pre { margin: 0; padding: 0.9rem 1rem; overflow: auto; white-space: pre; font: 0.78rem/1.6 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
.cui-code code { font: inherit; }
.cui-code[data-copy] pre { padding-right: 5.5rem; }
.cui-copy { position: absolute; top: 0.45rem; right: 0.45rem; }
/* Image and Media (docs/structure.md section 10): never wider than the file or the column; a `ratio` crops to the shape */
.cui-img { display: block; overflow: hidden; border-radius: 12px; }
.cui-img img { display: block; width: 100%; height: auto; }
.cui-img[data-ratio] img { height: 100%; object-fit: cover; }
.cui-media { display: block; width: 100%; max-width: 100%; height: auto; border-radius: 12px; background: #000; }
[data-ratio="1:1"] { aspect-ratio: 1 / 1; } [data-ratio="4:3"] { aspect-ratio: 4 / 3; } [data-ratio="3:2"] { aspect-ratio: 3 / 2; }
[data-ratio="16:9"] { aspect-ratio: 16 / 9; } [data-ratio="21:9"] { aspect-ratio: 21 / 9; } [data-ratio="3:4"] { aspect-ratio: 3 / 4; } [data-ratio="2:3"] { aspect-ratio: 2 / 3; }
/* Waffle and Chip (docs/waffle.md): squares are drawn in the series colour (7.59:1 on white, 9.63:1 on the dark page) and a text colour of the page (6.4:1 and 6.5:1), outlined and not filled when they are not counted, so colour is never the only difference; a chip says its message in words and its tone in a colour and a border */
.cui-waffle svg { display: block; width: 100%; max-width: 60rem; height: auto; }
.cui-wf-on { fill: var(--cui-chart); }
.cui-wf-known { fill: none; stroke: var(--cui-chart); stroke-width: 2; }
.cui-wf-open { fill: none; stroke: var(--cui-chart-open); stroke-width: 1; }
.cui-waffle table { border-collapse: collapse; } .cui-waffle th, .cui-waffle td { text-align: start; padding: 0.25rem 0.75rem 0.25rem 0; }
.cui-chip { display: inline-block; font: 600 0.72rem ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; letter-spacing: 0.04em; text-transform: uppercase; padding: 0.1rem 0.5rem; border-radius: 999px; border: 1px solid currentColor; white-space: nowrap; }
.cui-chip-good { color: var(--cui-good, #1a7f37); }
.cui-chip-warn { color: var(--cui-warn, #9a6700); background: var(--cui-warn-bg, transparent); }
.cui-chip-accent { color: var(--cui-accent, #0550ae); }
.cui-chip-quiet { color: var(--cui-chart-open); border-style: dashed; }
@media (prefers-color-scheme: dark) { .cui-chip-good { color: var(--cui-good, #3fb950); } .cui-chip-warn { color: var(--cui-warn, #d29922); } .cui-chip-accent { color: var(--cui-accent, #79c0ff); } }
/* page (docs/page.md): a sticky Header stays at the top of the window, a skip link shows only while it has the focus, and a Main a link can start from is not outlined */
.cui-sticky { position: sticky; top: 0; z-index: 10; box-sizing: border-box; min-height: 4rem; display: flex; flex-wrap: wrap; align-items: center; column-gap: 1rem; background: Canvas; border-bottom: 1px solid color-mix(in srgb, currentColor 25%, transparent); }
.cui-skip { position: fixed; top: -4rem; left: 0.5rem; z-index: 20; display: inline-flex; align-items: center; min-height: 44px; padding: 0 1rem; border-radius: 8px; font-weight: 600; background: LinkText; color: Canvas; }
.cui-skip:focus { top: 0.5rem; }
main[tabindex="-1"]:focus { outline: none; }
.cui-sticky nav { display: flex; flex-wrap: wrap; column-gap: 1rem; }
@MOTION_CSS@</style>
@STICKY_CSS@@LOOK_CSS@@THEME_CSS@</head>
<body>
@SKIP@@ROOT@
<script type="module" src="./app.js"></script>
@APP_REGISTER@</body>
</html>
"##,
    );
    // A page with a map lets images in from its own origin and the tile hosts the program declared, and from nowhere else (docs/maps.md); a page with a
    // sealed island lets in no frame but its `srcdoc` ones (docs/islands.md section 1). The policy comes first, before anything that could ask.
    let csp = match csp {
        Some(policy) => format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">\n",
            html_escape(policy)
        ),
        None => String::new(),
    };
    let theme_css = match theme {
        Some(t) => format!("<style id=\"cui-theme\">\n{}</style>\n", t.css),
        None => String::new(),
    };
    let (theme, links, register) = if installable {
        (
            "<meta name=\"theme-color\" content=\"#1f6feb\">\n",
            // a page that declares its own icons keeps them: two `rel="icon"` would leave the browser to choose
            format!(
                "<link rel=\"manifest\" href=\"./manifest.webmanifest\">\n{}{}",
                if page.icon {
                    ""
                } else {
                    "<link rel=\"icon\" type=\"image/png\" sizes=\"192x192\" href=\"./icon-192.png\">\n"
                },
                if page.touch_icon {
                    ""
                } else {
                    "<link rel=\"apple-touch-icon\" href=\"./icon-192.png\">\n"
                }
            ),
            "<script>\nif ('serviceWorker' in navigator) {\n    window.addEventListener('load', function () { navigator.serviceWorker.register('./sw.js'); });\n}\n</script>\n",
        )
    } else {
        ("", String::new(), "")
    };
    let sticky_css = if page.sticky { STICKY_CSS } else { "" };
    let lang = html_escape(page.lang.as_deref().unwrap_or("en"));
    // one pass over the template: the text of a program, a title or a description is never read as a token, whatever it says
    let motion_js = if page.motion.any() {
        crate::motion::script(page.motion)
    } else {
        String::new()
    };
    let measure = format!("{}{motion_js}", if page.sticky { STICKY_JS } else { "" });
    let look_css = look_css(page);
    let tokens: [(&str, &str); 12] = [
        ("@MOTION_CSS@", crate::motion::CSS),
        ("@CSP@", &csp),
        ("@TITLE@", &title),
        ("@LANG@", &lang),
        ("@PAGE_HEAD@", &page.head),
        ("@STICKY_CSS@", sticky_css),
        ("@LOOK_CSS@", &look_css),
        ("@THEME_CSS@", &theme_css),
        ("@APP_THEME@", theme),
        ("@APP_LINKS@", &links),
        ("@APP_REGISTER@", register),
        ("@SKIP@", &page.skip),
    ];
    // the program's own text goes in after every token is replaced, and the template is filled on either side of it, so that text can never be
    // read as a token
    let (head, tail) = html.split_once("@ROOT@").unwrap_or((html.as_str(), ""));
    format!(
        "{}{}{}",
        expand(head, &tokens),
        root_block(prerender, &measure),
        expand(tail, &tokens)
    )
}

/// The rules of the roles `card:`, `align: "center"` and `appearance: "pill"` (docs/www-port/look.md section 3), written only for a program that uses one. A pill is a 44 px
/// target drawn 30 px tall: the transparent border is the target and the inset shadow the drawn edge.
const COPY_CSS: &str = r#".cui-code[data-copy] pre { padding-top: 2.5rem; }
.cui-copy { position: absolute; top: calc(0.45rem - 7px); right: calc(0.45rem - 7px); box-sizing: border-box; min-height: 44px; padding: 0 0.6rem; border: 7px solid transparent; border-radius: 15px; background: #1a212b; background-clip: padding-box; box-shadow: inset 0 0 0 1px #2d3644; color: #c3ccd8; font: 600 0.7rem ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; cursor: pointer; }
.cui-copy:hover { box-shadow: inset 0 0 0 1px #6ea3ee; color: #fff; }
"#;

const ROLES_CSS: &str = r#".cui-card { box-sizing: border-box; padding: 1.1rem 1.2rem 1.3rem; border-radius: 16px; border: 1px solid color-mix(in srgb, currentColor 20%, transparent); }
.cui-align-center { align-items: center; }
.cui-overlay { display: grid; }
.cui-overlay.cui-card { padding: 0; overflow: hidden; }
.cui-overlay > * { grid-area: 1 / 1; min-width: 0; }
.cui-overlay > :nth-child(n+2) { box-sizing: border-box; margin: 0.75rem; padding: 0.5rem 0.8rem; border-radius: 12px; background: Canvas; color: CanvasText; z-index: 1; }
.cui-overlay:not(.cui-overlay-banner) > :nth-child(n+2) { font-size: 0.9rem; }
.cui-overlay > :nth-child(2) { align-self: start; justify-self: start; }
.cui-overlay > :nth-child(3) { align-self: end; }
.cui-overlay-banner { position: relative; min-height: 11rem; align-content: center; border-radius: 16px; overflow: hidden; color: #fff; }
.cui-overlay-banner > :first-child { position: absolute; inset: 0; background: #0b1016; }
.cui-overlay-banner > :first-child img { display: block; width: 100%; height: 100%; max-width: none; aspect-ratio: auto; object-fit: cover; }
.cui-overlay-banner > picture:first-child { max-width: none !important; }
.cui-overlay-banner > :nth-child(n+2) { position: relative; grid-area: auto; grid-column: 1; background: none; color: inherit; margin: 0; padding: 0 1.5rem; align-self: auto; justify-self: auto; max-width: 40rem; }
.cui-overlay-banner > :nth-child(2) { padding-top: 1.5rem; }
.cui-overlay-banner > :last-child { padding-bottom: 1.5rem; }
@media (max-width: 39.99rem) { .cui-from-40.cui-grid-w > * { flex-basis: 100%; } .cui-from-40:not(.cui-grid-w) { grid-template-columns: 1fr; } }
@media (max-width: 47.99rem) { .cui-from-48.cui-grid-w > * { flex-basis: 100%; } .cui-from-48:not(.cui-grid-w) { grid-template-columns: 1fr; } }
@media (max-width: 59.99rem) { .cui-from-60.cui-grid-w > * { flex-basis: 100%; } .cui-from-60:not(.cui-grid-w) { grid-template-columns: 1fr; } }
a.cui-card { display: block; color: inherit; text-decoration: none; }
.cui-cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr)); gap: 1rem; margin: 0; padding: 0; list-style-position: inside; }
.cui-cards > li { box-sizing: border-box; padding: 1.1rem 1.1rem 1.2rem; border-radius: 16px; border: 1px solid color-mix(in srgb, currentColor 20%, transparent); }
a.cui-link-pill { display: inline-flex; align-self: flex-start; align-items: center; box-sizing: border-box; min-height: 44px; padding: 0 0.6rem; border: 7px solid transparent; background-clip: padding-box; border-radius: 999px; box-shadow: inset 0 0 0 1px currentColor; font-size: 0.82rem; text-decoration: none; }
a.cui-link-pill::before { content: ""; width: 0.5rem; height: 0.5rem; margin-inline-end: 0.5rem; border-radius: 50%; background: currentColor; }
"#;

/// The rules of a page's `measure:` and `scale:` (docs/www-port/look.md section 3), written only for a page that declares them, after the baseline and before the theme.
/// The gutter of a measured page is 1.25 rem (20 px), and the column is `measure` rem wide with the gutters inside it.
fn look_css(page: &PageParts) -> String {
    let mut css = String::new();
    if page.roles {
        css.push_str(ROLES_CSS);
    }
    if page.copy {
        css.push_str(COPY_CSS);
    }
    if let Some(m) = page.measure {
        css.push_str(&format!(
            ":root {{ --cui-measure: {m}rem; --cui-gutter: max(1.25rem, env(safe-area-inset-left), env(safe-area-inset-right), calc((100% - var(--cui-measure)) / 2 + 1.25rem)); }}\nbody {{ padding: 0; }}\nheader.cui-sticky, main, footer:not(:is(article, section, aside, main) footer) {{ box-sizing: border-box; padding-inline: var(--cui-gutter); }}\n"
        ));
    }
    if page.display_scale {
        css.push_str(concat!(
            "body { line-height: 1.6; }\n",
            ":not(pre) > code { padding: 0.08em 0.38em; border-radius: 6px; }\n",
            "h1 { font-size: clamp(2.1rem, 4.6vw, 3.05rem); line-height: 1.02; letter-spacing: -0.035em; font-weight: 800; margin: 0 0 1.1rem; }\n",
            "h1 em { font-style: normal; color: var(--cui-accent, LinkText); }\n",
            "h2 { font-size: clamp(1.6rem, 3.4vw, 2rem); line-height: 1.18; letter-spacing: -0.025em; font-weight: 700; margin: 0 0 0.6rem; }\n",
            ".cui-card h3 { font-size: 1.35rem; line-height: 1.25; letter-spacing: -0.02em; margin: 0; }\n",
            ".cui-card > p:not([data-style]), .cui-card .cui-col > p:not([data-style]) { margin: 0; font-size: 0.93rem; }\n",
            ".cui-card .cui-row a { font-size: 0.88rem; }\n",
            "h3, .cui-card h2, .cui-cards h2 { font-size: 1.05rem; line-height: 1.6; letter-spacing: normal; font-weight: 700; margin: 0 0 0.25rem; }\n",
            ".cui-grid:not([class*=\"cui-gap-\"]) { gap: 1rem; }\n",
            "main > section > .cui-reveal { margin-top: 1.2rem; }\n",
            "main > section > [data-style=\"lead\"] { margin-bottom: 1rem; }\n",
            "main > section { padding-top: 5rem; }\n",
            "main > section:first-child { padding-top: 3rem; }\n",
            "[data-style=\"eyebrow\"], [data-style=\"label\"] { letter-spacing: 0.12em; }\n",
            "[data-style=\"eyebrow\"] { margin-bottom: 0.6rem; }\n",
            "[data-style=\"lead\"] { font-size: 1.125rem; line-height: 1.6; max-width: 34rem; }\n",
        ));
    }
    if css.is_empty() {
        css
    } else {
        format!("<style id=\"cui-look\">\n{css}</style>\n")
    }
}

/// What a sticky `Header` adds: room above an anchor, 4rem (the header's least height) and one rem more, until the script has measured the header.
const STICKY_CSS: &str = "<style>html { scroll-padding-top: calc(var(--cui-header-h, 4rem) + 1rem); }\n.cui-sticky > .cui-row { margin-inline-start: auto; align-self: center; }\n.cui-sticky > img, .cui-sticky > picture { flex: none; margin-inline-end: -0.5rem; }\n.cui-sticky > img, .cui-sticky > picture img { display: block; width: 1.7rem; height: 1.7rem; border-radius: 50%; }\n.cui-sticky > nav { display: none; }\n.cui-sticky > nav a { text-decoration: none; }\n.cui-sticky .cui-chip { text-transform: none; letter-spacing: 0; }\n.cui-sticky .cui-row a { font-size: 0.85rem; font-weight: 600; }\n.cui-sticky summary { display: grid; place-items: center; min-width: 44px; font-size: 0; list-style: none; }\n.cui-sticky summary::-webkit-details-marker { display: none; }\n.cui-sticky summary::before { content: \"\"; display: block; width: 1.25rem; height: 2px; background: currentColor; box-shadow: 0 -6px 0 currentColor, 0 6px 0 currentColor; }\n@media (max-width: 29.99rem) { .cui-sticky .cui-row > a { display: none; } }\n@media (max-width: 39.99rem) { .cui-sticky > :has(> .cui-chip) { display: none; } }\n@media (min-width: 62rem) { .cui-sticky > nav { display: flex; margin-inline-start: 0.5rem; } .cui-sticky details { display: none; } }</style>\n";
/// Measures the sticky header (it may be taller than 4rem when its content wraps) and keeps `--cui-header-h` equal to it: at once, when the
/// document is parsed, at load, when the window is resized and when a font arrives. Inline, so the page does not wait for `app.js`.
const STICKY_JS: &str = "<script>(function(){var h,r=document.documentElement;function s(){if(!h){h=document.querySelector('.cui-sticky');if(h&&window.ResizeObserver){new ResizeObserver(s).observe(h)}}if(h){r.style.setProperty('--cui-header-h',h.offsetHeight+'px')}}s();addEventListener('DOMContentLoaded',s);addEventListener('load',s);addEventListener('resize',s);if(document.fonts&&document.fonts.ready){document.fonts.ready.then(s)}})()</script>\n";

/// `template` with each token replaced by its value, in one pass from left to right: a value is never searched for tokens.
fn expand(template: &str, tokens: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((at, token, value)) = tokens
        .iter()
        .filter_map(|(t, v)| rest.find(t).map(|i| (i, *t, *v)))
        .min_by_key(|(i, _, _)| *i)
    {
        out.push_str(&rest[..at]);
        out.push_str(value);
        rest = &rest[at + token.len()..];
    }
    out.push_str(rest);
    out
}

/// The root of the page: the static HTML the compiler wrote for it (docs/prerender.md), a script that goes to a `#fragment` link at once (a page
/// whose script has not run is not finished parsing, and the browser would wait), and what a visitor without script is told. A program with
/// nothing static keeps the old line; one with holes says some parts need script; one with none says nothing, because nothing is missing.
fn root_block(prerender: Option<&Prerender>, measure: &str) -> String {
    const HASH: &str = "<script>var h=decodeURIComponent(location.hash.slice(1)),e=h&&document.getElementById(h);if(e)e.scrollIntoView()</script>\n";
    match prerender {
        Some(p) if p.content => format!(
            "<div id=\"root\"{}>{}</div>\n{measure}{}{}",
            // a skeleton says which one it is, and the script built for it checks (docs/prerender.md)
            p.hash
                .as_ref()
                .map_or(String::new(), |h| format!(" data-cui-skeleton=\"{h}\"")),
            p.html,
            if p.ids > 0 { HASH } else { "" },
            if p.holes.is_empty() {
                ""
            } else {
                "<noscript>Some parts of this page need JavaScript.</noscript>\n"
            }
        ),
        _ => format!(
            "<div id=\"root\"></div>\n{measure}<noscript>This app needs JavaScript.</noscript>\n"
        ),
    }
}

fn manifest(name: &str) -> String {
    let value = serde_json::json!({
        "name": name,
        "short_name": name,
        "start_url": "./",
        "scope": "./",
        "display": "standalone",
        "background_color": "#ffffff",
        "theme_color": "#1f6feb",
        "icons": [
            { "src": "icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
            { "src": "icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" },
            { "src": "icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "maskable" },
            { "src": "icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
        ]
    });
    let mut text = cancho_ui_ir::pretty_json(&value);
    text.push('\n');
    text
}

/// FNV-1a, 64 bit. A cache version label: it changes when any precached byte changes. Not a security measure.
fn fnv1a64(parts: &[&[u8]]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // A separator, so that moving a byte from one file to the next changes the hash.
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn service_worker(files: &PwaFiles) -> String {
    let mut parts: Vec<&[u8]> = Vec::new();
    for (name, bytes) in files {
        parts.push(name.as_bytes());
        parts.push(bytes);
    }
    let version = format!("{:016x}", fnv1a64(&parts));
    let mut precache = vec!["./".to_string()];
    // a video is not kept offline: it is the one kind of file that would make the install many times the size of the app (docs/structure.md section 10)
    precache.extend(
        files
            .iter()
            .filter(|(name, _)| !(name.ends_with(".mp4") || name.ends_with(".webm")))
            .map(|(name, _)| format!("./{name}")),
    );
    let list = precache
        .iter()
        .map(|p| format!("    \"{p}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    SW_TEMPLATE
        .replace("@VERSION@", &version)
        .replace("@FILES@", &list)
}

const SW_TEMPLATE: &str = r#"// Generated by `cancho-ui build --pwa` (docs/mobile-backend.md, section 5). Do not edit.
// The cache name carries an FNV-1a 64 hash of every file it holds: a version label, not a security measure.
const CACHE = "cancho-ui-@VERSION@";
const PRECACHE = [
@FILES@
];

self.addEventListener("install", function (event) {
    event.waitUntil(caches.open(CACHE).then(function (cache) { return cache.addAll(PRECACHE); }));
});

self.addEventListener("activate", function (event) {
    event.waitUntil(
        caches.keys().then(function (keys) {
            return Promise.all(keys
                .filter(function (key) { return key.indexOf("cancho-ui-") === 0 && key !== CACHE; })
                .map(function (key) { return caches.delete(key); }));
        })
    );
});

self.addEventListener("fetch", function (event) {
    const request = event.request;
    if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) {
        return;
    }
    event.respondWith(
        caches.match(request, { ignoreSearch: true }).then(function (hit) {
            if (hit) { return hit; }
            return fetch(request).catch(function () {
                if (request.mode === "navigate") { return caches.match("./index.html"); }
                return Response.error();
            });
        })
    );
});
"#;

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// A zlib stream of stored (uncompressed) deflate blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut rest = raw;
    loop {
        let take = rest.len().min(65_535);
        let (block, tail) = rest.split_at(take);
        let last = tail.is_empty();
        out.push(u8::from(last));
        out.extend_from_slice(&(take as u16).to_le_bytes());
        out.extend_from_slice(&(!(take as u16)).to_le_bytes());
        out.extend_from_slice(block);
        if last {
            break;
        }
        rest = tail;
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

/// The centred square of the icon, `lo..hi` on both axes. The background fills the whole image (so a launcher that masks it
/// to a circle or a squircle shows no transparent corner) and this mark sits inside the maskable **safe zone**, the circle of
/// radius 40% of the image about its centre (https://www.w3.org/TR/appmanifest/#icon-masks): its farthest corner is at 28.3%.
fn mark_bounds(size: u32) -> (u32, u32) {
    (size * 3 / 10, size * 7 / 10)
}

/// A `size` by `size` palette PNG, one bit a pixel: a flat colour with a centred square.
fn icon_png(size: u32) -> Vec<u8> {
    let (lo, hi) = mark_bounds(size);
    let row_bytes = size.div_ceil(8) as usize;
    let mut raw = Vec::with_capacity((row_bytes + 1) * size as usize);
    for y in 0..size {
        raw.push(0); // filter: none
        let mut row = vec![0u8; row_bytes];
        if (lo..hi).contains(&y) {
            for x in lo..hi {
                row[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        raw.extend_from_slice(&row);
    }
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&[1, 3, 0, 0, 0]); // 1 bit, palette, deflate, no filter, no interlace
    chunk(&mut png, b"IHDR", &ihdr);
    let mut plte = Vec::new();
    plte.extend_from_slice(&BACKGROUND);
    plte.extend_from_slice(&MARK);
    chunk(&mut png, b"PLTE", &plte);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_and_adler32_match_their_published_check_values() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn fnv1a64_matches_the_published_vector() {
        // FNV-1a 64 of "a" is af63dc4c8601ec8c; the separator is part of this crate's use, so check the bare loop.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        hash ^= u64::from(b'a');
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        assert_eq!(hash, 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn a_name_is_escaped_in_the_page_and_in_the_manifest() {
        let files = generate(
            "//",
            "A \"b\" <c> & d",
            None,
            None,
            &[],
            None,
            &PageParts::default(),
        );
        let html = String::from_utf8(files[1].1.clone()).unwrap();
        assert!(html.contains("<title>A &quot;b&quot; &lt;c&gt; &amp; d</title>"));
        let manifest: serde_json::Value = serde_json::from_slice(&files[2].1).unwrap();
        assert_eq!(manifest["name"], "A \"b\" <c> & d");
    }

    #[test]
    fn a_page_is_not_an_app() {
        // `--page` (docs/mobile-backend.md section 5.2): the script and the page, and nothing a browser installs from.
        let files = generate_page("//", "Demo", None, None, &[], None, &PageParts::default());
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["app.js", "index.html"]);
        let html = String::from_utf8(files[1].1.clone()).unwrap();
        for banned in [
            "manifest",
            "serviceWorker",
            "theme-color",
            "apple-touch-icon",
            "icon-192",
        ] {
            assert!(!html.contains(banned), "{banned}");
        }
        assert!(html.contains("<title>Demo</title>") && html.contains("src=\"./app.js\""));
        assert!(
            html.contains("viewport-fit=cover"),
            "still a page for a phone"
        );
        // The page of `--pwa` is the same page plus those lines.
        let app = String::from_utf8(
            generate("//", "Demo", None, None, &[], None, &PageParts::default())[1]
                .1
                .clone(),
        )
        .unwrap();
        assert!(app.contains("manifest") && app.contains("serviceWorker"));
    }

    #[test]
    fn the_worker_version_follows_every_byte_it_caches() {
        let a = generate("one", "x", None, None, &[], None, &PageParts::default());
        let b = generate("two", "x", None, None, &[], None, &PageParts::default());
        let sw = |f: &PwaFiles| String::from_utf8(f.last().unwrap().1.clone()).unwrap();
        assert_ne!(sw(&a), sw(&b));
        assert_eq!(
            sw(&a),
            sw(&generate(
                "one",
                "x",
                None,
                None,
                &[],
                None,
                &PageParts::default()
            ))
        );
    }

    #[test]
    fn the_maskable_icon_keeps_its_mark_inside_the_safe_zone() {
        // #67: the manifest has a maskable entry for each icon, and the mark of that icon lies inside the circle of radius
        // 40% of the image, whatever the size, so a circular mask cuts nothing of it.
        let files = generate("//", "x", None, None, &[], None, &PageParts::default());
        let manifest: serde_json::Value = serde_json::from_slice(&files[2].1).unwrap();
        let icons = manifest["icons"].as_array().unwrap();
        for size in [192u32, 512] {
            let maskable = icons
                .iter()
                .find(|i| {
                    i["purpose"] == "maskable" && i["sizes"] == format!("{size}x{size}").as_str()
                })
                .unwrap_or_else(|| panic!("no maskable icon of {size}"));
            let src = maskable["src"].as_str().unwrap();
            assert!(
                files.iter().any(|(name, _)| name == src),
                "{src} is not written"
            );
            let (lo, hi) = mark_bounds(size);
            let centre = f64::from(size) / 2.0;
            let farthest = [(lo, lo), (hi, lo), (lo, hi), (hi, hi)]
                .iter()
                .map(|&(x, y)| {
                    ((f64::from(x) - centre).powi(2) + (f64::from(y) - centre).powi(2)).sqrt()
                })
                .fold(0.0, f64::max);
            assert!(
                farthest <= 0.4 * f64::from(size),
                "{size}: the mark reaches {farthest} from the centre, the safe zone is {}",
                0.4 * f64::from(size)
            );
        }
        // the "any" icons are still there, and every icon file is in the precache list of the worker
        assert!(icons.iter().any(|i| i["purpose"] == "any"));
        let sw = String::from_utf8(files.last().unwrap().1.clone()).unwrap();
        assert!(sw.contains("./icon-192.png") && sw.contains("./icon-512.png"));
    }
}
