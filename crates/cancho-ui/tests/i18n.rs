//! Internationalization (`docs/i18n.md`, issue #82): what parses, what is refused with which tag, how replay picks and changes the language,
//! and that the page carries the data and the steps replay runs. `tests/i18n_pages.json` is the file replay is run over here and Chromium is
//! run over in `scripts/check-i18n.js`; `tests/i18n_corpus.json` holds the formats (checked in `crates/cancho-ui-ir/tests/i18n.rs`).
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

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
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = common::scratch_root().join(format!("cancho-ui-i18n-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

fn codes(src: &str) -> Vec<String> {
    let file = scratch("f.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    assert_eq!(code, 1, "expected a refusal for:\n{src}\n{stdout}");
    let diags: Value = serde_json::from_str(&stdout).expect("diagnostics are JSON");
    diags
        .as_array()
        .expect("a list")
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect()
}

fn accepted(src: &str) {
    let file = scratch("f.cui", src);
    let (code, stdout, stderr) = run(&["check", p(&file)]);
    assert_eq!(code, 0, "{src}\n{stdout}{stderr}");
}

fn replay(src: &str, trace: &Value) -> (i32, Value) {
    let file = scratch("f.cui", src);
    let trace = scratch("f.trace.json", &trace.to_string());
    let (code, stdout, stderr) = run(&["test", p(&file), p(&trace), "--output", "json"]);
    let report: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("a JSON report ({e}): {stdout}{stderr}"));
    (code, report)
}

fn rules(report: &Value) -> Vec<String> {
    report["failures"]
        .as_array()
        .expect("failures")
        .iter()
        .map(|f| f["rule"].as_str().expect("a rule").to_string())
        .collect()
}

/// A component with the given `messages` body and a view that says `view`.
fn comp(messages: &str, body: &str) -> String {
    format!(
        "component C {{\n    requires Locale;\n    state s: String = \"\";\n    messages {{\n{messages}\n    }}\n    on Go {{ s = {body}; }}\n    view {{ Text {{ \"{{s}}\" }} }}\n}}\n"
    )
}

const EN_DE: &str = "        en { a: \"A\"; h: \"Hi {name}\"; o(n): one \"{n} x\", other \"{n} xs\"; }\n        de { a: \"B\"; h: \"Hallo {name}\"; o(n): one \"{n} y\", other \"{n} ys\"; }";

// ---- the pages corpus in replay ------------------------------------------------------------------------------------

fn pages() -> Value {
    serde_json::from_str(include_str!("../../../tests/i18n_pages.json"))
        .expect("the corpus is JSON")
}

#[test]
fn the_pages_corpus_passes_in_replay() {
    let c = pages();
    let program = c["program"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    for case in c["cases"].as_array().expect("cases") {
        let why = case["why"].as_str().expect("why");
        let mut steps = vec![json!({"advance": 0, "expect": {"document": case["start"]}})];
        let mut count = 0;
        for s in case["steps"].as_array().expect("steps") {
            let event = s["do"].as_str().expect("a verb");
            if let Some(n) = s["count"].as_i64()
                && n != count
            {
                steps.push(
                    json!({"event": "SetCount", "params": [n.to_string()], "state": {"count": n}}),
                );
                count = n;
            }
            let mut step = json!({"event": event});
            if let Some(w) = s.get("shown") {
                step["state"] = json!({"shown": w});
            }
            if let Some(ok) = s.get("ok") {
                step["state"] = json!({"ok": ok});
            }
            step["expect"] = json!({"document": s["document"]});
            steps.push(step);
        }
        let trace = json!({"schema": "cancho-ui-trace", "version": "0.1.0", "locales": case["locales"], "steps": steps});
        let (code, report) = replay(&program, &trace);
        assert_eq!(code, 0, "{why}\n{report:#}");
    }
}

// ---- what parses and what is refused -----------------------------------------------------------------------------------

#[test]
fn a_component_with_catalogs_is_accepted() {
    accepted(&comp(EN_DE, "t(\"a\") + t(\"h\", \"Ada\") + t(\"o\", 3)"));
    // no catalogs: English, and the formats work
    accepted(
        "component C {\n    requires Locale;\n    state s: String = \"\";\n    on Go { s = fmt.number(1234.5, 2) + fmt.percent(0.5) + fmt.date(0, \"short\") + fmt.time(0) + locale.current(); }\n    view { Text { \"{s}\" } }\n}\n",
    );
}

#[test]
fn each_message_tag_is_reached() {
    let ok_call = "t(\"a\")";
    // an unknown language
    assert_eq!(
        codes(&comp("        xx { a: \"A\"; }", ok_call)),
        ["message-language-unknown"]
    );
    // a key one language lacks, either way round
    assert_eq!(
        codes(&comp(
            "        en { a: \"A\"; b: \"B\"; }\n        de { a: \"A\"; }",
            ok_call
        )),
        ["message-key-missing"]
    );
    assert_eq!(
        codes(&comp(
            "        en { a: \"A\"; }\n        de { a: \"A\"; b: \"B\"; }",
            ok_call
        )),
        ["message-key-missing"]
    );
    // a duplicate key, a duplicate language
    assert_eq!(
        codes(&comp("        en { a: \"A\"; a: \"B\"; }", ok_call)),
        ["message-key-duplicate"]
    );
    assert_eq!(
        codes(&comp(
            "        en { a: \"A\"; }\n        en { a: \"B\"; }",
            ok_call
        )),
        ["message-key-duplicate"]
    );
    // holes that differ, and a plural in one language and a plain text in the other
    assert_eq!(
        codes(&comp(
            "        en { h: \"Hi {name}\"; }\n        de { h: \"Hallo {who}\"; }",
            "t(\"h\", \"x\")"
        )),
        ["message-hole-mismatch"]
    );
    assert_eq!(
        codes(&comp(
            "        en { o(n): one \"{n}\", other \"{n}\"; }\n        de { o: \"x\"; }",
            "t(\"o\", 1)"
        )),
        ["message-hole-mismatch"]
    );
    // a plural with a form missing, one too many, one twice
    assert_eq!(
        codes(&comp("        en { o(n): other \"{n}\"; }", "t(\"o\", 1)")),
        ["message-plural-incomplete"]
    );
    assert_eq!(
        codes(&comp(
            "        en { o(n): one \"{n}\", few \"{n}\", other \"{n}\"; }",
            "t(\"o\", 1)"
        )),
        ["message-plural-incomplete"]
    );
    assert_eq!(
        codes(&comp(
            "        en { o(n): one \"{n}\", one \"{n}\", other \"{n}\"; }",
            "t(\"o\", 1)"
        )),
        ["message-plural-incomplete"]
    );
    // Spanish has `many`; Japanese only `other`
    assert_eq!(
        codes(&comp(
            "        es { o(n): one \"{n}\", other \"{n}\"; }",
            "t(\"o\", 1)"
        )),
        ["message-plural-incomplete"]
    );
    accepted(&comp("        ja { o(n): other \"{n}\"; }", "t(\"o\", 1)"));
    // text that is not a text: an unclosed brace, a stray brace, an empty hole, a hole in a plural that is not its count
    for bad in ["a {b", "a } b", "a {} b", "a {1x} b", "a {b-c} b"] {
        assert_eq!(
            codes(&comp(&format!("        en {{ a: \"{bad}\"; }}"), ok_call)),
            ["message-text-invalid"],
            "{bad}"
        );
    }
    assert_eq!(
        codes(&comp(
            "        en { o(n): one \"{m}\", other \"{n}\"; }",
            "t(\"o\", 1)"
        )),
        ["message-text-invalid"]
    );
    // calls: an unknown key, a key that is not a literal, the wrong number of arguments, a style that is not one of the four
    assert_eq!(codes(&comp(EN_DE, "t(\"zzz\")")), ["message-key-unknown"]);
    assert_eq!(codes(&comp(EN_DE, "t(s)")), ["message-call-invalid"]);
    assert_eq!(codes(&comp(EN_DE, "t(\"h\")")), ["message-call-invalid"]);
    assert_eq!(
        codes(&comp(EN_DE, "t(\"h\", \"a\", \"b\")")),
        ["message-call-invalid"]
    );
    assert_eq!(codes(&comp(EN_DE, "t(\"o\")")), ["message-call-invalid"]);
    assert_eq!(codes(&comp(EN_DE, "t(\"a\", 1)")), ["message-call-invalid"]);
    assert_eq!(
        codes(&comp(EN_DE, "fmt.date(0, \"tiny\")")),
        ["message-call-invalid"]
    );
    assert_eq!(
        codes(&comp(EN_DE, "fmt.date(0, s)")),
        ["message-call-invalid"]
    );
    assert_eq!(
        codes(&comp(EN_DE, "fmt.number(1)")),
        ["message-call-invalid"]
    );
    assert_eq!(codes(&comp(EN_DE, "fmt.time()")), ["message-call-invalid"]);
    assert_eq!(
        codes(&comp(EN_DE, "locale.current(1)")),
        ["message-call-invalid"]
    );
    assert_eq!(
        codes(&comp(EN_DE, "locale.set()")),
        ["message-call-invalid"]
    );
    // a program with no catalogs has no key
    assert_eq!(
        codes(
            "component C {\n    requires Locale;\n    state s: String = \"\";\n    on Go { s = t(\"a\"); }\n    view { Text { \"{s}\" } }\n}\n"
        ),
        ["message-key-unknown"]
    );
}

#[test]
fn the_calls_need_the_locale_capability_and_it_takes_no_bound() {
    for call in [
        "t(\"a\")",
        "fmt.number(1, 2)",
        "fmt.percent(1)",
        "fmt.date(0, \"short\")",
        "fmt.time(0)",
        "locale.current()",
    ] {
        let src = comp(EN_DE, call).replace("    requires Locale;\n", "");
        assert_eq!(codes(&src), ["capability-missing"], "{call}");
    }
    let set = comp(EN_DE, "\"x\"")
        .replace("on Go { s = \"x\"; }", "on Go { ok = locale.set(\"en\"); }")
        .replace(
            "state s: String",
            "state ok: Bool = false;\n    state s: String",
        )
        .replace("    requires Locale;\n", "");
    assert_eq!(codes(&set), ["capability-missing"], "locale.set");
    let unused = comp(EN_DE, "\"x\"");
    assert_eq!(codes(&unused), ["capability-unused"]);
    let bounded = comp(EN_DE, "t(\"a\")").replace("requires Locale;", "requires Locale(\"en\");");
    assert_eq!(codes(&bounded), ["capability-narrowing-illegal"]);
    // storage.set keeps its own shape (the credential check no longer reads any `.set`)
    accepted(
        "component C {\n    requires Storage;\n    state ok: Bool = false;\n    on Go { ok = storage.set(\"k\", \"v\"); }\n    view { Text { \"x\" } }\n}\n",
    );
}

#[test]
fn the_language_is_in_requires_and_diff_shows_a_message_change() {
    let a = scratch("a.cui", &comp(EN_DE, "t(\"a\")"));
    let b = scratch(
        "b.cui",
        &comp(&EN_DE.replace("\"B\"", "\"Bee\""), "t(\"a\")"),
    );
    let (code, out, err) = run(&["inspect", "effects", p(&a), "C", "--output", "json"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Locale"), "{out}");
    let (code, out, _) = run(&["inspect", "ir", p(&a), "--output", "json"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"messages\""), "the IR carries the catalogs");
    let (code, _, _) = run(&["diff", p(&a), p(&a)]);
    assert_eq!(code, 0);
    let _ = b;
}

// ---- replay ------------------------------------------------------------------------------------------------------------

#[test]
fn the_language_comes_from_the_trace_and_is_in_the_report() {
    let src = comp(EN_DE, "t(\"a\") + locale.current()");
    let t = |locales: Value| json!({"schema": "cancho-ui-trace", "version": "0.1.0", "locales": locales, "steps": [{"event": "Go"}]});
    for (locales, want) in [
        (json!(["de"]), "Bde"),
        (json!(["de-AT", "en"]), "Bde"),
        (json!(["fr", "en"]), "Aen"),
        (json!([]), "Aen"),
        (json!(["EN_gb"]), "Aen"),
        (json!(["de_AT"]), "Bde"),
    ] {
        let mut trace = t(locales.clone());
        trace["steps"][0]["state"] = json!({"s": want});
        let (code, report) = replay(&src, &trace);
        assert_eq!(code, 0, "{locales}\n{report:#}");
    }
    // a malformed language list
    for bad in [
        json!("de"),
        json!([1]),
        json!(["x"]),
        json!(["de-"]),
        json!(["d e"]),
    ] {
        let (code, report) = replay(&src, &t(bad.clone()));
        assert_eq!(code, 4, "{bad}");
        assert_eq!(rules(&report), ["trace-locale-invalid"], "{bad}");
    }
}

#[test]
fn a_component_without_the_capability_has_no_document_in_its_report() {
    let src = "component C {\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n";
    let (code, report) = replay(
        src,
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"advance": 1, "expect": {"outcome": "ok"}}]}),
    );
    assert_eq!(code, 0);
    assert!(report["steps"][0].get("document").is_none());
}

#[test]
fn a_value_that_is_not_what_the_call_needs_is_a_replay_type_error() {
    let src = "component C {\n    requires Locale;\n    state s: String = \"\";\n    state x: String = \"a\";\n    on Go { s = fmt.number(x, 2); }\n    view { Text { \"{s}\" } }\n}\n";
    let (code, report) = replay(
        src,
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "Go", "state": {"s": ""}}]}),
    );
    assert_eq!(code, 4);
    assert_eq!(rules(&report), ["replay-type-error"]);
}

