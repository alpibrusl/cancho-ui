//! `theme { .. }` at the command line (docs/theme.md): the font files are read and checked against their declared SHA-256, and the page's
//! theme stylesheet is written. The colours were checked when the program was lowered (`cancho_ui_ir::theme`); this module is where the
//! program meets a directory.
//!
//! SHA-256 is written out here (FIPS 180-4) because no dependency was approved for it (CONTRIBUTING.md); it is checked against the
//! published test vectors below.

use std::fmt::Write as _;
use std::path::Path;

use cancho_ui_ir::ir::{IrFont, IrTheme};

use crate::pipeline::{Diagnostic, import_diag};

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The SHA-256 of `data`, as 64 lowercase hex digits.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = String::with_capacity(64);
    for word in h {
        let _ = write!(out, "{word:08x}");
    }
    out
}

/// What to write beside the page for each rule tag of the theme.
pub fn remediation(code: &str) -> String {
    match code {
        "theme-token-unknown" => "The schemes are `light` and `dark`; the colour tokens are bg, fg, muted, line, surface, soft, ink, accent, accent_fg, accent_soft, good, warn, warn_bg, warn_line, bad, bad_bg, code_bg and code_fg; the fonts are `font body`, `font mono` and `font hand`; a font's keys are family, file, sha256, licence and weight. Each is written once (docs/theme.md section 3).".to_string(),
        "theme-colour-invalid" => "Write a colour as a string of six hex digits, \"#rrggbb\": `accent: \"#185fa5\";`. Names, three digits, alpha and functions are not colours of a theme.".to_string(),
        "theme-scheme-missing" => "Write both `light { .. }` and `dark { .. }`, each with all eighteen tokens: the compiler writes the dark mode from the `dark` block, so there is no default to fall back on.".to_string(),
        "theme-contrast" => "Darken or lighten the text token (or its background) of the pair named in the message until it reaches 4.5:1 (WCAG 2.x 1.4.3). The message gives the ratio it has.".to_string(),
        "theme-duplicate" => "A program has one `theme { .. }`, in its entry file.".to_string(),
        "font-remote" => "A font is a file of this program: `file: \"fonts/x.woff2\"`, copied into the build with its `sha256`. A font by address would be a request to another host.".to_string(),
        "font-licence-missing" => "Write the licence the font file is distributed under: `licence: \"OFL-1.1\";`.".to_string(),
        "font-hash-mismatch" => "`sha256` is the SHA-256 of the font file, 64 hex digits; run `sha256sum` on the file and write what it prints. A file that changed is a change of the program.".to_string(),
        "font-file-missing" => "`file` is a path relative to the directory of the program, naming a file that exists.".to_string(),
        "font-incomplete" => "A font has `family` (letters, digits, spaces, `-`, `_`), `file` (a relative `.woff2` path), `sha256` and `licence`, and may have `weight` (\"400\" or \"200 800\").".to_string(),
        _ => "See docs/theme.md.".to_string(),
    }
}

/// Read each declared font from `dir`, check it against its SHA-256, and return `(path as written, bytes)`.
pub fn load_fonts(
    theme: &IrTheme,
    dir: &Path,
    program: &Path,
) -> Result<Vec<(String, Vec<u8>)>, Box<Diagnostic>> {
    let mut out = Vec::new();
    for f in &theme.fonts {
        let path = dir.join(&f.file);
        let bytes = std::fs::read(&path).map_err(|e| {
            Box::new(import_diag(
                "font-file-missing",
                String::new(),
                format!(
                    "the `{}` font names `{}`, which cannot be read: {e}",
                    f.role, f.file
                ),
                f.span,
                program,
                Some(remediation("font-file-missing")),
            ))
        })?;
        let got = sha256_hex(&bytes);
        if got != f.sha256 {
            return Err(Box::new(import_diag(
                "font-hash-mismatch",
                String::new(),
                format!(
                    "the `{}` font's file `{}` has the SHA-256 {got}, and the program declares {}",
                    f.role, f.file, f.sha256
                ),
                f.span,
                program,
                Some(remediation("font-hash-mismatch")),
            )));
        }
        out.push((f.file.clone(), bytes));
    }
    Ok(out)
}

fn var(name: &str) -> String {
    format!("--cui-{}", name.replace('_', "-"))
}

fn font_stack(f: Option<&IrFont>, fallback: &str) -> Option<String> {
    f.map(|f| format!("\"{}\", {fallback}", f.family))
}

