//! Deep nesting (docs/nesting.md, issue #99): expressions, view nodes and types nest at most `MAX_NEST_DEPTH` (32) deep, and anything deeper is
//! `parse-nesting-too-deep`, not a stack overflow. Each construct is run at its limit and past it (and at the two depths of the report, 1,000 and
//! 100,000) in a child process whose stack is 2 MiB, the default of a thread a library user spawns, in `check`, `build` and `test`.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_cancho-ui");

fn rep(s: &str, n: usize) -> String {
    s.repeat(n)
}

fn handler(stmt: &str) -> String {
    format!(
        "component T {{\n    state a: Int = 0;\n    state r: Result[String, String] = Ok(\"\");\n    state l: List[String] = [];\n    on Go {{ {stmt} }}\n    view {{ Column {{ Text {{ \"{{a}}\" }} }} }}\n}}\n"
    )
}

fn view(v: &str) -> String {
    format!("component T {{\n    state a: Int = 0;\n    view {{ {v} }}\n}}\n")
}

/// (name, the program for a nesting of n, the largest n the parser accepts, whether the program at the limit is a valid program)
type Construct = (&'static str, fn(usize) -> String, usize, bool);

const CONSTRUCTS: &[Construct] = &[
    (
        "parenthesis",
        |n| handler(&format!("a = {}1{};", rep("(", n), rep(")", n))),
        31,
        true,
    ),
    (
        "unary minus",
        |n| handler(&format!("a = {}1;", rep("- ", n))),
        31,
        true,
    ),
    (
        "chain of +",
        |n| handler(&format!("a = 1{};", rep(" + 1", n))),
        32,
        true,
    ),
    (
        "chain of *",
        |n| handler(&format!("a = 1{};", rep(" * 1", n))),
        32,
        true,
    ),
    (
        "chain of &&",
        |n| handler(&format!("if true{} {{ a = 2; }}", rep(" && true", n))),
        32,
        true,
    ),
    (
        "call",
        |n| handler(&format!("a = {}1{};", rep("max(", n), rep(", 1)", n))),
        31,
        true,
    ),
    (
        "list literal",
        |n| handler(&format!("l = {}{};", rep("[", n), rep("]", n))),
        32,
        false,
    ),
    (
        "match arm",
        |n| {
            handler(&format!(
                "a = {}1{};",
                rep("match r { Ok(_) => { ", n),
                rep(" } Err(_) => { 2 } Cancelled => { 3 } }", n)
            ))
        },
        31,
        true,
    ),
    (
        "else if",
        |n| handler(&format!("a = {}{{ 2 }};", rep("if true { 1 } else ", n))),
        31,
        true,
    ),
    (
        "lambda",
        |n| {
            handler(&format!(
                "l = {}true{};",
                rep("filter(l, x => ", n),
                rep(")", n)
            ))
        },
        31,
        true,
    ),
    (
        "type argument",
        |n| {
            format!(
                "component T {{\n    state t: {}String{} = [];\n    view {{ Column {{ Text {{ \"x\" }} }} }}\n}}\n",
                rep("List[", n),
                rep("]", n)
            )
        },
        31,
        false,
    ),
    (
        "interpolation",
        |n| {
            view(&format!(
                "Text {{ \"{{{}a{}}}\" }}",
                rep("(", n),
                rep(")", n)
            ))
        },
        29,
        true,
    ),
    (
        "view node",
        |n| {
            view(&format!(
                "{}Text {{ \"x\" }}{}",
                rep("Column { ", n),
                rep(" }", n)
            ))
        },
        30,
        true,
    ),
    (
        "view If",
        |n| {
            view(&format!(
                "{}Text {{ \"x\" }}{}",
                rep("If { when: true ", n),
                rep(" }", n)
            ))
        },
        31,
        true,
    ),
    (
        "view For",
        |n| {
            view(&format!(
                "{}Text {{ \"x\" }}{}",
                rep("For { over: [1] as: i ", n),
                rep(" }", n)
            ))
        },
        30,
        true,
    ),
];

struct Out {
    code: Option<i32>,
    text: String,
}

/// Run the compiler with a 2 MiB stack (`ulimit -s 2048`); a stack overflow is a signal, so `code` is `None`.
fn run(args: &[&str]) -> Out {
    let out = Command::new("sh")
        .args(["-c", "ulimit -s 2048; exec \"$0\" \"$@\"", BIN])
        .args(args)
        .output()
        .expect("sh runs");
    Out {
        code: out.status.code(),
        text: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}

fn scratch(tag: &str, text: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cancho-ui-nesting-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(format!("{tag}.cui"));
    std::fs::write(&path, text).expect("a scratch file");
    path
}

fn codes(src: &str) -> (Option<i32>, Vec<String>) {
    let file = scratch("f", src);
    let out = run(&["check", file.to_str().expect("path"), "--output", "json"]);
    let codes = serde_json::from_str::<Value>(&out.text)
        .ok()
        .and_then(|v| {
            v.as_array().map(|a| {
                a.iter()
                    .filter_map(|d| d["code"].as_str().map(str::to_string))
                    .collect()
            })
        })
        .unwrap_or_default();
    (out.code, codes)
}

#[test]
fn past_the_limit_is_a_diagnostic_with_a_tag_at_every_depth() {
    for (name, make, limit, _) in CONSTRUCTS {
        for n in [limit + 1, 100, 1000, 100_000] {
            let (code, found) = codes(&make(n));
            assert_eq!(code, Some(1), "{name} at {n}: aborted or accepted");
            assert_eq!(found, ["parse-nesting-too-deep"], "{name} at {n}");
        }
    }
}

#[test]
fn at_the_limit_the_pipeline_runs_on_a_two_mebibyte_stack() {
    let trace = scratch("t", "").with_extension("trace.json");
    std::fs::write(
        &trace,
        r#"{"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "Go", "state": {"a": 0}}]}"#,
    )
    .expect("trace");
    for (name, make, limit, valid) in CONSTRUCTS {
        let (code, found) = codes(&make(*limit));
        assert!(
            !found.contains(&"parse-nesting-too-deep".to_string()),
            "{name} at {limit}: refused"
        );
        assert!(code.is_some(), "{name} at {limit}: check aborted");
        if !valid {
            continue;
        }
        let file = scratch("g", &make(*limit));
        let f = file.to_str().expect("path");
        let out_dir = file.with_extension("out");
        let built = run(&["build", f, "--page", "-o", out_dir.to_str().expect("path")]);
        assert!(
            built.code.is_some(),
            "{name} at {limit}: build aborted: {}",
            built.text
        );
        if name.starts_with("view") || *name == "interpolation" {
            continue; // the handler `Go` of these is not the one the trace fires; the checks above are the point
        }
        let replayed = run(&["test", f, trace.to_str().expect("path")]);
        assert!(
            replayed.code.is_some(),
            "{name} at {limit}: replay aborted: {}",
            replayed.text
        );
    }
}

#[test]
fn the_report_reproductions_are_refused() {
    // a = (((...(1)...))); and Column { Column { .. Text { "x" } } } at the depths that aborted the compiler
    for n in [1000, 2000, 5000, 200_000] {
        let (code, found) = codes(&handler(&format!("a = {}1{};", rep("(", n), rep(")", n))));
        assert_eq!(
            (code, found),
            (Some(1), vec!["parse-nesting-too-deep".to_string()]),
            "{n} parentheses"
        );
        let (code, found) = codes(&view(&format!(
            "{}Text {{ \"x\" }}{}",
            rep("Column { ", n),
            rep(" }", n)
        )));
        assert_eq!(
            (code, found),
            (Some(1), vec!["parse-nesting-too-deep".to_string()]),
            "{n} Columns"
        );
    }
}

#[test]
fn no_example_is_refused_by_the_bound() {
    // the bound refuses no example
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for entry in std::fs::read_dir(root.join("examples")).expect("examples") {
        let dir = entry.expect("entry").path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("dir") {
            let f = f.expect("file").path();
            if f.extension().is_some_and(|e| e == "cui") {
                let out = run(&["check", f.to_str().expect("path")]);
                assert_eq!(out.code, Some(0), "{}: {}", f.display(), out.text);
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "{checked}");
}

/// `n` blocks, each running the next (and the last assigning), run by the handler `Go`; with `branch` each `run` sits inside an `if`.
fn block_chain(n: usize, branch: bool) -> String {
    let mut out = String::from("component T {\n    state a: Int = 0;\n");
    for i in 0..n {
        let body = if i + 1 == n {
            "a = 1;".to_string()
        } else if branch {
            format!("if true {{ run B{}; }}", i + 1)
        } else {
            format!("run B{};", i + 1)
        };
        out.push_str(&format!("    block B{i} {{ {body} }}\n"));
    }
    out.push_str("    on Go { run B0; }\n    view { Column { Text { \"{a}\" } } }\n}\n");
    out
}

#[test]
fn blocks_that_run_blocks_are_bounded_after_the_expansion() {
    // `run` is replaced after parsing, so the parser's bound does not see a chain of blocks: 2,000 of them overflowed a 2 MiB stack
    let trace = scratch("t", "").with_extension("trace.json");
    std::fs::write(
        &trace,
        r#"{"schema": "cancho-ui-trace", "version": "0.1.0", "steps": [{"event": "Go", "state": {"a": 1}}]}"#,
    )
    .expect("trace");
    for branch in [false, true] {
        // at the limit: a chain of 20 runs (12 with an `if` around each: a `run` and an `if` count one each) on a 2 MiB stack, in all three commands
        let file = scratch("c", &block_chain(if branch { 12 } else { 20 }, branch));
        let f = file.to_str().expect("path");
        assert_eq!(run(&["check", f]).code, Some(0), "check, branch {branch}");
        let out_dir = file.with_extension("out");
        assert_eq!(
            run(&["build", f, "--page", "-o", out_dir.to_str().expect("path")]).code,
            Some(0),
            "build, branch {branch}"
        );
        assert_eq!(
            run(&["test", f, trace.to_str().expect("path")]).code,
            Some(0),
            "test, branch {branch}"
        );
        // 20 blocks with an `if` around each `run` are 40 deep once expanded: shorter than the chain bound (32 blocks), so only the
        // depth of the expansion refuses it
        if branch {
            let (code, found) = codes(&block_chain(20, true));
            assert_eq!(
                (code, found),
                (Some(1), vec!["block-expansion-too-deep".to_string()]),
                "20 blocks in ifs"
            );
        }
        for n in [40, 200, 2000, 100_000] {
            let (code, found) = codes(&block_chain(n, branch));
            assert_eq!(code, Some(1), "{n} blocks, branch {branch}");
            assert_eq!(
                found,
                ["block-expansion-too-deep"],
                "{n} blocks, branch {branch}"
            );
        }
    }
}

/// `n` files `m0.cui` .. each importing the next; returns the path of `m0.cui`.
fn import_chain(n: usize) -> PathBuf {
    let dir = scratch("m0", "").parent().expect("dir").to_path_buf();
    for i in 0..n {
        let imp = if i + 1 == n {
            String::new()
        } else {
            format!("import m{}.C{};\n", i + 1, i + 1)
        };
        let body = if i + 1 == n {
            "Text { \"x\" }".to_string()
        } else {
            format!("C{} {{ }}", i + 1)
        };
        std::fs::write(
            dir.join(format!("m{i}.cui")),
            format!(
                "module m{i};\n\n{imp}\ncomponent C{i} {{\n    view {{ Column {{ {body} }} }}\n}}\n"
            ),
        )
        .expect("a module");
    }
    dir.join("m0.cui")
}

#[test]
fn a_chain_of_imports_is_bounded() {
    // each file is resolved inside the resolution of the file that imports it: 2,000 files overflowed a 2 MiB stack
    let ok = import_chain(30);
    assert_eq!(run(&["check", ok.to_str().expect("path")]).code, Some(0));
    let out_dir = ok.with_extension("out");
    assert_eq!(
        run(&[
            "build",
            ok.to_str().expect("path"),
            "--page",
            "-o",
            out_dir.to_str().expect("path")
        ])
        .code,
        Some(0)
    );
    for n in [40, 2000] {
        let file = import_chain(n);
        let out = run(&["check", file.to_str().expect("path"), "--output", "json"]);
        assert_eq!(
            out.code,
            Some(1),
            "{n} files: {}",
            out.text.chars().take(200).collect::<String>()
        );
        let v: Value = serde_json::from_str(&out.text).expect("JSON");
        assert_eq!(v[0]["code"], "import-chain-too-deep", "{n} files");
    }
}
