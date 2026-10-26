//! Sealed islands (docs/islands.md): a piece of the page that is not written in cancho-ui, declared with the hash of every file it runs, drawn in a
//! sandboxed frame and talking to the page only through typed props and events. This module owns the rule tags of the declaration and of the
//! instance, and the frame's recipe (the `<iframe>` and the document it holds), which the JavaScript emitter and the prerenderer both read. Whether the
//! files exist and match their hashes is not decided here (the IR has no file system): the pipeline reads them and calls `frame_document`.

use std::sync::Arc;

use cancho_ui_syntax::Span;
use cancho_ui_syntax::ast::{File, IslandEntry};

use crate::ir::*;
use crate::lower::LowerError;
use crate::media::{Dom, asset_type};

/// How many scripts, props and events an island has at most.
pub const SCRIPTS_MAX: usize = 8;
pub const PROPS_MAX: usize = 16;
pub const EVENTS_MAX: usize = 16;
/// The range of `width:` and `height:`, in pixels.
pub const SIZE: (i64, i64) = (1, 4000);
/// The largest file of an island, and the most all its files together may be, in bytes (`island-source-too-large`, `island-too-large`).
pub const FILE_MAX_BYTES: u64 = 512 * 1024;
pub const TOTAL_MAX_BYTES: u64 = 1024 * 1024;
/// The most the runtime forwards the pointer to a frame, per second.
pub const POINTER_MAX_PER_SECOND: u32 = 20;
/// The media types of the files an island may name.
const SCRIPT_TYPES: &[(&str, &str)] = &[("js", "text/javascript")];
const STYLE_TYPES: &[(&str, &str)] = &[("css", "text/css")];

/// The policy of the frame's own document. The scripts are named by hash: the ones the compiler wrote, and no other, run; there is no `eval`, no
/// network, no navigation and no frame. (The page's own `frame-src about:` closes what this cannot: docs/islands.md section 1.)
pub const FRAME_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; form-action 'none'; base-uri 'none'; frame-src 'none'";

/// What the compiler writes before the island's scripts: the four names of the contract (`cancho.props`, `cancho.onprops`, `cancho.emit`,
/// `cancho.ready`) and the removal of the WebRTC constructors, which no policy governs (docs/islands.md section 1.2).
pub const SHIM: &str = "(function () {\n'use strict';\nvar dead = ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel'];\nfor (var i = 0; i < dead.length; i++) { try { Object.defineProperty(window, dead[i], { value: undefined, writable: false, configurable: false }); } catch (e) {} }\nvar fns = [];\nvar got = false;\nvar c = {\n  props: {},\n  onprops: function (f) { if (typeof f !== 'function') { return; } fns.push(f); if (got) { try { f(c.props); } catch (e) {} } },\n  emit: function (name, value) { parent.postMessage({ cui: 'emit', name: String(name), value: value }, '*'); },\n  ready: function () { parent.postMessage({ cui: 'ready' }, '*'); }\n};\naddEventListener('message', function (e) {\n  var d = e.data;\n  if (e.source !== parent || !d || d.cui !== 'props' || !d.props || typeof d.props !== 'object') { return; }\n  got = true;\n  c.props = d.props;\n  fns.slice().forEach(function (f) { try { f(d.props); } catch (x) {} });\n});\nvar lastp = 0, pend = null, timer = 0;\nfunction sendp() { timer = 0; lastp = Date.now(); if (pend) { parent.postMessage({ cui: 'pointer', x: pend.x, y: pend.y }, '*'); pend = null; } }\naddEventListener('pointermove', function (e) {\n  pend = { x: e.clientX, y: e.clientY };\n  var wait = 50 - (Date.now() - lastp);\n  if (wait <= 0) { sendp(); } else if (!timer) { timer = setTimeout(sendp, wait); }\n}, { passive: true });\nObject.defineProperty(window, 'cancho', { value: c });\n})();";

/// The line the compiler writes after the island's last script: the page answers with the whole current props.
pub const READY: &str = "cancho.ready();";

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

fn is_pascal_case(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric())
}

