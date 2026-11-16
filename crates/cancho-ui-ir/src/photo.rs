//! `PhotoField` and `Photo` (docs/structure.md 12.0r): a picture chosen by the person, shrunk by the page to a base64 JPEG that fits a number of characters,
//! and a picture drawn from such a text held in the program's state.
//!
//! * **`PhotoField { label: ".." icon: "camera" square: true max_side: 1024 max_chars: 48000 on: Chosen }`** is a native file chooser
//!   (`<input type=file accept=image/jpeg,image/png,image/webp>`), a user gesture and no capability. The page decodes the file (the EXIF orientation applied),
//!   cuts the middle of it (a square, or keeping the picture's proportions), and draws it again as a JPEG, smaller until the base64 text fits `max_chars:`. Drawing
//!   again is what takes the EXIF block (the place and time a camera wrote) out: the text the program gets has none. The event is given one JSON `String`:
//!   `{"data":"<base64>"}` or `{"error":"too big"}` or `{"error":"not a picture"}`.
//! * **`Photo { label: ".." data: picture ratio: "16:9" tone: colour }`** draws such a text on a canvas, with the decoder, the checks and the cache of `face(..)`
//!   (docs/structure.md 12.0l): no `<img>`, no `data:` address and no request. Until it is drawn, or when the text is not a picture, it is a gradient of the
//!   tone (twelve fixed colours) or a neutral one. `decorative: true` instead of a `label:` hides it from a screen reader.
//! * Both are drawn by the page's script (a state-reading element is a hole of the prerendered skeleton); the code of each is emitted only in a program that has it.

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The fewest and the most characters of a picture text: the same as the photo of `face(..)`, so that a `Photo` draws what a `face` draws.
pub const CHARS_MIN: i64 = 100;
pub const CHARS_MAX: i64 = 48_000;
/// What `max_chars:` is when it is not written: the most.
pub const CHARS_DEFAULT: i64 = CHARS_MAX;
/// The longest side of the picture the page makes, `max_side:`: from 32 to 4096 pixels, 1024 when it is not written.
pub const SIDE_MIN: i64 = 32;
pub const SIDE_MAX: i64 = 4096;
pub const SIDE_DEFAULT: i64 = 1024;
/// The words of `{"error": ..}`.
pub const ERRORS: &[&str] = &["too big", "not a picture"];
/// The types the chooser offers, and the ones the page accepts (a file whose type is none of them, and not empty, is `not a picture`).
pub const PICTURE_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp"];
/// The tones of a `Photo` and of `face(..)`: the twelve fixed colours (docs/structure.md 12.0l).
pub const TONES: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"];
/// What a `PhotoField` takes beside `label:`.
pub const FIELD_KEYS: &[&str] = &["icon", "square", "max_side", "max_chars", "on"];
/// What a `Photo` takes beside `label:`.
pub const PHOTO_KEYS: &[&str] = &["data", "ratio", "tone", "decorative"];
/// The icon of a `PhotoField` when `icon:` is not written.
pub const ICON_DEFAULT: &str = "camera";
/// The ratio of a `Photo` when `ratio:` is not written.
pub const RATIO_DEFAULT: &str = "1:1";
/// The most either number of a `ratio:` may be.
pub const RATIO_MAX: u32 = 32;

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

fn string_of(b: &IrBinding) -> Option<&str> {
    match &b.value {
        IrBindingValue::String(s) => Some(s),
        _ => None,
    }
}

