//! `SortList` (docs/drag-and-drop.md): the bindings it takes and the rule tags that guard them. A `SortList` shows the `List[String]` field
//! named by `items:` and reports a move to the event named by `on:` as two indexes; the program decides what the move does.

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// What a `SortList` takes; every other key is `binding-unknown` on it.
pub const SORTLIST_KEYS: &[&str] = &["label", "role", "items", "on"];

fn refused(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// The key of one binding against the element it is on: `items:` belongs to a `SortList` and names a `List` state field of the component;
/// a `SortList` takes nothing but its own keys.
pub fn check_binding(
    c: &IrComponent,
    kind: &IrElementKind,
    b: &IrBinding,
) -> Result<(), LowerError> {
    let is_sort = matches!(kind, IrElementKind::SortList);
    if b.key == "items" && !is_sort {
        return Err(refused(
            "binding-unknown",
            "`items:` is a binding of SortList, not of this element".to_string(),
            b.span,
        ));
    }
    if is_sort && !SORTLIST_KEYS.contains(&b.key.as_str()) {
        return Err(refused(
            "binding-unknown",
            format!(
                "`{}:` is not a binding of SortList; it takes {}",
                b.key,
                SORTLIST_KEYS.join(", ")
            ),
            b.span,
        ));
    }
    if is_sort && b.key == "items" {
        let ok = matches!(&b.value, IrBindingValue::StateRef(name)
            if c.state.iter().any(|f| &f.name == name && matches!(f.ty, IrType::List { .. })));
        if !ok {
            return Err(refused(
                "sortlist-binding-invalid",
                "a SortList's `items:` takes the name of a `List` state field of this component, the rows it shows and the program reorders"
                    .to_string(),
                b.span,
            ));
        }
    }
    Ok(())
}

/// A `SortList` has a name, the list it shows and the event it reports to, and it is not drawn once per row of a `For` (its drag belongs to
/// the place it has in the source).
pub fn check_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    span: Span,
    in_for: bool,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::SortList) {
        return Ok(());
    }
    if accessibility.label.is_none() && accessibility.label_expr.is_none() {
        return Err(refused(
            "sortlist-incomplete",
            "a SortList needs `label:`, the name of the list: `SortList { label: \"Tasks\" items: tasks on: Moved }`".to_string(),
            span,
        ));
    }
    for (key, what) in [
        ("items", "the `List` field it shows"),
        ("on", "the event that hears a move, `on Moved(from, to)`"),
    ] {
        if !bindings.iter().any(|b| b.key == key) {
            return Err(refused(
                "sortlist-incomplete",
                format!(
                    "a SortList needs `{key}:`, {what}: `SortList {{ label: \"Tasks\" items: tasks on: Moved }}`"
                ),
                span,
            ));
        }
    }
    if in_for {
        return Err(refused(
            "sortlist-in-loop",
            "a SortList is not drawn inside a `For`; draw one list, or one `SortList` per list"
                .to_string(),
            span,
        ));
    }
    Ok(())
}
