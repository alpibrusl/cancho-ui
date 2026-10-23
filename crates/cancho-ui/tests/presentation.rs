//! Presentation (`docs/presentation.md`, issue #42): a visible `label:`, `gap:` and `max_width:`, `tone:` on a status text,
//! and `Table`. What is accepted, what is refused with which tag, what the IR and the page say.

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
    let dir = std::env::temp_dir().join(format!("cancho-ui-presentation-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn program(view_body: &str) -> String {
    format!(
        "component C {{\n    state names: List[String] = [\"a\", \"b\"];\n    state tone: String = \"ok\";\n    state n: Int = 1;\n    state q: String = \"\";\n    on Pick(id) {{ q = id; }}\n    on Q(v) {{ q = v; }}\n    view {{\n        Column {{\n{view_body}\n        }}\n    }}\n}}\n"
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

fn built(view_body: &str, name: &str) -> String {
    let file = scratch(&format!("{name}.cui"), &program(view_body));
    let out = scratch(&format!("{name}.js"), "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(out).expect("the built program")
}

const TABLE: &str = "            Table {\n                label: \"Names\"\n                Row { Text { role: columnheader \"name\" } Text { role: columnheader \"pick\" } }\n                For { over: names as: r Row { Text { r } Button { label: \"Pick \" + r on: Pick(r) } } }\n            }";

#[test]
fn the_example_programs_check() {
    let (code, stdout, stderr) = run(&["check", "examples/ops-console/ops-console.cui"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn a_tone_is_a_literal_or_a_string_field() {
    accepted(
        "            Text { role: status tone: \"error\" \"x\" }",
        "tone_lit.cui",
    );
    accepted(
        "            Text { role: status tone: tone \"x\" }",
        "tone_field.cui",
    );
    accepted(
        "            Text { tone: \"warn\" \"x\" }",
        "tone_nostatus.cui",
    );
}

#[test]
fn a_bad_tone_is_refused() {
    for (i, body) in [
        "            Text { tone: \"loud\" \"x\" }",
        "            Text { tone: n \"x\" }",
        "            Text { tone: missing \"x\" }",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            codes(body, &format!("tone_bad{i}.cui")),
            ["tone-invalid"],
            "{body}"
        );
    }
    // a Button has no tone
    assert_eq!(
        codes(
            "            Button { label: \"x\" on: Q(\"a\") tone: \"ok\" }",
            "tone_button.cui"
        ),
        ["binding-unknown"]
    );
    let file = scratch(
        "tone_int.cui",
        &program("            Text { tone: 3 \"x\" }"),
    );
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
}

#[test]
fn gap_and_max_width_take_a_literal_in_their_range() {
    accepted("            Row { gap: 0 Text { \"a\" } }", "gap0.cui");
    accepted("            Row { gap: 4 Text { \"a\" } }", "gap4.cui");
    accepted(
        "            Column { max_width: 10 Text { \"a\" } }",
        "mw10.cui",
    );
    accepted(
        "            Column { max_width: 200 gap: 1 Text { \"a\" } }",
        "mw200.cui",
    );
    for (i, body) in [
        "            Row { gap: 5 Text { \"a\" } }",
        "            Column { max_width: 9 Text { \"a\" } }",
        "            Column { max_width: 201 Text { \"a\" } }",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            codes(body, &format!("layout_bad{i}.cui")),
            ["layout-value-invalid"],
            "{body}"
        );
    }
    // a name is not a literal: a parse error, the way `disabled: 3` is
    let file = scratch(
        "gap_name.cui",
        &program("            Row { gap: n Text { \"a\" } }"),
    );
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains("parse-unexpected-token"), "{stdout}");
    // a Text takes no bindings but `tone:`: the parser says so
    let file = scratch(
        "gap_text.cui",
        &program("            Text { gap: 1 \"a\" }"),
    );
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains("parse-unexpected-token"), "{stdout}");
    assert_eq!(
        codes(
            "            Button { label: \"x\" on: Q(\"a\") max_width: 20 }",
            "mw_button.cui"
        ),
        ["binding-unknown"]
    );
}

#[test]
fn a_table_holds_rows() {
    accepted(TABLE, "table_ok.cui");
    accepted(
        "            Table { Row { Text { \"a\" } \"b\" } }",
        "table_bare_string.cui",
    );
    for (i, body) in [
        "            Table { Text { \"a\" } }",
        "            Table { Button { label: \"x\" on: Q(\"a\") } }",
        "            Table { Row { Text { \"a\" } } Column { Text { \"a\" } } }",
        "            Table { For { over: names as: r Text { r } } }",
        "            Table { \"a\" }",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            codes(body, &format!("table_bad{i}.cui")),
            ["table-child-invalid"],
            "{body}"
        );
    }
}

#[test]
fn lowering_makes_rows_and_cells_of_a_table() {
    let file = scratch("table_ir.cui", &program(TABLE));
    let (code, stdout, _) = run(&["inspect", "ir", p(&file), "--output", "json"]);
    assert_eq!(code, 0, "{stdout}");
    for kind in ["\"Table\"", "\"TableRow\"", "\"Cell\"", "\"HeaderCell\""] {
        assert!(stdout.contains(kind), "{kind}: {stdout}");
    }
    // a Row outside a table is still a Row
    let file = scratch("row_ir.cui", &program("            Row { Text { \"a\" } }"));
    let (_, stdout, _) = run(&["inspect", "ir", p(&file), "--output", "json"]);
    assert!(
        !stdout.contains("TableRow") && !stdout.contains("\"Cell\""),
        "{stdout}"
    );
}

#[test]
fn the_page_has_a_real_table_with_header_cells_and_a_cell_around_a_button() {
    let js = built(TABLE, "table_js");
    assert!(js.contains("createElement('table')"), "{js}");
    assert!(js.contains("createElement('tbody')"), "{js}");
    assert!(
        js.contains("createElement('th')") && js.contains("setAttribute('scope', 'col')"),
        "{js}"
    );
    assert!(
        js.contains("createElement('tr')") && js.contains("createElement('td')"),
        "{js}"
    );
    assert!(js.contains("setAttribute('aria-label', \"Names\")"), "{js}");
    // the rows are appended to the body, not to the table
    assert!(js.contains("_body.appendChild("), "{js}");
    // a row of a text and a button is two cells: the button is not a child of the row itself
    assert_eq!(js.matches("createElement('td')").count(), 2, "{js}");
    // the button is inside a td, and still has its handler with its argument
    let td = js.rfind("createElement('td')").expect("a td");
    let button = js.rfind("createElement('button')").expect("a button");
    assert!(
        td < button,
        "the button comes after its cell is created: {js}"
    );
    assert!(js.contains("__on_Pick(__n"), "{js}");
}

#[test]
fn the_page_says_tone_gap_width_and_label() {
    let js = built(
        "            Text { role: status tone: tone \"x\" }\n            Text { tone: \"warn\" \"y\" }\n            Row { gap: 3 Text { \"a\" } }\n            Column { max_width: 60 Text { \"b\" } }\n            TextField { label: \"Search\" value: q on: Q }",
        "presentation_js",
    );
    assert!(
        js.contains("setAttribute('data-tone', String(__state_tone))"),
        "{js}"
    );
    assert!(js.contains("setAttribute('data-tone', \"warn\")"), "{js}");
    assert!(js.contains("classList.add('cui-gap-3')"), "{js}");
    assert!(
        js.contains("style.maxWidth = '60ch'") && js.contains("style.marginInline = 'auto'"),
        "{js}"
    );
    // the label is shown, in a <label> around the field, and the field keeps its aria-label
    assert!(
        js.contains("createElement('label')") && js.contains("classList.add('cui-field')"),
        "{js}"
    );
    assert!(js.contains("createTextNode(\"Search\")"), "{js}");
    assert!(
        js.contains("setAttribute('aria-label', \"Search\")"),
        "{js}"
    );
    assert!(js.contains("_field.appendChild("), "{js}");
}

#[test]
fn a_program_without_them_is_emitted_as_it_was() {
    let js = built(
        "            Text { \"a\" }\n            Row { Button { label: \"x\" on: Q(\"a\") } }",
        "plain_js",
    );
    for gone in [
        "data-tone",
        "cui-gap",
        "maxWidth",
        "createElement('label')",
        "createElement('table')",
    ] {
        assert!(!js.contains(gone), "{gone}: {js}");
    }
}

#[test]
fn a_toned_text_drops_its_tone_when_it_has_nothing_to_say() {
    // docs/presentation.md section 3 (#94): the check after the text is appended, only for a `Text` that has a `tone:`.
    let with = scratch(
        "tone.cui",
        &program("            Text { role: status tone: \"error\" \"{q}\" }"),
    );
    let out = scratch("tone.html", "");
    let (code, _, stderr) = run(&["build", p(&with), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(&out).expect("the page");
    assert!(
        js.contains(".textContent.trim()) {") && js.contains(".removeAttribute('data-tone');"),
        "{js}"
    );
    let without = scratch(
        "plain.cui",
        &program("            Text { role: status \"{q}\" }"),
    );
    let (code, _, stderr) = run(&["build", p(&without), "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let js = std::fs::read_to_string(&out).expect("the page");
    assert!(!js.contains("removeAttribute('data-tone')"), "{js}");
}

#[test]
fn the_default_stylesheet_styles_what_the_page_emits() {
    let file = scratch("css.cui", &program("            Text { \"a\" }"));
    let out = scratch("css.cui", "").with_file_name("css_out");
    let _ = std::fs::remove_dir_all(&out);
    let (code, _, stderr) = run(&["build", p(&file), "--pwa", "-o", p(&out)]);
    assert_eq!(code, 0, "{stderr}");
    let html = std::fs::read_to_string(out.join("index.html")).expect("the page");
    for rule in [
        ".cui-row {",
        ".cui-gap-4",
        ".cui-field {",
        ".cui-table {",
        "[data-tone=\"ok\"]::before",
        "[data-tone=\"warn\"]::before",
        "[data-tone=\"error\"]::before",
        "prefers-color-scheme: dark",
    ] {
        assert!(html.contains(rule), "{rule}");
    }
}

#[test]
fn replay_runs_a_program_with_a_table_and_a_tone() {
    let file = scratch("replay.cui", &program(TABLE));
    let trace = scratch(
        "replay.trace.json",
        r#"{"schema":"cancho-ui-trace","version":"0.1.0","component":"C","steps":[{"event":"Pick","params":["b"],"state":{"q":"b"}}]}"#,
    );
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
}

#[test]
fn the_diff_shows_a_changed_tone() {
    let base = scratch(
        "diff_base.cui",
        &program("            Text { role: status tone: \"ok\" \"x\" }"),
    );
    let cand = scratch(
        "diff_cand.cui",
        &program("            Text { role: status tone: \"error\" \"x\" }"),
    );
    let (code, stdout, _) = run(&["diff", p(&base), p(&cand)]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("tone="), "{stdout}");
}
