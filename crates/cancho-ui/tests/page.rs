//! `page { .. }`, the skip link and the sticky header (docs/page.md): every refusal of `tests/page_corpus.json` is refused by its rule tag, the valid
//! programs check, the head the compiler writes is the head cancho-www writes by hand (a fixture of that head, compared line for line), the skip link is
//! the first thing in the page, and `inspect ir`, `diff` and the replay each say the page. What the page does in a browser is checked by
//! `scripts/check-page.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/page_corpus.json");
const README: &str = include_str!("../../../spec/diagnostics/README.md");
const REAL_HEAD: &str = include_str!("../../../tests/fixtures/page/cancho-www-head.html");

/// Every rule tag of the page module: a case of the corpus reaches each.
const TAGS: &[&str] = &[
    "page-duplicate",
    "page-entry-unknown",
    "page-entry-duplicate",
    "page-entry-type",
    "page-title-missing",
    "page-description-length",
    "page-lang-invalid",
    "page-canonical-not-https",
    "page-image-not-https",
    "page-image-size-missing",
    "page-image-size-invalid",
    "page-image-alt-missing",
    "page-link-invalid",
    "page-alternate-invalid",
    "page-skip-text-missing",
    "page-skip-text-invalid",
    "page-main-missing",
    "skip-target-missing",
    "page-file-missing",
    "page-file-too-large",
    "header-sticky-invalid",
    "header-sticky-duplicate",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
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

fn scratch_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-page-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A directory with the corpus files and a program in it.
fn workspace(c: &Value, program: &str) -> PathBuf {
    let dir = scratch_dir();
    for (name, v) in c["files"].as_object().expect("files") {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("a directory")).expect("a directory");
        let bytes = match v {
            Value::String(s) => s.as_bytes().to_vec(),
            Value::Object(o) => o["fill"]
                .as_str()
                .expect("fill")
                .repeat(o["bytes"].as_u64().expect("bytes") as usize)
                .into_bytes(),
            other => panic!("a corpus file is {other}"),
        };
        std::fs::write(path, bytes).expect("a corpus file");
    }
    std::fs::write(dir.join("p.cui"), program).expect("the program");
    dir.join("p.cui")
}

fn source_of(c: &Value, case: &Value) -> String {
    if let Some(whole) = case["program"].as_str() {
        return whole.to_string();
    }
    let view = case["view"]
        .as_str()
        .unwrap_or(c["view"].as_str().expect("view"));
    format!(
        "{}component Site {{\n    view {{ Column {{ {view} }} }}\n}}\n",
        case["page"].as_str().expect("page")
    )
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

#[test]
fn every_valid_program_of_the_corpus_checks_and_builds() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let file = workspace(&c, &source_of(&c, case));
        assert_eq!(codes(&file), (0, vec![]), "{}", case["name"]);
        let out = file.parent().expect("dir").join("out");
        let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&out)]);
        assert_eq!(code, 0, "{}: {err}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    let mut reached = std::collections::BTreeSet::new();
    for case in c["refusals"].as_array().expect("refusals") {
        let file = workspace(&c, &source_of(&c, case));
        let want = case["code"].as_str().expect("code");
        let (code, got) = codes(&file);
        assert_eq!(code, 1, "{}", case["name"]);
        assert_eq!(got, vec![want.to_string()], "{}", case["name"]);
        reached.insert(want.to_string());
    }
    for tag in TAGS {
        assert!(reached.contains(*tag), "no case reaches {tag}");
    }
}

#[test]
fn every_tag_is_documented() {
    for tag in TAGS {
        assert!(
            README.contains(&format!("`{tag}`")),
            "spec/diagnostics/README.md does not document `{tag}`"
        );
    }
}

