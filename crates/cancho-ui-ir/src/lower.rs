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
    Role, Stmt, StmtBlock, StateField, TextPart, TextValue, Type, UnOp, ViewNode,
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
        Expr::Call { path, args, span } => {
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
            let (l, mut le) = lower_expr_with_effects(lhs)?;
            let (r, mut re) = lower_expr_with_effects(rhs)?;
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
    }
}

/// Phase 1 / Tier 4 hard-codes the capability for known builtin
/// calls. Phase 2 will replace this with a registered vocabulary
/// (`spec/security-model.md`) and a richer `capability-missing`
/// permission set.
fn builtin_capability(path: &[String]) -> Option<&'static str> {
    if path.len() == 2 && path[0] == "clock" && path[1] == "now" {
        Some("Clock")
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
fn check_capabilities(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
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