fn is_snake_case(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The elements and keywords a name may not take: an island is written like an element, so it may not be one.
const BUILT_IN: &[&str] = &[
    "Column",
    "Row",
    "Table",
    "Text",
    "Button",
    "TextField",
    "TextArea",
    "Checkbox",
    "Select",
    "FileField",
    "Chart",
    "NavLink",
    "SortList",
    "Map",
    "Heading",
    "Section",
    "Header",
    "Nav",
    "Main",
    "Footer",
    "Aside",
    "Article",
    "Link",
    "Details",
    "Figure",
    "Caption",
    "Rich",
    "Strong",
    "Em",
    "Small",
    "Reveal",
    "CountUp",
    "Waffle",
    "Chip",
    "Break",
    "Summary",
    "Code",
    "Image",
    "Media",
    "Grid",
    "For",
    "If",
];

fn is_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn type_of(name: &str, pointer_ok: bool) -> Option<IrIslandType> {
    Some(match name {
        "Bool" => IrIslandType::Bool,
        "Int" => IrIslandType::Int,
        "Float" => IrIslandType::Float,
        "String" => IrIslandType::String,
        "Pointer" if pointer_ok => IrIslandType::Pointer,
        _ => return None,
    })
}

fn file_of(
    path: &str,
    sha256: &str,
    types: &[(&str, &'static str)],
    what: &str,
    span: Span,
) -> Result<IrIslandFile, LowerError> {
    if let Err(why) = asset_type(path, types) {
        return Err(refuse(
            "island-path-invalid",
            format!("the {what} `{path}`: {why}"),
            span,
        ));
    }
    if !is_hash(sha256) {
        return Err(refuse(
            "island-hash-invalid",
            format!(
                "the sha256 of `{path}` is not 64 lowercase hexadecimal digits; it is the hash the file is pinned to (`sha256sum {path}`)"
            ),
            span,
        ));
    }
    Ok(IrIslandFile {
        path: path.to_string(),
        sha256: sha256.to_string(),
        bytes: 0,
    })
}

/// The islands a file declares, each checked as text. The files themselves are the pipeline's.
pub fn lower_decls(file: &File) -> Result<Vec<IrIsland>, LowerError> {
    let mut out: Vec<IrIsland> = Vec::new();
    for d in &file.islands {
        if !is_pascal_case(&d.name) {
            return Err(refuse(
                "island-name-invalid",
                format!(
                    "`{}`: an island is named like a component, in PascalCase",
                    d.name
                ),
                d.span,
            ));
        }
        if BUILT_IN.contains(&d.name.as_str()) || file.components.iter().any(|c| c.name == d.name) {
            return Err(refuse(
                "island-name-conflict",
                format!(
                    "`{}` is already the name of an element or a component; an island is written like one, so it needs a name of its own",
                    d.name
                ),
                d.span,
            ));
        }
        if out.iter().any(|o| o.name == d.name) {
            return Err(refuse(
                "island-duplicate",
                format!("island `{}` is declared twice", d.name),
                d.span,
            ));
        }
        let mut island = IrIsland {
            name: d.name.clone(),
            sealed: true,
            label: None,
            decorative: false,
            width: 0,
            height: 0,
            scripts: Vec::new(),
            style: None,
            props: Vec::new(),
            events: Vec::new(),
            srcdoc: String::new(),
            texts: Vec::new(),
            style_text: None,
            span: d.span,
        };
        let mut sealed_seen = false;
        let (mut decorative_seen, mut label_seen, mut props_seen, mut events_seen) =
            (false, false, false, false);
        let (mut width_seen, mut height_seen) = (false, false);
        let mut paths: Vec<&str> = Vec::new();
        for e in &d.entries {
            let twice = |what: &str, span: Span| {
                refuse(
                    "island-entry-duplicate",
                    format!("island `{}`: `{what}` is written twice", d.name),
                    span,
                )
            };
            match e {
                IslandEntry::Script { path, sha256, span } => {
                    if paths.contains(&path.as_str()) {
                        return Err(refuse(
                            "island-file-duplicate",
                            format!("island `{}` names `{path}` twice", d.name),
                            *span,
                        ));
                    }
                    paths.push(path);
                    island
                        .scripts
                        .push(file_of(path, sha256, SCRIPT_TYPES, "script", *span)?);
                }
                IslandEntry::Style { path, sha256, span } => {
                    if island.style.is_some() {
                        return Err(twice("style", *span));
                    }
                    if paths.contains(&path.as_str()) {
                        return Err(refuse(
                            "island-file-duplicate",
                            format!("island `{}` names `{path}` twice", d.name),
                            *span,
                        ));
                    }
                    paths.push(path);
                    island.style = Some(file_of(path, sha256, STYLE_TYPES, "style", *span)?);
                }
                IslandEntry::Label { text, span } => {
                    if label_seen {
                        return Err(twice("label", *span));
                    }
                    label_seen = true;
                    if text.trim().is_empty() {
                        return Err(refuse(
                            "island-label-missing",
                            format!(
                                "island `{}`: the label is empty; say in words what the island shows, or write `decorative: true;`",
                                d.name
                            ),
                            *span,
                        ));
                    }
                    island.label = Some(text.clone());
                }
                IslandEntry::Sealed { value, span } => {
                    if sealed_seen {
                        return Err(twice("sealed", *span));
                    }
                    sealed_seen = true;
                    island.sealed = *value;
                }
                IslandEntry::Decorative { value, span } => {
                    if decorative_seen {
                        return Err(twice("decorative", *span));
                    }
                    decorative_seen = true;
                    island.decorative = *value;
                }
                IslandEntry::Size { key, value, span } => {
                    let seen = if key == "width" {
                        &mut width_seen
                    } else {
                        &mut height_seen
                    };
                    if *seen {
                        return Err(twice(key, *span));
                    }
                    *seen = true;
                    if !(SIZE.0..=SIZE.1).contains(value) {
                        return Err(refuse(
                            "island-size-invalid",
                            format!(
                                "island `{}`: `{key}: {value}` is outside {}..{} pixels",
                                d.name, SIZE.0, SIZE.1
                            ),
                            *span,
                        ));
                    }
                    if key == "width" {
                        island.width = *value as u32;
                    } else {
                        island.height = *value as u32;
                    }
                }
                IslandEntry::Props { items, span } => {
                    if props_seen {
                        return Err(twice("props", *span));
                    }
                    props_seen = true;
                    if items.len() > PROPS_MAX {
                        return Err(refuse(
                            "island-too-many",
                            format!(
                                "island `{}` has {} props; the most is {PROPS_MAX}",
                                d.name,
                                items.len()
                            ),
                            *span,
                        ));
                    }
                    for (n, t, s) in items {
                        if !is_snake_case(n) {
                            return Err(refuse(
                                "island-prop-name",
                                format!(
                                    "island `{}`: a prop `{n}` is named in lower case with underscores, like a state field",
                                    d.name
                                ),
                                *s,
                            ));
                        }
                        if island.props.iter().any(|p| &p.name == n) {
                            return Err(refuse(
                                "island-prop-duplicate",
                                format!("island `{}` declares the prop `{n}` twice", d.name),
                                *s,
                            ));
                        }
                        let Some(ty) = type_of(t, true) else {
                            return Err(refuse(
                                "island-prop-type",
                                format!(
                                    "island `{}`: the prop `{n}` has the type `{t}`; a prop is a `Bool`, `Int`, `Float`, `String` or `Pointer`",
                                    d.name
                                ),
                                *s,
                            ));
                        };
                        if ty == IrIslandType::Pointer
                            && island.props.iter().any(|p| p.ty == IrIslandType::Pointer)
                        {
                            return Err(refuse(
                                "island-pointer-twice",
                                format!(
                                    "island `{}` declares two `Pointer` props; the runtime feeds one",
                                    d.name
                                ),
                                *s,
                            ));
                        }
                        island.props.push(IrIslandProp {
                            name: n.clone(),
                            ty,
                        });
                    }
                }
                IslandEntry::Events { items, span } => {
                    if events_seen {
                        return Err(twice("events", *span));
                    }
                    events_seen = true;
                    if items.len() > EVENTS_MAX {
                        return Err(refuse(
                            "island-too-many",
                            format!(
                                "island `{}` has {} events; the most is {EVENTS_MAX}",
                                d.name,
                                items.len()
                            ),
                            *span,
                        ));
                    }
                    for (n, t, s) in items {
                        if !is_pascal_case(n) {
                            return Err(refuse(
                                "island-event-name",
                                format!(
                                    "island `{}`: an event `{n}` is named in PascalCase, like the handler that hears it",
                                    d.name
                                ),
                                *s,
                            ));
                        }
                        if island.events.iter().any(|e| &e.name == n) {
                            return Err(refuse(
                                "island-event-duplicate",
                                format!("island `{}` declares the event `{n}` twice", d.name),
                                *s,
                            ));
                        }
                        let ty = match t {
                            None => None,
                            Some(t) => match type_of(t, false) {
                                Some(ty) => Some(ty),
                                None => {
                                    return Err(refuse(
                                        "island-event-type",
                                        format!(
                                            "island `{}`: the event `{n}` carries `{t}`; an event carries a `Bool`, `Int`, `Float` or `String`, or nothing",
                                            d.name
                                        ),
                                        *s,
                                    ));
                                }
                            },
                        };
                        island.events.push(IrIslandEvent {
                            name: n.clone(),
                            ty,
                        });
                    }
                }
            }
        }
        if island.scripts.is_empty() {
            return Err(refuse(
                "island-script-missing",
                format!(
                    "island `{}` runs no script; it names at least one `script: \"..\" sha256: \"..\";`",
                    d.name
                ),
                d.span,
            ));
        }
        if island.scripts.len() > SCRIPTS_MAX {
            return Err(refuse(
                "island-too-many",
                format!(
                    "island `{}` runs {} scripts; the most is {SCRIPTS_MAX}",
                    d.name,
                    island.scripts.len()
                ),
                d.span,
            ));
        }
        if island.width == 0 || island.height == 0 {
            return Err(refuse(
                "island-size-missing",
                format!(
                    "island `{}` needs `width:` and `height:` in pixels: they give the frame its shape before it loads",
                    d.name
                ),
                d.span,
            ));
        }
        match (&island.label, island.decorative) {
            (None, false) => {
                return Err(refuse(
                    "island-label-missing",
                    format!(
                        "island `{}` has no `label:` and is not `decorative: true;`; a frame is read by a screen reader as its label",
                        d.name
                    ),
                    d.span,
                ));
            }
            (Some(_), true) => {
                return Err(refuse(
                    "island-label-conflict",
                    format!(
                        "island `{}` has a `label:` and is `decorative: true;`: a decorative frame is hidden from a screen reader, so its label would be read by no one",
                        d.name
                    ),
                    d.span,
                ));
            }
            _ => {}
        }
        if !island.sealed {
            if island.props.iter().any(|p| p.ty == IrIslandType::Pointer) {
                return Err(refuse(
                    "island-open-pointer",
                    format!(
                        "open island `{}` declares a `Pointer` prop: an open island runs in the page and listens to the pointer itself; the runtime feeds only a sealed frame",
                        d.name
                    ),
                    d.span,
                ));
            }
            if island.props.iter().any(|p| p.name == "text") {
                return Err(refuse(
                    "island-prop-name",
                    format!(
                        "open island `{}` declares a prop `text`: `text:` on an open island names the element that is its text alternative",
                        d.name
                    ),
                    d.span,
                ));
            }
        }
        out.push(island);
    }
    Ok(out)
}

/// Replaces each instance of a declared island in the views by the island itself, and checks the instance against the declaration: the props it
/// binds, the events it hears, nothing else.
pub fn resolve_instances(ir: &mut IrFile) -> Result<(), LowerError> {
    if ir.islands.is_empty() {
        return Ok(());
    }
    let islands = ir.islands.clone();
    for c in &mut ir.components {
        let mut ctx = Ctx {
            handlers: c
                .events
                .iter()
                .map(|e| (e.name.clone(), e.params.len()))
                .collect(),
            state: c
                .state
                .iter()
                .map(|f| (f.name.clone(), f.ty.clone()))
                .collect(),
            has_dom: c.requires.iter().any(|r| r.name == "Dom"),
            ids: Vec::new(),
            dynamic: Vec::new(),
        };
        collect_ids(&c.view.body, &mut ctx.ids);
        resolve_node(&islands, &mut ctx, &mut c.view.body)?;
    }
    Ok(())
}

/// What an instance is checked against besides its island: the component it is in.
struct Ctx {
    handlers: Vec<(String, usize)>,
    state: Vec<(String, IrType)>,
    has_dom: bool,
    /// the `id:` of every element of the view (the text alternative of an open island names one)
    ids: Vec<String>,
    /// the open islands already instantiated with a prop read from state
    dynamic: Vec<String>,
}

fn collect_ids(node: &IrViewNode, out: &mut Vec<String>) {
    match node {
        IrViewNode::Element {
            bindings, children, ..
        } => {
            for b in bindings {
                if b.key == "id"
                    && let IrBindingValue::String(s) = &b.value
                {
                    out.push(s.clone());
                }
            }
            for c in children {
                collect_ids(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                collect_ids(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

fn resolve_node(
    islands: &[IrIsland],
    ctx: &mut Ctx,
    node: &mut IrViewNode,
) -> Result<(), LowerError> {
    match node {
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span,
        } => {
            if let IrElementKind::Named(name) = kind
                && let Some(island) = islands.iter().find(|i| &i.name == name)
            {
                check_instance(island, ctx, accessibility, bindings, children, *span)?;
                *kind = IrElementKind::Island(Arc::new(island.clone()));
            }
            for ch in children {
                resolve_node(islands, ctx, ch)?;
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for ch in children {
                resolve_node(islands, ctx, ch)?;
            }
        }
        IrViewNode::Text { .. } => {}
    }
    Ok(())
}

/// A prop given as a literal: the JSON of it and the type it has.
fn literal_of(b: &IrBinding) -> Option<(serde_json::Value, IrIslandType)> {
    match &b.value {
        IrBindingValue::Const { expr, .. } => match &**expr {
            IrExpr::IntLit { value, .. } => Some((serde_json::json!(value), IrIslandType::Int)),
            IrExpr::FloatLit { value, .. } => Some((serde_json::json!(value), IrIslandType::Float)),
            IrExpr::BoolLit { value, .. } => Some((serde_json::json!(value), IrIslandType::Bool)),
            IrExpr::StringLit { value, .. } => {
                Some((serde_json::json!(value), IrIslandType::String))
            }
            _ => None,
        },
        _ => None,
    }
}

fn check_instance(
    island: &IrIsland,
    ctx: &mut Ctx,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
) -> Result<(), LowerError> {
    let name = &island.name;
    if !children.is_empty() {
        return Err(refuse(
            "island-instance-children",
            format!("`{name}` takes no children: an island draws itself, from its own files"),
            span,
        ));
    }
    if accessibility.role.is_some()
        || accessibility.label.is_some()
        || accessibility.label_expr.is_some()
    {
        return Err(refuse(
            "island-instance-label",
            format!(
                "`{name}` takes no `role:` or `label:`: its label is written once, where the island is declared"
            ),
            span,
        ));
    }
    if !island.sealed && !ctx.has_dom {
        return Err(refuse(
            "island-open-needs-dom",
            format!(
                "`{name}` is an open island: it runs plain JavaScript in the page, with the page's authority. The component that uses it declares `requires Dom;`"
            ),
            span,
        ));
    }
    let mut bound: Vec<&str> = Vec::new();
    let mut heard: Vec<&str> = Vec::new();
    let mut described = false;
    let mut dynamic_here = false;
    for b in bindings {
        if !island.sealed && b.key == "text" {
            let id = match &b.value {
                IrBindingValue::Const { expr, .. } => match &**expr {
                    IrExpr::StringLit { value, .. } => Some(value.as_str()),
                    _ => None,
                },
                _ => None,
            };
            if !id.is_some_and(|i| ctx.ids.iter().any(|x| x == i)) {
                return Err(refuse(
                    "island-text-alternative-invalid",
                    format!(
                        "`text:` on `{name}` names the `id:` of an element of this component that holds the island's text alternative; no element has that id"
                    ),
                    b.span,
                ));
            }
            described = true;
            continue;
        }
        if b.key == "on" {
            let ev = match &b.value {
                IrBindingValue::EventRef(n) => n,
                IrBindingValue::EventCall { name: n, .. } => {
                    return Err(refuse(
                        "island-event-args",
                        format!(
                            "`on: {n}(..)` on `{name}`: the island's event arrives with its own value as the handler's argument; write `on: {n}`"
                        ),
                        b.span,
                    ));
                }
                _ => {
                    return Err(refuse(
                        "island-event-undeclared",
                        format!("`on:` on `{name}` names an event of the island"),
                        b.span,
                    ));
                }
            };
            let Some(declared) = island.events.iter().find(|e| &e.name == ev) else {
                let known: Vec<&str> = island.events.iter().map(|e| e.name.as_str()).collect();
                return Err(refuse(
                    "island-event-undeclared",
                    format!(
                        "`on: {ev}`: island `{name}` declares no event `{ev}` (it declares {})",
                        if known.is_empty() {
                            "none".to_string()
                        } else {
                            known.join(", ")
                        }
                    ),
                    b.span,
                ));
            };
            let Some((_, params)) = ctx.handlers.iter().find(|(h, _)| h == ev) else {
                return Err(refuse(
                    "island-event-handler-missing",
                    format!("`on: {ev}` on `{name}`: this component has no `on {ev}` handler"),
                    b.span,
                ));
            };
            let want = usize::from(declared.ty.is_some());
            if *params != want {
                return Err(refuse(
                    "island-event-arity",
                    format!(
                        "`on {ev}` takes {params} parameter(s); the island's event `{ev}` carries {}",
                        if want == 1 { "one value" } else { "nothing" }
                    ),
                    b.span,
                ));
            }
            if heard.contains(&ev.as_str()) {
                return Err(refuse(
                    "island-event-bound-twice",
                    format!("`on: {ev}` is written twice on one `{name}`"),
                    b.span,
                ));
            }
            heard.push(ev);
            continue;
        }
        let Some(prop) = island.props.iter().find(|p| p.name == b.key) else {
            let known: Vec<&str> = island.props.iter().map(|p| p.name.as_str()).collect();
            return Err(refuse(
                "island-prop-unknown",
                format!(
                    "`{}:` is not a prop of island `{name}` (it declares {})",
                    b.key,
                    if known.is_empty() {
                        "none".to_string()
                    } else {
                        known.join(", ")
                    }
                ),
                b.span,
            ));
        };
        if prop.ty == IrIslandType::Pointer {
            return Err(refuse(
                "island-prop-pointer-bound",
                format!(
                    "`{}:` is the pointer prop of `{name}`: the runtime feeds it (at most {POINTER_MAX_PER_SECOND} a second); a program does not bind it",
                    b.key
                ),
                b.span,
            ));
        }
        if bound.contains(&b.key.as_str()) {
            return Err(refuse(
                "island-prop-bound-twice",
                format!("`{}:` is written twice on one `{name}`", b.key),
                b.span,
            ));
        }
        bound.push(&b.key);
        match literal_of(b) {
            Some((_, ty)) if ty == prop.ty => {}
            Some((_, ty)) => {
                return Err(refuse(
                    "island-prop-type",
                    format!(
                        "`{}:` is a {} of island `{name}`; it was given a {}",
                        b.key,
                        prop.ty.name(),
                        ty.name()
                    ),
                    b.span,
                ));
            }
            None => {
                if island.sealed {
                    return Err(refuse(
                        "island-prop-dynamic",
                        format!(
                            "`{}:` on `{name}` is read from state; a prop of a sealed island is a literal, written where the island is used",
                            b.key
                        ),
                        b.span,
                    ));
                }
                // an open island's prop may be a state field of the component: the page's script gives it the value at every render
                let field = match &b.value {
                    IrBindingValue::Const { expr, .. } => match &**expr {
                        IrExpr::Ident { name: f, .. } => ctx.state.iter().find(|(n, _)| n == f),
                        _ => None,
                    },
                    _ => None,
                };
                let Some((field_name, field_ty)) = field else {
                    return Err(refuse(
                        "island-prop-source-unknown",
                        format!(
                            "`{}:` on `{name}` is not a literal and not a `state` field of this component",
                            b.key
                        ),
                        b.span,
                    ));
                };
                let ty = match field_ty {
                    IrType::Int => Some(IrIslandType::Int),
                    IrType::Float => Some(IrIslandType::Float),
                    IrType::Bool => Some(IrIslandType::Bool),
                    IrType::String => Some(IrIslandType::String),
                    _ => None,
                };
                if ty != Some(prop.ty) {
                    return Err(refuse(
                        "island-prop-type",
                        format!(
                            "`{}:` is a {} of island `{name}`; the state field `{field_name}` is not",
                            b.key,
                            prop.ty.name()
                        ),
                        b.span,
                    ));
                }
                if !dynamic_here && ctx.dynamic.contains(name) {
                    return Err(refuse(
                        "island-open-instance-twice",
                        format!(
                            "`{name}` is used twice in one component with props read from state: the page's script gives a state value to an island by its name, so it can tell only one from another when there is one"
                        ),
                        span,
                    ));
                }
                dynamic_here = true;
            }
        }
    }
    if dynamic_here {
        ctx.dynamic.push(name.clone());
    }
    if !island.sealed && !island.decorative && !described {
        return Err(refuse(
            "island-text-alternative-missing",
            format!(
                "`{name}` is an open island that is not decorative: its drawing is not readable by a screen reader, so name the element that says the same in text, as `text: \"list-id\"`"
            ),
            span,
        ));
    }
    for p in &island.props {
        if p.ty != IrIslandType::Pointer && !bound.contains(&p.name.as_str()) {
            return Err(refuse(
                "island-prop-missing",
                format!("island `{name}` needs `{}: <{}>`", p.name, p.ty.name()),
                span,
            ));
        }
    }
    Ok(())
}

/// The `data-cui-island` attribute: what the runtime needs to feed the frame and to hear it, nothing else.
pub fn spec(island: &IrIsland, bindings: &[IrBinding]) -> String {
    let mut props = serde_json::Map::new();
    for b in bindings {
        if let Some((v, _)) = literal_of(b) {
            props.insert(b.key.clone(), v);
        }
    }
    let mut dynamic = Vec::new();
    for b in bindings {
        if b.key != "on" && b.key != "text" && literal_of(b).is_none() {
            dynamic.push(b.key.clone());
        }
    }
    let mut events = serde_json::Map::new();
    for b in bindings {
        if b.key == "on"
            && let IrBindingValue::EventRef(n) = &b.value
            && let Some(e) = island.events.iter().find(|e| &e.name == n)
        {
            events.insert(
                n.clone(),
                serde_json::json!({ "type": e.ty.map(|t| t.name()) }),
            );
        }
    }
    let mut v = serde_json::Map::new();
    v.insert("name".into(), serde_json::json!(island.name));
    v.insert("props".into(), serde_json::Value::Object(props));
    if let Some(p) = island.props.iter().find(|p| p.ty == IrIslandType::Pointer) {
        v.insert("pointer".into(), serde_json::json!(p.name));
    }
    v.insert("events".into(), serde_json::Value::Object(events));
    if !island.sealed {
        v.insert("open".into(), serde_json::json!(true));
        v.insert("dynamic".into(), serde_json::json!(dynamic));
    }
    serde_json::Value::Object(v).to_string()
}

/// The `<iframe>` an instance becomes. Call it on an element that `resolve_instances` accepted.
pub fn island_dom(island: &IrIsland, bindings: &[IrBinding]) -> Dom {
    if !island.sealed {
        // an open island is drawn into a `div` of the page by its own scripts; there is no document of its own
        let mut d = Dom::element("div");
        if island.decorative {
            d = d.attr("aria-hidden", "true");
        } else {
            d = d
                .attr("role", "group")
                .attr("aria-label", island.label.clone().unwrap_or_default());
            if let Some(b) = bindings.iter().find(|b| b.key == "text")
                && let IrBindingValue::Const { expr, .. } = &b.value
                && let IrExpr::StringLit { value, .. } = &**expr
            {
                d = d.attr("aria-describedby", value.clone());
            }
        }
        return d
            .attr(
                "style",
                format!(
                    "display:block;width:100%;max-width:{w}px;aspect-ratio:{w} / {h}",
                    w = island.width,
                    h = island.height
                ),
            )
            .attr("data-cui-island", spec(island, bindings));
    }
    let mut d = Dom::element("iframe").attr("sandbox", "allow-scripts");
    d = d.attr("title", island.label.clone().unwrap_or_default());
    if island.decorative {
        d = d.attr("aria-hidden", "true").attr("tabindex", "-1");
    }
    d.attr("width", island.width.to_string())
        .attr("height", island.height.to_string())
        .attr(
            "style",
            format!(
                "border:0;display:block;width:100%;max-width:{w}px;height:auto;aspect-ratio:{w} / {h}",
                w = island.width,
                h = island.height
            ),
        )
        .attr("referrerpolicy", "no-referrer")
        .attr("data-cui-island", spec(island, bindings))
        .attr("srcdoc", island.srcdoc.clone())
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// A CSP hash source from the 64 hex digits of a SHA-256: `'sha256-<base64>'`.
fn csp_hash(hex: &str) -> String {
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok())
        .collect();
    format!("'sha256-{}'", base64(&bytes))
}

/// What the frame's document needs of a script: its text and the SHA-256 of that text, as hex.
pub struct Piece<'a> {
    pub text: &'a str,
    pub sha256: &'a str,
}

/// The document of the frame: the policy, the style, the shim, the island's scripts in order, and the line that asks the page for the props. The
/// scripts are the files' bytes, unchanged.
pub fn frame_document(
    scripts: &[Piece],
    style: Option<&str>,
    shim_sha256: &str,
    ready_sha256: &str,
) -> String {
    let mut policy = String::from("default-src 'none'; script-src ");
    policy.push_str(&csp_hash(shim_sha256));
    for s in scripts {
        policy.push(' ');
        policy.push_str(&csp_hash(s.sha256));
    }
    policy.push(' ');
    policy.push_str(&csp_hash(ready_sha256));
    policy.push_str(&FRAME_POLICY["default-src 'none'".len()..]);
    let mut d = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">"
    );
    if let Some(css) = style {
        d.push_str("<style>");
        d.push_str(css);
        d.push_str("</style>");
    }
    d.push_str("</head><body><script>");
    d.push_str(SHIM);
    d.push_str("</script>");
    for s in scripts {
        d.push_str("<script>");
        d.push_str(s.text);
        d.push_str("</script>");
    }
    d.push_str("<script>");
    d.push_str(READY);
    d.push_str("</script></body></html>");
    d
}

/// Why a file's text cannot be put in the frame's document as it is, if it cannot: the byte for byte copy would end its own element, or hide a
/// comment or a NUL in the page.
pub fn unsafe_in_html(text: &str, style: bool) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if text.contains('\0') {
        return Some("a NUL character");
    }
    if style {
        if lower.contains("</style") {
            return Some("`</style`");
        }
    } else if lower.contains("</script") || lower.contains("<!--") {
        return Some("`</script` or `<!--`");
    }
    None
}

/// Every setting of an island that `diff` compares, as `(setting, value)`: the island itself, each file with its hash, the order the scripts run in,
/// the label, the size, the props and the events with their types.
pub fn flatten(i: &IrIsland) -> Vec<(String, String)> {
    let n = &i.name;
    let mut out = vec![(
        n.clone(),
        if i.sealed { "sealed" } else { "open" }.to_string(),
    )];
    for s in &i.scripts {
        out.push((format!("{n}.script {}", s.path), s.sha256.clone()));
    }
    out.push((
        format!("{n}.script-order"),
        i.scripts
            .iter()
            .map(|s| s.path.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    ));
    if let Some(s) = &i.style {
        out.push((format!("{n}.style {}", s.path), s.sha256.clone()));
    }
    if let Some(l) = &i.label {
        out.push((format!("{n}.label"), l.clone()));
    }
    out.push((format!("{n}.decorative"), i.decorative.to_string()));
    out.push((format!("{n}.size"), format!("{}x{}", i.width, i.height)));
    for p in &i.props {
        out.push((format!("{n}.prop {}", p.name), p.ty.name().to_string()));
    }
    for e in &i.events {
        out.push((
            format!("{n}.event {}", e.name),
            e.ty.map_or("nothing", |t| t.name()).to_string(),
        ));
    }
    out
}

/// Puts the islands the pipeline completed (sizes, documents) into the file and into every view that uses them.
pub fn attach(ir: &mut IrFile, done: Vec<IrIsland>) {
    fn walk(node: &mut IrViewNode, done: &[IrIsland]) {
        match node {
            IrViewNode::Element { kind, children, .. } => {
                if let IrElementKind::Island(i) = kind
                    && let Some(d) = done.iter().find(|d| d.name == i.name)
                {
                    *kind = IrElementKind::Island(Arc::new(d.clone()));
                }
                for c in children {
                    walk(c, done);
                }
            }
            IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                for c in children {
                    walk(c, done);
                }
            }
            IrViewNode::Text { .. } => {}
        }
    }
    for c in &mut ir.components {
        walk(&mut c.view.body, &done);
    }
    ir.islands = done;
}

/// The `Content-Security-Policy` of a generated page, when the program needs one (`None`: no policy): `img-src` for a map (docs/maps.md) and
/// `frame-src about:` for a sealed island (docs/islands.md section 1). The second rules out every other frame on the page: a video player or a
/// payment form embedded by address would be refused by the browser, not loaded.
pub fn page_policy(ir: &IrFile) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(img) = crate::map::csp_img_sources(ir) {
        parts.push(format!("img-src {img}"));
    }
    let mut used: Vec<&IrIsland> = Vec::new();
    for c in &ir.components {
        collect(&c.view.body, &mut used);
    }
    // an open island runs in the page and has no frame: only a sealed one needs the page to allow `about:` frames
    if used.iter().any(|i| i.sealed) {
        parts.push("frame-src about:".to_string());
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

fn collect<'a>(node: &'a IrViewNode, out: &mut Vec<&'a IrIsland>) {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            if let IrElementKind::Island(i) = kind {
                out.push(i);
            }
            for c in children {
                collect(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                collect(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// What a sealed island is, as `inspect effects` and `report` say it: what it runs, what it reaches, and what the sandbox does not bound.
pub fn describe(i: &IrIsland) -> serde_json::Value {
    if !i.sealed {
        return serde_json::json!({
            "name": i.name,
            "sealed": false,
            "authority": OPEN_AUTHORITY,
            "label": i.label,
            "decorative": i.decorative,
            "files": i.scripts.iter().chain(i.style.iter()).map(|f| serde_json::json!({ "path": f.path, "sha256": f.sha256, "bytes": f.bytes })).collect::<Vec<_>>(),
            "reaches": "everything the page can reach: its DOM, its storage, its cookies, its network, its origin",
            "props": i.props.iter().map(|p| format!("{}: {}", p.name, p.ty.name())).collect::<Vec<_>>(),
            "events": i.events.iter().map(|e| match e.ty { Some(t) => format!("{}: {}", e.name, t.name()), None => e.name.clone() }).collect::<Vec<_>>(),
            "not_bounded": ["everything: an open island has no sandbox and no policy of its own; what the sealed battery shows it cannot do, this one can (docs/islands.md section 12)"],
        });
    }
    serde_json::json!({
        "name": i.name,
        "sealed": true,
        "label": i.label,
        "decorative": i.decorative,
        "files": i.scripts.iter().chain(i.style.iter()).map(|f| serde_json::json!({ "path": f.path, "sha256": f.sha256, "bytes": f.bytes })).collect::<Vec<_>>(),
        "reaches": "nothing but this page, by messages (props in, events out)",
        "props": i.props.iter().map(|p| format!("{}: {}", p.name, p.ty.name())).collect::<Vec<_>>(),
        "events": i.events.iter().map(|e| match e.ty { Some(t) => format!("{}: {}", e.name, t.name()), None => e.name.clone() }).collect::<Vec<_>>(),
        "not_bounded": NOT_BOUNDED,
    })
}

/// What `inspect effects` prints as the authority of an open island.
pub const OPEN_AUTHORITY: &str =
    "UNBOUNDED (runs plain JavaScript in the page's origin; no sandbox)";

/// What the sandbox of a sealed island does not guarantee (docs/islands.md section 9).
pub const NOT_BOUNDED: &[&str] = &[
    "time: a sealed island that loops stalls the page it is in",
    "memory",
    "WebRTC beyond the removal of its constructors in the frame (measured: 0 packets with the removal, 3 without; no other way was found, and none is proved absent)",
    "timing and cache side channels, and DNS prefetch",
    "anything a browser adds after the attempts that were measured",
];

/// Does the view instantiate the island of this name?
pub fn view_uses(node: &IrViewNode, name: &str) -> bool {
    let mut used: Vec<&IrIsland> = Vec::new();
    collect(node, &mut used);
    used.iter().any(|i| i.name == name)
}

/// The names of the islands the view instantiates, once each, in order.
pub fn used_in(node: &IrViewNode) -> Vec<String> {
    let mut used: Vec<&IrIsland> = Vec::new();
    collect(node, &mut used);
    let mut names: Vec<String> = Vec::new();
    for i in used {
        if !names.contains(&i.name) {
            names.push(i.name.clone());
        }
    }
    names
}

/// Does the view instantiate an open island? That is the use of `Dom` a component declares.
pub fn view_has_open(node: &IrViewNode) -> bool {
    let mut used: Vec<&IrIsland> = Vec::new();
    collect(node, &mut used);
    used.iter().any(|i| !i.sealed)
}
