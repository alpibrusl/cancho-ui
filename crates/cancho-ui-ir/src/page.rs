//! `page { .. }` (docs/page.md): the metadata of the document, checked; and the two things a page does to the program, a skip link and a
//! `Main` to land on. The entries are decided here, in one place, so the build, `inspect ir` and `diff` read the same page.

use crate::ir::*;
use crate::lower::LowerError;
use cancho_ui_syntax::ast::{File, PageDecl, PageEntry, PageValue};
use cancho_ui_syntax::span::Span;

/// The settings a `page { .. }` takes.
pub const KEYS: &[&str] = &[
    "title",
    "description",
    "social_description",
    "lang",
    "canonical",
    "image",
    "image_width",
    "image_height",
    "image_alt",
    "icon",
    "apple_icon",
    "alternate",
    "skip",
];

/// The length of `description` in characters. The page's own is 261, so the 160 of `www.md` 5.3 would have refused it; the ceiling is against
/// stuffing, not a measure of any search engine's snippet.
pub const DESCRIPTION: (usize, usize) = (50, 300);
/// The length of `social_description` (`og:description`); the page's own is 111.
pub const SOCIAL_DESCRIPTION: (usize, usize) = (20, 300);
pub const TITLE_MAX: usize = 120;
pub const ALTERNATES_MAX: usize = 8;
pub const IMAGE_SIDE: (i64, i64) = (1, 8192);
pub const SKIP_MAX: usize = 60;
pub const DEFAULT_SKIP: &str = "Skip to content";

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// An `https://host/path` address: a host, and no space, quote, angle bracket or control character that would end an attribute.
pub fn is_https(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
        && !s.chars().any(|c| {
            c.is_control() || c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '\\')
        })
        && s.len() <= 400
}

/// A file beside the program (`logo.png`, `data/ecosystem.json`): relative, no `..`, no empty segment, no scheme.
pub fn is_relative_file(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && !s.starts_with('/')
        && s.split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn is_lang(s: &str) -> bool {
    let mut parts = s.split('-');
    let ok_first = parts
        .next()
        .is_some_and(|p| (2..=3).contains(&p.len()) && p.chars().all(|c| c.is_ascii_lowercase()));
    let rest: Vec<&str> = parts.collect();
    ok_first
        && rest.len() <= 3
        && rest
            .iter()
            .all(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn is_mime(s: &str) -> bool {
    let mut it = s.split('/');
    match (it.next(), it.next(), it.next()) {
        (Some(a), Some(b), None) => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_lowercase())
                && b.chars().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '+' | '-')
                })
        }
        _ => false,
    }
}

fn one_line(s: &str) -> bool {
    !s.chars().any(|c| c.is_control())
}

fn text(e: &PageEntry) -> Result<&str, LowerError> {
    match &e.value {
        PageValue::Text(s) => Ok(s),
        PageValue::Int(_) => Err(refuse(
            "page-entry-type",
            format!("`{}:` is text: write it as a string literal", e.key),
            e.span,
        )),
    }
}

fn int(e: &PageEntry) -> Result<i64, LowerError> {
    match &e.value {
        PageValue::Int(n) => Ok(*n),
        PageValue::Text(_) => Err(refuse(
            "page-entry-type",
            format!("`{}:` is a whole number of pixels", e.key),
            e.span,
        )),
    }
}

/// A link a page makes to a file or an address: `https://..`, or a file beside the program.
fn link(e: &PageEntry, what: &str) -> Result<String, LowerError> {
    let s = text(e)?;
    if is_https(s) || is_relative_file(s) {
        Ok(s.to_string())
    } else {
        Err(refuse(
            "page-link-invalid",
            format!(
                "`{}: \"{s}\"`: {what} is an `https://` address or a file beside the program (letters, digits, `.`, `_`, `-` and `/`; no `..`, no leading `/`, no scheme)",
                e.key
            ),
            e.span,
        ))
    }
}

/// The page of a file: none, or the one it declares.
pub fn lower_decls(file: &File) -> Result<Option<IrPage>, LowerError> {
    if let Some(second) = file.pages.get(1) {
        return Err(refuse(
            "page-duplicate",
            "a program has one `page { .. }`; this is a second one in the same file".to_string(),
            second.span,
        ));
    }
    file.pages.first().map(lower_page).transpose()
}

