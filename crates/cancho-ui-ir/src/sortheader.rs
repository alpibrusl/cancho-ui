//! `Text { role: columnheader sort: field key: "k" "label" }` (docs/table-sort.md): a header cell that is a button and says how its column is sorted.
//!
//! The source names a `String` state field and a column key. Before anything else sees the component, this pass writes what the author would otherwise write by hand: an
//! event `__sort_<field>(key)` that moves the field through `"k"`, `"-k"` and `""`, a `Button { label: "label" on: __sort_<field>("k") }` inside the cell, and a `sorted:`
//! binding on the cell, a pure `String` expression (`"ascending"`, `"descending"` or `""`) that the backends write as `aria-sort`. So replay, the checks and both runtimes
//! see ordinary events and ordinary expressions, and the compiler sorts nothing.

use std::borrow::Cow;

use cancho_ui_syntax::ast::*;
use cancho_ui_syntax::lexer::lex;
use cancho_ui_syntax::parser::parse;
use cancho_ui_syntax::span::Span;

use crate::lower::LowerError;

/// The longest key.
const KEY_MAX: usize = 64;

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// The name of the event that moves `field`.
pub fn event_name(field: &str) -> String {
    format!("__sort_{field}")
}

fn has_sort(n: &ViewNode) -> bool {
    match n {
        ViewNode::Element {
            bindings, children, ..
        } => {
            bindings.iter().any(|b| b.key == "sort" || b.key == "key")
                || children.iter().any(has_sort)
        }
        ViewNode::For { children, .. } | ViewNode::If { children, .. } => {
            children.iter().any(has_sort)
        }
        ViewNode::Text { .. } => false,
    }
}

/// The component a source text of this pass reads as; a refusal, not a panic, if it does not (it does for every field and key the checks let through).
fn read(program: &str, span: Span) -> Result<Component, LowerError> {
    let fail = || {
        refuse(
            "sort-key-invalid",
            "the sortable header could not be written out: its field or key is not one this pass can spell".to_string(),
            span,
        )
    };
    let tokens = lex(program).map_err(|_| fail())?;
    let file = parse(tokens).map_err(|_| fail())?;
    file.components.into_iter().next().ok_or_else(fail)
}

/// An expression written in the language, read back through the parser (the pass writes source, not trees, so that it cannot write a tree the parser would not).
fn expr_of(source: &str, span: Span) -> Result<Expr, LowerError> {
    let program = format!(
        "component A {{\n    state x: String = \"\";\n    on E {{ x = {source}; }}\n    view {{ Text {{ \"\" }} }}\n}}\n"
    );
    let c = read(&program, span)?;
    match c.events.first().and_then(|e| e.body.stmts.first()) {
        Some(Stmt::Assign { value, .. }) => Ok(value.clone()),
        _ => Err(refuse(
            "sort-key-invalid",
            "the sortable header could not be written out".to_string(),
            span,
        )),
    }
}

/// `__sort_<field>(key)`: unsorted or another column -> ascending -> descending -> unsorted.
fn handler(field: &str, span: Span) -> Result<EventHandler, LowerError> {
    let program = format!(
        "component A {{\n    state {field}: String = \"\";\n    on E(key) {{ {field} = if {field} == key {{ \"-\" + key }} else if {field} == \"-\" + key {{ \"\" }} else {{ key }}; }}\n    view {{ Text {{ \"\" }} }}\n}}\n"
    );
    let c = read(&program, span)?;
    let Some(mut e) = c.events.into_iter().next() else {
        return Err(refuse(
            "sort-key-invalid",
            "the sortable header could not be written out".to_string(),
            span,
        ));
    };
    e.name = event_name(field);
    e.span = span;
    Ok(e)
}

struct Pass<'a> {
    c: &'a Component,
    fields: Vec<String>,
    /// the keys of the table being walked
    keys: Vec<String>,
    in_table: bool,
}