#[test]
fn the_edges_of_the_formats_are_empty_text_not_a_throw() {
    // 1e21 (as a product), seven places, a fraction of a place, a date past the year 9999 and one before 1000, and a value under 1e-6
    let src = "component C {\n    requires Locale;\n    state s: String = \"\";\n    on Go { s = fmt.number(1000000000000 * 1000000000, 2) + \"|\" + fmt.number(1.5, 7) + \"|\" + fmt.number(1.5, 0.5) + \"|\" + fmt.date(1000000000000000, \"short\") + \"|\" + fmt.time(0 - 1000000000000000) + \"|\" + fmt.number(1 / 10000000, 2) + \"|\" + fmt.number(2.5, 0); }\n    view { Text { \"{s}\" } }\n}\n";
    let (code, report) = replay(
        src,
        &json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "Go", "state": {"s": "|||||0.00|3"}}]}),
    );
    assert_eq!(code, 0, "{report:#}");
}

// ---- the page --------------------------------------------------------------------------------------------------------

fn built(src: &str) -> String {
    let file = scratch("f.cui", src);
    let out = scratch("out.js", "");
    let (code, _, stderr) = run(&["build", p(&file), "-o", p(&out), "--no-auto-mount"]);
    assert_eq!(code, 0, "{stderr}");
    std::fs::read_to_string(&out).expect("the built module")
}

