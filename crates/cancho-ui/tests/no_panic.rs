//! CONTRIBUTING.md: no input may reach a panic. Every prefix and every
//! single-character deletion of every example program must be refused or
//! accepted by the front end, never crash it. The same mutants are then
//! diffed (both directions) against the original: `cancho-ui diff` must
//! never panic on any program the front end accepts.

use std::panic::catch_unwind;
use std::path::Path;

use cancho_ui_diff::{diff_ir, render_human};
use cancho_ui_ir::{IrFile, lower_file};
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_types::check_file;

fn front_end(src: &str) {
    let Ok(toks) = lex(src) else { return };
    let Ok(ast) = parse(toks) else { return };
    if check_file(&ast).is_err() {
        return;
    }
    let _ = lower_file(&ast);
}

fn cui_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("examples directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            cui_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "cui") {
            out.push(path);
        }
    }
}

#[test]
fn mutated_examples_never_panic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = Vec::new();
    cui_files(&root, &mut files);
    assert!(!files.is_empty(), "no examples found under {root:?}");

    let mut failures = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).expect("example is UTF-8");
        let starts: Vec<usize> = src.char_indices().map(|(i, _)| i).collect();
        for &i in &starts {
            let prefix = src[..i].to_string();
            let mut deleted = src.clone();
            deleted.remove(i);
            for (kind, text) in [("prefix", prefix), ("deletion", deleted)] {
                if catch_unwind(|| front_end(&text)).is_err() {
                    failures.push(format!("{} {kind} at byte {i}", file.display()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "panics:\n{}", failures.join("\n"));
}

fn ir_of(src: &str) -> Option<IrFile> {
    let ast = parse(lex(src).ok()?).ok()?;
    check_file(&ast).ok()?;
    lower_file(&ast).ok()
}

fn diff_both_ways(orig: &IrFile, mutant: &IrFile) {
    for (a, b) in [(orig, mutant), (mutant, orig), (mutant, mutant)] {
        let r = diff_ir(a, b, "a", "b");
        // Serialising and rendering are part of the command, so they are
        // part of the claim.
        serde_json::to_string_pretty(&r).expect("report serializes");
        let _ = render_human(&r);
        assert_eq!(r.changed, r.change_count > 0);
    }
    assert!(!diff_ir(mutant, mutant, "m", "m").changed);
}

#[test]
fn diff_on_mutated_examples_never_panics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = Vec::new();
    cui_files(&root, &mut files);

    let (mut diffed, mut failures) = (0usize, Vec::new());
    for file in &files {
        let src = std::fs::read_to_string(file).expect("example is UTF-8");
        // An example with an `import` needs its sibling file; the in-process
        // front end here does not resolve imports, so it is skipped by
        // `ir_of` returning None and covered by the mutation test above.
        let Some(orig) = ir_of(&src) else { continue };
        let starts: Vec<usize> = src.char_indices().map(|(i, _)| i).collect();
        for &i in &starts {
            let prefix = src[..i].to_string();
            let mut deleted = src.clone();
            deleted.remove(i);
            for (kind, text) in [("prefix", prefix), ("deletion", deleted)] {
                let Some(mutant) = ir_of(&text) else { continue };
                diffed += 1;
                if catch_unwind(|| diff_both_ways(&orig, &mutant)).is_err() {
                    failures.push(format!("{} {kind} at byte {i}", file.display()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "panics:\n{}", failures.join("\n"));
    // The claim is only worth something if mutants actually reached the
    // diff; a one-character deletion that still compiles is common
    // (whitespace, comments, a digit).
    eprintln!("diff exercised on {diffed} compiling mutants");
    assert!(diffed > 1000, "only {diffed} mutants compiled");
}

// ---------------------------------------------------------------- replay
//
// `cancho-ui test` over (a) every prefix and one-character deletion of every
// example trace, (b) every example trace with each JSON node replaced by an
// awkward value, and (c) every compiling mutant of an example program replayed
// with that example's own trace. A replay may fail; it may not panic, and its
// report must stay self-consistent.

use cancho_ui_replay::{TestReport, replay};

fn example_pairs() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut pairs = Vec::new();
    for dir in std::fs::read_dir(&root).expect("examples directory is readable") {
        let dir = dir.expect("entry").path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        for f in std::fs::read_dir(&dir).expect("example dir") {
            let f = f.expect("entry").path();
            if f.to_string_lossy().ends_with(".trace.json") {
                pairs.push((dir.join(format!("{name}.cui")), f));
            }
        }
    }
    pairs.sort();
    pairs
}

fn consistent(r: &TestReport) {
    assert_eq!(r.result == "pass", r.failures.is_empty());
    assert!(r.steps_run <= r.steps_total);
    assert_eq!(r.steps.len(), r.steps_run);
    // Serialising and rendering are part of the command, so part of the claim.
    serde_json::to_string_pretty(r).expect("report serializes");
    let _ = cancho_ui_replay::render_human(r);
}

/// Every node's address in a JSON value, as a list of keys and indices.
fn addresses(j: &serde_json::Value) -> Vec<Vec<String>> {
    fn walk(j: &serde_json::Value, at: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        out.push(at.clone());
        match j {
            serde_json::Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    at.push(i.to_string());
                    walk(x, at, out);
                    at.pop();
                }
            }
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    at.push(k.clone());
                    walk(x, at, out);
                    at.pop();
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(j, &mut Vec::new(), &mut out);
    out
}

fn replace_at(j: &mut serde_json::Value, addr: &[String], with: serde_json::Value) {
    match addr.split_first() {
        None => *j = with,
        Some((k, rest)) => {
            let child = match j {
                serde_json::Value::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
                serde_json::Value::Object(m) => m.get_mut(k),
                _ => None,
            };
            if let Some(c) = child {
                replace_at(c, rest, with);
            }
        }
    }
}

#[test]
fn replay_on_mutated_traces_never_panics() {
    use serde_json::json;
    let pairs = example_pairs();
    assert!(pairs.len() >= 9, "only {} example traces", pairs.len());
    let (mut runs, mut ran_steps, mut failures) = (0usize, 0usize, Vec::new());
    for (cui, tr) in &pairs {
        let src = std::fs::read_to_string(cui).expect("example is UTF-8");
        let orig = ir_of(&src).expect("the example compiles");
        let text = std::fs::read_to_string(tr).expect("trace is UTF-8");

        let mut mutants: Vec<(String, String)> = Vec::new();
        for (i, _) in text.char_indices() {
            let mut deleted = text.clone();
            deleted.remove(i);
            mutants.push((format!("prefix at {i}"), text[..i].to_string()));
            mutants.push((format!("deletion at {i}"), deleted));
        }
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("trace is JSON");
        let addrs = addresses(&parsed);
        let values = [
            json!(null),
            json!(-1),
            json!(0),
            json!(1e308),
            json!(1.5),
            json!(""),
            json!("x"),
            json!(true),
            json!([]),
            json!({}),
            json!({"Ok": null}),
            json!({"Err": "e"}),
            json!([[[]]]),
        ];
        for addr in &addrs {
            for v in &values {
                let mut m = parsed.clone();
                replace_at(&mut m, addr, v.clone());
                mutants.push((format!("replace {addr:?} with {v}"), m.to_string()));
            }
        }
        for (what, mutant) in mutants {
            runs += 1;
            match catch_unwind(|| {
                let r = replay(&orig, &mutant, "p.cui", "t.json");
                consistent(&r);
                r.steps_run
            }) {
                Ok(n) => ran_steps += n,
                Err(_) => failures.push(format!("{} {what}", tr.display())),
            }
        }
    }
    assert!(failures.is_empty(), "panics:\n{}", failures.join("\n"));
    eprintln!("replay exercised on {runs} mutated traces ({ran_steps} steps interpreted)");
    assert!(runs > 5000, "only {runs} mutants");
    assert!(
        ran_steps > 2000,
        "only {ran_steps} steps reached the interpreter"
    );
}

#[test]
fn replay_on_mutated_programs_never_panics() {
    let (mut runs, mut failures) = (0usize, Vec::new());
    for (cui, tr) in &example_pairs() {
        let src = std::fs::read_to_string(cui).expect("example is UTF-8");
        let text = std::fs::read_to_string(tr).expect("trace is UTF-8");
        for (i, _) in src.char_indices() {
            let mut deleted = src.clone();
            deleted.remove(i);
            for mutant in [src[..i].to_string(), deleted] {
                let Some(mir) = ir_of(&mutant) else { continue };
                runs += 1;
                if catch_unwind(|| consistent(&replay(&mir, &text, "p.cui", "t.json"))).is_err() {
                    failures.push(format!("{} byte {i}", cui.display()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "panics:\n{}", failures.join("\n"));
    eprintln!("replay exercised on {runs} compiling program mutants");
    assert!(runs > 1000, "only {runs} mutants compiled");
}
