//! `error:` on a `TextField` (docs/controls.md section 7): the corpus `tests/field_error_corpus.json` replayed in-process (the
//! file `scripts/check-field-error.js` checks in Chromium, with the accessibility tree), and the rules that refuse a misuse.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/field_error_corpus.json");

fn source() -> String {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    c["program"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn replay_agrees_with_every_step_of_the_corpus() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let steps: Vec<Value> = c["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| json!({"event": "V", "params": [s["typed"]], "state": s["state"]}))
        .collect();
    assert!(steps.len() >= 5, "the corpus shrank");
    let trace =
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string();
    let src = source();
    let ast = parse(lex(&src).expect("lexes")).expect("parses");
    check_file(&ast).expect("type-checks");
    let ir = lower_file(&ast).expect("lowers");
    let r = replay(&ir, &trace, "prog.cui", "t.json");
    assert!(
        r.failures.is_empty() && r.result == "pass",
        "{:?}",
        r.failures
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
    let dir = std::env::temp_dir().join(format!("cancho-ui-err-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(binding: &str, element: &str) -> String {
    format!(
        "component F {{\n    state v: String = \"\";\n    state n: Int = 0;\n    state b: Bool = true;\n    state msg: String = \"\";\n    on V(x) {{ v = x; }}\n    on Go {{ n = n + 1; }}\n    view {{ Column {{ {element} }} }}\n}}\n",
        element = element.replace("{BINDING}", binding)
    )
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
fn error_names_a_string_state_field() {
    let field = "TextField { label: \"F\" value: v on: V {BINDING} }";
    for binding in ["error: n", "error: b", "error: nope"] {
        assert_eq!(
            codes(&program(binding, field)),
            ["error-binding-invalid"],
            "{binding}"
        );
    }
    for binding in ["error: \"x\"", "error: 3", "error: true"] {
        let found = codes(&program(binding, field));
        assert_eq!(found.len(), 1, "{binding}: {found:?}");
    }
    let (code, _, _) = {
        let file = scratch("ok.cui", &program("error: msg", field));
        run(&["check", p(&file)])
    };
    assert_eq!(code, 0);
}

#[test]
fn error_is_a_binding_of_a_text_field_only() {
    for element in [
        "Button { label: \"Go\" on: Go {BINDING} }",
        "Row { {BINDING} Text { \"x\" } }",
    ] {
        assert_eq!(
            codes(&program("error: msg", element)),
            ["binding-unknown"],
            "{element}"
        );
    }
}

#[test]
fn the_message_is_next_to_the_field_and_the_field_points_at_it() {
    let file = scratch(
        "emit.cui",
        &program(
            "error: msg",
            "TextField { label: \"F\" value: v on: V {BINDING} }",
        ),
    );
    let out = scratch("emit.html", "");
    let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    for needle in [
        "setAttribute('aria-invalid', 'true')",
        "setAttribute('aria-describedby',",
        "_err.setAttribute('role', 'status')",
        "'cui-err-' + (window.__cui_uid",
    ] {
        assert!(html.contains(needle), "{needle}");
    }
    // A field without `error:` carries none of it.
    let plain = scratch(
        "plain.cui",
        &program("", "TextField { label: \"F\" value: v on: V }"),
    );
    let (code, _, err) = run(&["build", p(&plain), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    assert!(!html.contains("aria-invalid") && !html.contains("aria-describedby"));
}
