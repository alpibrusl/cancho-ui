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
    /// `start` and `every` declarations (docs/schedules.md), in the order written.
    pub schedules: Vec<IrSchedule>,
    pub reachable_effects: Vec<IrReachableEffect>,
    pub events: Vec<IrEventHandler>,
    pub view: IrViewBlock,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrScheduleKind {
    Start,
    /// The period in milliseconds, 100 to 86,400,000.
    Every(u32),
}

/// An event that runs without the user (docs/schedules.md): the event exists and takes no parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct IrSchedule {
    pub kind: IrScheduleKind,
    pub event: String,
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
    /// The first argument when it is a string literal (`net.get("https://a.example/x")`): what the static
    /// bound check reads. Not part of the IR JSON.
    pub literal_arg: Option<String>,
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
    Result {
        ok: Box<IrType>,
        err: Box<IrType>,
    },
    Request {
        payload: Box<IrType>,
    },
    /// `Option[T]` — Tier 8.
    Option {
        some: Box<IrType>,
    },
    /// `List[T]` — Tier 8.
    List {
        elem: Box<IrType>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrCapabilityRef {
    pub name: String,
    /// The bounds written after the name (`Network("a.example")`); empty for a bare capability.
    pub bounds: Vec<String>,
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
    Ident {
        name: String,
        span: Span,
    },
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
    /// `if cond { then } else { else_ }` (docs/conditionals.md). Pure: lowering refuses an effect or an `await` in the
    /// condition or either branch (`if-effect-in-expression`), and `else` is required (`if-else-required`). Only the
    /// chosen branch is evaluated.
    If {
        cond: Box<IrExpr>,
        then: Box<IrExpr>,
        else_: Box<IrExpr>,
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
    Ok {
        value: Box<IrExpr>,
        span: Span,
    },
    /// `Err(reason)` — see `Ok`.
    Err {
        reason: Box<IrExpr>,
        span: Span,
    },
    /// `await(r)` — Tier 7. Awaits a `Request[T]`; the lowering
    /// lowers it to a JS Promise chain that resolves to a
    /// `RequestResult[T]` (`Ok | Err | Cancelled`).
    Await {
        request: Box<IrExpr>,
        span: Span,
    },
    /// `Cancelled` — Tier 7. The third variant of a
    /// `RequestResult[T]`. The runtime produces it when a request
    /// is dropped; user code matches against it.
    Cancelled {
        span: Span,
    },
    /// `Some(value)` — Tier 8. `Option[T]` value constructor;
    /// `{__cui_tag: "Some", value: <v>}`.
    Some {
        value: Box<IrExpr>,
        span: Span,
    },
    /// `None` — Tier 8. The zero-payload `Option[T]`
    /// constructor; `{__cui_tag: "None"}`.
    None {
        span: Span,
    },
    /// `[a, b, c]` — Tier 8. A `List[T]` literal; a JS array.
    ListLit {
        items: Vec<IrExpr>,
        span: Span,
    },
    /// `filter(list, x => cond)` — Tier 11. Keeps the elements
    /// for which `cond` (with `x` bound) is truthy; a JS
    /// `Array.prototype.filter` with an inline callback. Pure —
    /// no capability.
    Filter {
        list: Box<IrExpr>,
        item: String,
        cond: Box<IrExpr>,
        span: Span,
    },
    /// `map(list, x => body)` — Tier 11. Maps each element
    /// through `body` (with `x` bound); a JS
    /// `Array.prototype.map` with an inline callback.
    Map {
        list: Box<IrExpr>,
        item: String,
        body: Box<IrExpr>,
        span: Span,
    },
    /// `sort(list, x => key)` — docs/lists.md. A new list, stable and ascending by `key` (with `x` bound):
    /// numbers before strings, numbers numerically, strings by UTF-16 code units. Pure.
    SortBy {
        list: Box<IrExpr>,
        item: String,
        key: Box<IrExpr>,
        span: Span,
    },
    /// `len(list)` — Tier 11. Element count of a list (or
    /// character count of a string, the same JS `.length`).
    /// Pure — no capability.
    Len {
        list: Box<IrExpr>,
        span: Span,
    },
}

impl IrExpr {
    pub fn span(&self) -> Span {
        match self {
            IrExpr::IntLit { span, .. }
            | IrExpr::FloatLit { span, .. }
            | IrExpr::StringLit { span, .. }
            | IrExpr::BoolLit { span, .. }
            | IrExpr::Ident { span, .. }
            | IrExpr::Call { span, .. }
            | IrExpr::Binary { span, .. }
            | IrExpr::Unary { span, .. }
            | IrExpr::If { span, .. }
            | IrExpr::Match { span, .. }
            | IrExpr::Ok { span, .. }
            | IrExpr::Err { span, .. }
            | IrExpr::Await { span, .. }
            | IrExpr::Cancelled { span }
            | IrExpr::Some { span, .. }
            | IrExpr::None { span }
            | IrExpr::ListLit { span, .. }
            | IrExpr::Filter { span, .. }
            | IrExpr::Map { span, .. }
            | IrExpr::SortBy { span, .. }
            | IrExpr::Len { span, .. } => *span,
        }
    }
}

impl IrExpr {
    /// The direct sub-expressions, in source order. One exhaustive `match` with no wildcard: a new `IrExpr` variant
    /// is a compile error here, so a walk built on `children` cannot silently skip it (the `match-non-exhaustive` walk
    /// once missed `len`, `filter`, `map` and `sort`, #63).
    pub fn children(&self) -> Vec<&IrExpr> {
        match self {
            IrExpr::IntLit { .. }
            | IrExpr::FloatLit { .. }
            | IrExpr::StringLit { .. }
            | IrExpr::BoolLit { .. }
            | IrExpr::Ident { .. }
            | IrExpr::Cancelled { .. }
            | IrExpr::None { .. } => Vec::new(),
            IrExpr::Call { args, .. } => args.iter().collect(),
            IrExpr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
            IrExpr::Unary { operand, .. } => vec![operand],
            IrExpr::If {
                cond, then, else_, ..
            } => vec![cond, then, else_],
            IrExpr::Match {
                scrutinee, arms, ..
            } => {
                let mut out: Vec<&IrExpr> = vec![scrutinee];
                out.extend(arms.iter().map(|a| &a.body));
                out
            }
            IrExpr::Ok { value, .. } | IrExpr::Some { value, .. } => vec![value],
            IrExpr::Err { reason, .. } => vec![reason],
            IrExpr::Await { request, .. } => vec![request],
            IrExpr::ListLit { items, .. } => items.iter().collect(),
            IrExpr::Filter { list, cond, .. } => vec![list, cond],
            IrExpr::Map { list, body, .. } => vec![list, body],
            IrExpr::SortBy { list, key, .. } => vec![list, key],
            IrExpr::Len { list, .. } => vec![list],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrMatchArm {
    pub pattern: IrPattern,
    pub body: IrExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrPattern {
    Wildcard {
        span: Span,
    },
    Ident {
        name: String,
        span: Span,
    },
    Variant {
        name: String,
        binding: String,
        span: Span,
    },
    UnitVariant {
        name: String,
        span: Span,
    },
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
    And,
    Or,
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
            IrBinOp::And => "&&",
            IrBinOp::Or => "||",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrUnOp {
    Neg,
    Not,
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
    /// `If { when: cond ... }` (docs/view-conditional.md): `children` are in the page while the pure `Bool` expression
    /// `cond` is true, and absent (not hidden) while it is false.
    If {
        cond: IrExpr,
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
    /// `Table { .. }` (docs/presentation.md section 4) and what lowering makes of its descendants: a `Row` of a table is
    /// a `TableRow`, a `Text` of that row a `Cell` (a `HeaderCell` with `role: columnheader`), and any other element of
    /// it is wrapped in a `Cell`.
    Table,
    TableRow,
    Cell,
    HeaderCell,
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
            IrElementKind::Table => "table",
            IrElementKind::TableRow => "tr",
            IrElementKind::Cell => "td",
            IrElementKind::HeaderCell => "th",
            IrElementKind::Named(_) => "div",
        }
    }

    /// The elements whose children are text parts rather than elements.
    pub fn holds_text(&self) -> bool {
        matches!(
            self,
            IrElementKind::Text | IrElementKind::Cell | IrElementKind::HeaderCell
        )
    }

    pub fn layout_class(&self) -> Option<&'static str> {
        match self {
            IrElementKind::Column => Some("cui-col"),
            IrElementKind::Row => Some("cui-row"),
            IrElementKind::Table => Some("cui-table"),
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
    /// A label that is an expression (docs/row-actions.md); `label` is `None` then.
    pub label_expr: Option<Box<IrExpr>>,
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
    /// `on: Pick(m)`: the event and the arguments it is given, evaluated at render time (docs/row-actions.md).
    EventCall {
        name: String,
        args: Vec<IrExpr>,
    },
    Role(IrRole),
    /// `value: <state-field>` — a one-way binding to a state field.
    /// Tier 5: used for `TextField` so the value reflects the
    /// current state on every re-render.
    StateRef(String),
    /// Tier 13: a component-instance property — a constant
    /// initial value for one of the child's state fields. Also
    /// carries the lowered form of `String(...)` binding values
    /// when they appear on an instance (string properties).
    Const {
        expr: Box<IrExpr>,
        span: Span,
    },
}
