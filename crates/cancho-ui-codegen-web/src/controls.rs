//! The guard of a `submit:` binding (docs/controls.md section 3).
//!
//! Enter in a field is the same act as pressing the button of the same event, so it must not get through when that
//! button is `disabled`: a field with `submit: Save` beside `Button { on: Save disabled: sending }` would otherwise send
//! a second write while the first is in flight. The page gets one table, `__submit_blocked`, event name to a function
//! that answers "is any control that fires this event disabled right now?".

use cancho_ui_ir::ir::*;

use crate::emit::STATE_PREFIX;

/// Does `e` read only state fields? A condition that does not (it reads a `For` binding or a lambda's) cannot be evaluated
/// where `__submit_blocked` runs, and replay treats it as true for the same reason (`Machine::event_refused`).
fn reads_only_state(e: &IrExpr, state: &[&str]) -> bool {
    match e {
        IrExpr::Ident { name, .. } => state.contains(&name.as_str()),
        IrExpr::Filter { .. }
        | IrExpr::Map { .. }
        | IrExpr::SortBy { .. }
        | IrExpr::Match { .. } => false,
        other => other.children().iter().all(|c| reads_only_state(c, state)),
    }
}

/// `disabled:` of every control of the view that fires `event` with `on:`, as JavaScript conditions. A control inside an
/// `If` is in the page only while the `If`'s condition holds, so its condition is `(when && disabled)`: a hidden button
/// does not block Enter (docs/view-conditional.md section 4).
fn disabled_conditions(
    node: &IrViewNode,
    event: &str,
    state: &[&str],
    guards: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    let guarded = |cond: String, guards: &Vec<String>| {
        if guards.is_empty() {
            cond
        } else {
            format!("({} && {cond})", guards.join(" && "))
        }
    };
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
                                out.push(guarded("true".to_string(), guards));
                            }
                        }
                        IrBindingValue::StateRef(name) => {
                            out.push(guarded(format!("!!{STATE_PREFIX}{name}"), guards));
                        }
                        _ => {}
                    }
                }
            }
            for c in children {
                disabled_conditions(c, event, state, guards, out);
            }
        }
        IrViewNode::For { children, .. } => {
            for c in children {
                disabled_conditions(c, event, state, guards, out);
            }
        }
        IrViewNode::If { cond, children, .. } => {
            let kept = reads_only_state(cond, state);
            if kept {
                guards.push(format!("!!({})", crate::expr::emit_expr_runtime(cond, &[])));
            }
            for c in children {
                disabled_conditions(c, event, state, guards, out);
            }
            if kept {
                guards.pop();
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
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
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
    let state: Vec<&str> = c.state.iter().map(|f| f.name.as_str()).collect();
    for e in &events {
        let mut conds = Vec::new();
        disabled_conditions(&c.view.body, e, &state, &mut Vec::new(), &mut conds);
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
