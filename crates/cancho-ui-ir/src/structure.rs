//! The structure elements (docs/structure.md, step 2 of docs/www.md): headings, sections, landmarks, `Article`, `Link`, `Details`,
//! `Figure`/`Caption`, and the `style:` of a `Text`. This module owns their rule tags and their DOM recipe (the tag, the attributes and the
//! class an element becomes), which the JavaScript emitter reads today and the prerenderer will read, so that the two cannot drift.

use std::collections::BTreeSet;

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The `appearance:` of a `Link`: a closed set, drawn by the baseline stylesheet (and, with a theme, in the theme's colours).
pub const APPEARANCES: &[&str] = &["primary", "secondary", "quiet"];
/// The `style:` of a `Text`: a typographic role, a closed set.
pub const TEXT_STYLES: &[&str] = &["eyebrow", "lead", "note", "label"];
/// The range of a code block's `max_height:`, in rem.
pub const CODE_HEIGHT: (i64, i64) = (4, 100);
/// The range of a heading's `level:`.
pub const LEVELS: (i64, i64) = (1, 4);
/// The `tone:` of a `Chip`: a closed set, each a text colour the theme checks against the page (`good`, `warn` on `warn_bg`, `accent`, `muted`).
pub const CHIP_TONES: &[&str] = &["good", "warn", "accent", "quiet"];
/// The words that do not say where a link goes (compared lower-cased, with punctuation trimmed), `link-label-generic`.
pub const GENERIC_LABELS: &[&str] = &[
    "here",
    "click here",
    "click",
    "link",
    "this link",
    "this",
    "more",
    "read more",
    "learn more",
];

/// The elements this module defines.
pub fn is_structure(kind: &IrElementKind) -> bool {
    matches!(
        kind,
        IrElementKind::Heading
            | IrElementKind::Section
            | IrElementKind::Header
            | IrElementKind::Nav
            | IrElementKind::Main
            | IrElementKind::Footer
            | IrElementKind::Aside
            | IrElementKind::Article
            | IrElementKind::Link
            | IrElementKind::Details
            | IrElementKind::Figure
            | IrElementKind::Caption
            | IrElementKind::Rich
            | IrElementKind::Strong
            | IrElementKind::Em
            | IrElementKind::Small
            | IrElementKind::Reveal
            | IrElementKind::CountUp
            | IrElementKind::Chip
            | IrElementKind::Break
            | IrElementKind::Summary
            | IrElementKind::Code
            | IrElementKind::InlineCode
    )
}

/// The elements whose children are inline content: text, and `Strong`, `Em`, `Small`, `Break`, `Code` (as `<code>`) and `Link`.
pub fn is_inline_container(kind: &IrElementKind) -> bool {
    matches!(
        kind,
        IrElementKind::Rich
            | IrElementKind::Heading
            | IrElementKind::Caption
            | IrElementKind::Summary
            | IrElementKind::Strong
            | IrElementKind::Em
            | IrElementKind::Small
            | IrElementKind::CountUp
            | IrElementKind::Chip
            | IrElementKind::Link
    )
}

/// The same set before lowering, so a `Code` written inside inline content can be made an `InlineCode`.
pub fn ast_is_inline_container(kind: &cancho_ui_syntax::ast::ElementKind) -> bool {
    use cancho_ui_syntax::ast::ElementKind as K;
    matches!(
        kind,
        K::Rich | K::Heading | K::Caption | K::Summary | K::Strong | K::Em | K::Small | K::Link
    )
}

/// What may stand inside an inline container: `Link` not inside a `Link`.
fn inline_child_ok(parent: &IrElementKind, child: &IrElementKind) -> bool {
    // a chip is a short label: text and nothing else
    if matches!(parent, IrElementKind::Chip) {
        return false;
    }
    match child {
        IrElementKind::Strong
        | IrElementKind::Em
        | IrElementKind::Small
        | IrElementKind::CountUp
        | IrElementKind::Chip
        | IrElementKind::Break
        | IrElementKind::InlineCode => true,
        IrElementKind::Link => !matches!(parent, IrElementKind::Link),
        _ => false,
    }
}

fn is_inline_only(kind: &IrElementKind) -> bool {
    matches!(
        kind,
        IrElementKind::Strong
            | IrElementKind::Em
            | IrElementKind::Small
            | IrElementKind::CountUp
            | IrElementKind::Chip
            | IrElementKind::Break
            | IrElementKind::InlineCode
    )
}

fn is_landmark_or_container(kind: &IrElementKind) -> bool {
    matches!(
        kind,
        IrElementKind::Section
            | IrElementKind::Header
            | IrElementKind::Nav
            | IrElementKind::Main
            | IrElementKind::Footer
            | IrElementKind::Aside
            | IrElementKind::Article
            | IrElementKind::Figure
            | IrElementKind::Reveal
    )
}

fn name(kind: &IrElementKind) -> &'static str {
    match kind {
        IrElementKind::Heading => "Heading",
        IrElementKind::Section => "Section",
        IrElementKind::Header => "Header",
        IrElementKind::Nav => "Nav",
        IrElementKind::Main => "Main",
        IrElementKind::Footer => "Footer",
        IrElementKind::Aside => "Aside",
        IrElementKind::Article => "Article",
        IrElementKind::Link => "Link",
        IrElementKind::Details => "Details",
        IrElementKind::Figure => "Figure",
        IrElementKind::Caption => "Caption",
        IrElementKind::Rich => "Rich",
        IrElementKind::Strong => "Strong",
        IrElementKind::Em => "Em",
        IrElementKind::Small => "Small",
        IrElementKind::Reveal => "Reveal",
        IrElementKind::CountUp => "CountUp",
        IrElementKind::Chip => "Chip",
        IrElementKind::Break => "Break",
        IrElementKind::Summary => "Summary",
        IrElementKind::Code | IrElementKind::InlineCode => "Code",
        IrElementKind::Text => "Text",
        _ => "this element",
    }
}

