//! The bindings of the built-in elements (docs/controls.md): which keys exist, and what `disabled:` may be.
//!
//! Before this pass an unknown key on `Column`, `Text`, `Button` or `TextField` was accepted and either dropped or
//! written as an HTML attribute, so `Button { disabled: true }` compiled and did nothing (issue #35).

use crate::ir::*;
use crate::lower::LowerError;

/// The keys a built-in element takes. `label`, `role`, `on`, `value`, `disabled` and `submit` have meaning;
/// `placeholder` is written as the attribute of that name.
pub const ELEMENT_KEYS: &[&str] = &[
    "label",
    "role",
    "on",
    "value",
    "disabled",
    "placeholder",
    "submit",
    "tone",
    "gap",
    "max_width",
    "type",
];

/// The `type:` of a `TextField` (docs/controls.md section 5): the HTML input types that hold one line of text and give the
/// handler a string. `number` is left out on purpose (measured: `1e5` typed key by key ends as `5`).
pub const INPUT_TYPES: &[&str] = &["text", "password", "email", "search", "tel", "url"];

/// The tones of a status text (docs/presentation.md section 3).
pub const TONES: &[&str] = &["ok", "warn", "error"];
/// The `gap:` steps of a `Row` or `Column` (section 2), and the `max_width:` range in characters.
pub const GAP_MAX: i64 = 4;
pub const MAX_WIDTH_RANGE: (i64, i64) = (10, 200);

pub fn check_controls(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
        check_node(c, &c.view.body)?;
    }
    Ok(())
}

fn check_node(c: &IrComponent, node: &IrViewNode) -> Result<(), LowerError> {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            if !matches!(kind, IrElementKind::Named(_)) {
                for b in bindings {
                    check_binding(c, kind, b)?;
                }
            }
            for ch in children {
                check_node(c, ch)?;
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for ch in children {
                check_node(c, ch)?;
            }
        }
        IrViewNode::Text { .. } => {}
    }
    Ok(())
}

fn element_name(kind: &IrElementKind) -> &'static str {
    match kind {
        IrElementKind::Column => "Column",
        IrElementKind::Row => "Row",
        IrElementKind::Table => "Table",
        IrElementKind::TableRow => "a row of a Table",
        IrElementKind::Cell | IrElementKind::HeaderCell => "a cell of a Table",
        IrElementKind::Text => "Text",
        IrElementKind::Button => "Button",
        IrElementKind::TextField => "TextField",
        IrElementKind::Named(_) => "a component",
    }
}