fn bool_of(b: &IrBinding) -> Option<bool> {
    match &b.value {
        IrBindingValue::Const { expr, .. } => match **expr {
            IrExpr::BoolLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    }
}

fn int_of(b: &IrBinding) -> Option<i64> {
    match &b.value {
        IrBindingValue::Const { expr, .. } => match **expr {
            IrExpr::IntLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    }
}

fn find<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrBinding> {
    bindings.iter().find(|b| b.key == key)
}

/// `"W:H"`: two whole numbers from 1 to `RATIO_MAX`, plain digits.
pub fn parse_ratio(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once(':')?;
    let num = |t: &str| {
        if t.is_empty()
            || t.len() > 2
            || t.starts_with('0')
            || !t.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        t.parse::<u32>()
            .ok()
            .filter(|n| (1..=RATIO_MAX).contains(n))
    };
    Some((num(w)?, num(h)?))
}

/// The ratio a `Photo` draws at: `ratio:` or the default.
pub fn ratio_of(bindings: &[IrBinding]) -> (u32, u32) {
    find(bindings, "ratio")
        .and_then(string_of)
        .and_then(parse_ratio)
        .unwrap_or((1, 1))
}

/// Whether a text is one a `Photo` (and a `face`) draws: base64 (only its alphabet, `=` at most twice and only at the end) that starts like a JPEG (`/9j/`),
/// `CHARS_MIN` to `CHARS_MAX` characters. The page's script has its own copy of this (`__cui_photo_ok`); `tests/photo_corpus.json` is run against both.
pub fn picture_ok(text: &str) -> bool {
    let n = text.len();
    if !(CHARS_MIN as usize..=CHARS_MAX as usize).contains(&n) || !text.starts_with("/9j/") {
        return false;
    }
    let body = text.trim_end_matches('=');
    text.len() - body.len() <= 2
        && !body.is_empty()
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// What one `PhotoField` promises about the value it gives its event.
#[derive(Debug, Clone, PartialEq)]
pub struct PhotoSpec {
    pub max_chars: i64,
}

/// Is `value` (the text a handler receives) one a `PhotoField` with any of these specs could have produced? `Err` says why not.
pub fn check_value(value: &str, specs: &[PhotoSpec]) -> Result<(), String> {
    let json: serde_json::Value =
        serde_json::from_str(value).map_err(|e| format!("it is not JSON: {e}"))?;
    let obj = json.as_object().ok_or("it is not a JSON object")?;
    if obj.len() != 1 {
        return Err("it has exactly one of `data` or `error`".into());
    }
    if let Some(error) = obj.get("error") {
        let word = error
            .as_str()
            .filter(|w| ERRORS.contains(w))
            .ok_or_else(|| format!("`error` is one of {}", ERRORS.join(", ")))?;
        let _ = word;
        return Ok(());
    }
    let data = obj
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or("it has exactly one of `data` (a string) or `error`")?;
    if !picture_ok(data) {
        return Err(
            "`data` is base64 of a JPEG (it starts `/9j/`), 100 to 48,000 characters".into(),
        );
    }
    if !specs.iter().any(|s| (data.len() as i64) <= s.max_chars) {
        return Err(
            "`data` is longer than `max_chars`, so the page would have said `too big`".into(),
        );
    }
    Ok(())
}

/// Checks one binding. `Ok(true)`: this module owns it and it is fine; `Ok(false)`: not its business (or, for `on:`, the event check's).
pub fn check_binding(
    c: &IrComponent,
    kind: &IrElementKind,
    b: &IrBinding,
) -> Result<bool, LowerError> {
    match kind {
        IrElementKind::PhotoField => match b.key.as_str() {
            "icon" => match string_of(b) {
                Some(n) if crate::icon::known(n) => Ok(true),
                _ => Err(crate::icon::unknown(string_of(b).unwrap_or(""), b.span)),
            },
            "square" => match bool_of(b) {
                Some(_) => Ok(true),
                None => Err(refuse(
                    "photo-square-invalid",
                    "`square:` is `true` (the middle square of the picture) or `false` (the whole picture, in its proportions), written in the program".to_string(),
                    b.span,
                )),
            },
            "max_side" => match int_of(b) {
                Some(n) if (SIDE_MIN..=SIDE_MAX).contains(&n) => Ok(true),
                _ => Err(refuse(
                    "photo-limit-invalid",
                    format!("`max_side:` takes an integer literal from {SIDE_MIN} to {SIDE_MAX}: the longest side, in pixels, of the picture the page makes"),
                    b.span,
                )),
            },
            "max_chars" => match int_of(b) {
                Some(n) if (CHARS_MIN..=CHARS_MAX).contains(&n) => Ok(true),
                _ => Err(refuse(
                    "photo-limit-invalid",
                    format!("`max_chars:` takes an integer literal from {CHARS_MIN} to {CHARS_MAX}: the most characters of the base64 text the page gives the program"),
                    b.span,
                )),
            },
            "on" => Ok(false),
            other => Err(refuse(
                "binding-unknown",
                format!(
                    "`{other}:` is not a binding of PhotoField; it takes label, {}",
                    FIELD_KEYS.join(", ")
                ),
                b.span,
            )),
        },
        IrElementKind::Photo => match b.key.as_str() {
            "data" => match &b.value {
                IrBindingValue::Const { expr, .. } => {
                    if let IrExpr::Ident { name, .. } = &**expr
                        && let Some(f) = c.state.iter().find(|f| &f.name == name)
                        && f.ty != IrType::String
                    {
                        return Err(refuse(
                            "photo-data-invalid",
                            format!("`data: {name}` is a state field that is not a `String`: a Photo draws the base64 text of a picture"),
                            b.span,
                        ));
                    }
                    Ok(true)
                }
                _ => Err(refuse(
                    "photo-data-invalid",
                    "`data:` of a Photo is a `String` the view reads (a state field, or a name of a `For`): `Photo { label: \"Cover\" data: cover }`".to_string(),
                    b.span,
                )),
            },
            "tone" => match &b.value {
                IrBindingValue::Const { expr, .. } => match &**expr {
                    IrExpr::StringLit { value, .. } if !TONES.contains(&value.as_str()) => {
                        Err(refuse(
                            "photo-tone-invalid",
                            format!("`tone: {value:?}` is not a tone: \"0\" to \"11\" (the twelve colours of `face(..)`), or a `String` the view reads"),
                            b.span,
                        ))
                    }
                    IrExpr::Ident { name, .. }
                        if c.state
                            .iter()
                            .any(|f| &f.name == name && f.ty != IrType::String) =>
                    {
                        Err(refuse(
                            "photo-tone-invalid",
                            format!("`tone: {name}` is a state field that is not a `String` (its value is \"0\" to \"11\")"),
                            b.span,
                        ))
                    }
                    _ => Ok(true),
                },
                _ => Err(refuse(
                    "photo-tone-invalid",
                    "`tone:` of a Photo is \"0\" to \"11\" or a `String` the view reads".to_string(),
                    b.span,
                )),
            },
            "ratio" => match string_of(b) {
                Some(r) if parse_ratio(r).is_some() => Ok(true),
                _ => Err(refuse(
                    "photo-ratio-invalid",
                    format!("`ratio:` is a string literal \"W:H\" of two whole numbers from 1 to {RATIO_MAX}, like \"16:9\""),
                    b.span,
                )),
            },
            "decorative" => match bool_of(b) {
                Some(_) => Ok(true),
                None => Err(refuse(
                    "photo-decorative-invalid",
                    "`decorative:` is `true` or `false`, written in the program".to_string(),
                    b.span,
                )),
            },
            other => Err(refuse(
                "binding-unknown",
                format!(
                    "`{other}:` is not a binding of Photo; it takes label, {}",
                    PHOTO_KEYS.join(", ")
                ),
                b.span,
            )),
        },
        _ => {
            if matches!(b.key.as_str(), "square" | "max_side" | "max_chars") {
                return Err(refuse(
                    "binding-unknown",
                    format!("`{}:` is a binding of PhotoField, not of this element", b.key),
                    b.span,
                ));
            }
            if b.key == "data" {
                return Err(refuse(
                    "binding-unknown",
                    "`data:` is a binding of Photo, not of this element".to_string(),
                    b.span,
                ));
            }
            Ok(false)
        }
    }
}

/// What a `PhotoField` and a `Photo` need to be complete.
pub fn check_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
    in_for: bool,
) -> Result<(), LowerError> {
    let named = accessibility
        .label
        .as_deref()
        .is_some_and(|l| !l.trim().is_empty())
        || accessibility.label_expr.is_some();
    match kind {
        IrElementKind::PhotoField => {
            if !named || find(bindings, "on").is_none() {
                return Err(refuse(
                    "photo-field-incomplete",
                    "a PhotoField needs `label:` (its name, read aloud and shown under the icon) and `on:`, the event that is told the picture: `PhotoField { label: \"Photo\" on: Chosen }`".to_string(),
                    span,
                ));
            }
            if !children.is_empty() {
                return Err(refuse(
                    "photo-has-children",
                    "a PhotoField has no content of its own: it is an icon over its label"
                        .to_string(),
                    span,
                ));
            }
            if in_for {
                return Err(refuse(
                    "photo-field-in-loop",
                    "a PhotoField is not drawn inside a `For`: the page keeps what was chosen by the place of the field in the source, and every row would share it".to_string(),
                    span,
                ));
            }
            Ok(())
        }
        IrElementKind::Photo => {
            if find(bindings, "data").is_none() {
                return Err(refuse(
                    "photo-incomplete",
                    "a Photo needs `data:`, the `String` that holds the picture: `Photo { label: \"Cover\" data: cover }`".to_string(),
                    span,
                ));
            }
            if !children.is_empty() {
                return Err(refuse(
                    "photo-has-children",
                    "a Photo has no content of its own: it is drawn from `data:`".to_string(),
                    span,
                ));
            }
            let decorative = find(bindings, "decorative").and_then(bool_of) == Some(true);
            if decorative && (accessibility.label.is_some() || accessibility.label_expr.is_some()) {
                return Err(refuse(
                    "photo-label-conflict",
                    "a Photo is `decorative: true` or has a `label:`, not both: the first is hidden from a screen reader, the second names it".to_string(),
                    span,
                ));
            }
            if !decorative && !named {
                return Err(refuse(
                    "photo-label-missing",
                    "a Photo needs `label:` (what it shows, read aloud) or `decorative: true` (words beside it say it): `Photo { label: \"The lake\" data: cover }`".to_string(),
                    span,
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn any_kind(ir: &IrFile, pred: fn(&IrElementKind) -> bool) -> bool {
    fn walk(node: &IrViewNode, pred: fn(&IrElementKind) -> bool) -> bool {
        match node {
            IrViewNode::Element { kind, children, .. } => {
                pred(kind) || children.iter().any(|c| walk(c, pred))
            }
            IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                children.iter().any(|c| walk(c, pred))
            }
            IrViewNode::Text { .. } => false,
        }
    }
    ir.components.iter().any(|c| walk(&c.view.body, pred))
}

/// Whether any view has a `PhotoField` or a `Photo`: the rules and the script of the pictures are written for a program that does, so every other page is the bytes it was.
pub fn uses_photo(ir: &IrFile) -> bool {
    any_kind(ir, |k| {
        matches!(k, IrElementKind::PhotoField | IrElementKind::Photo)
    })
}

/// Whether any view has a `PhotoField`: it draws an icon, so the page carries the rules and the licence of the icons.
pub fn uses_photo_field(ir: &IrFile) -> bool {
    any_kind(ir, |k| matches!(k, IrElementKind::PhotoField))
}

/// The icon of a `PhotoField`: `icon:` or the default.
pub fn icon_of(bindings: &[IrBinding]) -> &str {
    find(bindings, "icon")
        .and_then(string_of)
        .unwrap_or(ICON_DEFAULT)
}

/// The `max_side:`, `max_chars:` and `square:` of a `PhotoField`, with the defaults.
pub fn limits_of(bindings: &[IrBinding]) -> (i64, i64, bool) {
    (
        find(bindings, "max_side")
            .and_then(int_of)
            .unwrap_or(SIDE_DEFAULT),
        find(bindings, "max_chars")
            .and_then(int_of)
            .unwrap_or(CHARS_DEFAULT),
        find(bindings, "square").and_then(bool_of).unwrap_or(false),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pic(n: usize) -> String {
        let mut s = String::from("/9j/");
        while s.len() < n {
            s.push('A');
        }
        s
    }

    #[test]
    fn ratios() {
        for ok in ["1:1", "16:9", "32:32", "4:3", "9:16", "1:32"] {
            assert!(parse_ratio(ok).is_some(), "{ok}");
        }
        for bad in [
            "", "16", "16:", ":9", "0:1", "1:0", "33:1", "1:33", "016:9", "a:b", "16:9:1", "16 :9",
            "-1:2", "+1:2", "1.5:1", "01:1", "9:09", "00:5",
        ] {
            assert!(parse_ratio(bad).is_none(), "{bad:?}");
        }
        assert_eq!(parse_ratio("16:9"), Some((16, 9)));
    }

    #[test]
    fn what_a_picture_text_is() {
        assert!(picture_ok(&pic(100)), "100 is the least");
        assert!(!picture_ok(&pic(99)));
        assert!(picture_ok(&pic(48_000)), "48,000 is the most");
        assert!(!picture_ok(&pic(48_001)));
        assert!(
            !picture_ok(&pic(200).replacen("/9j/", "iVBO", 1)),
            "not a JPEG start"
        );
        assert!(
            !picture_ok(&format!("{}!", pic(200))),
            "outside the alphabet"
        );
        assert!(!picture_ok(&format!("{} ", pic(200))), "a space");
        assert!(!picture_ok(&format!("{}\n", pic(200))), "a newline");
        assert!(picture_ok(&format!("{}==", pic(200))), "padding at the end");
        assert!(
            !picture_ok(&format!("{}===", pic(200))),
            "three padding signs"
        );
        assert!(
            !picture_ok(&format!("{}=A", pic(200))),
            "padding in the middle"
        );
        assert!(
            !picture_ok(&pic(200).replacen("AAAA", "AA\u{e9}A", 1)),
            "not ASCII"
        );
    }

    #[test]
    fn values() {
        let specs = [PhotoSpec { max_chars: 300 }];
        let data = |n: usize| format!("{{\"data\":\"{}\"}}", pic(n));
        assert!(check_value(&data(300), &specs).is_ok(), "exactly the cap");
        assert!(check_value(&data(301), &specs).is_err(), "one over the cap");
        assert!(
            check_value(&data(99), &specs).is_err(),
            "too short to be a picture"
        );
        assert!(check_value(r#"{"error":"too big"}"#, &specs).is_ok());
        assert!(check_value(r#"{"error":"not a picture"}"#, &specs).is_ok());
        assert!(check_value(r#"{"error":"unreadable"}"#, &specs).is_err());
        assert!(check_value(r#"{"error":"too big","data":"x"}"#, &specs).is_err());
        assert!(check_value(r#"{}"#, &specs).is_err());
        assert!(check_value("", &specs).is_err());
        assert!(check_value(r#"["data"]"#, &specs).is_err());
        assert!(check_value(r#"{"data":5}"#, &specs).is_err());
    }
}
