//! Row actions (`docs/row-actions.md`, issue #41): a `Button` label that is an expression and `on: Event(args)`. What is
//! accepted, what is refused with which tag, what is emitted, what the IR and the diff say, and a trace.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-rows-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component with `view_body` as its view and the handlers `Pick(id, pos)`, `Plain` and `Change(v)`.
fn program(view_body: &str) -> String {
    format!(
        "component C {{\n    state names: List[String] = [\"a\", \"b\"];\n    state picked: String = \"\";\n    on Pick(id, pos) {{ picked = id + pos; }}\n    on Plain {{ picked = \"\"; }}\n    on Change(v) {{ picked = v; }}\n    view {{\n        Column {{\n{view_body}\n        }}\n    }}\n}}\n"
    )
}

fn codes(view_body: &str, name: &str) -> Vec<String> {
    let file = scratch(name, &program(view_body));
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{view_body}: {stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

fn accepted(view_body: &str, name: &str) {
    let file = scratch(name, &program(view_body));
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{view_body}: {stdout}{stderr}");
}

#[test]
fn the_example_checks_and_its_trace_passes() {
    let (code, stdout, stderr) = run(&["check", "examples/rows/rows.cui"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let (code, stdout, stderr) = run(&[
        "test",
        "examples/rows/rows.cui",
        "examples/rows/rows.trace.json",
    ]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_button_in_a_for_takes_an_expression_label_and_event_arguments() {
    accepted(
        "            For { over: names as: n Button { label: n on: Pick(n, 1) } }",
        "ok_ident.cui",
    );
    accepted(
        "            For { over: names as: n Button { label: \"Pick \" + n on: Pick(n, len(names) - 1) } }",
        "ok_concat.cui",
    );
    accepted(
        "            Button { label: \"Pick \" + picked on: Pick(picked, 2) }",
        "ok_outside_for.cui",
    );
    accepted(
        "            Button { label: \"Plain\" on: Pick(\"a\", \"b\") }",
        "ok_literals.cui",
    );
    // a literal label and a bare event are what they were
    accepted(
        "            Button { label: \"Go\" on: Plain }",
        "ok_bare.cui",
    );
}

#[test]
fn an_event_that_does_not_exist_is_refused_for_every_form() {
    for (i, body) in [
        "            Button { label: \"x\" on: Missing }",
        "            Button { label: \"x\" on: Missing(1) }",
        "            TextField { label: \"x\" value: picked on: Missing }",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            codes(body, &format!("unknown{i}.cui")),
            ["event-unknown"],
            "{body}"
        );
    }
}

#[test]
fn the_arguments_must_be_as_many_as_the_handler_has_parameters() {
    for (i, body) in [
        "            Button { label: \"x\" on: Pick(\"a\") }",
        "            Button { label: \"x\" on: Pick(\"a\", \"b\", \"c\") }",
        "            Button { label: \"x\" on: Plain(1) }",
        // a click has no arguments, so a handler that wants some cannot be named bare (it used to receive the
        // MouseEvent in its first parameter)
        "            Button { label: \"x\" on: Pick }",
        "            Button { label: \"x\" on: Change }",
        // a field gives the typed value, one argument
        "            TextField { label: \"x\" value: picked on: Pick }",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            codes(body, &format!("arity{i}.cui")),
            ["event-arity-mismatch"],
            "{body}"
        );
    }
}

#[test]
fn arguments_belong_to_a_button_only() {
    for (i, body) in [
        "            TextField { label: \"x\" value: picked on: Change(1) }",
        "            Column { on: Plain(1) }",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            codes(body, &format!("args{i}.cui")),
            ["event-args-unsupported"],
            "{body}"
        );
    }
}

#[test]
fn arguments_are_not_taken_by_submit() {
    let body = "            TextField { label: \"x\" value: picked on: Change submit: Plain(1) }";
    let file = scratch("submit_args.cui", &program(body));
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains("parse-unexpected-token"), "{stdout}");
}

#[test]
fn an_instance_takes_neither_an_expression_label_nor_event_arguments() {
    let src = "component Card {\n    state title: String = \"\";\n    view { Text { \"{title}\" } }\n}\n\ncomponent Page {\n    state names: List[String] = [\"a\"];\n    on Pick(id) { names = [id]; }\n    view { Column { For { over: names as: n Card { label: n } } } }\n}\n";
    let file = scratch("instance_label.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    let src = src.replace("Card { label: n }", "Card { on: Pick(n) }");
    let file = scratch("instance_on.cui", &src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
}

#[test]
fn a_label_that_is_not_an_expression_or_a_string_is_a_parse_error() {
    for (i, body) in [
        "            Button { label: on: Plain }",
        "            Button { label: }",
        "            Button { label: [1] on: Plain }",
    ]
    .iter()
    .enumerate()
    {
        let file = scratch(&format!("badlabel{i}.cui"), &program(body));
        let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
        assert_eq!(code, 1, "{body}: {stdout}");
        assert!(
            stdout.contains("parse-unexpected-token"),
            "{body}: {stdout}"
        );
    }
}

#[test]
fn a_literal_label_still_refuses_a_brace() {
    // The literal path is unchanged: a `{` in static text is refused (docs/language-design.md decision 15).
    let file = scratch(
        "brace.cui",
        &program("            Button { label: \"a{b}\" on: Plain }"),
    );
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
}

fn built(view_body: &str, name: &str) -> String {
    let file = scratch(&format!("{name}.cui"), &program(view_body));
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

#[test]
fn the_emitted_page_evaluates_the_label_and_the_arguments_at_render_time() {
    let js = built(
        "            For { over: names as: n Button { label: \"Pick \" + n on: Pick(n, 7) } }",
        "emit",
    );
    assert!(
        js.contains("_label = String(__cui_add(\"Pick \", n));"),
        "{js}"
    );
    assert!(js.contains(".setAttribute('aria-label', __n"), "{js}");
    assert!(js.contains("_arg0 = n;"), "{js}");
    assert!(js.contains("_arg1 = 7;"), "{js}");
    assert!(
        js.contains("addEventListener('click', function () { __on_Pick(__n"),
        "{js}"
    );
    // the visible text is the same constant
    assert!(js.contains("document.createTextNode(__n"), "{js}");
    // a literal label is emitted as it was: no constant, no closure
    let plain = built(
        "            Button { label: \"Go\" on: Plain }",
        "emit_plain",
    );
    assert!(!plain.contains("_label ="), "{plain}");
    assert!(
        plain.contains("addEventListener('click', __on_Plain)"),
        "{plain}"
    );
}

#[test]
fn the_ir_has_a_label_expr_only_for_a_label_that_is_an_expression() {
    let file = scratch(
        "ir_expr.cui",
        &program("            For { over: names as: n Button { label: n on: Pick(n, 1) } }"),
    );
    let (code, stdout, _) = run(&["inspect", "ir", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("\"label_expr\""), "{stdout}");
    assert!(stdout.contains("\"event\": \"Pick\""), "{stdout}");
    assert!(stdout.contains("\"args\""), "{stdout}");
    let file = scratch(
        "ir_literal.cui",
        &program("            Button { label: \"Go\" on: Plain }"),
    );
    let (code, stdout, _) = run(&["inspect", "ir", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(
        !stdout.contains("label_expr") && !stdout.contains("\"event\""),
        "a program that uses neither has the IR it had: {stdout}"
    );
}

#[test]
fn the_diff_shows_a_changed_argument() {
    let base = scratch(
        "diff_base.cui",
        &program("            For { over: names as: n Button { label: n on: Pick(n, 1) } }"),
    );
    let cand = scratch(
        "diff_cand.cui",
        &program("            For { over: names as: n Button { label: n on: Pick(n, 2) } }"),
    );
    let (code, stdout, _) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{stdout}");
    assert!(
        stdout.contains("Pick(n, 1)") && stdout.contains("Pick(n, 2)"),
        "{stdout}"
    );
}

#[test]
fn replay_counts_a_button_with_arguments_as_a_way_to_fire_the_event() {
    // `disabled: sending` on a row button disables the event in replay as it does in the browser.
    let src = "component C {\n    state names: List[String] = [\"a\"];\n    state sending: Bool = true;\n    state picked: String = \"\";\n    on Pick(id) { picked = id; }\n    view { Column { For { over: names as: n Button { label: n on: Pick(n) disabled: sending } } } }\n}\n";
    let file = scratch("disabled_row.cui", src);
    let trace = scratch(
        "disabled_row.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Pick","params":["a"],"state":{"picked":""}}]}"#,
    );
    let (code, stdout, _) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("disabled"), "{stdout}");
}