fn lower_page(d: &PageDecl) -> Result<IrPage, LowerError> {
    let mut seen: Vec<&str> = Vec::new();
    for e in &d.entries {
        if !KEYS.contains(&e.key.as_str()) {
            return Err(refuse(
                "page-entry-unknown",
                format!(
                    "`{}:` is not a setting of a page; the settings are {}",
                    e.key,
                    KEYS.join(", ")
                ),
                e.span,
            ));
        }
        if e.key != "alternate" && seen.contains(&e.key.as_str()) {
            return Err(refuse(
                "page-entry-duplicate",
                format!("`{}:` is written twice", e.key),
                e.span,
            ));
        }
        if e.key != "alternate" && !e.extras.is_empty() {
            return Err(refuse(
                "page-entry-unknown",
                format!(
                    "`{}:` takes one value; `{}:` after it belongs to `alternate:`",
                    e.key, e.extras[0].0
                ),
                e.span,
            ));
        }
        seen.push(&e.key);
    }
    let get = |k: &str| d.entries.iter().find(|e| e.key == k);
    let chars = |s: &str| s.chars().count();

    let title = match get("title") {
        Some(e) => text(e)?.to_string(),
        None => String::new(),
    };
    if title.trim().is_empty() || chars(&title) > TITLE_MAX || !one_line(&title) {
        return Err(refuse(
            "page-title-missing",
            format!(
                "a page has a `title:` of one line, from 1 to {TITLE_MAX} characters: it is what a tab, a bookmark and a search result call the page"
            ),
            get("title").map_or(d.span, |e| e.span),
        ));
    }
    let description = match get("description") {
        Some(e) => text(e)?.to_string(),
        None => String::new(),
    };
    if chars(&description) < DESCRIPTION.0
        || chars(&description) > DESCRIPTION.1
        || !one_line(&description)
    {
        return Err(refuse(
            "page-description-length",
            format!(
                "`description:` is one line of {} to {} characters (it is {}): shorter says nothing a link preview can use, longer is stuffing",
                DESCRIPTION.0,
                DESCRIPTION.1,
                chars(&description)
            ),
            get("description").map_or(d.span, |e| e.span),
        ));
    }
    let social_description = match get("social_description") {
        Some(e) => {
            let s = text(e)?;
            if chars(s) < SOCIAL_DESCRIPTION.0 || chars(s) > SOCIAL_DESCRIPTION.1 || !one_line(s) {
                return Err(refuse(
                    "page-description-length",
                    format!(
                        "`social_description:` is one line of {} to {} characters (it is {})",
                        SOCIAL_DESCRIPTION.0,
                        SOCIAL_DESCRIPTION.1,
                        chars(s)
                    ),
                    e.span,
                ));
            }
            Some(s.to_string())
        }
        None => None,
    };
    let lang = match get("lang") {
        Some(e) => {
            let s = text(e)?;
            if !is_lang(s) {
                return Err(refuse(
                    "page-lang-invalid",
                    format!(
                        "`lang: \"{s}\"` is a language tag such as `en`, `es` or `pt-BR`: lower-case letters, then up to three `-` parts"
                    ),
                    e.span,
                ));
            }
            s.to_string()
        }
        None => "en".to_string(),
    };
    let canonical = match get("canonical") {
        Some(e) => {
            let s = text(e)?;
            if !is_https(s) || s.contains('#') {
                return Err(refuse(
                    "page-canonical-not-https",
                    format!(
                        "`canonical: \"{s}\"` is the address of this page as an `https://` address without a `#`: a link preview and a search engine take it as the page's name"
                    ),
                    e.span,
                ));
            }
            s.to_string()
        }
        None => {
            return Err(refuse(
                "page-canonical-not-https",
                "a page has a `canonical:`, the `https://` address it is served at".to_string(),
                d.span,
            ));
        }
    };
    let image = match get("image") {
        Some(e) => {
            let url = text(e)?;
            if !is_https(url) {
                return Err(refuse(
                    "page-image-not-https",
                    format!(
                        "`image: \"{url}\"` is an `https://` address: a link preview fetches it from outside the site, so it cannot be a file beside the program"
                    ),
                    e.span,
                ));
            }
            let side = |k: &str| -> Result<u32, LowerError> {
                match get(k) {
                    None => Err(refuse(
                        "page-image-size-missing",
                        format!(
                            "an `image:` needs `{k}:` in pixels: a preview reserves the space before the picture has loaded"
                        ),
                        e.span,
                    )),
                    Some(s) => {
                        let n = int(s)?;
                        if (IMAGE_SIDE.0..=IMAGE_SIDE.1).contains(&n) {
                            Ok(n as u32)
                        } else {
                            Err(refuse(
                                "page-image-size-invalid",
                                format!(
                                    "`{k}:` is from {} to {} pixels",
                                    IMAGE_SIDE.0, IMAGE_SIDE.1
                                ),
                                s.span,
                            ))
                        }
                    }
                }
            };
            let (width, height) = (side("image_width")?, side("image_height")?);
            let alt = match get("image_alt") {
                Some(a) => {
                    let s = text(a)?;
                    if s.trim().is_empty() || chars(s) > 420 || !one_line(s) {
                        return Err(refuse(
                            "page-image-alt-missing",
                            "`image_alt:` says in one line, up to 420 characters, what the picture shows".to_string(),
                            a.span,
                        ));
                    }
                    s.to_string()
                }
                None => {
                    return Err(refuse(
                        "page-image-alt-missing",
                        "an `image:` needs `image_alt:`, what the picture shows, for the people and tools that cannot see it".to_string(),
                        e.span,
                    ));
                }
            };
            Some(IrPageImage {
                url: url.to_string(),
                width,
                height,
                alt,
            })
        }
        None => {
            if let Some(e) = ["image_width", "image_height", "image_alt"]
                .iter()
                .find_map(|k| get(k))
            {
                return Err(refuse(
                    "page-image-size-missing",
                    format!(
                        "`{}:` describes the `image:`, and the page declares none",
                        e.key
                    ),
                    e.span,
                ));
            }
            None
        }
    };
    let icon = get("icon").map(|e| link(e, "an icon")).transpose()?;
    let apple_icon = get("apple_icon").map(|e| link(e, "an icon")).transpose()?;
    let mut alternates = Vec::new();
    for e in d.entries.iter().filter(|e| e.key == "alternate") {
        let href = link(e, "an alternate")?;
        let find = |k: &str| {
            e.extras
                .iter()
                .filter(|(x, _, _)| x == k)
                .map(|(_, v, _)| v.as_str())
                .collect::<Vec<_>>()
        };
        let (mimes, titles) = (find("mime"), find("title"));
        let extra_unknown = e
            .extras
            .iter()
            .find(|(k, _, _)| k != "mime" && k != "title");
        if mimes.len() != 1 || titles.len() != 1 || extra_unknown.is_some() {
            return Err(refuse(
                "page-alternate-invalid",
                "an `alternate:` is `alternate: \"agents.md\" mime: \"text/markdown\" title: \"..\";`: the file, its media type and a title, once each".to_string(),
                e.span,
            ));
        }
        if !is_mime(mimes[0])
            || titles[0].trim().is_empty()
            || chars(titles[0]) > 120
            || !one_line(titles[0])
        {
            return Err(refuse(
                "page-alternate-invalid",
                format!(
                    "`mime: \"{}\"` is a media type such as `text/markdown` or `application/json`, and `title:` is one line of up to 120 characters",
                    mimes[0]
                ),
                e.span,
            ));
        }
        alternates.push(IrAlternate {
            href,
            mime: mimes[0].to_string(),
            title: titles[0].to_string(),
        });
    }
    if alternates.len() > ALTERNATES_MAX {
        return Err(refuse(
            "page-alternate-invalid",
            format!("a page has at most {ALTERNATES_MAX} alternates"),
            d.span,
        ));
    }
    let skip = match get("skip") {
        Some(e) => {
            let s = text(e)?;
            if s.trim().is_empty() || chars(s) > SKIP_MAX || !one_line(s) {
                return Err(refuse(
                    "page-skip-text-invalid",
                    format!(
                        "`skip:` is the words of the skip link, one line of up to {SKIP_MAX} characters"
                    ),
                    e.span,
                ));
            }
            Some(s.to_string())
        }
        None => None,
    };
    if skip.is_none() && lang.split('-').next() != Some("en") {
        return Err(refuse(
            "page-skip-text-missing",
            format!(
                "the page is `lang: \"{lang}\"` and the compiler writes the skip link's words only in English: write `skip: \"..\";` in the page's language"
            ),
            d.span,
        ));
    }
    Ok(IrPage {
        title,
        description,
        social_description,
        lang,
        canonical,
        image,
        icon,
        apple_icon,
        alternates,
        skip,
        span: d.span,
    })
}

