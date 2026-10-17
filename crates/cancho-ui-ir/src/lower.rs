//! AST → IR lowering + capability checking for the Tier 4 supported
//! subset.
//!
//! Lowering is structural: every AST node has a one-to-one IR
//! equivalent. `lower_file` produces an IR then runs the
//! capability pass against it (every reachable effect's
//! capability must be in the component's `requires` list, and
//! every declared capability must have at least one reachable
//! use). Errors from either pass come back through the same
//! `LowerError` enum.

use cancho_ui_syntax::ast::{
    Binding, BindingValue, BinOp, CapabilityRef, Component, ElementKind, EventHandler, Expr, File,
    MatchArm, Pattern, Role, Stmt, StmtBlock, StateField, TextPart, TextValue, Type, UnOp, ViewNode,
};
use cancho_ui_syntax::span::{Position, Span};
use std::collections::BTreeSet;
use thiserror::Error;

use crate::ir::*;
use crate::serialize::schema_version;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum LowerError {
    #[error("internal: AST node has empty span")]
    EmptySpan { span: Span },

    #[error(
        "component `{component}` performs `{call_path}` which requires the `{capability}` capability, but `requires {capability}` is not declared; add `requires {capability};` to the component header"
    )]
    CapabilityMissing {
        component: String,
        capability: String,
        call_path: String,
        span: Span,
    },

    #[error(
        "component `{component}` declares `requires {capability};` but no reachable effect uses it"
    )]
    CapabilityUnused {
        component: String,
        capability: String,
        span: Span,
    },

    #[error(
        "`match` on `{field}` in component `{component}` is not exhaustive; missing arm(s): {missing:?}"
    )]
    MatchNonExhaustive {
        component: String,
        field: String,
        missing: Vec<String>,
        span: Span,
    },

    #[error(
        "a lambda (`x => expr`) is only supported as the second argument of `filter(list, pred)` or `map(list, transform)`; remove the lambda or move it into a `filter`/`map` call"
    )]
    LambdaUnsupported { span: Span },

    #[error(
        "`{name}` is misused here (expected {detail}); see Tier 11 call forms: `filter(list, x => cond)`, `map(list, x => body)`, `len(value)`"
    )]
    BuiltinMisuse { name: String, detail: String, span: Span },

    #[error(
        "the lambda body must be pure: it may not `await(...)` or call effectful/suspending builtins — list operations run synchronously inside the render pass"
    )]
    PureLambdaRequired { span: Span },
}

pub fn lower_file(file: &File) -> Result<IrFile, LowerError> {
    let mut next_id = 0u32;
    let mut components = Vec::with_capacity(file.components.len());
    for c in &file.components {
        components.push(lower_component(c, &mut next_id)?);
    }
    let ir = IrFile {
        schema: SCHEMA.to_string(),
        version: schema_version().to_string(),
        module: file.module.as_ref().map(|m| m.name.clone()),
        components,
    };
    check_capabilities(&ir)?;
    check_match_exhaustiveness(&ir)?;
    Ok(ir)
}

fn lower_component(c: &Component, next_id: &mut u32) -> Result<IrComponent, LowerError> {
    if c.span == Span::DUMMY {
        return Err(LowerError::EmptySpan { span: c.span });
    }
    let id = IrId(*next_id);
    *next_id += 1;
    let mut state = Vec::with_capacity(c.state.len());
    for s in &c.state {
        state.push(lower_state(s)?);
    }
    let requires = c.requires.iter().map(lower_capability).collect();
    let mut events = Vec::with_capacity(c.events.len());
    let mut reachable_effects: Vec<IrReachableEffect> = Vec::new();
    for e in &c.events {
        let (ir_event, effects) = lower_event(e)?;
        events.push(ir_event);
        reachable_effects.extend(effects);
    }
    let (view_body, view_effects) = lower_view_node_with_effects(&c.view.body)?;
    reachable_effects.extend(view_effects);
    let view = IrViewBlock { body: view_body, span: c.view.span };
    Ok(IrComponent {
        id,
        name: c.name.clone(),
        state,
        requires,
        reachable_effects,
        events,
        view,
        span: c.span,
    })
}

fn lower_state(s: &StateField) -> Result<IrStateField, LowerError> {
    Ok(IrStateField {
        name: s.name.clone(),
        ty: lower_type(&s.ty),
        initial: lower_expr(&s.initial)?,
        span: s.span,
    })
}

