//! `Checkbox` (docs/controls.md section 9): the corpus `tests/checkbox_corpus.json` replayed in-process (the file
//! `scripts/check-checkbox.js` clicks, presses Space on and reads in Chromium), and the rules that refuse a misuse.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/checkbox_corpus.json");

#[test]
fn replay_agrees_with_every_step_of_the_corpus() {
    let c: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let src = c["program"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let steps: Vec<Value> = c["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| {
            json!({"event": s["event"], "params": s["params"], "state": s["state"],
                   "expect": {"outcome": s["outcome"]}})
        })
        .collect();
    assert!(steps.len() >= 8, "the corpus shrank");
    let trace =
        json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string();
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
    let dir = std::env::temp_dir().join(format!("cancho-ui-cb-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(element: &str) -> String {
    format!(
        "component F {{\n    state on_: Bool = false;\n    state s: String = \"\";\n    state n: Int = 0;\n    state msg: String = \"\";\n    state busy: Bool = false;\n    on Toggle(v) {{ on_ = v; }}\n    on Flip {{ on_ = !on_; }}\n    view {{ Column {{ {element} }} }}\n}}\n"
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
fn a_well_formed_checkbox_is_accepted() {
    for element in [
        "Checkbox { label: \"A\" checked: on_ on: Toggle }",
        "Checkbox { label: \"A\" checked: on_ on: Flip }",
        "Checkbox { label: \"A\" checked: on_ on: Toggle disabled: busy error: msg name: \"agree\" }",
        "Checkbox { label: \"A\" checked: on_ on: Toggle disabled: true }",
    ] {
        let file = scratch("ok.cui", &program(element));
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 0, "{element}: {stdout}");
    }
}

#[test]
fn a_checkbox_needs_what_it_shows_and_what_it_reports_to() {
    for element in [
        "Checkbox { label: \"A\" on: Toggle }",
        "Checkbox { label: \"A\" checked: on_ }",
        "Checkbox { label: \"A\" }",
    ] {
        assert_eq!(
            codes(&program(element)),
            ["checkbox-incomplete"],
            "{element}"
        );
    }
}

#[test]
fn checked_names_a_bool_state_field() {
    for binding in ["checked: s", "checked: n", "checked: nope"] {
        assert_eq!(
            codes(&program(&format!(
                "Checkbox {{ label: \"A\" {binding} on: Toggle }}"
            ))),
            ["checked-binding-invalid"],
            "{binding}"
        );
    }
    for binding in ["checked: true", "checked: \"x\""] {
        let found = codes(&program(&format!(
            "Checkbox {{ label: \"A\" {binding} on: Toggle }}"
        )));
        assert_eq!(found.len(), 1, "{binding}: {found:?}");
    }
}

#[test]
fn checked_belongs_to_a_checkbox_and_a_checkbox_takes_only_its_own_keys() {
    for element in [
        "Button { label: \"A\" on: Flip checked: on_ }",
        "TextField { label: \"A\" value: s on: Toggle checked: on_ }",
    ] {
        assert_eq!(codes(&program(element)), ["binding-unknown"], "{element}");
    }
    for binding in [
        "value: s",
        "placeholder: \"x\"",
        "type: \"password\"",
        "submit: Flip",
        "maxlength: 3",
        "autocomplete: \"off\"",
        "rows: 2",
        "tone: \"ok\"",
    ] {
        let found = codes(&program(&format!(
            "Checkbox {{ label: \"A\" checked: on_ on: Flip {binding} }}"
        )));
        assert!(
            found
                .iter()
                .all(|c| c == "binding-unknown" || c == "submit-event-invalid"),
            "{binding}: {found:?}"
        );
    }
}

#[test]
fn the_handler_takes_at_most_the_one_value_the_box_reports() {
    let src = program("Checkbox { label: \"A\" checked: on_ on: Two }")
        .replace("on Flip", "on Two(a, b) { on_ = a; }\n    on Flip");
    assert_eq!(codes(&src), ["event-arity-mismatch"]);
}

#[test]
fn the_page_draws_a_checkbox_in_a_label_and_reports_its_state() {
    let file = scratch(
        "emit.cui",
        &program("Checkbox { label: \"A\" checked: on_ on: Toggle }"),
    );
    let out = scratch("emit.html", "");
    let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    for needle in [
        "setAttribute('type', 'checkbox')",
        ".checked = !!__state_on_",
        "classList.add('cui-check')",
        "addEventListener('change', function (e) { __on_Toggle(e.target.checked); })",
    ] {
        assert!(html.contains(needle), "{needle}");
    }
}

#[test]
fn the_semantic_diff_names_the_element() {
    let a = scratch(
        "a.cui",
        &program("Checkbox { label: \"A\" checked: on_ on: Toggle }"),
    );
    let b = scratch(
        "b.cui",
        &program("Checkbox { label: \"B\" checked: on_ on: Toggle }"),
    );
    let (code, stdout, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("Checkbox"), "{stdout}");
}
