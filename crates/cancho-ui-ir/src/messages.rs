//! Message catalogs (docs/i18n.md section 3): the `messages` clause, the `t(..)` call, the `fmt.*` and `locale.*` calls. This pass owns their
//! rule tags. The text of a message is checked here, once; the page and replay only look it up.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use cancho_ui_syntax::ast::{Component, MessageCatalog, MessageEntry};
use cancho_ui_syntax::span::Span;

use crate::i18n;
use crate::ir::*;
use crate::lower::LowerError;

fn refused(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// A message as a call sees it: how many arguments it takes.
#[derive(Debug, Clone)]
struct Sig {
    plural: bool,
    holes: Vec<String>,
}

thread_local! {
    /// The signatures of the component being lowered, for the `t(..)` calls in it. Set by `enter`, emptied when its guard drops.
    static SIGS: RefCell<BTreeMap<String, Sig>> = const { RefCell::new(BTreeMap::new()) };
}

/// While alive, `t(..)` calls are checked against this component's catalogs.
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        SIGS.with(|s| s.borrow_mut().clear());
    }
}

pub fn enter(catalogs: &[IrCatalog]) -> Guard {
    let mut sigs = BTreeMap::new();
    if let Some(first) = catalogs.first() {
        for m in &first.entries {
            let holes = match &m.plural {
                Some(_) => Vec::new(),
                None => {
                    let mut seen: Vec<String> = Vec::new();
                    for p in m.forms.iter().flat_map(|(_, parts)| parts) {
                        if let MsgPart::Hole(h) = p
                            && !seen.contains(h)
                        {
                            seen.push(h.clone());
                        }
                    }
                    seen
                }
            };
            sigs.insert(
                m.key.clone(),
                Sig {
                    plural: m.plural.is_some(),
                    holes,
                },
            );
        }
    }
    SIGS.with(|s| *s.borrow_mut() = sigs);
    Guard
}

/// The holes of a message in the order a call gives its arguments: first appearance in the default language.
pub fn hole_order(m: &IrMessage) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for p in m.forms.iter().flat_map(|(_, parts)| parts) {
        if let MsgPart::Hole(h) = p
            && !seen.contains(h)
        {
            seen.push(h.clone());
        }
    }
    seen
}

/// A text with its holes written back as `{name}`.
pub fn text_of(parts: &[MsgPart]) -> String {
    parts
        .iter()
        .map(|p| match p {
            MsgPart::Lit(s) => s.clone(),
            MsgPart::Hole(h) => format!("{{{h}}}"),
        })
        .collect()
}

fn parse_text(text: &str, key: &str, span: Span) -> Result<Vec<MsgPart>, LowerError> {
    let bad = |why: &str| {
        refused(
            "message-text-invalid",
            format!("message `{key}`: {why}; a hole is `{{name}}` (letters, digits and `_`) and there is no way to write a brace in a message"),
            span,
        )
    };
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let mut name = String::new();
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == '}' {
                        closed = true;
                        break;
                    }
                    name.push(d);
                }
                if !closed {
                    return Err(bad("a `{` is never closed"));
                }
                let ok = !name.is_empty()
                    && !name.starts_with(|d: char| d.is_ascii_digit())
                    && name.chars().all(|d| d.is_ascii_alphanumeric() || d == '_');
                if !ok {
                    return Err(bad(&format!("`{{{name}}}` is not a hole")));
                }
                if !lit.is_empty() {
                    out.push(MsgPart::Lit(std::mem::take(&mut lit)));
                }
                out.push(MsgPart::Hole(name));
            }
            '}' => return Err(bad("a `}` closes nothing")),
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        out.push(MsgPart::Lit(lit));
    }
    Ok(out)
}

fn holes_of(parts: &[MsgPart]) -> BTreeSet<String> {
    parts
        .iter()
        .filter_map(|p| match p {
            MsgPart::Hole(h) => Some(h.clone()),
            MsgPart::Lit(_) => None,
        })
        .collect()
}

