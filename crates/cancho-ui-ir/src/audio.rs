//! `AudioField` and `AudioClip` (docs/structure.md 12.0t): a short voice note recorded by the person and kept in state as base64 text, and the same text played back.
//!
//! * **`AudioField { label: "Voice note" icon: "mic" max_seconds: 15 max_chars: 48000 on: Recorded }`** is a key that records. A tap asks the browser for the microphone (the
//!   browser's own prompt: the page needs no capability, a user gesture and the person's yes), records with `MediaRecorder` until the person taps again, `max_seconds:` have gone, or the
//!   base64 text would pass `max_chars:`, and gives the event one JSON `String`: `{"data":"<base64>","seconds":7,"mime":"audio/webm"}` or `{"error":"denied"}` (and `unsupported`,
//!   `no microphone`, `failed`, `too big`). The status line says what is happening in words, so the person is never left guessing.
//! * **`AudioClip { label: "Voice note from Ana" data: clip }`** plays such a text: a play and pause key, a progress bar and the time. The sound is decoded by the browser's audio engine
//!   (`decodeAudioData`) and played from there: **no `<audio>`, no `blob:` or `data:` address and no request**, so the page's policy needs nothing for it. One clip plays at a time, and a
//!   clip keeps playing through the renders every state change makes.
//! * The text is base64 of WebM, Ogg, MP4 or WAV, told by its first bytes (never by what a sender says), 100 to 48,000 characters: the same closed list and limits the server of the
//!   console keeps.

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The fewest and the most characters of a recording text.
pub const CHARS_MIN: i64 = 100;
pub const CHARS_MAX: i64 = 48_000;
/// What `max_chars:` is when it is not written: the most.
pub const CHARS_DEFAULT: i64 = CHARS_MAX;
/// The least and the most seconds a field records, and what `max_seconds:` is when it is not written.
pub const SECONDS_MIN: i64 = 1;
pub const SECONDS_MAX: i64 = 60;
pub const SECONDS_DEFAULT: i64 = 15;
/// The words of `{"error": ..}`.
pub const ERRORS: &[&str] = &[
    "denied",
    "unsupported",
    "no microphone",
    "failed",
    "too big",
];
/// What a text can be, told by its first bytes, and the media type the field and the player name it by.
pub const KINDS: &[(&str, &str)] = &[
    ("webm", "audio/webm"),
    ("ogg", "audio/ogg"),
    ("mp4", "audio/mp4"),
    ("wav", "audio/wav"),
];
/// What an `AudioField` takes beside `label:`.
pub const FIELD_KEYS: &[&str] = &["icon", "max_seconds", "max_chars", "on"];
/// What an `AudioClip` takes beside `label:`.
pub const CLIP_KEYS: &[&str] = &["data"];
/// The icon of an `AudioField` when `icon:` is not written.
pub const ICON_DEFAULT: &str = "mic";

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

fn sextet(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// The first twelve bytes of a base64 text (its first sixteen characters), `None` if they are not base64.
fn head_bytes(text: &str) -> Option<[u8; 12]> {
    let c = text.as_bytes().get(..16)?;
    let mut out = [0u8; 12];
    for i in 0..4 {
        let a = sextet(c[4 * i])?;
        let b = sextet(c[4 * i + 1])?;
        let c2 = sextet(c[4 * i + 2])?;
        let d = sextet(c[4 * i + 3])?;
        out[3 * i] = (a << 2) | (b >> 4);
        out[3 * i + 1] = (b << 4) | (c2 >> 2);
        out[3 * i + 2] = (c2 << 6) | d;
    }
    Some(out)
}

/// What a recording text is, if it is one: base64 (only its alphabet, `=` at most twice and only at the end), `CHARS_MIN` to `CHARS_MAX` characters, whose first bytes are those of WebM
/// (`1A 45 DF A3`), Ogg (`OggS`), MP4 (`ftyp` after four bytes) or WAV (`RIFF`, four bytes, `WAVE`). The page's script has its own copy of this (`__cui_audio_kind`);
/// `tests/audio_corpus.json` is run against both.
pub fn audio_kind(text: &str) -> Option<&'static str> {
    let n = text.len();
    if !(CHARS_MIN as usize..=CHARS_MAX as usize).contains(&n) {
        return None;
    }
    let body = text.trim_end_matches('=');
    if text.len() - body.len() > 2 || body.is_empty() || !body.bytes().all(|b| sextet(b).is_some())
    {
        return None;
    }
    let h = head_bytes(text)?;
    if h[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        Some("webm")
    } else if &h[..4] == b"OggS" {
        Some("ogg")
    } else if &h[4..8] == b"ftyp" {
        Some("mp4")
    } else if &h[..4] == b"RIFF" && &h[8..12] == b"WAVE" {
        Some("wav")
    } else {
        None
    }
}

