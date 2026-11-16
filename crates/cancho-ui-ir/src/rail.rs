//! `Rail` (docs/structure.md 12.0s): a scrolling row (or column) of the children it holds, each snapping into place, each one a thing to choose.
//!
//! * **`Rail { label: "Legs" on: Pick selected: current follow: true snap: "center" gap: 2 item_width: 74 dots: true orientation: "horizontal" .. children }`.**
//!   The scrolling and the snapping are CSS (`scroll-snap-type`, `scroll-snap-align`): no script scrolls it. What the script does is the choosing: a child
//!   is told apart by its place (0 is the first child), a tap or Enter or Space on a child gives `on:` its place, and `selected:` (an `Int` the view reads)
//!   marks the one that is chosen. A child cannot tell its parent anything (docs/semantics.md: there are no child-to-parent events), so the rail is the one that
//!   listens, as a `Select` is the one that hears its options; this is the same one-way flow, state to the view and the event back to the state.
//! * With no `on:` a rail is a scrolling group (`role="group"`, named, focusable so the keyboard can scroll it). With `on:` it is a `listbox`, each child an
//!   `option` with `aria-selected`, one of them in the tab order and the arrow keys moving among them (Home and End too).
//! * The page is drawn again from nothing at every render, which would put the row back at its start: the position of each rail is kept and given back.

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// What a `Rail` takes beside `label:`.
pub const RAIL_KEYS: &[&str] = &[
    "on",
    "selected",
    "follow",
    "orientation",
    "snap",
    "gap",
    "item_width",
    "dots",
];
/// The words of `orientation:`.
pub const ORIENTATIONS: &[&str] = &["horizontal", "vertical"];
/// The words of `snap:`: where a child stops when the rail settles on it (`none` turns snapping off).
pub const SNAPS: &[&str] = &["start", "center", "end", "none"];
/// The least and the most of `item_width:`, a percentage of the rail's width.
pub const ITEM_WIDTH_MIN: i64 = 20;
pub const ITEM_WIDTH_MAX: i64 = 100;

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

