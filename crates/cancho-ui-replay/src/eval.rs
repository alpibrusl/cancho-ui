//! The expression evaluator (docs/replay.md §3). A plain recursive function over
//! the typed IR. Anything that depends on the world outside the expression (a
//! state read, `clock.now()`, an `await`'s answer) goes through the statement's
//! `Journal`, which is how an `await` suspends without a JavaScript runtime.

use std::collections::BTreeMap;

use cancho_ui_ir::ir::{IrBinOp, IrExpr, IrMatchArm, IrPattern, IrUnOp};
use cancho_ui_syntax::span::Span;

use crate::value::{Effect, Req, Value, js_number};

/// A refusal or runtime fault, with the source it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Fault {
    pub rule: &'static str,
    pub message: String,
    pub span: Span,
}

/// Why evaluation stopped before producing a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// An `await` with no recorded answer: the handler waits for this request.
    Suspend(Req),
    Fault(Fault),
}

impl From<Fault> for Stop {
    fn from(f: Fault) -> Stop {
        Stop::Fault(f)
    }
}

pub fn fault(rule: &'static str, message: impl Into<String>, span: Span) -> Stop {
    Stop::Fault(Fault {
        rule,
        message: message.into(),
        span,
    })
}

/// The recorded outside-world inputs of one statement, in the order the
/// evaluator asked for them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Journal {
    entries: Vec<Value>,
    cursor: usize,
}

impl Journal {
    /// Start a fresh evaluation of the statement, consuming what is recorded.
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }

    /// The recorded answer at the cursor, if the statement has been this far.
    fn replayed(&mut self) -> Option<Value> {
        let v = self.entries.get(self.cursor).cloned();
        if v.is_some() {
            self.cursor += 1;
        }
        v
    }

    fn record(&mut self, v: Value) {
        self.entries.push(v);
        self.cursor += 1;
    }

    /// Append the answer an `await` was waiting for.
    pub fn deliver(&mut self, v: Value) {
        self.entries.push(v);
    }
}

pub struct Ctx<'a> {
    pub state: &'a BTreeMap<String, Value>,
    pub clock: f64,
    pub journal: &'a mut Journal,
    pub locals: Vec<(String, Value)>,
    /// Evaluating a state initial value: identifiers, calls, `match` and
    /// `await` are `undefined` in the emitted JavaScript, so refused here.
    pub init: bool,
}

pub fn span_of(e: &IrExpr) -> Span {
    match e {
        IrExpr::IntLit { span, .. }
        | IrExpr::FloatLit { span, .. }
        | IrExpr::StringLit { span, .. }
        | IrExpr::BoolLit { span, .. }
        | IrExpr::Ident { span, .. }
        | IrExpr::Call { span, .. }
        | IrExpr::Binary { span, .. }
        | IrExpr::Unary { span, .. }
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
        | IrExpr::Len { span, .. } => *span,
    }
}

