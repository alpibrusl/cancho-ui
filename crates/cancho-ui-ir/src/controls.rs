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
];

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
        IrViewNode::For { children, .. } => {
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