fn find<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrBinding> {
    bindings.iter().find(|b| b.key == key)
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

fn int_of(b: &IrBinding) -> Option<i64> {
    match &b.value {
        IrBindingValue::Const { expr, .. } => match **expr {
            IrExpr::IntLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    }
}

/// `true` when the rail is a column of children that scrolls up and down.
pub fn vertical(bindings: &[IrBinding]) -> bool {
    find(bindings, "orientation").and_then(string_of) == Some("vertical")
}

/// The word of `snap:`, `"start"` when it is not written.
pub fn snap_of(bindings: &[IrBinding]) -> &str {
    find(bindings, "snap")
        .and_then(string_of)
        .unwrap_or("start")
}

/// `item_width:` as a percentage, when written.
pub fn item_width_of(bindings: &[IrBinding]) -> Option<i64> {
    find(bindings, "item_width").and_then(int_of)
}

/// Whether `flag:` is written `true`.
pub fn flag(bindings: &[IrBinding], key: &str) -> bool {
    find(bindings, key).and_then(bool_of) == Some(true)
}

/// Whether the rail has an `on:`: the children are then the things to choose.
pub fn selectable(bindings: &[IrBinding]) -> bool {
    find(bindings, "on").is_some()
}

/// Checks one binding. `Ok(true)`: this module owns it and it is fine; `Ok(false)`: not its business (or, for `on:` and `gap:`, the generic checks').
pub fn check_binding(
    c: &IrComponent,
    kind: &IrElementKind,
    b: &IrBinding,
) -> Result<bool, LowerError> {
    match kind {
        IrElementKind::Rail => match b.key.as_str() {
            "on" | "gap" => Ok(false),
            "orientation" => match string_of(b) {
                Some(w) if ORIENTATIONS.contains(&w) => Ok(true),
                _ => Err(refuse(
                    "rail-orientation-invalid",
                    format!(
                        "`orientation:` is one of {}, written as a string literal",
                        ORIENTATIONS
                            .iter()
                            .map(|w| format!("{w:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    b.span,
                )),
            },
            "snap" => match string_of(b) {
                Some(w) if SNAPS.contains(&w) => Ok(true),
                _ => Err(refuse(
                    "rail-snap-invalid",
                    format!(
                        "`snap:` is where a child stops: one of {}, written as a string literal",
                        SNAPS
                            .iter()
                            .map(|w| format!("{w:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    b.span,
                )),
            },
            "item_width" => match int_of(b) {
                Some(n) if (ITEM_WIDTH_MIN..=ITEM_WIDTH_MAX).contains(&n) => Ok(true),
                _ => Err(refuse(
                    "rail-item-width-invalid",
                    format!(
                        "`item_width:` takes an integer literal from {ITEM_WIDTH_MIN} to {ITEM_WIDTH_MAX}: the width of a child, as a percentage of the rail (the next one shows at its edge when it is under 100)"
                    ),
                    b.span,
                )),
            },
            "follow" | "dots" => match bool_of(b) {
                Some(_) => Ok(true),
                None => Err(refuse(
                    "rail-flag-invalid",
                    format!("`{}:` is `true` or `false`, written in the program", b.key),
                    b.span,
                )),
            },
            "selected" => match &b.value {
                IrBindingValue::Const { expr, .. } => {
                    let bad = match &**expr {
                        IrExpr::StringLit { .. }
                        | IrExpr::BoolLit { .. }
                        | IrExpr::FloatLit { .. } => true,
                        IrExpr::Ident { name, .. } => c
                            .state
                            .iter()
                            .any(|f| &f.name == name && f.ty != IrType::Int),
                        _ => false,
                    };
                    if bad {
                        Err(refuse(
                            "rail-selected-invalid",
                            "`selected:` is an `Int` the view reads (a state field, or a name of a `For`): the place of the chosen child, 0 for the first; a number out of range, or -1, marks none".to_string(),
                            b.span,
                        ))
                    } else {
                        Ok(true)
                    }
                }
                _ => Err(refuse(
                    "rail-selected-invalid",
                    "`selected:` is an `Int` the view reads: `Rail { label: \"Legs\" on: Pick selected: current .. }`".to_string(),
                    b.span,
                )),
            },
            other => Err(refuse(
                "binding-unknown",
                format!(
                    "`{other}:` is not a binding of Rail; it takes label, {}",
                    RAIL_KEYS.join(", ")
                ),
                b.span,
            )),
        },
        _ => {
            if matches!(
                b.key.as_str(),
                "selected" | "follow" | "orientation" | "snap" | "item_width" | "dots"
            ) {
                return Err(refuse(
                    "binding-unknown",
                    format!("`{}:` is a binding of Rail, not of this element", b.key),
                    b.span,
                ));
            }
            Ok(false)
        }
    }
}

/// What a `Rail` needs to be complete: a name, something to hold, and the `on:` that `selected:` and `follow:` are about.
pub fn check_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Rail) {
        return Ok(());
    }
    let named = accessibility
        .label
        .as_deref()
        .is_some_and(|l| !l.trim().is_empty())
        || accessibility.label_expr.is_some();
    if !named || children.is_empty() {
        return Err(refuse(
            "rail-incomplete",
            "a Rail needs `label:` (its name: a scrolling row is announced by it) and at least one child (or a `For` that makes them): `Rail { label: \"Legs\" Column { .. } Column { .. } }`".to_string(),
            span,
        ));
    }
    if !selectable(bindings) {
        for key in ["selected", "follow"] {
            if let Some(b) = find(bindings, key) {
                return Err(refuse(
                    if key == "selected" {
                        "rail-selected-needs-on"
                    } else {
                        "rail-follow-needs-on"
                    },
                    format!(
                        "`{key}:` is about the child that is chosen, and a Rail with no `on:` chooses nothing: add `on: Pick` (the event that is told the place of the child)"
                    ),
                    b.span,
                ));
            }
        }
    }
    Ok(())
}

fn any_kind(ir: &IrFile, pred: fn(&IrElementKind) -> bool) -> bool {
    fn walk(node: &IrViewNode, pred: fn(&IrElementKind) -> bool) -> bool {
        match node {
            IrViewNode::Element { kind, children, .. } => {
                pred(kind) || children.iter().any(|c| walk(c, pred))
            }
            IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                children.iter().any(|c| walk(c, pred))
            }
            IrViewNode::Text { .. } => false,
        }
    }
    ir.components.iter().any(|c| walk(&c.view.body, pred))
}

/// Whether any view has a `Rail`: its rules and its script are written for a program that does, so every other page is the bytes it was.
pub fn uses_rail(ir: &IrFile) -> bool {
    any_kind(ir, |k| matches!(k, IrElementKind::Rail))
}

/// Whether a view (one component's body) has a `Rail`.
pub fn view_has_rail(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            matches!(kind, IrElementKind::Rail) || children.iter().any(view_has_rail)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(view_has_rail)
        }
        IrViewNode::Text { .. } => false,
    }
}

/// Is `value` (what a handler fired by a rail receives) a place a rail could give: a whole number, 0 or more?
pub fn check_index(value: f64) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value < 1.0e9 {
        Ok(())
    } else {
        Err("a rail gives the place of the child that was chosen: a whole number, 0 or more".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_a_whole_number_from_zero() {
        assert!(check_index(0.0).is_ok());
        assert!(check_index(7.0).is_ok());
        assert!(check_index(-1.0).is_err());
        assert!(check_index(1.5).is_err());
        assert!(check_index(f64::NAN).is_err());
        assert!(check_index(f64::INFINITY).is_err());
    }

    #[test]
    fn the_words_are_the_documented_ones() {
        assert_eq!(ORIENTATIONS, &["horizontal", "vertical"]);
        assert_eq!(SNAPS, &["start", "center", "end", "none"]);
        assert_eq!((ITEM_WIDTH_MIN, ITEM_WIDTH_MAX), (20, 100));
    }
}
