//! CONTRIBUTING.md: no input may reach a panic. Every prefix and every
//! single-character deletion of every example program must be refused or
//! accepted by the front end, never crash it.

use std::panic::catch_unwind;
use std::path::Path;

use cancho_ui_ir::lower_file;
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
