//! `AudioField` and `AudioClip` (docs/structure.md 12.0t): every refusal is refused by its rule tag and only by it, the valid views check, the texts a clip plays are the ones the compiler's check and
//! the page's agree on (`tests/audio_corpus.json`), the script and the rules of the sound are in the page only of a program that has one, the page's policy needs nothing for it, replay refuses a
//! value the recorder could not give, and the lists of the docs name every tag. What a browser does (recording, playing) is `scripts/check-audio.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/audio_corpus.json");
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

fn scratch(text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-audio-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let f = dir.join("r.cui");
    std::fs::write(&f, text).expect("the program");
    f
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

/// A component with the view `view`.
fn program(view: &str) -> String {
    format!(
        "component C {{\n    state clip: String = \"\";\n    state n: Int = 0;\n    state rows: List[String] = [\"a\", \"b\"];\n    on Recorded(r) {{ clip = json.get(r, \"data\", \"\"); }}\n    on Plain {{ n = 1; }}\n    on Two(a, b) {{ n = 2; }}\n    view {{ Column {{ {view} }} }}\n}}\n"
    )
}

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("the corpus is JSON")
}

/// The text a corpus case describes: head, then fill repeated, then tail, `len` characters in all.
fn recording(case: &Value) -> String {
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

#[test]
fn the_texts_of_the_corpus_are_recordings_or_not_as_the_compiler_says() {
    let c = corpus();
    let cases = c["recordings"].as_array().expect("recordings");
    assert!(cases.len() >= 20, "the corpus shrank");
    for case in cases {
        let want = case["ok"]
            .as_bool()
            .expect("ok")
            .then(|| case["kind"].as_str().expect("kind"));
        assert_eq!(
            cancho_ui_ir::audio::audio_kind(&recording(case)),
            want,
            "{}",
            case["why"]
        );
    }
}

const VALID: &[(&str, &str)] = &[
    ("a field", "AudioField { label: \"Voice\" on: Recorded }"),
    (
        "a field with every key",
        "AudioField { label: \"Voice\" icon: \"mic\" max_seconds: 60 max_chars: 48000 on: Recorded }",
    ),
    (
        "a field with the least limits",
        "AudioField { label: \"Voice\" max_seconds: 1 max_chars: 100 on: Recorded }",
    ),
    (
        "a field whose event ignores the text",
        "AudioField { label: \"Voice\" on: Plain }",
    ),
    (
        "a field named from state",
        "AudioField { label: clip on: Recorded }",
    ),
    ("a clip", "AudioClip { label: \"Voice note\" data: clip }"),
    (
        "a clip for every row",
        "For { over: rows as: r AudioClip { label: \"Voice note\" data: r } }",
    ),
    (
        "a clip named for its row",
        "For { over: rows as: r AudioClip { label: \"Note \" + r data: r } }",
    ),
    (
        "a field and a photo field",
        "AudioField { label: \"Voice\" on: Recorded } PhotoField { label: \"Photo\" max_chars: 300 on: Recorded }",
    ),
];

#[test]
fn every_valid_view_checks() {
    for (name, view) in VALID {
        let f = scratch(&program(view));
        assert_eq!(codes(&f), (0, vec![]), "{name}");
    }
}

/// (what is wrong, the view, the one tag that refuses it)
const REFUSED: &[(&str, &str, &str)] = &[
    (
        "a field with no label",
        "AudioField { on: Recorded }",
        "audio-field-incomplete",
    ),
    (
        "a field with a blank label",
        "AudioField { label: \"  \" on: Recorded }",
        "audio-field-incomplete",
    ),
    (
        "a field with no event",
        "AudioField { label: \"Voice\" }",
        "audio-field-incomplete",
    ),
    (
        "a field in a For",
        "For { over: rows as: r AudioField { label: \"Voice\" on: Recorded } }",
        "audio-field-in-loop",
    ),
    (
        "a field with a child",
        "AudioField { label: \"Voice\" on: Recorded Text { \"x\" } }",
        "audio-has-children",
    ),
    (
        "a clip with a child",
        "AudioClip { label: \"Voice\" data: clip Text { \"x\" } }",
        "audio-has-children",
    ),
    (
        "no seconds at all",
        "AudioField { label: \"Voice\" max_seconds: 0 on: Recorded }",
        "audio-limit-invalid",
    ),
    (
        "more than a minute",
        "AudioField { label: \"Voice\" max_seconds: 61 on: Recorded }",
        "audio-limit-invalid",
    ),
    (
        "a text under the least",
        "AudioField { label: \"Voice\" max_chars: 99 on: Recorded }",
        "audio-limit-invalid",
    ),
    (
        "a text over the most",
        "AudioField { label: \"Voice\" max_chars: 48001 on: Recorded }",
        "audio-limit-invalid",
    ),
    (
        "seconds that are text",
        "AudioField { label: \"Voice\" max_seconds: \"9\" on: Recorded }",
        "audio-limit-invalid",
    ),
    (
        "a clip with no data",
        "AudioClip { label: \"Voice\" }",
        "audio-clip-incomplete",
    ),
    (
        "a clip with no label",
        "AudioClip { data: clip }",
        "audio-clip-label-missing",
    ),
    (
        "a clip with a blank label",
        "AudioClip { label: \" \" data: clip }",
        "audio-clip-label-missing",
    ),
    (
        "a clip over an Int",
        "AudioClip { label: \"Voice\" data: n }",
        "audio-data-invalid",
    ),
    (
        "an icon that is not in the set",
        "AudioField { label: \"Voice\" icon: \"microphone-9\" on: Recorded }",
        "icon-unknown",
    ),
    (
        "a key of the picture field",
        "AudioField { label: \"Voice\" square: true on: Recorded }",
        "binding-unknown",
    ),
    (
        "max_side, which is a picture's",
        "AudioField { label: \"Voice\" max_side: 200 on: Recorded }",
        "binding-unknown",
    ),
    (
        "on on a clip",
        "AudioClip { label: \"Voice\" data: clip on: Recorded }",
        "binding-unknown",
    ),
    (
        "max_seconds on a picture field",
        "PhotoField { label: \"Photo\" max_seconds: 9 on: Recorded }",
        "binding-unknown",
    ),
    (
        "an event that does not exist",
        "AudioField { label: \"Voice\" on: Nowhere }",
        "event-unknown",
    ),
    (
        "an event with two parameters",
        "AudioField { label: \"Voice\" on: Two }",
        "event-arity-mismatch",
    ),
    (
        "arguments of its own",
        "AudioField { label: \"Voice\" on: Recorded(\"x\") }",
        "event-args-unsupported",
    ),
];

#[test]
fn every_refusal_is_refused_by_its_tag() {
    for (why, view, tag) in REFUSED {
        let f = scratch(&program(view));
        let (code, found) = codes(&f);
        assert_eq!(code, 1, "{why}: {found:?}");
        assert_eq!(found, vec![tag.to_string()], "{why}");
    }
}

#[test]
fn every_new_tag_is_documented_with_a_row_of_its_own_and_reached() {
    for tag in [
        "audio-field-incomplete",
        "audio-field-in-loop",
        "audio-has-children",
        "audio-limit-invalid",
        "audio-clip-incomplete",
        "audio-clip-label-missing",
        "audio-data-invalid",
        "trace-value-not-a-recording",
    ] {
        assert!(
            README.contains(&format!("`{tag}`")),
            "`{tag}` is not documented"
        );
        if !tag.starts_with("trace-") {
            assert!(
                README.contains(&format!("`{tag}` | ")),
                "`{tag}` has no row of its own in the README"
            );
            assert!(
                REFUSED.iter().any(|r| r.2 == tag),
                "no case reaches `{tag}`"
            );
        }
    }
}

#[test]
fn a_program_cut_anywhere_is_refused_and_never_a_panic() {
    let full = program(
        "AudioField { label: \"Voice\" icon: \"mic\" max_seconds: 15 max_chars: 30000 on: Recorded } AudioClip { label: \"Voice note\" data: clip }",
    );
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

const BOTH: &str = "component C {\n    state clip: String = \"\";\n    on Recorded(r) { clip = json.get(r, \"data\", \"\"); }\n    view { Column { AudioField { label: \"Voice\" max_seconds: 12 max_chars: 30000 on: Recorded } AudioClip { label: \"Voice note\" data: clip } } }\n}\n";

#[test]
fn the_script_makes_a_key_with_its_icons_a_time_and_a_status_line_and_a_clip_from_the_text() {
    let js = script(BOTH);
    for want in [
        "className = 'cui-audio-field'",
        "setAttribute('type', 'button')",
        "className = 'cui-audio-key'",
        "setAttribute('aria-label', \"Voice\")",
        "__cui_icon(__n1_i0, \"mic\", \"cui-icon cui-icon-m\", \"20\", null)",
        "__cui_icon(__n1_i1, \"square\", \"cui-icon cui-icon-m\", \"20\", null)",
        "className = 'cui-audio-time'",
        "setAttribute('role', 'status')",
        "__cui_audio_field_wire(root, ",
        ", 12, 30000, function (v) { __on_Recorded(v); })",
        "className = 'cui-audio-clip'",
        "setAttribute('role', 'group')",
        "__cui_icon(__n2_i0, \"play\", ",
        "__cui_icon(__n2_i1, \"pause\", ",
        "__cui_audio_clip_draw(__n2, __state_clip, \"Voice note\", [__n2_i0, __n2_i1])",
        "function __cui_audio_kind(",
    ] {
        assert!(js.contains(want), "no `{want}` in\n{js}");
    }
    // the sound is made and played by the audio engine: no element, no address, no request, nothing the program writes read as markup
    for banned in [
        "innerHTML",
        "insertAdjacentHTML",
        "<audio",
        "new Audio",
        "createElement('audio')",
        "createElement(\"audio\")",
        "createObjectURL",
        "blob:",
        "data:audio",
        "XMLHttpRequest",
        "fetch(",
        "sendBeacon",
        "eval(",
    ] {
        assert!(!js.contains(banned), "`{banned}` in the sound code");
    }
}

#[test]
fn the_page_has_the_rules_of_the_sound_and_the_icons_it_draws_and_the_policy_is_the_baseline() {
    let html = page(BOTH);
    for want in [
        ".cui-audio-key, .cui-audio-play {",
        "min-height: 44px",
        ".cui-audio-key[aria-pressed=\"true\"] {",
        ".cui-audio-bar {",
        ".cui-icon { display: inline-block;",
    ] {
        assert!(html.contains(want), "no `{want}`");
    }
    // no policy entry for sound: the engine decodes and plays, the page makes no media request
    let policy = common::csp(&html).expect("a policy");
    assert!(policy.starts_with("default-src 'self'; "), "{policy}");
    for banned in ["media-src", "blob:", "data:audio", "unsafe-eval", "https:"] {
        assert!(!policy.contains(banned), "{banned} in {policy}");
    }
    assert_eq!(
        common::directive(&html, "img-src").as_deref(),
        Some("'self' data:")
    );
    assert_eq!(
        common::directive(&html, "connect-src").as_deref(),
        Some("'self'")
    );
    let plain = page(
        "component C {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Column { Button { label: \"Add\" on: Go } Text { \"{n}\" } } }\n}\n",
    );
    assert_eq!(
        common::csp(&plain).expect("a policy"),
        policy,
        "the policy of a page with sound is the policy of a page without it"
    );
}

#[test]
fn a_clip_alone_has_the_player_and_no_recorder_and_a_field_alone_the_recorder_and_no_player() {
    let clip = "component C {\n    state clip: String = \"\";\n    view { AudioClip { label: \"x\" data: clip } }\n}\n";
    let js = script(clip);
    assert!(js.contains("function __cui_audio_clip_draw("), "{js}");
    assert!(js.contains("function __cui_audio_kind("));
    assert!(!js.contains("__cui_audio_field_wire") && !js.contains("getUserMedia"));
    let field = "component C {\n    on Recorded(r) { }\n    view { AudioField { label: \"x\" on: Recorded } }\n}\n";
    let js = script(field);
    assert!(js.contains("function __cui_audio_field_wire("), "{js}");
    assert!(
        js.contains("getUserMedia"),
        "the recorder asks for the microphone"
    );
    assert!(!js.contains("__cui_audio_clip_draw") && !js.contains("decodeAudioData"));
}

#[test]
fn a_program_with_neither_element_is_the_page_it_was() {
    let plain = "component C {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Column { Button { label: \"Add\" on: Go } Text { \"{n}\" } Icon { label: \"i\" name: \"bell\" } } }\n}\n";
    let js = script(plain);
    let html = page(plain);
    assert!(
        !js.contains("__cui_audio") && !js.contains("cui-audio") && !js.contains("getUserMedia"),
        "{js}"
    );
    assert!(!html.contains("cui-audio"), "no rule of the sound");
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

const FIELD: &str = "component C {\n    state s: String = \"\";\n    on Recorded(r) { s = r; }\n    view { AudioField { label: \"Voice\" max_seconds: 10 max_chars: 300 on: Recorded } }\n}\n";

fn text(len: usize, head: &str) -> String {
    json!({"head": head, "fill": "A", "tail": "", "len": len}).to_string()
}

fn made(len: usize, kind: &str, seconds: i64, mime: &str) -> String {
    let c: Value = serde_json::from_str(&text(len, &head_of(kind))).expect("a case");
    json!({"data": recording(&c), "seconds": seconds, "mime": mime}).to_string()
}

fn head_of(kind: &str) -> String {
    let c = corpus();
    let why = match kind {
        "webm" => "webm, the least length",
        "ogg" => "ogg",
        "mp4" => "mp4 (ftyp after four bytes)",
        _ => "wav (RIFF, four bytes, WAVE)",
    };
    c["recordings"]
        .as_array()
        .expect("recordings")
        .iter()
        .find(|r| r["why"] == why)
        .expect("a case")["head"]
        .as_str()
        .expect("head")
        .to_string()
}

#[test]
fn replay_takes_what_the_recorder_gives_and_refuses_what_it_cannot() {
    for ok in [
        made(300, "webm", 7, "audio/webm"),
        made(100, "ogg", 1, "audio/ogg"),
        made(250, "mp4", 10, "audio/mp4"),
        made(250, "wav", 3, "audio/wav"),
        json!({"error": "denied"}).to_string(),
        json!({"error": "unsupported"}).to_string(),
        json!({"error": "no microphone"}).to_string(),
        json!({"error": "failed"}).to_string(),
        json!({"error": "too big"}).to_string(),
    ] {
        let (code, report) = replay(
            FIELD,
            vec![json!({"event": "Recorded", "params": [ok], "state": {"s": ok}})],
        );
        assert_eq!(code, 0, "{ok}: {report}");
    }
    let cases = [
        (
            "one character over the cap",
            made(301, "webm", 7, "audio/webm"),
        ),
        ("under 100 characters", made(99, "webm", 7, "audio/webm")),
        ("no seconds", made(300, "webm", 0, "audio/webm")),
        (
            "more seconds than the field records",
            made(300, "webm", 11, "audio/webm"),
        ),
        (
            "a mime that is not the one the first bytes say",
            made(300, "webm", 7, "audio/ogg"),
        ),
        (
            "a mime the page never gives",
            made(300, "webm", 7, "video/webm"),
        ),
        ("a text that is not a recording", {
            let c: Value = serde_json::from_str(&text(300, "/9j/")).expect("a case");
            json!({"data": recording(&c), "seconds": 7, "mime": "audio/webm"}).to_string()
        }),
        (
            "an error word of its own",
            json!({"error": "unreadable"}).to_string(),
        ),
        ("an error and a recording", {
            let c: Value = serde_json::from_str(&text(300, &head_of("webm"))).expect("a case");
            json!({"error": "denied", "data": recording(&c)}).to_string()
        }),
        ("a recording with no mime", {
            let c: Value = serde_json::from_str(&text(300, &head_of("webm"))).expect("a case");
            json!({"data": recording(&c), "seconds": 7}).to_string()
        }),
        ("seconds that are text", {
            let c: Value = serde_json::from_str(&text(300, &head_of("webm"))).expect("a case");
            json!({"data": recording(&c), "seconds": "7", "mime": "audio/webm"}).to_string()
        }),
        ("an extra key", {
            let c: Value = serde_json::from_str(&text(300, &head_of("webm"))).expect("a case");
            json!({"data": recording(&c), "seconds": 7, "mime": "audio/webm", "name": "a.webm"})
                .to_string()
        }),
        ("not a JSON object", "[\"data\"]".to_string()),
        ("not JSON", "hello".to_string()),
    ];
    for (why, value) in cases {
        let (code, report) = replay(
            FIELD,
            vec![
                json!({"event": "Recorded", "params": [value], "expect": {"outcome": "completed"}}),
            ],
        );
        assert_eq!(code, 4, "{why}: {report}");
        assert_eq!(rules(&report), ["trace-value-not-a-recording"], "{why}");
    }
}

#[test]
fn a_button_that_can_give_the_event_any_string_lifts_the_check() {
    let src = "component C {\n    state s: String = \"\";\n    on Recorded(r) { s = r; }\n    view { Column { AudioField { label: \"Voice\" on: Recorded } Button { label: \"Any\" on: Recorded(\"zzz\") } } }\n}\n";
    let (code, report) = replay(
        src,
        vec![json!({"event": "Recorded", "params": ["zzz"], "state": {"s": "zzz"}})],
    );
    assert_eq!(code, 0, "{report}");
}

#[test]
fn the_example_checks_builds_and_has_both_elements() {
    let (code, _, err) = run(&["check", "examples/audio/audio.cui"]);
    assert_eq!(code, 0, "{err}");
    let out = scratch("").parent().expect("dir").join("demo");
    let (code, _, err) = run(&["build", "examples/audio/audio.cui", "--page", "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let js = std::fs::read_to_string(out.join("app.js")).expect("app.js");
    assert_eq!(js.matches("__cui_audio_clip_draw(__n").count(), 2);
    assert_eq!(js.matches("__cui_audio_field_wire(root, \"").count(), 1);
}
