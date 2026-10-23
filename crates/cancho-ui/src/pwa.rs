//! `cancho-ui build --pwa`: the files that make a compiled program an installable web app.
//!
//! Designed in `docs/mobile-backend.md` first. Everything here is deterministic text or bytes with no clock, no
//! network and no image or hashing library: the PNG icons are written by hand (a palette image in a stored-deflate
//! zlib stream) and the cache version is FNV-1a 64, which is a version label and not a security measure.

use std::path::Path;

/// The files of one PWA, in the order they are written: `(file name, bytes)`.
pub type PwaFiles = Vec<(&'static str, Vec<u8>)>;

const BACKGROUND: [u8; 3] = [0x1f, 0x6f, 0xeb];
const MARK: [u8; 3] = [0xff, 0xff, 0xff];

/// Everything `--pwa` writes for a program compiled to `js`, named `name`.
pub fn generate(js: &str, name: &str) -> PwaFiles {
    let mut files: PwaFiles = vec![
        ("app.js", js.as_bytes().to_vec()),
        ("index.html", index_html(name, true).into_bytes()),
        ("manifest.webmanifest", manifest(name).into_bytes()),
        ("icon-192.png", icon_png(192)),
        ("icon-512.png", icon_png(512)),
    ];
    let sw = service_worker(&files);
    files.push(("sw.js", sw.into_bytes()));
    files
}

/// What `--page` writes for a program compiled to `js`, named `name`: a web page and its script, and nothing that makes it an app
/// (no manifest, no icons, no service worker, no `theme-color`), so a browser has nothing to offer to install and nothing
/// caches it behind the author's back (docs/mobile-backend.md section 5.2).
pub fn generate_page(js: &str, name: &str) -> PwaFiles {
    vec![
        ("app.js", js.as_bytes().to_vec()),
        ("index.html", index_html(name, false).into_bytes()),
    ]
}

/// Write `files` into `dir`, creating it. Nothing is written before the caller has a program that compiled.
pub fn write_all(dir: &Path, files: &PwaFiles) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creating `{}`: {}", dir.display(), e))?;
    for (file, bytes) in files {
        let path = dir.join(file);
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
fn index_html(name: &str, installable: bool) -> String {
    let title = html_escape(name);
    let mut html = String::from(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
@APP_THEME@<title>@TITLE@</title>
@APP_LINKS@<style>
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
/* gap: 0..4 on a Row or Column (docs/presentation.md section 2) */
.cui-gap-0 { gap: 0; } .cui-gap-1 { gap: 0.25rem; } .cui-gap-2 { gap: 0.5rem; } .cui-gap-3 { gap: 1rem; } .cui-gap-4 { gap: 2rem; }
/* a TextField with a label: the label is shown above the field */
.cui-field { display: flex; flex-direction: column; gap: 0.25rem; }
/* a Checkbox: the box and its text in one label that is the touch target (docs/controls.md section 9) */
.cui-check { display: flex; align-items: center; gap: 0.5rem; min-height: 44px; min-width: 44px; }
.cui-check input { min-height: 0; min-width: 0; width: 1.25rem; height: 1.25rem; margin: 0; }
/* Table */
.cui-table { border-collapse: collapse; width: 100%; }
.cui-table th, .cui-table td { text-align: start; padding: 0.25rem 0.5rem; border-bottom: 1px solid color-mix(in srgb, currentColor 25%, transparent); }
.cui-table th { font-weight: 600; }
/* Chart (docs/charts.md): the series colour is 7.59:1 on white and 9.63:1 on the dark page (WCAG 1.4.11 asks 3:1) */
:root { --cui-chart: #0550ae; }
.cui-chart { margin: 0; }
.cui-chart figcaption { font-weight: 600; }
.cui-chart summary { min-height: 44px; display: flex; align-items: center; cursor: pointer; }
/* tone: a status text says what kind it is by a prefix as well as by colour, so colour is never the only signal */
[data-tone] { font-weight: 600; }
[data-tone="ok"] { color: #1a7f37; } [data-tone="ok"]::before { content: "\2713  "; }
[data-tone="warn"] { color: #9a6700; } [data-tone="warn"]::before { content: "\26A0  "; }
[data-tone="error"] { color: #cf222e; } [data-tone="error"]::before { content: "\2717  "; }
@media (prefers-color-scheme: dark) {
    :root { --cui-chart: #79c0ff; }
    [data-tone="ok"] { color: #3fb950; } [data-tone="warn"] { color: #d29922; } [data-tone="error"] { color: #ff7b72; }
}
button, input, textarea { min-height: 44px; min-width: 44px; font: inherit; }
</style>
</head>
<body>
<div id="root"></div>
<noscript>This app needs JavaScript.</noscript>
<script type="module" src="./app.js"></script>
@APP_REGISTER@</body>
</html>
"##,
    );
    html = html.replace("@TITLE@", &title);
    let (theme, links, register) = if installable {
        (
            "<meta name=\"theme-color\" content=\"#1f6feb\">\n",
            "<link rel=\"manifest\" href=\"./manifest.webmanifest\">\n<link rel=\"icon\" type=\"image/png\" sizes=\"192x192\" href=\"./icon-192.png\">\n<link rel=\"apple-touch-icon\" href=\"./icon-192.png\">\n",
            "<script>\nif ('serviceWorker' in navigator) {\n    window.addEventListener('load', function () { navigator.serviceWorker.register('./sw.js'); });\n}\n</script>\n",
        )
    } else {
        ("", "", "")
    };
    html.replace("@APP_THEME@", theme)
        .replace("@APP_LINKS@", links)
        .replace("@APP_REGISTER@", register)
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
    precache.extend(files.iter().map(|(name, _)| format!("./{name}")));
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
        let files = generate("//", "A \"b\" <c> & d");
        let html = String::from_utf8(files[1].1.clone()).unwrap();
        assert!(html.contains("<title>A &quot;b&quot; &lt;c&gt; &amp; d</title>"));
        let manifest: serde_json::Value = serde_json::from_slice(&files[2].1).unwrap();
        assert_eq!(manifest["name"], "A \"b\" <c> & d");
    }

    #[test]
    fn a_page_is_not_an_app() {
        // `--page` (docs/mobile-backend.md section 5.2): the script and the page, and nothing a browser installs from.
        let files = generate_page("//", "Demo");
        let names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
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
        let app = String::from_utf8(generate("//", "Demo")[1].1.clone()).unwrap();
        assert!(app.contains("manifest") && app.contains("serviceWorker"));
    }

    #[test]
    fn the_worker_version_follows_every_byte_it_caches() {
        let a = generate("one", "x");
        let b = generate("two", "x");
        let sw = |f: &PwaFiles| String::from_utf8(f.last().unwrap().1.clone()).unwrap();
        assert_ne!(sw(&a), sw(&b));
        assert_eq!(sw(&a), sw(&generate("one", "x")));
    }

    #[test]
    fn the_maskable_icon_keeps_its_mark_inside_the_safe_zone() {
        // #67: the manifest has a maskable entry for each icon, and the mark of that icon lies inside the circle of radius
        // 40% of the image, whatever the size, so a circular mask cuts nothing of it.
        let files = generate("//", "x");
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
                files.iter().any(|(name, _)| *name == src),
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