/// The keys (beside `label:` on the landmarks) an element of this module takes.
fn keys(kind: &IrElementKind) -> &'static [&'static str] {
    match kind {
        IrElementKind::Heading => &["level", "id"],
        IrElementKind::Link => &["to", "appearance"],
        IrElementKind::Details => &["summary", "id"],
        IrElementKind::Caption => &["id"],
        IrElementKind::Rich => &["style"],
        IrElementKind::Code => &["copy", "max_height"],
        IrElementKind::Header => &["id", "sticky"],
        IrElementKind::CountUp => &["to"],
        IrElementKind::Chip => &["tone"],
        IrElementKind::Section
        | IrElementKind::Article
        | IrElementKind::Aside
        | IrElementKind::Figure => &["id", "visible", "hidden"],
        k if is_landmark_or_container(k) => &["id"],
        _ => &[],
    }
}

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

/// An `id:` is a letter then letters, digits, `-` and `_`, at most 64; `cui-` is the compiler's own.
pub fn id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !id.to_ascii_lowercase().starts_with("cui-")
}

/// What a `to:` is: nothing wrong, or the reason it is refused.
pub fn check_to(to: &str) -> Result<(), String> {
    if to.is_empty() {
        return Err("`to:` is empty".to_string());
    }
    if to.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("`to:` holds a space or a control character (a browser drops tabs and newlines inside a scheme, so `java\\tscript:` is not what it looks like)".to_string());
    }
    if to.starts_with("//") {
        return Err(
            "`to:` starts with `//`, an address that hides its scheme; write `https://host/..`"
                .to_string(),
        );
    }
    if let Some(i) = to.find(':') {
        let head = &to[..i];
        let is_scheme = !head.is_empty()
            && head.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && head
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'));
        if is_scheme {
            let scheme = head.to_ascii_lowercase();
            return match scheme.as_str() {
                "https" | "http" => host_of(to).map(|_| ()).ok_or_else(|| {
                    "`to:` has no host, or has a user name and password before it".to_string()
                }),
                "mailto" => {
                    if to.len() > 7 && to[7..].contains('@') {
                        Ok(())
                    } else {
                        Err("`mailto:` needs an address with an `@`".to_string())
                    }
                }
                other => Err(format!(
                    "`to:` has the scheme `{other}:`; a link goes to `https:`, `http:`, `mailto:`, a `#id` of this page, or a file of this site"
                )),
            };
        }
    }
    if let Some(frag) = to.strip_prefix('#') {
        return if id_ok(frag) {
            Ok(())
        } else {
            Err(format!(
                "`to: \"{to}\"` is not the id of an element (letters, digits, `-`, `_`)"
            ))
        };
    }
    Ok(())
}

