//! `PhotoField` and `Photo` (docs/structure.md 12.0r): every refusal of `tests/photo_corpus.json` is refused by its rule tag, the valid views check, the texts a `Photo` draws are the ones
//! the compiler's check and the page's agree on, the script of each element is in the page only of a program that has it (every other page is the bytes it was), a `Photo` and a `face(..)`
//! share one decoder, replay refuses a value the chooser could not give, and the lists of the docs name every tag. What a browser does is `scripts/check-photo.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/photo_corpus.json");
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

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

fn scratch(text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-photo-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let f = dir.join("p.cui");
    std::fs::write(&f, text).expect("the program");
    f
}

fn source(c: &Value, case: &Value) -> String {
    c["component"]
        .as_str()
        .expect("component")
        .replace("{view}", case["view"].as_str().expect("view"))
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

fn page(text: &str) -> String {
    let f = scratch(text);
    let out = f.parent().expect("dir").join("out");
    let (code, o, e) = run(&["build", p(&f), "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{o}{e}");
    std::fs::read_to_string(out.join("index.html")).expect("index.html")
}

fn script(text: &str) -> String {
    let f = scratch(text);
    let (code, js, e) = run(&["build", p(&f)]);
    assert_eq!(code, 0, "{e}");
    js
}

/// The text a corpus case describes: head, then fill repeated, then tail, `len` characters in all.
fn picture(case: &Value) -> String {
    let head = case["head"].as_str().expect("head");
    let fill = case["fill"].as_str().expect("fill");
    let tail = case["tail"].as_str().expect("tail");
    let len = case["len"].as_u64().expect("len") as usize;
    let mut s = String::from(head);
    let body = len.saturating_sub(head.chars().count() + tail.chars().count());
    s.extend(fill.chars().cycle().take(body));
    s.push_str(tail);
    s
}

const NEW: &[&str] = &[
    "photo-field-incomplete",
    "photo-field-in-loop",
    "photo-has-children",
    "photo-limit-invalid",
    "photo-square-invalid",
    "photo-incomplete",
    "photo-label-missing",
    "photo-label-conflict",
    "photo-data-invalid",
    "photo-tone-invalid",
    "photo-ratio-invalid",
    "photo-decorative-invalid",
    "photo-fit-invalid",
    "trace-value-not-a-photo",
];
const OLD: &[&str] = &["binding-unknown", "icon-unknown", "event-arity-mismatch"];

#[test]
fn the_texts_of_the_corpus_are_drawn_or_not_as_the_compiler_says() {
    let c = corpus();
    let pictures = c["pictures"].as_array().expect("pictures");
    assert!(pictures.len() >= 20, "the corpus shrank");
    for case in pictures {
        assert_eq!(
            cancho_ui_ir::photo::picture_ok(&picture(case)),
            case["ok"].as_bool().expect("ok"),
            "{}",
            case["why"]
        );
    }
}

#[test]
fn every_valid_view_of_the_corpus_checks() {
    let c = corpus();
    for case in c["valid"].as_array().expect("valid") {
        let f = scratch(&source(&c, case));
        assert_eq!(codes(&f), (0, vec![]), "{}", case["name"]);
    }
}

#[test]
fn every_refusal_of_the_corpus_is_refused_by_its_tag() {
    let c = corpus();
    for case in c["refusals"].as_array().expect("refusals") {
        let f = scratch(&source(&c, case));
        let (code, found) = codes(&f);
        let tag = case["tag"].as_str().expect("tag");
        assert_eq!(code, 1, "{}: {found:?}", case["name"]);
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
    for tag in NEW.iter().chain(OLD) {
        // the replay tag is reached by the trace tests below
        if *tag != "trace-value-not-a-photo" {
            assert!(reached.contains(tag), "no case reaches `{tag}`");
        }
        assert!(
            README.contains(&format!("`{tag}`")),
            "`{tag}` is not documented"
        );
    }
    for tag in NEW.iter().filter(|t| !t.starts_with("trace-")) {
        assert!(
            README.contains(&format!("`{tag}` | ")),
            "`{tag}` has no row of its own in the README"
        );
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let c = corpus();
    let full = source(&c, &c["valid"][4]);
    let dir = scratch(&full).parent().expect("dir").to_path_buf();
    let mut cuts: Vec<usize> = full.char_indices().map(|(i, _)| i).collect();
    cuts.push(full.len());
    for (n, at) in cuts.iter().enumerate() {
        let f = dir.join(format!("cut{n}.cui"));
        std::fs::write(&f, &full[..*at]).expect("cut");
        let (code, _, err) = run(&["check", p(&f)]);
        assert!(code == 0 || code == 1, "prefix {at}: exit {code}: {err}");
    }
}

const BOTH: &str = "component C {\n    state pic: String = \"\";\n    on Chosen(r) { pic = json.get(r, \"data\", \"\"); }\n    view { Column { PhotoField { label: \"Photo\" square: true max_side: 200 max_chars: 12000 on: Chosen } Photo { label: \"Shown\" data: pic ratio: \"16:9\" tone: \"4\" } } }\n}\n";

#[test]
fn the_script_makes_a_key_with_a_native_input_over_it_an_icon_a_caption_and_a_status_line() {
    let js = script(BOTH);
    for want in [
        "setAttribute('type', 'file')",
        "setAttribute('accept', 'image/jpeg,image/png,image/webp')",
        "setAttribute('aria-label', \"Photo\")",
        "className = 'cui-photo-key'",
        "className = 'cui-photo-in'",
        "className = 'cui-photo-cap'",
        "setAttribute('role', 'status')",
        "__cui_photo_wire(root, ",
        ", 200, true, 12000, function (v) { __on_Chosen(v); })",
        "__cui_icon(__n1_key, \"camera\", \"cui-icon cui-icon-m\", \"20\", null)",
        "style.aspectRatio = '16 / 9'",
        "setAttribute('role', 'img')",
        "setAttribute('aria-label', \"Shown\")",
        "__cui_photo_draw(__n2, __state_pic, \"4\", 16, 9)",
    ] {
        assert!(js.contains(want), "no `{want}` in\n{js}");
    }
    // nothing the program writes is read as markup, and nothing is fetched or addressed
    for banned in [
        "innerHTML",
        "insertAdjacentHTML",
        "<img",
        "data:image",
        "new Image",
        "XMLHttpRequest",
        "fetch(",
        "createObjectURL",
    ] {
        assert!(!js.contains(banned), "`{banned}` in the picture code");
    }
}

#[test]
fn the_page_has_the_rules_of_the_pictures_and_the_icon_notice_a_field_draws() {
    let html = page(BOTH);
    assert!(
        html.contains(".cui-photo-key {") && html.contains(".cui-photo { --cui-pt:"),
        "no photo rule"
    );
    assert!(
        html.contains("min-height: 44px"),
        "the key is a 44 px target"
    );
    assert!(
        html.contains(".cui-icon { display: inline-block;") && html.contains("ISC License"),
        "a PhotoField draws an icon: the page carries its rules and the licence"
    );
    // the twelve tones of face(..), as gradient starts
    assert_eq!(html.matches(".cui-photo[data-tone=\"").count(), 12);
    // no picture of any kind, and the policy needs nothing for it
    let root = html
        .split("<body>")
        .nth(1)
        .expect("a body")
        .split("<script")
        .next()
        .expect("root");
    for banned in ["<img", "data:image", "<canvas"] {
        assert!(!root.contains(banned), "{banned} in {root}");
    }
    assert_eq!(
        common::directive(&html, "img-src").as_deref(),
        Some("'self' data:"),
        "a picture on a canvas adds nothing to the policy"
    );
    assert_eq!(
        common::directive(&html, "connect-src").as_deref(),
        Some("'self'")
    );
}

#[test]
fn a_photo_alone_has_the_drawing_and_the_decoder_of_face_and_no_chooser() {
    let js = script(
        "component C {\n    state pic: String = \"\";\n    view { Photo { label: \"x\" data: pic } }\n}\n",
    );
    assert!(js.contains("function __cui_photo_draw("), "{js}");
    assert!(js.contains("function __cui_photo_ok("));
    assert!(
        js.contains("function __cui_face_bmp("),
        "the decoder and the cache of face(..)"
    );
    assert!(!js.contains("__cui_photo_wire"), "no chooser");
    let html = page(
        "component C {\n    state pic: String = \"\";\n    view { Photo { label: \"x\" data: pic } }\n}\n",
    );
    assert!(html.contains(".cui-photo {"));
    assert!(
        !html.contains(".cui-icon {"),
        "a Photo draws no icon, so no icon rule and no notice"
    );
}

#[test]
fn a_field_alone_has_the_chooser_and_no_drawing() {
    let js = script(
        "component C {\n    on Chosen(r) { }\n    view { PhotoField { label: \"x\" on: Chosen } }\n}\n",
    );
    assert!(js.contains("function __cui_photo_wire("));
    assert!(
        js.contains("function __cui_photo_ok("),
        "the chooser checks its own text"
    );
    assert!(!js.contains("__cui_photo_draw"), "no drawing");
    assert!(!js.contains("__cui_face_bmp"), "no decoder of face(..)");
}

#[test]
fn a_program_with_neither_element_is_the_page_it_was() {
    let plain = "component C {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Column { Button { label: \"Add\" on: Go } Text { \"{n}\" } Icon { label: \"i\" name: \"bell\" } } }\n}\n";
    let js = script(plain);
    let html = page(plain);
    assert!(
        !js.contains("__cui_photo") && !js.contains("cui-photo"),
        "{js}"
    );
    assert!(!js.contains("__cui_face"), "and no face(..) either");
    assert!(!html.contains("cui-photo"), "no rule of the pictures");
}

#[test]
fn a_face_alone_is_unchanged_and_a_face_with_a_photo_has_one_decoder() {
    let face = "component C {\n    state i: String = \"AR\";\n    view { Text { style: \"avatar\" face(i, \"3\", \"\", \"\") } }\n}\n";
    let js = script(face);
    assert!(js.contains("function __cui_face_draw("));
    assert!(
        !js.contains("__cui_photo"),
        "a program that draws a face does not get the code of a photo"
    );
    let both = "component C {\n    state i: String = \"AR\";\n    state pic: String = \"\";\n    view { Column { Text { style: \"avatar\" face(i, \"3\", \"\", pic) } Photo { label: \"x\" data: pic } } }\n}\n";
    let js = script(both);
    assert_eq!(js.matches("function __cui_face_bmp(").count(), 1);
    assert_eq!(js.matches("function __cui_face_draw(").count(), 1);
    assert_eq!(js.matches("function __cui_photo_ok(").count(), 1);
}

#[test]
fn the_page_policy_is_the_baseline_a_field_and_a_photo_need_nothing_more() {
    // docs/structure.md 12.0r: the picture is made and drawn by canvases, from `File` and `Blob` objects the page itself holds
    let html = page(BOTH);
    let policy = common::csp(&html).expect("a policy");
    assert!(policy.starts_with("default-src 'self'; "), "{policy}");
    for banned in ["blob:", "data:image", "unsafe-eval", "https:"] {
        assert!(!policy.contains(banned), "{banned} in {policy}");
    }
}

fn replay(src: &str, steps: Vec<Value>) -> (i32, Value) {
    let file = scratch(src);
    let trace = file.with_file_name("t.trace.json");
    std::fs::write(
        &trace,
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string(),
    )
    .expect("the trace");
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    (
        code,
        serde_json::from_str(&stdout).unwrap_or(Value::String(stderr)),
    )
}

fn rules(report: &Value) -> Vec<String> {
    report["failures"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| f["rule"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default()
}

const FIELD: &str = "component C {\n    state s: String = \"\";\n    on Chosen(r) { s = r; }\n    view { PhotoField { label: \"Photo\" max_chars: 300 on: Chosen } }\n}\n";

#[test]
fn replay_takes_what_the_chooser_gives_and_refuses_what_it_cannot() {
    let data = |n: usize| {
        let c = json!({"head": "/9j/", "fill": "A", "tail": "", "len": n});
        json!({ "data": picture(&c) }).to_string()
    };
    for ok in [
        data(300),
        data(100),
        json!({"error": "too big"}).to_string(),
        json!({"error": "not a picture"}).to_string(),
    ] {
        let (code, report) = replay(
            FIELD,
            vec![json!({"event": "Chosen", "params": [ok], "state": {"s": ok}})],
        );
        assert_eq!(code, 0, "{ok}: {report}");
    }
    let cases = [
        ("one character over the cap", data(301)),
        ("under 100 characters", data(99)),
        ("an error word of its own", json!({"error": "unreadable"}).to_string()),
        ("both a picture and an error", json!({"data": picture(&json!({"head": "/9j/", "fill": "A", "tail": "", "len": 200})), "error": "too big"}).to_string()),
        ("not a JSON object", "[\"data\"]".to_string()),
        ("not JSON", "hello".to_string()),
        ("a data that is not text", json!({"data": 5}).to_string()),
        ("a text that is not base64 of a JPEG", json!({"data": "A".repeat(200)}).to_string()),
        // a Photo draws these, but the field draws a picture again as a JPEG and never gives one
        ("a PNG (a field never gives one)", json!({"data": picture(&json!({"head": "iVBORw0KGgo", "fill": "A", "tail": "", "len": 200}))}).to_string()),
        ("a WebP (a field never gives one)", json!({"data": picture(&json!({"head": "UklGRiQAAABXRUJQ", "fill": "A", "tail": "", "len": 200}))}).to_string()),
        ("an extra key", json!({"data": picture(&json!({"head": "/9j/", "fill": "A", "tail": "", "len": 200})), "name": "a.jpg"}).to_string()),
    ];
    for (why, value) in cases {
        let (code, report) = replay(
            FIELD,
            vec![json!({"event": "Chosen", "params": [value], "expect": {"outcome": "completed"}})],
        );
        assert_eq!(code, 4, "{why}: {report}");
        assert_eq!(rules(&report), ["trace-value-not-a-photo"], "{why}");
    }
}

#[test]
fn a_button_that_can_give_the_event_any_string_lifts_the_check() {
    let src = "component C {\n    state s: String = \"\";\n    on Chosen(r) { s = r; }\n    view { Column { PhotoField { label: \"Photo\" on: Chosen } Button { label: \"Any\" on: Chosen(\"zzz\") } } }\n}\n";
    let (code, report) = replay(
        src,
        vec![json!({"event": "Chosen", "params": ["zzz"], "state": {"s": "zzz"}})],
    );
    assert_eq!(code, 0, "{report}");
}

#[test]
fn the_example_checks_builds_and_has_both_elements_and_the_three_places_a_photo_goes() {
    let (code, _, err) = run(&["check", "examples/photo/photo.cui"]);
    assert_eq!(code, 0, "{err}");
    let out = scratch("").parent().expect("dir").join("demo");
    let (code, _, err) = run(&["build", "examples/photo/photo.cui", "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    assert_eq!(
        js.matches("__cui_photo_draw(__n").count(),
        3,
        "wide, cover and banner"
    );
    assert!(js.contains("__cui_photo_wire("));
}
