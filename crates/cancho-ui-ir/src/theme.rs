//! `theme { light { .. } dark { .. } font body { .. } }` (docs/theme.md): the sixteen colour tokens of each of two schemes, checked for contrast,
//! and the fonts a program declares. Pure: nothing here reads a file (the font files are checked by the CLI, which has a directory).

use cancho_ui_syntax::Span;
use cancho_ui_syntax::ast::{ThemeBlock, ThemeDecl, ThemeEntry};

use crate::ir::{IrFont, IrScheme, IrTheme};
use crate::lower::LowerError;

/// The closed set of colour tokens, in the order they are written to the page and to `diff`.
pub const TOKENS: [&str; 16] = [
    "bg",
    "fg",
    "muted",
    "line",
    "surface",
    "soft",
    "ink",
    "accent",
    "accent_fg",
    "accent_soft",
    "good",
    "warn",
    "warn_bg",
    "warn_line",
    "bad",
    "bad_bg",
];

/// The pairs whose contrast is checked in each scheme: (text token, background token). WCAG 2.x 1.4.3 asks 4.5:1 for text. `line`, `ink` and
/// `warn_line` are decoration: no text is set in them by the baseline stylesheet, so they have no pair (docs/theme.md section 4).
pub const PAIRS: [(&str, &str); 13] = [
    ("fg", "bg"),
    ("fg", "surface"),
    ("muted", "bg"),
    ("muted", "surface"),
    ("muted", "soft"),
    ("accent", "bg"),
    ("accent", "surface"),
    ("accent", "accent_soft"),
    ("accent_fg", "accent"),
    ("good", "bg"),
    ("warn", "warn_bg"),
    ("bad", "bad_bg"),
    ("bad", "bg"),
];

pub const MIN_RATIO: f64 = 4.5;
pub const SCHEMES: [&str; 2] = ["light", "dark"];
pub const FONT_ROLES: [&str; 3] = ["body", "mono", "hand"];

/// `"#rrggbb"` (six hex digits, either case) as three channels.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let digits = s.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([ch(0)?, ch(2)?, ch(4)?])
}

/// WCAG 2.x relative luminance of an sRGB colour.
pub fn luminance(c: [u8; 3]) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG 2.x contrast ratio of two colours, from 1 to 21.
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// A ratio written with two decimals, cut and not rounded: 4.499 is shown as 4.49, never as the 4.50 it fails to reach.
pub fn ratio_text(r: f64) -> String {
    format!("{:.2}", (r * 100.0).floor() / 100.0)
}

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

fn is_remote(file: &str) -> bool {
    let f = file.trim().to_ascii_lowercase();
    f.contains("://") || f.starts_with("//") || f.starts_with("data:") || f.starts_with("http")
}

/// A relative path of letters, digits, `.`, `_`, `-` and `/` inside the program's directory, ending in `.woff2`.
fn usable_file(file: &str) -> bool {
    !file.is_empty()
        && file.len() <= 200
        && file.ends_with(".woff2")
        && !file.starts_with('/')
        && file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
        && !file
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
}

fn usable_family(f: &str) -> bool {
    !f.is_empty()
        && f.len() <= 64
        && f.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'_' | b'-'))
}

/// `400` or `200 800`: one weight, or a range of two, each 1 to 1000.
fn usable_weight(w: &str) -> bool {
    let parts: Vec<&str> = w.split(' ').collect();
    (parts.len() == 1 || parts.len() == 2)
        && parts
            .iter()
            .all(|p| p.parse::<u16>().is_ok_and(|n| (1..=1000).contains(&n)))
}

fn usable_licence(l: &str) -> bool {
    !l.is_empty()
        && l.len() <= 40
        && l.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'))
}

fn is_sha256(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())
}

fn entry<'a>(entries: &'a [ThemeEntry], key: &str) -> Option<&'a ThemeEntry> {
    entries.iter().find(|e| e.key == key)
}

