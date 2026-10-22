//! No lowering refusal is a panic ("no input may reach a panic"): one program for each variant of `LowerError`, through
//! `check`, which turns each into a tagged diagnostic and exits 1. The conversion in `cancho-ui-types` has no wildcard
//! arm, so a variant added without a conversion does not compile; this test is the other half, that each existing variant
//! is reached by real input and answers with its rule. (`EmptySpan` takes a syntax tree no source produces: its test is in
//! `crates/cancho-ui-types/src/check.rs`.)

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-paths-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

const NET: &str = "state r: Result[String, String] = Ok(\"\");";

/// (the `LowerError` variant, a program that reaches it, the rule it must answer with)
fn cases() -> Vec<(&'static str, String, &'static str)> {
    let comp = |header: &str, body: &str, view: &str| {
        format!(
            "component C {{\n    {header}\n    state n: Int = 0;\n    state w: String = \"\";\n    state b: Bool = true;\n    state names: List[String] = [\"a\"];\n    {NET}\n    {body}\n    view {{ {view} }}\n}}\n"
        )
    };
    let text = "Text { \"x\" }";
    vec![
        (
            "CapabilityMissing",
            comp("", "on Go { r = await(net.get(\"/x\")); }", text),
            "capability-missing",
        ),
        (
            "CapabilityUnused",
            comp("requires Network;", "on Go { n = 1; }", text),
            "capability-unused",
        ),
        (
            "ScheduleEventUnknown",
            comp("start Nope;", "on Go { n = 1; }", text),
            "schedule-event-unknown",
        ),
        (
            "ScheduleEventParams",
            comp("start Go;", "on Go(v) { n = len(v); }", text),
            "schedule-event-params",
        ),
        (
            "SchedulePeriodInvalid",
            comp("every 5 Go;", "on Go { n = 1; }", text),
            "schedule-period-invalid",
        ),
        (
            "ScheduleDuplicate",
            comp("start Go;\n    start Go;", "on Go { n = 1; }", text),
            "schedule-duplicate",
        ),
        (
            "NarrowingIllegal",
            comp(
                "requires Network;\n    requires Credential;",
                "on Go { r = await(net.get(\"/x\")); }",
                text,
            ),
            "capability-narrowing-illegal",
        ),
        (
            "CredentialMisuse",
            comp(
                "requires Network;\n    requires Credential(\"a\");\n    requires Credential(\"b\");",
                "on Go { r = await(net.get(\"/x\")); }",
                text,
            ),
            "credential-ambiguous",
        ),
        (
            "BoundViolated",
            comp(
                "requires Network(\"a.example\");",
                "on Go { r = await(net.get(\"https://b.example/x\")); }",
                text,
            ),
            "capability-bound-violated",
        ),
        (
            "MatchNonExhaustive",
            comp(
                "",
                "on Go { r = await(sleep(1)); w = match r { Ok(t) => { \"a\" } Err(e) => { \"b\" } }; }",
                text,
            ),
            "match-non-exhaustive",
        ),
        (
            "LambdaUnsupported",
            comp("", "on Go { n = len(rotate(names, x => x)); }", text),
            "lambda-unsupported",
        ),
        (
            "PatternLiteralUnsupported",
            comp(
                "",
                "on Go { n = len(match b { true => { \"y\" } false => { \"n\" } }); }",
                text,
            ),
            "pattern-literal-unsupported",
        ),
        (
            "ConditionRefused",
            comp("", "on Go { n = if b { 1 }; }", text),
            "if-else-required",
        ),
        (
            "ControlRefused",
            comp(
                "",
                "on Go { n = 1; }",
                "Button { label: \"x\" on: Go disabled: n }",
            ),
            "disabled-not-bool",
        ),
        (
            "UnknownCall",
            comp("", "on Go { n = sqrt(4); }", text),
            "call-unknown",
        ),
        (
            "BuiltinMisuse",
            comp("", "on Go { n = min(1); }", text),
            "builtin-shape-mismatch",
        ),
        (
            "PureLambdaRequired",
            comp(
                "",
                "on Go { n = len(filter(names, x => len(await(sleep(1))) > 0)); }",
                text,
            ),
            "pure-lambda-required",
        ),
    ]
}

#[test]
fn every_lowering_refusal_is_a_tagged_diagnostic_not_a_panic() {
    let cases = cases();
    assert_eq!(
        cases.len(),
        17,
        "one case per reachable variant of LowerError"
    );
    for (variant, src, rule) in cases {
        let file = scratch(&format!("{variant}.cui"), &src);
        let (code, stdout, stderr) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{variant} (exit 101 is a panic): {stdout}{stderr}");
        assert!(!stderr.contains("panicked"), "{variant}: {stderr}");
        let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
        let codes: Vec<&str> = diags
            .as_array()
            .expect("a list")
            .iter()
            .map(|d| d["code"].as_str().expect("a code"))
            .collect();
        assert_eq!(codes, [rule], "{variant}");
        // build and test refuse it the same way
        let out = scratch(&format!("{variant}.js"), "");
        let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
        assert_eq!(code, 1, "{variant} build: {stderr}");
        assert!(stderr.contains(rule), "{variant} build: {stderr}");
    }
}
