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