/// The host of an `http(s)` address, lower-cased; `None` when there is none or a user name comes before it.
pub fn host_of(to: &str) -> Option<String> {
    let rest = to.split_once("://")?.1;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let host = authority.rsplit_once(':').map_or(authority, |(h, port)| {
        if port.bytes().all(|b| b.is_ascii_digit()) {
            h
        } else {
            authority
        }
    });
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The text of a node when it is all literal: `None` when it holds an expression.
fn literal_text(children: &[IrViewNode]) -> Option<String> {
    let mut out = String::new();
    for c in children {
        match c {
            IrViewNode::Text { parts, .. } => {
                for p in parts {
                    match p {
                        IrTextPart::Literal(s) => out.push_str(s),
                        IrTextPart::Expr(_) => return None,
                    }
                }
            }
            IrViewNode::Element { kind, children, .. }
                if is_inline_container(kind) || matches!(kind, IrElementKind::InlineCode) =>
            {
                out.push_str(&literal_text(children)?);
            }
            IrViewNode::Element {
                kind: IrElementKind::Break,
                ..
            } => out.push(' '),
            _ => return None,
        }
    }
    Some(out)
}

/// Checks one binding. `Ok(true)`: this module owns it and it is fine; `Ok(false)`: not its business.
pub fn check_binding(kind: &IrElementKind, b: &IrBinding) -> Result<bool, LowerError> {
    const NEW: &[&str] = &[
        "id",
        "level",
        "appearance",
        "style",
        "summary",
        "copy",
        "max_height",
        "sticky",
        "visible",
        "hidden",
    ];
    let structure = is_structure(kind);
    if !structure && !(NEW.contains(&b.key.as_str())) {
        return Ok(false);
    }
    if !structure && b.key == "style" && matches!(kind, IrElementKind::Text) {
        return match string_of(b) {
            Some(s) if TEXT_STYLES.contains(&s) => Ok(true),
            _ => Err(refuse(
                "text-style-invalid",
                format!(
                    "`style:` is one of {} as a string literal",
                    TEXT_STYLES.join(", ")
                ),
                b.span,
            )),
        };
    }
    if !structure {
        return Err(refuse(
            "binding-unknown",
            format!(
                "`{}:` is not a binding of {}; it belongs to {}",
                b.key,
                match kind {
                    IrElementKind::Named(_) => "a component".to_string(),
                    _ => "this element".to_string(),
                },
                match b.key.as_str() {
                    "level" => "Heading",
                    "appearance" => "Link",
                    "summary" => "Details",
                    "style" => "Text and Rich",
                    "copy" | "max_height" => "Code",
                    "sticky" => "Header",
                    "visible" | "hidden" => "Section, Article, Aside and Figure",
                    _ => "the structure elements (Section, Article, Heading, ..)",
                }
            ),
            b.span,
        ));
    }
    let own = keys(kind);
    if !own.contains(&b.key.as_str()) {
        return Err(refuse(
            "binding-unknown",
            format!(
                "`{}:` is not a binding of {}; it takes {}",
                b.key,
                name(kind),
                if own.is_empty() {
                    "only `label:`".to_string()
                } else {
                    own.join(", ")
                }
            ),
            b.span,
        ));
    }
    match b.key.as_str() {
        "id" => match string_of(b) {
            Some(s) if id_ok(s) => Ok(true),
            _ => Err(refuse(
                "id-invalid",
                "`id:` is a string literal: a letter, then letters, digits, `-` and `_` (at most 64), and not starting with `cui-`, which is the compiler's".to_string(),
                b.span,
            )),
        },
        "level" => match int_of(b) {
            Some(n) if (LEVELS.0..=LEVELS.1).contains(&n) => Ok(true),
            _ => Err(refuse(
                "heading-level-invalid",
                format!("`level:` is an integer literal from {} to {}", LEVELS.0, LEVELS.1),
                b.span,
            )),
        },
        // the parser gives these an event name and nothing else; that the event exists and takes no parameters is checked with the component
        "tone" => match string_of(b) {
            Some(s) if CHIP_TONES.contains(&s) => Ok(true),
            _ => Err(refuse(
                "chip-tone-invalid",
                format!("`tone:` is one of {} as a string literal", CHIP_TONES.join(", ")),
                b.span,
            )),
        },
        "visible" | "hidden" => Ok(true),
        // a CountUp's `to:` is checked where its text is made (motion.rs)
        "to" if !matches!(kind, IrElementKind::Link) => Ok(true),
        "to" => match string_of(b) {
            None => Err(refuse(
                "link-to-invalid",
                "`to:` of a Link is a string literal, or a text that a `For` over a `data` file writes out at build (docs/data.md): a text that reads state is not a link".to_string(),
                b.span,
            )),
            Some(s) => check_to(s)
                .map(|()| true)
                .map_err(|why| refuse("link-scheme-refused", why, b.span)),
        },
        "style" => match string_of(b) {
            Some(s) if TEXT_STYLES.contains(&s) => Ok(true),
            _ => Err(refuse(
                "text-style-invalid",
                format!(
                    "`style:` is one of {} as a string literal",
                    TEXT_STYLES.join(", ")
                ),
                b.span,
            )),
        },
        "sticky" => match &b.value {
            IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { .. }) => {
                Ok(true)
            }
            _ => Err(refuse(
                "header-sticky-invalid",
                "`sticky:` is `true` or `false`: `Header { sticky: true .. }`".to_string(),
                b.span,
            )),
        },
        "copy" => match &b.value {
            IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { .. }) => {
                Ok(true)
            }
            _ => Err(refuse(
                "code-copy-invalid",
                "`copy:` is `true` or `false`: `Code { copy: true \"..\" }`".to_string(),
                b.span,
            )),
        },
        "max_height" => match int_of(b) {
            Some(n) if (CODE_HEIGHT.0..=CODE_HEIGHT.1).contains(&n) => Ok(true),
            _ => Err(refuse(
                "code-max-height-invalid",
                format!(
                    "`max_height:` is an integer literal from {} to {}, in rem",
                    CODE_HEIGHT.0, CODE_HEIGHT.1
                ),
                b.span,
            )),
        },
        "appearance" => match string_of(b) {
            Some(s) if APPEARANCES.contains(&s) => Ok(true),
            _ => Err(refuse(
                "link-appearance-invalid",
                format!("`appearance:` is one of {} as a string literal", APPEARANCES.join(", ")),
                b.span,
            )),
        },
        "summary" => match string_of(b) {
            Some(s) if !s.trim().is_empty() => Ok(true),
            _ => Err(refuse(
                "details-incomplete",
                "`summary:` is the text of the line that opens the Details, a string literal that says something".to_string(),
                b.span,
            )),
        },
        _ => Ok(true),
    }
}