pub fn eval(ctx: &mut Ctx, e: &IrExpr) -> Result<Value, Stop> {
    let span = span_of(e);
    match e {
        IrExpr::IntLit { value, .. } => Ok(Value::Num(*value as f64)),
        IrExpr::FloatLit { value, .. } => Ok(Value::Num(*value)),
        IrExpr::StringLit { value, .. } => Ok(Value::Str(value.clone())),
        IrExpr::BoolLit { value, .. } => Ok(Value::Bool(*value)),
        IrExpr::Ident { name, .. } => ident(ctx, name, span),
        IrExpr::Call { path, args, .. } => call(ctx, path, args, span),
        IrExpr::Unary {
            op: IrUnOp::Neg,
            operand,
            ..
        } => match eval(ctx, operand)? {
            Value::Num(n) => finite(0.0 - n, span),
            other => Err(fault(
                "replay-type-error",
                format!("unary `-` needs a number, got {}", describe(&other)),
                span,
            )),
        },
        IrExpr::Binary { op, lhs, rhs, .. } => {
            let l = eval(ctx, lhs)?;
            let r = eval(ctx, rhs)?;
            binary(*op, l, r, span)
        }
        IrExpr::Ok { value, .. } => Ok(Value::Ok(Box::new(eval(ctx, value)?))),
        IrExpr::Err { reason, .. } => Ok(Value::Err(Box::new(eval(ctx, reason)?))),
        IrExpr::Some { value, .. } => Ok(Value::Some(Box::new(eval(ctx, value)?))),
        IrExpr::None { .. } => Ok(Value::None),
        IrExpr::Cancelled { .. } => {
            if ctx.init {
                Err(unsupported_init(span))
            } else {
                Ok(Value::Cancelled)
            }
        }
        IrExpr::ListLit { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                out.push(eval(ctx, i)?);
            }
            Ok(Value::List(out))
        }
        IrExpr::Len { list, .. } => match eval(ctx, list)? {
            Value::List(items) => Ok(Value::Num(items.len() as f64)),
            Value::Str(s) => Ok(Value::Num(s.encode_utf16().count() as f64)),
            other => Err(fault(
                "replay-type-error",
                format!("`len` needs a list or a string, got {}", describe(&other)),
                span,
            )),
        },
        IrExpr::Filter {
            list, item, cond, ..
        } => {
            let items = list_of(eval(ctx, list)?, "filter", span)?;
            let mut out = Vec::new();
            for v in items {
                ctx.locals.push((item.clone(), v.clone()));
                let keep = eval_lambda(ctx, cond);
                ctx.locals.pop();
                if truthy(&keep?, span)? {
                    out.push(v);
                }
            }
            Ok(Value::List(out))
        }
        IrExpr::Map {
            list, item, body, ..
        } => {
            let items = list_of(eval(ctx, list)?, "map", span)?;
            let mut out = Vec::with_capacity(items.len());
            for v in items {
                ctx.locals.push((item.clone(), v));
                let mapped = eval_lambda(ctx, body);
                ctx.locals.pop();
                out.push(mapped?);
            }
            Ok(Value::List(out))
        }
        IrExpr::Match {
            scrutinee, arms, ..
        } => {
            if ctx.init {
                return Err(unsupported_init(span));
            }
            let s = eval(ctx, scrutinee)?;
            eval_match(ctx, &s, arms, span)
        }
        IrExpr::Await { request, .. } => {
            if ctx.init {
                return Err(unsupported_init(span));
            }
            match eval(ctx, request)? {
                Value::Request(req) => match ctx.journal.replayed() {
                    Some(answer) => Ok(answer),
                    None => Err(Stop::Suspend(req)),
                },
                other => Err(fault(
                    "replay-unsupported",
                    format!(
                        "`await` of {}: replay awaits only `net.search(..)` and `sleep(..)`",
                        describe(&other)
                    ),
                    span,
                )),
            }
        }
    }
}

/// A `filter`/`map` callback is synchronous JavaScript: an `await` in it cannot
/// suspend the handler.
fn eval_lambda(ctx: &mut Ctx, e: &IrExpr) -> Result<Value, Stop> {
    match eval(ctx, e) {
        Err(Stop::Suspend(_)) => Err(fault(
            "replay-unsupported",
            "`await` inside a `filter`/`map` callback",
            span_of(e),
        )),
        other => other,
    }
}

fn unsupported_init(span: Span) -> Stop {
    fault(
        "replay-unsupported",
        "a state initial value that reads an identifier, calls a function, matches or awaits is `undefined` in the emitted JavaScript; replay refuses it instead of inventing a value",
        span,
    )
}

