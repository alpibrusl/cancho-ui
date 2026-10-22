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

/// The store key of `key` under namespace `ns` (docs/adapters.md section 12), without the adapter's `cui:`.
fn storage_key(ns: &str, key: &str) -> String {
    if ns.is_empty() {
        key.to_string()
    } else {
        format!("{ns}:{key}")
    }
}

/// The value at `path` of the JSON `text` (docs/language-design.md decision 13), or None when the text is not JSON,
/// is nested more than 127 deep, holds a number out of range, or the path does not lead to a value. A path is
/// dot-separated: an object key, or, in an array, a decimal index (no leading zero, at most 9 digits); the empty
/// path is the root.
fn json_at(text: &str, path: &str) -> Option<serde_json::Value> {
    use serde_json::Value as J;
    let root = serde_json::from_str::<J>(text).ok()?;
    let mut cur = &root;
    if !path.is_empty() {
        for part in path.split('.') {
            cur = match cur {
                J::Array(items) => items.get(array_index(part)?)?,
                J::Object(map) => map.get(part)?,
                _ => return None,
            };
        }
    }
    Some(cur.clone())
}

/// `json.get(text, path, default)`, the model of the emitted `__cui_json_get`. A pure function: the String at `path`
/// of the JSON `text`, or `default` when there is no such value or it is not a String, a Boolean or an
/// Number that is an integer of magnitude 2^53 or less, or a decimal with 1e-6 <= |x| < 1e15 (rendered as the
/// shortest digits that read back as the same number).
pub fn json_get(text: &str, path: &str, default: &str) -> String {
    use serde_json::Value as J;
    match json_at(text, path) {
        Some(J::String(s)) => s,
        Some(J::Bool(b)) => b.to_string(),
        Some(J::Number(n)) => match n.as_f64() {
            Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_992.0 => {
                format!("{}", f as i64)
            }
            // A decimal prints as its shortest round-trip digits, which Rust and JavaScript agree on while neither
            // uses an exponent: 1e-6 <= |f| < 1e15 (docs/adapters.md 14.1).
            Some(f) if f.is_finite() && f.abs() >= 1e-6 && f.abs() < 1e15 => format!("{f}"),
            _ => default.to_string(),
        },
        _ => default.to_string(),
    }
}

/// The most elements `json.items` answers.
pub const JSON_ITEMS_MAX: usize = 1000;

