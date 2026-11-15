//! The outline of the view (docs/view-test.md): what the page shows for a state, as one line for each node a person or an assistive technology meets, in the words of the accessibility tree.
//!
//! A pure function of the state: the view is walked with the state, an `If` that is false is not in it, and a `For` is drawn once for each element of its list with the loop variable in scope. A node this
//! module cannot say is a line `? Kind`, and a view that asserts one is refused (`view-unsupported`); a view expression that needs a request, the clock or the storage is
//! `view-expression-unsupported`.

use cancho_ui_ir::ir::{IrBinding, IrBindingValue, IrElementKind, IrExpr, IrTextPart, IrViewNode};

use crate::eval::Stop;
use crate::machine::Machine;
use crate::value::Value;

/// Why an outline could not be written.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineError {
    pub rule: &'static str,
    pub message: String,
}

fn unsupported(message: impl Into<String>) -> OutlineError {
    OutlineError {
        rule: "view-expression-unsupported",
        message: message.into(),
    }
}

/// The text JavaScript's `String(n)` gives a number, where it is the same as Rust's shortest form; `None` where the two differ (very small and very large numbers).
fn number_text(n: f64) -> Option<String> {
    if n.is_nan() {
        return Some("NaN".into());
    }
    if n.is_infinite() {
        return Some(if n > 0.0 { "Infinity" } else { "-Infinity" }.into());
    }
    if n == 0.0 {
        return Some("0".into());
    }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        return Some(format!("{}", n as i128));
    }
    if n.abs() < 1e-6 || n.abs() >= 1e21 {
        return None;
    }
    Some(format!("{n}"))
}

