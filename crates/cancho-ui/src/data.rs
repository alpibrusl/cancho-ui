//! `data Name { file: ".." sha256: ".." }` at the command line (docs/data.md): the file is read when the page is built, checked (inside the program's
//! directory, at most 1 MB, UTF-8 JSON that nests at most 32 deep, the SHA-256 the program pinned), and every `For` over `data.items(Name, "path")` is
//! **written out** into the elements it stands for, before the program is checked, lowered, replayed, diffed or built: so every tool sees the same
//! elements, and the page has them in its HTML. What a text, a link, a label or a number of those elements is, is worked out by the evaluator replay
//! uses (`cancho_ui_replay::eval_closed`), so `json.get(p, "name", "")` is the same function at build, in replay and in the page's script.

use std::collections::BTreeMap;
use std::path::Path;

use cancho_ui_ir::data::{DEPTH_MAX, EXPANDED_MAX, FILE_MAX_BYTES, ROWS_MAX};
use cancho_ui_ir::ir::IrFile;
use cancho_ui_replay::{Closed, eval_closed, js_number};
use cancho_ui_syntax::Span;
use cancho_ui_syntax::ast::*;
use serde_json::Value as Json;

use crate::pipeline::{Diagnostic, import_diag};
use crate::theme::sha256_hex;

pub fn remediation(code: &str) -> String {
    match code {
        "data-hash-mismatch" => "`sha256` is the SHA-256 of the file as it is on disk (`sha256sum <file>`). A file that changed is a change of the program: read it again, then write the new hash.".to_string(),
        "data-file-missing" => "A data file is a file inside the directory of the program: put it at that path, or correct the path.".to_string(),
        "data-too-large" => "A data file is at most 1 MB: split it, or keep in it what the page shows.".to_string(),
        "data-malformed" => "A data file is UTF-8 text that is one JSON value (no comments, no trailing commas, no byte order mark).".to_string(),
        "data-too-deep" => "A data file nests at most 32 deep, as a program does.".to_string(),
        "data-shape-invalid" => "`data.items(Name, \"path\")` names an array of the file: `\"projects\"`, `\"measured.figures\"`, `\"paths.0.entries\"`.".to_string(),
        "data-too-many-rows" => "`data.items` answers at most 1000 items, as `json.items` does: take fewer, or split the list.".to_string(),
        "data-expansion-too-large" => "The `For`s over data of this program write more than 20,000 nodes: show less of it, or split the page.".to_string(),
        "data-unknown" => "Declare it: `data Name { file: \"data/x.json\" sha256: \"..\"; }`, in the entry file.".to_string(),
        "data-use-invalid" => "A data file is read by `For { over: data.items(Name, \"path\") as: item .. }`: two arguments, the name and a path in a string literal.".to_string(),
        "data-path-invalid" | "data-hash-invalid" | "data-duplicate" => "See docs/data.md section 2.".to_string(),
        _ => "See docs/data.md.".to_string(),
    }
}

/// A data file, read and checked.
pub struct Loaded {
    pub name: String,
    pub bytes: u64,
    pub json: Json,
}

fn refuse(code: &str, name: &str, message: String, span: Span, program: &Path) -> Box<Diagnostic> {
    Box::new(import_diag(
        code,
        name.to_string(),
        message,
        span,
        program,
        Some(remediation(code)),
    ))
}

