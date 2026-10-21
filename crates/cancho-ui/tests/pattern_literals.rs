//! A `match` arm `true` or `false` (`pattern-literal-unsupported`, issue #26): a bare name in a pattern binds the value
//! and matches everything, so the arm that reads like a literal used to always win. It is refused now, by every command
//! that checks a program: `check`, `build` and `test`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const PROGRAM: &str = "component Flag {\n    state flag: Bool = false;\n    state label: String = \"-\";\n    on Go { label = match flag { true => { \"yes\" } false => { \"no\" } }; }\n    view { Text { \"{label}\" } }\n}\n";

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-pattern-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn diagnostics(src: &str, name: &str) -> Vec<Value> {
    let file = scratch(name, src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    serde_json::from_str::<Value>(&stdout)
        .expect("diagnostics are JSON")
        .as_array()
        .expect("a list")
        .clone()
}

#[test]
fn true_and_false_arms_are_refused_with_a_rule_and_a_remediation() {
    for (src, name) in [
        (PROGRAM.to_string(), "true"),
        (PROGRAM.replace("true =>", "yes =>"), "false"),
        (PROGRAM.replace("false =>", "no =>"), "true"),
    ] {
        let diags = diagnostics(&src, "lit.cui");
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0]["code"], "pattern-literal-unsupported");
        let message = diags[0]["message"].as_str().expect("a message");
        assert!(message.contains(&format!("`{name}`")), "{name}: {message}");
        let remediation = diags[0]["remediation"]["description"]
            .as_str()
            .expect("a remediation");
        assert!(remediation.contains("no literal patterns"), "{remediation}");
        assert!(diags[0]["span"]["start"]["line"].as_u64().is_some());
    }
}

#[test]
fn names_and_the_wildcard_are_still_patterns() {
    for arms in [
        "match flag { yes => { \"a\" } }",
        "match flag { _ => { \"a\" } }",
        "match flag { truthy => { \"a\" } }",
    ] {
        let src = PROGRAM.replace(
            "match flag { true => { \"yes\" } false => { \"no\" } }",
            arms,
        );
        assert_ne!(src, PROGRAM);
        let file = scratch("ok.cui", &src);
        let (code, stdout, stderr) = run(&["check", p(&file)]);
        assert_eq!(code, 0, "{arms}: {stdout}{stderr}");
    }
}

#[test]
fn build_and_test_refuse_it_too_and_emit_nothing() {
    let file = scratch("build.cui", PROGRAM);
    let out = scratch("build.js", "");
    std::fs::remove_file(&out).expect("no output yet");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("pattern-literal-unsupported"), "{stderr}");
    assert!(!out.exists(), "nothing was written");

    let trace = scratch(
        "build.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"Flag","steps":[{"event":"Go","state":{"label":"no"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 1, "{stdout}{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains("pattern-literal-unsupported"),
        "{stdout}{stderr}"
    );
}