fn ident(ctx: &mut Ctx, name: &str, span: Span) -> Result<Value, Stop> {
    if ctx.init {
        return Err(unsupported_init(span));
    }
    if let Some((_, v)) = ctx.locals.iter().rev().find(|(n, _)| n == name) {
        return Ok(v.clone());
    }
    match ctx.state.get(name) {
        Some(_) => {
            // A state read is journaled: after an `await` the statement reads
            // again from its start, and must see what it saw the first time.
            if let Some(v) = ctx.journal.replayed() {
                return Ok(v);
            }
            let v = ctx.state[name].clone();
            ctx.journal.record(v.clone());
            Ok(v)
        }
        None => Err(fault(
            "replay-unknown-identifier",
            format!("`{name}` is neither a parameter, a local binding nor a state field"),
            span,
        )),
    }
}

fn call(ctx: &mut Ctx, path: &[String], args: &[IrExpr], span: Span) -> Result<Value, Stop> {
    if ctx.init {
        return Err(unsupported_init(span));
    }
    let name = path.join(".");
    match (name.as_str(), args.len()) {
        ("clock.now", 0) => {
            if let Some(v) = ctx.journal.replayed() {
                return Ok(v);
            }
            let v = Value::Num(ctx.clock);
            ctx.journal.record(v.clone());
            Ok(v)
        }
        ("net.search", 1) => {
            let q = eval(ctx, &args[0])?;
            Ok(Value::Request(Req {
                effect: Effect::NetSearch,
                args: vec![q],
            }))
        }
        ("sleep", 1) => match eval(ctx, &args[0])? {
            Value::Num(ms) if ms >= 0.0 => Ok(Value::Request(Req {
                effect: Effect::Sleep,
                args: vec![Value::Num(ms)],
            })),
            other => Err(fault(
                "replay-type-error",
                format!(
                    "`sleep` needs a non-negative number of milliseconds, got {}",
                    describe(&other)
                ),
                span,
            )),
        },
        ("clock.now" | "net.search" | "sleep", n) => Err(fault(
            "replay-type-error",
            format!(
                "`{name}` called with {n} argument(s); the emitted adapter takes a fixed number"
            ),
            span,
        )),
        _ => Err(fault(
            "replay-unsupported",
            format!("call to `{name}`: replay models `clock.now`, `net.search` and `sleep` only"),
            span,
        )),
    }
}

fn list_of(v: Value, what: &str, span: Span) -> Result<Vec<Value>, Stop> {
    match v {
        Value::List(items) => Ok(items),
        other => Err(fault(
            "replay-type-error",
            format!("`{what}` needs a list, got {}", describe(&other)),
            span,
        )),
    }
}

fn truthy(v: &Value, span: Span) -> Result<bool, Stop> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Num(n) => Ok(*n != 0.0),
        Value::Str(s) => Ok(!s.is_empty()),
        other => Err(fault(
            "replay-type-error",
            format!(
                "a `filter` condition must be a bool, number or string, got {}",
                describe(other)
            ),
            span,
        )),
    }
}

fn eval_match(
    ctx: &mut Ctx,
    scrutinee: &Value,
    arms: &[IrMatchArm],
    span: Span,
) -> Result<Value, Stop> {
    for arm in arms {
        match &arm.pattern {
            IrPattern::Wildcard { .. } | IrPattern::Ident { .. } => {
                return eval(ctx, &arm.body);
            }
            IrPattern::UnitVariant { name, .. } => {
                if scrutinee.tag() == Some(name.as_str()) {
                    return eval(ctx, &arm.body);
                }
            }
            IrPattern::Variant { name, binding, .. } => {
                if scrutinee.tag() == Some(name.as_str()) {
                    if binding == "_" {
                        return eval(ctx, &arm.body);
                    }
                    let payload = scrutinee.payload().cloned().unwrap_or(Value::Unit);
                    ctx.locals.push((binding.clone(), payload));
                    let out = eval(ctx, &arm.body);
                    ctx.locals.pop();
                    return out;
                }
            }
        }
    }
    Err(fault(
        "replay-no-match-arm",
        format!("no arm of this `match` matches {}", describe(scrutinee)),
        span,
    ))
}