fn depth(j: &Json) -> usize {
    match j {
        Json::Array(a) => 1 + a.iter().map(depth).max().unwrap_or(0),
        Json::Object(o) => 1 + o.values().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Reads and checks every declared file (relative to `dir`, the directory of the entry file).
pub fn load(
    decls: &[DataDecl],
    dir: &Path,
    program: &Path,
) -> Result<Vec<Loaded>, Box<Diagnostic>> {
    let root = dir.canonicalize().ok();
    let mut out = Vec::new();
    for d in decls {
        // names and paths are checked as text when the program is lowered; here they are looked at again so that a program refused there is not read
        if cancho_ui_ir::data::path_ok(&d.file).is_err() {
            continue;
        }
        let full = dir.join(&d.file);
        // a link to a file elsewhere is not the file of the site: the real path stays inside the directory of the program
        let inside = match (&root, full.canonicalize()) {
            (Some(r), Ok(f)) => f.starts_with(r),
            _ => false,
        };
        let meta = std::fs::metadata(&full).ok().filter(|m| m.is_file());
        let Some(meta) = meta.filter(|_| inside) else {
            return Err(refuse(
                "data-file-missing",
                &d.name,
                format!(
                    "data `{}`: `{}` is not a file inside the directory of the program ({})",
                    d.name,
                    d.file,
                    dir.display()
                ),
                d.span,
                program,
            ));
        };
        if meta.len() > FILE_MAX_BYTES {
            return Err(refuse(
                "data-too-large",
                &d.name,
                format!(
                    "data `{}`: `{}` is {} bytes; the most a data file may be is {FILE_MAX_BYTES}",
                    d.name,
                    d.file,
                    meta.len()
                ),
                d.span,
                program,
            ));
        }
        let bytes = std::fs::read(&full).map_err(|e| {
            refuse(
                "data-file-missing",
                &d.name,
                format!("data `{}`: `{}` cannot be read: {e}", d.name, d.file),
                d.span,
                program,
            )
        })?;
        let actual = sha256_hex(&bytes);
        if actual != d.sha256.to_ascii_lowercase() {
            return Err(refuse(
                "data-hash-mismatch",
                &d.name,
                format!(
                    "data `{}`: `{}` has the SHA-256 {actual}, and the program pins {}",
                    d.name, d.file, d.sha256
                ),
                d.span,
                program,
            ));
        }
        let text = std::str::from_utf8(&bytes).map_err(|e| {
            refuse(
                "data-malformed",
                &d.name,
                format!("data `{}`: `{}` is not UTF-8 text ({e})", d.name, d.file),
                d.span,
                program,
            )
        })?;
        let json: Json = serde_json::from_str(text).map_err(|e| {
            let deep = e.to_string().contains("recursion limit");
            refuse(
                if deep {
                    "data-too-deep"
                } else {
                    "data-malformed"
                },
                &d.name,
                format!(
                    "data `{}`: `{}` is not {}: {e}",
                    d.name,
                    d.file,
                    if deep { "within 32 levels" } else { "JSON" }
                ),
                d.span,
                program,
            )
        })?;
        if depth(&json) > DEPTH_MAX {
            return Err(refuse(
                "data-too-deep",
                &d.name,
                format!(
                    "data `{}`: `{}` nests {} deep; at most {DEPTH_MAX}",
                    d.name,
                    d.file,
                    depth(&json)
                ),
                d.span,
                program,
            ));
        }
        out.push(Loaded {
            name: d.name.clone(),
            bytes: bytes.len() as u64,
            json,
        });
    }
    Ok(out)
}

// ---- the expansion ---------------------------------------------------------------------------------------------------------------------

/// What the walk of one program does: the data it reads, the nodes it has written, and what each file was used for.
struct Walk<'a> {
    data: &'a [Loaded],
    program: &'a Path,
    nodes: u64,
    /// per data name: items read, and the components that read them
    used: BTreeMap<String, (u64, Vec<String>)>,
    component: String,
}

type Env = Vec<(String, Closed)>;
type R<T> = Result<T, Box<Diagnostic>>;

const PURE: &[&str] = &[
    "json.get",
    "json.items",
    "filter",
    "map",
    "len",
    "lower",
    "upper",
    "trim",
    "fold",
    "concat",
    "join",
    "split",
    "lines",
    "take",
    "skip",
    "reverse",
    "contains",
    "starts_with",
    "ends_with",
    "num",
    "max",
    "min",
    "abs",
    "floor",
    "ceil",
    "round",
];

/// Whether an expression holds only what a closed evaluation may use: literals, names, operators, a pure builtin and a lambda in `filter` and `map`.
fn pure(e: &Expr) -> bool {
    match e {
        Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::StringLit { .. }
        | Expr::BoolLit { .. }
        | Expr::Ident { .. } => true,
        Expr::Call { path, args, .. } => {
            PURE.contains(&path.join(".").as_str()) && args.iter().all(pure)
        }
        Expr::Binary { lhs, rhs, .. } => pure(lhs) && pure(rhs),
        Expr::Unary { operand, .. } => pure(operand),
        Expr::If {
            cond, then, else_, ..
        } => pure(cond) && pure(then) && else_.as_deref().is_some_and(pure),
        Expr::ListLit { items, .. } => items.iter().all(pure),
        Expr::Lambda { body, .. } => pure(body),
        _ => false,
    }
}

fn names_in(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ident { name, .. } => out.push(name.clone()),
        Expr::Call { args, .. } => args.iter().for_each(|a| names_in(a, out)),
        Expr::Binary { lhs, rhs, .. } => {
            names_in(lhs, out);
            names_in(rhs, out);
        }
        Expr::Unary { operand, .. } => names_in(operand, out),
        Expr::If {
            cond, then, else_, ..
        } => {
            names_in(cond, out);
            names_in(then, out);
            if let Some(e) = else_ {
                names_in(e, out);
            }
        }
        Expr::ListLit { items, .. } => items.iter().for_each(|a| names_in(a, out)),
        Expr::Lambda { body, .. } => names_in(body, out),
        _ => {}
    }
}

