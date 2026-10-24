//! `block` and `run` (docs/handler-sequences.md): a block is a named sequence of statements, and `run Name;` is replaced by
//! those statements *before* lowering, so the IR, replay and the page see a handler exactly as if it had been written out.
//! What is checked here is what the expansion would hide: a `run` of nothing, two blocks of one name, a block named like a state
//! field or an event, a block that runs itself, and an expansion that would grow without bound.

use std::collections::{BTreeMap, BTreeSet};

use cancho_ui_syntax::ast::{Component, Stmt};
use cancho_ui_syntax::parser::MAX_STMT_DEPTH;
use cancho_ui_syntax::span::Span;

use crate::lower::LowerError;

/// The most statements one handler or block may have after its `run`s are replaced (the nested ones counted).
pub const MAX_EXPANDED_STMTS: usize = 10_000;

/// How deep the expansion may go (docs/nesting.md section 5): blocks that run blocks, and `if`s inside what they run, counted together. The
/// expansion happens after parsing, so the parser's bound on statements does not cover a chain of blocks (2,000 blocks that each run the next
/// overflowed the stack of a 2 MiB thread).
pub const MAX_EXPANSION_DEPTH: usize = MAX_STMT_DEPTH;

fn too_deep(span: Span) -> LowerError {
    refuse(
        "block-expansion-too-deep",
        format!(
            "after its `run`s are replaced, blocks run blocks and statements nest more than {MAX_EXPANSION_DEPTH} deep; flatten the chain of blocks"
        ),
        span,
    )
}

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::BlockRefused {
        code,
        message,
        span,
    }
}

/// The names of the blocks `stmts` runs directly, with the span of each `run`.
fn runs(stmts: &[Stmt], out: &mut Vec<(String, Span)>) {
    for s in stmts {
        match s {
            Stmt::Run { name, span } => out.push((name.clone(), *span)),
            Stmt::If { then, else_, .. } => {
                runs(then, out);
                runs(else_, out);
            }
            Stmt::Assign { .. } | Stmt::Expr { .. } => {}
        }
    }
}

/// The statements of a component once its `run`s are replaced: one list per handler and one per block, in source order.
pub struct Expanded {
    pub handlers: Vec<Vec<Stmt>>,
    pub blocks: Vec<Vec<Stmt>>,
}

/// Check the blocks of `c` and return, for each handler (in order) and each block (in order), its statements with every `run`
/// replaced. The handlers' bodies are what lowering lowers; the blocks' are lowered on their own, to be scope-checked without
/// the parameters of any handler.
pub fn expand(c: &Component) -> Result<Expanded, LowerError> {
    let mut by_name: BTreeMap<&str, &[Stmt]> = BTreeMap::new();
    for b in &c.blocks {
        if by_name.insert(&b.name, &b.body.stmts).is_some() {
            return Err(refuse(
                "block-duplicate",
                format!("the block `{}` is declared twice", b.name),
                b.span,
            ));
        }
        if c.state.iter().any(|s| s.name == b.name) || c.events.iter().any(|e| e.name == b.name) {
            return Err(refuse(
                "block-name-conflict",
                format!(
                    "the block `{}` has the name of a state field or an event of this component",
                    b.name
                ),
                b.span,
            ));
        }
    }
    let mut all: Vec<(String, Span)> = Vec::new();
    for e in &c.events {
        runs(&e.body.stmts, &mut all);
    }
    for b in &c.blocks {
        runs(&b.body.stmts, &mut all);
    }
    for (name, span) in &all {
        if !by_name.contains_key(name.as_str()) {
            return Err(refuse(
                "block-unknown",
                format!("`run {name};` names a block this component does not declare"),
                *span,
            ));
        }
    }
    // A cycle: a block reached again while it is still being explored. Each block is explored once (`done`), so a chain of
    // blocks that each run the next twice is not walked 2^n times.
    let mut done: BTreeSet<&str> = BTreeSet::new();
    for b in &c.blocks {
        let mut path = vec![b.name.as_str()];
        cycle(&b.name, &by_name, &mut path, &mut done)?;
    }
    let mut handlers = Vec::with_capacity(c.events.len());
    for e in &c.events {
        let mut budget = MAX_EXPANDED_STMTS;
        handlers.push(replace(&e.body.stmts, &by_name, &mut budget, e.span, 0)?);
    }
    let mut blocks = Vec::with_capacity(c.blocks.len());
    for b in &c.blocks {
        let mut budget = MAX_EXPANDED_STMTS;
        blocks.push(replace(&b.body.stmts, &by_name, &mut budget, b.span, 0)?);
    }
    Ok(Expanded { handlers, blocks })
}

fn cycle<'a>(
    name: &'a str,
    by_name: &BTreeMap<&'a str, &'a [Stmt]>,
    path: &mut Vec<&'a str>,
    done: &mut BTreeSet<&'a str>,
) -> Result<(), LowerError> {
    if done.contains(name) {
        return Ok(());
    }
    let Some(body) = by_name.get(name) else {
        return Ok(());
    };
    let mut direct = Vec::new();
    runs(body, &mut direct);
    for (next, span) in &direct {
        if path.len() >= MAX_EXPANSION_DEPTH {
            return Err(too_deep(*span));
        }
        if path.contains(&next.as_str()) {
            return Err(refuse(
                "block-cycle",
                format!(
                    "the block `{}` runs itself ({} then `{next}`); its expansion would never end",
                    path[0],
                    path.join(" then ")
                ),
                *span,
            ));
        }
        if let Some((key, _)) = by_name.get_key_value(next.as_str()) {
            path.push(key);
            cycle(key, by_name, path, done)?;
            path.pop();
        }
    }
    done.insert(name);
    Ok(())
}

/// `stmts` with each `run` replaced by the statements of its block (themselves replaced). `budget` is what is left of the
/// statements this expansion may still produce; `depth` is how many `run`s and `if`s are open around `stmts`.
fn replace(
    stmts: &[Stmt],
    by_name: &BTreeMap<&str, &[Stmt]>,
    budget: &mut usize,
    owner: Span,
    depth: usize,
) -> Result<Vec<Stmt>, LowerError> {
    if depth > MAX_EXPANSION_DEPTH {
        return Err(too_deep(owner));
    }
    let mut out = Vec::new();
    for s in stmts {
        match s {
            Stmt::Run { name, .. } => {
                // A `run` counts too, so blocks that run empty blocks many times cannot make the expansion slow.
                take(budget, owner)?;
                let body = by_name.get(name.as_str()).copied().unwrap_or(&[]);
                out.extend(replace(body, by_name, budget, owner, depth + 1)?);
            }
            Stmt::If {
                cond,
                then,
                else_,
                span,
            } => {
                take(budget, owner)?;
                out.push(Stmt::If {
                    cond: cond.clone(),
                    then: replace(then, by_name, budget, owner, depth + 1)?,
                    else_: replace(else_, by_name, budget, owner, depth + 1)?,
                    span: *span,
                });
            }
            leaf => {
                take(budget, owner)?;
                out.push(leaf.clone());
            }
        }
    }
    Ok(out)
}

fn take(budget: &mut usize, owner: Span) -> Result<(), LowerError> {
    if *budget == 0 {
        return Err(refuse(
            "block-expansion-too-large",
            format!(
                "after its `run`s are replaced this has more than {MAX_EXPANDED_STMTS} statements; blocks that run blocks several times grow quickly, so split the work differently"
            ),
            owner,
        ));
    }
    *budget -= 1;
    Ok(())
}