fn finite(n: f64, span: Span) -> Result<Value, Stop> {
    if n.is_finite() {
        Ok(Value::Num(n))
    } else {
        Err(fault(
            "replay-non-finite",
            "the result is not a finite number (JSON cannot carry it, and `Infinity`/`NaN` are not part of the language)",
            span,
        ))
    }
}

/// A scalar's JavaScript string form, for `+` with a string.
fn concat_form(v: &Value, span: Span) -> Result<String, Stop> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Num(n) => js_number(*n).ok_or_else(|| {
            fault(
                "replay-number-format",
                format!("{n} prints in exponent form in JavaScript; replay does not imitate it"),
                span,
            )
        }),
        other => Err(fault(
            "replay-type-error",
            format!(
                "`+` with a string needs a string, number or bool, got {}",
                describe(other)
            ),
            span,
        )),
    }
}

fn binary(op: IrBinOp, l: Value, r: Value, span: Span) -> Result<Value, Stop> {
    use IrBinOp::*;
    match op {
        Add => match (&l, &r) {
            (Value::Num(a), Value::Num(b)) => finite(a + b, span),
            (Value::Str(_), _) | (_, Value::Str(_)) => {
                Ok(Value::Str(concat_form(&l, span)? + &concat_form(&r, span)?))
            }
            _ => Err(operands("+", &l, &r, span)),
        },
        Sub | Mul | Div => match (&l, &r) {
            (Value::Num(a), Value::Num(b)) => match op {
                Sub => finite(a - b, span),
                Mul => finite(a * b, span),
                _ => finite(a / b, span),
            },
            _ => Err(operands(
                match op {
                    Sub => "-",
                    Mul => "*",
                    _ => "/",
                },
                &l,
                &r,
                span,
            )),
        },
        Eq | Ne => {
            let same = match (&l, &r) {
                (Value::Num(a), Value::Num(b)) => a == b,
                (Value::Str(a), Value::Str(b)) => a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (a, b) if scalar(a) && scalar(b) => false,
                _ => {
                    return Err(fault(
                        "replay-type-error",
                        format!(
                            "`===` on {} and {}: JavaScript compares lists and variants by reference, which replay does not model",
                            describe(&l),
                            describe(&r)
                        ),
                        span,
                    ));
                }
            };
            Ok(Value::Bool(same == (op == Eq)))
        }
        Lt | Le | Gt | Ge => {
            let ord = match (&l, &r) {
                (Value::Num(a), Value::Num(b)) => a.partial_cmp(b),
                // JavaScript orders strings by UTF-16 code unit.
                (Value::Str(a), Value::Str(b)) => Some(a.encode_utf16().cmp(b.encode_utf16())),
                _ => return Err(operands("<", &l, &r, span)),
            };
            let Some(ord) = ord else {
                return Err(operands("<", &l, &r, span));
            };
            Ok(Value::Bool(match op {
                Lt => ord.is_lt(),
                Le => ord.is_le(),
                Gt => ord.is_gt(),
                _ => ord.is_ge(),
            }))
        }
    }
}

fn scalar(v: &Value) -> bool {
    matches!(v, Value::Num(_) | Value::Str(_) | Value::Bool(_))
}

fn operands(op: &str, l: &Value, r: &Value, span: Span) -> Stop {
    fault(
        "replay-type-error",
        format!(
            "`{op}` is not defined on {} and {}",
            describe(l),
            describe(r)
        ),
        span,
    )
}

pub fn describe(v: &Value) -> String {
    match v {
        Value::Num(_) => "a number".into(),
        Value::Str(_) => "a string".into(),
        Value::Bool(_) => "a bool".into(),
        Value::List(_) => "a list".into(),
        Value::Ok(_) => "an `Ok`".into(),
        Value::Err(_) => "an `Err`".into(),
        Value::Some(_) => "a `Some`".into(),
        Value::None => "`None`".into(),
        Value::Cancelled => "`Cancelled`".into(),
        Value::Unit => "the unit value".into(),
        Value::Request(_) => "a request".into(),
    }
}