fn lower_type(t: &Type) -> IrType {
    match t {
        Type::Int => IrType::Int,
        Type::Float => IrType::Float,
        Type::Bool => IrType::Bool,
        Type::String => IrType::String,
        Type::Result { ok, err } => IrType::Result {
            ok: Box::new(lower_type(ok)),
            err: Box::new(lower_type(err)),
        },
        Type::Request { payload } => IrType::Request {
            payload: Box::new(lower_type(payload)),
        },
        Type::Option { some } => IrType::Option {
            some: Box::new(lower_type(some)),
        },
        Type::List { elem } => IrType::List {
            elem: Box::new(lower_type(elem)),
        },
    }
}

fn lower_capability(c: &CapabilityRef) -> IrCapabilityRef {
    IrCapabilityRef {
        name: c.name.clone(),
        span: c.span,
    }
}

fn lower_event(e: &EventHandler) -> Result<(IrEventHandler, Vec<IrReachableEffect>), LowerError> {
    let (body, effects) = lower_stmts_with_effects(&e.body)?;
    Ok((
        IrEventHandler {
            name: e.name.clone(),
            params: e.params.clone(),
            body,
            span: e.span,
        },
        effects,
    ))
}

fn lower_stmts_with_effects(
    block: &StmtBlock,
) -> Result<(Vec<IrStmt>, Vec<IrReachableEffect>), LowerError> {
    let mut out = Vec::with_capacity(block.stmts.len());
    let mut effects = Vec::new();
    for s in &block.stmts {
        let (ir, mut e) = lower_stmt(s)?;
        out.push(ir);
        effects.append(&mut e);
    }
    Ok((out, effects))
}

fn lower_stmts(block: &StmtBlock) -> Result<Vec<IrStmt>, LowerError> {
    Ok(lower_stmts_with_effects(block)?.0)
}

fn lower_stmt(s: &Stmt) -> Result<(IrStmt, Vec<IrReachableEffect>), LowerError> {
    match s {
        Stmt::Assign { target, value, span } => {
            let (v, effects) = lower_expr_with_effects(value)?;
            Ok((
                IrStmt::Assign {
                    target: target.clone(),
                    value: v,
                    span: *span,
                },
                effects,
            ))
        }
        Stmt::Expr { expr, span } => {
            let (e, effects) = lower_expr_with_effects(expr)?;
            Ok((
                IrStmt::Expr {
                    expr: e,
                    span: *span,
                },
                effects,
            ))
        }
    }
}

fn lower_expr(e: &Expr) -> Result<IrExpr, LowerError> {
    Ok(lower_expr_with_effects(e)?.0)
}

