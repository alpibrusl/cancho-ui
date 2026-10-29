//! `page { .. }` (docs/page.md): the title, language, description, canonical address, card image, icons and alternates of the document, and the text of its
//! skip link. Pure: nothing here reads a file (the CLI reads the files a page names, fills their hashes and the size of the card image). The head
//! is written from the checked value by `head_html`, which the page generator calls, so the head is static HTML, in the document before any script.

use cancho_ui_syntax::Span;
use cancho_ui_syntax::ast::{File, PageDecl};

use crate::ir::*;
use crate::lower::LowerError;

/// The most characters of a `description:` or a `share_description:`. Not a limit of any search engine (none was measured): the page's own is
/// 261 characters, and this is the smallest round number that admits it (docs/page.md section 2).
pub const DESCRIPTION_MAX: usize = 300;
/// The formats of the card image, with the media type each extension says.
pub const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
];
/// The formats of an icon.
pub const ICON_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("ico", "image/x-icon"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
];
/// The formats of an alternate representation of the page.
pub const ALTERNATE_TYPES: &[(&str, &str)] = &[
    ("md", "text/markdown"),
    ("json", "application/json"),
    ("txt", "text/plain"),
    ("xml", "application/xml"),
];
/// The entries of a `page { .. }` that may be written once, and the entries that may be written many times.
const ONCE: &[&str] = &[
    "title",
    "lang",
    "description",
    "share_description",
    "canonical",
    "image",
    "icon",
    "touch_icon",
    "skip_link",
    "measure",
    "scale",
];
/// The width of the centred column, in rem: from a phone held on its side to a wide desktop.
pub const MEASURE_RANGE: (u32, u32) = (30, 100);
const MANY: &[&str] = &["alternate"];

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// The attributes an entry takes beside its own value.
fn attributes(key: &str) -> &'static [&'static str] {
    match key {
        "image" => &["alt"],
        "alternate" => &["title"],
        _ => &[],
    }
}

/// `Some(why)` when a text cannot be one line of a `<meta>` or a `<title>`.
fn control_in(text: &str) -> Option<char> {
    text.chars().find(|c| c.is_control())
}

fn checked_text(
    what: &str,
    text: &str,
    blank: &'static str,
    span: Span,
) -> Result<String, LowerError> {
    if let Some(c) = control_in(text) {
        return Err(refuse(
            "page-text-invalid",
            format!(
                "{what} holds a control character (U+{:04X}): a line break in a `<meta>` or a `<title>` is not part of its text",
                c as u32
            ),
            span,
        ));
    }
    if text.trim().is_empty() {
        return Err(refuse(blank, format!("{what} is blank"), span));
    }
    Ok(text.to_string())
}

/// A BCP 47 shape: a language of two or three letters, then parts of one to eight letters or digits (`en`, `es-419`, `zh-Hant-TW`).
pub fn lang_ok(lang: &str) -> bool {
    let mut parts = lang.split('-');
    let first = parts.next().unwrap_or("");
    (2..=3).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && lang.len() <= 35
        && parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// What is wrong with a `canonical:`, as a rule tag and a sentence; `Ok` carries nothing.
pub fn check_canonical(url: &str) -> Result<(), (&'static str, String)> {
    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("https://")
        .map(|_| &url["https://".len()..])
    else {
        return Err((
            "page-canonical-not-https",
            "`canonical:` is an address that starts with `https://`: a canonical address is the one a reader is meant to share".to_string(),
        ));
    };
    if let Some(c) = url.chars().find(|c| {
        c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\'' | '\\' | '#')
    }) {
        return Err((
            "page-canonical-invalid",
            format!(
                "`canonical:` holds `{}`: no space, control character, quote, angle bracket, backslash or `#fragment`",
                c.escape_default()
            ),
        ));
    }
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let authority = &rest[..end];
    // a user name before the host holds an `@`, which is not a character of a host: the host check below refuses it
    let host = authority.rsplit_once(':').map_or(authority, |(h, port)| {
        if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) {
            h
        } else {
            authority
        }
    });
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return Err((
            "page-canonical-invalid",
            "`canonical:` has no host, or one that is not a lower-case name (letters, digits, `.` and `-`)".to_string(),
        ));
    }
    Ok(())
}

