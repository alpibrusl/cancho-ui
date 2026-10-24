//! `location.current()` (`docs/adapters.md` section 11): what is emitted, what is refused, how a trace scripts it.
//! The browser-side checks (rounding, permission, no API, errors) are `scripts/check-location.js`.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const WHERE: &str = include_str!("../../../examples/where/where.cui");

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
    let dir = common::scratch_root().join(format!("cancho-ui-location-{}", std::process::id()));
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

/// The example with its declaration replaced.
fn with_requires(requires: &str) -> String {
    let src = WHERE.replace("\n    requires Location;\n", &format!("\n    {requires}\n"));
    assert_ne!(src, WHERE, "the declaration was replaced");
    src
}

#[test]
fn the_adapter_is_emitted_only_for_a_program_that_uses_location() {
    let js = build_js(WHERE, "where.cui");
    assert!(js.contains("function __cui_location_current(digits)"));
    assert!(
        js.contains("__cui_location_current(__cui_location_digits)"),
        "{js}"
    );
    assert_eq!(
        js.matches("navigator.geolocation").count(),
        2,
        "one guard and one call"
    );
    let counter = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter/counter.cui"),
    )
    .expect("the counter example");
    assert!(!build_js(&counter, "counter.cui").contains("geolocation"));
}

#[test]
fn the_default_is_two_decimals_and_fine_is_five_and_each_component_keeps_its_own() {
    assert!(build_js(WHERE, "bare.cui").contains("const __cui_location_digits = 2;"));
    assert!(
        build_js(
            &with_requires("requires Location(\"coarse\");"),
            "coarse.cui"
        )
        .contains("const __cui_location_digits = 2;")
    );
    assert!(
        build_js(&with_requires("requires Location(\"fine\");"), "fine.cui")
            .contains("const __cui_location_digits = 5;")
    );
    let two = format!(
        "{}\n{}",
        with_requires("requires Location(\"fine\");"),
        WHERE.replace("component Where", "component Elsewhere")
    );
    let js = build_js(&two, "two.cui");
    assert_eq!(
        js.matches("const __cui_location_digits = 5;").count(),
        1,
        "{js}"
    );
    assert_eq!(
        js.matches("const __cui_location_digits = 2;").count(),
        1,
        "{js}"
    );
}

#[test]
fn a_location_bound_other_than_coarse_or_fine_is_narrowing_illegal() {
    for requires in [
        "requires Location(\"exact\");",
        "requires Location(\"\");",
        "requires Location(\"fine\", \"coarse\");",
        "requires Location(\"Fine\");",
    ] {
        assert_eq!(
            check_codes(&with_requires(requires), "bound.cui"),
            ["capability-narrowing-illegal"],
            "{requires}"
        );
    }
}

#[test]
fn location_without_a_declaration_is_missing_and_arguments_are_refused() {
    let missing = WHERE.replace("\n    requires Location;\n", "\n");
    assert_eq!(check_codes(&missing, "missing.cui"), ["capability-missing"]);
    let args = WHERE.replace("location.current()", "location.current(1)");
    assert_eq!(check_codes(&args, "args.cui"), ["builtin-shape-mismatch"]);
}

#[test]
fn the_example_trace_passes_and_the_effect_is_its_own_cancellation_group() {
    let (code, stdout, stderr) = run(&[
        "test",
        "examples/where/where.cui",
        "examples/where/where.trace.json",
    ]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("issued #1 location.current()"), "{stdout}");
    let src = "component A {\n    requires Location;\n    requires Network;\n    state a: Result[String, String] = Ok(\"\");\n    state b: Result[String, String] = Ok(\"\");\n    on Where { a = await(location.current()); }\n    on Get { b = await(net.get(\"u\")); }\n    view { Text { \"x\" } }\n}\n";
    let file = scratch("groups.cui", src);
    let trace = scratch(
        "groups.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","steps":[
            {"event":"Where","expect":{"pending":[1]}},
            {"event":"Get","expect":{"pending":[1,2]}},
            {"resolve":1,"with":{"Ok":"1.00,2.00"},"state":{"a":{"Ok":"1.00,2.00"}}},
            {"resolve":2,"with":{"Ok":"y"},"state":{"b":{"Ok":"y"}}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}
