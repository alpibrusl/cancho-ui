//! Keys a program binds (`docs/keys.md`): the combinations, what the compiler writes, what it refuses, and that replay fires the events a key is bound to. What the browser does with a
//! real key is `scripts/check-keys.js`, outside the gate because it needs a browser.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const KEYS: &str = include_str!("../../../examples/keys/keys.cui");

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
    let dir = common::scratch_root().join(format!("cancho-ui-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn build_js(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

fn check_codes(src: &str, name: &str) -> Vec<String> {
    let file = scratch(name, src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

use cancho_ui_ir::keys::parse_combo;

/// A component with the events the keys name and one container around `inner`.
fn with(inner: &str) -> String {
    format!(
        "component A {{\n    state n: Int = 0;\n    state off: Bool = false;\n    state s: String = \"\";\n    on Go {{ n = n + 1; }}\n    on Step(by) {{ n = n + by; }}\n    view {{ Column {{ label: \"C\" {inner} Text {{ \"{{n}}\" }} }} }}\n}}\n"
    )
}

#[test]
fn the_grammar_takes_the_closed_list_and_nothing_else() {
    for ok in [
        "ArrowDown",
        "Shift+ArrowDown",
        "Control+ArrowDown",
        "Control+Shift+ArrowDown",
        "Control+Home",
        "Shift+Home",
        "Shift+PageDown",
        "Enter",
        "Shift+Enter",
        "Space",
        "Shift+Space",
        "Escape",
        "Delete",
        "Backspace",
        "PageUp",
        "End",
        "a",
        "z",
        "0",
        "9",
    ] {
        assert!(parse_combo(ok).is_some(), "{ok}");
    }
    for bad in [
        "",
        "Tab",
        "Shift+Tab",
        "Alt+ArrowDown",
        "Meta+ArrowDown",
        "Control+Enter",
        "Control+a",
        "Control+Delete",
        "Shift+a",
        "Shift+1",
        "A",
        "ab",
        "arrowdown",
        "Shift+Control+ArrowDown",
        "Shift+",
        "+",
        "Control+",
        "Shift+Shift+Home",
        "F1",
        " ",
        "ArrowDown ",
    ] {
        assert!(parse_combo(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn the_example_is_one_listener_for_each_key_and_a_program_without_keys_has_none() {
    let js = build_js(KEYS, "keys.cui");
    assert_eq!(js.matches("addEventListener('keydown'").count(), 8, "{js}");
    assert!(
        js.contains("isComposing") && js.contains("isContentEditable"),
        "{js}"
    );
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    let other = build_js(&counter, "counter.cui");
    assert!(!other.contains("'keydown'"), "{other}");
}

#[test]
fn replay_fires_the_events_a_key_is_bound_to() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (code, out, err) = run(&[
        "test",
        root.join("examples/keys/keys.cui").to_str().expect("path"),
        root.join("examples/keys/keys.trace.json")
            .to_str()
            .expect("path"),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("PASS  6 steps"), "{out}");
}

#[test]
fn each_refusal_has_its_code() {
    let key = |p: &str| with(&format!("Key {{ press: \"{p}\" on: Go }}"));
    build_js(&key("ArrowDown"), "ok.cui");
    for bad in ["Tab", "Alt+x", "Shift+a", "Control+Enter", ""] {
        assert_eq!(
            check_codes(&key(bad), "c.cui"),
            ["key-combo-invalid"],
            "{bad:?}"
        );
    }
    // a name for the container
    let unnamed = "component A {\n    on Go {}\n    view { Column { Key { press: \"ArrowDown\" on: Go } Text { \"x\" } } }\n}\n";
    assert_eq!(check_codes(unnamed, "u.cui"), ["key-parent-unnamed"]);
    // two keys of one container with one combination, and the same one in two containers
    assert_eq!(
        check_codes(
            &with("Key { press: \"ArrowDown\" on: Go } Key { press: \"ArrowDown\" on: Go }"),
            "d.cui"
        ),
        ["key-duplicate"]
    );
    let two = "component A {\n    on Go {}\n    view { Column { Column { label: \"A\" Key { press: \"ArrowDown\" on: Go } Text { \"x\" } } Column { label: \"B\" Key { press: \"ArrowDown\" on: Go } Text { \"y\" } } } }\n}\n";
    build_js(two, "two.cui");
    // not inside the top level, a For or an If
    let top = "component A {\n    on Go {}\n    view { Key { press: \"ArrowDown\" on: Go } }\n}\n";
    assert_eq!(check_codes(top, "t.cui"), ["key-outside-container"]);
    let looped = "component A {\n    state l: List[String] = [\"a\"];\n    on Go {}\n    view { Column { label: \"C\" For { over: l as: x Key { press: \"ArrowDown\" on: Go } } } }\n}\n";
    assert_eq!(check_codes(looped, "f.cui"), ["key-outside-container"]);
    // what a key holds, and what it needs
    assert_eq!(
        check_codes(
            &with("Key { press: \"ArrowDown\" on: Go Text { \"x\" } }"),
            "h.cui"
        ),
        ["key-child-invalid"]
    );
    assert_eq!(
        check_codes(&with("Key { press: \"ArrowDown\" }"), "e.cui"),
        ["key-event-missing"]
    );
    assert_eq!(
        check_codes(&with("Key { on: Go }"), "p.cui"),
        ["key-combo-invalid"]
    );
    // the event is the component's and takes what the key gives it
    assert_eq!(
        check_codes(&with("Key { press: \"ArrowDown\" on: Nope }"), "n.cui"),
        ["event-unknown"]
    );
    assert_eq!(
        check_codes(&with("Key { press: \"ArrowDown\" on: Step }"), "a.cui"),
        ["event-arity-mismatch"]
    );
    assert_eq!(
        check_codes(
            &with("Key { press: \"ArrowDown\" on: Step(1, 2) }"),
            "b.cui"
        ),
        ["event-arity-mismatch"]
    );
    build_js(
        &with("Key { press: \"ArrowDown\" on: Step(2) disabled: off }"),
        "ok2.cui",
    );
    assert_eq!(
        check_codes(
            &with("Key { press: \"ArrowDown\" on: Go disabled: n }"),
            "g.cui"
        ),
        ["disabled-not-bool"]
    );
    // `press:` belongs to a Key
    assert_eq!(
        check_codes(
            "component A {\n    on Go {}\n    view { Button { label: \"B\" press: \"ArrowDown\" on: Go } }\n}\n",
            "k.cui"
        ),
        ["binding-unknown"]
    );
    // an event that only a key fires is used
    build_js(
        &with("Key { press: \"Enter\" on: Go }").replace("Text { \"{n}\" }", "Text { \"x\" }"),
        "used.cui",
    );
}
