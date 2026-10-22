//! `name-unknown` (alpibrusl/cancho-ui#55): a name that is neither a state field nor a binding in scope is refused at
//! `check`, in a handler, a view expression, a label, an event argument and the `over:` of a `For`; the bindings that do
//! introduce a name (an event parameter, a `For` binding, a lambda parameter, a `match` binding) are not refused.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-names-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn component(handler: &str, view: &str) -> String {
    format!(
        "component C {{\n    state n: Int = 0;\n    state names: List[String] = [\"a\"];\n    state r: Result[String, String] = Ok(\"\");\n    on Go {{ {handler} }}\n    on Pick(id) {{ n = len(id); }}\n    view {{ {view} }}\n}}\n"
    )
}

/// Each call has a file of its own: the tests of a binary run in parallel.
fn diagnostics(src: &str) -> (i32, Value) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let file = scratch(&format!("p{n}.cui"), src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    let diags = if stdout.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&stdout).unwrap_or(Value::Null)
    };
    (code, diags)
}

#[test]
fn an_unknown_name_is_refused_in_every_position() {
    for (handler, view, position) in [
        ("n = nothere;", "Text { \"x\" }", "an assignment"),
        ("n = 1 + nothere;", "Text { \"x\" }", "an operand"),
        ("n = len(nothere);", "Text { \"x\" }", "a builtin argument"),
        ("n = 1;", "Text { \"{nothere}\" }", "a text interpolation"),
        ("n = 1;", "Text { nothere }", "a text expression"),
        (
            "n = 1;",
            "Button { label: \"x\" + nothere on: Go }",
            "a label expression",
        ),
        (
            "n = 1;",
            "Button { label: \"x\" on: Pick(nothere) }",
            "an event argument",
        ),
        (
            "n = 1;",
            "For { over: nothere as: m Text { m } }",
            "the over: of a For",
        ),
        (
            "n = 1;",
            "For { over: names as: m Text { m + nothere } }",
            "inside a For",
        ),
        (
            "n = len(filter(names, x => x == nothere));",
            "Text { \"x\" }",
            "a lambda body",
        ),
        (
            "n = len(match r { Ok(t) => { t } Err(e) => { nothere } Cancelled => { \"\" } });",
            "Text { \"x\" }",
            "a match arm",
        ),
        (
            "n = if nothere > 1 { 1 } else { 2 };",
            "Text { \"x\" }",
            "an if condition",
        ),
        ("n = !nothere;", "Text { \"x\" }", "a negation"),
    ] {
        let (code, diags) = diagnostics(&component(handler, view));
        assert_eq!(code, 1, "{position}: {diags}");
        let diags = diags.as_array().expect("a list");
        assert_eq!(diags.len(), 1, "{position}: {diags:?}");
        assert_eq!(diags[0]["code"], "name-unknown", "{position}");
        let msg = diags[0]["message"].as_str().expect("a message");
        assert!(
            msg.contains("`nothere`") && msg.contains("in scope: "),
            "{position}: {msg}"
        );
        assert!(diags[0]["span"].is_object(), "{position} is located");
    }
}

#[test]
fn the_names_that_a_binding_introduces_are_in_scope() {
    for (handler, view, what) in [
        (
            "n = 1;",
            "For { over: names as: m Text { m } }",
            "a For binding",
        ),
        (
            "n = len(filter(names, x => len(x) > 0));",
            "Text { \"x\" }",
            "a lambda parameter",
        ),
        (
            "n = len(map(names, x => x + \"!\"));",
            "Text { \"x\" }",
            "a map parameter",
        ),
        (
            "n = len(match r { Ok(t) => { t } Err(e) => { e } Cancelled => { \"\" } });",
            "Text { \"x\" }",
            "a match binding",
        ),
        (
            "n = len(id);",
            "Text { \"x\" }",
            "an event parameter (of another handler)",
        ),
        (
            "n = 1;",
            "For { over: names as: m Button { label: \"p\" + m on: Pick(m) } }",
            "a For binding as an event argument",
        ),
    ] {
        let src = component(handler, view);
        let src = if what.contains("another handler") {
            src.replace("on Go { n = len(id); }", "on Go(id) { n = len(id); }")
        } else {
            src
        };
        let (code, diags) = diagnostics(&src);
        assert_eq!(code, 0, "{what}: {diags}");
    }
}

#[test]
fn a_lambda_parameter_is_not_in_scope_outside_its_lambda() {
    let (code, diags) = diagnostics(&component(
        "n = len(filter(names, x => len(x) > 0)) + len(x);",
        "Text { \"x\" }",
    ));
    assert_eq!(code, 1, "{diags}");
    assert_eq!(diags[0]["code"], "name-unknown");
}

#[test]
fn every_example_still_checks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut seen = 0;
    for dir in std::fs::read_dir(&root).expect("examples") {
        let dir = dir.expect("an entry").path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("an example directory") {
            let f = f.expect("an entry").path();
            if f.extension().is_some_and(|e| e == "cui") && !f.to_string_lossy().contains("refused")
            {
                let (code, stdout, stderr) = run(&["check", p(&f)]);
                assert_eq!(code, 0, "{}: {stdout}{stderr}", f.display());
                seen += 1;
            }
        }
    }
    assert!(seen >= 15, "only {seen} examples");
}
