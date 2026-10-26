//! The files of a `page { .. }` (docs/page.md): where the program meets a directory. The IR crate checks the declaration as text; this reads each file
//! the page names (the card image, the icons, the alternates), checks that it is inside the program's directory, within the size bound and the format
//! its extension says, fills its SHA-256, and reads the card image's width and height from its header, so there is no pair of numbers to declare
//! and get wrong. The build copies the same files next to `index.html`.

use std::path::Path;

use cancho_ui_ir::ir::{IrFile, IrPageFile};
use cancho_ui_ir::media::ASSET_MAX_BYTES;

use crate::assets::sniff;
use crate::pipeline::{Diagnostic, import_diag};
use crate::pwa::PageParts;
use crate::theme::sha256_hex;

pub fn remediation(code: &str) -> String {
    match code {
        "page-title-missing" => "Write `title: \"..\";` in `page { .. }`: it is the `<title>` and the title of the card.".to_string(),
        "page-lang-missing" | "page-lang-invalid" => "Write `lang: \"en\";` (a language tag: `en`, `es-419`, `zh-Hant-TW`).".to_string(),
        "page-description-length" => "A description is one to 300 characters. The card can have a shorter text of its own: `share_description: \"..\";`.".to_string(),
        "page-canonical-not-https" | "page-canonical-invalid" => "`canonical:` is the address people should share, `https://host/dir/` in lower case, with no user name and no `#fragment`.".to_string(),
        "page-image-alt-missing" => "Write the text for a person who cannot see the card: `image: \"og.png\" alt: \"..\";`.".to_string(),
        "page-image-needs-canonical" => "Write `canonical: \"https://host/dir/\";`: the card's address is the file's path joined to that directory.".to_string(),
        "page-image-size-unreadable" => "The card image is a PNG, a JPEG or a WebP whose header gives its width and height; re-export it.".to_string(),
        "page-alternate-title-missing" => "Write `alternate: \"agents.md\" title: \"..\";`.".to_string(),
        "page-file-invalid" => "A file of the page is relative to the program (`logo.png`, `data/ecosystem.json`), inside its directory, with an extension its entry takes (docs/page.md section 5).".to_string(),
        "page-entry-unknown" | "page-entry-duplicate" | "page-duplicate" | "page-text-invalid" | "page-skip-link-empty" => "See docs/page.md section 3: the entries are title, lang, description, share_description, canonical, image (with alt), icon, touch_icon, alternate (with title) and skip_link, each once but `alternate`; a program has one `page { .. }`.".to_string(),
        "skip-link-target-missing" | "skip-link-target-dynamic" => "Write `Main { id: \"main\" .. }` in the component that is mounted into the page, outside any `For` or `If` (docs/page.md section 4).".to_string(),
        "header-sticky-invalid" | "header-sticky-duplicate" | "header-sticky-misplaced" => "`Header { sticky: true .. }` once, as a child of the page's column and not inside a Section, Article, Aside, Main, Nav, Footer, Figure, Details or Header (docs/page.md section 4).".to_string(),
        _ => "See docs/page.md.".to_string(),
    }
}

fn refuse(code: &str, message: String, f: &IrPageFile, program: &Path) -> Box<Diagnostic> {
    Box::new(import_diag(
        code,
        String::new(),
        message,
        f.span,
        program,
        Some(match code {
            "asset-missing" => "Put the file at that path, relative to the program's directory, or correct the path (docs/page.md section 3).".to_string(),
            "asset-too-large" => "Re-encode the file smaller; a page of this kind ships files of at most 20 MB (docs/page.md section 3).".to_string(),
            "asset-type-mismatch" => "Use a file that is really in the format its extension names (docs/page.md section 3).".to_string(),
            c => remediation(c),
        }),
    ))
}

fn u16le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from(*b.get(i)?) | u32::from(*b.get(i + 1)?) << 8)
}

fn u16be(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from(*b.get(i)?) << 8 | u32::from(*b.get(i + 1)?))
}

