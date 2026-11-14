//! `Icon` (docs/structure.md 12.0q): a line icon drawn as an inline `<svg>` from a closed, vendored set (Lucide, ISC; see `vendor/lucide/` and THIRD_PARTY.md).
//!
//! * The name is a string literal of the set (`icon-unknown` otherwise, with the nearest names). The set is the files in `vendor/lucide/`, each written
//!   verbatim and read here: nothing is fetched, nothing is a picture (`<img>`, `data:`, a sprite), so a page's Content-Security-Policy is unchanged.
//! * Every stroke is `currentColor` at width 2 with round caps and joins, so the icon takes the text colour of whatever holds it: pressed and disabled
//!   states, dark mode and the page's 4.5:1 checks apply to it without a rule of their own.
//! * An icon is `aria-hidden` when it stands in a `Button` or `NavLink` that has a `label:` (the label is the name) or when it says `decorative: true`
//!   (words beside it carry the meaning); anywhere else it needs `label:` and is `role="img"` (`icon-label-missing`).
//! * A `Button` also takes `icon: "name"`: the icon is drawn before the label's text, for a key that has words.
//! * Everything is a literal, so the element is static: it is in the prerendered HTML.

use cancho_ui_syntax::Span;

use crate::icon_set::SET;
use crate::ir::*;
use crate::lower::LowerError;
use crate::media::Dom;

/// The sizes of an icon: 1 rem, 1.25 rem (the default) and 1.75 rem.
pub const SIZES: &[&str] = &["small", "medium", "large"];
/// The keys of an `Icon` beside `label:`.
pub const KEYS: &[&str] = &["name", "size", "decorative"];

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

fn find<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrBinding> {
    bindings.iter().find(|b| b.key == key)
}

/// The names of the set, sorted.
pub fn names() -> Vec<&'static str> {
    let mut v: Vec<&str> = SET.iter().map(|(n, _)| *n).collect();
    v.sort_unstable();
    v
}

/// Whether the set has this icon.
pub fn known(name: &str) -> bool {
    SET.iter().any(|(n, _)| *n == name)
}

/// The names nearest to a mistyped one (a shared word or a shared prefix), at most five.
fn nearest(name: &str) -> Vec<&'static str> {
    let words: Vec<&str> = name.split('-').filter(|w| w.len() > 1).collect();
    let mut v: Vec<&str> = names()
        .into_iter()
        .filter(|n| {
            n.starts_with(name)
                || name.starts_with(n)
                || words.iter().any(|w| n.split('-').any(|x| x == *w))
        })
        .collect();
    v.truncate(5);
    v
}

fn static_attr(a: &str) -> Option<&'static str> {
    Some(match a {
        "d" => "d",
        "cx" => "cx",
        "cy" => "cy",
        "r" => "r",
        "x" => "x",
        "y" => "y",
        "x1" => "x1",
        "y1" => "y1",
        "x2" => "x2",
        "y2" => "y2",
        "rx" => "rx",
        "ry" => "ry",
        "width" => "width",
        "height" => "height",
        "points" => "points",
        "fill" => "fill",
        _ => return None,
    })
}

fn static_tag(t: &str) -> Option<&'static str> {
    Some(match t {
        "path" => "path",
        "circle" => "circle",
        "rect" => "rect",
        "line" => "line",
        "polyline" => "polyline",
        _ => return None,
    })
}

/// The shapes of one icon, read from its vendored file: only `path`, `circle`, `rect`, `line`, `polyline`, with the attributes of those. `Err` names what is
/// not plain (a test runs it over the whole set, and the vendoring script refuses the same).
pub fn shapes(name: &str) -> Result<Vec<Dom>, String> {
    let (_, text) = SET
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("no icon {name}"))?;
    let open = text.find('>').ok_or("no svg tag")?;
    let end = text.rfind("</svg>").ok_or("no end tag")?;
    let mut out = Vec::new();
    let mut rest = &text[open + 1..end];
    while let Some(i) = rest.find('<') {
        let j = rest[i..].find("/>").ok_or("a shape without />")? + i;
        let inner = rest[i + 1..j].trim();
        let (tag, mut attrs) = inner.split_once(char::is_whitespace).ok_or("a bare tag")?;
        let tag = static_tag(tag).ok_or_else(|| format!("element <{tag}> is not allowed"))?;
        let mut dom = Dom::element(tag);
        while let Some(eq) = attrs.find('=') {
            let key = attrs[..eq].trim();
            let after = &attrs[eq + 1..];
            let q = after.strip_prefix('"').ok_or("an unquoted attribute")?;
            let close = q.find('"').ok_or("an unclosed attribute")?;
            let value = &q[..close];
            let key = static_attr(key).ok_or_else(|| format!("attribute {key} is not allowed"))?;
            if key == "fill" && value != "currentColor" && value != "none" {
                return Err(format!("fill={value} is not allowed"));
            }
            if value.contains("url(") || value.contains("javascript") {
                return Err("an external reference".to_string());
            }
            dom = dom.attr(key, value);
            attrs = &q[close + 1..];
        }
        out.push(dom);
        rest = &rest[j + 2..];
    }
    if out.is_empty() {
        return Err("no shapes".to_string());
    }
    Ok(out)
}

