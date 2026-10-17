//! AST → IR lowering for the Tier 2 supported subset.

use cancho_ui_syntax::ast::{
    Binding, BindingValue, BinOp, CapabilityRef, Component, ElementKind, EventHandler, Expr, File,
    Role, Stmt, StmtBlock, StateField, TextPart, TextValue, Type, UnOp, ViewNode,
};
use cancho_ui_syntax::span::{Position, Span};
use thiserror::Error;

use crate::ir::*;
use crate::serialize::schema_version;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum LowerError {
    #[error("internal: AST node has empty span")]
    EmptySpan { span: Span },
}

pub fn lower_file(file: &File) -> Result<IrFile, LowerError> {
    let mut next_id = 0u32;
    let mut components = Vec::with_capacity(file.components.len());
    for c in &file.components {
        components.push(lower_component(c, &mut next_id)?);
    }
    Ok(IrFile {
        schema: SCHEMA.to_string(),
        version: schema_version().to_string(),
        module: file.module.as_ref().map(|m| m.name.clone()),
        components,
    })
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
    for e in &c.events {
        events.push(lower_event(e)?);
    }
    let view = IrViewBlock {
        body: lower_view_node(&c.view.body)?,
        span: c.view.span,
    };
    Ok(IrComponent {
        id,
        name: c.name.clone(),
        state,
        requires,
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

fn lower_event(e: &EventHandler) -> Result<IrEventHandler, LowerError> {
    let body = lower_stmts(&e.body)?;
    Ok(IrEventHandler {
        name: e.name.clone(),
        params: e.params.clone(),
        body,
        span: e.span,
    })
}

fn lower_stmts(block: &StmtBlock) -> Result<Vec<IrStmt>, LowerError> {
    let mut out = Vec::with_capacity(block.stmts.len());
    for s in &block.stmts {
        out.push(lower_stmt(s)?);
    }
    Ok(out)
}

fn lower_stmt(s: &Stmt) -> Result<IrStmt, LowerError> {
    match s {
        Stmt::Assign { target, value, span } => Ok(IrStmt::Assign {
            target: target.clone(),
            value: lower_expr(value)?,
            span: *span,
        }),
        Stmt::Expr { expr, span } => Ok(IrStmt::Expr {
            expr: lower_expr(expr)?,
            span: *span,
        }),
    }
}

fn lower_expr(e: &Expr) -> Result<IrExpr, LowerError> {
    match e {
        Expr::IntLit { value, span } => Ok(IrExpr::IntLit {
            value: *value,
            span: *span,
        }),
        Expr::FloatLit { value, span } => Ok(IrExpr::FloatLit {
            value: *value,
            span: *span,
        }),
        Expr::StringLit { value, span } => Ok(IrExpr::StringLit {
            value: value.clone(),
            span: *span,
        }),
        Expr::BoolLit { value, span } => Ok(IrExpr::BoolLit {
            value: *value,
            span: *span,
        }),
        Expr::Ident { name, span } => Ok(IrExpr::Ident {
            name: name.clone(),
            span: *span,
        }),
        Expr::Binary { op, lhs, rhs, span } => Ok(IrExpr::Binary {
            op: lower_binop(*op),
            lhs: Box::new(lower_expr(lhs)?),
            rhs: Box::new(lower_expr(rhs)?),
            span: *span,
        }),
        Expr::Unary { op, operand, span } => Ok(IrExpr::Unary {
            op: lower_unop(*op),
            operand: Box::new(lower_expr(operand)?),
            span: *span,
        }),
    }
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

fn lower_view_node(node: &ViewNode) -> Result<IrViewNode, LowerError> {
    match node {
        ViewNode::Element { kind, accessibility, bindings, children, span } => {
            let mut lowered_children = Vec::with_capacity(children.len());
            for c in children {
                lowered_children.push(lower_view_node(c)?);
            }
            let mut lowered_bindings = Vec::with_capacity(bindings.len());
            for b in bindings {
                lowered_bindings.push(lower_binding(b)?);
            }
            Ok(IrViewNode::Element {
                kind: lower_kind(kind),
                accessibility: lower_accessibility(accessibility),
                bindings: lowered_bindings,
                children: lowered_children,
                span: *span,
            })
        }
        ViewNode::Text { value, span } => {
            let parts = match value {
                TextValue::Interpolated(parts) => {
                    let mut lowered = Vec::with_capacity(parts.len());
                    for p in parts {
                        match p {
                            TextPart::Literal(s) => lowered.push(IrTextPart::Literal(s.clone())),
                            TextPart::Expr { expr, span } => {
                                let _ = span;
                                lowered.push(IrTextPart::Expr(lower_expr(expr)?))
                            }
                        }
                    }
                    lowered
                }
            };
            Ok(IrViewNode::Text { parts, span: *span })
        }
    }
}

fn lower_kind(k: &ElementKind) -> IrElementKind {
    match k {
        ElementKind::Column => IrElementKind::Column,
        ElementKind::Row => IrElementKind::Row,
        ElementKind::Text => IrElementKind::Text,
        ElementKind::Button => IrElementKind::Button,
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
    };
    Ok(IrBinding {
        key: b.key.clone(),
        value,
        span: b.span,
    })
}

// Marker so Position is referenced (avoids dead-code warnings if
// Position is only used by error variants in LowerError).
#[allow(dead_code)]
const _POSITION: Position = Position { line: 0, column: 0 };