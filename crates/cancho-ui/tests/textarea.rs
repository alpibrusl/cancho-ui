//! `TextArea` (docs/controls.md section 8): the corpus `tests/textarea_corpus.json` replayed in-process (the file
//! `scripts/check-textarea.js` types into a real `<textarea>` in Chromium), and the rules that refuse a misuse.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/textarea_corpus.json");

fn program(bindings: &str) -> String {
    format!(
        "component N {{\n    state note: String = \"\";\n    state msg: String = \"\";\n    state n: Int = 0;\n    on Note(v) {{ note = v; }}\n    on Go {{ n = n + 1; }}\n    view {{ Column {{ TextArea {{ label: \"Note\" value: note on: Note {bindings} }} Text {{ \"{{note}}\" }} }} }}\n}}\n"
    )
}

fn trace(value: &str) -> String {
    json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
        {"event": "Note", "params": [value], "state": {"note": value}}
    ]})
    .to_string()
}

fn report(bindings: &str, value: &str) -> cancho_ui_replay::TestReport {
    let src = program(bindings);
    let ast = parse(lex(&src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    replay(&ir, &trace(value), "prog.cui", "t.json")
}

#[test]
fn replay_takes_what_the_browser_reports_and_refuses_what_it_cannot() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let cases = c["typing"].as_array().expect("a list");
    assert!(cases.len() >= 8, "the corpus shrank");
    let mut limited = 0;
    for case in cases {
        let received = case["received"].as_str().expect("received");
        let typed = case["typed"].as_str().expect("typed");
        let bindings = case
            .get("maxlength")
            .map(|m| format!("maxlength: {m}"))
            .unwrap_or_default();
        let ok = report(&bindings, received);
        assert!(
            ok.failures.is_empty() && ok.result == "pass",
            "{}: {:?}",
            case["why"],
            ok.failures
        );
        if let Some(max) = case.get("maxlength").and_then(Value::as_u64)
            && typed.encode_utf16().count() as u64 > max
        {
            limited += 1;
            let bad = report(&bindings, typed);
            let rules: Vec<&str> = bad.failures.iter().map(|f| f.rule).collect();
            assert_eq!(rules, ["trace-value-too-long"], "{}", case["why"]);
        }
    }
    assert!(
        limited >= 2,
        "the corpus must show the limit cutting a value"
    );
}

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

/// A file of its own per call: the tests run in parallel, and a shared name would be a race.
fn scratch(name: &str, text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cancho-ui-ta-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("p.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{src}: {stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

fn accepted(src: &str) {
    let file = scratch("ok.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{src}: {stdout}");
}

#[test]
fn the_bindings_a_text_area_takes_are_checked() {
    accepted(&program(
        "rows: 6 maxlength: 200 name: \"note\" autocomplete: \"off\" inputmode: \"text\" error: msg placeholder: \"..\"",
    ));
    accepted(&program("disabled: false"));
    for (binding, rule) in [
        ("rows: 0", "input-attribute-invalid"),
        ("rows: 31", "input-attribute-invalid"),
        ("rows: \"4\"", "input-attribute-invalid"),
        ("maxlength: 0", "input-attribute-invalid"),
        ("inputmode: \"number\"", "input-attribute-invalid"),
        ("error: n", "error-binding-invalid"),
        ("type: \"password\"", "binding-unknown"),
        ("submit: Go", "binding-unknown"),
    ] {
        assert_eq!(codes(&program(binding)), [rule], "{binding}");
    }
}

#[test]
fn rows_belongs_to_a_text_area() {
    for element in [
        "TextField { label: \"F\" value: note on: Note rows: 3 }",
        "Button { label: \"Go\" on: Go rows: 3 }",
    ] {
        let src = format!(
            "component N {{\n    state note: String = \"\";\n    state n: Int = 0;\n    on Note(v) {{ note = v; }}\n    on Go {{ n = n + 1; }}\n    view {{ Column {{ {element} }} }}\n}}\n"
        );
        assert_eq!(codes(&src), ["binding-unknown"], "{element}");
    }
}

#[test]
fn the_page_has_a_textarea_with_its_label_and_attributes() {
    let file = scratch("emit.cui", &program("rows: 6 maxlength: 200 error: msg"));
    let out = scratch("emit.html", "");
    let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    for needle in [
        "document.createElement('textarea')",
        "setAttribute('rows', '6')",
        "setAttribute('maxlength', '200')",
        "setAttribute('aria-describedby',",
        "'input', function (e) { __on_Note(e.target.value); }",
    ] {
        assert!(html.contains(needle), "{needle}");
    }
    assert!(
        !html.contains("setAttribute('type'"),
        "a textarea has no type attribute"
    );
}

#[test]
fn the_semantic_diff_names_the_element() {
    let a = scratch("a.cui", &program(""));
    let b = scratch("b.cui", &program("rows: 6"));
    let (code, stdout, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("TextArea"), "{stdout}");
}