/// What an element needs to be complete, and what it may not carry.
pub fn check_complete(
    c: &IrComponent,
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
    in_for: bool,
) -> Result<(), LowerError> {
    if !is_structure(kind) {
        return Ok(());
    }
    let has = |k: &str| bindings.iter().any(|b| b.key == k);
    let text = literal_text(children);
    // the name of a heading, a link, a caption and a details is its text; a `label:` would make the name differ from what is seen
    if (!is_landmark_or_container(kind) || matches!(kind, IrElementKind::Reveal))
        && (accessibility.label.is_some() || accessibility.label_expr.is_some())
    {
        return Err(refuse(
            "binding-unknown",
            format!(
                "`label:` is not a binding of {}: its text is its name",
                name(kind)
            ),
            span,
        ));
    }
    if accessibility.role.is_some() {
        return Err(refuse(
            "binding-unknown",
            format!(
                "`role:` is not a binding of {}: it is what it says it is",
                name(kind)
            ),
            span,
        ));
    }
    if in_for && has("id") {
        return Err(refuse(
            "anchor-duplicate",
            format!(
                "an `id:` on {} inside a `For` would be repeated for every item; an id is one element",
                name(kind)
            ),
            span,
        ));
    }
    for b in bindings
        .iter()
        .filter(|b| b.key == "visible" || b.key == "hidden")
    {
        if let IrBindingValue::EventRef(n) = &b.value {
            match c.events.iter().find(|e| &e.name == n) {
                None => {
                    return Err(refuse(
                        "visible-event-invalid",
                        format!("`{}: {n}`: this component has no `on {n}` handler", b.key),
                        b.span,
                    ));
                }
                Some(e) if !e.params.is_empty() => {
                    return Err(refuse(
                        "visible-event-invalid",
                        format!(
                            "`{}: {n}`: `on {n}` takes parameters, and being seen has none to give; use an event without parameters",
                            b.key
                        ),
                        b.span,
                    ));
                }
                Some(_) => {}
            }
        }
    }
    if matches!(kind, IrElementKind::Chip) {
        if !has("tone") {
            return Err(refuse(
                "chip-tone-invalid",
                format!(
                    "a Chip needs `tone:`, one of {}: `Chip {{ tone: \"good\" \"working\" }}`",
                    CHIP_TONES.join(", ")
                ),
                span,
            ));
        }
        if text.as_deref().is_some_and(|t| t.trim().is_empty()) || children.is_empty() {
            return Err(refuse(
                "chip-empty",
                "a Chip with no text says nothing, and its tone alone would be the whole message"
                    .to_string(),
                span,
            ));
        }
    }
    match kind {
        IrElementKind::Heading => {
            if !has("level") {
                return Err(refuse(
                    "heading-level-invalid",
                    "a Heading needs `level:`, an integer from 1 to 4: `Heading { level: 2 \"Title\" }`".to_string(),
                    span,
                ));
            }
            if text.as_deref().is_some_and(|t| t.trim().is_empty()) || children.is_empty() {
                return Err(refuse(
                    "heading-empty",
                    "a Heading with no text is an empty entry in the outline of the page"
                        .to_string(),
                    span,
                ));
            }
        }
        IrElementKind::Link => {
            if !has("to") {
                return Err(refuse(
                    "link-to-invalid",
                    "a Link needs `to:`: `Link { to: \"https://..\" \"what it is\" }`".to_string(),
                    span,
                ));
            }
            if text.as_deref().is_some_and(|t| t.trim().is_empty()) || children.is_empty() {
                return Err(refuse(
                    "link-label-missing",
                    "a Link with no text has no name for a screen reader".to_string(),
                    span,
                ));
            }
            if let Some(t) = &text {
                let word = t
                    .trim()
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if GENERIC_LABELS.contains(&word.as_str()) {
                    return Err(refuse(
                        "link-label-generic",
                        format!(
                            "the link text \"{}\" does not say where the link goes: a screen reader reads links out of their sentence",
                            t.trim()
                        ),
                        span,
                    ));
                }
            }
        }
        IrElementKind::Details => {
            let summaries = children
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        IrViewNode::Element {
                            kind: IrElementKind::Summary,
                            ..
                        }
                    )
                })
                .count();
            let first_is_summary = matches!(
                children.first(),
                Some(IrViewNode::Element {
                    kind: IrElementKind::Summary,
                    ..
                })
            );
            if !has("summary") && summaries == 0 {
                return Err(refuse(
                    "details-incomplete",
                    "a Details needs `summary:`, the line that opens it (`Details { summary: \"More\" .. }`), or a `Summary { .. }` as its first child when that line has inline content".to_string(),
                    span,
                ));
            }
            if has("summary") && summaries > 0 {
                return Err(refuse(
                    "details-incomplete",
                    "a Details has `summary:` or a `Summary { .. }` child, not both".to_string(),
                    span,
                ));
            }
            if summaries > 1 || (summaries == 1 && !first_is_summary) {
                return Err(refuse(
                    "details-incomplete",
                    "a Details has one `Summary`, and it is the first thing in it".to_string(),
                    span,
                ));
            }
        }
        IrElementKind::Rich | IrElementKind::Strong | IrElementKind::Em | IrElementKind::Small => {
            if children.is_empty() || text.as_deref().is_some_and(|t| t.trim().is_empty()) {
                return Err(refuse(
                    "rich-empty",
                    format!("{} with nothing in it says nothing", name(kind)),
                    span,
                ));
            }
        }
        IrElementKind::Break if !children.is_empty() => {
            return Err(refuse(
                "rich-child-invalid",
                "a Break is a line break and holds nothing".to_string(),
                span,
            ));
        }
        IrElementKind::Code | IrElementKind::InlineCode => {
            if !children
                .iter()
                .all(|c| matches!(c, IrViewNode::Text { .. }))
            {
                return Err(refuse(
                    "code-child-invalid",
                    "a Code holds text and nothing else: it is shown as it is written".to_string(),
                    span,
                ));
            }
            if matches!(kind, IrElementKind::Code)
                && code_spec(bindings).0
                && !c.requires.iter().any(|r| r.name == "Clipboard")
            {
                return Err(refuse(
                    "clipboard-not-declared",
                    "`Code { copy: true }` puts text on the clipboard when its button is tapped: declare `requires Clipboard;` in this component".to_string(),
                    span,
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

// ---- the DOM recipe ------------------------------------------------------------------------------------------------------------------

/// The tag an element becomes: a `Heading` is `h1` to `h4` by its level, a `Text` with a `style:` is a paragraph.
pub fn tag(kind: &IrElementKind, bindings: &[IrBinding]) -> &'static str {
    match kind {
        IrElementKind::Heading => match bindings.iter().find(|b| b.key == "level").and_then(int_of)
        {
            Some(1) => "h1",
            Some(3) => "h3",
            Some(4) => "h4",
            _ => "h2",
        },
        IrElementKind::Text if bindings.iter().any(|b| b.key == "style") => "p",
        k => k.tag_name(),
    }
}

/// The attributes an element gets from its bindings, in a fixed order: `(name, value)`.
pub fn attrs(kind: &IrElementKind, bindings: &[IrBinding]) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for b in bindings {
        match (b.key.as_str(), string_of(b)) {
            ("id", Some(s)) if is_structure(kind) => {
                out.push(("id", s.to_string()));
                // a `Main` that can be linked to can be started from (docs/page.md section 4)
                if matches!(kind, IrElementKind::Main) {
                    out.push(("tabindex", "-1".to_string()));
                }
            }
            ("visible", _) | ("hidden", _)
                if matches!(&b.value, IrBindingValue::EventRef(_)) && is_structure(kind) =>
            {
                if let IrBindingValue::EventRef(n) = &b.value {
                    out.push((
                        if b.key == "visible" {
                            "data-cui-visible"
                        } else {
                            "data-cui-hidden"
                        },
                        n.clone(),
                    ));
                }
            }
            ("to", _) if matches!(kind, IrElementKind::CountUp) => {
                if let Some(n) = int_of(b) {
                    out.push(("data-to", n.to_string()));
                }
            }
            ("to", Some(s)) if matches!(kind, IrElementKind::Link) => {
                out.push(("href", s.to_string()))
            }
            ("style", Some(s)) if matches!(kind, IrElementKind::Text | IrElementKind::Rich) => {
                out.push(("data-style", s.to_string()))
            }
            _ => {}
        }
    }
    out
}

/// The class an element gets: a `Link` with an `appearance:`.
pub fn class(kind: &IrElementKind, bindings: &[IrBinding]) -> Option<String> {
    if matches!(kind, IrElementKind::Header) {
        return is_sticky(bindings).then(|| "cui-sticky".to_string());
    }
    match kind {
        IrElementKind::Reveal => return Some("cui-reveal".to_string()),
        IrElementKind::CountUp => return Some("cui-countup".to_string()),
        IrElementKind::Chip => {
            return bindings
                .iter()
                .find(|b| b.key == "tone")
                .and_then(string_of)
                .map(|t| format!("cui-chip cui-chip-{t}"));
        }
        _ => {}
    }
    if !matches!(kind, IrElementKind::Link) {
        return None;
    }
    bindings
        .iter()
        .find(|b| b.key == "appearance")
        .and_then(string_of)
        .map(|a| format!("cui-link-{a}"))
}

/// Whether a `Header` is written `sticky: true`.
pub fn is_sticky(bindings: &[IrBinding]) -> bool {
    bindings.iter().any(|b| {
        b.key == "sticky"
            && matches!(&b.value, IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { value: true, .. }))
    })
}

/// The bindings the structure recipe writes (`attrs`, `class`, a `Details`' summary): the generic binding loop skips them.
pub fn handles(kind: &IrElementKind, key: &str) -> bool {
    (is_structure(kind) && keys(kind).contains(&key))
        || (matches!(kind, IrElementKind::Text) && key == "style")
}

/// What a `Code { .. }` block asks for: whether it has a copy button, and the height (in rem) after which it scrolls.
pub fn code_spec(bindings: &[IrBinding]) -> (bool, Option<i64>) {
    let copy = bindings.iter().any(|b| {
        b.key == "copy"
            && matches!(&b.value, IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { value: true, .. }))
    });
    let max = bindings
        .iter()
        .find(|b| b.key == "max_height")
        .and_then(int_of);
    (copy, max)
}

