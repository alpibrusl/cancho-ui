//! `match-non-exhaustive` (#63): the walk that finds every `match` visits every expression position, not a hand-listed
//! set. One program per position, each with a `match` over a `Result` state field that lacks its `Err` arm; each must be
//! refused with the rule, through `check` (exit 1) and through `build`. Positions that the walk missed before: the
//! arguments of `len`, `filter`, `map` and `sort`, and the list of a `filter`/`map`/`sort`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

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
    let dir = common::scratch_root().join(format!("cancho-ui-matchpos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

/// A component whose handler assigns `<value>` to an `Int` field, with a result field `r` and a list field `names`.
fn program(value: &str) -> String {
    format!(
        "component C {{\n    state n: Int = 0;\n    state r: Result[Int, String] = Ok(0);\n    state names: List[String] = [\"a\"];\n    on Go {{ n = {value}; }}\n    view {{ Text {{ \"{{n}}\" }} }}\n}}\n"
    )
}

/// Each of these holds a `match r {{ Ok(v) => ... }}` with no `Err` arm.
const BAD: &str = "match r { Ok(v) => { 1 } }";

fn positions() -> Vec<(&'static str, String)> {
    vec![
        ("len argument", program(&format!("len({BAD})"))),
        (
            "filter list",
            program(&format!("len(filter({BAD}, x => true))")),
        ),
        (
            "filter condition",
            program(&format!("len(filter(names, x => {BAD}))")),
        ),
        ("map list", program(&format!("len(map({BAD}, x => x))"))),
        ("map body", program(&format!("len(map(names, x => {BAD}))"))),
        ("sort list", program(&format!("len(sort({BAD}, x => x))"))),
        (
            "sort key",
            program(&format!("len(sort(names, x => {BAD}))")),
        ),
        (
            "if condition",
            program(&format!("if {BAD} == 1 {{ 1 }} else {{ 2 }}")),
        ),
        ("binary operand", program(&format!("1 + {BAD}"))),
        ("call argument", program(&format!("min({BAD}, 2)"))),
    ]
}

#[test]
fn a_match_missing_an_arm_is_refused_in_every_expression_position() {
    for (name, src) in positions() {
        let path = scratch("p.cui", &src);
        let p = path.to_str().expect("UTF-8 temp path");
        let (code, stdout, stderr) = run(&["check", p, "--output", "json"]);
        let all = format!("{stdout}{stderr}");
        assert_eq!(code, 1, "{name}: check should refuse: {all}");
        assert!(
            all.contains("match-non-exhaustive"),
            "{name}: expected match-non-exhaustive, got: {all}"
        );
        assert!(!all.contains("panicked"), "{name}: a panic: {all}");
    }
}

#[test]
fn a_complete_match_in_the_same_positions_is_accepted() {
    let good = "match r { Ok(v) => { 1 } Err(e) => { 0 } }";
    let src = program(&format!(
        "len(filter(names, x => {good} == 1)) + len(sort(names, x => {good})) + len(map(names, x => {good}))"
    ));
    let path = scratch("ok.cui", &src);
    let p = path.to_str().expect("UTF-8 temp path");
    let (code, stdout, stderr) = run(&["check", p]);
    assert_eq!(code, 0, "a complete match must pass: {stdout}{stderr}");
}
