//! `+ - * /` never answer `Infinity` or `NaN` (docs/semantics.md, "Arithmetic"): the model in replay against
//! `tests/arith_corpus.json` (the file `scripts/check-arith.js` runs against the emitted JavaScript, comparing with
//! `Object.is`), and what is emitted.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_replay::arith;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../tests/arith_corpus.json");

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-arith-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

#[test]
fn the_model_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let entries = corpus.as_array().expect("a list");
    assert!(entries.len() >= 20, "the corpus shrank");
    let mut faults = 0;
    for e in entries {
        let op = e["op"].as_str().expect("an op");
        let (a, b) = (e["a"].as_f64().expect("a"), e["b"].as_f64().expect("b"));
        let got = arith(op, a, b);
        if e["expect"] == "fault" {
            faults += 1;
            assert!(got.is_none(), "{a} {op} {b} [{}]: got {got:?}", e["why"]);
        } else {
            let want = e["expect"].as_f64().expect("a number");
            let got = got.unwrap_or_else(|| panic!("{a} {op} {b} [{}] faulted", e["why"]));
            assert_eq!(got.to_bits(), want.to_bits(), "{a} {op} {b} [{}]", e["why"]);
        }
    }
    assert!(
        faults >= 6,
        "the corpus must refuse division, overflow and 0/0"
    );
}

#[test]
fn the_model_answers_nothing_for_an_unknown_operator() {
    assert!(arith("%", 5.0, 2.0).is_none());
    assert!(arith("", 5.0, 2.0).is_none());
}

fn program(stmt: &str) -> String {
    format!(
        "component C {{\n    state n: Int = 0;\n    state m: Int = 0;\n    state s: String = \"\";\n    on Go {{ {stmt} }}\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n"
    )
}

fn build(src: &str, name: &str) -> String {
    let file = scratch(name, src);
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

#[test]
fn arithmetic_is_emitted_through_the_checked_helpers_once() {
    let js = build(&program("n = m / 2 - m * 3 + 1;"), "arith.cui");
    for f in ["__cui_fin", "__cui_add"] {
        assert_eq!(js.matches(&format!("function {f}(")).count(), 1, "{f}");
    }
    for expr in ["__cui_fin(__state_m / 2)", "__cui_fin(__state_m * 3)"] {
        assert!(js.contains(expr), "{expr} in {js}");
    }
    assert!(js.contains("__cui_add("), "a sum is checked");
}

#[test]
fn a_program_with_no_arithmetic_carries_no_helper() {
    let js = build(&program("n = 5; s = \"a\";"), "none.cui");
    assert!(!js.contains("__cui_fin"), "{js}");
    assert!(!js.contains("__cui_add"), "{js}");
}

#[test]
fn string_concatenation_still_goes_through_add() {
    let js = build(&program("s = \"x\" + s;"), "concat.cui");
    assert!(js.contains("__cui_add("), "{js}");
}

#[test]
fn a_handler_that_never_awaits_can_still_rethrow_what_it_catches() {
    // Every handler's catch compares with `__cui_cancelled`; without an await the adapter that declares it is not emitted,
    // and a thrown `replay-non-finite` would have surfaced as a ReferenceError.
    let js = build(&program("n = m / 2;"), "catch.cui");
    assert_eq!(js.matches("const __cui_cancelled").count(), 1, "{js}");
}
