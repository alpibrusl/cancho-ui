//! The DOM of a static element, and the prerenderer (docs/prerender.md). `shell` is what the JavaScript emitter writes for every static element
//! of a view, so the HTML this module writes for the same element cannot differ from the DOM the script builds: there is one function, and both
//! call it. The elements with a recipe of their own (`structure::*`, `media::*`) are called from here and not copied.
//!
//! A node is **static** when it and all it contains are literals; a **hole** is any other node and its whole subtree. A hole draws nothing
//! without JavaScript. A text container (a `Heading`, a `Rich`, a `Link`, a `Caption`, a `Text`, a `Strong` ..) is atomic: a sentence is
//! written whole or not at all, so a hole inside it makes the container a hole.

use crate::ir::*;
use crate::island;
use crate::media::{self, Dom};
use crate::structure;

/// The element a static node becomes, before its children: the tag, the attributes in a fixed order (`class` and `style` as they are
/// written), and the line of a `Details`' summary.
#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    pub tag: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub summary: Option<String>,
}

fn has_shell(kind: &IrElementKind) -> bool {
    matches!(
        kind,
        IrElementKind::Column
            | IrElementKind::Row
            | IrElementKind::Grid
            | IrElementKind::Text
            | IrElementKind::Heading
            | IrElementKind::Section
            | IrElementKind::Header
            | IrElementKind::Nav
            | IrElementKind::Main
            | IrElementKind::Footer
            | IrElementKind::Aside
            | IrElementKind::Article
            | IrElementKind::Link
            | IrElementKind::Details
            | IrElementKind::Summary
            | IrElementKind::Figure
            | IrElementKind::Caption
            | IrElementKind::Rich
            | IrElementKind::Strong
            | IrElementKind::Em
            | IrElementKind::Small
            | IrElementKind::Reveal
            | IrElementKind::CountUp
            | IrElementKind::Chip
            | IrElementKind::List
            | IrElementKind::Item
            | IrElementKind::Break
            | IrElementKind::InlineCode
            | IrElementKind::Table
            | IrElementKind::TableRow
            | IrElementKind::Cell
            | IrElementKind::HeaderCell
    )
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

/// Whether a binding is one the recipe writes: a literal, under a key of this element.
fn binding_ok(kind: &IrElementKind, b: &IrBinding) -> bool {
    let literal = matches!(
        b.value,
        IrBindingValue::String(_) | IrBindingValue::Const { .. }
    );
    match b.key.as_str() {
        "gap" | "max_width" => {
            let on = if b.key == "gap" {
                matches!(
                    kind,
                    IrElementKind::Column | IrElementKind::Row | IrElementKind::Grid
                )
            } else {
                matches!(kind, IrElementKind::Column | IrElementKind::Row)
            };
            on && int_of(b).is_some()
        }
        // being seen is the page's business, not script that draws the element: the element stays static
        k @ ("visible" | "hidden") => {
            matches!(b.value, IrBindingValue::EventRef(_)) && structure::handles(kind, k)
        }
        k => literal && (structure::handles(kind, k) || media::handles(kind, k)),
    }
}

/// The shell of a static element, or `None` when the element is not static: it is not one the recipe covers (a `Button`, a `TextField`, a
/// `Table`, a component instance ..), or it has a binding that reads state, handles an event or names a role by binding.
pub fn shell(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
) -> Option<Shell> {
    if !has_shell(kind)
        || accessibility.label_expr.is_some()
        // a `Text` with another role is a status or an alert: a live region the script keeps the node of from one render to the next
        || (matches!(kind, IrElementKind::Text)
            && accessibility
                .role
                .as_ref()
                .is_some_and(|r| !matches!(r, IrRole::Heading)))
        || !bindings.iter().all(|b| binding_ok(kind, b))
    {
        return None;
    }
    let mut attrs: Vec<(&'static str, String)> =
        structure::attrs(kind, bindings).into_iter().collect();
    let mut class: Vec<String> = Vec::new();
    if let Some(c) = kind.layout_class() {
        class.push(c.to_string());
    }
    if let Some(c) = structure::class(kind, bindings) {
        class.push(c);
    }
    if let Some(c) = media::class(kind, bindings) {
        class.push(c.to_string());
    }
    if let Some(n) = bindings.iter().find(|b| b.key == "gap").and_then(int_of) {
        class.push(format!("cui-gap-{n}"));
    }
    if !class.is_empty() {
        attrs.push(("class", class.join(" ")));
    }
    if matches!(kind, IrElementKind::Grid) {
        if let Some(style) = media::grid_style(bindings) {
            attrs.push(("style", style));
        }
    } else if let Some(n) = bindings
        .iter()
        .find(|b| b.key == "max_width")
        .and_then(int_of)
    {
        attrs.push(("style", format!("max-width: {n}ch; margin-inline: auto;")));
    }
    if let Some(role) = &accessibility.role {
        attrs.push((
            "role",
            match role {
                IrRole::Text => "text",
                IrRole::Heading => "heading",
                IrRole::Button => "button",
                IrRole::Other(s) => s.as_str(),
            }
            .to_string(),
        ));
    }
    if let Some(label) = &accessibility.label {
        attrs.push(("aria-label", label.clone()));
    }
    // a header cell says it heads a column (docs/presentation.md section 4)
    if matches!(kind, IrElementKind::HeaderCell) {
        attrs.push(("scope", "col".to_string()));
    }
    Some(Shell {
        tag: structure::tag(kind, bindings),
        attrs,
        summary: if matches!(kind, IrElementKind::Details) {
            structure::summary(bindings)
        } else {
            None
        },
    })
}

/// The box a `Table` scrolls in when it is wider than a narrow phone (docs/presentation.md section 4a): a keyboard-focusable region named like the table.
/// `None` when the name is an expression (the box is then written by script, with the name it has at that render).
pub fn table_scroll(accessibility: &IrAccessibility) -> Option<Dom> {
    if accessibility.label_expr.is_some() {
        return None;
    }
    let mut d = Dom::element("div")
        .attr("class", "cui-table-scroll")
        .attr("tabindex", "0");
    if let Some(l) = &accessibility.label {
        d = d.attr("role", "region").attr("aria-label", l.clone());
    }
    Some(d)
}

/// The tag that stands, in a recipe, for the place of a `Code`'s copy button.
pub const COPY_BUTTON_HOLE: &str = "!copy-button";

/// The box of a `Code`: `div.cui-code > pre > code`, empty. The text goes into the `code`, and the copy button, which is script, after the `pre`.
pub fn code_skeleton(bindings: &[IrBinding]) -> Dom {
    let (copy, max_height) = structure::code_spec(bindings);
    let mut code_box = Dom::element("div").attr("class", "cui-code");
    if copy {
        code_box = code_box.attr("data-copy", "");
    }
    let mut pre = Dom::element("pre").attr("tabindex", "0");
    if let Some(h) = max_height {
        pre = pre.attr("style", format!("max-height: {h}rem;"));
    }
    pre.children.push(Dom::element("code"));
    code_box.children.push(pre);
    if copy {
        // the button is script: a place for it, which the prerenderer turns into a hole and the script's own box skips
        code_box.children.push(Dom::element(COPY_BUTTON_HOLE));
    }
    code_box
}

// ---- the prerenderer ----------------------------------------------------------------------------------------------------------------

/// A part of a view that the page draws only with JavaScript, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    pub what: String,
    pub reason: String,
    /// its number in the page, in document order: the markers `<!--cui:N-->` and `<!--/cui:N-->` of a skeleton
    pub id: usize,
    /// where it is in the view: the index among the children of each element from the body down
    pub path: Vec<usize>,
    /// the hole is the copy button of the `Code` at `path`, drawn inside the box after its `pre`; every other hole is the node at `path`
    pub copy_button: bool,
}

/// The static HTML of a view body, and what was left out of it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prerender {
    pub html: String,
    /// elements written (tags, counting those inside an image, a video and a code box)
    pub static_elements: usize,
    /// how many of them carry an `id`
    pub ids: usize,
    /// whether anything a visitor reads or sees was written (text, a picture, a video, code): layout shells alone are a blank page
    pub content: bool,
    pub holes: Vec<Hole>,
    /// for a skeleton (below): a hash of its HTML, which the page carries and the script built for it checks
    pub hash: Option<String>,
}

