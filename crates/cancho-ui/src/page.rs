//! The files a `page { .. }` names beside the program (docs/page.md section 3): its icons and its alternates. The IR crate checks the paths as text;
//! this is where the program meets a directory, as `assets.rs` does for pictures.

use std::path::Path;

use cancho_ui_ir::ir::IrPage;

use crate::pipeline::{Diagnostic, import_diag};

/// The most a file named by a page may be: an icon or a data file, not a download.
pub const FILE_MAX_BYTES: u64 = 5 * 1024 * 1024;

fn inside(dir: &Path, rel: &str) -> Option<std::fs::Metadata> {
    let full = dir.join(rel);
    let root = crate::pipeline::real_dir(dir)?;
    let real = full.canonicalize().ok()?;
    if !real.starts_with(root) {
        return None;
    }
    std::fs::metadata(&full).ok().filter(|m| m.is_file())
}

/// Every file the page names exists inside the directory of the program and is not larger than [`FILE_MAX_BYTES`].
pub fn verify(page: &IrPage, dir: &Path, program: &Path) -> Result<(), Box<Diagnostic>> {
    for rel in cancho_ui_ir::page::files(page) {
        let Some(meta) = inside(dir, rel) else {
            return Err(Box::new(import_diag(
                "page-file-missing",
                String::new(),
                format!(
                    "`{rel}` is not a file inside the directory of the program ({})",
                    dir.display()
                ),
                page.span,
                program,
                Some("Put the file at that path, relative to the program's directory, or correct the path (docs/page.md section 3).".to_string()),
            )));
        };
        if meta.len() > FILE_MAX_BYTES {
            return Err(Box::new(import_diag(
                "page-file-too-large",
                String::new(),
                format!(
                    "`{rel}` is {} bytes; a file a page names is at most {FILE_MAX_BYTES}",
                    meta.len()
                ),
                page.span,
                program,
                Some("Use a smaller file (docs/page.md section 3).".to_string()),
            )));
        }
    }
    Ok(())
}

/// The files, at the paths the page uses, to be copied into the build.
pub fn load(page: &IrPage, dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for rel in cancho_ui_ir::page::files(page) {
        if out.iter().any(|(p, _)| p == rel) {
            continue;
        }
        let bytes = std::fs::read(dir.join(rel))
            .map_err(|e| format!("reading `{}`: {e}", dir.join(rel).display()))?;
        out.push((rel.to_string(), bytes));
    }
    Ok(out)
}
