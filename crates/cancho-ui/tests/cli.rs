//! The documented command lines must work as documented.

use std::path::Path;
use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn inspect_effects_takes_the_output_flag_after_the_name() {
    let (code, stdout) = run(&[
        "inspect",
        "effects",
        "examples/clock/clock.cui",
        "Clock",
        "--output",
        "json",
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("\"capability\": \"Clock\""), "{stdout}");
}

#[test]
fn the_project_page_demo_is_what_the_compiler_emits() {
    // docs/demo/counter.js is served on the project page; it must be the
    // compiler's current output for examples/counter, not a stale copy.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::env::temp_dir().join("cancho-ui-demo-counter.js");
    let (code, stdout) = run(&[
        "build",
        "examples/counter/counter.cui",
        "-o",
        out.to_str().expect("temp path is UTF-8"),
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    let fresh = std::fs::read_to_string(&out).expect("build wrote the file");
    let committed =
        std::fs::read_to_string(root.join("docs/demo/counter.js")).expect("demo is committed");
    assert_eq!(
        fresh, committed,
        "docs/demo/counter.js is stale: rebuild it with `cancho-ui build examples/counter/counter.cui -o docs/demo/counter.js`"
    );
}

#[test]
fn a_button_with_a_label_shows_the_label_as_its_text() {
    // `Button { label: "Increment" }` used to compile to an `aria-label` and an empty text node: a button with
    // nothing written on it.
    let dir = std::env::temp_dir().join("cancho-ui-button-label");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let source = dir.join("labelled.cui");
    std::fs::write(
        &source,
        "component Labelled {\n    state n: Int = 0;\n    on Tap { n = n + 1; }\n    view {\n        Column {\n            Button {\n                role: button\n                label: \"Tap me\"\n                on: Tap\n            }\n            Button {\n                label: \"Not shown\"\n                on: Tap\n                Text { \"Child text\" }\n            }\n        }\n    }\n}\n",
    )
    .expect("a source file");
    let out = dir.join("labelled.js");
    let (code, stdout) = run(&[
        "build",
        source.to_str().expect("temp path is UTF-8"),
        "-o",
        out.to_str().expect("temp path is UTF-8"),
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    let js = std::fs::read_to_string(&out).expect("build wrote the file");
    assert!(
        js.contains("createTextNode(\"Tap me\")"),
        "the label is the button's text:\n{js}"
    );
    assert!(
        js.contains("setAttribute('aria-label', \"Tap me\")"),
        "and stays its accessible name:\n{js}"
    );
    assert!(
        !js.contains("createTextNode(\"Not shown\")"),
        "a button with children of its own shows those, not its label:\n{js}"
    );
    assert!(js.contains("createTextNode(\"Child text\")"), "{js}");
}

#[test]
fn a_flag_with_a_value_does_not_swallow_the_file_after_it() {
    // `build -o out.js file.cui` used to fail with "requires at least one file": each flag that takes a value
    // advanced past the next argument twice.
    let out = std::env::temp_dir().join(format!("cancho-ui-flag-order-{}.js", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let (code, stdout) = run(&[
        "build",
        "-o",
        out.to_str().expect("temp path is UTF-8"),
        "--target",
        "web",
        "examples/counter/counter.cui",
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(out.exists(), "the build wrote the file named by -o");
    // The same for the commands whose only flag is `--output`.
    for command in ["check", "report"] {
        let (code, stdout) = run(&[command, "--output", "json", "examples/counter/counter.cui"]);
        assert_eq!(code, 0, "`{command}` with the flag first: {stdout}");
    }
}

#[test]
fn focus_survives_a_rerender_in_every_program_with_a_focusable_control() {
    // The view is rebuilt on every state change, which dropped focus after each keystroke in a TextField and after
    // each Enter on a Button (measured in a browser; `scripts/check-focus.js`). The helper is emitted for a program
    // with a Button or a TextField and for no other, so a program without one is unchanged.
    let build = |example: &str| {
        let out = std::env::temp_dir().join(format!(
            "cancho-ui-focus-{}-{}.js",
            std::process::id(),
            example.replace('/', "-")
        ));
        let (code, stdout) = run(&["build", example, "-o", out.to_str().expect("UTF-8 path")]);
        assert_eq!(code, 0, "stdout: {stdout}");
        std::fs::read_to_string(&out).expect("build wrote the file")
    };
    for example in ["examples/counter/counter.cui", "examples/search/search.cui"] {
        let js = build(example);
        assert!(
            js.contains("function __cui_focus_save(root)"),
            "{example}: the helper"
        );
        assert!(
            js.contains("const __focus = __cui_focus_save(root);"),
            "{example}: saved before the rebuild"
        );
        let save = js
            .find("const __focus = __cui_focus_save(root);")
            .expect("save");
        let clear = js.find("root.textContent = '';").expect("rebuild");
        let restore = js
            .find("__cui_focus_restore(root, __focus);")
            .expect("restore");
        assert!(
            save < clear && clear < restore,
            "{example}: save, then rebuild, then restore"
        );
    }
    let hello = build("examples/hello/hello.cui");
    assert!(
        !hello.contains("__cui_focus"),
        "a program with nothing to focus is unchanged:\n{hello}"
    );
}

/// Run in a scratch directory, so a command that wrongly writes a file named
/// after a flag leaves evidence there rather than in the repository.
fn run_in_scratch(name: &str, args: &[&str]) -> (i32, String, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("cancho-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let out = Command::new(env!("CARGO_BIN_EXE_cancho-ui"))
        .args(args)
        .current_dir(&dir)
        .output()
        .expect("the cancho-ui binary runs");
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        dir,
    )
}

#[test]
fn introspect_output_json_prints_json_and_writes_no_file() {
    let (code, stdout, dir) =
        run_in_scratch("introspect-flag", &["introspect", "--output", "json"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(v["schema"], "cancho-ui-cli");
    let entries: Vec<_> = std::fs::read_dir(&dir).expect("scratch dir").collect();
    assert!(
        entries.is_empty(),
        "introspect --output json wrote files: {entries:?}"
    );
}

#[test]
fn skill_output_json_prints_json_and_writes_no_file() {
    let (code, stdout, dir) = run_in_scratch("skill-flag", &["skill", "--output", "json"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(v["name"], "cancho-ui");
    assert!(
        v["content"]
            .as_str()
            .is_some_and(|c| c.contains("# cancho-ui"))
    );
    assert!(
        std::fs::read_dir(&dir)
            .expect("scratch dir")
            .next()
            .is_none()
    );
}

#[test]
fn a_bare_path_is_the_output_file_for_skill_and_introspect() {
    for cmd in ["skill", "introspect"] {
        let (code, _, dir) = run_in_scratch(&format!("{cmd}-path"), &[cmd, "out.txt"]);
        assert_eq!(code, 0);
        let written = std::fs::read_to_string(dir.join("out.txt")).expect("file written");
        assert!(!written.is_empty());
    }
    // The flag and the path together: the flag takes `json`, the path is the file.
    let (code, stdout, dir) = run_in_scratch(
        "introspect-both",
        &["introspect", "--output", "json", "o.json"],
    );
    assert_eq!(code, 0);
    assert!(
        stdout.is_empty(),
        "nothing on stdout when a file is named: {stdout}"
    );
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("o.json")).expect("file"))
            .expect("file is JSON");
    assert_eq!(v["schema"], "cancho-ui-cli");
    assert!(!dir.join("--output").exists() && !dir.join("json").exists());
}

#[test]
fn skill_and_introspect_refuse_bad_flags_with_exit_2() {
    for args in [
        &["introspect", "--bogus"][..],
        &["skill", "--output"][..],
        &["skill", "--output", "xml"][..],
        &["introspect", "a", "b"][..],
    ] {
        let (code, _, dir) = run_in_scratch("selfdescribe-bad", args);
        assert_eq!(code, 2, "{args:?}");
        assert!(
            std::fs::read_dir(&dir)
                .expect("scratch dir")
                .next()
                .is_none(),
            "{args:?}"
        );
    }
}