/// `json.items(text, path)`, the model of the emitted `__cui_json_items`: the elements of the array at `path`, each
/// as the compact JSON text of the element (opaque: it is for `json.get` and `json.items`), at most `JSON_ITEMS_MAX`
/// of them; an empty list when there is no such array.
pub fn json_items(text: &str, path: &str) -> Vec<String> {
    match json_at(text, path) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .take(JSON_ITEMS_MAX)
            .map(|v| v.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// `0` or a decimal without a leading zero, at most 9 digits.
fn array_index(part: &str) -> Option<usize> {
    let b = part.as_bytes();
    let ok = !b.is_empty()
        && b.len() <= 9
        && b.iter().all(u8::is_ascii_digit)
        && (b[0] != b'0' || b.len() == 1);
    if ok { part.parse().ok() } else { None }
}

pub struct Ctx<'a> {
    pub state: &'a BTreeMap<String, Value>,
    pub clock: f64,
    /// The `Storage` store, key to value, without the adapter's `cui:` prefix (a namespaced key is `ns:key`).
    pub storage: &'a mut BTreeMap<String, String>,
    /// The component's `Storage("ns")` namespace, or "" : a key is looked up as `ns:key`, as the adapter does.
    pub storage_ns: &'a str,
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
        IrExpr::Unary {
            op: IrUnOp::Not,
            operand,
            ..
        } => Ok(Value::Bool(!boolean(&eval(ctx, operand)?, "`!`", span)?)),
        // `&&` and `||` evaluate the right side only when the left does not decide the answer, as JavaScript does
        // (docs/conditionals.md section 3.1); both operands must be a `Bool`, which JavaScript's own `&&` does not ask.
        IrExpr::Binary {
            op: op @ (IrBinOp::And | IrBinOp::Or),
            lhs,
            rhs,
            ..
        } => {
            let name = if *op == IrBinOp::And { "`&&`" } else { "`||`" };
            let l = boolean(&eval(ctx, lhs)?, name, span)?;
            if l == (*op == IrBinOp::Or) {
                return Ok(Value::Bool(l));
            }
            Ok(Value::Bool(boolean(&eval(ctx, rhs)?, name, span)?))
        }
        IrExpr::Binary { op, lhs, rhs, .. } => {
            let l = eval(ctx, lhs)?;
            let r = eval(ctx, rhs)?;
            binary(*op, l, r, span)
        }
        IrExpr::If {
            cond, then, else_, ..
        } => {
            if boolean(&eval(ctx, cond)?, "an `if` condition", span)? {
                eval(ctx, then)
            } else {
                eval(ctx, else_)
            }
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
        IrExpr::SortBy {
            list, item, key, ..
        } => {
            let items = list_of(eval(ctx, list)?, "sort", span)?;
            let mut rows = Vec::with_capacity(items.len());
            for v in items {
                ctx.locals.push((item.clone(), v.clone()));
                let k = eval_lambda(ctx, key);
                ctx.locals.pop();
                rows.push((k?, v));
            }
            // `sort_by` is stable, as `Array.prototype.sort` is.
            rows.sort_by(|a, b| sort_key_order(&a.0, &b.0));
            Ok(Value::List(rows.into_iter().map(|(_, v)| v).collect()))
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
        ("storage.get", 2) => {
            let key = eval(ctx, &args[0])?;
            let fallback = eval(ctx, &args[1])?;
            if let Some(v) = ctx.journal.replayed() {
                return Ok(v);
            }
            let (Value::Str(k), Value::Str(_)) = (&key, &fallback) else {
                return Err(fault(
                    "replay-type-error",
                    format!(
                        "`storage.get` needs a String key and a String default, got {} and {}",
                        describe(&key),
                        describe(&fallback)
                    ),
                    span,
                ));
            };
            let full = storage_key(ctx.storage_ns, k);
            let v = match ctx.storage.get(&full) {
                Some(found) => Value::Str(found.clone()),
                None => fallback,
            };
            ctx.journal.record(v.clone());
            Ok(v)
        }
        ("storage.set", 2) => {
            let key = eval(ctx, &args[0])?;
            let value = eval(ctx, &args[1])?;
            if let Some(v) = ctx.journal.replayed() {
                return Ok(v);
            }
            let (Value::Str(k), Value::Str(v)) = (&key, &value) else {
                return Err(fault(
                    "replay-type-error",
                    format!(
                        "`storage.set` needs a String key and a String value, got {} and {}",
                        describe(&key),
                        describe(&value)
                    ),
                    span,
                ));
            };
            ctx.storage
                .insert(storage_key(ctx.storage_ns, k), v.clone());
            ctx.journal.record(Value::Bool(true));
            Ok(Value::Bool(true))
        }
        ("json.quote", 1) => match eval(ctx, &args[0])? {
            Value::Str(t) => Ok(Value::Str(json_quote(&t))),
            other => Err(fault(
                "replay-type-error",
                format!("`json.quote` needs a String, got {}", describe(&other)),
                span,
            )),
        },
        ("json.items", 2) => {
            let text = eval(ctx, &args[0])?;
            let path = eval(ctx, &args[1])?;
            match (&text, &path) {
                (Value::Str(t), Value::Str(p)) => Ok(Value::List(
                    json_items(t, p).into_iter().map(Value::Str).collect(),
                )),
                _ => Err(fault(
                    "replay-type-error",
                    format!(
                        "`json.items` needs two Strings (text, path), got {} and {}",
                        describe(&text),
                        describe(&path)
                    ),
                    span,
                )),
            }
        }
        ("json.get", 3) => {
            let text = eval(ctx, &args[0])?;
            let path = eval(ctx, &args[1])?;
            let fallback = eval(ctx, &args[2])?;
            match (&text, &path, &fallback) {
                (Value::Str(t), Value::Str(p), Value::Str(d)) => Ok(Value::Str(json_get(t, p, d))),
                _ => Err(fault(
                    "replay-type-error",
                    format!(
                        "`json.get` needs three Strings (text, path, default), got {}, {} and {}",
                        describe(&text),
                        describe(&path),
                        describe(&fallback)
                    ),
                    span,
                )),
            }
        }
        ("reverse", 1) => {
            let mut items = list_of(eval(ctx, &args[0])?, "reverse", span)?;
            items.reverse();
            Ok(Value::List(items))
        }
        ("contains", 2) => {
            let text = eval(ctx, &args[0])?;
            let needle = eval(ctx, &args[1])?;
            match (&text, &needle) {
                (Value::Str(t), Value::Str(n)) => Ok(Value::Bool(t.contains(n.as_str()))),
                _ => Err(fault(
                    "replay-type-error",
                    format!(
                        "`contains` needs two Strings (text, needle), got {} and {}",
                        describe(&text),
                        describe(&needle)
                    ),
                    span,
                )),
            }
        }
        ("num", 2) => {
            let text = eval(ctx, &args[0])?;
            let default = eval(ctx, &args[1])?;
            match (&text, &default) {
                (Value::Str(t), Value::Num(d)) => Ok(Value::Num(num_of_text(t, *d))),
                _ => Err(fault(
                    "replay-type-error",
                    format!(
                        "`num` needs a String and a number default, got {} and {}",
                        describe(&text),
                        describe(&default)
                    ),
                    span,
                )),
            }
        }
        ("take", 2) | ("skip", 2) => {
            let list = eval(ctx, &args[0])?;
            let n = eval(ctx, &args[1])?;
            match (list, &n) {
                (Value::List(items), Value::Num(n)) => {
                    let k = page_count(*n, items.len());
                    Ok(Value::List(if name == "take" {
                        items[..k].to_vec()
                    } else {
                        items[k..].to_vec()
                    }))
                }
                (list, _) => Err(fault(
                    "replay-type-error",
                    format!(
                        "`{name}` needs a list and a number, got {} and {}",
                        describe(&list),
                        describe(&n)
                    ),
                    span,
                )),
            }
        }
        ("min" | "max" | "abs" | "floor" | "ceil" | "round", n) if math_arity(&name) == Some(n) => {
            let mut nums = Vec::with_capacity(n);
            for a in args {
                match eval(ctx, a)? {
                    Value::Num(x) => nums.push(x),
                    other => {
                        return Err(fault(
                            "replay-type-error",
                            format!("`{name}` needs numbers, got {}", describe(&other)),
                            span,
                        ));
                    }
                }
            }
            match math_builtin(&name, &nums) {
                Some(x) => Ok(Value::Num(x)),
                None => Err(fault(
                    "replay-type-error",
                    format!("`{name}` called with {} argument(s)", nums.len()),
                    span,
                )),
            }
        }
        ("lines" | "trim" | "upper" | "split" | "join" | "starts_with" | "ends_with", n)
            if text_builtin_arity(&name) == Some(n) =>
        {
            let mut vals = Vec::with_capacity(n);
            for a in args {
                vals.push(eval(ctx, a)?);
            }
            text_builtin(&name, &vals).map_err(|why| fault("replay-type-error", why, span))
        }
        ("lower", 1) => match eval(ctx, &args[0])? {
            Value::Str(t) => Ok(Value::Str(t.to_ascii_lowercase())),
            other => Err(fault(
                "replay-type-error",
                format!("`lower` needs a String, got {}", describe(&other)),
                span,
            )),
        },
        ("location.current", 0) => Ok(Value::Request(Req {
            effect: Effect::LocationCurrent,
            args: Vec::new(),
        })),
        ("net.get", 1) => match eval(ctx, &args[0])? {
            url @ Value::Str(_) => Ok(Value::Request(Req {
                effect: Effect::NetGet,
                args: vec![url],
            })),
            other => Err(fault(
                "replay-type-error",
                format!("`net.get` needs a String URL, got {}", describe(&other)),
                span,
            )),
        },
        ("net.fetch", 1) => match eval(ctx, &args[0])? {
            url @ Value::Str(_) => Ok(Value::Request(Req {
                effect: Effect::NetFetch,
                args: vec![url],
            })),
            other => Err(fault(
                "replay-type-error",
                format!("`net.fetch` needs a String URL, got {}", describe(&other)),
                span,
            )),
        },
        ("net.post", 2) => {
            let url = eval(ctx, &args[0])?;
            let body = eval(ctx, &args[1])?;
            match (&url, &body) {
                (Value::Str(_), Value::Str(_)) => Ok(Value::Request(Req {
                    effect: Effect::NetPost,
                    args: vec![url, body],
                })),
                _ => Err(fault(
                    "replay-type-error",
                    format!(
                        "`net.post` needs a String URL and a String body, got {} and {}",
                        describe(&url),
                        describe(&body)
                    ),
                    span,
                )),
            }
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
        (
            "clock.now" | "net.search" | "net.get" | "net.fetch" | "net.post" | "location.current"
            | "sleep" | "storage.get" | "storage.set" | "json.get" | "json.items" | "json.quote"
            | "reverse" | "contains" | "lower" | "num" | "take" | "skip" | "min" | "max" | "abs"
            | "floor" | "ceil" | "round" | "lines" | "split" | "join" | "trim" | "upper"
            | "starts_with" | "ends_with",
            n,
        ) => Err(fault(
            "replay-type-error",
            format!(
                "`{name}` called with {n} argument(s); the emitted adapter takes a fixed number"
            ),
            span,
        )),
        _ => Err(fault(
            "replay-unsupported",
            format!(
                "call to `{name}`: replay models `clock.now`, `net.search`, `net.get`, `net.fetch`, `net.post`, `location.current`, `sleep`, `storage.get`, `storage.set`, `json.get` and `json.items` only"
            ),
            span,
        )),
    }
}

/// `take(list, n)` / `skip(list, n)` (docs/lists.md section 6): how many elements `n` counts, for a list of `len`. `n`
/// is cut toward zero; a negative `n` and NaN count 0; more than `len` counts `len`. The emitted `__cui_page_count`
/// is the same function (it leaves the clamp to `slice`); `tests/paging_corpus.json` is checked against both.
pub fn page_count(n: f64, len: usize) -> usize {
    let k = n.trunc();
    if k.is_nan() || k <= 0.0 {
        0
    } else if k >= len as f64 {
        len
    } else {
        k as usize
    }
}

fn math_arity(name: &str) -> Option<usize> {
    match name {
        "min" | "max" => Some(2),
        "abs" | "floor" | "ceil" | "round" => Some(1),
        _ => None,
    }
}

/// `min`, `max`, `abs`, `floor`, `ceil` and `round` (docs/lists.md section 8): the model of the emitted helpers, which are
/// `Math.*` of the same numbers. `None` for an unknown name or the wrong number of arguments. NaN and the sign of zero
/// are modelled as JavaScript has them (`Math.max(0, -0)` is `0`, `Math.min(0, -0)` is `-0`, either NaN gives NaN,
/// `Math.round(-0.4)` is `-0`), which `f64::min` and `f64::max` do not promise; `tests/math_corpus.json` is checked
/// against both, comparing bits.
pub fn math_builtin(name: &str, a: &[f64]) -> Option<f64> {
    if math_arity(name)? != a.len() {
        return None;
    }
    Some(match name {
        "min" | "max" => {
            let (x, y) = (a[0], a[1]);
            if x.is_nan() || y.is_nan() {
                f64::NAN
            } else if x == y {
                // Equal, so only zeros differ: the sign decides, negative is the smaller.
                let neg = x.is_sign_negative() || y.is_sign_negative();
                let pos = x.is_sign_positive() || y.is_sign_positive();
                if name == "min" && neg || name == "max" && !pos {
                    -x.abs()
                } else {
                    x.abs()
                }
            } else if (x < y) == (name == "min") {
                x
            } else {
                y
            }
        }
        "abs" => a[0].abs(),
        "floor" => a[0].floor(),
        "ceil" => a[0].ceil(),
        _ => {
            // round: halves toward +Infinity; exact, since x - floor(x) is exact for a double below 2^52.
            let x = a[0];
            if !x.is_finite() || x == 0.0 {
                x
            } else {
                let f = x.floor();
                let r = if x - f >= 0.5 { f + 1.0 } else { f };
                if r == 0.0 && x < 0.0 { -0.0 } else { r }
            }
        }
    })
}

/// The most elements `lines` and `split` return (docs/lists.md section 7.1).
pub const TEXT_LIST_MAX: usize = 10_000;

fn text_builtin_arity(name: &str) -> Option<usize> {
    match name {
        "lines" | "trim" | "upper" => Some(1),
        "split" | "join" | "starts_with" | "ends_with" => Some(2),
        _ => None,
    }
}

fn ascii_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c' | '\x0b')
}

/// `lines`, `split`, `join`, `trim`, `upper`, `starts_with` and `ends_with` (docs/lists.md section 7): the model of the
/// emitted helpers. An `Err` is a `replay-type-error` message.
fn text_builtin(name: &str, a: &[Value]) -> Result<Value, String> {
    let s = |i: usize| match &a[i] {
        Value::Str(t) => Ok(t.as_str()),
        other => Err(format!(
            "`{name}` needs a String for argument {}, got {}",
            i + 1,
            describe(other)
        )),
    };
    Ok(match name {
        "lines" => {
            let t = s(0)?;
            let mut parts: Vec<&str> = t.split('\n').collect();
            if parts.last() == Some(&"") {
                parts.pop();
            }
            Value::List(
                parts
                    .into_iter()
                    .take(TEXT_LIST_MAX)
                    .map(|l| Value::Str(l.strip_suffix('\r').unwrap_or(l).to_string()))
                    .collect(),
            )
        }
        "split" => {
            let (t, sep) = (s(0)?, s(1)?);
            if sep.is_empty() {
                Value::List(vec![Value::Str(t.to_string())])
            } else {
                Value::List(
                    t.split(sep)
                        .take(TEXT_LIST_MAX)
                        .map(|p| Value::Str(p.to_string()))
                        .collect(),
                )
            }
        }
        "join" => {
            let sep = s(1)?;
            let Value::List(items) = &a[0] else {
                return Err(format!("`join` needs a list, got {}", describe(&a[0])));
            };
            let mut parts = Vec::with_capacity(items.len());
            for v in items {
                match v {
                    Value::Str(t) => parts.push(t.as_str()),
                    other => {
                        return Err(format!(
                            "`join` needs a list of Strings, found {}",
                            describe(other)
                        ));
                    }
                }
            }
            Value::Str(parts.join(sep))
        }
        "trim" => Value::Str(s(0)?.trim_matches(ascii_space).to_string()),
        "upper" => Value::Str(s(0)?.to_ascii_uppercase()),
        "starts_with" => Value::Bool(s(0)?.starts_with(s(1)?)),
        "ends_with" => Value::Bool(s(0)?.ends_with(s(1)?)),
        _ => return Err(format!("`{name}` is not a text builtin")),
    })
}

/// `text_builtin` on JSON values, for the corpus check (`tests/text_corpus.json`): the argument list and the result are
/// JSON (a list of Strings is an array). `None` for an unknown name or a wrong argument shape.
pub fn text_builtin_json(name: &str, args: &[serde_json::Value]) -> Option<serde_json::Value> {
    fn to_value(j: &serde_json::Value) -> Option<Value> {
        Some(match j {
            serde_json::Value::String(t) => Value::Str(t.clone()),
            serde_json::Value::Array(items) => {
                Value::List(items.iter().map(to_value).collect::<Option<Vec<_>>>()?)
            }
            _ => return None,
        })
    }
    fn to_json(v: &Value) -> Option<serde_json::Value> {
        Some(match v {
            Value::Str(t) => serde_json::Value::String(t.clone()),
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::List(items) => {
                serde_json::Value::Array(items.iter().map(to_json).collect::<Option<Vec<_>>>()?)
            }
            _ => return None,
        })
    }
    if text_builtin_arity(name)? != args.len() {
        return None;
    }
    let vals = args.iter().map(to_value).collect::<Option<Vec<_>>>()?;
    to_json(&text_builtin(name, &vals).ok()?)
}

/// `json.quote(text)` (docs/language-design.md decision 15): the JSON string literal of a text. The emitted `__cui_json_quote`
/// is `JSON.stringify`; `tests/json_quote_corpus.json` is checked against both.
pub fn json_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `num(text, default)` (docs/lists.md section 1): `text` must be exactly a JSON number, and finite once read;
/// otherwise `default`. The emitted `__cui_num` is the same function; `tests/num_corpus.json` is checked against both.
pub fn num_of_text(text: &str, default: f64) -> f64 {
    let b = text.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = 0;
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => i += digits(i),
        _ => return default,
    }
    if b.get(i) == Some(&b'.') {
        let n = digits(i + 1);
        if n == 0 {
            return default;
        }
        i += 1 + n;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let n = digits(i);
        if n == 0 {
            return default;
        }
        i += n;
    }
    if i != b.len() {
        return default;
    }
    match text.parse::<f64>() {
        Ok(n) if n.is_finite() => n,
        _ => default,
    }
}

/// The order `sort` puts keys in (docs/lists.md), the same as the emitted `__cui_sort`: numbers (NaN last), then
/// strings by UTF-16 code units, then anything else, which compares equal.
fn sort_key_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let rank = |v: &Value| match v {
        Value::Num(_) => 0,
        Value::Str(_) => 1,
        _ => 2,
    };
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => match (x.is_nan(), y.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        },
        (Value::Str(x), Value::Str(y)) => x.encode_utf16().cmp(y.encode_utf16()),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// `sort_key_order` on JSON-shaped keys, for the corpus check (`tests/lists_corpus.json`): `{"nan": true}` is NaN and
/// `null` is a key that is neither a number nor a string.
pub fn sort_key_order_json(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
    fn value(j: &serde_json::Value) -> Value {
        match j {
            serde_json::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Value::Str(s.clone()),
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Array(a) => Value::List(a.iter().map(value).collect()),
            serde_json::Value::Object(_) => Value::Num(f64::NAN),
            serde_json::Value::Null => Value::Unit,
        }
    }
    sort_key_order(&value(a), &value(b))
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

/// A `Bool` and nothing else: `!`, `&&`, `||` and an `if` condition do not use JavaScript's truthiness.
fn boolean(v: &Value, what: &str, span: Span) -> Result<bool, Stop> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(fault(
            "replay-type-error",
            format!("{what} needs a bool, got {}", describe(other)),
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

fn finite_of(n: Option<f64>, span: Span) -> Result<Value, Stop> {
    match n {
        Some(n) => Ok(Value::Num(n)),
        None => finite(f64::NAN, span),
    }
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

/// `+ - * /` on two numbers, as the language has them: a result that is not finite (a division by zero, an overflow) is
/// `None`, which `binary` reports as `replay-non-finite` and the emitted JavaScript throws (`__cui_fin`).
/// `tests/arith_corpus.json` is checked against this and against that JavaScript, comparing bits.
pub fn arith(op: &str, a: f64, b: f64) -> Option<f64> {
    let n = match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

fn binary(op: IrBinOp, l: Value, r: Value, span: Span) -> Result<Value, Stop> {
    use IrBinOp::*;
    match op {
        Add => match (&l, &r) {
            (Value::Num(a), Value::Num(b)) => finite_of(arith("+", *a, *b), span),
            (Value::Str(_), _) | (_, Value::Str(_)) => {
                Ok(Value::Str(concat_form(&l, span)? + &concat_form(&r, span)?))
            }
            _ => Err(operands("+", &l, &r, span)),
        },
        Sub | Mul | Div => match (&l, &r) {
            (Value::Num(a), Value::Num(b)) => match op {
                Sub => finite_of(arith("-", *a, *b), span),
                Mul => finite_of(arith("*", *a, *b), span),
                _ => finite_of(arith("/", *a, *b), span),
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
        And | Or => match (&l, &r) {
            (Value::Bool(a), Value::Bool(b)) => {
                Ok(Value::Bool(if op == And { *a && *b } else { *a || *b }))
            }
            _ => Err(operands(if op == And { "&&" } else { "||" }, &l, &r, span)),
        },
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