/// The page as canonical JSON, for `inspect ir`.
pub fn to_json(p: &IrPage) -> serde_json::Value {
    let mut v = serde_json::Map::new();
    for (k, val) in flatten(p) {
        v.insert(k, serde_json::Value::String(val));
    }
    serde_json::Value::Object(v)
}

/// Every setting of a page as `(name, value)`, for the diff and `inspect ir`: `title`, `image.width`, `alternate.agents.md.mime`.
pub fn flatten(p: &IrPage) -> Vec<(String, String)> {
    let mut o = vec![
        ("title".to_string(), p.title.clone()),
        ("description".to_string(), p.description.clone()),
        ("lang".to_string(), p.lang.clone()),
        ("canonical".to_string(), p.canonical.clone()),
    ];
    if let Some(s) = &p.social_description {
        o.push(("social_description".into(), s.clone()));
    }
    if let Some(i) = &p.image {
        o.push(("image".into(), i.url.clone()));
        o.push(("image.width".into(), i.width.to_string()));
        o.push(("image.height".into(), i.height.to_string()));
        o.push(("image.alt".into(), i.alt.clone()));
    }
    if let Some(s) = &p.icon {
        o.push(("icon".into(), s.clone()));
    }
    if let Some(s) = &p.apple_icon {
        o.push(("apple_icon".into(), s.clone()));
    }
    for a in &p.alternates {
        o.push((format!("alternate.{}", a.href), a.href.clone()));
        o.push((format!("alternate.{}.mime", a.href), a.mime.clone()));
        o.push((format!("alternate.{}.title", a.href), a.title.clone()));
    }
    if let Some(s) = &p.skip {
        o.push(("skip".into(), s.clone()));
    }
    o
}

