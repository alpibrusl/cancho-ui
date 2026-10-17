//! AST → IR lowering. Phase 1 is a structural pass: every AST node
//! has a one-to-one IR equivalent for the Tier 1 supported subset.
//! Capability and effect tracking is a Phase 2 addition.

use cancho_ui_syntax::ast::{Accessibility, Component, ElementKind, File, Role, ViewNode};
use cancho_ui_syntax::span::Span;
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
    Ok(IrComponent {
        id,
        name: c.name.clone(),
        view: IrViewBlock {
            body: lower_view_node(&c.view.body)?,
            span: c.view.span,
        },
        span: c.span,
    })
}

fn lower_view_node(node: &ViewNode) -> Result<IrViewNode, LowerError> {
    match node {
        ViewNode::Element { kind, accessibility, children, span } => {
            let mut lowered_children = Vec::with_capacity(children.len());
            for c in children {
                lowered_children.push(lower_view_node(c)?);
            }
            Ok(IrViewNode::Element {
                kind: lower_kind(kind),
                accessibility: lower_accessibility(accessibility),
                children: lowered_children,
                span: *span,
            })
        }
        ViewNode::Text { value, span } => {
            let s = match value {
                cancho_ui_syntax::ast::TextValue::Literal(s) => s.clone(),
            };
            Ok(IrViewNode::Text { value: s, span: *span })
        }
    }
}

fn lower_kind(k: &ElementKind) -> IrElementKind {
    match k {
        ElementKind::Column => IrElementKind::Column,
        ElementKind::Row => IrElementKind::Row,
        ElementKind::Text => IrElementKind::Text,
        ElementKind::Named(s) => IrElementKind::Named(s.clone()),
    }
}

fn lower_accessibility(acc: &Accessibility) -> IrAccessibility {
    IrAccessibility {
        role: acc.role.as_ref().map(lower_role),
        label: acc.label.clone(),
    }
}

fn lower_role(r: &Role) -> IrRole {
    match r {
        Role::Text => IrRole::Text,
        Role::Other(s) => IrRole::Other(s.clone()),
    }
}