fn has_data_call(e: &Expr) -> bool {
    match e {
        Expr::Call { path, args, .. } => {
            path.first().is_some_and(|p| p == "data") || args.iter().any(has_data_call)
        }
        Expr::Binary { lhs, rhs, .. } => has_data_call(lhs) || has_data_call(rhs),
        Expr::Unary { operand, .. } => has_data_call(operand),
        Expr::If {
            cond, then, else_, ..
        } => {
            has_data_call(cond)
                || has_data_call(then)
                || else_.as_deref().is_some_and(has_data_call)
        }
        Expr::ListLit { items, .. } => items.iter().any(has_data_call),
        Expr::Lambda { body, .. } => has_data_call(body),
        _ => false,
    }
}

/// Whether the expression reads a name the walk has bound, or a data file: then what it says is the walk's to say.
fn touched(e: &Expr, env: &Env) -> bool {
    let mut names = Vec::new();
    names_in(e, &mut names);
    has_data_call(e) || names.iter().any(|n| env.iter().any(|(k, _)| k == n))
}

fn literal(c: &Closed, span: Span) -> Expr {
    match c {
        Closed::Str(s) => Expr::StringLit {
            value: s.clone(),
            span,
        },
        Closed::Bool(b) => Expr::BoolLit { value: *b, span },
        Closed::Num(n) if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_992.0 => Expr::IntLit {
            value: *n as i64,
            span,
        },
        Closed::Num(n) => Expr::FloatLit { value: *n, span },
        Closed::List(l) => Expr::ListLit {
            items: l.iter().map(|c| literal(c, span)).collect(),
            span,
        },
    }
}

fn text_of(c: &Closed) -> Option<String> {
    match c {
        Closed::Str(s) => Some(s.clone()),
        Closed::Bool(b) => Some(b.to_string()),
        Closed::Num(n) if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_992.0 => {
            Some(format!("{}", *n as i64))
        }
        Closed::Num(n) => js_number(*n),
        Closed::List(_) => None,
    }
}