fn lower_entry(lang: &str, e: &MessageEntry) -> Result<IrMessage, LowerError> {
    let mut forms = Vec::new();
    match &e.plural {
        None => {
            let (_, text) = &e.forms[0];
            forms.push((String::new(), parse_text(text, &e.key, e.span)?));
        }
        Some(param) => {
            let want = i18n::categories(lang).unwrap_or(&["other"]);
            let mut seen: Vec<&str> = Vec::new();
            for (cat, text) in &e.forms {
                let cat = cat.as_deref().unwrap_or("");
                if !want.contains(&cat) {
                    return Err(refused(
                        "message-plural-incomplete",
                        format!(
                            "message `{}` in `{lang}` has a form `{cat}`, which {lang} does not have; its forms are {}",
                            e.key,
                            want.join(", ")
                        ),
                        e.span,
                    ));
                }
                if seen.contains(&cat) {
                    return Err(refused(
                        "message-plural-incomplete",
                        format!("message `{}` in `{lang}` gives the form `{cat}` twice", e.key),
                        e.span,
                    ));
                }
                seen.push(cat);
                let parts = parse_text(text, &e.key, e.span)?;
                if let Some(h) = holes_of(&parts).into_iter().find(|h| h != param) {
                    return Err(refused(
                        "message-text-invalid",
                        format!("message `{}`: a plural form can only use its count `{{{param}}}`, not `{{{h}}}`", e.key),
                        e.span,
                    ));
                }
                forms.push((cat.to_string(), parts));
            }
            if let Some(missing) = want.iter().find(|c| !seen.contains(c)) {
                return Err(refused(
                    "message-plural-incomplete",
                    format!(
                        "message `{}` in `{lang}` has no `{missing}` form; {lang} needs {}",
                        e.key,
                        want.join(", ")
                    ),
                    e.span,
                ));
            }
        }
    }
    Ok(IrMessage {
        key: e.key.clone(),
        plural: e.plural.clone(),
        forms,
        span: e.span,
    })
}

fn lower_catalog(c: &Component, cat: &MessageCatalog) -> Result<IrCatalog, LowerError> {
    if !i18n::is_known(&cat.lang) {
        let known: Vec<&str> = i18n::LANGS.iter().map(|l| l.code).collect();
        return Err(refused(
            "message-language-unknown",
            format!(
                "component `{}`: `{}` is not a language the compiler has the formats of; they are {}",
                c.name,
                cat.lang,
                known.join(", ")
            ),
            cat.span,
        ));
    }
    let mut entries: Vec<IrMessage> = Vec::new();
    for e in &cat.entries {
        if entries.iter().any(|m| m.key == e.key) {
            return Err(refused(
                "message-key-duplicate",
                format!("component `{}`: `{}` has the message `{}` twice", c.name, cat.lang, e.key),
                e.span,
            ));
        }
        entries.push(lower_entry(&cat.lang, e)?);
    }
    Ok(IrCatalog {
        lang: cat.lang.clone(),
        entries,
        span: cat.span,
    })
}