fn escape(s: &str, attr: bool, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            // the parser turns a carriage return into a line feed; a reference is kept as it is
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

fn representable(s: &str) -> bool {
    // a NUL cannot be written in HTML: the parser makes it U+FFFD
    !s.contains('\0')
}

fn is_void(tag: &str) -> bool {
    matches!(tag, "br" | "img" | "source")
}

struct Writer {
    out: Prerender,
    /// write `<!--cui:N--><!--/cui:N-->` where a hole is (a skeleton), or nothing (P1a)
    markers: bool,
    path: Vec<usize>,
}

impl Writer {
    fn open(&mut self, tag: &str, attrs: &[(&'static str, String)]) {
        self.out.html.push('<');
        self.out.html.push_str(tag);
        for (k, v) in attrs {
            self.out.html.push(' ');
            self.out.html.push_str(k);
            self.out.html.push_str("=\"");
            escape(v, true, &mut self.out.html);
            self.out.html.push('"');
            if *k == "id" {
                self.out.ids += 1;
            }
        }
        self.out.html.push('>');
        self.out.static_elements += 1;
    }

    fn text(&mut self, t: &str) {
        if !t.trim().is_empty() {
            self.out.content = true;
        }
        escape(t, false, &mut self.out.html);
    }

    fn close(&mut self, tag: &str) {
        if !is_void(tag) {
            self.out.html.push_str("</");
            self.out.html.push_str(tag);
            self.out.html.push('>');
        }
    }

    fn dom(&mut self, d: &Dom) {
        if d.tag == COPY_BUTTON_HOLE {
            self.hole_of(
                "Code",
                "its copy button: a button is drawn by script, and a visitor without script is not shown one that does nothing",
                true,
            );
            return;
        }
        self.open(d.tag, &d.attrs);
        if let Some(t) = &d.text {
            self.text(t);
        }
        for c in &d.children {
            self.dom(c);
        }
        self.close(d.tag);
    }

    fn hole(&mut self, what: &str, reason: &str) {
        self.hole_of(what, reason, false);
    }

    fn hole_of(&mut self, what: &str, reason: &str, copy_button: bool) {
        let id = self.out.holes.len();
        self.out.holes.push(Hole {
            what: what.to_string(),
            reason: reason.to_string(),
            id,
            path: self.path.clone(),
            copy_button,
        });
        if self.markers {
            self.out
                .html
                .push_str(&format!("<!--cui:{id}--><!--/cui:{id}-->"));
        }
    }
}

fn kind_name(kind: &IrElementKind) -> String {
    match kind {
        IrElementKind::Named(n) => format!("component {n}"),
        IrElementKind::InlineCode => "Code".to_string(),
        k => format!("{k:?}"),
    }
}

fn literal_parts(parts: &[IrTextPart]) -> Option<String> {
    let mut s = String::new();
    for p in parts {
        match p {
            IrTextPart::Literal(t) => s.push_str(t),
            IrTextPart::Expr(_) => return None,
        }
    }
    Some(s)
}

fn code_text(children: &[IrViewNode]) -> Option<String> {
    let mut s = String::new();
    for c in children {
        match c {
            IrViewNode::Text { parts, .. } => s.push_str(&literal_parts(parts)?),
            _ => return None,
        }
    }
    Some(s)
}

/// The first hole inside a node, or `None` when the node is wholly static.
fn first_hole(node: &IrViewNode) -> Option<(String, String)> {
    match node {
        IrViewNode::Text { parts, .. } => match literal_parts(parts) {
            Some(t) if representable(&t) => None,
            Some(_) => Some((
                "Text".to_string(),
                "text with a NUL character cannot be written in HTML".to_string(),
            )),
            None => Some((
                "Text".to_string(),
                "a text that reads state or data".to_string(),
            )),
        },
        IrViewNode::For { .. } => Some((
            "For".to_string(),
            "its rows are made at run time".to_string(),
        )),
        IrViewNode::If { .. } => Some((
            "If".to_string(),
            "it is shown or hidden at run time".to_string(),
        )),
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            ..
        } => match kind {
            IrElementKind::Named(n) => Some((
                format!("component {n}"),
                "an instance of a component: its own script draws it".to_string(),
            )),
            IrElementKind::Image | IrElementKind::Media => {
                let dom = if matches!(kind, IrElementKind::Image) {
                    media::image_dom(bindings)
                } else {
                    media::media_dom(accessibility, bindings)
                };
                (!dom_representable(&dom)).then(|| {
                    (
                        kind_name(kind),
                        "text with a NUL character cannot be written in HTML".to_string(),
                    )
                })
            }
            IrElementKind::Island(i) => {
                // a frame is a static element: its document is written by the compiler and the script only posts props to it
                (!dom_representable(&island::island_dom(i, bindings))).then(|| {
                    (
                        kind_name(kind),
                        "text with a NUL character cannot be written in HTML".to_string(),
                    )
                })
            }
            // a table is static when its box is, its rows are rows and its cells are literals (or what a `For` over data wrote out): any hole in it
            // makes the whole table the script's, as a sentence is
            IrElementKind::Table => {
                if accessibility.label_expr.is_some() {
                    return Some((
                        "Table".to_string(),
                        "its name is read from state".to_string(),
                    ));
                }
                if accessibility
                    .label
                    .as_deref()
                    .is_some_and(|l| !representable(l))
                {
                    return Some((
                        "Table".to_string(),
                        "text with a NUL character cannot be written in HTML".to_string(),
                    ));
                }
                children.iter().find_map(first_hole)
            }
            IrElementKind::Code => match code_text(children) {
                Some(t) if representable(&t) => None,
                Some(_) => Some((
                    "Code".to_string(),
                    "text with a NUL character cannot be written in HTML".to_string(),
                )),
                None => Some((
                    "Code".to_string(),
                    "a code block that reads state".to_string(),
                )),
            },
            k => match shell(k, accessibility, bindings) {
                None => Some((kind_name(k), not_static_reason(k, bindings))),
                Some(s) => {
                    if !s.attrs.iter().all(|(_, v)| representable(v))
                        || s.summary.as_deref().is_some_and(|t| !representable(t))
                    {
                        return Some((
                            kind_name(k),
                            "text with a NUL character cannot be written in HTML".to_string(),
                        ));
                    }
                    children.iter().find_map(first_hole)
                }
            },
        },
    }
}

fn dom_representable(d: &Dom) -> bool {
    d.attrs.iter().all(|(_, v)| representable(v))
        && d.text.as_deref().is_none_or(representable)
        && d.children.iter().all(dom_representable)
}

fn not_static_reason(kind: &IrElementKind, bindings: &[IrBinding]) -> String {
    if has_shell(kind) {
        "a binding that reads state or handles an event".to_string()
    } else if bindings.is_empty() {
        "the page draws it with script: it is a control or has behaviour".to_string()
    } else {
        "the page draws it with script: it is a control, or reads state".to_string()
    }
}

fn walk(w: &mut Writer, node: &IrViewNode) {
    match node {
        IrViewNode::Text { .. } => match first_hole(node) {
            Some((what, why)) => w.hole(&what, &why),
            None => {
                if let IrViewNode::Text { parts, .. } = node
                    && let Some(t) = literal_parts(parts)
                {
                    w.open("span", &[]);
                    w.text(&t);
                    w.close("span");
                }
            }
        },
        IrViewNode::For { .. } | IrViewNode::If { .. } => {
            if let Some((what, why)) = first_hole(node) {
                w.hole(&what, &why);
            }
        }
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            ..
        } => {
            match kind {
                IrElementKind::Image | IrElementKind::Media => {
                    if let Some((what, why)) = first_hole(node) {
                        w.hole(&what, &why);
                    } else if matches!(kind, IrElementKind::Image) {
                        w.out.content = true;
                        w.dom(&media::image_dom(bindings));
                    } else {
                        w.out.content = true;
                        w.dom(&media::media_dom(accessibility, bindings));
                    }
                    return;
                }
                IrElementKind::Waffle => {
                    w.out.content = true;
                    w.dom(&crate::waffle::dom(accessibility, bindings));
                    return;
                }
                IrElementKind::Island(i) => {
                    if let Some((what, why)) = first_hole(node) {
                        w.hole(&what, &why);
                    } else {
                        w.out.content = true;
                        w.dom(&island::island_dom(i, bindings));
                    }
                    return;
                }
                IrElementKind::Table => {
                    if let Some((what, why)) = first_hole(node) {
                        w.hole(&what, &why);
                    } else if let Some(scroll) = table_scroll(accessibility)
                        && let Some(s) = shell(kind, accessibility, bindings)
                    {
                        // div.cui-table-scroll > table.cui-table > tbody > tr ..: what the script builds, from the same functions
                        w.open(scroll.tag, &scroll.attrs);
                        w.open(s.tag, &s.attrs);
                        w.open("tbody", &[]);
                        for (i, c) in children.iter().enumerate() {
                            w.path.push(i);
                            walk(w, c);
                            w.path.pop();
                        }
                        w.close("tbody");
                        w.close(s.tag);
                        w.close(scroll.tag);
                    }
                    return;
                }
                IrElementKind::Code => {
                    if let Some((what, why)) = first_hole(node) {
                        w.hole(&what, &why);
                    } else {
                        let mut d = code_skeleton(bindings);
                        d.children[0].children[0].text = code_text(children);
                        w.dom(&d);
                    }
                    return;
                }
                _ => {}
            }
            let Some(s) = shell(kind, accessibility, bindings) else {
                if let Some((what, why)) = first_hole(node) {
                    w.hole(&what, &why);
                }
                return;
            };
            // a sentence is whole or not at all
            if kind.holds_text()
                && let Some((inner, why)) = first_hole(node)
            {
                w.hole(
                    &kind_name(kind),
                    &format!("it holds {inner} ({why}): text is drawn whole or not at all"),
                );
                return;
            }
            if !s.attrs.iter().all(|(_, v)| representable(v))
                || s.summary.as_deref().is_some_and(|t| !representable(t))
            {
                w.hole(
                    &kind_name(kind),
                    "text with a NUL character cannot be written in HTML",
                );
                return;
            }
            w.open(s.tag, &s.attrs);
            if let Some(t) = &s.summary {
                w.open("summary", &[]);
                w.text(t);
                w.close("summary");
            }
            let text_path = kind.holds_text()
                && (structure::is_inline_container(kind)
                    || children
                        .iter()
                        .all(|c| matches!(c, IrViewNode::Text { .. })));
            for (i, c) in children.iter().enumerate() {
                match c {
                    IrViewNode::Text { parts, .. } if text_path => {
                        if let Some(t) = literal_parts(parts) {
                            w.text(&t);
                        }
                    }
                    other => {
                        w.path.push(i);
                        walk(w, other);
                        w.path.pop();
                    }
                }
            }
            w.close(s.tag);
        }
    }
}

/// The static HTML of a view body.
pub fn prerender(body: &IrViewNode) -> Prerender {
    let mut w = Writer {
        out: Prerender::default(),
        markers: false,
        path: Vec::new(),
    };
    walk(&mut w, body);
    w.out
}

/// FNV-1a 64 of some text, as 16 hex digits: a label that two builds of one page share, not a security measure.
fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The static HTML of a view body with a marker pair where each hole goes, and a hash of it. A page that carries it is **hydrated**, not
/// rebuilt: the script builds only the holes, between their markers, and never touches a node of the skeleton (docs/prerender.md).
pub fn skeleton(body: &IrViewNode) -> Prerender {
    let mut w = Writer {
        out: Prerender::default(),
        markers: true,
        path: Vec::new(),
    };
    walk(&mut w, body);
    let mut out = w.out;
    out.hash = Some(fnv(&out.html));
    out
}

/// The component whose page is hydrated, and its skeleton: the last component of the program (the one mounted into `#root`) when it has static
/// content and no other component draws it as an instance. `None` means the script builds the whole page, as it does for every other build.
pub fn hydrating(ir: &IrFile) -> Option<(usize, Prerender)> {
    fn instantiates(node: &IrViewNode, name: &str) -> bool {
        match node {
            IrViewNode::Element { kind, children, .. } => {
                matches!(kind, IrElementKind::Named(n) if n == name)
                    || children.iter().any(|c| instantiates(c, name))
            }
            IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                children.iter().any(|c| instantiates(c, name))
            }
            IrViewNode::Text { .. } => false,
        }
    }
    let idx = ir.components.len().checked_sub(1)?;
    let last = &ir.components[idx];
    if ir
        .components
        .iter()
        .any(|c| instantiates(&c.view.body, &last.name))
    {
        return None;
    }
    let sk = skeleton(&last.view.body);
    sk.content.then_some((idx, sk))
}

