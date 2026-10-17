//! AST types for cancho-ui.
//!
//! Phase 1 supports:
//!   - `module foo;` declarations,
//!   - `component Name { ... }` with `state`, `requires`, `on`, `view`,
//!   - element / text view nodes with `role:` / `label:` accessibility
//!     blocks,
//!   - typed state fields (`Int`, `Float`, `Bool`, `String`),
//!   - event handlers with simple expression bodies
//!     (literals, identifiers, arithmetic, string interpolation),
//!   - the four built-in primitives (`Column`, `Row`, `Text`, `Button`)
//!     and any user-declared PascalCase component.
//!
//! Phase 2 extends with `match`, `await`, `foreign`, `target`,
//! `List[T]`, `Option[T]`, `Result[T, E]`, `Request[T]`,
//! `Event(...)`, `Capability(...)`.

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
    pub state: Vec<StateField>,
    pub requires: Vec<CapabilityRef>,
    pub events: Vec<EventHandler>,
    pub view: ViewBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateField {
    pub name: String,
    pub ty: Type,
    pub initial: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityRef {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventHandler {
    pub name: String,
    pub params: Vec<String>,
    pub body: StmtBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
}

impl Type {
    pub fn from_ident(s: &str) -> Option<Type> {
        match s {
            "Int" => Some(Type::Int),
            "Float" => Some(Type::Float),
            "Bool" => Some(Type::Bool),
            "String" => Some(Type::String),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewBlock {
    pub body: ViewNode,
    pub span: Span,
}

impl ViewBlock {
    /// Returns true if the view body has no children / no text.
    pub fn body_is_empty(&self) -> bool {
        match &self.body {
            ViewNode::Element { children, .. } => children.is_empty(),
            ViewNode::Text { value, .. } => match value {
                TextValue::Interpolated(parts) => {
                    parts.is_empty()
                        || parts
                            .iter()
                            .all(|p| matches!(p, TextPart::Literal(s) if s.is_empty()))
                }
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewNode {
    Element {
        kind: ElementKind,
        accessibility: Accessibility,
        bindings: Vec<Binding>,
        children: Vec<ViewNode>,
        span: Span,
    },
    Text {
        value: TextValue,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextValue {
    /// A string literal, optionally with `{expr}` interpolations.
    Interpolated(Vec<TextPart>),
}

impl TextValue {
    pub fn literal(s: String) -> Self {
        TextValue::Interpolated(vec![TextPart::Literal(s)])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextPart {
    Literal(String),
    Expr { expr: Expr, span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementKind {
    Column,
    Row,
    Text,
    Button,
    Named(String),
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
    Heading,
    Button,
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub key: String,
    pub value: BindingValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingValue {
    /// `label: "Increment"` etc.
    String(String),
    /// `on: Increment` — reference to an event declared in the
    /// enclosing component.
    EventRef(String),
    /// `role: heading` etc.
    Role(Role),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StmtBlock {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `x = expr;` — assignment to a state field.
    Assign { target: String, value: Expr, span: Span },
    /// `expr;` — expression statement (e.g. `net.search(query);`).
    Expr { expr: Expr, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit { value: i64, span: Span },
    FloatLit { value: f64, span: Span },
    StringLit { value: String, span: Span },
    BoolLit { value: bool, span: Span },
    /// A bare identifier — references a state field (in handlers)
    /// or interpolates into text.
    Ident { name: String, span: Span },
    /// A builtin call: `path(args)` where `path` is one or more
    /// identifiers separated by `.` (e.g. `clock.now()`). Phase 2+
    /// will generalize this to user-defined functions, but the AST
    /// shape already accommodates that.
    Call {
        path: Vec<String>,
        args: Vec<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn js_op(&self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Eq => "===",
            BinOp::Ne => "!==",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
}