#[test]
fn a_page_is_one_per_file_and_one_per_program() {
    let c = corpus();
    let file = workspace(&c, "");
    let dir = file.parent().expect("dir").to_path_buf();
    let page = "page {\n    title: \"A page\";\n    description: \"A description that is long enough to be one: more than fifty characters.\";\n    canonical: \"https://example.org/\";\n}\n";
    std::fs::write(
        dir.join("lib.cui"),
        format!(
            "module lib;\n\n{page}\ncomponent Inner {{\n    view {{ Text {{ \"inner\" }} }}\n}}\n"
        ),
    )
    .expect("a module");
    std::fs::write(
        &file,
        format!("import lib.Inner;\n\n{page}\ncomponent Site {{\n    view {{ Main {{ id: \"main\" Inner {{ }} }} }}\n}}\n"),
    )
    .expect("the entry");
    assert_eq!(codes(&file), (1, vec!["page-duplicate".to_string()]));
    // a page in the imported module alone is the program's page, and a Main in the entry is the one it skips to
    std::fs::write(
        &file,
        "import lib.Inner;\n\ncomponent Site {\n    view { Main { id: \"main\" Inner { } } }\n}\n",
    )
    .expect("the entry");
    assert_eq!(codes(&file), (0, vec![]));
}

/// The `<title>`, `<meta>` and `<link>` lines of a built `index.html` from the title to the style: the head the page declares.
fn head_of(html: &str) -> String {
    let a = html.find("<title>").expect("a title");
    let b = html.find("<style>").expect("a style");
    html[a..b].to_string()
}

