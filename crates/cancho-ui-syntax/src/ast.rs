//! AST types for the Tier 1 subset of cancho-ui.
//!
//! Every node carries a `Span` so that later phases can produce
//! source-mapped diagnostics. The shape follows
//! `../../../spec/grammar/README.md` and the IR sketch in
//! `../../../spec/ir-schema/README.md`, but is deliberately a
//! strict subset for the first vertical slice.

use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub module: Option<ModuleDecl>,
    pub components: Vec<Component>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub view: ViewBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewBlock {
    pub body: ViewNode,
    pub span: Span,
}

impl ViewBlock {
    /// Returns true if the view body has no children / no text.
    /// Used by the type checker to refuse an empty view.
    pub fn body_is_empty(&self) -> bool {
        match &self.body {
            ViewNode::Element { children, .. } => children.is_empty(),
            ViewNode::Text { value, .. } => match value {
                TextValue::Literal(s) => s.is_empty(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewNode {
    Element {
        kind: ElementKind,
        accessibility: Accessibility,
        children: Vec<ViewNode>,
        span: Span,
    },
    Text {
        value: TextValue,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementKind {
    /// Built-in primitive kinds for the Tier 1 supported subset.
    /// Phase 2+ extends this with `TextField`, `Button`, `List`, etc.
    Column,
    Row,
    Text,
    /// Any user-declared component by name (PascalCase).
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextValue {
    Literal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Accessibility {
    pub role: Option<Role>,
    pub label: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Text,
    /// A catch-all for `role: <identifier>` whose name is not one of
    /// the Phase 1 built-in roles. Phase 2 enumerates them.
    Other(String),
}