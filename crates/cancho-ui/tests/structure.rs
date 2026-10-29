//! The structure elements (docs/structure.md): every refusal of `tests/structure_corpus.json` is refused by its rule tag, the valid views
//! check, the DOM they become is checked in Chromium (`scripts/check-structure.js`) against the same corpus, authority is unchanged except
//! the hosts a `Link` names, and the nesting bound of 32 applies to the new elements.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/structure_corpus.json");

fn run(args: &[&str]) -> (i32, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(name: &str, text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        common::scratch_root().join(format!("cancho-ui-structure-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    write_assets(&dir);
    path
}

/// The first bytes of a file of each format the corpus names: enough for the compiler to see the format, and no more.
fn magic(path: &str) -> Vec<u8> {
    let ext = path.rsplit('.').next().expect("an extension");
    let mut b: Vec<u8> = match ext {
        "png" => vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        "jpg" => vec![0xff, 0xd8, 0xff, 0xe0],
        "gif" => b"GIF89a".to_vec(),
        "webp" => b"RIFF\0\0\0\0WEBP".to_vec(),
        "avif" => b"\0\0\0\x1cftypavif".to_vec(),
        "mp4" => b"\0\0\0\x1cftypisom".to_vec(),
        "webm" => vec![0x1a, 0x45, 0xdf, 0xa3],
        // a file a link names (docs/www-port/gaps.md section 6): text
        "md" | "txt" | "json" | "cast" | "csv" => return b"# a file of the site\n".to_vec(),
        "pdf" => b"%PDF-1.4\n".to_vec(),
        other => panic!("no magic for {other}"),
    };
    b.extend_from_slice(&[0; 16]);
    b
}

/// The files the corpus names, in a directory: real formats by their first bytes, one over the size bound (a sparse file), one that is not
/// what its extension says.
fn write_assets(dir: &Path) {
    let c = corpus();
    for kind in ["ok", "big", "fake", "binary"] {
        for f in c["assets"][kind].as_array().expect("assets") {
            let rel = f.as_str().expect("a path");
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().expect("a directory")).expect("a directory");
            let bytes = if kind == "fake" {
                b"<html>not a png</html>".to_vec()
            } else if kind == "binary" {
                // bytes, with a name that says text: a NUL is not text
                vec![0x50, 0x00, 0x01, 0x02]
            } else {
                magic(rel)
            };
            std::fs::write(&path, bytes).expect("an asset");
            if kind == "big" {
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .expect("open");
                file.set_len(20 * 1024 * 1024 + 1).expect("a sparse file");
            }
        }
    }
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn program(c: &Value, view: &str) -> String {
    program_with(c, view, None)
}

fn program_with(c: &Value, view: &str, requires: Option<&str>) -> String {
    let requires = requires.map_or(String::new(), |r| format!("requires {r};\n    "));
    format!(
        "component C {{\n    {requires}{}\n    view {{ Column {{ {view} }} }}\n}}\n",
        c["state"].as_str().expect("state")
    )
}

fn codes(text: &str) -> (i32, Vec<String>) {
    let file = scratch("p.cui", text);
    let (code, out, _) = run(&["check", p(&file), "--output", "json"]);
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
fn every_valid_view_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let text = program_with(
            &c,
            case["view"].as_str().expect("view"),
            case["requires"].as_str(),
        );
        let (code, got) = codes(&text);
        assert_eq!((code, got), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    let mut tags = std::collections::BTreeSet::new();
    for case in c["refusals"].as_array().expect("refusals") {
        let text = match case["program"].as_str() {
            Some(whole) => whole.to_string(),
            None => program_with(
                &c,
                case["view"].as_str().expect("view"),
                case["requires"].as_str(),
            ),
        };
        let want = case["code"].as_str().expect("code");
        let (code, got) = codes(&text);
        assert_eq!(code, 1, "{}", case["name"]);
        assert_eq!(got, vec![want.to_string()], "{}", case["name"]);
        tags.insert(want.to_string());
    }
    // every tag the structure module owns is reached
    for tag in [
        "heading-level-invalid",
        "heading-level-skipped",
        "heading-empty",
        "id-invalid",
        "anchor-duplicate",
        "anchor-unknown",
        "link-scheme-refused",
        "link-to-invalid",
        "link-label-missing",
        "link-label-generic",
        "link-appearance-invalid",
        "text-style-invalid",
        "details-incomplete",
        "caption-misplaced",
        "landmark-duplicate",
        "nav-label-missing",
        "binding-unknown",
        "rich-child-invalid",
        "rich-empty",
        "inline-outside-rich",
        "code-copy-invalid",
        "code-max-height-invalid",
        "code-child-invalid",
        "clipboard-not-declared",
        "capability-unused",
        "image-src-invalid",
        "image-alt-missing",
        "image-alt-conflict",
        "image-size-missing",
        "image-size-invalid",
        "image-child-invalid",
        "image-priority-invalid",
        "ratio-invalid",
        "media-kind-invalid",
        "media-src-invalid",
        "media-poster-invalid",
        "media-label-missing",
        "media-text-alternative-missing",
        "media-reference-invalid",
        "media-child-invalid",
        "grid-min-invalid",
        "grid-weights-invalid",
        "grid-align-invalid",
        "card-invalid",
        "overlay-invalid",
        "grid-from-invalid",
        "layout-value-invalid",
        "asset-missing",
        "asset-too-large",
        "asset-type-mismatch",
        "list-child-invalid",
        "item-outside-list",
        "list-empty",
        "item-empty",
        "list-ordered-invalid",
        "link-file-invalid",
        "condition-not-bool",
        "name-unknown",
    ] {
        assert!(tags.contains(tag), "no case reaches {tag}");
    }
}

#[test]
fn an_element_the_old_compiler_took_as_a_component_name_is_now_the_primitive() {
    // `Section`, `Link` and the others were names a program could give its own components (an instance of an undeclared name is refused);
    // a program that declares `component Link` and uses it is a program that now has a clash, and is told by the shape check, not run
    let text = "component Link {\n    view { Text { \"x\" } }\n}\n\ncomponent App {\n    view { Link { to: \"https://a.example/\" \"Go on\" } }\n}\n";
    let (code, got) = codes(text);
    assert_eq!(
        code, 0,
        "{got:?}: the built-in Link is what `Link {{ .. }}` means"
    );
}

#[test]
fn a_link_lists_its_host_in_inspect_and_diff_and_authority_stays() {
    let a = "component C {\n    view { Link { to: \"https://github.com/alpibrusl/cancho\" \"The compiler\" } }\n}\n";
    let b = "component C {\n    view { Column { Link { to: \"https://github.com/alpibrusl/cancho\" \"The compiler\" } Link { to: \"https://Evil.Example:8443/x\" \"The other\" } Link { to: \"mailto:Hi@Example.org\" \"Write\" } } }\n}\n";
    let (fa, fb) = (scratch("a.cui", a), scratch("b.cui", b));
    let (code, out, _) = run(&["inspect", "effects", p(&fb), "C"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(
        v["links"],
        serde_json::json!(["evil.example", "github.com", "mailto:example.org"])
    );
    assert_eq!(v["requires"], serde_json::json!([]));
    let (_, out, _) = run(&["inspect", "effects", p(&fa), "C"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["links"], serde_json::json!(["github.com"]));
    // diff: the new hosts are lines of their own, and the authority is not expanded
    let (code, human, _) = run(&["diff", p(&fa), p(&fb)]);
    assert_eq!(code, 5, "{human}");
    assert!(
        human.contains("+ link evil.example") && human.contains("+ link mailto:example.org"),
        "{human}"
    );
    assert!(!human.contains("+ link github.com"), "{human}");
    assert!(human.contains("authority: unchanged"), "{human}");
    let (_, js, _) = run(&["diff", p(&fa), p(&fb), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged");
    assert_eq!(
        v["components"]["changed"][0]["links"]["added"],
        serde_json::json!(["evil.example", "mailto:example.org"])
    );
    // a change of host and nothing else: two link lines (one host out, one in) and two view lines (the descriptor of a Link names its `to`),
    // so `change_count` is 4, and it is 2 if the link lines are not counted
    let one = "component C {\n    view { Link { to: \"https://a.example/\" \"x\" } }\n}\n";
    let two = "component C {\n    view { Link { to: \"https://b.example/\" \"x\" } }\n}\n";
    let (f1, f2) = (scratch("one.cui", one), scratch("two.cui", two));
    let (code, js, _) = run(&["diff", p(&f1), p(&f2), "--output", "json"]);
    assert_eq!(code, 5, "a changed host is a change: {js}");
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["change_count"], 4, "{js}");
    assert_eq!(v["authority"], "unchanged");
    // the same links twice: no `links` key at all
    let (_, js, _) = run(&["diff", p(&fa), p(&fa), "--output", "json"]);
    assert!(!js.contains("\"links\""), "{js}");
}

fn nested(depth: usize, innermost: &str) -> String {
    let mut open = String::new();
    let mut close = String::new();
    for _ in 0..depth {
        open.push_str("Section { ");
        close.push_str(" }");
    }
    format!("component C {{\n    view {{ {open}{innermost}{close} }}\n}}\n")
}

#[test]
fn the_nesting_bound_of_32_applies_to_the_new_elements() {
    // one bound over view nodes, expressions and types together (docs/nesting.md): the text of a heading is an expression of its own, so
    // `Heading` with a text is 2 deep and `Details { Text { .. } }` is 3. With the budget of 32, the deepest program that checks has
    // `32 - d` sections around an innermost of depth `d`, and one more section is `parse-nesting-too-deep`.
    for (inner, d) in [
        ("Heading { level: 1 \"deep\" }", 2usize),
        ("Link { to: \"https://a.example/\" \"Go on\" }", 2),
        ("Details { summary: \"s\" Text { \"x\" } }", 3),
        ("Figure { Caption { \"c\" } }", 3),
        ("Article { Text { \"x\" } }", 3),
        ("Rich { \"a \" Strong { Em { \"x\" } } }", 4),
        ("Code { \"x\" }", 2),
        ("Grid { min: 12 Text { \"x\" } }", 3),
    ] {
        assert_eq!(
            codes(&nested(32 - d, inner)),
            (0, vec![]),
            "{inner} at the bound"
        );
        let (code, got) = codes(&nested(33 - d, inner));
        assert_eq!(code, 1, "{inner} one deeper");
        assert_eq!(
            got,
            vec!["parse-nesting-too-deep".to_string()],
            "{inner} one deeper"
        );
    }
}

#[test]
fn a_copy_button_is_the_clipboard_capability_and_only_a_write() {
    let plain = "component C {\n    view { Code { \"x\" } }\n}\n";
    let copy =
        "component C {\n    requires Clipboard;\n    view { Code { copy: true \"x\" } }\n}\n";
    let (fp, fc) = (scratch("plain.cui", plain), scratch("copy.cui", copy));
    // authority: the capability is declared, shown by `inspect effects`, and added to a program by `diff` as an expansion
    let (_, out, _) = run(&["inspect", "effects", p(&fc), "C"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["requires"][0]["name"], "Clipboard");
    let (code, human, _) = run(&["diff", p(&fp), p(&fc)]);
    assert_eq!(code, 5, "{human}");
    assert!(
        human.contains("+ requires Clipboard") && human.contains("authority: expanded"),
        "{human}"
    );
    let (_, js, _) = run(&["diff", p(&fp), p(&fc), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "expanded");
    // the emitted module: no clipboard at all without a copy button; with one, `writeText` and never `readText`, and the write is in the adapter the
    // button's click calls
    let (_, plain_js, _) = run(&["build", p(&fp)]);
    assert!(
        !plain_js.contains("clipboard") && !plain_js.contains("__cui_copy"),
        "no copy button, no clipboard code"
    );
    let (_, copy_js, _) = run(&["build", p(&fc)]);
    assert!(
        copy_js.contains("navigator.clipboard.writeText(text)") && !copy_js.contains("readText"),
        "{copy_js}"
    );
    assert_eq!(copy_js.matches("writeText(").count(), 1);
    assert!(
        copy_js.contains("addEventListener('click', function () { __cui_copy("),
        "the write is reached from a click only"
    );
}

#[test]
fn garbage_is_a_refusal_and_not_a_panic() {
    let c = corpus();
    for view in [
        "Heading {",
        "Heading { level: }",
        "Heading { level: 2 \"unterminated }",
        "Link { to: \"https://a.example/\" \"x\" Link { to: \"#\" \"y\" } }",
        "Details { summary: \"s\" summary: \"t\" Text { \"x\" } }",
        "Section { id: \"a\" id: \"b\" Text { \"x\" } }",
        "Caption { Caption { \"x\" } }",
        "Figure { Figure { Caption { \"a\" } } }",
        "Nav { Nav { label: \"a\" Text { \"x\" } } Nav { label: \"b\" Text { \"y\" } } }",
        "Link { to: \"\\u{0}\" \"x\" }",
        "Text { style: \"eyebrow\" style: \"note\" \"x\" }",
        "Rich { \"unterminated }",
        "Rich { Strong { Strong { Strong { } } } }",
        "Code { copy: copy \"x\" }",
        "Code { max_height: 99999999999999999999 \"x\" }",
        "Details { Summary { Summary { \"x\" } } }",
        "Heading { level: 9223372036854775807 \"x\" }",
        "Heading { level: -1 \"x\" }",
        "Image {",
        "Image { src: }",
        "Image { src: \"assets/a.webp\" alt: \"unterminated }",
        "Image { src: \"\\u{0}.webp\" alt: \"x\" width: 1 height: 1 }",
        "Image { src: \"assets/a.webp\" alt: \"x\" width: 99999999999999999999 height: 1 }",
        "Image { src: \"assets/a.webp\" alt: \"x\" width: -1 height: 1 }",
        "Image { src: \"assets/a.webp\" src: \"assets/a.jpg\" alt: \"x\" width: 1 height: 1 }",
        "Media { src: [ }",
        "Media { src: [\"a.mp4\", ] }",
        "Media { src: [1, 2] kind: \"video\" }",
        "Media { src: [[\"assets/v.mp4\"]] kind: \"video\" }",
        "Media { caption: \"cap\" caption: \"cap\" }",
        "Grid { weights: [ }",
        "Grid { min: 18 weights: [1, 2,] Text { \"a\" } Text { \"b\" } }",
        "Grid { min: 18 weights: [] Text { \"a\" } }",
        "Grid { min: 18 weights: [1e999, 1] Text { \"a\" } Text { \"b\" } }",
        "Grid { min: 18 weights: [-1, 1] Text { \"a\" } Text { \"b\" } }",
        "Grid { min: 99999999999999999999 Text { \"a\" } }",
        "Grid { min: 18 min: 19 Text { \"a\" } }",
        "Grid { min: 18 Grid { min: 18 Grid { min: 18 Text { \"a\" } } } }",
    ] {
        let (code, got) = codes(&program(&c, view));
        assert!(code == 1 || code == 0, "{view}: exit {code}");
        if code == 1 {
            assert!(!got.is_empty(), "{view}");
        }
    }
}

fn site(name: &str, text: &str) -> PathBuf {
    scratch(name, text)
}

const PICTURES: &str = "component C {\n    view { Column { Image { src: \"assets/a.webp\" fallback: \"assets/a.jpg\" alt: \"A field\" width: 1024 height: 768 } Figure { Media { label: \"Demo\" kind: \"video\" src: [\"assets/v.mp4\", \"assets/v.webm\"] poster: \"assets/p.jpg\" caption: \"cap\" } Caption { id: \"cap\" \"One minute.\" } } Grid { min: 17 Text { \"a\" } Text { \"b\" } } } }\n}\n";

#[test]
fn pictures_video_and_grids_ask_for_no_authority() {
    // a program with the same text and no pictures, and the one with them: the same `requires`, the same effects, and `diff` says unchanged
    let none = "component C {\n    view { Column { Text { \"a\" } } }\n}\n";
    let (fa, fb) = (site("a.cui", none), site("b.cui", PICTURES));
    let (code, out, _) = run(&["inspect", "effects", p(&fb), "C"]);
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(v["requires"], serde_json::json!([]));
    assert_eq!(v["reachable_effects"], serde_json::json!([]));
    let (_, js, _) = run(&["diff", p(&fa), p(&fb), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged", "{js}");
    // and the built module asks the network for nothing: no `fetch`, no address of another host
    let (code, built, _) = run(&["build", p(&fb)]);
    assert_eq!(code, 0);
    assert!(
        !built.contains("fetch(") && !built.contains("http://") && !built.contains("https://"),
        "no request in the module"
    );
}

#[test]
fn a_changed_file_is_a_change_of_the_program_and_the_same_bytes_are_not() {
    let a = site("a.cui", PICTURES);
    let dir = a.parent().expect("a directory").to_path_buf();
    // the same program in a second directory with the same files: nothing differs
    let b = site("b.cui", PICTURES);
    let (code, out, _) = run(&["diff", p(&a), p(&b)]);
    assert_eq!(code, 0, "{out}");
    // a file with other bytes (same format, same size): one changed file, named, with the first twelve digits of each SHA-256
    let other = b.parent().expect("a directory").join("assets/a.webp");
    let mut bytes = std::fs::read(&other).expect("read");
    let last = bytes.len() - 1;
    bytes[last] = 1;
    std::fs::write(&other, bytes).expect("write");
    let (code, human, _) = run(&["diff", p(&a), p(&b)]);
    assert_eq!(code, 5, "{human}");
    assert!(human.contains("~ file assets/a.webp: "), "{human}");
    let (_, js, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    assert_eq!(v["authority"], "unchanged");
    assert_eq!(v["change_count"], 1, "{js}");
    let changed = &v["assets"][0];
    assert_eq!(changed["path"], "assets/a.webp");
    assert_eq!(changed["base"].as_str().map(str::len), Some(64));
    assert_ne!(changed["base"], changed["candidate"]);
    // a file removed from the program is a removed file
    let none = scratch_in(
        &dir,
        "none.cui",
        "component C {\n    view { Text { \"a\" } }\n}\n",
    );
    let (_, js, _) = run(&["diff", p(&a), p(&none), "--output", "json"]);
    let v: Value = serde_json::from_str(&js).expect("JSON");
    let removed: Vec<&str> = v["assets"]
        .as_array()
        .expect("assets")
        .iter()
        .filter(|c| c["candidate"].is_null())
        .map(|c| c["path"].as_str().expect("path"))
        .collect();
    assert_eq!(
        removed,
        vec![
            "assets/a.jpg",
            "assets/a.webp",
            "assets/p.jpg",
            "assets/v.mp4",
            "assets/v.webm"
        ],
        "{js}"
    );
    // two programs with no files have no `assets` key at all
    let (_, js, _) = run(&["diff", p(&none), p(&none), "--output", "json"]);
    assert!(!js.contains("\"assets\""), "{js}");
}

fn scratch_in(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a file");
    path
}

#[test]
fn a_build_copies_the_files_and_the_offline_cache_keeps_pictures_and_not_video() {
    let a = site("a.cui", PICTURES);
    let out = a.parent().expect("a directory").join("out");
    let (code, _, err) = run(&["build", p(&a), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    for f in [
        "assets/a.webp",
        "assets/a.jpg",
        "assets/p.jpg",
        "assets/v.mp4",
        "assets/v.webm",
    ] {
        assert!(out.join(f).is_file(), "{f} is copied into the build");
    }
    assert!(
        !out.join("assets/b.png").exists(),
        "a file no view names is not copied"
    );
    // --pwa: the worker precaches the pictures and the poster, not the videos, and the cache version changes when a picture changes
    let app = a.parent().expect("a directory").join("app");
    let (code, _, err) = run(&["build", p(&a), "--pwa", "-o", p(&app)]);
    assert_eq!(code, 0, "{err}");
    let sw = std::fs::read_to_string(app.join("sw.js")).expect("sw.js");
    for f in ["./assets/a.webp", "./assets/a.jpg", "./assets/p.jpg"] {
        assert!(sw.contains(&format!("\"{f}\"")), "{f} is precached");
    }
    for f in ["./assets/v.mp4", "./assets/v.webm"] {
        assert!(!sw.contains(f), "{f} is not precached");
    }
    assert!(app.join("assets/v.mp4").is_file(), "but it is in the build");
    let version = |s: &str| {
        s.lines()
            .find(|l| l.contains("const CACHE"))
            .map(str::to_string)
    };
    let before = version(&sw);
    let webp = a.parent().expect("a directory").join("assets/a.webp");
    let mut bytes = std::fs::read(&webp).expect("read");
    let last = bytes.len() - 1;
    bytes[last] = 7;
    std::fs::write(&webp, bytes).expect("write");
    let (code, _, err) = run(&["build", p(&a), "--pwa", "-o", p(&app)]);
    assert_eq!(code, 0, "{err}");
    let sw2 = std::fs::read_to_string(app.join("sw.js")).expect("sw.js");
    assert_ne!(
        before,
        version(&sw2),
        "the cache name moves with the picture"
    );
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_directory_is_not_a_file_of_the_site() {
    let a = site(
        "a.cui",
        "component C {\n    view { Image { src: \"assets/out.png\" alt: \"x\" width: 1 height: 1 } }\n}\n",
    );
    let dir = a.parent().expect("a directory").to_path_buf();
    // a real PNG elsewhere, linked into the directory of the program
    let elsewhere =
        common::scratch_root().join(format!("cancho-ui-elsewhere-{}.png", std::process::id()));
    std::fs::write(&elsewhere, magic("x.png")).expect("a file");
    std::os::unix::fs::symlink(&elsewhere, dir.join("assets/out.png")).expect("a link");
    let (code, got) = codes_of(&a);
    assert_eq!((code, got), (1, vec!["asset-missing".to_string()]));
    // the same file copied in is fine
    std::fs::remove_file(dir.join("assets/out.png")).expect("remove");
    std::fs::copy(&elsewhere, dir.join("assets/out.png")).expect("copy");
    assert_eq!(codes_of(&a), (0, vec![]));
    std::fs::remove_file(&elsewhere).expect("remove");
}

fn codes_of(file: &Path) -> (i32, Vec<String>) {
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
fn an_image_inside_a_directory_of_files_with_odd_names_is_still_a_refusal_and_not_a_panic() {
    // a name the compiler does not take is refused before any file is looked at; a directory named like the file is not a file
    let a = site(
        "a.cui",
        "component C {\n    view { Image { src: \"assets/dir.png\" alt: \"x\" width: 1 height: 1 } }\n}\n",
    );
    let dir = a.parent().expect("a directory").join("assets/dir.png");
    std::fs::create_dir_all(&dir).expect("a directory named like a file");
    assert_eq!(codes_of(&a), (1, vec!["asset-missing".to_string()]));
    // an empty file is not a png
    std::fs::remove_dir_all(&dir).expect("remove");
    std::fs::write(&dir, b"").expect("an empty file");
    assert_eq!(codes_of(&a), (1, vec!["asset-type-mismatch".to_string()]));
}

#[test]
fn a_refusal_of_a_picture_says_what_to_change() {
    // the message is what a person reads: an address of another host is told that a page makes no request to one, and a missing file is named
    let remote = site(
        "r.cui",
        "component C {\n    view { Image { src: \"https://example.org/a.webp\" alt: \"x\" width: 1 height: 1 } }\n}\n",
    );
    let (_, out, _) = run(&["check", p(&remote), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    let message = v[0]["message"].as_str().expect("message");
    assert!(
        message.contains("makes no request to another host"),
        "{message}"
    );
    assert!(
        v[0]["remediation"]["description"]
            .as_str()
            .is_some_and(|d| d.contains("structure.md")),
        "{out}"
    );
    let missing = site(
        "m.cui",
        "component C {\n    view { Image { src: \"assets/nope.webp\" alt: \"x\" width: 1 height: 1 } }\n}\n",
    );
    let (_, out, _) = run(&["check", p(&missing), "--output", "json"]);
    let v: Value = serde_json::from_str(&out).expect("JSON");
    let message = v[0]["message"].as_str().expect("message");
    assert!(message.contains("assets/nope.webp"), "{message}");
    // and a refusal of an asset points at the line of the element that asked for it
    assert!(
        v[0]["span"]["start"]["line"]
            .as_u64()
            .is_some_and(|l| l == 2),
        "{out}"
    );
}
