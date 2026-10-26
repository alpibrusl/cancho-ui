//! Pictures, video and grids (docs/structure.md section 10, step 2 of docs/www.md): `Image`, `Media` and `Grid`. This module owns their rule tags
//! and their DOM recipe (the elements, attributes and style a view becomes), which the JavaScript emitter reads today and the prerenderer will
//! read, so that the two cannot drift. Whether the files exist is not decided here (the IR has no file system): `assets_of` lists what a view
//! asks for, and the pipeline checks the files (`asset-missing`, `asset-too-large`, `asset-type-mismatch`).

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The image formats an `Image` may name, with the media type the page declares for each. SVG is not here: it can carry script.
pub const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("avif", "image/avif"),
    ("gif", "image/gif"),
];
/// What a `Link { to: "a file of the site" }` may name besides pictures and videos (docs/www-port/gaps.md section 6): text and data a visitor or an agent reads.
/// Not here: anything a browser runs or renders as a document of the site's own origin (`html`, `svg`, `xml`, `js`, `css`, `wasm`).
pub const LINK_FILE_TYPES: &[(&str, &str)] = &[
    ("md", "text/markdown"),
    ("txt", "text/plain"),
    ("json", "application/json"),
    ("cast", "application/x-asciicast"),
    ("csv", "text/csv"),
    ("pdf", "application/pdf"),
];
/// The video formats a `Media` may name.
pub const VIDEO_TYPES: &[(&str, &str)] = &[("mp4", "video/mp4"), ("webm", "video/webm")];
/// The `ratio:` of an `Image` or a `Media`: a closed set, `width:height`.
pub const RATIOS: &[&str] = &["1:1", "4:3", "3:2", "16:9", "21:9", "3:4", "2:3"];
/// The range of `width:` and `height:` of an `Image`, in pixels.
pub const SIZE: (i64, i64) = (1, 10_000);
/// The range of a `Grid`'s `min:`, in rem.
pub const GRID_MIN: (i64, i64) = (8, 60);
/// How many weights a `Grid` has, and the range of each.
pub const WEIGHTS_LEN: (usize, usize) = (2, 4);
pub const WEIGHT_RANGE: (f64, f64) = (1.0, 4.0);
/// How many sources a `Media` has.
pub const SOURCES_MAX: usize = 4;
/// The largest file of the site a program may name, in bytes (`asset-too-large`).
pub const ASSET_MAX_BYTES: u64 = 20 * 1024 * 1024;
/// The longest path of an asset.
const PATH_MAX: usize = 200;

pub fn is_media(kind: &IrElementKind) -> bool {
    matches!(
        kind,
        IrElementKind::Image | IrElementKind::Media | IrElementKind::Grid
    )
}

fn name(kind: &IrElementKind) -> &'static str {
    match kind {
        IrElementKind::Image => "Image",
        IrElementKind::Media => "Media",
        IrElementKind::Grid => "Grid",
        _ => "this element",
    }
}

/// The keys (beside `label:` on a `Media`) an element of this module takes.
fn keys(kind: &IrElementKind) -> &'static [&'static str] {
    match kind {
        IrElementKind::Image => &[
            "src",
            "fallback",
            "alt",
            "decorative",
            "width",
            "height",
            "ratio",
            "priority",
        ],
        IrElementKind::Media => &["kind", "src", "poster", "ratio", "caption", "transcript"],
        IrElementKind::Grid => &["min", "weights", "gap"],
        _ => &[],
    }
}

/// The keys that belong to this module's elements and to no other: on another element they are `binding-unknown` and say where they belong.
const OWN_KEYS: &[&str] = &[
    "src",
    "fallback",
    "alt",
    "decorative",
    "width",
    "height",
    "ratio",
    "priority",
    "poster",
    "caption",
    "transcript",
    "min",
    "weights",
];

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

