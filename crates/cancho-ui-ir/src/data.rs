//! `data Name { file: ".." sha256: ".." }` (docs/data.md): the declaration, checked as text. Pure: the file is read by the CLI, which has a directory.

use cancho_ui_syntax::ast::File;

use crate::ir::IrData;
use crate::lower::LowerError;

/// The most bytes of a data file.
pub const FILE_MAX_BYTES: u64 = 1024 * 1024;
/// The deepest a data file may nest (the bound of docs/nesting.md).
pub const DEPTH_MAX: usize = 32;
/// The most items one `data.items` answers: `json.items`' own bound, so the page and the build read the same list.
pub const ROWS_MAX: usize = 1000;
/// The most nodes the expansion of one program may write.
pub const EXPANDED_MAX: u64 = 20_000;

fn refuse(code: &'static str, message: String, span: cancho_ui_syntax::Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// A path of a data file: written relative to the program, `.json`, the characters of an asset path.
pub fn path_ok(path: &str) -> Result<(), String> {
    crate::media::asset_type(path, &[("json", "application/json")]).map(|_| ())
}

/// The declarations of a file, checked as text.
pub fn lower_decls(file: &File) -> Result<Vec<IrData>, LowerError> {
    let mut out: Vec<IrData> = Vec::new();
    for d in &file.data {
        if out.iter().any(|o| o.name == d.name) {
            return Err(refuse(
                "data-duplicate",
                format!("`data {}` is declared twice; a name is one file", d.name),
                d.span,
            ));
        }
        if let Err(why) = path_ok(&d.file) {
            return Err(refuse(
                "data-path-invalid",
                format!("`file: \"{}\"`: {why}", d.file),
                d.span,
            ));
        }
        let hex = d.sha256.len() == 64
            && d.sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex {
            return Err(refuse(
                "data-hash-invalid",
                format!(
                    "`sha256: \"{}\"` is 64 lower-case hex digits (`sha256sum {}`)",
                    d.sha256, d.file
                ),
                d.span,
            ));
        }
        out.push(IrData {
            name: d.name.clone(),
            file: d.file.clone(),
            sha256: d.sha256.clone(),
            bytes: 0,
            rows: 0,
            components: Vec::new(),
            span: d.span,
        });
    }
    Ok(out)
}

/// The settings of the data files as `(name, value)`, the way `diff` compares them and `inspect data` lists them.
pub fn flatten(data: &[IrData]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in data {
        out.push((format!("{}.file", d.name), d.file.clone()));
        out.push((format!("{}.sha256", d.name), d.sha256.clone()));
        out.push((format!("{}.bytes", d.name), d.bytes.to_string()));
        out.push((format!("{}.rows", d.name), d.rows.to_string()));
    }
    out
}

/// A data file as JSON, for `inspect data` and `inspect effects`.
pub fn describe(d: &IrData) -> serde_json::Value {
    serde_json::json!({
        "name": d.name,
        "file": d.file,
        "sha256": d.sha256,
        "bytes": d.bytes,
        "rows": d.rows,
        "components": d.components,
    })
}