#[test]
fn the_head_is_the_one_cancho_www_writes_by_hand() {
    let out = scratch_dir().join("out");
    let (code, _, err) = run(&["build", "examples/page/page.cui", "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert_eq!(head_of(&html), REAL_HEAD);
    assert!(html.contains("<html lang=\"en\">"));
    // the files the page names are copied beside it, at the paths it uses
    for f in [
        "logo-mark.png",
        "logo.png",
        "agents.md",
        "data/ecosystem.json",
    ] {
        assert!(out.join(f).is_file(), "{f} is copied into the build");
    }
}

#[test]
fn a_page_with_no_image_is_a_summary_card() {
    let c = corpus();
    let case = c["valid"]
        .as_array()
        .expect("valid")
        .iter()
        .find(|v| {
            v["name"]
                .as_str()
                .is_some_and(|n| n.contains("only what is required"))
        })
        .expect("the case");
    let file = workspace(&c, &source_of(&c, case));
    let out = file.parent().expect("dir").join("out");
    let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    let head = head_of(&html);
    assert!(
        head.contains("<meta name=\"twitter:card\" content=\"summary\">"),
        "{head}"
    );
    assert!(
        !head.contains("og:image")
            && !head.contains("rel=\"icon\"")
            && !head.contains("rel=\"alternate\""),
        "{head}"
    );
    // og:description is the description when there is no other
    assert!(
        head.contains("<meta property=\"og:description\" content=\"cancho is a programming")
            || head.contains("og:description\" content=\"d"),
        "{head}"
    );
}

#[test]
fn the_head_of_a_program_with_no_page_is_what_it_was() {
    let c = corpus();
    let file = workspace(&c, "component Site {\n    view { Text { \"x\" } }\n}\n");
    let out = file.parent().expect("dir").join("out");
    let (code, _, _) = run(&[
        "build",
        p(&file),
        "--page",
        "-o",
        p(&out),
        "--name",
        "plain",
    ]);
    assert_eq!(code, 0);
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(html.contains("<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">"));
    assert!(
        html.contains("<title>plain</title>\n<style>"),
        "no page, no head lines"
    );
    assert!(!html.contains("og:title") && !html.contains("rel=\"canonical\""));
    assert!(!html[html.find("<div id=\"root\"").expect("root")..].contains("cui-link-skip"));
    // and the stylesheet it always had: a program with neither a page nor a sticky header gets none of the page's rules
    assert!(
        !html.contains("cui-link-skip") && !html.contains("cui-sticky"),
        "the page's rules are for a page"
    );
    // a sticky header with no page gets the sticky rules and not the skip link's
    let sticky = workspace(
        &c,
        "component Site {\n    view { Column { Header { sticky: true Link { to: \"#a\" \"a\" } } Section { label: \"A\" id: \"a\" Text { \"x\" } } } }\n}\n",
    );
    let out = sticky.parent().expect("dir").join("out");
    let (code, _, err) = run(&["build", p(&sticky), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(
        html.contains("@media (scripting: enabled)")
            && html.contains("<header class=\"cui-sticky\">")
    );
    assert!(
        !html.contains("og:title")
            && !html[html.find("<div id=\"root\"").expect("root")..].contains("cui-link-skip")
    );
}

#[test]
fn text_of_the_program_cannot_end_an_attribute_or_become_a_token() {
    let c = corpus();
    let case = c["valid"]
        .as_array()
        .expect("valid")
        .iter()
        .find(|v| v["name"].as_str().is_some_and(|n| n.contains("an at sign")))
        .expect("the case");
    let file = workspace(&c, &source_of(&c, case));
    let out = file.parent().expect("dir").join("out");
    let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(
        html.contains("<title>Tom &amp; &quot;Jerry&quot; &lt;b&gt; &#64;TITLE&#64;</title>"),
        "{}",
        head_of(&html)
    );
    assert!(html.contains("<meta property=\"og:title\" content=\"Tom &amp; &quot;Jerry&quot; &lt;b&gt; &#64;TITLE&#64;\">"));
    // the title appears once as a title: the token inside it was not replaced by the title again
    assert_eq!(
        html.matches("Jerry").count(),
        2,
        "the title, and og:title: no more"
    );
}

#[test]
fn the_skip_link_is_the_first_thing_in_the_page_and_goes_to_the_main() {
    let c = corpus();
    let file = workspace(
        &c,
        &format!(
            "{}component Site {{\n    view {{ Column {{ {} }} }}\n}}\n",
            c["valid"][0]["page"].as_str().expect("page"),
            c["view"]
                .as_str()
                .expect("view")
                .replace("id: \"main\"", "id: \"content\"")
        ),
    );
    let out = file.parent().expect("dir").join("out");
    let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    let root = &html[html.find("<div id=\"root\"").expect("the root")..];
    let first_link = root.find("<a ").expect("a link");
    assert!(
        root[first_link..]
            .starts_with("<a href=\"#content\" class=\"cui-link-skip\">Skip to content</a>"),
        "{}",
        &root[first_link..first_link + 120]
    );
    // before the header, before the first heading, before anything a visitor could tab to
    assert!(first_link < root.find("<header").expect("the header"));
}

#[test]
fn the_skip_link_is_in_the_page_language() {
    let c = corpus();
    let case = c["valid"]
        .as_array()
        .expect("valid")
        .iter()
        .find(|v| v["name"].as_str().is_some_and(|n| n.contains("Spanish")))
        .expect("the case");
    let file = workspace(&c, &source_of(&c, case));
    let out = file.parent().expect("dir").join("out");
    let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(html.contains("<html lang=\"es\">"));
    assert!(html.contains(">Saltar al contenido</a>") && !html.contains("Skip to content"));
}

#[test]
fn a_sticky_header_has_its_class_the_baseline_and_the_runtime() {
    let c = corpus();
    let file = workspace(&c, &source_of(&c, &c["valid"][0]));
    let out = file.parent().expect("dir").join("out");
    let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    assert!(html.contains("<header class=\"cui-sticky\">"));
    assert!(html.contains(
        "@media (scripting: enabled) {\n    header.cui-sticky { position: sticky; top: 0;"
    ));
    assert!(html.contains("html:has(.cui-sticky) { scroll-padding-top: 5rem; }"));
    assert!(js.contains("function __cui_sticky_sync()") && js.contains("__cui_sticky_sync();"));
    // a header that says `sticky: false` is not sticky either
    let not = c["valid"]
        .as_array()
        .expect("valid")
        .iter()
        .find(|v| v["name"].as_str().is_some_and(|n| n.contains("says false")))
        .expect("the case");
    let file = workspace(&c, &source_of(&c, not));
    let out = file.parent().expect("dir").join("out");
    let (code, _, _) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0);
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    assert!(
        !html.contains("<header class=\"cui-sticky\">"),
        "sticky: false is a header like any other"
    );
    // a header that is not sticky gets none of it
    let plain = c["valid"]
        .as_array()
        .expect("valid")
        .iter()
        .find(|v| {
            v["name"]
                .as_str()
                .is_some_and(|n| n.contains("no sticky header"))
        })
        .expect("the case");
    let file = workspace(&c, &source_of(&c, plain));
    let out = file.parent().expect("dir").join("out");
    let (code, _, _) = run(&["build", p(&file), "--page", "-o", p(&out)]);
    assert_eq!(code, 0);
    let html = std::fs::read_to_string(out.join("index.html")).expect("index.html");
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    assert!(!html.contains("<header class=\"cui-sticky\">") && !js.contains("__cui_sticky_sync"));
}

#[test]
fn inspect_ir_diff_and_authority_say_the_page() {
    let c = corpus();
    let a = workspace(&c, &source_of(&c, &c["valid"][0]));
    let (code, out, _) = run(&["inspect", "ir", p(&a)]);
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["page"]["title"], "cancho: tools built for agents");
    assert_eq!(v["page"]["image.width"], "1200");
    assert_eq!(v["page"]["alternate.agents.md.mime"], "text/markdown");
    // another page: the title, one description and one alternate removed
    let text = std::fs::read_to_string(&a).expect("source");
    let text2 = text
        .replace("cancho: tools built for agents", "cancho: tools for agents")
        .replace("    alternate: \"data/ecosystem.json\" mime: \"application/json\" title: \"The ecosystem as data\";\n", "");
    let b = workspace(&c, &text2);
    let (code, human, _) = run(&["diff", p(&a), p(&b)]);
    assert_eq!(code, 5, "{human}");
    assert!(
        human.contains("~ page title: cancho: tools built for agents -> cancho: tools for agents"),
        "{human}"
    );
    assert!(
        human.contains("- page alternate.data/ecosystem.json = data/ecosystem.json"),
        "{human}"
    );
    assert!(human.contains("authority: unchanged"), "{human}");
    let (_, js, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged");
    assert_eq!(v["page"].as_array().expect("page changes").len(), 4);
    // the same page twice: no `page` key at all
    let (code, js, _) = run(&["diff", p(&a), p(&a), "--output", "json"]);
    assert_eq!(code, 0);
    assert!(!js.contains("\"page\""), "{js}");
    // adding a page to a program with none: lines, and still no authority
    let none = workspace(
        &c,
        "component Site {\n    view { Column { Main { id: \"main\" Section { label: \"Top\" id: \"top\" Text { \"x\" } } } } }\n}\n",
    );
    let (code, human, _) = run(&["diff", p(&none), p(&a)]);
    assert_eq!(code, 5);
    assert!(
        human.contains("+ page title = cancho: tools built for agents")
            && human.contains("authority: unchanged"),
        "{human}"
    );
}

#[test]
fn a_program_with_a_page_is_replayable_and_the_skip_link_is_not_a_trace_event() {
    let (code, out, err) = run(&[
        "test",
        "examples/page/page.cui",
        "examples/page/page.trace.json",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let v: Value = serde_json::from_str(
        &run(&[
            "test",
            "examples/page/page.cui",
            "examples/page/page.trace.json",
            "--output",
            "json",
        ])
        .1,
    )
    .expect("JSON");
    assert_eq!(
        (v["result"].as_str(), v["steps_run"].as_u64()),
        (Some("pass"), Some(2)),
        "{v}"
    );
    assert_eq!(v["final_state"]["clicks"], 2);
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let c = corpus();
    let full = source_of(&c, &c["valid"][0]);
    let file = workspace(&c, "");
    let cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).step_by(5).collect();
    for cut in cuts {
        std::fs::write(&file, &full[..cut]).expect("a cut program");
        let (code, _, err) = run(&["check", p(&file)]);
        assert!(code == 0 || code == 1, "cut at {cut}: exit {code}\n{err}");
        assert!(!err.contains("panicked"), "cut at {cut}: {err}");
    }
}

#[test]
fn a_program_named_without_a_directory_finds_the_files_beside_it() {
    // `cancho-ui build page.cui` run in the program's directory: the parent of `page.cui` is "", which is the current directory. The files a program
    // names beside it were refused as missing (assets, islands and now pages) until the directory was resolved
    let c = corpus();
    let file = workspace(&c, &source_of(&c, &c["valid"][0]));
    let dir = file.parent().expect("dir").to_path_buf();
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(["build", "p.cui", "--page", "-o", "out"])
        .current_dir(&dir)
        .output()
        .expect("runs");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("out/logo-mark.png").is_file());
}
