//! The types of `if`, `&&`, `||` and `!` (docs/conditionals.md section 3.1 and 3.4): the condition of an `if` and the
//! operands of the three operators must be a `Bool`, and the two branches of an `if` must have the same type.
//!
//! The language has no general expression typing: replay refuses an ill-typed operator when it runs
//! (`replay-type-error`), and the page does whatever JavaScript does. This pass types an expression only as far as it is
//! evident from the source (a literal, a state field's declared type, a comparison, a builtin whose answer is fixed) and
//! says `Unknown` for the rest (an event parameter, a `For` or lambda binding, a `match` arm, a `Result`). It refuses only
//! when a type is known and wrong, so it never refuses a program that is right; what it cannot see, replay still
//! refuses when the program runs (and the page's `&&` would then answer an operand, not a `Bool`).

use crate::ir::*;
use crate::lower::LowerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Num,
    Str,
    List,
    /// A type that is known and is none of the four above (`Result`, `Option`, `Request`).
    Other,
    Unknown,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Bool => "a Bool",
            Kind::Num => "a number",
            Kind::Str => "a string",
            Kind::List => "a list",
            Kind::Other => "a Result, Option or Request",
            Kind::Unknown => "a value of unknown type",
        }
    }

    /// One of the four types two branches are compared by.
    fn comparable(self) -> bool {
        matches!(self, Kind::Bool | Kind::Num | Kind::Str | Kind::List)
    }
}

fn kind_of_type(t: &IrType) -> Kind {
    match t {
        IrType::Int | IrType::Float => Kind::Num,
        IrType::Bool => Kind::Bool,
        IrType::String => Kind::Str,
        IrType::List { .. } => Kind::List,
        IrType::Result { .. } | IrType::Request { .. } | IrType::Option { .. } => Kind::Other,
    }
}

/// The answer of a builtin call whose type is fixed; `Unknown` for the rest.
fn kind_of_call(path: &[String]) -> Kind {
    let name = path.join(".");
    match name.as_str() {
        "contains" | "starts_with" | "ends_with" => Kind::Bool,
        "min" | "max" | "abs" | "floor" | "ceil" | "round" | "num" | "distance" => Kind::Num,
        "lower" | "fold" | "upper" | "trim" | "join" | "json.quote" | "json.get"
        | "storage.get" | "t" | "fmt.number" | "fmt.percent" | "fmt.date" | "fmt.time"
        | "locale.current" => Kind::Str,
        "locale.set" => Kind::Bool,
        "lines" | "split" | "take" | "skip" | "reverse" | "concat" | "move" | "json.items" => {
            Kind::List
        }
        _ => Kind::Unknown,
    }
}

struct Scope<'a> {
    state: &'a [IrStateField],
    locals: Vec<String>,
}

impl Scope<'_> {
    /// The type of a name, or the refusal `name-unknown` (alpibrusl/cancho-ui#55) when it is neither a local (an event
    /// parameter, a `For` binding, a lambda parameter, a `match` binding) nor a state field.
    fn ident(&self, name: &str, span: cancho_ui_syntax::span::Span) -> Result<Kind, LowerError> {
        if self.locals.iter().any(|l| l == name) {
            return Ok(Kind::Unknown);
        }
        match self.state.iter().find(|s| s.name == name) {
            Some(s) => Ok(kind_of_type(&s.ty)),
            None => {
                let mut known: Vec<&str> = self.state.iter().map(|s| s.name.as_str()).collect();
                known.extend(self.locals.iter().map(|l| l.as_str()));
                known.sort_unstable();
                known.dedup();
                Err(LowerError::ConditionRefused {
                    code: "name-unknown",
                    message: format!(
                        "`{name}` is neither a state field nor a binding in scope here (in scope: {})",
                        if known.is_empty() {
                            "nothing".to_string()
                        } else {
                            known.join(", ")
                        }
                    ),
                    span,
                })
            }
        }
    }
}

pub fn check_conditions(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
        let mut scope = Scope {
            state: &c.state,
            locals: Vec::new(),
        };
        for e in &c.events {
            scope.locals = e.params.clone();
            stmts(&mut scope, &e.body)?;
        }
        scope.locals.clear();
        node(&mut scope, &c.view.body)?;
    }
    Ok(())
}

/// A block's statements, checked with no parameters in scope (docs/handler-sequences.md section 4).
pub fn check_block(state: &[IrStateField], body: &[IrStmt]) -> Result<(), LowerError> {
    let mut scope = Scope {
        state,
        locals: Vec::new(),
    };
    stmts(&mut scope, body)
}

fn stmts(scope: &mut Scope, body: &[IrStmt]) -> Result<(), LowerError> {
    for s in body {
        match s {
            IrStmt::Assign { value, .. } => {
                kind(scope, value)?;
            }
            IrStmt::Expr { expr, .. } => {
                kind(scope, expr)?;
            }
            IrStmt::If {
                cond, then, else_, ..
            } => {
                want_bool(scope, cond, "the condition of an `if` statement")?;
                stmts(scope, then)?;
                stmts(scope, else_)?;
            }
        }
    }
    Ok(())
}