impl Walk<'_> {
    fn err(&self, code: &str, message: String, span: Span) -> Box<Diagnostic> {
        refuse(code, "", message, span, self.program)
    }

    fn count(&mut self, n: u64, span: Span) -> R<()> {
        self.nodes += n;
        if self.nodes > EXPANDED_MAX {
            return Err(self.err(
                "data-expansion-too-large",
                format!(
                    "the `For`s over data of this program write more than {EXPANDED_MAX} nodes"
                ),
                span,
            ));
        }
        Ok(())
    }

    /// `data.items(Name, "path")` anywhere in the expression becomes the list of the compact JSON texts of the items, which is what `json.items` answers.
    fn resolve(&mut self, e: &Expr) -> R<Expr> {
        Ok(match e {
            Expr::Call { path, args, span } if path.first().is_some_and(|p| p == "data") => {
                if path.as_slice() != ["data", "items"] {
                    return Err(self.err(
                        "data-use-invalid",
                        format!(
                            "`{}` is not a function of data: there is `data.items(Name, \"path\")`",
                            path.join(".")
                        ),
                        *span,
                    ));
                }
                let (Some(Expr::Ident { name, .. }), Some(Expr::StringLit { value: at, .. }), 2) =
                    (args.first(), args.get(1), args.len())
                else {
                    return Err(self.err("data-use-invalid", "`data.items` takes the name of a data file and a path in a string literal: `data.items(Ecosystem, \"projects\")`".to_string(), *span));
                };
                let Some(file) = self.data.iter().find(|d| &d.name == name) else {
                    return Err(self.err(
                        "data-unknown",
                        format!("`data.items({name}, ..)`: the program declares no `data {name}`"),
                        *span,
                    ));
                };
                // the same path language as `json.get` and `json.items`
                let items = match at_path(&file.json, at) {
                    Some(Json::Array(a)) => a.clone(),
                    _ => return Err(self.err("data-shape-invalid", format!("`data.items({name}, \"{at}\")`: there is no array at `{at}` in the file"), *span)),
                };
                if items.len() > ROWS_MAX {
                    return Err(self.err(
                        "data-too-many-rows",
                        format!(
                            "`data.items({name}, \"{at}\")` has {} items; at most {ROWS_MAX}",
                            items.len()
                        ),
                        *span,
                    ));
                }
                let entry = self.used.entry(name.clone()).or_default();
                entry.0 += items.len() as u64;
                if !entry.1.contains(&self.component) {
                    entry.1.push(self.component.clone());
                }
                Expr::ListLit {
                    items: items
                        .iter()
                        .map(|v| Expr::StringLit {
                            value: v.to_string(),
                            span: *span,
                        })
                        .collect(),
                    span: *span,
                }
            }
            Expr::Call { path, args, span } => Expr::Call {
                path: path.clone(),
                args: args.iter().map(|a| self.resolve(a)).collect::<R<_>>()?,
                span: *span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op: *op,
                lhs: Box::new(self.resolve(lhs)?),
                rhs: Box::new(self.resolve(rhs)?),
                span: *span,
            },
            Expr::Unary { op, operand, span } => Expr::Unary {
                op: *op,
                operand: Box::new(self.resolve(operand)?),
                span: *span,
            },
            Expr::If {
                cond,
                then,
                else_,
                span,
            } => Expr::If {
                cond: Box::new(self.resolve(cond)?),
                then: Box::new(self.resolve(then)?),
                else_: else_
                    .as_ref()
                    .map(|e| self.resolve(e).map(Box::new))
                    .transpose()?,
                span: *span,
            },
            Expr::ListLit { items, span } => Expr::ListLit {
                items: items.iter().map(|a| self.resolve(a)).collect::<R<_>>()?,
                span: *span,
            },
            Expr::Lambda { param, body, span } => Expr::Lambda {
                param: param.clone(),
                body: Box::new(self.resolve(body)?),
                span: *span,
            },
            other => other.clone(),
        })
    }

    /// The value of a closed expression, or `None`.
    fn fold(&self, e: &Expr, env: &Env) -> Option<Closed> {
        if !pure(e) {
            return None;
        }
        let ir = cancho_ui_ir::lower::lower_expr_closed(e).ok()?;
        eval_closed(&ir, env)
    }

    /// The expression with its data resolved, the names the walk has bound replaced by their values, and every part that is closed worked out.
    fn simplify(&mut self, e: &Expr, env: &Env) -> R<Expr> {
        let e = self.resolve(e)?;
        let e = subst(&e, env);
        Ok(self.fold_parts(&e))
    }

    /// `simplify` for what the walk has a say in (it reads a bound name or a data file); anything else stays as the program wrote it.
    fn maybe(&mut self, e: &Expr, env: &Env) -> R<Expr> {
        if touched(e, env) {
            self.simplify(e, env)
        } else {
            Ok(e.clone())
        }
    }

    fn fold_parts(&self, e: &Expr) -> Expr {
        if let Some(c) = self.fold(e, &Vec::new()) {
            return literal(&c, e.span());
        }
        match e {
            Expr::Call { path, args, span } => Expr::Call {
                path: path.clone(),
                args: args.iter().map(|a| self.fold_parts(a)).collect(),
                span: *span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op: *op,
                lhs: Box::new(self.fold_parts(lhs)),
                rhs: Box::new(self.fold_parts(rhs)),
                span: *span,
            },
            Expr::Unary { op, operand, span } => Expr::Unary {
                op: *op,
                operand: Box::new(self.fold_parts(operand)),
                span: *span,
            },
            Expr::If {
                cond,
                then,
                else_,
                span,
            } => Expr::If {
                cond: Box::new(self.fold_parts(cond)),
                then: Box::new(self.fold_parts(then)),
                else_: else_.as_ref().map(|x| Box::new(self.fold_parts(x))),
                span: *span,
            },
            other => other.clone(),
        }
    }

    fn nodes(&mut self, nodes: &[ViewNode], env: &Env) -> R<Vec<ViewNode>> {
        let mut out = Vec::new();
        for n in nodes {
            self.node(n, env, &mut out)?;
        }
        Ok(out)
    }

    fn node(&mut self, n: &ViewNode, env: &Env, out: &mut Vec<ViewNode>) -> R<()> {
        match n {
            ViewNode::Text {
                value: TextValue::Interpolated(parts),
                span,
            } => {
                self.count(1, *span)?;
                let mut merged: Vec<TextPart> = Vec::new();
                for p in parts {
                    let part = match p {
                        TextPart::Literal(s) => TextPart::Literal(s.clone()),
                        TextPart::Expr { expr, span } => {
                            let e = self.maybe(expr, env)?;
                            match self.fold(&e, &Vec::new()).as_ref().and_then(text_of) {
                                Some(t) => TextPart::Literal(t),
                                None => TextPart::Expr {
                                    expr: e,
                                    span: *span,
                                },
                            }
                        }
                    };
                    match (merged.last_mut(), &part) {
                        (Some(TextPart::Literal(a)), TextPart::Literal(b)) => a.push_str(b),
                        _ => merged.push(part),
                    }
                }
                out.push(ViewNode::Text {
                    value: TextValue::Interpolated(merged),
                    span: *span,
                });
            }
            ViewNode::Element {
                kind,
                accessibility,
                bindings,
                children,
                span,
            } => {
                self.count(1, *span)?;
                let mut acc = accessibility.clone();
                if let Some(e) = &accessibility.label_expr {
                    let e = self.maybe(e, env)?;
                    if let Expr::StringLit { value, .. } = &e {
                        acc.label = Some(value.clone());
                        acc.label_expr = None;
                    } else {
                        acc.label_expr = Some(e);
                    }
                }
                let mut bs = Vec::new();
                for b in bindings {
                    let value = match &b.value {
                        BindingValue::Prop(e) => {
                            let e = self.maybe(e, env)?;
                            match e {
                                Expr::StringLit { value, .. } if b.key == "to" => {
                                    BindingValue::String(value)
                                }
                                other => BindingValue::Prop(other),
                            }
                        }
                        BindingValue::StateRef(name) => {
                            match env.iter().rev().find(|(k, _)| k == name) {
                                Some((_, Closed::Str(s))) => BindingValue::String(s.clone()),
                                _ => b.value.clone(),
                            }
                        }
                        BindingValue::EventCall { name, args } => BindingValue::EventCall {
                            name: name.clone(),
                            args: args.iter().map(|a| self.maybe(a, env)).collect::<R<_>>()?,
                        },
                        other => other.clone(),
                    };
                    bs.push(Binding {
                        key: b.key.clone(),
                        value,
                        span: b.span,
                    });
                }
                out.push(ViewNode::Element {
                    kind: kind.clone(),
                    accessibility: acc,
                    bindings: bs,
                    children: self.nodes(children, env)?,
                    span: *span,
                });
            }
            ViewNode::If {
                cond,
                children,
                span,
            } => {
                let mine = touched(cond, env);
                let c = self.simplify(cond, env)?;
                match (&c, mine) {
                    (Expr::BoolLit { value: true, .. }, true) => {
                        out.extend(self.nodes(children, env)?)
                    }
                    (Expr::BoolLit { value: false, .. }, true) => {}
                    _ => {
                        self.count(1, *span)?;
                        out.push(ViewNode::If {
                            cond: c,
                            children: self.nodes(children, env)?,
                            span: *span,
                        });
                    }
                }
            }
            ViewNode::For {
                over,
                binding,
                children,
                span,
            } => {
                let mine = touched(over, env);
                let list = self.simplify(over, env)?;
                let items: Option<Vec<Closed>> = match (&list, mine) {
                    (Expr::ListLit { items, .. }, true) => {
                        items.iter().map(|i| self.fold(i, &Vec::new())).collect()
                    }
                    _ => None,
                };
                match items {
                    Some(items) => {
                        for item in items {
                            let mut inner = env.clone();
                            inner.push((binding.clone(), item));
                            self.count(1, *span)?;
                            out.extend(self.nodes(children, &inner)?);
                        }
                    }
                    None => {
                        // a `For` over state stays as it is; the name it binds hides one of the walk's
                        let inner: Env =
                            env.iter().filter(|(k, _)| k != binding).cloned().collect();
                        self.count(1, *span)?;
                        out.push(ViewNode::For {
                            over: list,
                            binding: binding.clone(),
                            children: self.nodes(children, &inner)?,
                            span: *span,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// The value at a path of a JSON value, in the language of `json.get`: names and indexes joined by `.`.
fn at_path<'a>(j: &'a Json, path: &str) -> Option<&'a Json> {
    let mut cur = j;
    if !path.is_empty() {
        for part in path.split('.') {
            cur = match cur {
                Json::Array(items) => {
                    let b = part.as_bytes();
                    let ok = !b.is_empty()
                        && b.len() <= 9
                        && b.iter().all(u8::is_ascii_digit)
                        && (b[0] != b'0' || b.len() == 1);
                    items.get(if ok {
                        part.parse::<usize>().ok()?
                    } else {
                        return None;
                    })?
                }
                Json::Object(map) => map.get(part)?,
                _ => return None,
            };
        }
    }
    Some(cur)
}

/// The names bound by the walk replaced by their values (a lambda's own parameter hides one).
fn subst(e: &Expr, env: &Env) -> Expr {
    match e {
        Expr::Ident { name, span } => match env.iter().rev().find(|(k, _)| k == name) {
            Some((_, c)) => literal(c, *span),
            None => e.clone(),
        },
        Expr::Call { path, args, span } => Expr::Call {
            path: path.clone(),
            args: args.iter().map(|a| subst(a, env)).collect(),
            span: *span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op: *op,
            lhs: Box::new(subst(lhs, env)),
            rhs: Box::new(subst(rhs, env)),
            span: *span,
        },
        Expr::Unary { op, operand, span } => Expr::Unary {
            op: *op,
            operand: Box::new(subst(operand, env)),
            span: *span,
        },
        Expr::If {
            cond,
            then,
            else_,
            span,
        } => Expr::If {
            cond: Box::new(subst(cond, env)),
            then: Box::new(subst(then, env)),
            else_: else_.as_ref().map(|x| Box::new(subst(x, env))),
            span: *span,
        },
        Expr::ListLit { items, span } => Expr::ListLit {
            items: items.iter().map(|a| subst(a, env)).collect(),
            span: *span,
        },
        Expr::Lambda { param, body, span } => {
            let inner: Env = env.iter().filter(|(k, _)| k != param).cloned().collect();
            Expr::Lambda {
                param: param.clone(),
                body: Box::new(subst(body, &inner)),
                span: *span,
            }
        }
        other => other.clone(),
    }
}

fn body_span(n: &ViewNode) -> Span {
    match n {
        ViewNode::Element { span, .. }
        | ViewNode::Text { span, .. }
        | ViewNode::For { span, .. }
        | ViewNode::If { span, .. } => *span,
    }
}

fn body_accessibility(n: &ViewNode) -> Accessibility {
    match n {
        ViewNode::Element { accessibility, .. } => accessibility.clone(),
        other => Accessibility {
            role: None,
            label: None,
            label_expr: None,
            span: body_span(other),
        },
    }
}

/// Writes out the `For`s over data of every component of the file. Returns, per data name, the items read and the components that read them.
pub fn expand(
    file: &mut File,
    data: &[Loaded],
    program: &Path,
) -> R<BTreeMap<String, (u64, Vec<String>)>> {
    let mut walk = Walk {
        data,
        program,
        nodes: 0,
        used: BTreeMap::new(),
        component: String::new(),
    };
    for c in file.components.iter_mut() {
        walk.component = c.name.clone();
        let body = c.view.body.clone();
        let mut out = Vec::new();
        walk.node(&body, &Vec::new(), &mut out)?;
        c.view.body = match out.len() {
            1 => out.remove(0),
            // a view that is itself a `For` or an `If` over data writes several nodes, or none: they go in a column
            _ => ViewNode::Element {
                kind: ElementKind::Column,
                accessibility: body_accessibility(&body),
                bindings: Vec::new(),
                children: out,
                span: body_span(&body),
            },
        };
    }
    Ok(walk.used)
}

/// What the pipeline writes into the IR once the program is lowered: the size of each file, the items read and the components that read them.
pub fn record(ir: &mut IrFile, loaded: &[Loaded], used: &BTreeMap<String, (u64, Vec<String>)>) {
    for d in ir.data.iter_mut() {
        if let Some(l) = loaded.iter().find(|l| l.name == d.name) {
            d.bytes = l.bytes;
        }
        if let Some((rows, comps)) = used.get(&d.name) {
            d.rows = *rows;
            d.components = comps.clone();
        }
    }
}