/// `on: Event` and `on: Event(args)` (docs/row-actions.md section 2): the event is declared by the component, a click
/// gives it exactly the arguments the view writes, and a field gives it the typed value.
fn check_on(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    let (name, args) = match &b.value {
        IrBindingValue::EventRef(n) => (n, None),
        IrBindingValue::EventCall { name, args } => (name, Some(args.len())),
        _ => return Ok(()),
    };
    let Some(event) = c.events.iter().find(|e| &e.name == name) else {
        let known: Vec<&str> = c.events.iter().map(|e| e.name.as_str()).collect();
        return Err(LowerError::ControlRefused {
            code: "event-unknown",
            message: format!(
                "`on: {name}`: this component has no `on {name}` handler (it has {})",
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            ),
            span: b.span,
        });
    };
    if args.is_some() && !matches!(kind, IrElementKind::Button) {
        return Err(LowerError::ControlRefused {
            code: "event-args-unsupported",
            message: format!(
                "`on: {name}(..)` gives a click its arguments, so it is a binding of Button, not of {}: a field gives its handler the typed value (`on: {name}`)",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let given = match (args, kind) {
        (Some(n), _) => n,
        // A click gives no arguments; a field gives the value it holds.
        (None, IrElementKind::TextField) => event.params.len().min(1),
        (None, _) => 0,
    };
    if event.params.len() != given {
        return Err(LowerError::ControlRefused {
            code: "event-arity-mismatch",
            message: format!(
                "`on {name}` takes {} parameter(s) and this binding gives it {given}{}",
                event.params.len(),
                if args.is_none()
                    && matches!(kind, IrElementKind::Button)
                    && !event.params.is_empty()
                {
                    format!("; a click has no arguments, so write them: `on: {name}(..)`")
                } else {
                    String::new()
                }
            ),
            span: b.span,
        });
    }
    Ok(())
}

/// `tone:` is a binding of a `Text` (or the cell it becomes in a table): `"ok"`, `"warn"`, `"error"` or a `String` state field.
fn check_tone(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(
        kind,
        IrElementKind::Text | IrElementKind::Cell | IrElementKind::HeaderCell
    ) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`tone:` is a binding of Text (a status text), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let bad = |message: String| LowerError::ControlRefused {
        code: "tone-invalid",
        message,
        span: b.span,
    };
    match &b.value {
        IrBindingValue::String(s) if TONES.contains(&s.as_str()) => Ok(()),
        IrBindingValue::String(s) => Err(bad(format!(
            "`tone: \"{s}\"`: the tones are {}",
            TONES.join(", ")
        ))),
        IrBindingValue::StateRef(name) => {
            if c.state
                .iter()
                .any(|f| &f.name == name && f.ty == IrType::String)
            {
                Ok(())
            } else {
                Err(bad(format!(
                    "`tone: {name}` needs a `String` state field named `{name}` in this component (its value is one of {})",
                    TONES.join(", ")
                )))
            }
        }
        _ => Err(bad(format!(
            "`tone:` takes {} as a string, or the name of a `String` state field",
            TONES.join(", ")
        ))),
    }
}

/// `type:` is a binding of a `TextField` and takes one of `INPUT_TYPES` as a string literal.
fn check_input_type(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::TextField) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`type:` is a binding of TextField (`type: \"password\"`), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    match &b.value {
        IrBindingValue::String(s) if INPUT_TYPES.contains(&s.as_str()) => Ok(()),
        _ => Err(LowerError::ControlRefused {
            code: "input-type-invalid",
            message: format!(
                "`type:` takes one of {} as a string literal; `number` is not one (a number field answers an empty string until what is typed is a number, and the page draws the field again at every key, so `1e5` typed key by key ends as `5`), and a checkbox, a choice or a date is a different control",
                INPUT_TYPES.join(", ")
            ),
            span: b.span,
        }),
    }
}

/// `gap:` and `max_width:` are bindings of a `Column` or a `Row`, and take an integer literal in a fixed range.
fn check_layout(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Column | IrElementKind::Row) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is a binding of Column and Row, not of {}",
                b.key,
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let (lo, hi) = if b.key == "gap" {
        (0, GAP_MAX)
    } else {
        MAX_WIDTH_RANGE
    };
    match &b.value {
        IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::IntLit { value, .. } if (lo..=hi).contains(&value)) => {
            Ok(())
        }
        _ => Err(LowerError::ControlRefused {
            code: "layout-value-invalid",
            message: format!(
                "`{}:` takes an integer literal from {lo} to {hi}{}",
                b.key,
                if b.key == "gap" {
                    " (a step of 0.25, 0.5, 1 and 2 rem after 0)"
                } else {
                    " (a width in characters; the page is centred in it)"
                }
            ),
            span: b.span,
        }),
    }
}

fn check_binding(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !ELEMENT_KEYS.contains(&b.key.as_str()) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is not a binding of {}; it would be dropped or written as a stray attribute. The keys are {}",
                b.key,
                element_name(kind),
                ELEMENT_KEYS.join(", ")
            ),
            span: b.span,
        });
    }
    if b.key == "on" {
        check_on(c, kind, b)?;
    }
    if b.key == "tone" {
        check_tone(c, kind, b)?;
    }
    if b.key == "gap" || b.key == "max_width" {
        check_layout(kind, b)?;
    }
    if b.key == "type" {
        check_input_type(kind, b)?;
    }
    if b.key == "submit" {
        if !matches!(kind, IrElementKind::TextField) {
            return Err(LowerError::ControlRefused {
                code: "binding-unknown",
                message: format!(
                    "`submit:` is a binding of TextField (Enter in the field fires the event), not of {}",
                    element_name(kind)
                ),
                span: b.span,
            });
        }
        let IrBindingValue::EventRef(name) = &b.value else {
            return Err(LowerError::ControlRefused {
                code: "submit-event-invalid",
                message: "`submit:` names an event of the component, e.g. `submit: Apply`"
                    .to_string(),
                span: b.span,
            });
        };
        match c.events.iter().find(|e| &e.name == name) {
            None => {
                return Err(LowerError::ControlRefused {
                    code: "submit-event-invalid",
                    message: format!("`submit: {name}`: this component has no `on {name}` handler"),
                    span: b.span,
                });
            }
            Some(e) if !e.params.is_empty() => {
                return Err(LowerError::ControlRefused {
                    code: "submit-event-invalid",
                    message: format!(
                        "`submit: {name}`: `on {name}` takes parameters, and Enter has none to give; use an event without parameters"
                    ),
                    span: b.span,
                });
            }
            Some(_) => {}
        }
    }
    if b.key == "disabled" {
        let interactive = matches!(kind, IrElementKind::Button | IrElementKind::TextField);
        if !interactive {
            return Err(LowerError::ControlRefused {
                code: "binding-unknown",
                message: format!(
                    "`disabled:` is a binding of Button and TextField, not of {}",
                    element_name(kind)
                ),
                span: b.span,
            });
        }
        match &b.value {
            IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { .. }) => {}
            IrBindingValue::StateRef(name) => {
                let is_bool = c
                    .state
                    .iter()
                    .any(|f| &f.name == name && f.ty == IrType::Bool);
                if !is_bool {
                    return Err(LowerError::ControlRefused {
                        code: "disabled-not-bool",
                        message: format!(
                            "`disabled: {name}` needs a `Bool` state field named `{name}` in this component"
                        ),
                        span: b.span,
                    });
                }
            }
            _ => {
                return Err(LowerError::ControlRefused {
                    code: "disabled-not-bool",
                    message:
                        "`disabled:` takes `true`, `false` or the name of a `Bool` state field"
                            .to_string(),
                    span: b.span,
                });
            }
        }
    }
    Ok(())
}