/// The width and height of a PNG, a JPEG or a WebP, read from its header. `None` when the header does not say.
pub fn image_size(ext: &str, b: &[u8]) -> Option<(u32, u32)> {
    let size = match ext {
        "png" => {
            // the signature, then the IHDR chunk: length, the name, the width and the height as 32-bit big-endian numbers
            if b.get(12..16)? != b"IHDR" {
                return None;
            }
            let be = |i: usize| -> Option<u32> { Some(u16be(b, i)? << 16 | u16be(b, i + 2)?) };
            (be(16)?, be(20)?)
        }
        "jpg" | "jpeg" => {
            let mut i = 2;
            loop {
                // a marker is 0xFF and a code; the frame header (SOF0..SOF15 but DHT, JPG and DAC) carries the size
                if *b.get(i)? != 0xff {
                    return None;
                }
                let code = *b.get(i + 1)?;
                if code == 0xff {
                    i += 1;
                    continue;
                }
                if (0xc0..=0xcf).contains(&code) && !matches!(code, 0xc4 | 0xc8 | 0xcc) {
                    break (u16be(b, i + 7)?, u16be(b, i + 5)?);
                }
                if code == 0xd8 || code == 0x01 || (0xd0..=0xd7).contains(&code) {
                    i += 2;
                    continue;
                }
                i += 2 + usize::try_from(u16be(b, i + 2)?).ok()?;
            }
        }
        "webp" => {
            if b.get(..4)? != b"RIFF" || b.get(8..12)? != b"WEBP" {
                return None;
            }
            match b.get(12..16)? {
                // lossy: a frame header with the start code 9d 01 2a, then the size as two 14-bit numbers
                b"VP8 " if b.get(23..26)? == [0x9d, 0x01, 0x2a] => {
                    (u16le(b, 26)? & 0x3fff, u16le(b, 28)? & 0x3fff)
                }
                // lossless: a signature byte, then width - 1 and height - 1 as two 14-bit numbers
                b"VP8L" if *b.get(20)? == 0x2f => {
                    let bits = u32::from(*b.get(21)?)
                        | u32::from(*b.get(22)?) << 8
                        | u32::from(*b.get(23)?) << 16
                        | u32::from(*b.get(24)?) << 24;
                    ((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1)
                }
                // extended: width - 1 and height - 1 as two 24-bit numbers
                b"VP8X" => {
                    let n = |i: usize| -> Option<u32> {
                        Some(u16le(b, i)? | u32::from(*b.get(i + 2)?) << 16)
                    };
                    (n(24)? + 1, n(27)? + 1)
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    (size.0 > 0 && size.1 > 0).then_some(size)
}

/// Whether the bytes are what the extension of a page file says.
fn well_formed(ext: &str, b: &[u8]) -> bool {
    match ext {
        "ico" => b.starts_with(&[0, 0, 1, 0]),
        "md" | "txt" => std::str::from_utf8(b).is_ok(),
        "json" => serde_json::from_slice::<serde_json::Value>(b).is_ok(),
        "xml" => std::str::from_utf8(b).is_ok_and(|t| {
            t.trim_start_matches('\u{feff}')
                .trim_start()
                .starts_with('<')
        }),
        other => sniff(other, b),
    }
}

/// Reads, checks and hashes every file the page names; fills the card image's size.
pub fn load(ir: &mut IrFile, dir: &Path, program: &Path) -> Result<(), Box<Diagnostic>> {
    let Some(page) = ir.page.as_mut() else {
        return Ok(());
    };
    let root = dir.canonicalize().ok();
    let mut files: Vec<(&mut IrPageFile, bool)> = Vec::new();
    if let Some(i) = page.image.as_mut() {
        files.push((&mut i.file, true));
    }
    if let Some(i) = page.icon.as_mut() {
        files.push((i, false));
    }
    if let Some(i) = page.touch_icon.as_mut() {
        files.push((i, false));
    }
    for a in page.alternates.iter_mut() {
        files.push((&mut a.file, false));
    }
    let mut sizes: Option<(u32, u32)> = None;
    for (f, is_image) in files {
        let full = dir.join(&f.path);
        // a link to a file elsewhere is not the file of the site: the real path stays inside the directory of the program
        let inside = match (&root, full.canonicalize()) {
            (Some(r), Ok(p)) => p.starts_with(r),
            _ => false,
        };
        let meta = std::fs::metadata(&full).ok().filter(|m| m.is_file());
        let Some(meta) = meta.filter(|_| inside) else {
            return Err(refuse(
                "asset-missing",
                format!(
                    "`{}` is not a file inside the directory of the program ({})",
                    f.path,
                    dir.display()
                ),
                f,
                program,
            ));
        };
        if meta.len() > ASSET_MAX_BYTES {
            return Err(refuse(
                "asset-too-large",
                format!(
                    "`{}` is {} bytes; the most a file of the site may be is {} bytes",
                    f.path,
                    meta.len(),
                    ASSET_MAX_BYTES
                ),
                f,
                program,
            ));
        }
        let bytes = std::fs::read(&full).map_err(|e| {
            refuse(
                "asset-missing",
                format!("`{}` cannot be read: {e}", f.path),
                f,
                program,
            )
        })?;
        let ext = f
            .path
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !well_formed(&ext, &bytes) {
            return Err(refuse(
                "asset-type-mismatch",
                format!(
                    "`{}` is not a well formed `.{ext}` file: the extension says one format and the bytes another",
                    f.path
                ),
                f,
                program,
            ));
        }
        if is_image {
            sizes = Some(image_size(&ext, &bytes).ok_or_else(|| {
                refuse(
                    "page-image-size-unreadable",
                    format!(
                        "the width and height of `{}` cannot be read from its header",
                        f.path
                    ),
                    f,
                    program,
                )
            })?);
        }
        f.sha256 = sha256_hex(&bytes);
        f.bytes = bytes.len() as u64;
    }
    if let (Some(i), Some((w, h))) = (page.image.as_mut(), sizes) {
        i.width = w;
        i.height = h;
    }
    Ok(())
}

/// The bytes of every file the page names, to be copied into a build: `(path, bytes)`, without repeats.
pub fn copies(ir: &IrFile, dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    if let Some(p) = &ir.page {
        for (_, f) in cancho_ui_ir::page::files(p) {
            if out.iter().any(|(path, _)| *path == f.path) {
                continue;
            }
            let bytes = std::fs::read(dir.join(&f.path))
                .map_err(|e| format!("reading `{}`: {e}", dir.join(&f.path).display()))?;
            out.push((f.path.clone(), bytes));
        }
    }
    Ok(out)
}

/// What the page generator needs from the program's `page { .. }` and sticky `Header`: the head, the skip link, the title and the language.
pub fn parts(ir: &IrFile) -> PageParts {
    let sticky = cancho_ui_ir::page::has_sticky(ir);
    let motion = crate::motion::uses(ir);
    let Some(p) = &ir.page else {
        return PageParts {
            sticky,
            motion,
            ..PageParts::default()
        };
    };
    PageParts {
        title: Some(p.title.clone()),
        lang: Some(p.lang.clone()),
        head: cancho_ui_ir::page::head_html(p),
        skip: match (&p.skip_link, cancho_ui_ir::page::skip_target(ir)) {
            (Some(text), Some(target)) => cancho_ui_ir::page::skip_html(text, &target),
            _ => String::new(),
        },
        icon: p.icon.is_some(),
        touch_icon: p.touch_icon.is_some(),
        sticky,
        motion,
    }
}

#[cfg(test)]
mod tests {
    use super::image_size;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/page")
                .join(name),
        )
        .unwrap_or_default()
    }

    #[test]
    fn the_size_of_each_format_is_read_from_its_header() {
        // fixtures written by Pillow, which is the oracle and not part of the product
        assert_eq!(image_size("png", &fixture("card.png")), Some((120, 63)));
        assert_eq!(image_size("jpg", &fixture("card.jpg")), Some((90, 40)));
        assert_eq!(image_size("webp", &fixture("lossy.webp")), Some((64, 32)));
        assert_eq!(
            image_size("webp", &fixture("lossless.webp")),
            Some((48, 24))
        );
        assert_eq!(image_size("webp", &fixture("alpha.webp")), Some((33, 17)));
    }

    #[test]
    fn a_header_cut_short_or_wrong_is_no_size_and_never_a_panic() {
        for name in [
            "card.png",
            "card.jpg",
            "lossy.webp",
            "lossless.webp",
            "alpha.webp",
        ] {
            let ext = name.rsplit('.').next().unwrap_or("");
            let full = fixture(name);
            for n in 0..full.len() {
                let _ = image_size(ext, &full[..n]);
            }
        }
        assert_eq!(image_size("png", b"not a png at all, not a png"), None);
        assert_eq!(image_size("gif", &fixture("card.png")), None);
    }
}