/// The text `String(value)` gives in the page.
fn text_of_value(v: &Value) -> Option<String> {
    match v {
        Value::Num(n) => number_text(*n),
        Value::Str(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::List(items) => {
            let parts: Option<Vec<String>> = items.iter().map(text_of_value).collect();
            parts.map(|p| p.join(","))
        }
        _ => None,
    }
}

/// A quoted name or value, as JSON writes a string.
fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

/// Whitespace collapsed and trimmed, as the accessible name of a node made of its content is.
fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn kind_name(kind: &IrElementKind) -> String {
    match kind {
        IrElementKind::Named(n) => format!("component {n}"),
        IrElementKind::Island(_) => "Island".to_string(),
        other => {
            let d = format!("{other:?}");
            d.split(['(', ' ', '{']).next().unwrap_or("").to_string()
        }
    }
}

fn binding<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrBindingValue> {
    bindings.iter().find(|b| b.key == key).map(|b| &b.value)
}

struct Walk<'m, 'c> {
    m: &'m Machine<'c>,
    locals: Vec<(String, Value)>,
    out: Vec<String>,
}

impl Walk<'_, '_> {
    fn eval(&self, e: &IrExpr) -> Result<Value, OutlineError> {
        match self.m.eval_view_with(e, &self.locals) {
            Ok(v) => Ok(v),
            Err(Stop::Suspend(_)) => {
                Err(unsupported("a view expression cannot wait for a request"))
            }
            Err(Stop::Fault(f)) => Err(unsupported(format!(
                "a view expression the replay cannot evaluate: {}",
                f.message
            ))),
        }
    }

    fn text(&self, e: &IrExpr) -> Result<String, OutlineError> {
        let v = self.eval(e)?;
        text_of_value(&v).ok_or_else(|| {
            unsupported("a view expression whose text in the page the replay cannot say exactly")
        })
    }

    fn flag(&self, bindings: &[IrBinding], key: &str) -> Result<bool, OutlineError> {
        match binding(bindings, key) {
            Some(IrBindingValue::StateRef(name)) => {
                Ok(matches!(self.m.state.get(name), Some(Value::Bool(true))))
            }
            Some(IrBindingValue::Const { expr, .. }) => {
                Ok(matches!(self.eval(expr)?, Value::Bool(true)))
            }
            _ => Ok(false),
        }
    }

    fn label(&self, n: &cancho_ui_ir::ir::IrAccessibility) -> Result<Option<String>, OutlineError> {
        if let Some(l) = &n.label {
            return Ok(Some(l.clone()));
        }
        match &n.label_expr {
            Some(e) => Ok(Some(self.text(e)?)),
            None => Ok(None),
        }
    }

    /// The text content of nodes: what the page's `textContent` is for them.
    fn content(&mut self, nodes: &[IrViewNode]) -> Result<String, OutlineError> {
        let mut s = String::new();
        for n in nodes {
            match n {
                IrViewNode::Text { parts, .. } => {
                    for p in parts {
                        match p {
                            IrTextPart::Literal(l) => s.push_str(l),
                            IrTextPart::Expr(e) => s.push_str(&self.text(e)?),
                        }
                    }
                }
                IrViewNode::Element {
                    kind,
                    accessibility,
                    children,
                    ..
                } => match kind {
                    IrElementKind::Key => {}
                    IrElementKind::Button => {
                        if let Some(l) = self.label(accessibility)? {
                            s.push_str(&l);
                        }
                    }
                    _ => s.push_str(&self.content(children)?),
                },
                IrViewNode::For {
                    over,
                    binding,
                    children,
                    ..
                } => {
                    for item in self.items(over)? {
                        self.locals.push((binding.clone(), item));
                        let r = self.content(children);
                        self.locals.pop();
                        s.push_str(&r?);
                    }
                }
                IrViewNode::If { cond, children, .. } => {
                    if !matches!(self.eval(cond)?, Value::Bool(false)) {
                        s.push_str(&self.content(children)?);
                    }
                }
            }
        }
        Ok(normalize(&s))
    }

    fn items(&self, over: &IrExpr) -> Result<Vec<Value>, OutlineError> {
        match self.eval(over)? {
            Value::List(items) => Ok(items),
            _ => Err(unsupported("a `For` over something that is not a list")),
        }
    }

    fn line(&mut self, depth: usize, role: &str, name: Option<String>, props: Vec<String>) {
        let mut l = "  ".repeat(depth);
        l.push_str(role);
        if let Some(n) = name {
            l.push(' ');
            l.push_str(&quote(&n));
        }
        for p in props {
            l.push(' ');
            l.push_str(&p);
        }
        self.out.push(l);
    }

    fn nodes(&mut self, nodes: &[IrViewNode], depth: usize) -> Result<(), OutlineError> {
        for n in nodes {
            self.node(n, depth)?;
        }
        Ok(())
    }

    fn node(&mut self, n: &IrViewNode, depth: usize) -> Result<(), OutlineError> {
        match n {
            IrViewNode::Text { .. } => {
                let t = self.content(std::slice::from_ref(n))?;
                self.line(depth, "text", Some(t), vec![]);
                Ok(())
            }
            IrViewNode::For {
                over,
                binding,
                children,
                ..
            } => {
                for item in self.items(over)? {
                    self.locals.push((binding.clone(), item));
                    let r = self.nodes(children, depth);
                    self.locals.pop();
                    r?;
                }
                Ok(())
            }
            IrViewNode::If { cond, children, .. } => {
                if !matches!(self.eval(cond)?, Value::Bool(false)) {
                    self.nodes(children, depth)?;
                }
                Ok(())
            }
            IrViewNode::Element {
                kind,
                accessibility,
                bindings,
                children,
                ..
            } => self.element(kind, accessibility, bindings, children, depth),
        }
    }

    fn int_of(&self, bindings: &[IrBinding], key: &str) -> Option<i64> {
        match binding(bindings, key) {
            Some(IrBindingValue::Const { expr, .. }) => match **expr {
                IrExpr::IntLit { value, .. } => Some(value),
                _ => None,
            },
            _ => None,
        }
    }

    fn element(
        &mut self,
        kind: &IrElementKind,
        accessibility: &cancho_ui_ir::ir::IrAccessibility,
        bindings: &[IrBinding],
        children: &[IrViewNode],
        depth: usize,
    ) -> Result<(), OutlineError> {
        use IrElementKind as K;
        let numeric = matches!(
            binding(bindings, "numeric"),
            Some(IrBindingValue::Const { expr, .. }) if matches!(**expr, IrExpr::BoolLit { value: true, .. })
        );
        match kind {
            // a layout box has no role: its children are the lines; one that is given a role (`role: banner`) is that, with its name, and holds its children
            K::Column
            | K::Row
            | K::Section
            | K::Header
            | K::Nav
            | K::Main
            | K::Footer
            | K::Aside
            | K::Article
            | K::Grid => match &accessibility.role {
                Some(cancho_ui_ir::ir::IrRole::Other(r)) => {
                    let name = self.label(accessibility)?;
                    self.line(depth, r, name, vec![]);
                    self.nodes(children, depth + 1)
                }
                _ => self.nodes(children, depth),
            },
            K::Key => Ok(()),
            K::Text => {
                let t = self.content(children)?;
                let role = match &accessibility.role {
                    Some(cancho_ui_ir::ir::IrRole::Heading) => {
                        self.line(depth, "heading", Some(t), vec!["level=2".into()]);
                        return Ok(());
                    }
                    Some(cancho_ui_ir::ir::IrRole::Other(r)) if r == "status" || r == "alert" => {
                        r.clone()
                    }
                    _ => "text".to_string(),
                };
                self.line(depth, &role, Some(t), vec![]);
                Ok(())
            }
            K::Heading => {
                let t = self.content(children)?;
                let level = self.int_of(bindings, "level").unwrap_or(2);
                self.line(depth, "heading", Some(t), vec![format!("level={level}")]);
                Ok(())
            }
            K::Button => {
                let name = self.label(accessibility)?.unwrap_or_default();
                let mut props = Vec::new();
                if self.flag(bindings, "pressed")? {
                    props.push("pressed".to_string());
                }
                if self.flag(bindings, "disabled")? {
                    props.push("disabled".to_string());
                }
                self.line(depth, "button", Some(name), props);
                Ok(())
            }
            K::TextField | K::TextArea => {
                let name = self.label(accessibility)?.unwrap_or_default();
                let value = match binding(bindings, "value") {
                    Some(IrBindingValue::StateRef(n)) => match self.m.state.get(n) {
                        Some(v) => text_of_value(v).unwrap_or_default(),
                        None => String::new(),
                    },
                    Some(IrBindingValue::String(s)) => s.clone(),
                    _ => String::new(),
                };
                let mut props = vec![format!("value={}", quote(&value))];
                if self.flag(bindings, "disabled")? {
                    props.push("disabled".to_string());
                }
                if let Some(IrBindingValue::StateRef(n)) = binding(bindings, "error")
                    && matches!(self.m.state.get(n), Some(Value::Str(s)) if !s.is_empty())
                {
                    props.push("invalid".to_string());
                }
                self.line(depth, "textbox", Some(name), props);
                Ok(())
            }
            K::Checkbox => {
                let name = self.label(accessibility)?.unwrap_or_default();
                let mut props = Vec::new();
                if self.flag(bindings, "checked")? {
                    props.push("checked".to_string());
                }
                if self.flag(bindings, "disabled")? {
                    props.push("disabled".to_string());
                }
                self.line(depth, "checkbox", Some(name), props);
                Ok(())
            }
            K::Table => {
                let name = self.label(accessibility)?;
                let mut props = Vec::new();
                if let Some(IrBindingValue::Const { expr, .. }) = binding(bindings, "total") {
                    let total = match self.eval(expr)? {
                        Value::Num(n) if n.is_finite() => n.floor().max(0.0) as i64,
                        _ => 0,
                    };
                    // the header rows count: the leading rows made of header cells only
                    let header_rows = children
                        .iter()
                        .take_while(|c| match c {
                            IrViewNode::Element {
                                kind: K::TableRow,
                                children,
                                ..
                            } => {
                                !children.is_empty()
                                    && children.iter().all(|h| {
                                        matches!(
                                            h,
                                            IrViewNode::Element {
                                                kind: K::HeaderCell,
                                                ..
                                            }
                                        )
                                    })
                            }
                            _ => false,
                        })
                        .count() as i64;
                    props.push(format!("rows={}", total + header_rows));
                }
                self.line(depth, "table", name, props);
                self.nodes(children, depth + 1)
            }
            K::TableRow => {
                self.line(depth, "row", None, vec![]);
                self.nodes(children, depth + 1)
            }
            K::HeaderCell | K::Cell => {
                let name = self.content(children)?;
                let mut props = Vec::new();
                if matches!(kind, K::HeaderCell)
                    && let Some(IrBindingValue::Const { expr, .. }) = binding(bindings, "sorted")
                {
                    let s = self.text(expr)?;
                    if !s.is_empty() {
                        props.push(format!("sort={s}"));
                    }
                }
                if numeric {
                    props.push("numeric".to_string());
                }
                let role = if matches!(kind, K::HeaderCell) {
                    "columnheader"
                } else {
                    "cell"
                };
                self.line(depth, role, Some(name), props);
                // what a cell holds that is a control is a line of its own, a level in
                let controls: Vec<IrViewNode> = children
                    .iter()
                    .filter(|c| {
                        matches!(
                            c,
                            IrViewNode::Element { kind, .. } if !matches!(kind, K::Text | K::Key)
                        )
                    })
                    .cloned()
                    .collect();
                self.nodes(&controls, depth + 1)
            }
            K::List => {
                self.line(depth, "list", self.label(accessibility)?, vec![]);
                self.nodes(children, depth + 1)
            }
            K::Item => {
                let t = self.content(children)?;
                self.line(depth, "listitem", Some(t), vec![]);
                Ok(())
            }
            other => {
                self.line(depth, &format!("? {}", kind_name(other)), None, vec![]);
                Ok(())
            }
        }
    }
}

impl Machine<'_> {
    /// The outline of the component's view for the state it is in now.
    pub fn outline(&self) -> Result<Vec<String>, OutlineError> {
        // a view may read what the browser holds, which a trace does not give the outline (docs/view-test.md section 1)
        if let Some(r) = self
            .comp
            .requires
            .iter()
            .find(|r| matches!(r.name.as_str(), "Storage" | "Router" | "Credential"))
        {
            return Err(unsupported(format!(
                "this component `requires {}`: its view may read what the browser holds, so an outline of it would be the page for an empty browser",
                r.name
            )));
        }
        let mut w = Walk {
            m: self,
            locals: Vec::new(),
            out: Vec::new(),
        };
        w.node(&self.comp.view.body, 0)?;
        Ok(w.out)
    }
}