/// Whether a view draws a `Code { copy: true }`: the use of `Clipboard` it declares.
pub fn view_has_copy(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            (matches!(kind, IrElementKind::Code) && code_spec(bindings).0)
                || children.iter().any(view_has_copy)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(view_has_copy)
        }
        IrViewNode::Text { .. } => false,
    }
}

/// The line that opens a `Details`.
pub fn summary(bindings: &[IrBinding]) -> Option<String> {
    bindings
        .iter()
        .find(|b| b.key == "summary")
        .and_then(string_of)
        .map(str::to_string)
}

// ---- what the whole program says ----------------------------------------------------------------------------------------------------

struct Walk<'a> {
    ids: Vec<(&'a str, Span)>,
    /// the kind of the element each id is on
    id_kinds: Vec<(&'a str, IrElementKind)>,
    /// a `Media`'s `caption:` (true) and `transcript:` (false): the id, and where it is written
    media_refs: Vec<(bool, String, Span)>,
    anchors: Vec<(&'a str, Span)>,
    mains: Vec<Span>,
    navs: Vec<(bool, Span)>,
    /// how many sectioning elements the walk is inside: a sticky `Header` is not one of them
    inside: usize,
}

fn walk<'a>(
    node: &'a IrViewNode,
    parent: Option<&IrElementKind>,
    w: &mut Walk<'a>,
    last_level: &mut Option<i64>,
) -> Result<(), LowerError> {
    match node {
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span,
        } => {
            for b in bindings {
                if b.key == "id"
                    && is_structure(kind)
                    && let Some(s) = string_of(b)
                {
                    w.ids.push((s, b.span));
                    w.id_kinds.push((s, kind.clone()));
                }
                if b.key == "to"
                    && matches!(kind, IrElementKind::Link)
                    && let Some(frag) = string_of(b).and_then(|s| s.strip_prefix('#'))
                {
                    w.anchors.push((frag, b.span));
                }
            }
            w.media_refs
                .extend(crate::media::references(kind, bindings));
            if let Some(p) = parent
                && is_inline_container(p)
                && !inline_child_ok(p, kind)
            {
                return Err(refuse(
                    "rich-child-invalid",
                    format!(
                        "{} holds text and `Strong`, `Em`, `Small`, `Break`, `Code` and `Link`: {} does not belong in it",
                        name(p),
                        name(kind)
                    ),
                    *span,
                ));
            }
            if is_inline_only(kind) && !parent.is_some_and(is_inline_container) {
                return Err(refuse(
                    "inline-outside-rich",
                    format!(
                        "{} is inline content: it goes inside a `Rich`, a `Heading`, a `Caption`, a `Link` or another inline element",
                        name(kind)
                    ),
                    *span,
                ));
            }
            if matches!(kind, IrElementKind::Summary)
                && !matches!(parent, Some(IrElementKind::Details))
            {
                return Err(refuse(
                    "details-incomplete",
                    "a Summary is the first child of a Details".to_string(),
                    *span,
                ));
            }
            if matches!(kind, IrElementKind::Header) && is_sticky(bindings) && w.inside > 0 {
                return Err(refuse(
                    "header-sticky-misplaced",
                    "a sticky Header is the banner of the page: inside a Section, an Article, an Aside, a Main, a Nav, a Footer, a Figure, a Details or another Header it is not the banner, and it would stick only as far as its parent goes".to_string(),
                    *span,
                ));
            }
            match kind {
                IrElementKind::Main => w.mains.push(*span),
                IrElementKind::Nav => w.navs.push((
                    accessibility.label.is_some() || accessibility.label_expr.is_some(),
                    *span,
                )),
                IrElementKind::Caption if !matches!(parent, Some(IrElementKind::Figure)) => {
                    return Err(refuse(
                        "caption-misplaced",
                        "a Caption is the direct child of a Figure: it is the figcaption of that figure".to_string(),
                        *span,
                    ));
                }
                IrElementKind::Figure => {
                    let n = children
                        .iter()
                        .filter(|c| {
                            matches!(
                                c,
                                IrViewNode::Element {
                                    kind: IrElementKind::Caption,
                                    ..
                                }
                            )
                        })
                        .count();
                    if n > 1 {
                        return Err(refuse(
                            "caption-misplaced",
                            "a Figure has one Caption".to_string(),
                            *span,
                        ));
                    }
                }
                IrElementKind::Heading => {
                    if let Some(level) = bindings.iter().find(|b| b.key == "level").and_then(int_of)
                    {
                        if let Some(prev) = *last_level
                            && level > prev + 1
                        {
                            return Err(refuse(
                                "heading-level-skipped",
                                format!(
                                    "a level {level} heading follows a level {prev} heading: a level is not skipped, because the outline of the page would have a hole"
                                ),
                                *span,
                            ));
                        }
                        *last_level = Some(level);
                    }
                }
                // a component placed here has headings of its own that this view cannot see: the order is unknown after it
                IrElementKind::Named(_) => *last_level = None,
                _ => {}
            }
            let sectioning = matches!(
                kind,
                IrElementKind::Section
                    | IrElementKind::Article
                    | IrElementKind::Aside
                    | IrElementKind::Main
                    | IrElementKind::Nav
                    | IrElementKind::Footer
                    | IrElementKind::Figure
                    | IrElementKind::Details
                    | IrElementKind::Header
            );
            w.inside += usize::from(sectioning);
            for c in children {
                walk(c, Some(kind), w, last_level)?;
            }
            w.inside -= usize::from(sectioning);
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                walk(c, parent, w, last_level)?;
            }
        }
        IrViewNode::Text { .. } => {}
    }
    Ok(())
}

