//! `if` statements and `block`/`run` (docs/handler-sequences.md): every case of `tests/stmt_if_corpus.json` replayed in-process
//! (the file `scripts/check-handler-seq.js` runs against the emitted page in Chromium), the rules that refuse a misuse, and
//! what is emitted.

use std::path::{Path, PathBuf};
use std::process::Command;

use cancho_ui_ir::lower_file;
use cancho_ui_replay::replay;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../tests/stmt_if_corpus.json");

fn source(case: &Value) -> String {
    case["source"]
        .as_array()
        .expect("source lines")
        .iter()
        .map(|l| l.as_str().expect("a line"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The trace of a case. Requests are numbered in the order they are issued, which is how the page's requests are matched too.
fn trace_of(case: &Value) -> String {
    let mut next = 1u32;
    let mut issue = |urls: &Value| -> Vec<Value> {
        urls.as_array()
            .expect("issued")
            .iter()
            .map(|u| {
                let r = json!({"request": next, "effect": "net.get", "args": [u]});
                next += 1;
                r
            })
            .collect()
    };
    let mut steps = Vec::new();
    for s in case["steps"].as_array().expect("steps") {
        let issued = issue(&s["issued"]);
        let outcome = if s.get("fault").is_some() {
            "faulted"
        } else if s.get("dropped").is_some() {
            "dropped"
        } else if issued.is_empty() {
            "completed"
        } else {
            "suspended"
        };
        let mut expect = json!({"outcome": outcome, "issued": issued});
        if s.get("dropped").is_some() {
            expect["fired"] = json!([{"request": s["resolve"], "delivered": "dropped"}]);
        }
        let mut step = if let Some(id) = s.get("resolve") {
            let a = &s["answer"];
            let status = a["status"].as_u64().expect("a status");
            let with = if (200..300).contains(&status) {
                json!({"Ok": a["body"]})
            } else {
                json!({"Err": format!("HTTP {status}")})
            };
            json!({"resolve": id, "with": with})
        } else {
            json!({"event": s["event"], "params": s.get("params").cloned().unwrap_or(json!([]))})
        };
        step["state"] = s["state"].clone();
        step["expect"] = expect;
        if let Some(f) = s.get("fault") {
            step["fault"] = f.clone();
        }
        steps.push(step);
    }
    json!({"schema": "cancho-ui-trace", "version": "0.1.0", "steps": steps}).to_string()
}

#[test]
fn replay_agrees_with_every_case_of_the_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let cases = corpus.as_array().expect("a list");
    assert!(cases.len() >= 6, "the corpus shrank");
    let mut skipped = 0;
    for case in cases {
        let src = source(case);
        let ast = parse(lex(&src).expect("lexes")).expect("parses");
        check_file(&ast).expect("type-checks");
        let ir = lower_file(&ast).expect("lowers");
        let report = replay(&ir, &trace_of(case), "prog.cui", "t.json");
        let rules: Vec<&str> = report.failures.iter().map(|f| f.rule).collect();
        assert!(
            rules.is_empty() && report.result == "pass",
            "{}: {rules:?} {}\n{:#?}",
            case["why"],
            report.result,
            report.failures
        );
        for s in case["steps"].as_array().expect("steps") {
            if s["event"].is_string() && s["issued"].as_array().is_some_and(|i| i.is_empty()) {
                skipped += 1;
            }
        }
    }
    assert!(
        skipped >= 6,
        "the corpus must show events that issue nothing"
    );
}

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

/// A file of its own per call: the tests run in parallel, and a shared name would be a race.
fn scratch(name: &str, text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cancho-ui-seq-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn p(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// A component with a few fields, the given members, and a trivial view.
fn program(members: &str) -> String {
    format!(
        "component C {{\n    requires Network;\n    state n: Int = 0;\n    state t: Bool = true;\n    state s: String = \"x\";\n    state r: Result[String, String] = Ok(\"\");\n{members}\n    view {{ Column {{ Button {{ label: \"Go\" on: Go }} Text {{ \"{{n}}\" }} }} }}\n}}\n"
    )
}

fn diagnostics(src: &str) -> (i32, Vec<String>) {
    let file = scratch("p.cui", src);
    let (code, stdout, _) = run(&["check", p(&file), "--output", "json"]);
    let diags: Value = serde_json::from_str(&stdout).unwrap_or(json!([]));
    let codes = diags
        .as_array()
        .map(|a| {
            a.iter()
                .map(|d| d["code"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default();
    (code, codes)
}

fn refused(src: &str) -> Vec<String> {
    let (code, codes) = diagnostics(src);
    assert_eq!(code, 1, "{src}: {codes:?}");
    codes
}

fn accepted(src: &str) {
    let (code, codes) = diagnostics(src);
    assert_eq!(code, 0, "{src}: {codes:?}");
}

const GO: &str = "    on Go { r = await(net.get(\"a\")); }\n";

#[test]
fn a_run_of_nothing_is_refused() {
    let src = program(&format!("{GO}    on Other {{ run Missing; }}"));
    assert_eq!(refused(&src), ["block-unknown"]);
}

#[test]
fn two_blocks_of_one_name_are_refused() {
    let src = program(&format!(
        "{GO}    block B {{ n = 1; }}\n    block B {{ n = 2; }}"
    ));
    assert_eq!(refused(&src), ["block-duplicate"]);
}

#[test]
fn a_block_named_like_a_field_or_an_event_is_refused() {
    for name in ["n", "Go"] {
        let src = program(&format!("{GO}    block {name} {{ n = 1; }}"));
        assert_eq!(refused(&src), ["block-name-conflict"], "{name}");
    }
}

#[test]
fn a_block_that_runs_itself_is_refused() {
    for blocks in [
        "    block A { run A; }",
        "    block A { run B; }\n    block B { run A; }",
        "    block A { if t { run B; } }\n    block B { run C; }\n    block C { n = 1; run A; }",
    ] {
        let src = program(&format!("{GO}{blocks}\n    on Use {{ run A; }}"));
        assert_eq!(refused(&src), ["block-cycle"], "{blocks}");
    }
}

#[test]
fn a_blow_up_of_runs_is_refused_and_a_diamond_is_not() {
    // Each block runs the next twice: 2^15 statements, far over the limit, and no cycle.
    let mut blocks = String::new();
    for i in 0..15 {
        blocks.push_str(&format!(
            "    block B{i} {{ run B{}; run B{}; }}\n",
            i + 1,
            i + 1
        ));
    }
    blocks.push_str("    block B15 { n = n + 1; }\n    on Use { run B0; }");
    assert_eq!(
        refused(&program(&format!("{GO}{blocks}"))),
        ["block-expansion-too-large"]
    );
    // Many blocks that all run one block are a diamond, not a cycle, and small.
    let mut diamond = String::from("    block Leaf { n = n + 1; }\n");
    for i in 0..40 {
        diamond.push_str(&format!("    block M{i} {{ run Leaf; }}\n"));
    }
    diamond.push_str("    on Use { run M0; run M1; }");
    accepted(&program(&format!("{GO}{diamond}")));
}

#[test]
fn a_block_cannot_see_the_parameters_of_the_handler_that_runs_it() {
    let src = program(&format!(
        "{GO}    block B {{ n = v; }}\n    on Use(v) {{ run B; }}"
    ));
    assert_eq!(refused(&src), ["name-unknown"]);
}

#[test]
fn the_condition_of_an_if_statement_is_a_pure_bool() {
    for cond in ["n", "s", "n + 1"] {
        let src = program(&format!("    on Go {{ if {cond} {{ n = 1; }} }}"));
        assert_eq!(
            refused(&src.replace("requires Network;", "")),
            ["condition-not-bool"],
            "{cond}"
        );
    }
    let src = program("    on Go { if net.get(\"a\") == \"\" { n = 1; } }");
    assert_eq!(refused(&src), ["if-effect-in-expression"]);
    let src = program("    on Go { if await(net.get(\"a\")) == \"\" { n = 1; } }");
    assert_eq!(refused(&src), ["if-effect-in-expression"]);
}

#[test]
fn an_if_statement_may_omit_else_and_hold_an_effect() {
    accepted(&program(
        "    on Go { if t { r = await(net.get(\"a\")); } }",
    ));
    accepted(&program(
        "    on Go { if t { n = 1; } else if s == \"x\" { n = 2; } else { r = await(net.get(\"a\")); } }",
    ));
    // The expression form is still an expression, and still needs both sides.
    let src = program(&format!("{GO}    on Other {{ n = if t {{ 1 }}; }}"));
    assert_eq!(refused(&src), ["if-else-required"]);
}

#[test]
fn run_and_block_are_words_only_where_they_are_keywords() {
    accepted(
        "component C {\n    state run: Int = 0;\n    state block: Int = 0;\n    on Go { run = block + 1; block = run; }\n    view { Text { \"{run}\" } }\n}\n",
    );
}

#[test]
fn a_deep_nest_is_a_parse_error_and_not_a_crash() {
    let mut open = String::new();
    let mut close = String::new();
    for _ in 0..200 {
        open.push_str("if t { ");
        close.push_str(" }");
    }
    let src = program(&format!("    on Go {{ {open}n = 1;{close} }}"));
    assert_eq!(refused(&src), ["parse-nesting-too-deep"]);
    let mut chain = String::from("if t { n = 0; }");
    for _ in 0..200 {
        chain.push_str(" else if t { n = 1; }");
    }
    let src = program(&format!("    on Go {{ {chain} }}"));
    assert_eq!(refused(&src), ["parse-nesting-too-deep"]);
}

#[test]
fn the_page_awaits_inside_an_if() {
    let src = program("    on Go { if t { r = await(net.get(\"a\")); n = 1; } }");
    let file = scratch("emit.cui", &src);
    let out = scratch("emit.html", "");
    let (code, _, err) = run(&["build", p(&file), "-o", p(&out)]);
    assert_eq!(code, 0, "{err}");
    let html = std::fs::read_to_string(&out).expect("the page");
    assert!(html.contains("if (__state_t) {"), "a JavaScript if");
    assert!(html.contains("await __cui_call("), "an await in the branch");
}

#[test]
fn the_semantic_diff_shows_an_if_and_the_expanded_block() {
    let a = scratch(
        "a.cui",
        &program(&format!(
            "{GO}    block B {{ n = 1; }}\n    on Other {{ run B; }}"
        )),
    );
    let b = scratch(
        "b.cui",
        &program(&format!(
            "{GO}    block B {{ n = 2; }}\n    on Other {{ if t {{ run B; }} }}"
        )),
    );
    let (code, stdout, _) = run(&["diff", p(&a), p(&b), "--output", "json"]);
    assert_eq!(code, 5, "{stdout}");
    assert!(stdout.contains("if t { n = 2; }"), "{stdout}");
}
