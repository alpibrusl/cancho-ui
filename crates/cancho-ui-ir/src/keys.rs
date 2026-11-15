//! `Key { press: "Shift+ArrowDown" on: Extend }` (docs/keys.md): a key a container binds to an event.
//!
//! A `Key` draws nothing. It belongs to the element that holds it, which the web backend makes focusable and gives one `keydown` listener for each key. What is checked here is what the
//! page could not say wrongly without a refusal: the combination is one of a closed list, the container has a name, two keys of one container are not the same combination, and the event
//! is the component's own and takes the arguments the key gives it.

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The keys a `press:` names, beside one lowercase letter or digit.
pub const NAMED: &[&str] = &[
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Enter",
    "Space",
    "Escape",
    "Delete",
    "Backspace",
];

/// The keys that repeat while held: a held arrow walks, a held `Delete` is one deletion.
pub const REPEATING: &[&str] = &[
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "PageUp",
    "PageDown",
];

/// A combination: `Control+` and `Shift+` in that order, then the key.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Combo {
    pub control: bool,
    pub shift: bool,
    pub key: String,
}

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

fn is_arrow_or_edge(k: &str) -> bool {
    matches!(
        k,
        "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Home" | "End"
    )
}

/// The combination a `press:` spells, or `None` when it is not one of the closed list.
pub fn parse_combo(s: &str) -> Option<Combo> {
    let mut rest = s;
    let control = rest
        .strip_prefix("Control+")
        .inspect(|r| rest = r)
        .is_some();
    let shift = rest.strip_prefix("Shift+").inspect(|r| rest = r).is_some();
    let named = NAMED.contains(&rest);
    let mut chars = rest.chars();
    let single = matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_lowercase() || c.is_ascii_digit());
    if !named && !single {
        return None;
    }
    // `Control+` is for the keys of the grid pattern; a letter takes no modifier (`Shift+a` is a capital letter, typed)
    if control && !is_arrow_or_edge(rest) {
        return None;
    }
    if shift && single {
        return None;
    }
    Some(Combo {
        control,
        shift,
        key: rest.to_string(),
    })
}

fn press_of(b: &IrBinding) -> Option<&str> {
    match &b.value {
        IrBindingValue::String(s) => Some(s.as_str()),
        _ => None,
    }
}

/// The bindings of a `Key`: `press:` (a combination), `on:` (an event of the component, with arguments or none) and `disabled:` (a `Bool` state field).
pub fn check_binding(c: &IrComponent, b: &IrBinding) -> Result<(), LowerError> {
    match b.key.as_str() {
        "press" => match press_of(b) {
            Some(s) if parse_combo(s).is_some() => Ok(()),
            Some(s) => Err(refuse(
                "key-combo-invalid",
                format!(
                    "`press: \"{s}\"` is not a key a program can bind: `Control+` (only with an arrow, Home or End) then `Shift+`, then one of {} or a lowercase letter or digit (a letter takes no modifier); `Tab`, `Alt+` and `Meta+` are never bound",
                    NAMED.join(", ")
                ),
                b.span,
            )),
            None => Err(refuse(
                "key-combo-invalid",
                "`press:` is a text literal: `press: \"ArrowDown\"`".to_string(),
                b.span,
            )),
        },
        "on" => crate::controls::check_on_key(c, b),
        "disabled" => match &b.value {
            IrBindingValue::StateRef(name)
                if c.state
                    .iter()
                    .any(|f| &f.name == name && f.ty == IrType::Bool) =>
            {
                Ok(())
            }
            _ => Err(refuse(
                "disabled-not-bool",
                "`disabled:` of a Key is the name of a `Bool` state field".to_string(),
                b.span,
            )),
        },
        other => Err(refuse(
            "binding-unknown",
            format!("`{other}:` is not a binding of Key; it takes press, on and disabled"),
            b.span,
        )),
    }
}

/// The keys of every container of a component: where they are, whom they belong to, whether two are the same.
pub fn check(c: &IrComponent) -> Result<(), LowerError> {
    walk(&c.view.body, true)
}

fn walk(node: &IrViewNode, outside: bool) -> Result<(), LowerError> {
    match node {
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span,
        } => {
            if matches!(kind, IrElementKind::Key) {
                if outside {
                    return Err(refuse(
                        "key-outside-container",
                        "a `Key` belongs to the element that holds it, so it is a direct child of a container, not of the component's top level, a `For` or an `If`".to_string(),
                        *span,
                    ));
                }
                if !bindings.iter().any(|b| b.key == "press") {
                    return Err(refuse(
                        "key-combo-invalid",
                        "a `Key` says which key it is: `Key { press: \"ArrowDown\" on: Move }`"
                            .to_string(),
                        *span,
                    ));
                }
                if !bindings.iter().any(|b| b.key == "on") {
                    return Err(refuse(
                        "key-event-missing",
                        "a `Key` says which event it fires: `Key { press: \"ArrowDown\" on: Move }`".to_string(),
                        *span,
                    ));
                }
                if !children.is_empty() {
                    return Err(refuse(
                        "key-child-invalid",
                        "a `Key` holds nothing: `Key { press: \"ArrowDown\" on: Move }`"
                            .to_string(),
                        *span,
                    ));
                }
                return Ok(());
            }
            let keys: Vec<(&IrBinding, &Span)> = children
                .iter()
                .filter_map(|n| match n {
                    IrViewNode::Element {
                        kind: IrElementKind::Key,
                        bindings,
                        span,
                        ..
                    } => bindings
                        .iter()
                        .find(|b| b.key == "press")
                        .map(|b| (b, span)),
                    _ => None,
                })
                .collect();
            let has_key = children.iter().any(|n| {
                matches!(
                    n,
                    IrViewNode::Element {
                        kind: IrElementKind::Key,
                        ..
                    }
                )
            });
            if has_key && accessibility.label.is_none() && accessibility.label_expr.is_none() {
                return Err(refuse(
                    "key-parent-unnamed",
                    "an element that takes keys is a place the keyboard goes to, and needs a name: add `label: \"..\"` to the container of the `Key`s".to_string(),
                    *span,
                ));
            }
            let mut seen: Vec<Combo> = Vec::new();
            for (b, _) in &keys {
                if let Some(combo) = press_of(b).and_then(parse_combo) {
                    if seen.contains(&combo) {
                        return Err(refuse(
                            "key-duplicate",
                            format!(
                                "two keys of one container are both `{}`",
                                press_of(b).unwrap_or_default()
                            ),
                            b.span,
                        ));
                    }
                    seen.push(combo);
                }
            }
            let _ = bindings;
            for ch in children {
                walk(ch, false)?;
            }
            Ok(())
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for ch in children {
                walk(ch, true)?;
            }
            Ok(())
        }
        IrViewNode::Text { .. } => Ok(()),
    }
}
