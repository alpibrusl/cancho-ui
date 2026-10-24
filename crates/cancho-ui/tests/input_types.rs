//! The `type:` of a `TextField` (docs/controls.md section 5): every case of `tests/input_types_corpus.json` replayed in-process
//! (the file `scripts/check-input-types.js` types into a field of that type in Chromium), the rules that refuse a misuse, and
//! what is emitted.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/input_types_corpus.json");

fn program(type_binding: &str) -> String {
    format!(
        "component F {{\n    state v: String = \"\";\n    on V(x) {{ v = x; }}\n    view {{ Column {{ TextField {{ label: \"Field\" value: v on: V {type_binding} }} Text {{ \"got=[{{v}}]\" }} }} }}\n}}\n"
    )
}

#[test]
fn replay_takes_the_value_the_browser_gives_for_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let cases = corpus.as_array().expect("a list");
    assert!(cases.len() >= 7, "the corpus shrank");
    let mut sanitized = 0;
    for c in cases {
        let ty = c["type"].as_str().expect("a type");
        let src = program(&format!("type: \"{ty}\""));
        let ast = parse(lex(&src).expect("lexes")).expect("parses");
        check_file(&ast).expect("type-checks");
        let ir = lower_file(&ast).expect("lowers");
        let trace = json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [
            {"event": "V", "params": [c["received"]], "state": {"v": c["received"]}}
        ]})
        .to_string();
        let r = replay(&ir, &trace, "prog.cui", "t.json");
        assert!(
            r.failures.is_empty() && r.result == "pass",
            "{}: {:?}",
            c["why"],
            r.failures
        );
        if c["typed"] != c["received"] {
            sanitized += 1;
        }
    }
    assert!(sanitized >= 2, "the corpus must show a type that sanitizes");
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
    let dir = common::scratch_root().join(format!("cancho-ui-type-{}-{n}", std::process::id()));
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

#[test]
fn a_type_that_is_not_offered_is_refused() {
    for ty in [
        "checkbox", "date", "hidden", "file", "range", "PASSWORD", "", "text ",
    ] {
        assert_eq!(
            codes(&program(&format!("type: \"{ty}\""))),
            ["input-type-invalid"],
            "{ty}"
        );
    }
}

#[test]
fn a_type_that_is_not_a_string_literal_is_refused() {
    for value in ["password", "true", "3", "v"] {
        let found = codes(&program(&format!("type: {value}")));
        assert_eq!(found.len(), 1, "{value}: {found:?}");
    }
}

#[test]
fn type_is_a_binding_of_a_text_field_only() {
    for element in [
        "Button { label: \"Go\" on: V type: \"password\" }",
        "Row { type: \"password\" Text { \"x\" } }",
        "Column { type: \"password\" Text { \"x\" } }",
    ] {
        let src = format!(
            "component F {{\n    state v: String = \"\";\n    on V {{ v = \"x\"; }}\n    view {{ Column {{ {element} }} }}\n}}\n"
        );
        assert_eq!(codes(&src), ["binding-unknown"], "{element}");
    }
}

#[test]
fn the_attribute_is_on_the_field_and_text_is_the_default() {
    for (binding, want) in [
        ("type: \"password\"", "setAttribute('type', 'password')"),
        ("type: \"tel\"", "setAttribute('type', 'tel')"),
        ("", "setAttribute('type', 'text')"),
    ] {
        let file = scratch("emit.cui", &program(binding));
        let out = scratch("emit.html", "");
        let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 0, "{err}");
        let html = std::fs::read_to_string(&out).expect("the page");
        assert!(html.contains(want), "{binding}: {want}");
        assert_eq!(html.matches("setAttribute('type'").count(), 1, "{binding}");
    }
}

#[test]
fn the_semantic_diff_sees_a_change_of_type() {
    let a = scratch("a.cui", &program("type: \"text\""));
    let b = scratch("b.cui", &program("type: \"password\""));
    let (code, stdout, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("password"), "{stdout}");
}
