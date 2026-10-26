//! Evaluating a closed expression: one that reads no state, no clock, no store and no network, only literals and the names it is given. The build uses
//! it to write the elements of a `For` over a `data` file (docs/data.md section 3), so the value of `json.get(p, "name", "")` in a built page is the
//! one replay computes and the one the page's script computes: there is one evaluator, and this is a door into it.

use std::collections::BTreeMap;

use cancho_ui_ir::ir::IrExpr;

use crate::eval::{Ctx, Journal, eval};
use crate::locale::LocaleState;
use crate::router::Router;
use crate::value::Value;

/// A value a closed expression can have: a text, a number, a bool, or a list of them.
#[derive(Debug, Clone, PartialEq)]
pub enum Closed {
    Str(String),
    Num(f64),
    Bool(bool),
    List(Vec<Closed>),
}

fn to_value(c: &Closed) -> Value {
    match c {
        Closed::Str(s) => Value::Str(s.clone()),
        Closed::Num(n) => Value::Num(*n),
        Closed::Bool(b) => Value::Bool(*b),
        Closed::List(l) => Value::List(l.iter().map(to_value).collect()),
    }
}

fn from_value(v: &Value) -> Option<Closed> {
    Some(match v {
        Value::Str(s) => Closed::Str(s.clone()),
        Value::Num(n) if n.is_finite() => Closed::Num(*n),
        Value::Bool(b) => Closed::Bool(*b),
        Value::List(l) => Closed::List(l.iter().map(from_value).collect::<Option<Vec<_>>>()?),
        _ => return None,
    })
}

/// The value of `expr` with the names `locals` bound, or `None` when it needs anything else (a state field, a name nobody gave, a fault).
pub fn eval_closed(expr: &IrExpr, locals: &[(String, Closed)]) -> Option<Closed> {
    let state = BTreeMap::new();
    let mut storage = BTreeMap::new();
    let mut memory = BTreeMap::new();
    let mut router = Router::new("/");
    let mut credentials: Vec<String> = Vec::new();
    let mut locale = LocaleState {
        lang: "en".to_string(),
        langs: vec!["en".to_string()],
        catalogs: Vec::new(),
    };
    let mut journal = Journal::default();
    let mut ctx = Ctx {
        state: &state,
        clock: 0.0,
        storage: &mut storage,
        storage_ns: "",
        storage_quota: None,
        storage_memory: &mut memory,
        router: &mut router,
        credentials: &mut credentials,
        locale: &mut locale,
        journal: &mut journal,
        locals: locals
            .iter()
            .map(|(n, c)| (n.clone(), to_value(c)))
            .collect(),
        init: false,
    };
    eval(&mut ctx, expr).ok().as_ref().and_then(from_value)
}