/// Checks one binding. `Ok(true)`: this module owns it and it is fine; `Ok(false)`: not its business.
pub fn check_binding(kind: &IrElementKind, b: &IrBinding) -> Result<bool, LowerError> {
    if matches!(kind, IrElementKind::Button) && b.key == "icon" {
        return match string_of(b) {
            Some(n) if known(n) => Ok(true),
            _ => Err(unknown(string_of(b).unwrap_or(""), b.span)),
        };
    }
    if !matches!(kind, IrElementKind::Icon) {
        if b.key == "icon" {
            return Err(refuse(
                "binding-unknown",
                "`icon:` is a binding of Button (an icon before its label); to draw an icon anywhere else write `Icon { name: \"..\" }`".to_string(),
                b.span,
            ));
        }
        return Ok(false);
    }
    match b.key.as_str() {
        "name" => match string_of(b) {
            Some(n) if known(n) => Ok(true),
            _ => Err(unknown(string_of(b).unwrap_or(""), b.span)),
        },
        "size" => match string_of(b) {
            Some(s) if SIZES.contains(&s) => Ok(true),
            _ => Err(refuse(
                "icon-size-invalid",
                format!("`size:` is one of {} as a string literal", SIZES.join(", ")),
                b.span,
            )),
        },
        "decorative" => match bool_of(b) {
            Some(_) => Ok(true),
            None => Err(refuse(
                "icon-decorative-invalid",
                "`decorative:` is `true` or `false`, written in the program".to_string(),
                b.span,
            )),
        },
        other => Err(refuse(
            "binding-unknown",
            format!("`{other}:` is not a binding of Icon; it takes label, {}", KEYS.join(", ")),
            b.span,
        )),
    }
}

fn unknown(name: &str, span: Span) -> LowerError {
    let near = nearest(name);
    refuse(
        "icon-unknown",
        format!(
            "`{name}` is not an icon of the vendored set (`name:` is a string literal of one of its {} names){}; the set is the files of crates/cancho-ui-ir/vendor/lucide, add one with scripts/vendor_lucide.sh",
            SET.len(),
            if near.is_empty() {
                String::new()
            } else {
                format!("; near it: {}", near.join(", "))
            }
        ),
        span,
    )
}

/// What an `Icon` needs to be complete.
pub fn check_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
) -> Result<(), LowerError> {
    if matches!(kind, IrElementKind::Button)
        && find(bindings, "icon").is_some()
        && (!children.is_empty()
            || (accessibility.label.is_none() && accessibility.label_expr.is_none()))
    {
        return Err(refuse(
            "icon-button-invalid",
            "a Button with `icon:` draws the icon before its `label:` text: it has a `label:` and no children (for an icon on its own, `Button { label: \"..\" Icon { name: \"..\" } }`)".to_string(),
            span,
        ));
    }
    if !matches!(kind, IrElementKind::Icon) {
        return Ok(());
    }
    if find(bindings, "name").is_none() {
        return Err(refuse(
            "icon-incomplete",
            "an Icon needs `name:`, a string literal of the set: `Icon { name: \"bell\" }`".to_string(),
            span,
        ));
    }
    if !children.is_empty() {
        return Err(refuse(
            "icon-has-children",
            "an Icon has no content of its own: it is drawn from its name".to_string(),
            span,
        ));
    }
    let decorative = find(bindings, "decorative").and_then(bool_of) == Some(true);
    if decorative && accessibility.label.is_some() {
        return Err(refuse(
            "icon-label-conflict",
            "an Icon is `decorative: true` or has a `label:`, not both: the first is hidden from a screen reader, the second names it".to_string(),
            span,
        ));
    }
    if let Some(l) = &accessibility.label
        && l.trim().is_empty()
    {
        return Err(refuse(
            "icon-label-missing",
            "an Icon's `label:` says what it is; for an icon with words beside it write `decorative: true` instead".to_string(),
            span,
        ));
    }
    Ok(())
}

