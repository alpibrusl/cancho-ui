//! `RouteMap` (docs/routes.md): the bindings it takes and the rule tags that guard them. A `RouteMap` draws the located `points:` as numbered
//! pins in an inline SVG, joined in list order by a polyline when `route:` is true, and lists the same points in a collapsed `<details>`. With no
//! `tiles:` it takes no host and no capability: it is a diagram of positions, not a map of the land. With `tiles:` (and `attribution:`) it draws the
//! tiles of the program's own tile host under the pins, and the host is declared exactly as a `Map`'s is.

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// What a `RouteMap` takes; every other key is `binding-unknown` on it.
pub const ROUTEMAP_KEYS: &[&str] = &["label", "role", "points", "route", "tiles", "attribution"];
/// The keys that belong to a `RouteMap` and to nothing else (`tiles` and `attribution` are shared with `Map`, which refuses them elsewhere).
const OWN_KEYS: &[&str] = &["points", "route"];

/// The most points a `RouteMap` draws and lists (the first ones, in route order).
pub const MAX_POINTS: usize = 200;
/// The longest `kind` of a row, in characters.
pub const MAX_KIND: usize = 12;

fn refused(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// Does a view hold a `RouteMap`?
pub fn view_has_routemap(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            matches!(kind, IrElementKind::RouteMap) || children.iter().any(view_has_routemap)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(view_has_routemap)
        }
        IrViewNode::Text { .. } => false,
    }
}

fn has_tiles(bindings: &[IrBinding]) -> bool {
    bindings.iter().any(|b| b.key == "tiles")
}

/// Does a view hold a `RouteMap` with `tiles:` (one that asks a tile host for images)?
pub fn view_has_tiles(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            (matches!(kind, IrElementKind::RouteMap) && has_tiles(bindings))
                || children.iter().any(view_has_tiles)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(view_has_tiles)
        }
        IrViewNode::Text { .. } => false,
    }
}

/// The key of one binding against the element it is on.
pub fn check_binding(
    c: &IrComponent,
    kind: &IrElementKind,
    b: &IrBinding,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::RouteMap) {
        if OWN_KEYS.contains(&b.key.as_str()) {
            return Err(refused(
                "binding-unknown",
                format!("`{}:` is a binding of RouteMap, not of this element", b.key),
                b.span,
            ));
        }
        return Ok(());
    }
    if !ROUTEMAP_KEYS.contains(&b.key.as_str()) {
        return Err(refused(
            "binding-unknown",
            format!(
                "`{}:` is not a binding of RouteMap; it takes {}",
                b.key,
                ROUTEMAP_KEYS.join(", ")
            ),
            b.span,
        ));
    }
    let field = |name: &str| c.state.iter().find(|f| f.name == name);
    match (b.key.as_str(), &b.value) {
        ("points", IrBindingValue::StateRef(n))
            if field(n).is_some_and(|f| matches!(f.ty, IrType::List { .. })) =>
        {
            Ok(())
        }
        ("points", _) => Err(refused(
            "routemap-binding-invalid",
            "a RouteMap's `points:` is a `List` state field of rows like {\"name\":\"Chamonix\",\"pos\":\"45.9237,6.8694\",\"kind\":\"lodging\"}, in route order"
                .to_string(),
            b.span,
        )),
        ("route", IrBindingValue::StateRef(n))
            if field(n).is_some_and(|f| matches!(f.ty, IrType::Bool)) =>
        {
            Ok(())
        }
        ("route", IrBindingValue::Const { expr, .. })
            if matches!(**expr, IrExpr::BoolLit { .. }) =>
        {
            Ok(())
        }
        ("route", _) => Err(refused(
            "routemap-binding-invalid",
            "a RouteMap's `route:` is `true`, `false` or a `Bool` state field (it defaults to true)"
                .to_string(),
            b.span,
        )),
        ("tiles", IrBindingValue::String(s)) => {
            crate::map::tile_source(s).map(|_| ()).map_err(|why| {
                refused(
                    "routemap-tiles-invalid",
                    format!("a RouteMap's `tiles:` is not a tile address: {why}"),
                    b.span,
                )
            })
        }
        ("tiles", _) => Err(refused(
            "routemap-tiles-invalid",
            "a RouteMap's `tiles:` is a text literal like \"https://tiles.example/{z}/{x}/{y}.png\" (or a path on this page: \"/tiles/{z}/{x}/{y}.png\"): the host is the program's and is written where it can be read"
                .to_string(),
            b.span,
        )),
        ("attribution", IrBindingValue::String(s))
            if !s.trim().is_empty() && s.chars().count() <= crate::map::MAX_ATTRIBUTION =>
        {
            Ok(())
        }
        ("attribution", _) => Err(refused(
            "routemap-binding-invalid",
            format!(
                "a RouteMap's `attribution:` is a text literal of 1 to {} characters, shown under the drawing",
                crate::map::MAX_ATTRIBUTION
            ),
            b.span,
        )),
        _ => Ok(()),
    }
}

/// A `RouteMap` has a name and the points it draws, and is not drawn in a `For`. With `tiles:` it also has an `attribution:`, and its tiles come from
/// a host the component has declared (`requires Network("host")`, or a bare `requires Network;` for a path on this page).
pub fn check_complete(
    c: &IrComponent,
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    span: Span,
    in_for: bool,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::RouteMap) {
        return Ok(());
    }
    let has_label = accessibility.label.is_some() || accessibility.label_expr.is_some();
    if !has_label || !bindings.iter().any(|b| b.key == "points") {
        return Err(refused(
            "routemap-incomplete",
            "a RouteMap needs `label:` (its name) and `points:` (a `List` state field of rows with `name` and `pos`): `RouteMap { label: \"Trip\" points: rows route: true }`"
                .to_string(),
            span,
        ));
    }
    if in_for {
        return Err(refused(
            "routemap-in-loop",
            "a RouteMap is not drawn inside a `For`; draw one route map of a list, or one `RouteMap` per list".to_string(),
            span,
        ));
    }
    if has_tiles(bindings) && !bindings.iter().any(|b| b.key == "attribution") {
        return Err(refused(
            "routemap-incomplete",
            "a RouteMap with `tiles:` needs `attribution:`, the credit the tile provider asks for, shown under the drawing".to_string(),
            span,
        ));
    }
    if !has_tiles(bindings) && bindings.iter().any(|b| b.key == "attribution") {
        return Err(refused(
            "routemap-binding-invalid",
            "a RouteMap's `attribution:` credits tiles; with no `tiles:` there is nothing to credit".to_string(),
            span,
        ));
    }
    let template = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("tiles", IrBindingValue::String(s)) => Some((s, b.span)),
        _ => None,
    });
    if let Some((t, at)) = template
        && let Ok(source) = crate::map::tile_source(t)
    {
        let network = c.requires.iter().find(|r| r.name == "Network");
        let declared = match (&source.host, network) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(h), Some(n)) => n.bounds.iter().any(|b| b.to_ascii_lowercase() == *h),
        };
        if !declared {
            let line = match &source.host {
                Some(h) => format!("requires Network(\"{h}\");"),
                None => "requires Network;".to_string(),
            };
            return Err(refused(
                "routemap-host-undeclared",
                format!(
                    "this RouteMap asks {} for its tiles, and the component does not declare it: add `{line}` to the component (the tile host is authority, and the semantic diff reports it as a change)",
                    source.host.as_deref().unwrap_or("this page's own origin")
                ),
                at,
            ));
        }
    }
    Ok(())
}