/// Lower one `theme { .. }` or refuse it with a rule tag (docs/theme.md section 5).
pub fn lower_theme(decl: &ThemeDecl) -> Result<IrTheme, LowerError> {
    let mut schemes: Vec<IrScheme> = Vec::new();
    let mut fonts: Vec<IrFont> = Vec::new();
    for block in &decl.blocks {
        match block {
            ThemeBlock::Scheme(s) => {
                if !SCHEMES.contains(&s.name.as_str()) {
                    return Err(refuse(
                        "theme-token-unknown",
                        format!(
                            "`{}` is not a scheme of a theme; the schemes are `light` and `dark`",
                            s.name
                        ),
                        s.span,
                    ));
                }
                if schemes.iter().any(|x| x.name == s.name) {
                    return Err(refuse(
                        "theme-token-unknown",
                        format!("the `{}` scheme is written twice", s.name),
                        s.span,
                    ));
                }
                let mut tokens: Vec<(String, String)> = Vec::new();
                for e in &s.entries {
                    if !TOKENS.contains(&e.key.as_str()) {
                        return Err(refuse(
                            "theme-token-unknown",
                            format!(
                                "`{}` is not a colour token; the tokens are {}",
                                e.key,
                                TOKENS.join(", ")
                            ),
                            e.span,
                        ));
                    }
                    if tokens.iter().any(|(k, _)| *k == e.key) {
                        return Err(refuse(
                            "theme-token-unknown",
                            format!("`{}` is given twice in `{}`", e.key, s.name),
                            e.span,
                        ));
                    }
                    let Some(rgb) = parse_hex(&e.value) else {
                        return Err(refuse(
                            "theme-colour-invalid",
                            format!(
                                "`{}: {:?}` in `{}` is not a colour; a colour is six hex digits, \"#rrggbb\"",
                                e.key, e.value, s.name
                            ),
                            e.span,
                        ));
                    };
                    tokens.push((
                        e.key.clone(),
                        format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
                    ));
                }
                let missing: Vec<&str> = TOKENS
                    .iter()
                    .copied()
                    .filter(|t| !tokens.iter().any(|(k, _)| k == t))
                    .collect();
                if !missing.is_empty() {
                    return Err(refuse(
                        "theme-scheme-missing",
                        format!(
                            "the `{}` scheme lacks {} token{}: {}",
                            s.name,
                            missing.len(),
                            if missing.len() == 1 { "" } else { "s" },
                            missing.join(", ")
                        ),
                        s.span,
                    ));
                }
                tokens
                    .sort_by_key(|(k, _)| TOKENS.iter().position(|t| t == k).unwrap_or(usize::MAX));
                schemes.push(IrScheme {
                    name: s.name.clone(),
                    tokens,
                });
            }
            ThemeBlock::Font(f) => {
                if !FONT_ROLES.contains(&f.role.as_str()) {
                    return Err(refuse(
                        "theme-token-unknown",
                        format!(
                            "`{}` is not a font role; the roles are body, mono and hand",
                            f.role
                        ),
                        f.span,
                    ));
                }
                if fonts.iter().any(|x| x.role == f.role) {
                    return Err(refuse(
                        "theme-token-unknown",
                        format!("the `{}` font is written twice", f.role),
                        f.span,
                    ));
                }
                for e in &f.entries {
                    if !["family", "file", "sha256", "licence", "weight"].contains(&e.key.as_str())
                    {
                        return Err(refuse(
                            "theme-token-unknown",
                            format!(
                                "`{}` is not a key of a font; the keys are family, file, sha256, licence and weight",
                                e.key
                            ),
                            e.span,
                        ));
                    }
                    if f.entries.iter().filter(|x| x.key == e.key).count() > 1 {
                        return Err(refuse(
                            "theme-token-unknown",
                            format!("`{}` is given twice in the `{}` font", e.key, f.role),
                            e.span,
                        ));
                    }
                }
                // a font by address is refused before anything else: the page makes no request to another host
                if let Some(e) = entry(&f.entries, "file").filter(|e| is_remote(&e.value)) {
                    return Err(refuse(
                        "font-remote",
                        format!(
                            "the `{}` font names an address ({:?}); a font is a file of this program, declared with its sha256, never fetched from another host",
                            f.role, e.value
                        ),
                        e.span,
                    ));
                }
                let need = |key: &str| {
                    entry(&f.entries, key).ok_or_else(|| {
                        refuse(
                            "font-incomplete",
                            format!("the `{}` font lacks `{key}`", f.role),
                            f.span,
                        )
                    })
                };
                let family = need("family")?;
                let file = need("file")?;
                let sha = need("sha256")?;
                if !usable_family(&family.value) {
                    return Err(refuse(
                        "font-incomplete",
                        format!(
                            "`family: {:?}` is 1 to 64 letters, digits, spaces, `-` and `_`",
                            family.value
                        ),
                        family.span,
                    ));
                }
                if !usable_file(&file.value) {
                    return Err(refuse(
                        "font-incomplete",
                        format!(
                            "`file: {:?}` is a relative path to a `.woff2` file inside the program's directory (letters, digits, `.`, `_`, `-`, `/`; no `..`)",
                            file.value
                        ),
                        file.span,
                    ));
                }
                let weight = entry(&f.entries, "weight");
                if let Some(w) = weight.filter(|w| !usable_weight(&w.value)) {
                    return Err(refuse(
                        "font-incomplete",
                        format!(
                            "`weight: {:?}` is one weight or two, \"400\" or \"200 800\"",
                            w.value
                        ),
                        w.span,
                    ));
                }
                let licence = entry(&f.entries, "licence").filter(|l| usable_licence(&l.value));
                let Some(licence) = licence else {
                    return Err(refuse(
                        "font-licence-missing",
                        format!(
                            "the `{}` font declares no licence; write the licence it is distributed under, as an identifier such as \"OFL-1.1\"",
                            f.role
                        ),
                        entry(&f.entries, "licence").map_or(f.span, |l| l.span),
                    ));
                };
                if !is_sha256(&sha.value) {
                    return Err(refuse(
                        "font-hash-mismatch",
                        format!("`sha256: {:?}` is not a SHA-256 (64 hex digits)", sha.value),
                        sha.span,
                    ));
                }
                fonts.push(IrFont {
                    role: f.role.clone(),
                    family: family.value.clone(),
                    file: file.value.clone(),
                    sha256: sha.value.to_ascii_lowercase(),
                    licence: licence.value.clone(),
                    weight: weight.map(|w| w.value.clone()),
                    span: f.span,
                });
            }
        }
    }
    for want in SCHEMES {
        if !schemes.iter().any(|s| s.name == want) {
            return Err(refuse(
                "theme-scheme-missing",
                format!("the theme has no `{want}` scheme; it needs both `light` and `dark`"),
                decl.span,
            ));
        }
    }
    // contrast: every pair of every scheme, all failures in one message
    let mut failing: Vec<String> = Vec::new();
    for want in SCHEMES {
        let Some(scheme) = schemes.iter().find(|s| s.name == want) else {
            continue;
        };
        let get = |t: &str| {
            scheme
                .tokens
                .iter()
                .find(|(k, _)| k == t)
                .and_then(|(_, v)| parse_hex(v))
        };
        for (fg, bg) in PAIRS {
            if let (Some(a), Some(b)) = (get(fg), get(bg)) {
                let r = contrast_ratio(a, b);
                if r < MIN_RATIO {
                    failing.push(format!(
                        "{want}: {fg} on {bg} is {}:1 (needs {MIN_RATIO}:1)",
                        ratio_text(r)
                    ));
                }
            }
        }
    }
    if !failing.is_empty() {
        return Err(refuse("theme-contrast", failing.join("; "), decl.span));
    }
    schemes.sort_by_key(|s| SCHEMES.iter().position(|n| *n == s.name));
    fonts.sort_by_key(|f| FONT_ROLES.iter().position(|n| *n == f.role));
    Ok(IrTheme {
        schemes,
        fonts,
        span: decl.span,
    })
}