/// A child `Icon` with no `label:` and not `decorative:` is the visible content of a `Button` or `NavLink` that has a label: that is its name, so the icon is
/// hidden from a screen reader. Anywhere else such an icon would be a picture nobody names (`icon-label-missing`).
pub fn check_children(
    parent: &IrElementKind,
    parent_accessibility: &IrAccessibility,
    children: &[IrViewNode],
) -> Result<(), LowerError> {
    let named_parent = matches!(parent, IrElementKind::Button | IrElementKind::NavLink)
        && (parent_accessibility.label.as_deref().is_some_and(|l| !l.trim().is_empty())
            || parent_accessibility.label_expr.is_some());
    fn walk(named_parent: bool, nodes: &[IrViewNode]) -> Result<(), LowerError> {
        for n in nodes {
            match n {
                IrViewNode::Element {
                    kind: IrElementKind::Icon,
                    accessibility,
                    bindings,
                    span,
                    ..
                } => {
                    let decorative = find(bindings, "decorative").and_then(bool_of) == Some(true);
                    if accessibility.label.is_none() && !decorative && !named_parent {
                        return Err(refuse(
                            "icon-label-missing",
                            "an Icon outside a labelled Button needs `label:` (what it is, read aloud) or `decorative: true` (words beside it say it): `Icon { name: \"bell\" label: \"Muted\" }`".to_string(),
                            *span,
                        ));
                    }
                }
                IrViewNode::If { children, .. } | IrViewNode::For { children, .. } => {
                    walk(named_parent, children)?
                }
                _ => {}
            }
        }
        Ok(())
    }
    walk(named_parent, children)
}

/// The size class and the pixel size written as the fallback attribute.
fn size_of(bindings: &[IrBinding]) -> (&'static str, &'static str) {
    match find(bindings, "size").and_then(string_of) {
        Some("small") => ("cui-icon cui-icon-s", "16"),
        Some("large") => ("cui-icon cui-icon-l", "28"),
        _ => ("cui-icon cui-icon-m", "20"),
    }
}

/// The `<svg>` of a named icon. `label`: `Some` makes it `role="img"` with that name; `None` hides it from a screen reader.
pub fn svg(name: &str, class: &'static str, px: &'static str, label: Option<&str>) -> Dom {
    let mut svg = Dom::element("svg")
        .attr("class", class)
        .attr("data-icon", name.to_string())
        .attr("viewBox", "0 0 24 24")
        .attr("width", px)
        .attr("height", px)
        .attr("fill", "none")
        .attr("stroke", "currentColor")
        .attr("stroke-width", "2")
        .attr("stroke-linecap", "round")
        .attr("stroke-linejoin", "round");
    match label {
        Some(l) => {
            svg = svg.attr("role", "img").attr("aria-label", l);
        }
        None => {
            svg = svg.attr("aria-hidden", "true").attr("focusable", "false");
        }
    }
    svg.children = shapes(name).unwrap_or_default();
    svg
}

/// The `<svg>` an `Icon` becomes. Call it on an element that `check_complete` accepted.
pub fn dom(accessibility: &IrAccessibility, bindings: &[IrBinding]) -> Dom {
    let name = find(bindings, "name").and_then(string_of).unwrap_or("");
    let (class, px) = size_of(bindings);
    let decorative = find(bindings, "decorative").and_then(bool_of) == Some(true);
    let label = if decorative {
        None
    } else {
        accessibility.label.as_deref()
    };
    svg(name, class, px, label)
}

/// The icon of a `Button { icon: "name" .. }`, always hidden from a screen reader (the button's own label names it).
pub fn button_dom(bindings: &[IrBinding]) -> Option<Dom> {
    let name = find(bindings, "icon").and_then(string_of)?;
    Some(svg(name, "cui-icon cui-icon-m cui-icon-lead", "20", None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_of_the_set_is_plain() {
        assert!(SET.len() >= 100);
        for (name, text) in SET {
            let shapes = shapes(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!shapes.is_empty(), "{name}");
            // the file is Lucide's: 24 px box, stroke currentColor, width 2, round caps and joins
            for want in [
                "viewBox=\"0 0 24 24\"",
                "stroke=\"currentColor\"",
                "stroke-width=\"2\"",
                "stroke-linecap=\"round\"",
                "stroke-linejoin=\"round\"",
                "fill=\"none\"",
            ] {
                assert!(text.contains(want), "{name} lacks {want}");
            }
            for banned in ["<script", "<image", "href", "<use", "<style", "data:", "url(", "<foreignObject", "<!--"] {
                assert!(!text.contains(banned), "{name} has {banned}");
            }
            for s in &shapes {
                assert!(
                    matches!(s.tag, "path" | "circle" | "rect" | "line" | "polyline"),
                    "{name}: {}",
                    s.tag
                );
            }
        }
    }

    #[test]
    fn names_are_unique_and_kebab_case() {
        let n = names();
        let mut d = n.clone();
        d.dedup();
        assert_eq!(n, d);
        for x in n {
            assert!(x.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{x}");
        }
    }

    #[test]
    fn the_vendored_licence_keeps_both_notices() {
        let l = include_str!("../vendor/lucide/LICENSE");
        assert!(l.contains("ISC License") && l.contains("Lucide Icons and Contributors"));
        assert!(l.contains("The MIT License (MIT)") && l.contains("Cole Bemis"));
        let v = include_str!("../vendor/lucide/VERSION");
        assert!(v.starts_with("lucide ") && v.contains("commit "));
    }

    #[test]
    fn an_unknown_name_names_its_neighbours() {
        assert!(nearest("bell-ring").contains(&"bell"));
        assert!(nearest("zzzz").is_empty());
    }
}
