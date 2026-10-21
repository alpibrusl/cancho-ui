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
    /// Tier 16: `import <module>.<Component>;` declarations, in
    /// source order. Resolution to the sibling `<module>.cui` file
    /// happens in the CLI pipeline, not here.
    pub imports: Vec<ImportDecl>,
    pub components: Vec<Component>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub name: String,
    pub span: Span,
}

/// Tier 16: `import cards.HelloCard;` — imports exactly one
/// component from the sibling module file `cards.cui`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub module: String,
    pub component: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub state: Vec<StateField>,
    pub requires: Vec<CapabilityRef>,
    /// `start Event;` and `every <ms> Event;` (docs/schedules.md), in the order written.
    pub schedules: Vec<Schedule>,
    pub events: Vec<EventHandler>,
    pub view: ViewBlock,
    pub span: Span,
}

/// When an event runs without the user: once at mount, or on a period (docs/schedules.md section 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleKind {
    Start,
    /// The period in milliseconds, as written (its range is checked in lowering).
    Every(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    pub kind: ScheduleKind,
    pub event: String,
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
    /// `requires Network("a.example", "b.example:8443")`: the string-literal bounds, in order (docs/adapters.md 10).
    pub bounds: Vec<String>,
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
    /// `Result[T, E]` — Tier 6. Two constructors: `Ok(value)` and
    /// `Err(reason)`. The `match` on a `Result` must have both
    /// arms; missing one is the `match-non-exhaustive` refusal.
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },
    /// `Request[T]` — Tier 7. The payload of an asynchronous
    /// effect call. `await(r)` resolves it to a
    /// `RequestResult[T]` (`Ok | Err | Cancelled`).
    Request {
        payload: Box<Type>,
    },
    /// `Option[T]` — Tier 8. Two constructors: `Some(value)` and
    /// `None`.
    Option {
        some: Box<Type>,
    },
    /// `List[T]` — Tier 8. A list literal is `[a, b, c]`; the
    /// view renders it with `For { over: list as: x ... }`.
    List {
        elem: Box<Type>,
    },
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
            // `For` is never empty by definition — it repeats its
            // children at least once per list item.
            ViewNode::For { .. } => false,
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
    /// `For { over: list as: item ... }` — Tier 8. Renders the
    /// children once per element of the `over` list; inside the
    /// children, `item` is a read of the current element. Since
    /// Tier 11, `over` is a general (pure) expression: a state
    /// list or a derived one — `filter(results, n => ...)`.
    For {
        over: Expr,
        binding: String,
        children: Vec<ViewNode>,
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
    /// A table (docs/presentation.md section 4): its `Row`s are rows, the `Text`s in them cells.
    Table,
    Text,
    Button,
    /// An editable text input. Tier 5 primitive.
    TextField,
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Accessibility {
    pub role: Option<Role>,
    pub label: Option<String>,
    /// `label: m` / `label: "Pick " + m` (docs/row-actions.md): a label that is an expression, evaluated at each render.
    /// A lone string literal is `label` instead, so a program with literal labels has none of these.
    pub label_expr: Option<Expr>,
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
    /// `on: Pick(m)` — an event reference with arguments, evaluated at render time (docs/row-actions.md): a row of a
    /// `For` can name the element it is for.
    EventCall { name: String, args: Vec<Expr> },
    /// `role: heading` etc.
    Role(Role),
    /// `value: query` — reference to a state field. Tier 5
    /// supports this for `TextField`; other primitives may use it
    /// in Phase 2.
    StateRef(String),
    /// `title: 5` / `enabled: true` — Tier 13. A *property* on a
    /// component instance: a constant initial value for one of
    /// the child's state fields. String literals parse as
    /// `String(s)` and are interpreted as string properties at
    /// the shape pass (the only realistic use of a string
    /// binding on an opaque instance); the other three literal
    /// kinds parse as `Prop` directly.
    Prop(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StmtBlock {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `x = expr;` — assignment to a state field.
    Assign {
        target: String,
        value: Expr,
        span: Span,
    },
    /// `expr;` — expression statement (e.g. `net.search(query);`).
    Expr { expr: Expr, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit {
        value: i64,
        span: Span,
    },
    FloatLit {
        value: f64,
        span: Span,
    },
    StringLit {
        value: String,
        span: Span,
    },
    BoolLit {
        value: bool,
        span: Span,
    },
    /// A bare identifier — references a state field (in handlers)
    /// or interpolates into text.
    Ident {
        name: String,
        span: Span,
    },
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
    /// `match scrutinee { arms }`. Tier 6. Each arm has a pattern
    /// and a body; the body is a single expression for now. The
    /// arms' patterns must be exhaustive over the scrutinee's
    /// type (the `match-non-exhaustive` refusal); the first
    /// matching arm is taken.
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `Ok(value)` — constructs the `Ok` variant of `Result[T, E]`.
    /// The type of `value` is `T`; the surrounding context must
    /// determine `E` (Phase 2 will thread it through type
    /// inference). Tier 6: the constructed IR uses a discriminated
    /// pair of `kind` (`"Ok"` or `"Err"`) and `value`.
    Ok {
        value: Box<Expr>,
        span: Span,
    },
    /// `Err(reason)` — see `Ok` for the type-inference caveat.
    Err {
        reason: Box<Expr>,
        span: Span,
    },
    /// `await(expr)` — Tier 7. Awaits a `Request[T]` and resolves
    /// to a `RequestResult[T]`. The runtime representation is a
    /// Promise-based chain.
    Await {
        request: Box<Expr>,
        span: Span,
    },
    /// `Cancelled` — Tier 7. The third variant of a
    /// `RequestResult[T]`. Constructed only by the runtime when a
    /// request is dropped; users do not write `Cancelled(...)`
    /// (the constructor takes no payload).
    Cancelled {
        span: Span,
    },
    /// `Some(value)` — Tier 8. `Option[T]` value constructor.
    Some {
        value: Box<Expr>,
        span: Span,
    },
    /// `None` — Tier 8. The zero-payload `Option[T]` constructor.
    None {
        span: Span,
    },
    /// `[a, b, c]` — Tier 8. A `List[T]` literal.
    ListLit {
        items: Vec<Expr>,
        span: Span,
    },
    /// `x => body` — Tier 11. A single-parameter lambda. It is
    /// accepted *syntactically* anywhere an argument could go, but
    /// the lowering refuses it everywhere except as the second
    /// argument of `filter(list, x => ...)` / `map(list, x => ...)`
    /// (`lambda-unsupported`): Tier 11 has no first-class function
    /// values, so a lambda never outlives the builtin call it
    /// belongs to.
    Lambda {
        param: String,
        body: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` — matches anything, binds nothing.
    Wildcard { span: Span },
    /// `name` — matches anything, binds the matched value to `name`.
    /// Tier 6 only: a name that *is* a variant name like `Ok` or
    /// `Err` is still a binding; the constructor pattern below is
    /// the way to destructure.
    Ident { name: String, span: Span },
    /// `Name(binding)` — matches the variant `Name` with one
    /// payload, binds the payload to `binding`. Tier 6 supports
    /// only single-payload variants (`Ok(v)`, `Err(e)`,
    /// `Some(v)`).
    Variant {
        name: String,
        binding: String,
        span: Span,
    },
    /// `Name` — matches the zero-payload variant `Name`. Tier 7
    /// uses this for `Cancelled` (which carries no payload).
    /// Disambiguated from `Ident` by the fact that a constructor
    /// pattern has no `(binding)`; the parser must already know
    /// whether the name is a unit constructor or an identifier.
    UnitVariant { name: String, span: Span },
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard { span } => *span,
            Pattern::Ident { span, .. } => *span,
            Pattern::Variant { span, .. } => *span,
            Pattern::UnitVariant { span, .. } => *span,
        }
    }
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::Ident { span, .. }
            | Expr::Call { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Match { span, .. }
            | Expr::Ok { span, .. }
            | Expr::Err { span, .. }
            | Expr::Await { span, .. }
            | Expr::Cancelled { span, .. }
            | Expr::Some { span, .. }
            | Expr::None { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::Lambda { span, .. } => *span,
        }
    }
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