fn literal_label(children: &[ViewNode]) -> Option<String> {
    match children {
        [
            ViewNode::Text {
                value: TextValue::Interpolated(parts),
                ..
            },
        ] => match parts.as_slice() {
            [TextPart::Literal(s)] if !s.is_empty() => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

impl Pass<'_> {
    fn node(&mut self, n: &ViewNode) -> Result<ViewNode, LowerError> {
        match n {
            ViewNode::Element {
                kind,
                accessibility,
                bindings,
                children,
                span,
            } => {
                let sort = bindings.iter().find(|b| b.key == "sort");
                let key = bindings.iter().find(|b| b.key == "key");
                if sort.is_some() || key.is_some() {
                    return self.header(kind, accessibility, bindings, children, *span);
                }
                // a table has its own keys; anything else shares the keys of the table around it
                let is_table = matches!(kind, ElementKind::Table);
                let saved = is_table.then(|| (self.in_table, std::mem::take(&mut self.keys)));
                if is_table {
                    self.in_table = true;
                }
                let mut out = Vec::with_capacity(children.len());
                for c in children {
                    out.push(self.node(c)?);
                }
                if let Some((was, keys)) = saved {
                    self.in_table = was;
                    self.keys = keys;
                }
                Ok(ViewNode::Element {
                    kind: kind.clone(),
                    accessibility: accessibility.clone(),
                    bindings: bindings.clone(),
                    children: out,
                    span: *span,
                })
            }
            ViewNode::For {
                over,
                binding,
                children,
                span,
            } => Ok(ViewNode::For {
                over: over.clone(),
                binding: binding.clone(),
                children: children
                    .iter()
                    .map(|c| self.node(c))
                    .collect::<Result<_, _>>()?,
                span: *span,
            }),
            ViewNode::If {
                cond,
                children,
                span,
            } => Ok(ViewNode::If {
                cond: cond.clone(),
                children: children
                    .iter()
                    .map(|c| self.node(c))
                    .collect::<Result<_, _>>()?,
                span: *span,
            }),
            ViewNode::Text { .. } => Ok(n.clone()),
        }
    }

    fn header(
        &mut self,
        kind: &ElementKind,
        accessibility: &Accessibility,
        bindings: &[Binding],
        children: &[ViewNode],
        span: Span,
    ) -> Result<ViewNode, LowerError> {
        let Some(first) = bindings.iter().find(|b| b.key == "sort" || b.key == "key") else {
            return Err(refuse(
                "sort-key-missing",
                "a sortable header has both `sort: field` and `key: \"name\"`".to_string(),
                span,
            ));
        };
        let is_header = matches!(kind, ElementKind::Text)
            && matches!(&accessibility.role, Some(Role::Other(r)) if r == "columnheader");
        if !is_header || !self.in_table {
            return Err(refuse(
                "sort-outside-header",
                "`sort:` and `key:` make a column header sortable: put them on a `Text { role: columnheader .. }` in a `Row` of a `Table`".to_string(),
                first.span,
            ));
        }
        let (Some(sort), Some(key)) = (
            bindings.iter().find(|b| b.key == "sort"),
            bindings.iter().find(|b| b.key == "key"),
        ) else {
            return Err(refuse(
                "sort-key-missing",
                "a sortable header has both `sort: field` and `key: \"name\"`".to_string(),
                first.span,
            ));
        };
        let field = match &sort.value {
            BindingValue::StateRef(f)
                if self
                    .c
                    .state
                    .iter()
                    .any(|s| &s.name == f && s.ty == Type::String) =>
            {
                f.clone()
            }
            _ => {
                return Err(refuse(
                    "sort-not-string",
                    "`sort:` is the name of a `String` state field of this component: `sort: order`".to_string(),
                    sort.span,
                ));
            }
        };
        let k = match &key.value {
            BindingValue::String(k)
                if !k.is_empty()
                    && k.len() <= KEY_MAX
                    && !k.starts_with('-')
                    && k.chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')) =>
            {
                k.clone()
            }
            _ => {
                return Err(refuse(
                    "sort-key-invalid",
                    format!(
                        "`key:` is a text literal of 1 to {KEY_MAX} letters, digits and `_ - .`, not starting with `-` (that is the descending prefix)"
                    ),
                    key.span,
                ));
            }
        };
        if self.keys.contains(&k) {
            return Err(refuse(
                "sort-key-duplicate",
                format!("two headers of one table have the key \"{k}\""),
                key.span,
            ));
        }
        self.keys.push(k.clone());
        let Some(label) = literal_label(children) else {
            return Err(refuse(
                "sort-label-invalid",
                "the text of a sortable header is a plain text literal, with no `{..}`: it is the name of the button".to_string(),
                span,
            ));
        };
        if !self.fields.contains(&field) {
            self.fields.push(field.clone());
        }
        let mut cell_bindings: Vec<Binding> = bindings
            .iter()
            .filter(|b| b.key != "sort" && b.key != "key")
            .cloned()
            .collect();
        let sorted = expr_of(
            &format!(
                "if {field} == \"{k}\" {{ \"ascending\" }} else if {field} == \"-{k}\" {{ \"descending\" }} else {{ \"\" }}"
            ),
            sort.span,
        )?;
        cell_bindings.push(Binding {
            key: "sorted".to_string(),
            value: BindingValue::Prop(sorted),
            span: sort.span,
        });
        let button = ViewNode::Element {
            kind: ElementKind::Button,
            accessibility: Accessibility {
                role: None,
                label: Some(label),
                label_expr: None,
                span: accessibility.span,
            },
            bindings: vec![Binding {
                key: "on".to_string(),
                value: BindingValue::EventCall {
                    name: event_name(&field),
                    args: vec![Expr::StringLit {
                        value: k,
                        span: key.span,
                    }],
                },
                span: sort.span,
            }],
            children: Vec::new(),
            span,
        };
        Ok(ViewNode::Element {
            kind: ElementKind::Text,
            accessibility: accessibility.clone(),
            bindings: cell_bindings,
            children: vec![button],
            span,
        })
    }
}

/// `c` with every sortable header written out, or `c` itself when it has none.
pub fn desugar(c: &Component) -> Result<Cow<'_, Component>, LowerError> {
    if !has_sort(&c.view.body) {
        return Ok(Cow::Borrowed(c));
    }
    let mut pass = Pass {
        c,
        fields: Vec::new(),
        keys: Vec::new(),
        in_table: false,
    };
    let body = pass.node(&c.view.body)?;
    let mut out = c.clone();
    out.view.body = body;
    for f in &pass.fields {
        out.events.push(handler(f, c.view.span)?);
    }
    Ok(Cow::Owned(out))
}
