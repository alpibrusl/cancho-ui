//! The files a view names (docs/structure.md section 10): `Image` sources, `Media` sources and posters. The IR crate checks the paths as text; this
//! module is where the program meets a directory: the file exists inside the program's directory, is within the size bound, and is the format its
//! extension says (its first bytes are read), and its SHA-256 goes into the IR so that `diff` can see a changed picture.

use std::collections::BTreeMap;
use std::path::Path;

use cancho_ui_ir::ir::{IrAsset, IrFile};
use cancho_ui_ir::media::{ASSET_MAX_BYTES, AssetUse, assets_of};

use crate::pipeline::{Diagnostic, import_diag};
use crate::theme::sha256_hex;

/// Whether the first bytes of a file are those of the format its extension names.
pub fn sniff(ext: &str, b: &[u8]) -> bool {
    match ext {
        "png" => b.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "jpg" | "jpeg" => b.starts_with(&[0xff, 0xd8, 0xff]),
        "gif" => b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a"),
        "webp" => b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP",
        "avif" => {
            b.len() >= 12
                && &b[4..8] == b"ftyp"
                && b[8..b.len().min(32)]
                    .windows(4)
                    .any(|w| w == b"avif" || w == b"avis")
        }
        "mp4" => b.len() >= 12 && &b[4..8] == b"ftyp",
        "webm" => b.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]),
        _ => false,
    }
}

fn refuse(code: &str, message: String, u: &AssetUse, program: &Path) -> Box<Diagnostic> {
    Box::new(import_diag(
        code,
        String::new(),
        message,
        u.span,
        program,
        Some(match code {
            "asset-missing" => "Put the file at that path, relative to the program's directory, or correct the path (docs/structure.md section 10).".to_string(),
            "asset-too-large" => "Re-encode the file smaller; a page of this kind ships files of at most 20 MB (docs/structure.md section 10).".to_string(),
            _ => "Use a file that is really in the format its extension names (docs/structure.md section 10).".to_string(),
        }),
    ))
}

/// Every file the views name, checked and hashed, sorted by path and without repeats.
pub fn verify(ir: &IrFile, dir: &Path, program: &Path) -> Result<Vec<IrAsset>, Box<Diagnostic>> {
    let mut uses: Vec<AssetUse> = Vec::new();
    for c in &ir.components {
        uses.extend(assets_of(&c.view.body));
    }
    let root = crate::pipeline::real_dir(dir);
    let mut found: BTreeMap<String, IrAsset> = BTreeMap::new();
    for u in &uses {
        if found.contains_key(&u.path) {
            continue;
        }
        let full = dir.join(&u.path);
        // a link to a file elsewhere is not the file of the site: the real path stays inside the directory of the program
        let inside = match (&root, full.canonicalize()) {
            (Some(r), Ok(f)) => f.starts_with(r),
            _ => false,
        };
        let meta = std::fs::metadata(&full).ok().filter(|m| m.is_file());
        let Some(meta) = meta.filter(|_| inside) else {
            return Err(refuse(
                "asset-missing",
                format!(
                    "`{}` is not a file inside the directory of the program ({})",
                    u.path,
                    dir.display()
                ),
                u,
                program,
            ));
        };
        if meta.len() > ASSET_MAX_BYTES {
            return Err(refuse(
                "asset-too-large",
                format!(
                    "`{}` is {} bytes; the most a file of the site may be is {} bytes",
                    u.path,
                    meta.len(),
                    ASSET_MAX_BYTES
                ),
                u,
                program,
            ));
        }
        let bytes = std::fs::read(&full).map_err(|e| {
            refuse(
                "asset-missing",
                format!("`{}` cannot be read: {e}", u.path),
                u,
                program,
            )
        })?;
        let ext = u
            .path
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !sniff(&ext, &bytes) {
            return Err(refuse(
                "asset-type-mismatch",
                format!(
                    "`{}` does not begin like a `.{ext}` file: the extension says one format and the bytes another",
                    u.path
                ),
                u,
                program,
            ));
        }
        found.insert(
            u.path.clone(),
            IrAsset {
                path: u.path.clone(),
                sha256: sha256_hex(&bytes),
                bytes: bytes.len() as u64,
            },
        );
    }
    Ok(found.into_values().collect())
}

/// The bytes of every file `ir` names, to be copied into a build: `(path, bytes)`.
pub fn load(ir: &IrFile, dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for a in &ir.assets {
        let bytes = std::fs::read(dir.join(&a.path))
            .map_err(|e| format!("reading `{}`: {e}", dir.join(&a.path).display()))?;
        out.push((a.path.clone(), bytes));
    }
    Ok(out)
}