/// The theme as canonical JSON (keys sorted by `serde_json`), for `inspect ir` and for the diff.
pub fn to_json(t: &IrTheme) -> serde_json::Value {
    let schemes: Vec<serde_json::Value> = t
        .schemes
        .iter()
        .map(|s| {
            let tokens: serde_json::Map<String, serde_json::Value> = s
                .tokens
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            serde_json::json!({ "name": s.name, "tokens": tokens })
        })
        .collect();
    let fonts: Vec<serde_json::Value> = t
        .fonts
        .iter()
        .map(|f| {
            let mut v = serde_json::json!({
                "role": f.role, "family": f.family, "file": f.file, "sha256": f.sha256, "licence": f.licence
            });
            if let (Some(w), Some(o)) = (&f.weight, v.as_object_mut()) {
                o.insert("weight".into(), serde_json::Value::String(w.clone()));
            }
            v
        })
        .collect();
    serde_json::json!({ "schemes": schemes, "fonts": fonts })
}

/// Every setting of a theme as `(name, value)`, for the diff: `light.bg`, `dark.accent`, `font.body.sha256`.
pub fn flatten(t: &IrTheme) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for s in &t.schemes {
        for (k, v) in &s.tokens {
            out.push((format!("{}.{k}", s.name), v.clone()));
        }
    }
    for f in &t.fonts {
        let r = &f.role;
        out.push((format!("font.{r}.family"), f.family.clone()));
        out.push((format!("font.{r}.file"), f.file.clone()));
        out.push((format!("font.{r}.sha256"), f.sha256.clone()));
        out.push((format!("font.{r}.licence"), f.licence.clone()));
        if let Some(w) = &f.weight {
            out.push((format!("font.{r}.weight"), w.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_21() {
        assert!((contrast_ratio([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio([255, 255, 255], [255, 255, 255]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_ratio_is_cut_not_rounded() {
        assert_eq!(ratio_text(4.499), "4.49");
        assert_eq!(ratio_text(4.5), "4.50");
    }

    #[test]
    fn hex_is_six_digits_with_a_hash() {
        assert_eq!(parse_hex("#F7f4Eb"), Some([0xf7, 0xf4, 0xeb]));
        for bad in [
            "f7f4eb", "#f7f4e", "#f7f4eb0", "#fff", "red", "#gggggg", "", "#",
        ] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }
}
