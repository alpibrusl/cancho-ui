//! `page { .. }`, the skip link and the sticky `Header` (docs/page.md): every refusal of `tests/page_corpus.json` is refused by its rule tag, the
//! valid programs check, the head is written into `index.html` as static HTML before any script, the skip link is the first element of the body,
//! the files the page names are copied, hashed and (the card) measured, the host the metadata names is listed by `inspect effects` and `diff`, and
//! the page makes no request. What the browser does with it is checked by `scripts/check-page.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/page_corpus.json");
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

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/page")
            .join(name),
    )
    .expect("a fixture is committed")
}

fn scratch_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-page-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn unhex(h: &str) -> Vec<u8> {
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// The corpus files, written beside a program.
fn write_files(c: &Value, dir: &Path) {
    for (name, v) in c["files"].as_object().expect("files") {
        let path = dir.join(name);
        let bytes: Vec<u8> = match v {
            Value::String(s) => s.as_bytes().to_vec(),
            o if o.get("fixture").is_some() => fixture(o["fixture"].as_str().expect("name")),
            o if o.get("hex").is_some() => unhex(o["hex"].as_str().expect("hex")),
            o if o.get("sparse").is_some() => unhex(o["head"].as_str().expect("head")),
            other => panic!("a corpus file is {other}"),
        };
        std::fs::write(&path, bytes).expect("a corpus file");
        if let Some(n) = v.get("sparse").and_then(Value::as_u64) {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .expect("open")
                .set_len(n)
                .expect("a sparse file");
        }
    }
}

/// `{N:c}` is the character c, N times.
fn expand(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        let (head, tail) = rest.split_at(i);
        out.push_str(head);
        let close = tail.find('}').unwrap_or(0);
        let spec = tail.get(1..close).unwrap_or("");
        match spec.split_once(':') {
            Some((n, ch)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                out.push_str(&ch.repeat(n.parse().unwrap_or(0)));
                rest = &tail[close + 1..];
            }
            _ => {
                out.push('{');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn source_of(c: &Value, case: &Value) -> String {
    if let Some(whole) = case["program"].as_str() {
        return expand(whole);
    }
    let view = case["view"]
        .as_str()
        .unwrap_or(c["default_view"].as_str().expect("default_view"));
    let state = case["extra_state"].as_str().unwrap_or("");
    let comp = c["component"]
        .as_str()
        .expect("component")
        .replace("{state}", state)
        .replace("{view}", view);
    expand(&format!("{}\n{comp}", case["page"].as_str().unwrap_or("")))
}

fn workspace(c: &Value, source: &str) -> PathBuf {
    let dir = scratch_dir();
    write_files(c, &dir);
    std::fs::write(dir.join("p.cui"), source).expect("the program");
    dir.join("p.cui")
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
    "page-duplicate",
    "page-entry-unknown",
    "page-entry-duplicate",
    "page-title-missing",
    "page-lang-missing",
    "page-lang-invalid",
    "page-description-length",
    "page-text-invalid",
    "page-canonical-not-https",
    "page-canonical-invalid",
    "page-image-alt-missing",
    "page-image-needs-canonical",
    "page-image-size-unreadable",
    "page-file-invalid",
    "page-alternate-title-missing",
    "page-skip-link-empty",
    "page-measure-invalid",
    "page-scale-invalid",
    "skip-link-target-missing",
    "skip-link-target-dynamic",
    "header-sticky-invalid",
    "header-sticky-duplicate",
    "header-sticky-misplaced",
    "asset-missing",
    "asset-too-large",
    "asset-type-mismatch",
    "anchor-duplicate",
];

#[test]
fn every_valid_program_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let file = workspace(&c, &source_of(&c, case));
        let (code, found) = codes(&file);
        assert_eq!((code, found), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    for case in c["refusals"].as_array().expect("refusals") {
        let file = workspace(&c, &source_of(&c, case));
        let (code, found) = codes(&file);
        let tag = case["tag"].as_str().expect("tag");
        assert_eq!(code, 1, "{}: exit code (found {found:?})", case["name"]);
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
            "spec/diagnostics/README.md does not document `{tag}`"
        );
        assert!(
            reached.contains(tag),
            "no case of the corpus reaches `{tag}`"
        );
    }
    for tag in &reached {
        assert!(
            TAGS.contains(tag),
            "`{tag}` is not in the list of this test"
        );
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let c = corpus();
    let case = &c["valid"][1];
    let full = source_of(&c, case);
    let file = workspace(&c, &full);
    let dir = file.parent().expect("dir").to_path_buf();
    let mut cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).collect();
    cuts.push(full.len());
    // every prefix; and every one-character deletion
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

fn full_program(c: &Value) -> PathBuf {
    workspace(c, &source_of(c, &c["valid"][1]))
}

fn build(file: &Path, flag: &str, name: &str) -> (PathBuf, String) {
    let out = file.parent().expect("dir").join(format!("out-{name}"));
    let _ = std::fs::remove_dir_all(&out);
    let (code, o, e) = run(&["build", p(file), flag, "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    (out, html)
}

#[test]
fn the_head_is_static_html_in_the_page() {
    let c = corpus();
    let (out, html) = build(&full_program(&c), "--page", "a");
    let head = html.split("</head>").next().expect("a head");
    for want in [
        "<html lang=\"en\">",
        "<title>T</title>",
        "<meta name=\"description\" content=\"A description.\">",
        "<meta property=\"og:title\" content=\"T\">",
        "<meta property=\"og:description\" content=\"A card text.\">",
        "<meta property=\"og:type\" content=\"website\">",
        "<meta property=\"og:url\" content=\"https://example.org/dir/\">",
        "<meta property=\"og:image\" content=\"https://example.org/dir/card.png\">",
        "<meta property=\"og:image:width\" content=\"120\">",
        "<meta property=\"og:image:height\" content=\"63\">",
        "<meta property=\"og:image:alt\" content=\"A card.\">",
        "<meta name=\"twitter:card\" content=\"summary_large_image\">",
        "<link rel=\"canonical\" href=\"https://example.org/dir/\">",
        "<meta name=\"color-scheme\" content=\"light dark\">",
        "<link rel=\"icon\" type=\"image/png\" href=\"icon.png\">",
        "<link rel=\"apple-touch-icon\" href=\"icon.png\">",
        "<link rel=\"alternate\" type=\"text/markdown\" href=\"a.md\" title=\"As text\">",
        "<link rel=\"alternate\" type=\"application/json\" href=\"a.json\" title=\"As data\">",
    ] {
        assert!(head.contains(want), "{want}\n{head}");
    }
    // the head is the document's, and no script is needed to read it: it is before the first script of the body
    assert!(!head.contains("<script"), "{head}");
    for f in ["card.png", "icon.png", "a.md", "a.json"] {
        assert!(out.join(f).is_file(), "{f} is copied beside the page");
    }
    // the skip link is the first element of the body, before the root
    let body = html.split("<body>").nth(1).expect("a body").trim_start();
    assert!(
        body.starts_with("<a class=\"cui-skip\" href=\"#main\">Skip to content</a>"),
        "{body}"
    );
    assert!(
        body.find("class=\"cui-skip\"") < body.find("id=\"root\""),
        "the skip link comes before the page"
    );
    assert!(
        body.contains("<main id=\"main\" tabindex=\"-1\">"),
        "the target is in the prerendered HTML: {body}"
    );
}

#[test]
fn the_language_of_the_page_is_the_one_declared() {
    let c = corpus();
    let src =
        "page { title: \"T\"; lang: \"es-419\"; }\ncomponent C {\n    view { Text { \"x\" } }\n}\n";
    let (_, html) = build(&workspace(&c, src), "--page", "lang");
    assert!(html.contains("<html lang=\"es-419\">"), "{html}");
}

#[test]
fn a_program_without_a_page_is_the_page_it_was() {
    let c = corpus();
    let dir = scratch_dir();
    write_files(&c, &dir);
    let f = dir.join("n.cui");
    std::fs::write(&f, "component C {\n    view { Text { \"x\" } }\n}\n").expect("program");
    let (_, html) = build(&f, "--page", "n");
    assert!(html.contains("<html lang=\"en\">") && html.contains("<title>n</title>"));
    for banned in [
        "og:",
        "cui-skip\" href",
        "canonical",
        "rel=\"alternate\"",
        "--cui-header-h",
    ] {
        assert!(!html.contains(banned), "{banned}");
    }
}

#[test]
fn text_of_the_page_is_text_and_never_a_token_or_markup() {
    let c = corpus();
    let src = source_of(
        &c,
        &serde_json::json!({ "page": "page { title: \"</title><b>@ROOT@ @THEME_CSS@ & \\\"q\\\"\"; lang: \"en\"; description: \"@PAGE_HEAD@ <x>\"; skip_link: \"@SKIP@ <y>\"; }" }),
    );
    let file = workspace(&c, &src);
    let (_, html) = build(&file, "--page", "esc");
    assert!(
        html.contains(
            "<title>&lt;/title&gt;&lt;b&gt;@ROOT@ @THEME_CSS@ &amp; &quot;q&quot;</title>"
        ),
        "{html}"
    );
    assert!(html.contains("content=\"@PAGE_HEAD@ &lt;x&gt;\""));
    assert!(
        html.contains("<meta property=\"og:title\" content=\"&lt;/title&gt;&lt;b&gt;@ROOT@ @THEME_CSS@ &amp; &quot;q&quot;\">"),
        "{html}"
    );
    assert!(html.contains(">@SKIP@ &lt;y&gt;</a>"));
}

#[test]
fn the_page_title_replaces_the_name_and_pwa_keeps_the_icons_of_the_page() {
    let c = corpus();
    let file = full_program(&c);
    let out = file.parent().expect("dir").join("out-named");
    let (code, o, e) = run(&["build", p(&file), "--pwa", "--name", "Other", "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("page");
    assert!(html.contains("<title>T</title>") && !html.contains("Other"));
    assert_eq!(
        html.matches("rel=\"icon\"").count(),
        1,
        "one icon, the page's"
    );
    assert_eq!(html.matches("rel=\"apple-touch-icon\"").count(), 1);
    assert!(html.contains("rel=\"manifest\""));
    let manifest = std::fs::read_to_string(out.join("manifest.webmanifest")).expect("manifest");
    assert!(manifest.contains("\"Other\""), "the app keeps its name");
    let sw = std::fs::read_to_string(out.join("sw.js")).expect("worker");
    assert!(
        sw.contains("card.png") && sw.contains("a.json"),
        "the worker caches the page's files"
    );
}

#[test]
fn a_sticky_header_reserves_room_above_an_anchor_and_no_header_does_not() {
    let c = corpus();
    let sticky = serde_json::json!({ "page": "page { title: \"T\"; lang: \"en\"; }", "view": "Header { sticky: true Link { to: \"#main\" \"Top\" } } Main { id: \"main\" Text { \"x\" } }" });
    let (_, html) = build(&workspace(&c, &source_of(&c, &sticky)), "--page", "s");
    assert!(html.contains("<header class=\"cui-sticky\">"), "{html}");
    assert!(html.contains("scroll-padding-top: calc(var(--cui-header-h, 4rem) + 1rem)"));
    assert!(html.contains("setProperty('--cui-header-h'"));
    let plain = serde_json::json!({ "page": "page { title: \"T\"; lang: \"en\"; }", "view": "Header { Link { to: \"#main\" \"Top\" } } Main { id: \"main\" Text { \"x\" } }" });
    let (_, html) = build(&workspace(&c, &source_of(&c, &plain)), "--page", "p");
    assert!(html.contains("<header>") && !html.contains("scroll-padding-top"));
    assert!(!html.contains("--cui-header-h"));
}

#[test]
fn the_size_of_the_card_is_read_from_each_format() {
    let c = corpus();
    for (file, w, h) in [
        ("card.png", 120, 63),
        ("card.jpg", 90, 40),
        ("lossy.webp", 64, 32),
        ("lossless.webp", 48, 24),
    ] {
        let src = format!(
            "page {{ title: \"T\"; lang: \"en\"; canonical: \"https://example.org/\"; image: \"{file}\" alt: \"a\"; }}\ncomponent C {{ view {{ Text {{ \"x\" }} }} }}\n"
        );
        let f = workspace(&c, &src);
        let (code, out, err) = run(&["inspect", "page", p(&f)]);
        assert_eq!(code, 0, "{err}");
        let v: Value = serde_json::from_str(&out).expect("JSON");
        assert_eq!(
            (v["image"]["width"].as_u64(), v["image"]["height"].as_u64()),
            (Some(w), Some(h)),
            "{file}"
        );
        assert_eq!(
            v["image"]["url"],
            format!("https://example.org/{file}").as_str()
        );
    }
}

#[test]
fn the_host_the_metadata_names_is_listed_and_the_page_asks_nothing_of_it() {
    let c = corpus();
    let f = full_program(&c);
    let (code, out, err) = run(&["inspect", "effects", p(&f), "C"]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["page_hosts"], serde_json::json!(["example.org"]));
    assert_eq!(v["requires"], serde_json::json!([]), "no capability");
    // no component of a program with no page lists one
    let (_, out, _) = run(&[
        "inspect",
        "effects",
        p(&workspace(
            &c,
            "component C {\n    view { Text { \"x\" } }\n}\n",
        )),
        "C",
    ]);
    assert!(!out.contains("page_hosts"));
}

#[test]
fn diff_reports_the_head_as_lines_of_its_own_and_authority_does_not_move() {
    let c = corpus();
    let base = full_program(&c);
    let src = std::fs::read_to_string(&base).expect("source");
    let dir = base.parent().expect("dir").to_path_buf();
    let change = |name: &str, from: &str, to: &str| -> PathBuf {
        let f = dir.join(name);
        std::fs::write(&f, src.replace(from, to)).expect("candidate");
        f
    };
    let cand = change("c1.cui", "title: \"T\";", "title: \"U\";");
    let (code, human, err) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{human}{err}");
    assert!(human.contains("~ page title: T -> U"), "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
    let cand = change(
        "c2.cui",
        "https://example.org/dir/",
        "https://example.net/dir/",
    );
    let (code, human, _) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5);
    assert!(
        human.contains("~ page host: example.org -> example.net"),
        "{human}"
    );
    assert!(human.contains("authority: unchanged"), "{human}");
    // a changed file is a changed hash
    let cand = change("c3.cui", "icon: \"icon.png\";", "icon: \"icon.ico\";");
    let (_, human, _) = run(&["diff", p(&base), p(&cand)]);
    assert!(human.contains("page icon: icon.png -> icon.ico"), "{human}");
    assert!(human.contains("page icon.sha256"), "{human}");
    // JSON: a list of settings, absent when equal
    let (code, out, _) = run(&["diff", p(&base), p(&base), "--output", "json"]);
    assert_eq!(code, 0);
    assert!(!out.contains("\"page\""), "{out}");
    let cand = change(
        "c4.cui",
        "skip_link: \"Skip to content\";",
        "skip_link: \"Skip\";",
    );
    let (_, out, _) = run(&["diff", p(&base), p(&cand), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["page"][0]["setting"], "skip_link");
    assert_eq!(v["change_count"], 1);
}

#[test]
fn two_pages_across_an_import_are_refused() {
    let c = corpus();
    let dir = scratch_dir();
    write_files(&c, &dir);
    std::fs::write(
        dir.join("lib.cui"),
        "module lib;\npage { title: \"L\"; lang: \"en\"; }\ncomponent L {\n    view { Text { \"l\" } }\n}\n",
    )
    .expect("lib");
    std::fs::write(
        dir.join("main.cui"),
        "import lib.L;\npage { title: \"M\"; lang: \"en\"; }\ncomponent M {\n    view { L { } }\n}\n",
    )
    .expect("main");
    let (code, found) = codes(&dir.join("main.cui"));
    assert_eq!((code, found), (1, vec!["page-duplicate".to_string()]));
}