fn bool_of(b: &IrBinding) -> Option<bool> {
    match &b.value {
        IrBindingValue::Const { expr, .. } => match **expr {
            IrExpr::BoolLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    }
}

fn find<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrBinding> {
    bindings.iter().find(|b| b.key == key)
}

/// A path of an asset: a file of this site, written relative to the program, and the media type its extension says. `Err` says why not.
pub fn asset_type(path: &str, types: &[(&str, &'static str)]) -> Result<&'static str, String> {
    let lower = path.to_ascii_lowercase();
    if path.is_empty() {
        return Err("the path is empty".to_string());
    }
    if lower.starts_with("http:") || lower.starts_with("https:") || path.starts_with("//") {
        return Err("a file is part of the site, not an address: the page makes no request to another host. Copy the file next to the program and name it by its path".to_string());
    }
    if path.len() > PATH_MAX {
        return Err(format!("the path is longer than {PATH_MAX} characters"));
    }
    if path.starts_with('/') {
        return Err(
            "the path starts with `/`; write it relative to the program, as `assets/field.webp`"
                .to_string(),
        );
    }
    if let Some(bad) = path
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')))
    {
        return Err(format!(
            "the path holds `{bad}`; a path is letters, digits, `.`, `_`, `-` and `/`"
        ));
    }
    if path
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err("the path has an empty segment, `.` or `..`: it stays inside the directory of the program".to_string());
    }
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    types
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            format!(
                "`.{ext}` is not a format of this element; it takes {}",
                types
                    .iter()
                    .map(|(e, _)| format!(".{e}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// Whether a link's `to:` names a file of the site: not an address, not a `#id`, not a scheme.
pub fn is_file_link(to: &str) -> bool {
    if to.is_empty() || to.starts_with('#') || to.starts_with("//") {
        return false;
    }
    match to.find(':') {
        Some(i) => {
            let head = &to[..i];
            !(head.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && head
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-')))
        }
        None => true,
    }
}

/// `Err` says why a link's file is not one a page may name; the media type otherwise.
pub fn link_file_type(path: &str) -> Result<&'static str, String> {
    let types: Vec<(&str, &'static str)> = LINK_FILE_TYPES
        .iter()
        .chain(IMAGE_TYPES.iter())
        .chain(VIDEO_TYPES.iter())
        .copied()
        .collect();
    asset_type(path, &types)
}

/// The width and height of a ratio `"16:9"`.
pub fn ratio_parts(r: &str) -> Option<(u32, u32)> {
    let (w, h) = r.split_once(':')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

/// The weights of a `Grid`, when the binding is a list of numbers.
pub fn weights_of(bindings: &[IrBinding]) -> Option<Vec<f64>> {
    let b = find(bindings, "weights")?;
    let IrBindingValue::Const { expr, .. } = &b.value else {
        return None;
    };
    let IrExpr::ListLit { items, .. } = &**expr else {
        return None;
    };
    items
        .iter()
        .map(|i| match i {
            IrExpr::IntLit { value, .. } => Some(*value as f64),
            IrExpr::FloatLit { value, .. } => Some(*value),
            _ => None,
        })
        .collect()
}

fn sources_of(b: &IrBinding) -> Option<Vec<String>> {
    match &b.value {
        IrBindingValue::String(s) => Some(vec![s.clone()]),
        IrBindingValue::Const { expr, .. } => match &**expr {
            IrExpr::ListLit { items, .. } => items
                .iter()
                .map(|i| match i {
                    IrExpr::StringLit { value, .. } => Some(value.clone()),
                    _ => None,
                })
                .collect(),
            _ => None,
        },
        _ => None,
    }
}

/// Checks one binding. `Ok(true)`: this module owns it and it is fine; `Ok(false)`: not its business (a `gap:` of a `Grid` is the layout's).
pub fn check_binding(kind: &IrElementKind, b: &IrBinding) -> Result<bool, LowerError> {
    if !is_media(kind) {
        if OWN_KEYS.contains(&b.key.as_str()) && !matches!(kind, IrElementKind::Named(_)) {
            return Err(refuse(
                "binding-unknown",
                format!(
                    "`{}:` is not a binding of {}; it belongs to {}",
                    b.key,
                    "this element",
                    match b.key.as_str() {
                        "fallback" | "alt" | "decorative" | "width" | "height" | "priority" => {
                            "Image"
                        }
                        "poster" | "caption" | "transcript" => "Media",
                        "min" | "weights" => "Grid",
                        _ => "Image and Media",
                    }
                ),
                b.span,
            ));
        }
        return Ok(false);
    }
    let own = keys(kind);
    if !own.contains(&b.key.as_str()) {
        return Err(refuse(
            "binding-unknown",
            format!(
                "`{}:` is not a binding of {}; it takes {}",
                b.key,
                name(kind),
                own.join(", ")
            ),
            b.span,
        ));
    }
    let image = matches!(kind, IrElementKind::Image);
    match b.key.as_str() {
        "gap" => Ok(false),
        "src" if image => match string_of(b) {
            Some(s) => asset_type(s, IMAGE_TYPES)
                .map(|_| true)
                .map_err(|why| refuse("image-src-invalid", format!("`src:` {why}"), b.span)),
            None => Err(refuse(
                "image-src-invalid",
                "`src:` of an Image is a text literal, the path of a file of the site".to_string(),
                b.span,
            )),
        },
        "fallback" => match string_of(b) {
            Some(s) => asset_type(s, IMAGE_TYPES)
                .map(|_| true)
                .map_err(|why| refuse("image-src-invalid", format!("`fallback:` {why}"), b.span)),
            None => Err(refuse(
                "image-src-invalid",
                "`fallback:` is a text literal, the path of a file of the site".to_string(),
                b.span,
            )),
        },
        "alt" => match string_of(b) {
            Some(s) if !s.trim().is_empty() => Ok(true),
            _ => Err(refuse(
                "image-alt-missing",
                "`alt:` says what the picture shows, as a text literal; for a picture that is only decoration write `decorative: true` and no `alt:`".to_string(),
                b.span,
            )),
        },
        "decorative" => match bool_of(b) {
            Some(true) => Ok(true),
            _ => Err(refuse(
                "image-alt-missing",
                "`decorative:` is `true` (a picture that adds nothing to the text) or absent; a picture that is not decoration needs `alt:`".to_string(),
                b.span,
            )),
        },
        "width" | "height" => match int_of(b) {
            Some(n) if (SIZE.0..=SIZE.1).contains(&n) => Ok(true),
            _ => Err(refuse(
                "image-size-invalid",
                format!(
                    "`{}:` is an integer literal from {} to {}, in pixels: the size of the file, which reserves the space before it loads",
                    b.key, SIZE.0, SIZE.1
                ),
                b.span,
            )),
        },
        "ratio" => match string_of(b) {
            Some(s) if RATIOS.contains(&s) => Ok(true),
            _ => Err(refuse(
                "ratio-invalid",
                format!("`ratio:` is one of {} as a text literal", RATIOS.join(", ")),
                b.span,
            )),
        },
        "priority" => match bool_of(b) {
            Some(_) => Ok(true),
            None => Err(refuse(
                "image-priority-invalid",
                "`priority:` is `true` or `false`: a picture the reader sees first, which loads at once".to_string(),
                b.span,
            )),
        },
        "kind" => match string_of(b) {
            Some("video") => Ok(true),
            _ => Err(refuse(
                "media-kind-invalid",
                "`kind:` is \"video\" (audio is not offered yet)".to_string(),
                b.span,
            )),
        },
        "src" => {
            let Some(list) = sources_of(b) else {
                return Err(refuse(
                    "media-src-invalid",
                    "`src:` of a Media is a text literal, or a list of text literals: the video files of the site, in the order the browser should try them".to_string(),
                    b.span,
                ));
            };
            if list.is_empty() || list.len() > SOURCES_MAX {
                return Err(refuse(
                    "media-src-invalid",
                    format!("`src:` has from 1 to {SOURCES_MAX} files"),
                    b.span,
                ));
            }
            for (i, s) in list.iter().enumerate() {
                asset_type(s, VIDEO_TYPES).map_err(|why| {
                    refuse("media-src-invalid", format!("`src:` {why}"), b.span)
                })?;
                if list[..i].contains(s) {
                    return Err(refuse(
                        "media-src-invalid",
                        format!("`src:` names \"{s}\" twice"),
                        b.span,
                    ));
                }
            }
            Ok(true)
        }
        "poster" => match string_of(b) {
            Some(s) => asset_type(s, IMAGE_TYPES)
                .map(|_| true)
                .map_err(|why| refuse("media-poster-invalid", format!("`poster:` {why}"), b.span)),
            None => Err(refuse(
                "media-poster-invalid",
                "`poster:` is a text literal, the path of an image of the site".to_string(),
                b.span,
            )),
        },
        "caption" | "transcript" => match string_of(b) {
            Some(s) if crate::structure::id_ok(s) => Ok(true),
            _ => Err(refuse(
                "media-reference-invalid",
                format!(
                    "`{}:` is the `id:` of an element of this program, as a text literal",
                    b.key
                ),
                b.span,
            )),
        },
        "min" => match int_of(b) {
            Some(n) if (GRID_MIN.0..=GRID_MIN.1).contains(&n) => Ok(true),
            _ => Err(refuse(
                "grid-min-invalid",
                format!(
                    "`min:` is an integer literal from {} to {}, in rem: the narrowest a column gets before the grid has one column fewer",
                    GRID_MIN.0, GRID_MIN.1
                ),
                b.span,
            )),
        },
        "weights" => {
            let ok = weights_of(std::slice::from_ref(b)).is_some_and(|w| {
                (WEIGHTS_LEN.0..=WEIGHTS_LEN.1).contains(&w.len())
                    && w.iter().all(|x| {
                        (WEIGHT_RANGE.0..=WEIGHT_RANGE.1).contains(x)
                            && ((x * 100.0).round() - x * 100.0).abs() < 1e-9
                    })
            });
            if ok {
                Ok(true)
            } else {
                Err(refuse(
                    "grid-weights-invalid",
                    format!(
                        "`weights:` is a list of {} to {} number literals from {} to {}, with at most two decimals: `weights: [1, 1.75]`",
                        WEIGHTS_LEN.0, WEIGHTS_LEN.1, WEIGHT_RANGE.0, WEIGHT_RANGE.1
                    ),
                    b.span,
                ))
            }
        }
        _ => Ok(true),
    }
}

/// What an element needs to be complete, and what it may not carry.
pub fn check_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
) -> Result<(), LowerError> {
    if !is_media(kind) {
        return Ok(());
    }
    let has = |k: &str| bindings.iter().any(|b| b.key == k);
    let media = matches!(kind, IrElementKind::Media);
    if !media && (accessibility.label.is_some() || accessibility.label_expr.is_some()) {
        return Err(refuse(
            "binding-unknown",
            format!(
                "`label:` is not a binding of {}: {}",
                name(kind),
                if matches!(kind, IrElementKind::Image) {
                    "the name of a picture is its `alt:`"
                } else {
                    "a Grid is a layout and has no name"
                }
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
    match kind {
        IrElementKind::Image => {
            if !children.is_empty() {
                return Err(refuse(
                    "image-child-invalid",
                    "an Image holds nothing: its caption is a `Caption` beside it in a `Figure`"
                        .to_string(),
                    span,
                ));
            }
            let Some(src) = find(bindings, "src").and_then(string_of) else {
                return Err(refuse(
                    "image-src-invalid",
                    "an Image needs `src:`, the path of a file of the site: `Image { src: \"assets/field.webp\" .. }`".to_string(),
                    span,
                ));
            };
            if find(bindings, "fallback").and_then(string_of) == Some(src) {
                return Err(refuse(
                    "image-src-invalid",
                    "`fallback:` is the file for a browser that cannot read `src:`: another file, in another format".to_string(),
                    span,
                ));
            }
            if let Some(fb) = find(bindings, "fallback").and_then(string_of)
                && asset_type(fb, IMAGE_TYPES).ok() == asset_type(src, IMAGE_TYPES).ok()
            {
                return Err(refuse(
                    "image-src-invalid",
                    "`fallback:` is in another format than `src:` (a `.webp` with a `.jpg`): the same format would be the same picture twice".to_string(),
                    span,
                ));
            }
            match (has("alt"), has("decorative")) {
                (true, true) => {
                    return Err(refuse(
                        "image-alt-conflict",
                        "an Image has `alt:` (it shows something) or `decorative: true` (it shows nothing the text does not), not both".to_string(),
                        span,
                    ));
                }
                (false, false) => {
                    return Err(refuse(
                        "image-alt-missing",
                        "an Image needs `alt:`, what the picture shows, or `decorative: true` when it adds nothing to the text".to_string(),
                        span,
                    ));
                }
                _ => {}
            }
            if !has("width") || !has("height") {
                return Err(refuse(
                    "image-size-missing",
                    "an Image needs `width:` and `height:`, the size of the file in pixels, so that the page reserves the space before the file arrives".to_string(),
                    span,
                ));
            }
        }
        IrElementKind::Media => {
            if !children.is_empty() {
                return Err(refuse(
                    "media-child-invalid",
                    "a Media holds nothing: its caption is a `Caption` beside it in a `Figure`, named by `caption:`".to_string(),
                    span,
                ));
            }
            if !has("kind") {
                return Err(refuse(
                    "media-kind-invalid",
                    "a Media needs `kind:`, \"video\"".to_string(),
                    span,
                ));
            }
            if !has("src") {
                return Err(refuse(
                    "media-src-invalid",
                    "a Media needs `src:`, the video file or files of the site".to_string(),
                    span,
                ));
            }
            if accessibility
                .label
                .as_deref()
                .is_none_or(|l| l.trim().is_empty())
            {
                return Err(refuse(
                    "media-label-missing",
                    "a Media needs a static `label:` that says what the video is: it is the name of the player and the text of the link a browser without video support shows".to_string(),
                    span,
                ));
            }
            if !has("caption") && !has("transcript") {
                return Err(refuse(
                    "media-text-alternative-missing",
                    "a video needs a text alternative: `caption:` names the `Caption` that describes it, `transcript:` the element that holds what it says (one of them, or both)".to_string(),
                    span,
                ));
            }
        }
        IrElementKind::Grid => {
            if !has("min") {
                return Err(refuse(
                    "grid-min-invalid",
                    "a Grid needs `min:`, the narrowest a column gets, in rem: `Grid { min: 18 .. }`".to_string(),
                    span,
                ));
            }
            if let Some(w) = weights_of(bindings) {
                let direct = children
                    .iter()
                    .all(|c| matches!(c, IrViewNode::Element { .. } | IrViewNode::Text { .. }));
                if !direct || children.len() != w.len() {
                    return Err(refuse(
                        "grid-weights-invalid",
                        format!(
                            "`weights:` has {} numbers, one for each column: the Grid needs exactly that many children, written out (no `For`, no `If`)",
                            w.len()
                        ),
                        span,
                    ));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ---- the DOM recipe ------------------------------------------------------------------------------------------------------------------

/// One element of a recipe: a tag, its attributes in a fixed order, and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Dom {
    pub tag: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub children: Vec<Dom>,
    pub text: Option<String>,
}

impl Dom {
    pub fn element(tag: &'static str) -> Dom {
        Dom {
            tag,
            attrs: Vec::new(),
            children: Vec::new(),
            text: None,
        }
    }
    pub fn attr(mut self, k: &'static str, v: impl Into<String>) -> Dom {
        self.attrs.push((k, v.into()));
        self
    }
}

/// The `<picture>` an `Image` becomes. Call it on an element that `check_complete` accepted.
pub fn image_dom(bindings: &[IrBinding]) -> Dom {
    let s = |k: &str| find(bindings, k).and_then(string_of).map(str::to_string);
    let n = |k: &str| find(bindings, k).and_then(int_of).unwrap_or(0);
    let src = s("src").unwrap_or_default();
    let fallback = s("fallback");
    let priority = find(bindings, "priority").and_then(bool_of) == Some(true);
    let mut pic = Dom::element("picture").attr("class", "cui-img");
    if let Some(r) = s("ratio") {
        pic = pic.attr("data-ratio", r);
    }
    pic = pic.attr("style", format!("max-width: {}px;", n("width")));
    let (img_src, source) = match &fallback {
        Some(fb) => (
            fb.clone(),
            Some(
                Dom::element("source")
                    .attr(
                        "type",
                        asset_type(&src, IMAGE_TYPES).unwrap_or("image/webp"),
                    )
                    .attr("srcset", src.clone()),
            ),
        ),
        None => (src.clone(), None),
    };
    if let Some(source) = source {
        pic.children.push(source);
    }
    let alt = if find(bindings, "decorative").and_then(bool_of) == Some(true) {
        String::new()
    } else {
        s("alt").unwrap_or_default()
    };
    let mut img = Dom::element("img")
        .attr("src", img_src)
        .attr("width", n("width").to_string())
        .attr("height", n("height").to_string())
        .attr("alt", alt)
        .attr("loading", if priority { "eager" } else { "lazy" })
        .attr("decoding", "async");
    if priority {
        img = img.attr("fetchpriority", "high");
    }
    pic.children.push(img);
    pic
}

/// The `<video>` a `Media` becomes.
pub fn media_dom(accessibility: &IrAccessibility, bindings: &[IrBinding]) -> Dom {
    let s = |k: &str| find(bindings, k).and_then(string_of).map(str::to_string);
    let label = accessibility.label.clone().unwrap_or_default();
    let sources = find(bindings, "src")
        .and_then(sources_of)
        .unwrap_or_default();
    let mut v = Dom::element("video").attr("class", "cui-media");
    if let Some(r) = s("ratio") {
        v = v.attr("data-ratio", r);
    }
    v = v
        .attr("aria-label", label.clone())
        .attr("controls", "")
        .attr("playsinline", "")
        .attr("preload", "metadata");
    if let Some(p) = s("poster") {
        v = v.attr("poster", p);
    }
    if let Some(c) = s("caption") {
        v = v.attr("aria-describedby", c);
    }
    if let Some(t) = s("transcript") {
        v = v.attr("aria-details", t);
    }
    for src in &sources {
        v.children.push(
            Dom::element("source")
                .attr("src", src.clone())
                .attr("type", asset_type(src, VIDEO_TYPES).unwrap_or("video/mp4")),
        );
    }
    let mut a = Dom::element("a").attr("href", sources.first().cloned().unwrap_or_default());
    a.text = Some(label);
    v.children.push(a);
    v
}

/// The class a `Grid` gets beside `cui-grid`: `cui-grid-w` when it has weights.
pub fn class(kind: &IrElementKind, bindings: &[IrBinding]) -> Option<&'static str> {
    (matches!(kind, IrElementKind::Grid) && weights_of(bindings).is_some()).then_some("cui-grid-w")
}

/// The `style` attribute of a `Grid`: the minimum, and the weights as custom properties the stylesheet reads.
pub fn grid_style(bindings: &[IrBinding]) -> Option<String> {
    let min = find(bindings, "min").and_then(int_of)?;
    let mut s = format!("--cui-min: {min}rem;");
    for (i, w) in weights_of(bindings).unwrap_or_default().iter().enumerate() {
        s.push_str(&format!(" --cui-w{}: {};", i + 1, w));
    }
    Some(s)
}

/// The bindings the recipe writes: the generic binding loop skips them.
pub fn handles(kind: &IrElementKind, key: &str) -> bool {
    is_media(kind) && keys(kind).contains(&key) && key != "gap"
}

/// A file a view asks for: where it is written, and whether the page plays it (a video) rather than shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetUse {
    pub path: String,
    pub span: Span,
    pub video: bool,
}

/// The files a view asks for (`Image` source and fallback, `Media` sources and poster), in the order they appear, with repeats.
pub fn assets_of(node: &IrViewNode) -> Vec<AssetUse> {
    fn go(node: &IrViewNode, out: &mut Vec<AssetUse>) {
        match node {
            IrViewNode::Element {
                kind,
                bindings,
                children,
                ..
            } => {
                let mut push = |b: &IrBinding, path: String, video: bool| {
                    out.push(AssetUse {
                        path,
                        span: b.span,
                        video,
                    })
                };
                match kind {
                    IrElementKind::Image => {
                        for k in ["src", "fallback"] {
                            if let Some(b) = find(bindings, k)
                                && let Some(s) = string_of(b)
                            {
                                push(b, s.to_string(), false);
                            }
                        }
                    }
                    IrElementKind::Media => {
                        if let Some(b) = find(bindings, "src")
                            && let Some(l) = sources_of(b)
                        {
                            for s in l {
                                push(b, s, true);
                            }
                        }
                        if let Some(b) = find(bindings, "poster")
                            && let Some(s) = string_of(b)
                        {
                            push(b, s.to_string(), false);
                        }
                    }
                    IrElementKind::Link => {
                        // a link to a file of the site: the build checks it exists, hashes it and copies it (docs/www-port/gaps.md section 6)
                        if let Some(b) = find(bindings, "to")
                            && let Some(s) = string_of(b)
                            && is_file_link(s)
                        {
                            let video = s.rsplit_once('.').is_some_and(|(_, e)| {
                                VIDEO_TYPES.iter().any(|(x, _)| x.eq_ignore_ascii_case(e))
                            });
                            push(b, s.to_string(), video);
                        }
                    }
                    _ => {}
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
    let mut out = Vec::new();
    go(node, &mut out);
    out
}

/// The `caption:` and `transcript:` references of a `Media`: `(is_caption, id, span)`.
pub fn references(kind: &IrElementKind, bindings: &[IrBinding]) -> Vec<(bool, String, Span)> {
    if !matches!(kind, IrElementKind::Media) {
        return Vec::new();
    }
    bindings
        .iter()
        .filter_map(|b| match (b.key.as_str(), string_of(b)) {
            ("caption", Some(s)) => Some((true, s.to_string(), b.span)),
            ("transcript", Some(s)) => Some((false, s.to_string(), b.span)),
            _ => None,
        })
        .collect()
}
