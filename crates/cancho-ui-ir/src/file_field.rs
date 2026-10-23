//! `FileField` (docs/file-upload.md): the limits and the `accept` list, shared by the checks that refuse a bad binding and by replay,
//! which refuses a file value the page could not have produced. The page's own copy of `accept_matches` is in the file adapter
//! (`crates/cancho-ui-codegen-web/src/adapters.rs`); `tests/file_corpus.json` is run against both.

/// The most bytes a `FileField` reads, and the default (`max_bytes:` is 1 to this).
pub const MAX_BYTES: i64 = 1_048_576;
/// The most tokens an `accept:` lists.
pub const ACCEPT_TOKENS_MAX: usize = 20;
/// The four words of `{"error": ..}`.
pub const ERRORS: &[&str] = &[
    "too large",
    "not text",
    "not an accepted type",
    "unreadable",
];

/// One token of `accept:`: `.ext` (letters and digits), `type/subtype` or `type/*` (lower-case letters, digits, `.`, `+`, `-`).
pub fn accept_token_valid(t: &str) -> bool {
    if let Some(ext) = t.strip_prefix('.') {
        return !ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric());
    }
    let word = |w: &str| {
        !w.is_empty()
            && w.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-')
            })
    };
    match t.split_once('/') {
        Some((a, "*")) => word(a),
        Some((a, b)) => word(a) && word(b),
        None => false,
    }
}

/// A whole `accept:` literal: comma-separated tokens, no spaces, at most `ACCEPT_TOKENS_MAX`.
pub fn accept_valid(accept: &str) -> bool {
    let tokens: Vec<&str> = accept.split(',').collect();
    tokens.len() <= ACCEPT_TOKENS_MAX && tokens.iter().all(|t| accept_token_valid(t))
}

/// Does a file of this `name` and `type` satisfy the `accept:` list? An extension is compared with the end of the name, a type with
/// the file's type, `type/*` with its first part; case does not matter.
pub fn accept_matches(accept: &str, name: &str, file_type: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let file_type = file_type.to_ascii_lowercase();
    accept.split(',').any(|t| {
        let t = t.to_ascii_lowercase();
        if t.starts_with('.') {
            name.ends_with(&t)
        } else if let Some(prefix) = t.strip_suffix("/*") {
            file_type.split('/').next() == Some(prefix) && file_type.contains('/')
        } else {
            file_type == t
        }
    })
}

/// What one `FileField` promises about the value it gives its event.
#[derive(Debug, Clone, PartialEq)]
pub struct FileSpec {
    pub max_bytes: i64,
    pub accept: Option<String>,
}

/// Is `value` (the text a handler receives) one a `FileField` with any of these specs could have produced? `Err` says why not.
pub fn check_value(value: &str, specs: &[FileSpec]) -> Result<(), String> {
    let json: serde_json::Value =
        serde_json::from_str(value).map_err(|e| format!("it is not JSON: {e}"))?;
    let obj = json.as_object().ok_or("it is not a JSON object")?;
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("`name` is a string")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err("`name` is a file name: not empty, no `/` or `\\`".into());
    }
    let size = obj
        .get("size")
        .and_then(|v| v.as_i64())
        .filter(|n| *n >= 0)
        .ok_or("`size` is a whole number of bytes, 0 or more")?;
    let ty = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("`type` is a string (empty when the browser does not know)")?;
    let text = obj.get("text");
    let error = obj.get("error");
    if obj.len() != 4 || text.is_some() == error.is_some() {
        return Err("it has exactly `name`, `size`, `type` and one of `text` or `error`".into());
    }
    let fits = |s: &FileSpec| size <= s.max_bytes;
    let accepted = |s: &FileSpec| {
        s.accept
            .as_deref()
            .is_none_or(|a| accept_matches(a, name, ty))
    };
    if let Some(text) = text {
        let text = text.as_str().ok_or("`text` is a string")?;
        if !specs.iter().any(|s| fits(s) && accepted(s)) {
            return Err("the file is over `max_bytes` or not an accepted type, so the page would have said so with an `error`".into());
        }
        if (text.len() as i64) > size {
            return Err("`text` is longer in bytes than `size`".into());
        }
        return Ok(());
    }
    let word = error
        .and_then(|v| v.as_str())
        .filter(|w| ERRORS.contains(w))
        .ok_or_else(|| format!("`error` is one of {}", ERRORS.join(", ")))?;
    match word {
        "too large" if specs.iter().all(fits) => {
            Err("`too large` for a file within `max_bytes`".into())
        }
        "not an accepted type" if specs.iter().all(|s| s.accept.is_none() || accepted(s)) => {
            Err("`not an accepted type` for a file `accept` allows".into())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_tokens() {
        for ok in [
            ".txt",
            ".h264",
            "text/plain",
            "text/*",
            "application/vnd.ms-excel",
            "image/svg+xml",
        ] {
            assert!(accept_token_valid(ok), "{ok}");
        }
        for bad in [
            "",
            ".",
            "txt",
            ".t xt",
            ".tx.t",
            "text",
            "text/",
            "/plain",
            "Text/Plain",
            "*/*",
            "text/*/x",
            "a b/c",
            ".txt;",
        ] {
            assert!(!accept_token_valid(bad), "{bad}");
        }
        assert!(accept_valid(".txt,text/csv"));
        assert!(!accept_valid(".txt, text/csv"), "no spaces");
        assert!(!accept_valid(""));
        assert!(!accept_valid(".txt,"));
        let many = (0..21)
            .map(|i| format!(".e{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(!accept_valid(&many));
    }

    #[test]
    fn accept_matching() {
        assert!(accept_matches(".txt", "A.TXT", ""));
        assert!(accept_matches("text/csv", "data", "text/csv"));
        assert!(accept_matches("text/*", "x", "text/html"));
        assert!(!accept_matches("text/*", "x", "image/png"));
        assert!(
            !accept_matches("text/*", "x", "text"),
            "a type without a slash is no type"
        );
        assert!(
            !accept_matches(".txt", "txt", "text/plain"),
            "the dot is part of the extension"
        );
        assert!(!accept_matches(".txt", "a.txt.exe", "text/plain"));
    }

    #[test]
    fn values() {
        let specs = [FileSpec {
            max_bytes: 20,
            accept: Some(".txt".into()),
        }];
        assert!(
            check_value(
                r#"{"name":"a.txt","size":5,"type":"text/plain","text":"hello"}"#,
                &specs
            )
            .is_ok()
        );
        assert!(
            check_value(
                r#"{"name":"a.txt","size":5,"type":"","error":"unreadable"}"#,
                &specs
            )
            .is_ok()
        );
        assert!(
            check_value(
                r#"{"name":"a.txt","size":50,"type":"","error":"too large"}"#,
                &specs
            )
            .is_ok()
        );
        assert!(
            check_value(
                r#"{"name":"a.png","size":5,"type":"","error":"not an accepted type"}"#,
                &specs
            )
            .is_ok()
        );
        assert!(check_value(r#"{"name":"a.txt","size":50,"type":"","text":"x"}"#, &specs).is_err());
        assert!(check_value("", &specs).is_err());
    }
}