/// The files a page names beside the program: its icons and its alternates, in the order written.
pub fn files(p: &IrPage) -> Vec<&str> {
    let mut o: Vec<&str> = Vec::new();
    for s in [p.icon.as_deref(), p.apple_icon.as_deref()]
        .into_iter()
        .flatten()
    {
        o.push(s);
    }
    for a in &p.alternates {
        o.push(&a.href);
    }
    o.retain(|s| is_relative_file(s));
    o
}

fn mains<'a>(node: &'a IrViewNode, out: &mut Vec<(&'a [IrBinding], Span)>) {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            span,
            ..
        } => {
            if matches!(kind, IrElementKind::Main) {
                out.push((bindings, *span));
            }
            for c in children {
                mains(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                mains(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// What a page does to its program (docs/page.md section 4): it needs a `Main` with an `id:` to skip to, and it gets a skip link first in the
/// component the page mounts. Run on the merged program, so the `Main` may be in an imported module.
pub fn apply(ir: &mut IrFile) -> Result<(), LowerError> {
    let Some(page) = ir.page.clone() else {
        return Ok(());
    };
    let mut found = Vec::new();
    for c in &ir.components {
        mains(&c.view.body, &mut found);
    }
    let Some((bindings, main_span)) = found.first() else {
        return Err(refuse(
            "page-main-missing",
            "a program with a `page { .. }` is a document, and a document has a `Main`: the skip link goes there".to_string(),
            page.span,
        ));
    };
    let id = bindings
        .iter()
        .find(|b| b.key == "id")
        .and_then(|b| match &b.value {
            IrBindingValue::String(s) => Some(s.clone()),
            _ => None,
        });
    let Some(id) = id else {
        return Err(refuse(
            "skip-target-missing",
            "the `Main` has no `id:`; the skip link goes to it: `Main { id: \"main\" .. }`"
                .to_string(),
            *main_span,
        ));
    };
    let words = page
        .skip
        .clone()
        .unwrap_or_else(|| DEFAULT_SKIP.to_string());
    let Some(last) = ir.components.last_mut() else {
        return Ok(());
    };
    let span = last.view.span;
    let skip = IrViewNode::Element {
        kind: IrElementKind::Link,
        accessibility: IrAccessibility::default(),
        bindings: vec![
            IrBinding {
                key: "to".into(),
                value: IrBindingValue::String(format!("#{id}")),
                span,
            },
            IrBinding {
                key: "appearance".into(),
                value: IrBindingValue::String("skip".into()),
                span,
            },
        ],
        children: vec![IrViewNode::Text {
            parts: vec![IrTextPart::Literal(words)],
            span,
        }],
        span,
    };
    match &mut last.view.body {
        IrViewNode::Element { children, .. } => children.insert(0, skip),
        other => {
            let inner = std::mem::replace(
                other,
                IrViewNode::Text {
                    parts: Vec::new(),
                    span,
                },
            );
            *other = IrViewNode::Element {
                kind: IrElementKind::Column,
                accessibility: IrAccessibility::default(),
                bindings: Vec::new(),
                children: vec![skip, inner],
                span,
            };
        }
    }
    Ok(())
}