fn node(scope: &mut Scope, n: &IrViewNode) -> Result<(), LowerError> {
    match n {
        IrViewNode::Element {
            accessibility,
            bindings,
            children,
            ..
        } => {
            if let Some(l) = &accessibility.label_expr {
                kind(scope, l)?;
            }
            for b in bindings {
                match &b.value {
                    IrBindingValue::EventCall { args, .. } => {
                        for a in args {
                            kind(scope, a)?;
                        }
                    }
                    IrBindingValue::Const { expr, .. } => {
                        kind(scope, expr)?;
                    }
                    _ => {}
                }
            }
            for ch in children {
                node(scope, ch)?;
            }
        }
        IrViewNode::Text { parts, .. } => {
            for p in parts {
                if let IrTextPart::Expr(e) = p {
                    kind(scope, e)?;
                }
            }
        }
        IrViewNode::For {
            over,
            binding,
            children,
            ..
        } => {
            kind(scope, over)?;
            scope.locals.push(binding.clone());
            for ch in children {
                node(scope, ch)?;
            }
            scope.locals.pop();
        }
        IrViewNode::If { cond, children, .. } => {
            want_bool(scope, cond, "the `when:` of an `If`")?;
            for ch in children {
                node(scope, ch)?;
            }
        }
    }
    Ok(())
}

/// `==`, `!=`, `<`, `<=`, `>`, `>=` (docs/comparisons.md): replay defines `==` and `!=` on two numbers, two strings or two
/// bools (a number against a string is simply not equal, never an error) and refuses a list or a variant; it orders two
/// numbers or two strings and nothing else. The page does JavaScript's: strict equality (so a comparison across types is
/// always false), reference equality on lists, and a coercing `<` (`"7" <= 7` is true). So a comparison whose operand types
/// are both known and are not one of those pairs is refused, whichever evaluator would have been right.
fn comparison(
    op: IrBinOp,
    l: Kind,
    r: Kind,
    span: cancho_ui_syntax::span::Span,
) -> Result<(), LowerError> {
    if l == Kind::Unknown || r == Kind::Unknown {
        return Ok(());
    }
    let (sym, equality) = match op {
        IrBinOp::Eq => ("==", true),
        IrBinOp::Ne => ("!=", true),
        IrBinOp::Lt => ("<", false),
        IrBinOp::Le => ("<=", false),
        IrBinOp::Gt => (">", false),
        _ => (">=", false),
    };
    let scalar = |k: Kind| matches!(k, Kind::Bool | Kind::Num | Kind::Str);
    let ok = if equality {
        scalar(l) && l == r
    } else {
        l == r && matches!(l, Kind::Num | Kind::Str)
    };
    if ok {
        return Ok(());
    }
    let hint = match (l, r) {
        (Kind::Str, Kind::Num) | (Kind::Num, Kind::Str) => {
            "; to compare a text with a number, turn the text into a number with `num(text, default)` (`num(json.get(reply, \"status\", \"\"), 0) == 200`), or compare it with a string literal (`json.get(reply, \"status\", \"\") == \"200\"`)"
        }
        _ if l == Kind::List || r == Kind::List => {
            "; lists are not compared with `==`: compare `len(a) == len(b)`, or `join(a, \",\") == join(b, \",\")`"
        }
        _ if l == Kind::Other || r == Kind::Other => {
            "; a `Result`, `Option` or request is taken apart with `match`, not compared"
        }
        _ if !equality => "; `<`, `<=`, `>` and `>=` order two numbers or two strings",
        _ => "",
    };
    Err(LowerError::ConditionRefused {
        code: "comparison-type-mismatch",
        message: format!(
            "`{sym}` is not defined on {} and {}{hint}",
            l.name(),
            r.name()
        ),
        span,
    })
}

fn want_bool(scope: &mut Scope, e: &IrExpr, what: &str) -> Result<(), LowerError> {
    let k = kind(scope, e)?;
    if matches!(k, Kind::Num | Kind::Str | Kind::List | Kind::Other) {
        return Err(LowerError::ConditionRefused {
            code: "condition-not-bool",
            message: format!(
                "{what} must be a Bool, and this is {}; write a comparison (`n > 0`, `len(s) > 0`) or use a Bool state field",
                k.name()
            ),
            span: e.span(),
        });
    }
    Ok(())
}