/// The checks that need the whole program: heading order, anchors, landmarks, captions.
pub fn check_program(ir: &IrFile) -> Result<(), LowerError> {
    let mut w = Walk {
        ids: Vec::new(),
        id_kinds: Vec::new(),
        media_refs: Vec::new(),
        anchors: Vec::new(),
        mains: Vec::new(),
        navs: Vec::new(),
        inside: 0,
    };
    for c in &ir.components {
        let mut last = None;
        walk(&c.view.body, None, &mut w, &mut last)?;
    }
    let mut seen = BTreeSet::new();
    for (id, span) in &w.ids {
        if !seen.insert(*id) {
            return Err(refuse(
                "anchor-duplicate",
                format!(
                    "the id \"{id}\" is given to two elements of this program; a link to it would go to one of them"
                ),
                *span,
            ));
        }
    }
    for (is_caption, id, span) in &w.media_refs {
        let key = if *is_caption { "caption" } else { "transcript" };
        match w.id_kinds.iter().find(|(i, _)| i == id) {
            None => {
                return Err(refuse(
                    "media-reference-invalid",
                    format!(
                        "`{key}: \"{id}\"` points at an element with `id: \"{id}\"`, and no element of this program has it"
                    ),
                    *span,
                ));
            }
            Some((_, k)) if *is_caption && !matches!(k, IrElementKind::Caption) => {
                return Err(refuse(
                    "media-reference-invalid",
                    format!(
                        "`caption: \"{id}\"` names {}, and a caption is a `Caption`: write the description in a `Caption {{ id: \"{id}\" .. }}`",
                        name(k)
                    ),
                    *span,
                ));
            }
            _ => {}
        }
    }
    for (frag, span) in &w.anchors {
        if !seen.contains(frag) {
            return Err(refuse(
                "anchor-unknown",
                format!(
                    "`to: \"#{frag}\"` points at an element with `id: \"{frag}\"`, and no element of this program has it"
                ),
                *span,
            ));
        }
    }
    if let Some(second) = w.mains.get(1) {
        return Err(refuse(
            "landmark-duplicate",
            "a page has one Main: this is a second".to_string(),
            *second,
        ));
    }
    if w.navs.len() > 1
        && let Some((_, span)) = w.navs.iter().find(|(named, _)| !named)
    {
        return Err(refuse(
            "nav-label-missing",
            "with more than one Nav, each needs `label:` so a screen reader can tell them apart: `Nav { label: \"Sections\" .. }`".to_string(),
            *span,
        ));
    }
    Ok(())
}

