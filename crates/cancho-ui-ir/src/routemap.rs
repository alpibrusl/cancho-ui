//! `RouteMap` (docs/routes.md): the bindings it takes and the rule tags that guard them. A `RouteMap` draws the located `points:` as numbered
//! pins in an inline SVG, joined in list order by a polyline when `route:` is true, and lists the same points in a collapsed `<details>`. It takes
//! no tiles, no host and no capability: it is a diagram of positions, not a map of the land.

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// What a `RouteMap` takes; every other key is `binding-unknown` on it.
pub const ROUTEMAP_KEYS: &[&str] = &["label", "role", "points", "route"];
/// The keys that belong to a `RouteMap` and to nothing else.
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
        _ => Ok(()),
    }
}

/// A `RouteMap` has a name and the points it draws, and is not drawn in a `For`.
pub fn check_complete(
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
    Ok(())
}