/// The type of `e`, refusing on the way any `if`, `&&`, `||` or `!` whose operands are known to be wrong.
fn kind(scope: &mut Scope, e: &IrExpr) -> Result<Kind, LowerError> {
    Ok(match e {
        IrExpr::IntLit { .. } | IrExpr::FloatLit { .. } => Kind::Num,
        IrExpr::StringLit { .. } => Kind::Str,
        IrExpr::BoolLit { .. } => Kind::Bool,
        IrExpr::Ident { name, span } => scope.ident(name, *span)?,
        IrExpr::Call { path, args, .. } => {
            for a in args {
                kind(scope, a)?;
            }
            kind_of_call(path)
        }
        IrExpr::Binary { op, lhs, rhs, .. } => match op {
            IrBinOp::And | IrBinOp::Or => {
                let name = if *op == IrBinOp::And { "`&&`" } else { "`||`" };
                want_bool(scope, lhs, &format!("the left operand of {name}"))?;
                want_bool(scope, rhs, &format!("the right operand of {name}"))?;
                Kind::Bool
            }
            _ => {
                let l = kind(scope, lhs)?;
                let r = kind(scope, rhs)?;
                match op {
                    IrBinOp::Add => {
                        // `+` is for two numbers, or a string and a number, bool or string. A list, a Result, an
                        // Option or a Request is never an operand (JavaScript would turn a list into text), and a
                        // number plus a bool is not arithmetic here (docs/lists.md section 9, #39).
                        let bad = |k: Kind| matches!(k, Kind::List | Kind::Other);
                        let bool_with_number = matches!(
                            (l, r),
                            (Kind::Bool, Kind::Num | Kind::Bool) | (Kind::Num, Kind::Bool)
                        );
                        if bad(l) || bad(r) || bool_with_number {
                            return Err(LowerError::ConditionRefused {
                                code: "operand-type-mismatch",
                                message: format!(
                                    "`+` joins two numbers or a string with a value, and this is {} plus {}{}",
                                    l.name(),
                                    r.name(),
                                    if l == Kind::List && r == Kind::List {
                                        "; to join two lists write `concat(a, b)`"
                                    } else {
                                        ""
                                    }
                                ),
                                span: e.span(),
                            });
                        }
                        match (l, r) {
                            (Kind::Num, Kind::Num) => Kind::Num,
                            (Kind::Str, _) | (_, Kind::Str) => Kind::Str,
                            _ => Kind::Unknown,
                        }
                    }
                    IrBinOp::Sub | IrBinOp::Mul | IrBinOp::Div => Kind::Num,
                    IrBinOp::Eq
                    | IrBinOp::Ne
                    | IrBinOp::Lt
                    | IrBinOp::Le
                    | IrBinOp::Gt
                    | IrBinOp::Ge => {
                        comparison(*op, l, r, e.span())?;
                        Kind::Bool
                    }
                    _ => Kind::Bool,
                }
            }
        },
        IrExpr::Unary { op, operand, .. } => match op {
            IrUnOp::Neg => {
                kind(scope, operand)?;
                Kind::Num
            }
            IrUnOp::Not => {
                want_bool(scope, operand, "the operand of `!`")?;
                Kind::Bool
            }
        },
        IrExpr::If {
            cond, then, else_, ..
        } => {
            want_bool(scope, cond, "the condition of an `if`")?;
            let t = kind(scope, then)?;
            let f = kind(scope, else_)?;
            if t.comparable() && f.comparable() && t != f {
                return Err(LowerError::ConditionRefused {
                    code: "if-branch-type-mismatch",
                    message: format!(
                        "the two branches of this `if` have different types: the `then` side is {} and the `else` side is {}",
                        t.name(),
                        f.name()
                    ),
                    span: e.span(),
                });
            }
            if t == Kind::Unknown { f } else { t }
        }
        IrExpr::Match {
            scrutinee, arms, ..
        } => {
            kind(scope, scrutinee)?;
            for a in arms {
                let bound = match &a.pattern {
                    IrPattern::Ident { name, .. } => Some(name.clone()),
                    IrPattern::Variant { binding, .. } => Some(binding.clone()),
                    _ => None,
                };
                let pushed = bound.is_some();
                if let Some(b) = bound {
                    scope.locals.push(b);
                }
                kind(scope, &a.body)?;
                if pushed {
                    scope.locals.pop();
                }
            }
            Kind::Unknown
        }
        IrExpr::Ok { value, .. } | IrExpr::Some { value, .. } => {
            kind(scope, value)?;
            Kind::Other
        }
        IrExpr::Err { reason, .. } => {
            kind(scope, reason)?;
            Kind::Other
        }
        IrExpr::None { .. } | IrExpr::Cancelled { .. } => Kind::Other,
        IrExpr::Await { request, .. } => {
            kind(scope, request)?;
            Kind::Unknown
        }
        IrExpr::ListLit { items, .. } => {
            for i in items {
                kind(scope, i)?;
            }
            Kind::List
        }
        IrExpr::Filter {
            list, item, cond, ..
        } => {
            kind(scope, list)?;
            scope.locals.push(item.clone());
            let r = kind(scope, cond);
            scope.locals.pop();
            r?;
            Kind::List
        }
        IrExpr::Map {
            list, item, body, ..
        } => {
            kind(scope, list)?;
            scope.locals.push(item.clone());
            let r = kind(scope, body);
            scope.locals.pop();
            r?;
            Kind::List
        }
        IrExpr::SortBy {
            list, item, key, ..
        } => {
            kind(scope, list)?;
            scope.locals.push(item.clone());
            let r = kind(scope, key);
            scope.locals.pop();
            r?;
            Kind::List
        }
        IrExpr::Len { list, .. } => {
            kind(scope, list)?;
            Kind::Num
        }
    })
}