fn lower_expr_with_effects(
    e: &Expr,
) -> Result<(IrExpr, Vec<IrReachableEffect>), LowerError> {
    match e {
        Expr::IntLit { value, span } => Ok((IrExpr::IntLit { value: *value, span: *span }, vec![])),
        Expr::FloatLit { value, span } => {
            Ok((IrExpr::FloatLit { value: *value, span: *span }, vec![]))
        }
        Expr::StringLit { value, span } => Ok((
            IrExpr::StringLit { value: value.clone(), span: *span },
            vec![],
        )),
        Expr::BoolLit { value, span } => Ok((IrExpr::BoolLit { value: *value, span: *span }, vec![])),
        Expr::Ident { name, span } => Ok((IrExpr::Ident { name: name.clone(), span: *span }, vec![])),
        Expr::Lambda { span, .. } => {
            // `filter`/`map` consume their lambda before the
            // generic call path sees it; a lambda anywhere else
            // has no meaning (Tier 11 has no first-class
            // function values).
            Err(LowerError::LambdaUnsupported { span: *span })
        }
        Expr::Call { path, args, span } => {
            // Tier 11 call forms: `filter(list, x => cond)`,
            // `map(list, x => body)`, `len(value)`. They are pure
            // (no capability); the generic call path below does
            // not see them.
            if path.len() == 1 && path[0] == "filter" {
                if args.len() != 2 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "filter".to_string(),
                        detail: "exactly two arguments: filter(list, x => cond)".to_string(),
                        span: *span,
                    });
                }
                let list = Box::new(lower_expr_pure(&args[0])?);
                let (item, cond_body, cond_span) = match &args[1] {
                    Expr::Lambda { param, body, .. } => {
                        let lowered = lower_expr_pure(body)?;
                        (param.clone(), lowered, body.span())
                    }
                    _ => {
                        return Err(LowerError::BuiltinMisuse {
                            name: "filter".to_string(),
                            detail: "second argument must be a lambda: x => cond".to_string(),
                            span: args[1].span(),
                        });
                    }
                };
                return Ok((
                    IrExpr::Filter {
                        list,
                        item,
                        cond: Box::new(cond_body),
                        span: cond_span,
                    },
                    vec![],
                ));
            }
            if path.len() == 1 && path[0] == "map" {
                if args.len() != 2 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "map".to_string(),
                        detail: "exactly two arguments: map(list, x => body)".to_string(),
                        span: *span,
                    });
                }
                let list = Box::new(lower_expr_pure(&args[0])?);
                let (item, mapper_body, mapper_span) = match &args[1] {
                    Expr::Lambda { param, body, .. } => {
                        let lowered = lower_expr_pure(body)?;
                        (param.clone(), lowered, body.span())
                    }
                    _ => {
                        return Err(LowerError::BuiltinMisuse {
                            name: "map".to_string(),
                            detail: "second argument must be a lambda: x => body".to_string(),
                            span: args[1].span(),
                        });
                    }
                };
                return Ok((
                    IrExpr::Map {
                        list,
                        item,
                        body: Box::new(mapper_body),
                        span: mapper_span,
                    },
                    vec![],
                ));
            }
            if path.len() == 1 && path[0] == "len" {
                if args.len() != 1 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "len".to_string(),
                        detail: "exactly one argument: len(value)".to_string(),
                        span: *span,
                    });
                }
                return Ok((
                    IrExpr::Len {
                        list: Box::new(lower_expr_pure(&args[0])?),
                        span: *span,
                    },
                    vec![],
                ));
            }
            let mut lowered_args = Vec::with_capacity(args.len());
            let mut effects = Vec::new();
            for a in args {
                let (ir, e) = lower_expr_with_effects(a)?;
                lowered_args.push(ir);
                effects.extend(e);
            }
            if let Some(capability) = builtin_capability(path) {
                effects.push(IrReachableEffect {
                    capability: capability.to_string(),
                    path: path.clone(),
                    span: *span,
                });
            }
            Ok((
                IrExpr::Call {
                    path: path.clone(),
                    args: lowered_args,
                    span: *span,
                },
                effects,
            ))
        }
        Expr::Binary { op, lhs, rhs, span } => {
            let (l, mut le) = lower_expr_with_effects(lhs)?;            let (r, mut re) = lower_expr_with_effects(rhs)?;
            le.append(&mut re);
            Ok((
                IrExpr::Binary {
                    op: lower_binop(*op),
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                    span: *span,
                },
                le,
            ))
        }
        Expr::Unary { op, operand, span } => {
            let (o, e) = lower_expr_with_effects(operand)?;
            Ok((
                IrExpr::Unary {
                    op: lower_unop(*op),
                    operand: Box::new(o),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Match { scrutinee, arms, span } => {
            let (ir_scrutinee, mut effects) = lower_expr_with_effects(scrutinee)?;
            let mut ir_arms = Vec::with_capacity(arms.len());
            for MatchArm { pattern, body, span: arm_span } in arms {
                let ir_pattern = lower_pattern(pattern);
                let (ir_body, mut e) = lower_expr_with_effects(body)?;
                effects.append(&mut e);
                ir_arms.push(IrMatchArm {
                    pattern: ir_pattern,
                    body: ir_body,
                    span: *arm_span,
                });
            }
            Ok((
                IrExpr::Match {
                    scrutinee: Box::new(ir_scrutinee),
                    arms: ir_arms,
                    span: *span,
                },
                effects,
            ))
        }
        Expr::Ok { value, span } => {
            let (ir_value, e) = lower_expr_with_effects(value)?;
            Ok((
                IrExpr::Ok {
                    value: Box::new(ir_value),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Err { reason, span } => {
            let (ir_reason, e) = lower_expr_with_effects(reason)?;
            Ok((
                IrExpr::Err {
                    reason: Box::new(ir_reason),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Await { request, span } => {
            let (ir_request, e) = lower_expr_with_effects(request)?;
            Ok((
                IrExpr::Await {
                    request: Box::new(ir_request),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Cancelled { span } => Ok((
            IrExpr::Cancelled { span: *span },
            Vec::new(),
        )),
        Expr::Some { value, span } => {
            let (ir_value, e) = lower_expr_with_effects(value)?;
            Ok((
                IrExpr::Some {
                    value: Box::new(ir_value),
                    span: *span,
                },
                e,
            ))
        }
        Expr::None { span } => Ok((IrExpr::None { span: *span }, Vec::new())),
        Expr::ListLit { items, span } => {
            let mut ir_items = Vec::with_capacity(items.len());
            let mut effects = Vec::new();
            for item in items {
                let (ir, mut e) = lower_expr_with_effects(item)?;
                effects.append(&mut e);
                ir_items.push(ir);
            }
            Ok((IrExpr::ListLit { items: ir_items, span: *span }, effects))
        }
    }
}

fn lower_pattern(p: &Pattern) -> IrPattern {
    match p {
        Pattern::Wildcard { span } => IrPattern::Wildcard { span: *span },
        Pattern::Ident { name, span } => IrPattern::Ident {
            name: name.clone(),
            span: *span,
        },
        Pattern::Variant { name, binding, span } => IrPattern::Variant {
            name: name.clone(),
            binding: binding.clone(),
            span: *span,
        },
        Pattern::UnitVariant { name, span } => IrPattern::UnitVariant {
            name: name.clone(),
            span: *span,
        },
    }
}

/// Phase 1 / Tier 4 hard-codes the capability for known builtin
/// calls. Phase 2 will replace this with a registered vocabulary
/// (`spec/security-model.md`) and a richer `capability-missing`
/// permission set.
fn builtin_capability(path: &[String]) -> Option<&'static str> {
    if path.len() == 2 && path[0] == "clock" && path[1] == "now" {
        Some("Clock")
    } else if path.len() == 2 && path[0] == "net" && path[1] == "search" {
        // Tier 9: `net.search(query)` is the asynchronous search
        // effect. `await(net.search(q))` resolves to Ok | Err |
        // Cancelled with an Ok payload of `List[String]`.
        Some("Network")
    } else if path.len() == 1 && path[0] == "sleep" {
        // Tier 7: `sleep(ms)` is a pure-runtime wait. It does
        // not need a user-declared capability; the runtime
        // scheduler is its capability.
        None
    } else {
        None
    }
}

fn lower_view_node(node: &ViewNode) -> Result<IrViewNode, LowerError> {
    Ok(lower_view_node_with_effects(node)?.0)
}

fn lower_view_node_with_effects(
    node: &ViewNode,
) -> Result<(IrViewNode, Vec<IrReachableEffect>), LowerError> {
    match node {
        ViewNode::Element { kind, accessibility, bindings, children, span } => {
            let mut lowered_children = Vec::with_capacity(children.len());
            let mut effects = Vec::new();
            for c in children {
                let (n, mut e) = lower_view_node_with_effects(c)?;
                lowered_children.push(n);
                effects.append(&mut e);
            }
            let mut lowered_bindings = Vec::with_capacity(bindings.len());
            for b in bindings {
                lowered_bindings.push(lower_binding(b)?);
            }
            Ok((
                IrViewNode::Element {
                    kind: lower_kind(kind),
                    accessibility: lower_accessibility(accessibility),
                    bindings: lowered_bindings,
                    children: lowered_children,
                    span: *span,
                },
                effects,
            ))
        }
        ViewNode::Text { value, span } => {
            let mut effects = Vec::new();
            let parts = match value {
                TextValue::Interpolated(parts) => {
                    let mut lowered = Vec::with_capacity(parts.len());
                    for p in parts {
                        match p {
                            TextPart::Literal(s) => lowered.push(IrTextPart::Literal(s.clone())),
                            TextPart::Expr { expr, span } => {
                                let _ = span;
                                let (ir, mut e) = lower_expr_with_effects(expr)?;
                                effects.append(&mut e);
                                lowered.push(IrTextPart::Expr(ir))
                            }
                        }
                    }
                    lowered
                }
            };
            Ok((IrViewNode::Text { parts, span: *span }, effects))
        }
        ViewNode::For { over, binding, children, span } => {
            // Tier 11: `over` is a general (pure) expression — a
            // state list, or a derived one via `filter(list, n =>
            // ...)`. Purity matters less here than in a predicate
            // (a suspending `over` cannot even render), so the
            // same rule applies: no effects, no suspends.
            let ir_over = lower_expr_pure(over)?;
            let mut lowered_children = Vec::with_capacity(children.len());
            let mut effects = Vec::new();
            for ch in children {
                let (n, mut e) = lower_view_node_with_effects(ch)?;
                lowered_children.push(n);
                effects.append(&mut e);
            }
            Ok((
                IrViewNode::For {
                    over: ir_over,
                    binding: binding.clone(),
                    children: lowered_children,
                    span: *span,
                },
                effects,
            ))
        }
    }
}

fn lower_kind(k: &ElementKind) -> IrElementKind {
    match k {
        ElementKind::Column => IrElementKind::Column,
        ElementKind::Row => IrElementKind::Row,
        ElementKind::Text => IrElementKind::Text,
        ElementKind::Button => IrElementKind::Button,
        ElementKind::TextField => IrElementKind::TextField,
        ElementKind::Named(s) => IrElementKind::Named(s.clone()),
    }
}

fn lower_accessibility(acc: &cancho_ui_syntax::ast::Accessibility) -> IrAccessibility {
    IrAccessibility {
        role: acc.role.as_ref().map(lower_role),
        label: acc.label.clone(),
    }
}

fn lower_role(r: &Role) -> IrRole {
    match r {
        Role::Text => IrRole::Text,
        Role::Heading => IrRole::Heading,
        Role::Button => IrRole::Button,
        Role::Other(s) => IrRole::Other(s.clone()),
    }
}

fn lower_binding(b: &Binding) -> Result<IrBinding, LowerError> {
    let value = match &b.value {
        BindingValue::String(s) => IrBindingValue::String(s.clone()),
        BindingValue::EventRef(name) => IrBindingValue::EventRef(name.clone()),
        BindingValue::Role(r) => IrBindingValue::Role(lower_role(r)),
        BindingValue::StateRef(name) => IrBindingValue::StateRef(name.clone()),
    };
    Ok(IrBinding {
        key: b.key.clone(),
        value,
        span: b.span,
    })
}

fn lower_binop(op: BinOp) -> IrBinOp {
    match op {
        BinOp::Add => IrBinOp::Add,
        BinOp::Sub => IrBinOp::Sub,
        BinOp::Mul => IrBinOp::Mul,
        BinOp::Div => IrBinOp::Div,
        BinOp::Eq => IrBinOp::Eq,
        BinOp::Ne => IrBinOp::Ne,
        BinOp::Lt => IrBinOp::Lt,
        BinOp::Le => IrBinOp::Le,
        BinOp::Gt => IrBinOp::Gt,
        BinOp::Ge => IrBinOp::Ge,
    }
}

fn lower_unop(op: UnOp) -> IrUnOp {
    match op {
        UnOp::Neg => IrUnOp::Neg,
    }
}

/// Walk the lowered IR's `reachable_effects` against the
/// component's `requires`. Refuses on `CapabilityMissing` and
/// `CapabilityUnused`.
fn check_capabilities(ir: &IrFile) -> Result<(), LowerError> {    for c in &ir.components {
        let declared: BTreeSet<String> = c.requires.iter().map(|r| r.name.clone()).collect();
        for eff in &c.reachable_effects {
            if !declared.contains(&eff.capability) {
                return Err(LowerError::CapabilityMissing {
                    component: c.name.clone(),
                    capability: eff.capability.clone(),
                    call_path: eff.path.join("."),
                    span: eff.span,
                });
            }
        }
        for req in &c.requires {
            let used = c
                .reachable_effects
                .iter()
                .any(|e| &e.capability == &req.name);
            if !used {
                return Err(LowerError::CapabilityUnused {
                    component: c.name.clone(),
                    capability: req.name.clone(),
                    span: req.span,
                });
            }
        }
    }
    Ok(())
}

// Marker so Position is referenced (avoids dead-code warnings if
// Position is only used by error variants in LowerError).
#[allow(dead_code)]
const _POSITION: Position = Position { line: 0, column: 0 };

/// Phase 2: the `match-non-exhaustive` check.
///
/// A `match` whose scrutinee is a state field of type
/// `Result[T, E]` must cover every constructor the runtime can
/// produce, unless a catch-all arm (`_`, or a bare identifier —
/// Tier 6 semantics: it binds the whole value, so it matches
/// anything) is present. Required constructors:
///
/// - `{Ok, Err}` for a plain `Result` field,
/// - `{Ok, Err, Cancelled}` when the field is ever assigned from
///   an `await(...)` expression (an awaited `Request[T]`
///   resolves to all three) or when an arm matches `Cancelled`.
///
/// A `match` on anything other than a state field identifier is
/// not statically resolved here; the runtime throw (Tier 6)
/// remains the safety net for those. Phase 2's full type
/// inference will widen this check.
fn check_match_exhaustiveness(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
        let state_types: BTreeMap<String, &IrType> = c
            .state
            .iter()
            .map(|s| (s.name.clone(), &s.ty))
            .collect();
        let await_driven = await_driven_fields(c);

        for m in component_matches(c) {
            check_one_match(&c.name, m, &state_types, &await_driven)?;
        }
    }
    Ok(())
}

use std::collections::BTreeMap;

/// Every `IrExpr::Match` reachable from the component's event
/// handlers and its view (including nested inside arm bodies).
fn component_matches(c: &IrComponent) -> Vec<&IrExpr> {
    let mut out: Vec<&IrExpr> = Vec::new();
    for e in &c.events {
        for s in &e.body {
            stmt_matches(s, &mut out);
        }
    }
    view_matches(&c.view.body, &mut out);
    out
}

fn stmt_matches<'a>(s: &'a IrStmt, out: &mut Vec<&'a IrExpr>) {
    match s {
        IrStmt::Assign { value, .. } => expr_matches(value, out),
        IrStmt::Expr { expr, .. } => expr_matches(expr, out),
    }
}

fn expr_matches<'a>(e: &'a IrExpr, out: &mut Vec<&'a IrExpr>) {
    match e {
        IrExpr::Match { scrutinee, arms, .. } => {
            out.push(e);
            expr_matches(scrutinee, out);
            for a in arms {
                expr_matches(&a.body, out);
            }
        }
        IrExpr::Binary { lhs, rhs, .. } => {
            expr_matches(lhs, out);
            expr_matches(rhs, out);
        }
        IrExpr::Unary { operand, .. } => expr_matches(operand, out),
        IrExpr::Call { args, .. } => {
            for a in args {
                expr_matches(a, out);
            }
        }
        IrExpr::Ok { value, .. } => expr_matches(value, out),
        IrExpr::Err { reason, .. } => expr_matches(reason, out),
        IrExpr::Some { value, .. } => expr_matches(value, out),
        IrExpr::Await { request, .. } => expr_matches(request, out),
        IrExpr::ListLit { items, .. } => {
            for i in items {
                expr_matches(i, out);
            }
        }
        _ => {}
    }
}

fn view_matches<'a>(node: &'a IrViewNode, out: &mut Vec<&'a IrExpr>) {
    match node {
        IrViewNode::Element { children, .. } => {
            for ch in children {
                view_matches(ch, out);
            }
        }
        IrViewNode::Text { parts, .. } => {
            for p in parts {
                if let IrTextPart::Expr(e) = p {
                    expr_matches(e, out);
                }
            }
        }
        IrViewNode::For { over, children, .. } => {
            expr_matches(over, out);
            for ch in children {
                view_matches(ch, out);
            }
        }
    }
}

/// The state fields that are ever assigned from an
/// `await(...)` expression. These hold `Ok | Err | Cancelled`
/// at runtime even when their declared type says `Result[T, E]`.
fn await_driven_fields(c: &IrComponent) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for e in &c.events {
        for s in &e.body {
            if let IrStmt::Assign { target, value, .. } = s {
                if expr_contains_await(value) {
                    set.insert(target.clone());
                }
            }
        }
    }
    set
}

/// Lower a Tier 11 list-operation operand. Filter predicates,
/// map bodies, and list arguments must be pure: no effects (the
/// capability check) and no suspends (`await`, `sleep`) — the
/// list operation runs synchronously inside the render pass.
fn lower_expr_pure(e: &Expr) -> Result<IrExpr, LowerError> {
    let (ir, effects) = lower_expr_with_effects(e)?;
    if !effects.is_empty() {
        return Err(LowerError::PureLambdaRequired { span: e.span() });
    }
    if ir_expr_suspends(&ir) {
        return Err(LowerError::PureLambdaRequired { span: e.span() });
    }
    Ok(ir)
}

/// True when the (already lowered) expression can suspend: an
/// `await(...)`, a `sleep(ms)` call, or anything carrying one.
fn ir_expr_suspends(e: &IrExpr) -> bool {
    match e {
        IrExpr::Await { .. } => true,
        IrExpr::Call { path, args, .. } => {
            (path.len() == 1 && path[0] == "sleep") || args.iter().any(ir_expr_suspends)
        }
        IrExpr::Binary { lhs, rhs, .. } => {
            ir_expr_suspends(lhs) || ir_expr_suspends(rhs)
        }
        IrExpr::Unary { operand, .. } => ir_expr_suspends(operand),
        IrExpr::Ok { value, .. } | IrExpr::Some { value, .. } => ir_expr_suspends(value),
        IrExpr::Err { reason, .. } => ir_expr_suspends(reason),
        IrExpr::ListLit { items, .. } => items.iter().any(ir_expr_suspends),
        IrExpr::Filter { list, cond, .. } => {
            ir_expr_suspends(list) || ir_expr_suspends(cond)
        }
        IrExpr::Map { list, body, .. } => {
            ir_expr_suspends(list) || ir_expr_suspends(body)
        }
        IrExpr::Len { list, .. } => ir_expr_suspends(list),
        IrExpr::Match { scrutinee, arms, .. } => {
            ir_expr_suspends(scrutinee) || arms.iter().any(|a| ir_expr_suspends(&a.body))
        }
        _ => false,
    }
}

fn expr_contains_await(e: &IrExpr) -> bool {
    match e {
        IrExpr::Await { .. } => true,
        IrExpr::Binary { lhs, rhs, .. } => {
            expr_contains_await(lhs) || expr_contains_await(rhs)
        }
        IrExpr::Unary { operand, .. } => expr_contains_await(operand),
        IrExpr::Call { args, .. } => args.iter().any(expr_contains_await),
        IrExpr::Ok { value, .. } => expr_contains_await(value),
        IrExpr::Err { reason, .. } => expr_contains_await(reason),
        IrExpr::Some { value, .. } => expr_contains_await(value),
        IrExpr::Match { scrutinee, arms, .. } => {
            expr_contains_await(scrutinee) || arms.iter().any(|a| expr_contains_await(&a.body))
        }
        _ => false,
    }
}

fn check_one_match(
    component_name: &str,
    m: &IrExpr,
    state_types: &BTreeMap<String, &IrType>,
    await_driven: &BTreeSet<String>,
) -> Result<(), LowerError> {
    let (scrutinee, arms, span) = match m {
        IrExpr::Match { scrutinee, arms, span } => (scrutinee, arms, *span),
        _ => return Ok(()),
    };
    let field = match scrutinee.as_ref() {
        IrExpr::Ident { name, .. } => name,
        _ => return Ok(()),
    };
    let ty = match state_types.get(field) {
        Some(t) => *t,
        None => return Ok(()),
    };
    // Tier 8: `Result` needs {Ok, Err}, `Option` needs
    // {Some, None}. `List` has no patterns and is not matched.
    let mut required: Vec<&str> = match ty {
        IrType::Result { .. } => vec!["Ok", "Err"],
        IrType::Option { .. } => vec!["Some", "None"],
        IrType::Int | IrType::Float | IrType::Bool | IrType::String | IrType::List { .. }
        | IrType::Request { .. } => return Ok(()),
    };

    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut catch_all = false;
    for arm in arms {
        match &arm.pattern {
            IrPattern::Wildcard { .. } | IrPattern::Ident { .. } => catch_all = true,
            IrPattern::Variant { name, .. } | IrPattern::UnitVariant { name, .. } => {
                covered.insert(name.clone());
            }
        }
    }

    // An awaited value resolves to all three Result constructors
    // even though the declared type says `Result`, so treat the
    // match as tri-variant. The presence of a `Cancelled` arm is
    // the programmer announcing the same thing.
    let tri_variant = await_driven.contains(field) || covered.contains("Cancelled");
    if tri_variant {
        required.push("Cancelled");
    }
    let missing: Vec<String> = required
        .iter()
        .filter(|v| !covered.contains(**v))
        .map(|v| v.to_string())
        .collect();

    if !catch_all && !missing.is_empty() {
        return Err(LowerError::MatchNonExhaustive {
            component: component_name.to_string(),
            field: field.clone(),
            missing,
            span,
        });
    }
    Ok(())
}