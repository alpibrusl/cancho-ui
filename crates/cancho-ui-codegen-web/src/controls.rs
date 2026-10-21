//! The guard of a `submit:` binding (docs/controls.md section 3).
//!
//! Enter in a field is the same act as pressing the button of the same event, so it must not get through when that
//! button is `disabled`: a field with `submit: Save` beside `Button { on: Save disabled: sending }` would otherwise send
//! a second write while the first is in flight. The page gets one table, `__submit_blocked`, event name to a function
//! that answers "is any control that fires this event disabled right now?".

use cancho_ui_ir::ir::*;

use crate::emit::STATE_PREFIX;

/// `disabled:` of every control of the view that fires `event` with `on:`, as JavaScript conditions.
fn disabled_conditions(node: &IrViewNode, event: &str, out: &mut Vec<String>) {
    match node {
        IrViewNode::Element {
            bindings, children, ..
        } => {
            let fires = bindings.iter().any(|b| {
                b.key == "on"
                    && match &b.value {
                        IrBindingValue::EventRef(e) => e == event,
                        IrBindingValue::EventCall { name, .. } => name == event,
                        _ => false,
                    }
            });
            if fires {
                for b in bindings.iter().filter(|b| b.key == "disabled") {
                    match &b.value {
                        IrBindingValue::Const { expr, .. } => {
                            if matches!(**expr, IrExpr::BoolLit { value: true, .. }) {
                                out.push("true".to_string());
                            }
                        }
                        IrBindingValue::StateRef(name) => {
                            out.push(format!("!!{STATE_PREFIX}{name}"));
                        }
                        _ => {}
                    }
                }
            }
            for c in children {
                disabled_conditions(c, event, out);
            }
        }
        IrViewNode::For { children, .. } => {
            for c in children {
                disabled_conditions(c, event, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The events named by a `submit:` somewhere in the view.
fn submit_events(node: &IrViewNode, out: &mut Vec<String>) {
    match node {
        IrViewNode::Element {
            bindings, children, ..
        } => {
            for b in bindings.iter().filter(|b| b.key == "submit") {
                if let IrBindingValue::EventRef(e) = &b.value
                    && !out.contains(e)
                {
                    out.push(e.clone());
                }
            }
            for c in children {
                submit_events(c, out);
            }
        }
        IrViewNode::For { children, .. } => {
            for c in children {
                submit_events(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The `__submit_blocked` table of a component, or nothing when its view has no `submit:`.
pub(crate) fn submit_blocked_table(c: &IrComponent, indent: &str) -> String {
    let mut events = Vec::new();
    submit_events(&c.view.body, &mut events);
    if events.is_empty() {
        return String::new();
    }
    let mut out = format!("{indent}const __submit_blocked = {{\n");
    for e in &events {
        let mut conds = Vec::new();
        disabled_conditions(&c.view.body, e, &mut conds);
        if conds.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "{indent}    {e}: function () {{ return {}; }},\n",
            conds.join(" || ")
        ));
    }
    out.push_str(&format!("{indent}}};\n"));
    out
}
