//! Typed UI IR — the in-memory shape that the web backend and
//! every cancho-ui tool reads. Mirrors `../../../spec/ir-schema/README.md`,
//! restricted to the Tier 2 supported subset (state, requires,
//! events, basic expressions, primitive bindings).

use cancho_ui_syntax::span::Span;

pub const SCHEMA: &str = "cancho-ui-ir";

#[derive(Debug, Clone, PartialEq)]
pub struct IrFile {
    pub schema: String,
    pub version: String,
    pub module: Option<String>,
    pub components: Vec<IrComponent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrComponent {
    pub id: IrId,
    pub name: String,
    pub state: Vec<IrStateField>,
    pub requires: Vec<IrCapabilityRef>,
    pub reachable_effects: Vec<IrReachableEffect>,
    pub events: Vec<IrEventHandler>,
    pub view: IrViewBlock,
    pub span: Span,
}

/// An effect that is reachable from this component's expressions.
/// Populated by the lowering pass; the type checker verifies each
/// one against the component's `requires` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrReachableEffect {
    /// The capability that authorises this effect, e.g. `Clock`,
    /// `Network`. Phase 2 will populate `capability` precisely;
    /// Tier 4 hard-codes `Clock` for `clock.now()`.
    pub capability: String,
    /// The call path, e.g. `["clock", "now"]`. Phase 2 will also
    /// store the foreign symbol it lowers to.
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrStateField {
    pub name: String,
    pub ty: IrType,
    pub initial: IrExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrType {
    Int,
    Float,
    Bool,
    String,
    Result { ok: Box<IrType>, err: Box<IrType> },
    Request { payload: Box<IrType> },
    /// `Option[T]` — Tier 8.
    Option { some: Box<IrType> },
    /// `List[T]` — Tier 8.
    List { elem: Box<IrType> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrCapabilityRef {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrEventHandler {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<IrStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrStmt {
    Assign {
        target: String,
        value: IrExpr,
        span: Span,
    },
    Expr {
        expr: IrExpr,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrExpr {
    IntLit { value: i64, span: Span },
    FloatLit { value: f64, span: Span },
    StringLit { value: String, span: Span },
    BoolLit { value: bool, span: Span },
    Ident { name: String, span: Span },
    /// A builtin call: `path(args)`. The capability that authorises
    /// it lives on the enclosing `IrComponent::reachable_effects`,
    /// not on the call itself, so the same call path can be authorised
    /// by different capabilities in different components.
    Call {
        path: Vec<String>,
        args: Vec<IrExpr>,
        span: Span,
    },
    Binary {
        op: IrBinOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
        span: Span,
    },
    Unary {
        op: IrUnOp,
        operand: Box<IrExpr>,
        span: Span,
    },
    /// `match scrutinee { arms }`. Tier 6. Each arm's pattern is
    /// matched against the scrutinee at runtime; the first match
    /// wins and its body is evaluated. Patterns are kept as a
    /// separate IR enum so the type checker can reason about
    /// exhaustiveness and the JS emitter can desugar to JS.
    Match {
        scrutinee: Box<IrExpr>,
        arms: Vec<IrMatchArm>,
        span: Span,
    },
    /// `Ok(value)` — a `Result[T, E]` constructor with payload.
    /// The runtime representation is a discriminated pair:
    /// `{ __cui_tag: "Ok", value: <v> }`.
    Ok { value: Box<IrExpr>, span: Span },
    /// `Err(reason)` — see `Ok`.
    Err { reason: Box<IrExpr>, span: Span },
    /// `await(r)` — Tier 7. Awaits a `Request[T]`; the lowering
    /// lowers it to a JS Promise chain that resolves to a
    /// `RequestResult[T]` (`Ok | Err | Cancelled`).
    Await { request: Box<IrExpr>, span: Span },
    /// `Cancelled` — Tier 7. The third variant of a
    /// `RequestResult[T]`. The runtime produces it when a request
    /// is dropped; user code matches against it.
    Cancelled { span: Span },
    /// `Some(value)` — Tier 8. `Option[T]` value constructor;
    /// `{__cui_tag: "Some", value: <v>}`.
    Some { value: Box<IrExpr>, span: Span },
    /// `None` — Tier 8. The zero-payload `Option[T]`
    /// constructor; `{__cui_tag: "None"}`.
    None { span: Span },
    /// `[a, b, c]` — Tier 8. A `List[T]` literal; a JS array.
    ListLit { items: Vec<IrExpr>, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrMatchArm {
    pub pattern: IrPattern,
    pub body: IrExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrPattern {
    Wildcard { span: Span },
    Ident { name: String, span: Span },
    Variant { name: String, binding: String, span: Span },
    UnitVariant { name: String, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOp {
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

impl IrBinOp {
    pub fn js_op(&self) -> &'static str {
        match self {
            IrBinOp::Add => "+",
            IrBinOp::Sub => "-",
            IrBinOp::Mul => "*",
            IrBinOp::Div => "/",
            IrBinOp::Eq => "===",
            IrBinOp::Ne => "!==",
            IrBinOp::Lt => "<",
            IrBinOp::Le => "<=",
            IrBinOp::Gt => ">",
            IrBinOp::Ge => ">=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrUnOp {
    Neg,
}

/// A stable identifier derived from the declaration's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct IrViewBlock {
    pub body: IrViewNode,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrViewNode {
    Element {
        kind: IrElementKind,
        accessibility: IrAccessibility,
        bindings: Vec<IrBinding>,
        children: Vec<IrViewNode>,
        span: Span,
    },
    Text {
        parts: Vec<IrTextPart>,
        span: Span,
    },
    /// `For { over: list as: item ... }` — Tier 8. Renders
    /// `children` once per element of the `over` expression
    /// (normally an Ident referring to a `List[T]` state field).
    /// Inside the children, `binding` reads the current element.
    For {
        over: IrExpr,
        binding: String,
        children: Vec<IrViewNode>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrTextPart {
    Literal(String),
    Expr(IrExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrElementKind {
    Column,
    Row,
    Text,
    Button,
    TextField,
    Named(String),
}

impl IrElementKind {
    pub fn tag_name(&self) -> &'static str {
        match self {
            IrElementKind::Column => "div",
            IrElementKind::Row => "div",
            IrElementKind::Text => "span",
            IrElementKind::Button => "button",
            IrElementKind::TextField => "input",
            IrElementKind::Named(_) => "div",
        }
    }

    pub fn layout_class(&self) -> Option<&'static str> {
        match self {
            IrElementKind::Column => Some("cui-col"),
            IrElementKind::Row => Some("cui-row"),
            _ => None,
        }
    }

    pub fn is_interactive(&self) -> bool {
        matches!(self, IrElementKind::Button)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrAccessibility {
    pub role: Option<IrRole>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrRole {
    Text,
    Heading,
    Button,
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrBinding {
    pub key: String,
    pub value: IrBindingValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrBindingValue {
    String(String),
    EventRef(String),
    Role(IrRole),
    /// `value: <state-field>` — a one-way binding to a state field.
    /// Tier 5: used for `TextField` so the value reflects the
    /// current state on every re-render.
    StateRef(String),
}