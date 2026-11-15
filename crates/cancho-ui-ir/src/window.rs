//! `Table { row_height: 2 total: n first: first on: Scrolled max_height: 24 }` (docs/window.md): a table that draws only the rows in view.
//!
//! The program draws the rows it wants (`take(skip(rows, first), 16)`); the compiler makes the box as tall as `total` rows of `row_height` and places the rows with two spacer rows, and
//! fires `on:` with the index of the first row in view when the box is scrolled. What is checked here: the four bindings go together with a height, and `row_height` is a whole number of
//! rem. That `total` and `first` are numbers is checked with the other expressions of the view (`conditions.rs`).

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The most and the fewest rem a row is.
pub const ROW_HEIGHT: (i64, i64) = (1, 20);

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
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

/// The height of a row in rem, when the table is a window.
pub fn row_height(bindings: &[IrBinding]) -> Option<i64> {
    bindings
        .iter()
        .find(|b| b.key == "row_height")
        .and_then(int_of)
}

/// The expression of `total:` or `first:`.
pub fn expr_of<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrExpr> {
    bindings
        .iter()
        .find(|b| b.key == key)
        .and_then(|b| match &b.value {
            IrBindingValue::Const { expr, .. } => Some(&**expr),
            _ => None,
        })
}

/// The event of `on:`.
pub fn event_of(bindings: &[IrBinding]) -> Option<&str> {
    bindings.iter().find_map(|b| match (&b.key[..], &b.value) {
        ("on", IrBindingValue::EventRef(n)) => Some(n.as_str()),
        _ => None,
    })
}

/// Whether the table asks for any part of a window.
pub fn is_window(bindings: &[IrBinding]) -> bool {
    bindings
        .iter()
        .any(|b| matches!(b.key.as_str(), "row_height" | "total" | "first"))
}

/// `row_height:`, `total:` and `first:` of a `Table`: handled here (`true`), whatever the other checks say of them.
pub fn check_binding(kind: &IrElementKind, b: &IrBinding) -> Result<bool, LowerError> {
    if !matches!(kind, IrElementKind::Table)
        || !matches!(b.key.as_str(), "row_height" | "total" | "first")
    {
        return Ok(false);
    }
    if b.key == "row_height" {
        match int_of(b) {
            Some(n) if (ROW_HEIGHT.0..=ROW_HEIGHT.1).contains(&n) => {}
            _ => {
                return Err(refuse(
                    "window-row-height-invalid",
                    format!(
                        "`row_height:` of a Table is an integer literal from {} to {}, in rem",
                        ROW_HEIGHT.0, ROW_HEIGHT.1
                    ),
                    b.span,
                ));
            }
        }
    }
    Ok(true)
}

/// The four bindings of a window, and the height of its box, go together.
pub fn check_complete(
    kind: &IrElementKind,
    bindings: &[IrBinding],
    span: Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Table) || !is_window(bindings) {
        return Ok(());
    }
    let has = |k: &str| bindings.iter().any(|b| b.key == k);
    for need in ["row_height", "total", "first", "on", "max_height"] {
        if !has(need) {
            return Err(refuse(
                "window-incomplete",
                format!(
                    "a Table that draws only the rows in view needs `row_height:`, `total:`, `first:`, `on:` and `max_height:`; `{need}:` is missing"
                ),
                span,
            ));
        }
    }
    Ok(())
}