/// The stylesheet of a theme, written after the baseline stylesheet. It sets colours, a font and a focus ring colour and nothing else: it
/// has no rule about size, position, display or the outline's style or width, so the 44 px targets and the ring of the baseline stay the
/// compiler's whatever the tokens are (docs/theme.md section 6).
pub fn css(theme: &IrTheme) -> String {
    let mut s = String::new();
    let set = |name: &str| {
        let scheme = theme.schemes.iter().find(|x| x.name == name);
        scheme
            .map(|x| {
                x.tokens
                    .iter()
                    .map(|(k, v)| format!("{}: {v};", var(k)))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    };
    for f in &theme.fonts {
        let weight = f
            .weight
            .as_ref()
            .map(|w| format!(" font-weight: {w};"))
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "@font-face {{ font-family: \"{}\"; font-style: normal; font-display: swap;{weight} src: url(\"{}\") format(\"woff2\"); }}",
            f.family, f.file
        );
    }
    let find = |role: &str| theme.fonts.iter().find(|f| f.role == role);
    let mut fonts = String::new();
    if let Some(b) = font_stack(
        find("body"),
        "system-ui, -apple-system, \"Segoe UI\", sans-serif",
    ) {
        let _ = write!(fonts, " --cui-font: {b};");
    }
    if let Some(m) = font_stack(
        find("mono"),
        "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
    ) {
        let _ = write!(fonts, " --cui-mono: {m};");
    }
    if let Some(h) = font_stack(find("hand"), "cursive") {
        let _ = write!(fonts, " --cui-hand: {h};");
    }
    let _ = writeln!(
        s,
        ":root {{ {}{fonts} --cui-chart: var(--cui-accent); --cui-chart-open: var(--cui-muted); }}",
        set("light")
    );
    let _ = writeln!(
        s,
        "@media (prefers-color-scheme: dark) {{ :root {{ {} }} }}",
        set("dark")
    );
    s.push_str("body { background: var(--cui-bg); color: var(--cui-fg); }\n");
    if find("body").is_some() {
        s.push_str("body { font-family: var(--cui-font); }\n");
    }
    if find("mono").is_some() {
        s.push_str("code, pre, kbd, samp { font-family: var(--cui-mono); }\n");
    }
    s.push_str("a { color: var(--cui-accent); }\n");
    s.push_str("button, input:not([type=\"checkbox\"]):not([type=\"file\"]), textarea, select { background: var(--cui-surface); color: var(--cui-fg); border-color: var(--cui-muted); }\n");
    s.push_str(".cui-table th, .cui-table td { border-bottom-color: var(--cui-line); }\n");
    s.push_str("[data-tone=\"ok\"] { color: var(--cui-good); }\n");
    s.push_str(
        "[data-tone=\"warn\"] { color: var(--cui-warn); background: var(--cui-warn-bg); }\n",
    );
    s.push_str("[data-tone=\"error\"] { color: var(--cui-bad); background: var(--cui-bad-bg); }\n");
    s.push_str("[data-style=\"eyebrow\"], [data-style=\"label\"] { color: var(--cui-accent); }\n");
    s.push_str("[data-style=\"lead\"], [data-style=\"note\"] { color: var(--cui-muted); }\n");
    s.push_str(
        "a.cui-link-primary { background: var(--cui-accent); color: var(--cui-accent-fg); }\n",
    );
    s.push_str(
        "a.cui-link-secondary { color: var(--cui-accent); border-color: var(--cui-accent); }\n",
    );
    s.push_str("a.cui-link-quiet { color: var(--cui-muted); }\n");
    s.push_str(
        ".cui-sticky { background: var(--cui-bg); border-bottom-color: var(--cui-line); }\n",
    );
    s.push_str("a.cui-skip { background: var(--cui-accent); color: var(--cui-accent-fg); }\n");
    s.push_str(".cui-code { background: var(--cui-code-bg); color: var(--cui-code-fg); }\n");
    s.push_str(":focus-visible { outline: 3px solid var(--cui-accent); outline-offset: 2px; }\n");
    s
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_matches_the_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // a message of 55, 56 and 64 bytes crosses the padding boundaries
        for (n, want) in [
            (
                55usize,
                "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
            ),
            (
                56,
                "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
            ),
            (
                64,
                "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
            ),
        ] {
            assert_eq!(sha256_hex(&vec![b'a'; n]), want, "{n} bytes");
        }
        // a million `a`
        assert_eq!(
            sha256_hex(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