/// The hosts a view links to (`https://host/..`), and `mailto:` domains, sorted and without repeats: what `inspect effects` and `diff` list.
pub fn link_hosts(node: &IrViewNode) -> BTreeSet<String> {
    fn go(node: &IrViewNode, out: &mut BTreeSet<String>) {
        match node {
            IrViewNode::Element {
                kind,
                bindings,
                children,
                ..
            } => {
                if matches!(kind, IrElementKind::Link)
                    && let Some(to) = bindings.iter().find(|b| b.key == "to").and_then(string_of)
                {
                    let lower = to.to_ascii_lowercase();
                    if lower.starts_with("http://") || lower.starts_with("https://") {
                        if let Some(h) = host_of(to) {
                            out.insert(h);
                        }
                    } else if lower.starts_with("mailto:")
                        && let Some(domain) = to
                            .rsplit_once('@')
                            .map(|(_, d)| d.split(['?', '#']).next().unwrap_or(d))
                    {
                        out.insert(format!("mailto:{}", domain.to_ascii_lowercase()));
                    }
                }
                for c in children {
                    go(c, out);
                }
            }
            IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                for c in children {
                    go(c, out);
                }
            }
            IrViewNode::Text { .. } => {}
        }
    }
    let mut out = BTreeSet::new();
    go(node, &mut out);
    out
}
