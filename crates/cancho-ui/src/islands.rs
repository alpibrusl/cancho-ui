//! The files of the sealed islands (docs/islands.md): where the program meets a directory. The IR crate checks the declarations as text; this reads each
//! file an island names, checks that it is inside the program's directory, within the size bounds, text, safe to put in a document byte for byte,
//! and the file the program pinned (its SHA-256), and writes the document of the frame.

use std::path::Path;

use cancho_ui_ir::ir::{IrFile, IrIsland, IrIslandFile};
use cancho_ui_ir::island::{
    FILE_MAX_BYTES, Piece, READY, SHIM, TOTAL_MAX_BYTES, frame_document, unsafe_in_html,
};

use crate::pipeline::{Diagnostic, import_diag};
use crate::theme::sha256_hex;

pub fn remediation(code: &str) -> String {
    match code {
        "island-hash-mismatch" => "`sha256` is the SHA-256 of the file as it is on disk (`sha256sum <file>`). A file that changed is a change of the program: read it again, then write the new hash.".to_string(),
        "island-source-missing" => "A file of an island is a file inside the directory of the program: put it at that path, or correct the path.".to_string(),
        "island-source-too-large" | "island-too-large" => "An island's files are copied into the page: keep each under 512 kB and all of them together under 1 MB.".to_string(),
        "island-source-not-text" => "A script or a style is UTF-8 text.".to_string(),
        "island-source-unsafe" => "The file would end its own element, or hide a comment or a NUL, in the document of the frame. Write `<\\/script>` for `</script>` inside a string, and no `<!--`.".to_string(),
        "island-duplicate" => "Island names are unique across the program and its imports.".to_string(),
        _ => "See docs/islands.md.".to_string(),
    }
}

fn refuse(code: &str, message: String, island: &IrIsland, program: &Path) -> Box<Diagnostic> {
    Box::new(import_diag(
        code,
        island.name.clone(),
        message,
        island.span,
        program,
        Some(remediation(code)),
    ))
}

/// Reads, checks and hashes one file of an island.
fn read(
    f: &IrIslandFile,
    style: bool,
    html: bool,
    root: &Option<std::path::PathBuf>,
    dir: &Path,
    island: &IrIsland,
    program: &Path,
) -> Result<(IrIslandFile, String), Box<Diagnostic>> {
    let full = dir.join(&f.path);
    // a link to a file elsewhere is not the file of the site: the real path stays inside the directory of the program
    let inside = match (root, full.canonicalize()) {
        (Some(r), Ok(p)) => p.starts_with(r),
        _ => false,
    };
    let meta = std::fs::metadata(&full).ok().filter(|m| m.is_file());
    let Some(meta) = meta.filter(|_| inside) else {
        return Err(refuse(
            "island-source-missing",
            format!(
                "island `{}`: `{}` is not a file inside the directory of the program ({})",
                island.name,
                f.path,
                dir.display()
            ),
            island,
            program,
        ));
    };
    if meta.len() > FILE_MAX_BYTES {
        return Err(refuse(
            "island-source-too-large",
            format!(
                "island `{}`: `{}` is {} bytes; the most a file of an island may be is {FILE_MAX_BYTES} bytes",
                island.name,
                f.path,
                meta.len()
            ),
            island,
            program,
        ));
    }
    let bytes = std::fs::read(&full).map_err(|e| {
        refuse(
            "island-source-missing",
            format!("island `{}`: `{}` cannot be read: {e}", island.name, f.path),
            island,
            program,
        )
    })?;
    let actual = sha256_hex(&bytes);
    if actual != f.sha256 {
        return Err(refuse(
            "island-hash-mismatch",
            format!(
                "island `{}`: `{}` has the sha256 {actual}; the program pins {}",
                island.name, f.path, f.sha256
            ),
            island,
            program,
        ));
    }
    let Ok(text) = String::from_utf8(bytes) else {
        return Err(refuse(
            "island-source-not-text",
            format!("island `{}`: `{}` is not UTF-8 text", island.name, f.path),
            island,
            program,
        ));
    };
    // a sealed island's files are copied into the document of a frame; an open island's are run from a string, so what would end an element is no harm
    if html && let Some(what) = unsafe_in_html(&text, style) {
        return Err(refuse(
            "island-source-unsafe",
            format!(
                "island `{}`: `{}` holds {what}, which cannot be put in the document of the frame byte for byte",
                island.name, f.path
            ),
            island,
            program,
        ));
    }
    Ok((
        IrIslandFile {
            path: f.path.clone(),
            sha256: f.sha256.clone(),
            bytes: text.len() as u64,
        },
        text,
    ))
}

/// Reads every island's files and gives each island its document.
pub fn load(ir: &mut IrFile, dir: &Path, program: &Path) -> Result<(), Box<Diagnostic>> {
    if ir.islands.is_empty() {
        return Ok(());
    }
    for (n, island) in ir.islands.iter().enumerate() {
        if ir.islands[..n].iter().any(|o| o.name == island.name) {
            return Err(refuse(
                "island-duplicate",
                format!(
                    "island `{}` is declared in two files of the program",
                    island.name
                ),
                island,
                program,
            ));
        }
    }
    let root = dir.canonicalize().ok();
    let (shim, ready) = (sha256_hex(SHIM.as_bytes()), sha256_hex(READY.as_bytes()));
    let mut done = Vec::new();
    for island in &ir.islands {
        let mut island = island.clone();
        let mut texts = Vec::new();
        let mut total = 0u64;
        for n in 0..island.scripts.len() {
            let (f, text) = read(
                &island.scripts[n],
                false,
                island.sealed,
                &root,
                dir,
                &island,
                program,
            )?;
            total += f.bytes;
            island.scripts[n] = f;
            texts.push(text);
        }
        let mut css = None;
        if let Some(s) = island.style.clone() {
            let (f, text) = read(&s, true, island.sealed, &root, dir, &island, program)?;
            total += f.bytes;
            island.style = Some(f);
            css = Some(text);
        }
        if total > TOTAL_MAX_BYTES {
            return Err(refuse(
                "island-too-large",
                format!(
                    "island `{}`: its files are {total} bytes together; the most is {TOTAL_MAX_BYTES} bytes",
                    island.name
                ),
                &island,
                program,
            ));
        }
        let pieces: Vec<Piece> = island
            .scripts
            .iter()
            .zip(&texts)
            .map(|(f, t)| Piece {
                text: t,
                sha256: &f.sha256,
            })
            .collect();
        if island.sealed {
            island.srcdoc = frame_document(&pieces, css.as_deref(), &shim, &ready);
        } else {
            island.texts = texts.clone();
            island.style_text = css.clone();
        }
        done.push(island);
    }
    cancho_ui_ir::island::attach(ir, done);
    Ok(())
}
