//! `SortList` in replay (docs/drag-and-drop.md): the lists that are in the page now, what a move to one of them can be, and the `sortlists`
//! of a step report. A drag and the keyboard path are both the plain event `on Moved(from, to)`; replay cannot see either, and refuses a move
//! the page could not make.

use cancho_ui_ir::ir::{IrBindingValue, IrElementKind, IrViewNode};

use crate::machine::Machine;
use crate::value::Value;

/// One `SortList` that is in the page: its name, the event it reports to and the rows it shows now.
pub(crate) struct Shown {
    pub label: String,
    pub event: String,
    pub items: Vec<Value>,
}

impl Machine<'_> {
    /// The `SortList`s in the page now, in document order (one under an `If` whose condition is false is not in it).
    pub(crate) fn sortlists(&self) -> Vec<Shown> {
        fn walk(m: &Machine<'_>, node: &IrViewNode, out: &mut Vec<Shown>) {
            match node {
                IrViewNode::Element {
                    kind,
                    accessibility,
                    bindings,
                    children,
                    ..
                } => {
                    if matches!(kind, IrElementKind::SortList) {
                        let items = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
                            ("items", IrBindingValue::StateRef(n)) => match m.state.get(n) {
                                Some(Value::List(items)) => Some(items.clone()),
                                _ => None,
                            },
                            _ => None,
                        });
                        let event = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
                            ("on", IrBindingValue::EventRef(n)) => Some(n.clone()),
                            _ => None,
                        });
                        let label = match (&accessibility.label, &accessibility.label_expr) {
                            (Some(l), _) => Some(l.clone()),
                            (None, Some(e)) => match m.eval_view(e) {
                                Some(Value::Str(s)) => Some(s),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let (Some(items), Some(event), Some(label)) = (items, event, label) {
                            out.push(Shown {
                                label,
                                event,
                                items,
                            });
                        }
                    }
                    for c in children {
                        walk(m, c, out);
                    }
                }
                IrViewNode::For { children, .. } => {
                    for c in children {
                        walk(m, c, out);
                    }
                }
                IrViewNode::If { cond, children, .. } => {
                    if m.condition_holds(cond) {
                        for c in children {
                            walk(m, c, out);
                        }
                    }
                }
                IrViewNode::Text { .. } => {}
            }
        }
        let mut out = Vec::new();
        walk(self, &self.comp.view.body, &mut out);
        out
    }

    /// The lengths of the lists that can fire `event`, when every control in the page that fires it is a `SortList` (so its parameters are
    /// two indexes into one of them); `None` when some other control could give it any value, or none fires it.
    pub fn move_bounds(&self, event: &str) -> Option<Vec<usize>> {
        fn fires(bindings: &[cancho_ui_ir::ir::IrBinding], event: &str) -> bool {
            bindings.iter().any(|b| {
                b.key == "on"
                    && match &b.value {
                        IrBindingValue::EventRef(e) => e == event,
                        IrBindingValue::EventCall { name, .. } => name == event,
                        _ => false,
                    }
            })
        }
        fn others(m: &Machine<'_>, node: &IrViewNode, event: &str) -> bool {
            match node {
                IrViewNode::Element {
                    kind,
                    bindings,
                    children,
                    ..
                } => {
                    (fires(bindings, event) && !matches!(kind, IrElementKind::SortList))
                        || children.iter().any(|c| others(m, c, event))
                }
                IrViewNode::For { children, .. } => children.iter().any(|c| others(m, c, event)),
                IrViewNode::If { cond, children, .. } => {
                    m.condition_holds(cond) && children.iter().any(|c| others(m, c, event))
                }
                IrViewNode::Text { .. } => false,
            }
        }
        if others(self, &self.comp.view.body, event) {
            return None;
        }
        let lens: Vec<usize> = self
            .sortlists()
            .iter()
            .filter(|s| s.event == event)
            .map(|s| s.items.len())
            .collect();
        (!lens.is_empty()).then_some(lens)
    }

    /// The `sortlists` of a step report: `[{label, items}]`, the rows in the order the page shows them.
    pub fn sortlists_report(&self) -> Vec<serde_json::Value> {
        self.sortlists()
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "label": s.label,
                    "count": s.items.len(),
                    "items": s.items.iter().map(Value::to_json).collect::<Vec<_>>(),
                })
            })
            .collect()
    }
}