/// The host of a checked canonical address.
pub fn canonical_host(url: &str) -> Option<String> {
    crate::structure::host_of(url)
}

/// The address with its last path segment cut off: the directory the card image's file is joined to (`https://h/dir/index.html` and
/// `https://h/dir/` are both `https://h/dir/`; `https://h` is `https://h/`).
pub fn canonical_base(url: &str) -> String {
    let no_query = url.split('?').next().unwrap_or(url);
    let scheme = "https://".len();
    match no_query
        .get(scheme..)
        .and_then(|rest| rest.rfind('/'))
        .and_then(|i| no_query.get(..scheme + i + 1))
    {
        Some(base) => base.to_string(),
        None => format!("{no_query}/"),
    }
}

/// The absolute address of the card image: the directory of `canonical:` and the path of the file.
pub fn image_url(p: &IrPage) -> Option<String> {
    let (c, img) = (p.canonical.as_ref()?, p.image.as_ref()?);
    Some(format!("{}{}", canonical_base(c), img.file.path))
}

fn page_file(
    path: &str,
    types: &[(&str, &'static str)],
    span: Span,
) -> Result<IrPageFile, LowerError> {
    let mime = crate::media::asset_type(path, types)
        .map_err(|why| refuse("page-file-invalid", format!("`{path}`: {why}"), span))?;
    Ok(IrPageFile {
        path: path.to_string(),
        mime,
        sha256: String::new(),
        bytes: 0,
        span,
    })
}

/// The program's `page { .. }`, checked; `None` when it has none.
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

fn lower_page(decl: &PageDecl) -> Result<IrPage, LowerError> {
    let mut seen: Vec<&str> = Vec::new();
    let mut page = IrPage {
        title: String::new(),
        lang: String::new(),
        description: None,
        share_description: None,
        canonical: None,
        image: None,
        icon: None,
        touch_icon: None,
        alternates: Vec::new(),
        skip_link: None,
        skip_link_span: None,
        measure: None,
        display_scale: false,
        span: decl.span,
    };
    let (mut has_title, mut has_lang) = (false, false);
    let mut image_span = None;
    for e in &decl.entries {
        let Some((key, value, key_span)) = e.pairs.first() else {
            continue;
        };
        let key = key.as_str();
        if !ONCE.contains(&key) && !MANY.contains(&key) {
            return Err(refuse(
                "page-entry-unknown",
                format!(
                    "`{key}:` is not an entry of a page; it has {}",
                    ONCE.iter()
                        .chain(MANY)
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                *key_span,
            ));
        }
        if ONCE.contains(&key) {
            if seen.contains(&key) {
                return Err(refuse(
                    "page-entry-duplicate",
                    format!("`{key}:` is written twice; a page has one"),
                    *key_span,
                ));
            }
            seen.push(key);
        }
        let mut attrs: Vec<(&str, &str, Span)> = Vec::new();
        for (name, v, s) in e.pairs.iter().skip(1) {
            if !attributes(key).contains(&name.as_str()) {
                return Err(refuse(
                    "page-entry-unknown",
                    format!(
                        "`{name}:` is not an attribute of `{key}:`{}",
                        match attributes(key) {
                            [] => "; it takes none".to_string(),
                            a => format!("; it takes {}", a.join(", ")),
                        }
                    ),
                    *s,
                ));
            }
            if attrs.iter().any(|(n, _, _)| *n == name) {
                return Err(refuse(
                    "page-entry-duplicate",
                    format!("`{name}:` is written twice on one `{key}:`"),
                    *s,
                ));
            }
            attrs.push((name, v, *s));
        }
        let attr = |n: &str| attrs.iter().find(|(a, _, _)| *a == n);
        match key {
            "title" => {
                page.title = checked_text("the title", value, "page-title-missing", *key_span)?;
                has_title = true;
            }
            "lang" => {
                if !lang_ok(value) {
                    return Err(refuse(
                        "page-lang-invalid",
                        format!(
                            "`lang: \"{value}\"` is not a language tag: a language of two or three letters, then parts of one to eight letters or digits (`en`, `es-419`, `zh-Hant-TW`)"
                        ),
                        *key_span,
                    ));
                }
                page.lang = value.clone();
                has_lang = true;
            }
            "description" | "share_description" => {
                let text = checked_text(
                    &format!("`{key}:`"),
                    value,
                    "page-description-length",
                    *key_span,
                )?;
                if text.chars().count() > DESCRIPTION_MAX {
                    return Err(refuse(
                        "page-description-length",
                        format!(
                            "`{key}:` is {} characters; at most {DESCRIPTION_MAX}",
                            text.chars().count()
                        ),
                        *key_span,
                    ));
                }
                if key == "description" {
                    page.description = Some(text);
                } else {
                    page.share_description = Some(text);
                }
            }
            "canonical" => {
                check_canonical(value).map_err(|(code, why)| refuse(code, why, *key_span))?;
                page.canonical = Some(value.clone());
            }
            "image" => {
                let f = page_file(value, IMAGE_TYPES, *key_span)?;
                let Some((_, alt, alt_span)) = attr("alt") else {
                    return Err(refuse(
                        "page-image-alt-missing",
                        "an `image:` needs `alt:`, the text a person who cannot see the card is given: `image: \"og.png\" alt: \"..\";`".to_string(),
                        *key_span,
                    ));
                };
                let alt = checked_text("`alt:`", alt, "page-image-alt-missing", *alt_span)?;
                image_span = Some(*key_span);
                page.image = Some(IrPageImage {
                    file: f,
                    alt,
                    width: 0,
                    height: 0,
                });
            }
            "icon" => page.icon = Some(page_file(value, ICON_TYPES, *key_span)?),
            "touch_icon" => page.touch_icon = Some(page_file(value, ICON_TYPES, *key_span)?),
            "alternate" => {
                let f = page_file(value, ALTERNATE_TYPES, *key_span)?;
                let Some((_, title, t_span)) = attr("title") else {
                    return Err(refuse(
                        "page-alternate-title-missing",
                        "an `alternate:` needs `title:`, the name a reader or an agent is shown for it: `alternate: \"agents.md\" title: \"..\";`".to_string(),
                        *key_span,
                    ));
                };
                let title = checked_text(
                    "the title of an alternate",
                    title,
                    "page-alternate-title-missing",
                    *t_span,
                )?;
                page.alternates.push(IrPageAlternate { file: f, title });
            }
            "measure" => {
                let n = value.parse::<u32>().ok().filter(|n| {
                    (MEASURE_RANGE.0..=MEASURE_RANGE.1).contains(n)
                        && value.bytes().all(|b| b.is_ascii_digit())
                });
                let Some(n) = n else {
                    return Err(refuse(
                        "page-measure-invalid",
                        format!(
                            "`measure: \"{value}\"` is a whole number of rem from {} to {}, as a string literal: `measure: \"72\";`",
                            MEASURE_RANGE.0, MEASURE_RANGE.1
                        ),
                        *key_span,
                    ));
                };
                page.measure = Some(n);
            }
            "scale" => {
                if value != "display" {
                    return Err(refuse(
                        "page-scale-invalid",
                        format!(
                            "`scale: \"{value}\"` is not a scale; the one there is is `\"display\"`"
                        ),
                        *key_span,
                    ));
                }
                page.display_scale = true;
            }
            _ => {
                // `skip_link`
                page.skip_link = Some(checked_text(
                    "`skip_link:`",
                    value,
                    "page-skip-link-empty",
                    *key_span,
                )?);
                page.skip_link_span = Some(*key_span);
            }
        }
    }
    if !has_title {
        return Err(refuse(
            "page-title-missing",
            "a page needs `title:`: it is the `<title>` and the title of the card".to_string(),
            decl.span,
        ));
    }
    if !has_lang {
        return Err(refuse(
            "page-lang-missing",
            "a page needs `lang:` (`lang: \"en\";`): a screen reader chooses its voice by it"
                .to_string(),
            decl.span,
        ));
    }
    if page.image.is_some() && page.canonical.is_none() {
        return Err(refuse(
            "page-image-needs-canonical",
            "a card image is named by an absolute address, and the compiler makes it from the directory of `canonical:`; this page has no `canonical:`".to_string(),
            image_span.unwrap_or(decl.span),
        ));
    }
    Ok(page)
}

// ---- the head ----------------------------------------------------------------------------------------------------------------------

/// `&`, `<`, `>` and `"` as references: the one escape of the head, for text and for attribute values.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The `<meta>` and `<link>` elements of a page, one to a line, in the order the cancho-www page has them (after `<title>`).
pub fn head_html(p: &IrPage) -> String {
    let mut s = String::new();
    let mut meta = |attr: &str, name: &str, content: &str| {
        s.push_str(&format!(
            "<meta {attr}=\"{name}\" content=\"{}\">\n",
            esc(content)
        ));
    };
    if let Some(d) = &p.description {
        meta("name", "description", d);
    }
    meta("property", "og:title", &p.title);
    if let Some(d) = p.share_description.as_ref().or(p.description.as_ref()) {
        meta("property", "og:description", d);
    }
    meta("property", "og:type", "website");
    if let Some(c) = &p.canonical {
        meta("property", "og:url", c);
    }
    if let (Some(img), Some(url)) = (&p.image, image_url(p)) {
        meta("property", "og:image", &url);
        meta("property", "og:image:width", &img.width.to_string());
        meta("property", "og:image:height", &img.height.to_string());
        meta("property", "og:image:alt", &img.alt);
        meta("name", "twitter:card", "summary_large_image");
    }
    if let Some(c) = &p.canonical {
        s.push_str(&format!("<link rel=\"canonical\" href=\"{}\">\n", esc(c)));
    }
    s.push_str("<meta name=\"color-scheme\" content=\"light dark\">\n");
    if let Some(i) = &p.icon {
        s.push_str(&format!(
            "<link rel=\"icon\" type=\"{}\" href=\"{}\">\n",
            i.mime,
            esc(&i.path)
        ));
    }
    if let Some(i) = &p.touch_icon {
        s.push_str(&format!(
            "<link rel=\"apple-touch-icon\" href=\"{}\">\n",
            esc(&i.path)
        ));
    }
    for a in &p.alternates {
        s.push_str(&format!(
            "<link rel=\"alternate\" type=\"{}\" href=\"{}\" title=\"{}\">\n",
            a.file.mime,
            esc(&a.file.path),
            esc(&a.title)
        ));
    }
    s
}

/// The skip link, the first element of `<body>`.
pub fn skip_html(text: &str, target: &str) -> String {
    format!(
        "<a class=\"cui-skip\" href=\"#{}\">{}</a>\n",
        esc(target),
        esc(text)
    )
}

/// The hosts a page names: where a crawler is sent for the canonical address and the card. The page makes no request to them.
pub fn hosts(p: &IrPage) -> Vec<String> {
    p.canonical
        .as_deref()
        .and_then(canonical_host)
        .into_iter()
        .collect()
}

/// Every file a page names, in the order written: `(role, file)`.
pub fn files(p: &IrPage) -> Vec<(String, &IrPageFile)> {
    let mut out: Vec<(String, &IrPageFile)> = Vec::new();
    if let Some(i) = &p.image {
        out.push(("image".to_string(), &i.file));
    }
    if let Some(i) = &p.icon {
        out.push(("icon".to_string(), i));
    }
    if let Some(i) = &p.touch_icon {
        out.push(("touch_icon".to_string(), i));
    }
    for a in &p.alternates {
        out.push((format!("alternate {}", a.file.path), &a.file));
    }
    out
}

/// The settings of a page as `(name, value)`, the way `diff` compares them and `inspect page` lists them. Hashes are `<name>.sha256`.
pub fn flatten(p: &IrPage) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![
        ("title".into(), p.title.clone()),
        ("lang".into(), p.lang.clone()),
    ];
    let mut add = |k: &str, v: &Option<String>| {
        if let Some(v) = v {
            out.push((k.to_string(), v.clone()));
        }
    };
    add("description", &p.description);
    add("share_description", &p.share_description);
    add("canonical", &p.canonical);
    add("skip_link", &p.skip_link);
    add("measure", &p.measure.map(|m| m.to_string()));
    add("scale", &p.display_scale.then(|| "display".to_string()));
    out.push((
        "host".into(),
        hosts(p).into_iter().next().unwrap_or_default(),
    ));
    if let Some(i) = &p.image {
        out.push(("image".into(), i.file.path.clone()));
        out.push(("image.alt".into(), i.alt.clone()));
        out.push(("image.size".into(), format!("{}x{}", i.width, i.height)));
    }
    for (role, f) in files(p) {
        // the card image's path was added with its alt text; an icon's is its own setting; an alternate's setting is its title (below)
        if role == "icon" || role == "touch_icon" {
            out.push((role.clone(), f.path.clone()));
        }
        out.push((format!("{role}.sha256"), f.sha256.clone()));
    }
    for a in &p.alternates {
        out.push((format!("alternate {}", a.file.path), a.title.clone()));
    }
    out.retain(|(k, v)| !(k == "host" && v.is_empty()));
    out
}

/// The page as JSON: the checked values, the resolved card address and the host.
pub fn to_json(p: &IrPage) -> serde_json::Value {
    let file_json = |f: &IrPageFile| serde_json::json!({ "path": f.path, "type": f.mime, "sha256": f.sha256, "bytes": f.bytes });
    let mut v = serde_json::json!({
        "title": p.title,
        "lang": p.lang,
        "hosts": hosts(p),
    });
    if let Some(o) = v.as_object_mut() {
        let mut put = |k: &str, x: serde_json::Value| {
            o.insert(k.to_string(), x);
        };
        if let Some(d) = &p.description {
            put("description", d.clone().into());
        }
        if let Some(d) = &p.share_description {
            put("share_description", d.clone().into());
        }
        if let Some(c) = &p.canonical {
            put("canonical", c.clone().into());
        }
        if let Some(i) = &p.image {
            put(
                "image",
                serde_json::json!({
                    "file": file_json(&i.file), "alt": i.alt, "width": i.width, "height": i.height,
                    "url": image_url(p),
                }),
            );
        }
        if let Some(i) = &p.icon {
            put("icon", file_json(i));
        }
        if let Some(i) = &p.touch_icon {
            put("touch_icon", file_json(i));
        }
        if !p.alternates.is_empty() {
            put(
                "alternates",
                p.alternates
                    .iter()
                    .map(|a| serde_json::json!({ "file": file_json(&a.file), "title": a.title }))
                    .collect::<Vec<_>>()
                    .into(),
            );
        }
        if let Some(s) = &p.skip_link {
            put("skip_link", s.clone().into());
        }
        if let Some(m) = p.measure {
            put("measure", m.into());
        }
        if p.display_scale {
            put("scale", "display".into());
        }
    }
    v
}

// ---- the whole program ---------------------------------------------------------------------------------------------------------------

/// Every element of a view, depth first, with whether it is inside a `For` or an `If`.
fn visit<'a>(
    node: &'a IrViewNode,
    dynamic: bool,
    f: &mut impl FnMut(&'a IrElementKind, &'a [IrBinding], Span, bool),
) {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            span,
            ..
        } => {
            f(kind, bindings, *span, dynamic);
            for c in children {
                visit(c, dynamic, f);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                visit(c, true, f);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

fn id_of(bindings: &[IrBinding]) -> Option<&str> {
    bindings
        .iter()
        .find(|b| b.key == "id")
        .and_then(|b| match &b.value {
            IrBindingValue::String(s) => Some(s.as_str()),
            _ => None,
        })
}

/// Whether the HTML has a `<main>` with this id.
fn has_main(html: &str, id: &str) -> bool {
    html.match_indices("<main").any(|(i, _)| {
        html[i..]
            .split('>')
            .next()
            .is_some_and(|tag| tag.contains(&format!(" id=\"{id}\"")))
    })
}

/// The checks that need the whole merged program: one sticky `Header`, and a skip link that has a `Main` to go to, in the HTML the page is
/// written with.
pub fn check_program(ir: &IrFile) -> Result<(), LowerError> {
    let mut sticky: Vec<Span> = Vec::new();
    let mut mains: Vec<(Option<String>, bool, Span, usize)> = Vec::new();
    for (n, c) in ir.components.iter().enumerate() {
        visit(
            &c.view.body,
            false,
            &mut |kind, bindings, span, dynamic| match kind {
                IrElementKind::Header if crate::structure::is_sticky(bindings) => sticky.push(span),
                IrElementKind::Main => {
                    mains.push((id_of(bindings).map(str::to_string), dynamic, span, n))
                }
                _ => {}
            },
        );
    }
    if let Some(second) = sticky.get(1) {
        return Err(refuse(
            "header-sticky-duplicate",
            "a page has one sticky Header: the room reserved above an anchor is one number"
                .to_string(),
            *second,
        ));
    }
    let Some(page) = &ir.page else {
        return Ok(());
    };
    let Some(skip_span) = page.skip_link_span else {
        return Ok(());
    };
    let Some((id, dynamic, span, n)) = mains.first() else {
        return Err(refuse(
            "skip-link-target-missing",
            "a page with a `skip_link:` has a `Main` to skip to, and this program has none"
                .to_string(),
            skip_span,
        ));
    };
    let Some(id) = id else {
        return Err(refuse(
            "skip-link-target-missing",
            "the skip link goes to the `Main`, and this `Main` has no `id:`: write `Main { id: \"main\" .. }`".to_string(),
            *span,
        ));
    };
    let mounted = ir.components.len().saturating_sub(1);
    let in_html = ir
        .components
        .get(mounted)
        .is_some_and(|c| has_main(&crate::recipe::prerender(&c.view.body).html, id));
    if *dynamic || *n != mounted || !in_html {
        return Err(refuse(
            "skip-link-target-dynamic",
            "the skip link goes to a `Main` that is not in the page's HTML: it is inside an `If` or a `For`, or in a component other than the one mounted into the page, so it exists only once a script has run. Put the `Main` in the mounted component, outside any `For` or `If`".to_string(),
            *span,
        ));
    }
    Ok(())
}

/// The id the skip link goes to, when the program has one.
pub fn skip_target(ir: &IrFile) -> Option<String> {
    ir.page.as_ref()?.skip_link.as_ref()?;
    let mut found = None;
    for c in &ir.components {
        visit(&c.view.body, false, &mut |kind, bindings, _, _| {
            if matches!(kind, IrElementKind::Main) && found.is_none() {
                found = id_of(bindings).map(str::to_string);
            }
        });
    }
    found
}

/// Whether any view has a sticky `Header`.
pub fn has_sticky(ir: &IrFile) -> bool {
    let mut any = false;
    for c in &ir.components {
        visit(&c.view.body, false, &mut |kind, bindings, _, _| {
            if matches!(kind, IrElementKind::Header) && crate::structure::is_sticky(bindings) {
                any = true;
            }
        });
    }
    any
}

/// Whether any view uses a role of the look (docs/www-port/look.md section 3): `card: true`, a `Grid`'s `align: "center"`, or a `Link` that is a `pill`.
/// The baseline writes the rules of those roles only for a program that does, so every other page is the bytes it was.
pub fn uses_look_roles(ir: &IrFile) -> bool {
    let mut any = false;
    for c in &ir.components {
        visit(&c.view.body, false, &mut |kind, bindings, _, _| {
            let pill = matches!(kind, IrElementKind::Link)
                && bindings.iter().any(|b| {
                    b.key == "appearance"
                        && matches!(&b.value, IrBindingValue::String(s) if s == "pill")
                });
            let center = matches!(kind, IrElementKind::Grid)
                && bindings.iter().any(|b| {
                    b.key == "align"
                        && matches!(&b.value, IrBindingValue::String(s) if s == "center")
                });
            let wide = bindings.iter().any(|b| b.key == "from")
                || bindings.iter().any(|b| b.key == "overlay");
            if crate::structure::is_card(bindings) || pill || center || wide {
                any = true;
            }
        });
    }
    any
}