/// The media type of a kind from `audio_kind`.
pub fn mime_of(kind: &str) -> Option<&'static str> {
    KINDS.iter().find(|(k, _)| *k == kind).map(|(_, m)| *m)
}

/// What one `AudioField` promises about the value it gives its event.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSpec {
    pub max_chars: i64,
    pub max_seconds: i64,
}

/// Is `value` (the text a handler receives) one an `AudioField` with any of these specs could have produced? `Err` says why not.
pub fn check_value(value: &str, specs: &[AudioSpec]) -> Result<(), String> {
    let json: serde_json::Value =
        serde_json::from_str(value).map_err(|e| format!("it is not JSON: {e}"))?;
    let obj = json.as_object().ok_or("it is not a JSON object")?;
    if let Some(error) = obj.get("error") {
        if obj.len() != 1 {
            return Err("an `error` is the only key".into());
        }
        error
            .as_str()
            .filter(|w| ERRORS.contains(w))
            .ok_or_else(|| format!("`error` is one of {}", ERRORS.join(", ")))?;
        return Ok(());
    }
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    if keys != ["data", "mime", "seconds"] {
        return Err("it has exactly `data`, `seconds` and `mime`, or exactly `error`".into());
    }
    let data = obj["data"].as_str().ok_or("`data` is a string")?;
    let kind = audio_kind(data).ok_or(
        "`data` is base64 of WebM, Ogg, MP4 or WAV (told by its first bytes), 100 to 48,000 characters",
    )?;
    if obj["mime"].as_str() != mime_of(kind) {
        return Err(format!(
            "`mime` is the type the first bytes say: {}",
            mime_of(kind).unwrap_or("")
        ));
    }
    let seconds = obj["seconds"]
        .as_i64()
        .ok_or("`seconds` is a whole number")?;
    if !specs.iter().any(|s| {
        (data.len() as i64) <= s.max_chars && (SECONDS_MIN..=s.max_seconds).contains(&seconds)
    }) {
        return Err(
            "`data` is longer than `max_chars`, or `seconds` is outside 1 to `max_seconds`, so the page would not have said so"
                .into(),
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
        IrElementKind::AudioField => match b.key.as_str() {
            "icon" => match string_of(b) {
                Some(n) if crate::icon::known(n) => Ok(true),
                _ => Err(crate::icon::unknown(string_of(b).unwrap_or(""), b.span)),
            },
            "max_seconds" => match int_of(b) {
                Some(n) if (SECONDS_MIN..=SECONDS_MAX).contains(&n) => Ok(true),
                _ => Err(refuse(
                    "audio-limit-invalid",
                    format!("`max_seconds:` takes an integer literal from {SECONDS_MIN} to {SECONDS_MAX}: the most seconds the page records"),
                    b.span,
                )),
            },
            "max_chars" => match int_of(b) {
                Some(n) if (CHARS_MIN..=CHARS_MAX).contains(&n) => Ok(true),
                _ => Err(refuse(
                    "audio-limit-invalid",
                    format!("`max_chars:` takes an integer literal from {CHARS_MIN} to {CHARS_MAX}: the most characters of the base64 text the page gives the program"),
                    b.span,
                )),
            },
            "on" => Ok(false),
            other => Err(refuse(
                "binding-unknown",
                format!(
                    "`{other}:` is not a binding of AudioField; it takes label, {}",
                    FIELD_KEYS.join(", ")
                ),
                b.span,
            )),
        },
        IrElementKind::AudioClip => match b.key.as_str() {
            "data" => match &b.value {
                IrBindingValue::Const { expr, .. } => {
                    if let IrExpr::Ident { name, .. } = &**expr
                        && let Some(f) = c.state.iter().find(|f| &f.name == name)
                        && f.ty != IrType::String
                    {
                        return Err(refuse(
                            "audio-data-invalid",
                            format!("`data: {name}` is a state field that is not a `String`: an AudioClip plays the base64 text of a recording"),
                            b.span,
                        ));
                    }
                    Ok(true)
                }
                _ => Err(refuse(
                    "audio-data-invalid",
                    "`data:` of an AudioClip is a `String` the view reads (a state field, or a name of a `For`): `AudioClip { label: \"Voice note\" data: clip }`".to_string(),
                    b.span,
                )),
            },
            other => Err(refuse(
                "binding-unknown",
                format!(
                    "`{other}:` is not a binding of AudioClip; it takes label, {}",
                    CLIP_KEYS.join(", ")
                ),
                b.span,
            )),
        },
        _ => {
            if matches!(b.key.as_str(), "max_seconds") {
                return Err(refuse(
                    "binding-unknown",
                    "`max_seconds:` is a binding of AudioField, not of this element".to_string(),
                    b.span,
                ));
            }
            Ok(false)
        }
    }
}

/// What an `AudioField` and an `AudioClip` need to be complete.
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
        IrElementKind::AudioField => {
            if !named || find(bindings, "on").is_none() {
                return Err(refuse(
                    "audio-field-incomplete",
                    "an AudioField needs `label:` (its name, read aloud and shown under the icon) and `on:`, the event that is told the recording: `AudioField { label: \"Voice note\" on: Recorded }`".to_string(),
                    span,
                ));
            }
            if !children.is_empty() {
                return Err(refuse(
                    "audio-has-children",
                    "an AudioField has no content of its own: it is an icon over its label"
                        .to_string(),
                    span,
                ));
            }
            if in_for {
                return Err(refuse(
                    "audio-field-in-loop",
                    "an AudioField is not drawn inside a `For`: the page keeps the recording in progress and its words by the place of the field in the source, and every row would share them".to_string(),
                    span,
                ));
            }
            Ok(())
        }
        IrElementKind::AudioClip => {
            if find(bindings, "data").is_none() {
                return Err(refuse(
                    "audio-clip-incomplete",
                    "an AudioClip needs `data:`, the `String` that holds the recording: `AudioClip { label: \"Voice note\" data: clip }`".to_string(),
                    span,
                ));
            }
            if !children.is_empty() {
                return Err(refuse(
                    "audio-has-children",
                    "an AudioClip has no content of its own: it is a key, a bar and the time"
                        .to_string(),
                    span,
                ));
            }
            if !named {
                return Err(refuse(
                    "audio-clip-label-missing",
                    "an AudioClip needs `label:` (what it plays, read aloud: \"Voice note from Ana\"); a sound that nobody names is not a control a screen reader can use".to_string(),
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

/// Whether any view has an `AudioField` or an `AudioClip`: the rules and the script of the sound are written for a program that does, so every other page is the bytes it was.
pub fn uses_audio(ir: &IrFile) -> bool {
    any_kind(ir, |k| {
        matches!(k, IrElementKind::AudioField | IrElementKind::AudioClip)
    })
}

/// The icon of an `AudioField`: `icon:` or the default.
pub fn icon_of(bindings: &[IrBinding]) -> &str {
    find(bindings, "icon")
        .and_then(string_of)
        .unwrap_or(ICON_DEFAULT)
}

/// The `max_seconds:` and `max_chars:` of an `AudioField`, with the defaults.
pub fn limits_of(bindings: &[IrBinding]) -> (i64, i64) {
    (
        find(bindings, "max_seconds")
            .and_then(int_of)
            .unwrap_or(SECONDS_DEFAULT),
        find(bindings, "max_chars")
            .and_then(int_of)
            .unwrap_or(CHARS_DEFAULT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(head: [u8; 12], len: usize) -> String {
        // base64 of the twelve bytes, then characters of zero bits, `len` in all
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for i in 0..4 {
            let n = (u32::from(head[3 * i]) << 16)
                | (u32::from(head[3 * i + 1]) << 8)
                | u32::from(head[3 * i + 2]);
            for sh in [18, 12, 6, 0] {
                s.push(A[((n >> sh) & 63) as usize] as char);
            }
        }
        while s.len() < len {
            s.push('A');
        }
        s
    }

    fn head(kind: &str) -> [u8; 12] {
        let mut h = [0u8; 12];
        match kind {
            "webm" => h[..4].copy_from_slice(&[0x1A, 0x45, 0xDF, 0xA3]),
            "ogg" => h[..4].copy_from_slice(b"OggS"),
            "mp4" => {
                h[3] = 0x20;
                h[4..8].copy_from_slice(b"ftyp");
            }
            _ => {
                h[..4].copy_from_slice(b"RIFF");
                h[8..12].copy_from_slice(b"WAVE");
            }
        }
        h
    }

    #[test]
    fn each_kind_is_told_by_its_first_bytes() {
        for (k, _) in KINDS {
            assert_eq!(audio_kind(&text(head(k), 200)), Some(*k), "{k}");
        }
    }

    #[test]
    fn the_length_is_bounded_both_ways() {
        assert_eq!(audio_kind(&text(head("webm"), 99)), None);
        assert_eq!(audio_kind(&text(head("webm"), 100)), Some("webm"));
        assert_eq!(audio_kind(&text(head("webm"), 48_000)), Some("webm"));
        assert_eq!(audio_kind(&text(head("webm"), 48_001)), None);
    }

    #[test]
    fn a_text_that_is_not_base64_or_not_a_sound_is_none() {
        let mut t = text(head("wav"), 200);
        t.replace_range(50..51, "!");
        assert_eq!(audio_kind(&t), None);
        assert_eq!(audio_kind(&text([0; 12], 200)), None);
        // a JPEG is not a recording
        assert_eq!(audio_kind(&format!("/9j/{}", "A".repeat(200))), None);
        // a RIFF that is not WAVE (an AVI)
        let mut avi = head("wav");
        avi[8..12].copy_from_slice(b"AVI ");
        assert_eq!(audio_kind(&text(avi, 200)), None);
        // padding: at most two, only at the end
        let ok = text(head("ogg"), 200);
        assert_eq!(audio_kind(&format!("{}==", &ok[..198])), Some("ogg"));
        assert_eq!(audio_kind(&format!("{}===", &ok[..197])), None);
        assert_eq!(audio_kind(&format!("{}=A", &ok[..198])), None);
    }

    #[test]
    fn a_value_is_checked_against_the_field_that_gave_it() {
        let specs = [AudioSpec {
            max_chars: 300,
            max_seconds: 10,
        }];
        let data = text(head("webm"), 300);
        let good = format!(r#"{{"data":"{data}","seconds":7,"mime":"audio/webm"}}"#);
        assert!(check_value(&good, &specs).is_ok());
        let long = text(head("webm"), 301);
        assert!(
            check_value(
                &format!(r#"{{"data":"{long}","seconds":7,"mime":"audio/webm"}}"#),
                &specs
            )
            .is_err()
        );
        for (seconds, ok) in [(0, false), (1, true), (10, true), (11, false)] {
            let v = format!(r#"{{"data":"{data}","seconds":{seconds},"mime":"audio/webm"}}"#);
            assert_eq!(check_value(&v, &specs).is_ok(), ok, "{seconds}");
        }
        let wrong = format!(r#"{{"data":"{data}","seconds":7,"mime":"audio/ogg"}}"#);
        assert!(check_value(&wrong, &specs).is_err());
        assert!(check_value(r#"{"error":"denied"}"#, &specs).is_ok());
        assert!(check_value(r#"{"error":"unreadable"}"#, &specs).is_err());
        assert!(check_value(r#"{"error":"denied","data":"x"}"#, &specs).is_err());
        assert!(check_value("[]", &specs).is_err());
    }
}
