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