/// Lower and cross-check the catalogs of a component: every language has the same keys, of the same kind and with the same holes.
pub fn lower_messages(c: &Component) -> Result<Vec<IrCatalog>, LowerError> {
    let mut out: Vec<IrCatalog> = Vec::new();
    for cat in &c.messages {
        if out.iter().any(|o| o.lang == cat.lang) {
            return Err(refused(
                "message-key-duplicate",
                format!("component `{}`: the language `{}` has two catalogs", c.name, cat.lang),
                cat.span,
            ));
        }
        out.push(lower_catalog(c, cat)?);
    }
    let Some(first) = out.first() else {
        return Ok(out);
    };
    for other in &out[1..] {
        for m in &first.entries {
            let Some(o) = other.entries.iter().find(|x| x.key == m.key) else {
                return Err(refused(
                    "message-key-missing",
                    format!(
                        "component `{}`: `{}` has the message `{}` and `{}` does not",
                        c.name, first.lang, m.key, other.lang
                    ),
                    other.span,
                ));
            };
            if m.plural.is_some() != o.plural.is_some() {
                return Err(refused(
                    "message-hole-mismatch",
                    format!(
                        "component `{}`: `{}` is a plural message in one of `{}` and `{}` and a plain one in the other",
                        c.name, m.key, first.lang, other.lang
                    ),
                    o.span,
                ));
            }
            if m.plural.is_none() {
                let a = holes_of(&m.forms[0].1);
                let b = holes_of(&o.forms[0].1);
                if a != b {
                    return Err(refused(
                        "message-hole-mismatch",
                        format!(
                            "component `{}`: message `{}` has the holes {:?} in `{}` and {:?} in `{}`",
                            c.name,
                            m.key,
                            a.iter().collect::<Vec<_>>(),
                            first.lang,
                            b.iter().collect::<Vec<_>>(),
                            other.lang
                        ),
                        o.span,
                    ));
                }
            }
        }
        for o in &other.entries {
            if !first.entries.iter().any(|m| m.key == o.key) {
                return Err(refused(
                    "message-key-missing",
                    format!(
                        "component `{}`: `{}` has the message `{}` and `{}` does not",
                        c.name, other.lang, o.key, first.lang
                    ),
                    o.span,
                ));
            }
        }
    }
    Ok(out)
}

/// The calls of this pass: `t`, `fmt.number`, `fmt.percent`, `fmt.date`, `fmt.time`, `locale.current`, `locale.set`.
pub fn is_call(path: &[String]) -> bool {
    match path {
        [one] => one == "t",
        [a, b] => matches!(
            (a.as_str(), b.as_str()),
            ("fmt", "number" | "percent" | "date" | "time") | ("locale", "current" | "set")
        ),
        _ => false,
    }
}

/// Check the shape of one call at its place: the arity, the literal key and the literal style.
pub fn check_call(
    path: &[String],
    args: &[cancho_ui_syntax::ast::Expr],
    span: Span,
) -> Result<(), LowerError> {
    use cancho_ui_syntax::ast::Expr;
    let name = path.join(".");
    let invalid = |detail: String| refused("message-call-invalid", format!("`{name}`: {detail}"), span);
    let want = match name.as_str() {
        "fmt.number" | "fmt.date" => Some(2),
        "fmt.percent" | "fmt.time" | "locale.set" => Some(1),
        "locale.current" => Some(0),
        _ => None,
    };
    if let Some(n) = want
        && args.len() != n
    {
        return Err(invalid(format!(
            "takes exactly {n} argument{}",
            if n == 1 { "" } else { "s" }
        )));
    }
    if name == "fmt.date"
        && !matches!(&args[1], Expr::StringLit { value, .. } if i18n::DATE_STYLES.contains(&value.as_str()))
    {
        return Err(invalid(
            "the style is one of the literals \"short\", \"medium\", \"long\", \"full\"".to_string(),
        ));
    }
    if name == "t" {
        let Some(Expr::StringLit { value: key, .. }) = args.first() else {
            return Err(invalid("the first argument is the key, a string literal".to_string()));
        };
        let sig = SIGS.with(|s| s.borrow().get(key).cloned());
        let Some(sig) = sig else {
            return Err(refused(
                "message-key-unknown",
                format!("`t(\"{key}\")`: this component has no message `{key}`; add it to every language of its `messages`"),
                span,
            ));
        };
        let wanted = if sig.plural { 1 } else { sig.holes.len() };
        if args.len() != 1 + wanted {
            return Err(invalid(if sig.plural {
                format!("`{key}` is a plural message: `t(\"{key}\", count)`")
            } else {
                format!(
                    "`{key}` has {} hole{}{}: give that many arguments after the key",
                    sig.holes.len(),
                    if sig.holes.len() == 1 { "" } else { "s" },
                    if sig.holes.is_empty() {
                        String::new()
                    } else {
                        format!(" ({}, in this order)", sig.holes.join(", "))
                    }
                )
            }));
        }
    }
    Ok(())
}