/// The node at `path` (the index among the children of each element, from the body down), if there is one.
pub fn node_at<'a>(body: &'a IrViewNode, path: &[usize]) -> Option<&'a IrViewNode> {
    let mut node = body;
    for i in path {
        node = match node {
            IrViewNode::Element { children, .. }
            | IrViewNode::For { children, .. }
            | IrViewNode::If { children, .. } => children.get(*i)?,
            IrViewNode::Text { .. } => return None,
        };
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancho_ui_syntax::Span;

    fn text(s: &str) -> IrViewNode {
        IrViewNode::Text {
            parts: vec![IrTextPart::Literal(s.to_string())],
            span: Span::DUMMY,
        }
    }

    fn element(kind: IrElementKind, children: Vec<IrViewNode>) -> IrViewNode {
        IrViewNode::Element {
            kind,
            accessibility: IrAccessibility {
                role: None,
                label: None,
                label_expr: None,
            },
            bindings: Vec::new(),
            children,
            span: Span::DUMMY,
        }
    }

    #[test]
    fn text_is_escaped_and_a_carriage_return_is_a_reference() {
        let p = prerender(&element(
            IrElementKind::Column,
            vec![text("a<b>&\"c\"\r\nd")],
        ));
        assert_eq!(
            p.html,
            "<div class=\"cui-col\"><span>a&lt;b&gt;&amp;\"c\"&#13;\nd</span></div>"
        );
        assert!(p.holes.is_empty());
    }

    #[test]
    fn a_nul_cannot_be_written_so_the_text_is_a_hole_and_nothing_else_is_lost() {
        let p = prerender(&element(
            IrElementKind::Column,
            vec![text("fine"), text("a\0b"), text("after")],
        ));
        assert_eq!(
            p.html,
            "<div class=\"cui-col\"><span>fine</span><span>after</span></div>"
        );
        assert_eq!(p.holes.len(), 1);
        assert!(p.holes[0].reason.contains("NUL"));
        // inside a sentence the whole sentence goes
        let p = prerender(&element(IrElementKind::Rich, vec![text("a\0b")]));
        assert_eq!(p.html, "");
        assert_eq!(p.holes.len(), 1);
    }

    #[test]
    fn a_hole_inside_a_sentence_is_the_whole_sentence() {
        let expr = IrViewNode::Text {
            parts: vec![IrTextPart::Expr(IrExpr::Ident {
                name: "n".to_string(),
                span: Span::DUMMY,
            })],
            span: Span::DUMMY,
        };
        let sentence = element(
            IrElementKind::Rich,
            vec![text("Count: "), element(IrElementKind::Strong, vec![expr])],
        );
        let p = prerender(&element(
            IrElementKind::Column,
            vec![sentence, text("kept")],
        ));
        assert_eq!(p.html, "<div class=\"cui-col\"><span>kept</span></div>");
        assert!(
            p.holes[0].reason.contains("whole or not at all"),
            "{:?}",
            p.holes
        );
    }
}