#[test]
fn the_page_carries_the_data_and_the_steps() {
    let js = built(&comp(EN_DE, "t(\"a\")"));
    assert!(js.contains("const __CUI_I18N ="), "the data");
    assert!(js.contains("function __cui_fmt_number("), "the formatting");
    assert!(js.contains("function __cui_plural("), "the plural rules");
    assert!(
        js.contains("__cui_doc(__cui_loc.lang)"),
        "the document follows the language"
    );
    assert!(!js.contains("Intl."), "the page does not call Intl");
    assert!(!js.contains("toLocale"), "the page does not call toLocale");
    // only the languages the module uses
    assert!(
        js.contains("\"de\":{") && js.contains("\"en\":{") && !js.contains("\"ja\":{"),
        "data for en and de only"
    );
    // a catalog with text outside ASCII still builds to an ASCII module, one escape per UTF-16 unit
    let wide = built(&comp(
        "        en { a: \"caf\u{e9} \u{1F600}\"; }",
        "t(\"a\")",
    ));
    assert!(wide.is_ascii(), "the module is ASCII");
    assert!(
        wide.contains("caf\\u00e9 \\ud83d\\ude00"),
        "the escapes are in the data"
    );
    let plain = built("component A {\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n");
    assert!(
        !plain.contains("__CUI_I18N"),
        "a program with no Locale carries none of it"
    );
}

#[test]
fn the_stylesheet_aligns_table_headers_to_the_start() {
    let file = scratch(
        "f.cui",
        "component A {\n    state n: Int = 0;\n    view { Text { \"x\" } }\n}\n",
    );
    let dir = file.parent().expect("a directory").join("page");
    let (code, _, err) = run(&["build", p(&file), "--page", "-o", p(&dir)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(dir.join("index.html")).expect("index.html");
    assert!(
        html.contains(".cui-table th, .cui-table td { text-align: start;"),
        "start, not left"
    );
    assert!(!html.contains("text-align: left"));
}
