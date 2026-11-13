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
/* RouteMap (docs/routes.md): a pin is a fill with the page's own colour as its number and outline; fills are 7.59, 5.08 and 6.39:1 against white and 9.63, 7.37 and 6.43:1 against the dark page, and the number is the page colour on the fill (the same pairs), so a pin is read by its number and the list, never by its colour alone */
:root { --cui-route-lodging: #0550ae; --cui-route-activity: #1a7f37; --cui-route-transport: #57606a; --cui-route-other: CanvasText; --cui-route-ink: Canvas; }
.cui-routemap-svg { display: block; width: 100%; max-width: 40rem; height: auto; }
.cui-rm-lodging { fill: var(--cui-route-lodging); } .cui-rm-activity { fill: var(--cui-route-activity); }
.cui-rm-transport { fill: var(--cui-route-transport); } .cui-rm-other { fill: var(--cui-route-other); }
.cui-rm-list { list-style: none; margin: 0; padding: 0; }
/* tone: a status text says what kind it is by a prefix as well as by colour, so colour is never the only signal */
[data-tone] { font-weight: 600; }
[data-tone="ok"] { color: #1a7f37; } [data-tone="ok"]::before { content: "\2713  "; }
[data-tone="warn"] { color: #9a6700; } [data-tone="warn"]::before { content: "\26A0  "; }
[data-tone="error"] { color: #cf222e; } [data-tone="error"]::before { content: "\2717  "; }
@media (prefers-color-scheme: dark) {
    :root { --cui-chart: #79c0ff; --cui-chart-open: #9198a1; --cui-route-lodging: #79c0ff; --cui-route-activity: #3fb950; --cui-route-transport: #9198a1; }
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
/* an avatar (docs/structure.md 12.0c): the initials of a person on a coloured disc; the colours are fixed, each is 4.5:1 or better with the white text */
[data-style="avatar"] { display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; flex: none; width: 2.25rem; height: 2.25rem; margin: 0; border: 2px solid Canvas; border-radius: 50%; font-size: 0.78rem; font-weight: 700; line-height: 1; text-transform: uppercase; background: #b41d26; color: #fff; }
[data-style="avatar"]:nth-of-type(4n+2) { background: #6b5585; }
[data-style="avatar"]:nth-of-type(4n+3) { background: #1e7e34; }
[data-style="avatar"]:nth-of-type(4n) { background: #884e00; }
[data-style="avatar"] + [data-style="avatar"] { margin-left: -0.6rem; }
/* a star on the corner of the avatar before it (docs/structure.md 12.0k): an organiser's mark; the avatar after it overlaps as it would without the star */
[data-style="star"] { display: inline-flex; align-items: center; justify-content: center; position: relative; z-index: 1; flex: none; box-sizing: border-box; align-self: flex-end; width: 0.95rem; height: 0.95rem; margin: 0 0 0 -0.8rem; border: 1px solid Canvas; border-radius: 50%; font-size: 0.55rem; line-height: 1; background: #fff; color: #7a4a00; }
[data-style="star"] + [data-style="avatar"] { margin-left: -0.15rem; }
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
/* a docked Nav (docs/structure.md 12.0b) is the tab bar at the foot of the window: fixed, centred, and the page keeps room under it */
.cui-dock { position: fixed; left: 50%; bottom: max(0.75rem, env(safe-area-inset-bottom)); transform: translateX(-50%); z-index: 10; box-sizing: border-box; width: min(calc(100% - 2rem), 28rem); display: flex; justify-content: space-around; gap: 0.25rem; padding: 0.4rem; border-radius: 999px; background: color-mix(in srgb, Canvas 86%, transparent); -webkit-backdrop-filter: blur(16px); backdrop-filter: blur(16px); box-shadow: 0 8px 28px -6px rgba(0,0,0,0.28); border: 1px solid color-mix(in srgb, currentColor 14%, transparent); }
.cui-dock > * { flex: 1 1 0; min-width: 0; }
body:has(.cui-dock) { padding-bottom: calc(5.5rem + env(safe-area-inset-bottom)); }
/* a floating button and a sheet (docs/structure.md 12.0g): fixed over the page, above the tab bar, shown only while an `Aside { marks: .. }` of their kind is on the page */
.cui-fab, .cui-sheet { position: fixed; z-index: 11; box-sizing: border-box; margin: 0; }
.cui-fab { right: 1rem; bottom: calc(5.25rem + env(safe-area-inset-bottom)); }
.cui-fab > button { min-height: 52px; padding: 0 1.4rem; box-shadow: 0 8px 24px -4px rgba(0,0,0,0.35); }
.cui-sheet { left: 50%; bottom: calc(5.25rem + env(safe-area-inset-bottom)); transform: translateX(-50%); width: min(calc(100% - 1.5rem), 28rem); max-height: calc(100dvh - 9.5rem); overflow-y: auto; display: flex; flex-direction: column; gap: 0.75rem; padding: 1rem; border-radius: 24px; background: var(--cui-surface, Canvas); color: var(--cui-fg, CanvasText); border: 1px solid color-mix(in srgb, currentColor 18%, transparent); box-shadow: 0 16px 48px -8px rgba(0,0,0,0.45); }
[data-mark] { display: none; }
/* a chat (docs/structure.md 12.0h, 12.0i): a log with a faint pattern, bubbles that lean to one side (a run of one sender has one avatar and one name, and a tail on its first bubble), day pills, reaction chips on a bubble's edge, a header and a composer in a pill */
.cui-chatlog { display: flex; flex-direction: column; gap: 0; flex: 1 1 auto; min-height: 9rem; overflow-y: auto; padding: 0.4rem 0.5rem 1rem; border-radius: 18px; background-color: var(--cui-soft, color-mix(in srgb, currentColor 6%, transparent)); background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120' fill='none' stroke='%23785a46' stroke-opacity='.11' stroke-width='1.4' stroke-linecap='round'%3E%3Ccircle cx='20' cy='22' r='7'/%3E%3Cpath d='M70 14v12M64 20h12'/%3E%3Cpath d='M98 52c6-8 12 8 18 0'/%3E%3Crect x='14' y='70' width='13' height='13' rx='3'/%3E%3Ccircle cx='66' cy='78' r='3'/%3E%3Cpath d='M92 96l8-10 8 10z'/%3E%3Cpath d='M40 108c4-6 8 6 12 0'/%3E%3Cpath d='M52 44l3 6 6 1-4 5 1 6-6-3-6 3 1-6-4-5 6-1z'/%3E%3C/svg%3E"); background-size: 120px 120px; }
.cui-bubble { --b: var(--cui-surface, Canvas); position: relative; box-sizing: border-box; max-width: calc(100% - 4.8rem); gap: 0.05rem; margin-top: 0.12rem; padding: 0.3rem 0.6rem 0.2rem; border-radius: 12px; background: var(--b); color: var(--cui-fg, CanvasText); box-shadow: 0 1px 1px rgba(0,0,0,0.2); overflow-wrap: anywhere; line-height: 1.3; }
.cui-bubble p { margin: 0; }
.cui-bubble-theirs { align-self: flex-start; margin-left: 2.4rem; }
.cui-bubble-mine { --b: var(--cui-accent-soft, #dcf8c6); align-self: flex-end; margin-right: 0.1rem; }
.cui-bubble-system { align-self: center; max-width: 92%; background: transparent; box-shadow: none; color: var(--cui-muted, inherit); font-size: 0.8rem; text-align: center; margin-top: 0.4rem; }
.cui-bubble-day { align-self: center; max-width: 92%; margin: 0.7rem 0 0.4rem; padding: 0.15rem 0.7rem; border-radius: 8px; font-size: 0.74rem; color: var(--cui-muted, inherit); }
.cui-bubble:has(> [data-style="mark"]) { margin-top: 0.5rem; }
.cui-bubble-theirs:has(> [data-style="mark"]) { border-top-left-radius: 0; }
.cui-bubble-mine:has(> [data-style="mark"]) { border-top-right-radius: 0; }
.cui-bubble:has(> [data-style="mark"])::before { content: ""; position: absolute; top: 0; width: 0; height: 0; border-top: 9px solid var(--b); }
.cui-bubble-theirs:has(> [data-style="mark"])::before { left: -8px; border-left: 8px solid transparent; }
.cui-bubble-mine:has(> [data-style="mark"])::before { right: -8px; border-right: 8px solid transparent; }
.cui-bubble:has(> [data-style="big"]) { --b: transparent; background: transparent; box-shadow: none; padding-inline: 0.2rem; }
.cui-bubble:has(> [data-style="big"])::before { display: none; }
.cui-bubble [data-style="mark"] { display: none; }
.cui-bubble [data-style="big"] { font-size: 2.6rem; line-height: 1.15; }
.cui-bubble [data-style="eyebrow"] { text-transform: none; letter-spacing: 0; font-size: 0.78rem; font-weight: 700; }
.cui-bubble [data-style^="hue"] { font-size: 0.8rem; font-weight: 700; line-height: 1.25; text-transform: none; letter-spacing: 0; }
.cui-bubble [data-style="note"] { align-self: flex-end; font-size: 0.66rem; color: var(--cui-muted, inherit); }
[data-style^="disc"] { display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; flex: none; width: 2rem; height: 2rem; margin: 0; border-radius: 50%; font-size: 0.72rem; font-weight: 700; line-height: 1; text-transform: uppercase; color: #fff; }
.cui-bubble > [data-style^="disc"] { position: absolute; left: -2.5rem; top: 0; }
.cui-bubble [data-style="hue0"] { color: #b41d26; }
[data-style="disc0"] { background: #b41d26; }
.cui-bubble [data-style="hue1"] { color: #0b6a73; }
[data-style="disc1"] { background: #0b6a73; }
.cui-bubble [data-style="hue2"] { color: #6b4a9e; }
[data-style="disc2"] { background: #6b4a9e; }
.cui-bubble [data-style="hue3"] { color: #1e7e34; }
[data-style="disc3"] { background: #1e7e34; }
.cui-bubble [data-style="hue4"] { color: #8a4b00; }
[data-style="disc4"] { background: #8a4b00; }
.cui-bubble [data-style="hue5"] { color: #1d5fa8; }
[data-style="disc5"] { background: #1d5fa8; }
.cui-bubble [data-style="hue6"] { color: #a1236f; }
[data-style="disc6"] { background: #a1236f; }
.cui-bubble [data-style="hue7"] { color: #556b00; }
[data-style="disc7"] { background: #556b00; }
/* a person's own look (docs/structure.md 12.0l): `face(..)` gives a disc a tone (twelve fixed colours, each 6.00:1 or better with white), an emoji in place of the initials, or a photo drawn on a canvas over it */
[data-tone="0"][data-style] { background: #b41d26; }
[data-tone="1"][data-style] { background: #0b6a73; }
[data-tone="2"][data-style] { background: #6b4a9e; }
[data-tone="3"][data-style] { background: #1e7e34; }
[data-tone="4"][data-style] { background: #8a4b00; }
[data-tone="5"][data-style] { background: #1d5fa8; }
[data-tone="6"][data-style] { background: #a1236f; }
[data-tone="7"][data-style] { background: #556b00; }
[data-tone="8"][data-style] { background: #475569; }
[data-tone="9"][data-style] { background: #4338a0; }
[data-tone="10"][data-style] { background: #7b2d8e; }
[data-tone="11"][data-style] { background: #0e6b8c; }
[data-style][data-emoji] { font-size: 1.5em; font-weight: 400; text-transform: none; }
[data-style][data-photo] { position: relative; overflow: hidden; }
.cui-face-pic { position: absolute; inset: 0; width: 100%; height: 100%; border-radius: 50%; }
[data-style="avatar"][data-photo] { border-color: Canvas; }
/* a quote of an earlier message, a mention, the time and the ticks (docs/structure.md 12.0j) */
.cui-quote { display: flex; flex-direction: column; gap: 0.05rem; min-width: 0; margin: 0.1rem 0 0.3rem; padding: 0.2rem 0.5rem; border-inline-start: 4px solid var(--q, #1e7e34); border-radius: 6px; background: color-mix(in srgb, currentColor 8%, transparent); }
.cui-bubble .cui-quote [data-style^="hue"] { font-size: 0.78rem; }
.cui-bubble .cui-quote [data-style="note"] { align-self: flex-start; max-width: 100%; box-sizing: border-box; font-size: 0.82rem; color: inherit; opacity: 0.85; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.cui-quote:has([data-style="hue0"]) { --q: #b41d26; }
.cui-quote:has([data-style="hue1"]) { --q: #0b6a73; }
.cui-quote:has([data-style="hue2"]) { --q: #6b4a9e; }
.cui-quote:has([data-style="hue3"]) { --q: #1e7e34; }
.cui-quote:has([data-style="hue4"]) { --q: #8a4b00; }
.cui-quote:has([data-style="hue5"]) { --q: #1d5fa8; }
.cui-quote:has([data-style="hue6"]) { --q: #a1236f; }
.cui-quote:has([data-style="hue7"]) { --q: #556b00; }
.cui-sheet > .cui-row.cui-strip:not(:first-child):not(.cui-composer) { flex: none; flex-wrap: nowrap; align-items: center; gap: 0.4rem; overflow-x: auto; }
.cui-strip > [data-style] { flex: 1 1 auto; min-width: 0; margin: 0; font-size: 0.82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
:root .cui-sheet > .cui-row.cui-strip:not(:first-child):not(.cui-composer) button { flex: none; min-width: 1.9rem; min-height: 1.9rem; padding: 0 0.7rem; border-radius: 999px; font-size: 0.85rem; line-height: 1; white-space: nowrap; background: var(--cui-soft, transparent); color: var(--cui-fg, inherit); border: 1px solid color-mix(in srgb, currentColor 22%, transparent); }
.cui-mention { padding: 0 0.25em; border-radius: 6px; font-weight: 600; background: color-mix(in srgb, var(--cui-accent, #1e7e34) 22%, transparent); }
.cui-meta { align-self: flex-end; display: flex; flex-wrap: nowrap; align-items: baseline; gap: 0.25rem; }
.cui-bubble .cui-meta > [data-style] { align-self: auto; }
.cui-bubble [data-style="tick"], .cui-bubble [data-style="tickblue"] { font-size: 0.72rem; line-height: 1; letter-spacing: -0.14em; color: var(--cui-muted, inherit); }
.cui-bubble [data-style="tickblue"] { color: #1d5fa8; }
.cui-sheet:has(> .cui-chatlog) { overflow: hidden; height: min(34rem, calc(100dvh - 9.5rem)); }
.cui-sheet > .cui-chatlog { min-height: 0; }
:root .cui-bubble button { min-height: 28px; min-width: 0; padding: 0 0.7rem; font-size: 0.78rem; background: transparent; color: var(--cui-accent, LinkText); border: 1px solid color-mix(in srgb, currentColor 35%, transparent); box-shadow: none; }
:root .cui-bubble > button { position: absolute; top: 50%; transform: translateY(-50%); width: 1.75rem; min-width: 1.75rem; height: 1.75rem; min-height: 1.75rem; padding: 0; border-radius: 50%; font-size: 0.95rem; line-height: 1; color: var(--cui-muted, inherit); background: var(--cui-surface, Canvas); border-color: transparent; box-shadow: 0 1px 2px rgba(0,0,0,0.25); }
:root .cui-bubble-theirs > button { right: -2.2rem; }
:root .cui-bubble-mine > button { left: -2.2rem; }
.cui-bubble:has(> .cui-chips) { margin-bottom: 0.95rem; }
.cui-chips { position: absolute; bottom: -0.85rem; left: 0.4rem; z-index: 1; display: flex; gap: 0.2rem; }
:root .cui-bubble .cui-chips button { min-height: 24px; min-width: 24px; padding: 0 0.4rem; font-size: 0.78rem; line-height: 1; color: var(--cui-fg, CanvasText); background: var(--cui-surface, Canvas); border: 1px solid color-mix(in srgb, currentColor 20%, transparent); border-radius: 999px; box-shadow: 0 1px 2px rgba(0,0,0,0.25); }
.cui-sheet > .cui-row:first-child { justify-content: space-between; align-items: center; }
.cui-sheet > .cui-row:first-child button { background: transparent; color: var(--cui-fg, CanvasText); border: 1px solid color-mix(in srgb, currentColor 35%, transparent); }
.cui-chathead { flex-wrap: nowrap; gap: 0.65rem; }
.cui-chathead > .cui-col { flex: 1 1 auto; min-width: 0; gap: 0; }
.cui-chathead > [data-style="avatar"] { width: 2.5rem; height: 2.5rem; margin: 0; border: 0; font-size: 0.85rem; }
.cui-chathead > .cui-col > * { margin: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.cui-chathead > .cui-col > [role="heading"] { font-size: 1.05rem; line-height: 1.25; }
.cui-chathead > .cui-col > [data-style="note"] { font-size: 0.78rem; }
.cui-chathead > button { flex: none; }
.cui-sheet > .cui-row:not(:first-child):not(.cui-composer) { flex-wrap: wrap; gap: 0.25rem; }
:root .cui-sheet > .cui-row:not(:first-child):not(.cui-composer) button { min-width: 2.2rem; min-height: 2.2rem; padding: 0 0.2rem; font-size: 1.2rem; background: var(--cui-soft, transparent); color: var(--cui-fg, inherit); border: 0; box-shadow: none; }
:root .cui-bubble > .cui-row:not(.cui-chips) button { min-width: 2rem; padding: 0 0.3rem; }
.cui-composer { position: relative; display: flex; flex-wrap: nowrap; align-items: center; gap: 0.15rem; }
.cui-composer::before { content: ""; position: absolute; left: 0; right: calc(46px + 0.45rem); top: 0; bottom: 0; border-radius: 999px; background: var(--cui-surface, Canvas); border: 1px solid color-mix(in srgb, currentColor 22%, transparent); }
.cui-composer:focus-within::before { border-color: var(--cui-accent, Highlight); box-shadow: 0 0 0 3px color-mix(in srgb, var(--cui-accent, Highlight) 40%, transparent); }
.cui-composer > * { position: relative; z-index: 1; }
.cui-composer > .cui-field { flex: 1 1 0; min-width: 0; font-size: 0; }
:root .cui-composer > .cui-field input:not([type="checkbox"]):not([type="file"]):not([type="range"]) { width: 100%; box-sizing: border-box; font-size: 1rem; border: 0; border-radius: 0; background: transparent; box-shadow: none; outline: none; padding-inline: 0.2rem; min-height: 2.6rem; }
:root .cui-composer > .cui-field input:focus-visible { outline: none; }
:root .cui-composer > button:not(:last-child) { flex: none; width: 2.4rem; min-width: 2.4rem; height: 2.4rem; min-height: 2.4rem; padding: 0; border: 0; border-radius: 50%; background: transparent; box-shadow: none; font-size: 1.2rem; line-height: 1; color: var(--cui-muted, inherit); }
.cui-composer > button:last-child { flex: none; width: 46px; min-width: 46px; height: 46px; margin-left: 0.3rem; padding: 0; border-radius: 50%; font-size: 0; display: inline-flex; align-items: center; justify-content: center; }
.cui-composer > button:last-child::before { content: "\27A4"; font-size: 1.25rem; line-height: 1; }
@media (prefers-color-scheme: dark) {
.cui-chatlog { background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120' fill='none' stroke='%23ffffff' stroke-opacity='.07' stroke-width='1.4' stroke-linecap='round'%3E%3Ccircle cx='20' cy='22' r='7'/%3E%3Cpath d='M70 14v12M64 20h12'/%3E%3Cpath d='M98 52c6-8 12 8 18 0'/%3E%3Crect x='14' y='70' width='13' height='13' rx='3'/%3E%3Ccircle cx='66' cy='78' r='3'/%3E%3Cpath d='M92 96l8-10 8 10z'/%3E%3Cpath d='M40 108c4-6 8 6 12 0'/%3E%3Cpath d='M52 44l3 6 6 1-4 5 1 6-6-3-6 3 1-6-4-5 6-1z'/%3E%3C/svg%3E"); }
.cui-bubble [data-style="hue0"] { color: #ff8a8a; }
.cui-bubble [data-style="hue1"] { color: #4fd1d9; }
.cui-bubble [data-style="hue2"] { color: #b79cff; }
.cui-bubble [data-style="hue3"] { color: #6fdba5; }
.cui-bubble [data-style="hue4"] { color: #ffb366; }
.cui-bubble [data-style="hue5"] { color: #7db5ff; }
.cui-bubble [data-style="hue6"] { color: #ff8ccf; }
.cui-bubble [data-style="hue7"] { color: #c9d65a; }
.cui-quote:has([data-style="hue0"]) { --q: #ff8a8a; }
.cui-quote:has([data-style="hue1"]) { --q: #4fd1d9; }
.cui-quote:has([data-style="hue2"]) { --q: #b79cff; }
.cui-quote:has([data-style="hue3"]) { --q: #6fdba5; }
.cui-quote:has([data-style="hue4"]) { --q: #ffb366; }
.cui-quote:has([data-style="hue5"]) { --q: #7db5ff; }
.cui-quote:has([data-style="hue6"]) { --q: #ff8ccf; }
.cui-quote:has([data-style="hue7"]) { --q: #c9d65a; }
.cui-bubble [data-style="tickblue"] { color: #7db5ff; }
}
:root:not(:has([data-mark="fab"])) .cui-fab { display: none; }
:root:not(:has([data-mark="sheet"])) .cui-sheet { display: none; }
/* a second floating button and a second sheet (docs/structure.md 12.0k): the button sits at the start side of the first, the sheet is the first one's twin and is drawn by its own marker */
.cui-fab2, .cui-sheet2 { position: fixed; z-index: 11; box-sizing: border-box; margin: 0; }
.cui-fab2 { right: 6.1rem; bottom: calc(5.25rem + env(safe-area-inset-bottom)); }
.cui-fab2 > button { min-height: 52px; min-width: 52px; padding: 0 1rem; border-radius: 999px; box-shadow: 0 8px 24px -4px rgba(0,0,0,0.35); }
:root:has([data-mark="fab2"]) .cui-fab > button { min-width: 4.5rem; justify-content: center; }
.cui-sheet2 { left: 50%; bottom: calc(5.25rem + env(safe-area-inset-bottom)); transform: translateX(-50%); width: min(calc(100% - 1.5rem), 28rem); max-height: calc(100dvh - 9.5rem); overflow-y: auto; display: flex; flex-direction: column; gap: 0.75rem; padding: 1rem; border-radius: 24px; background: var(--cui-surface, Canvas); color: var(--cui-fg, CanvasText); border: 1px solid color-mix(in srgb, currentColor 18%, transparent); box-shadow: 0 16px 48px -8px rgba(0,0,0,0.45); }
.cui-sheet2 > .cui-row:first-child { justify-content: space-between; align-items: center; }
.cui-sheet2 > .cui-row:first-child button { background: transparent; color: var(--cui-fg, CanvasText); border: 1px solid color-mix(in srgb, currentColor 35%, transparent); }
:root:not(:has([data-mark="fab2"])) .cui-fab2 { display: none; }
:root:not(:has([data-mark="sheet2"])) .cui-sheet2 { display: none; }
:root:has([data-mark="sheet2"]) .cui-fab, :root:has([data-mark="sheet2"]) .cui-fab2 { display: none; }
/* a band (docs/structure.md 12.0k): a block in the flow, shown only while an `Aside { marks: "band" }` is on the page: a sealed island can sit in it, which an `If` cannot hold */
.cui-band { display: block; margin: 0 0 0.75rem; }
:root:not(:has([data-mark="band"])) .cui-band { display: none; }
/* a second band (docs/structure.md 12.0l): the same, for a second sealed island that is not on the page all the time (a person's picture) */
.cui-band2 { display: block; margin: 0 0 0.75rem; }
:root:not(:has([data-mark="band2"])) .cui-band2 { display: none; }
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

const ROLES_CSS: &str = r#":root button { border-radius: 999px; border-color: transparent; background: var(--cui-accent, #185fa5); color: var(--cui-accent-fg, #ffffff); font-weight: 600; }
:root button:disabled { background: var(--cui-soft, #efece1); color: var(--cui-muted, #4f545b); }
/* an icon-only button (its content is an element, not text) keeps its own size in a column instead of stretching to a bar */
:root .cui-col > .cui-ico { align-self: flex-start; }
/* an icon link or button: the icon is larger, and a link that holds one is a round target, not an underlined word */
:root button:has(> *) { font-size: 1.25rem; line-height: 1; }
:root a[title]:has(> *) { text-decoration: none; display: inline-flex; align-items: center; justify-content: center; min-width: 44px; padding: 0 0.5rem; border-radius: 999px; background: var(--cui-soft, #efece1); font-size: 1.25rem; line-height: 1; }
:root button[aria-pressed="true"] { box-shadow: 0 0 0 3px color-mix(in srgb, var(--cui-accent, #185fa5) 35%, transparent); }
:root nav button { background: transparent; color: var(--cui-fg, currentColor); }
:root nav button[aria-pressed="true"] { background: var(--cui-accent, #185fa5); color: var(--cui-accent-fg, #ffffff); box-shadow: none; }
/* the tab bar is flat (docs/structure.md 12.0m): a filled pill, with no border (the pressed rule above sets the border's colour, and a button's default border style is outset: that was the thick 3D edge) and no ring */
:root .cui-dock button, :root .cui-dock button[aria-pressed="true"] { border: 0; box-shadow: none; }
:root .cui-dock button:focus:not(:focus-visible) { outline: none; }
/* keys (12.0m): icon-only buttons side by side in one row, small, wrapping only when the line is full. A `Row { keys: true }` is one; so is a Column that holds an icon-only button directly
   (its other children take the whole line, in their order, so what was a stack of round buttons is a row of keys without a change to the program) */
:root .cui-keys, :root .cui-col:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *), :root .cui-row:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) { display: flex; flex-direction: row; flex-wrap: wrap; align-items: center; gap: 0.5rem 0.35rem; }
:root .cui-col:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) > :not(.cui-ico) { flex: 1 1 100%; min-width: 0; }
:root .cui-keys > button, :root .cui-col:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) > .cui-ico, :root .cui-row:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) > .cui-ico { flex: none; align-self: center; box-sizing: border-box; min-width: 2.5rem; min-height: 2.5rem; height: 2.5rem; padding: 0 0.6rem; border: 1px solid color-mix(in srgb, currentColor 16%, transparent); border-radius: 12px; background: var(--cui-soft, #efece1); color: var(--cui-fg, CanvasText); font-size: 1.1rem; line-height: 1; box-shadow: none; display: inline-flex; align-items: center; justify-content: center; gap: 0.3rem; }
:root .cui-keys > button[aria-pressed="true"], :root .cui-col:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) > .cui-ico[aria-pressed="true"], :root .cui-row:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) > .cui-ico[aria-pressed="true"] { background: color-mix(in srgb, var(--cui-accent, #185fa5) 16%, var(--cui-soft, #efece1)); color: var(--cui-accent, #185fa5); border-color: color-mix(in srgb, var(--cui-accent, #185fa5) 55%, transparent); box-shadow: none; }
:root .cui-keys > button:disabled { opacity: 0.55; }
/* a key with a caption: its second child, small, under the icon; the label is still its accessible name */
:root .cui-keys > button:has(> :nth-child(2)), :root .cui-row:has(> .cui-ico):not(.cui-bubble):not(.cui-chatlog):not(.cui-quote):not(.cui-overlay):not(.cui-chathead):not(.cui-composer):not(.cui-strip):not(.cui-chips):not(.cui-meta):not(.cui-bar):not(.cui-dock):not(.cui-sheet *):not(.cui-sheet2 *):not(.cui-band *):not(.cui-band2 *):not(.cui-bar *) > .cui-ico:has(> :nth-child(2)) { flex-direction: column; height: auto; min-height: 2.7rem; padding: 0.2rem 0.7rem; gap: 0.1rem; }
:root .cui-keys > button > * { margin: 0; line-height: 1.1; }
:root .cui-keys > button > :nth-child(2) { font-size: 0.64rem; font-weight: 600; letter-spacing: 0.01em; white-space: nowrap; }
:root .cui-row > .cui-keys { margin-inline-start: auto; }
:root .cui-keys > details { flex: none; }
:root .cui-keys > details[open] { flex: 1 1 100%; order: 9; }
/* a hint (12.0m): a Details that is a small round "i" or "more" button, not a "+" pill; what it holds opens under it, on a line of its own */
:root details.cui-hint:not(.cui-chart) > summary { min-height: 2rem; min-width: 2rem; padding: 0 0.45rem; justify-content: center; border: 0; border-radius: 999px; background: var(--cui-soft, #efece1); color: var(--cui-fg, CanvasText); font-size: 0.95rem; font-weight: 400; line-height: 1; }
:root details.cui-hint:not(.cui-chart) > summary::before, :root details.cui-hint[open]:not(.cui-chart) > summary::before { content: none; margin: 0; }
:root details.cui-hint[open]:not(.cui-chart) > summary { margin-bottom: 0.5rem; }
:root .cui-col:has(> .cui-ico) > details.cui-hint:not([open]) { flex: none; }
/* the bar of places (12.0m): one row that scrolls sideways, an icon over a tiny caption on each, the open place filled, groups divided by a hairline (a nested Row is a group) */
:root .cui-bar { display: flex; flex-wrap: nowrap; align-items: stretch; gap: 0.15rem; overflow-x: auto; overscroll-behavior-x: contain; scroll-snap-type: x proximity; scrollbar-width: none; padding: 0.15rem 0.1rem; }
:root .cui-bar::-webkit-scrollbar { display: none; }
:root .cui-bar > .cui-row { flex: none; flex-wrap: nowrap; align-items: stretch; gap: 0.15rem; padding-inline-end: 0.4rem; margin-inline-end: 0.25rem; border-inline-end: 1px solid color-mix(in srgb, currentColor 16%, transparent); }
:root .cui-bar > .cui-row:last-child { border-inline-end: 0; margin-inline-end: 0; padding-inline-end: 0; }
:root .cui-bar button { flex: none; scroll-snap-align: center; box-sizing: border-box; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.12rem; min-width: 3.5rem; min-height: 3.2rem; height: auto; padding: 0.3rem 0.45rem; border: 0; border-radius: 14px; background: transparent; box-shadow: none; color: var(--cui-muted, currentColor); font-size: 0.66rem; font-weight: 600; line-height: 1.1; white-space: nowrap; }
:root .cui-bar button > * { margin: 0; font-size: inherit; line-height: 1.1; }
:root .cui-bar button > :first-child { font-size: 1.3rem; line-height: 1; }
:root .cui-bar button[aria-pressed="true"] { background: color-mix(in srgb, var(--cui-accent, #185fa5) 14%, transparent); color: var(--cui-accent, #185fa5); box-shadow: none; }
/* fields (docs/structure.md 12.0e): a pill, a soft fill, one thin border, and a ring in the accent when it has the focus */
:root input:not([type="checkbox"]):not([type="file"]):not([type="range"]), :root select { border-radius: 999px; border-style: solid; border-width: 1px; padding-inline: 1.1rem; min-height: 2.75rem; }
:root textarea { border-radius: 20px; border-style: solid; border-width: 1px; padding: 0.7rem 1.1rem; }
/* a disclosure (docs/structure.md 12.0f): its summary is a pill with a plus that turns into a minus when it is open; the inside waits under it */
:root details:not(.cui-chart) > summary { display: inline-flex; box-sizing: border-box; padding: 0 1.1rem; border: 1px solid var(--cui-muted, currentColor); border-radius: 999px; font-weight: 600; list-style: none; }
:root details:not(.cui-chart) > summary::-webkit-details-marker { display: none; }
:root details:not(.cui-chart) > summary::before { content: "+"; margin-inline-end: 0.5rem; font-weight: 700; }
:root details[open]:not(.cui-chart) > summary::before { content: "\2212"; }
:root details[open]:not(.cui-chart) > summary { margin-bottom: 0.75rem; }
:root input:focus-visible, :root textarea:focus-visible, :root select:focus-visible { outline: 3px solid color-mix(in srgb, var(--cui-accent, #185fa5) 55%, transparent); outline-offset: 1px; border-color: var(--cui-accent, #185fa5); }
.cui-card { box-sizing: border-box; padding: 1.1rem 1.2rem 1.3rem; border-radius: 16px; border: 1px solid color-mix(in srgb, currentColor 20%, transparent); }
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
/* a cover card (docs/structure.md 12.0d): the picture on top with the second child over its foot on a dark scrim, and the rest as the panel under it */
.cui-overlay-cover { position: relative; border-radius: 24px; overflow: hidden; background: Canvas; }
.cui-overlay-cover > :first-child { grid-area: 1 / 1; background: #0b1016; }
.cui-overlay-cover > :first-child img { display: block; width: 100%; max-width: none; aspect-ratio: 16 / 10; object-fit: cover; }
.cui-overlay-cover > picture:first-child { max-width: none !important; }
.cui-overlay-cover > :nth-child(2) { grid-area: 1 / 1; align-self: end; justify-self: stretch; box-sizing: border-box; margin: 0; padding: 3rem 1.1rem 0.9rem; border-radius: 0; background: linear-gradient(to top, rgba(0,0,0,0.7), rgba(0,0,0,0)); color: #fff; font-size: inherit; box-shadow: none; z-index: 1; }
.cui-overlay-cover > :nth-child(n+3) { grid-area: auto; grid-column: 1; align-self: auto; justify-self: stretch; box-sizing: border-box; margin: 0; padding: 0.4rem 1.1rem; border-radius: 0; background: none; color: inherit; font-size: inherit; box-shadow: none; }
.cui-overlay-cover > :nth-child(3) { padding-top: 0.9rem; }
.cui-overlay-cover > :last-child:nth-child(n+3) { padding-bottom: 1.1rem; }
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
