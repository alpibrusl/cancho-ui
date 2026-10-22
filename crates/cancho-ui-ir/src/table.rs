//! What lowering makes of the descendants of a `Table` (docs/presentation.md section 4).
//!
//! The source says `Table { Row { Text { role: columnheader "id" } } For { over: l as: m Row { Text { m } } } }`: the same
//! three elements as everywhere. The IR says what they are in a table, so that the emitter writes a real `<table>` and the
//! assistive technology reads columns: a `Row` of a table is a `TableRow` (`<tr>`), a `Text` in it a `Cell` (`<td>`) or,
//! with `role: columnheader`, a `HeaderCell` (`<th scope="col">`), and any other element in it (a button) is wrapped in a
//! `Cell`. A `For` is kept, its body made rows (in a table) or cells (in a row).

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

fn refuse(message: &str, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code: "table-child-invalid",
        message: message.to_string(),
        span,
    }
}

fn span_of(n: &IrViewNode) -> Span {
    match n {
        IrViewNode::Element { span, .. }
        | IrViewNode::Text { span, .. }
        | IrViewNode::For { span, .. }
        | IrViewNode::If { span, .. } => *span,
    }
}

/// The children of a `Table`: rows, or loops that make rows.
pub fn table_rows(nodes: Vec<IrViewNode>) -> Result<Vec<IrViewNode>, LowerError> {
    nodes
        .into_iter()
        .map(|n| match n {
            IrViewNode::Element {
                kind: IrElementKind::Row,
                accessibility,
                bindings,
                children,
                span,
            } => Ok(IrViewNode::Element {
                kind: IrElementKind::TableRow,
                accessibility,
                bindings,
                children: table_cells(children),
                span,
            }),
            IrViewNode::For {
                over,
                binding,
                children,
                span,
            } => Ok(IrViewNode::For {
                over,
                binding,
                children: table_rows(children)?,
                span,
            }),
            IrViewNode::If {
                cond,
                children,
                span,
            } => Ok(IrViewNode::If {
                cond,
                children: table_rows(children)?,
                span,
            }),
            other => Err(refuse(
                "a `Table` holds `Row`s (and `For` loops that make `Row`s); each `Text` of a `Row` is a cell",
                span_of(&other),
            )),
        })
        .collect()
}

/// The children of a row of a table: cells, or loops that make cells.
fn table_cells(nodes: Vec<IrViewNode>) -> Vec<IrViewNode> {
    nodes
        .into_iter()
        .map(|n| match n {
            IrViewNode::Element {
                kind: IrElementKind::Text,
                accessibility,
                bindings,
                children,
                span,
            } => {
                let header =
                    matches!(&accessibility.role, Some(IrRole::Other(r)) if r == "columnheader");
                IrViewNode::Element {
                    kind: if header {
                        IrElementKind::HeaderCell
                    } else {
                        IrElementKind::Cell
                    },
                    accessibility,
                    bindings,
                    children,
                    span,
                }
            }
            IrViewNode::For {
                over,
                binding,
                children,
                span,
            } => IrViewNode::For {
                over,
                binding,
                children: table_cells(children),
                span,
            },
            IrViewNode::If {
                cond,
                children,
                span,
            } => IrViewNode::If {
                cond,
                children: table_cells(children),
                span,
            },
            // A button, a field, a column, a nested table, or a bare string: the cell around it.
            other => {
                let span = span_of(&other);
                IrViewNode::Element {
                    kind: IrElementKind::Cell,
                    accessibility: IrAccessibility {
                        role: None,
                        label: None,
                        label_expr: None,
                    },
                    bindings: Vec::new(),
                    children: vec![other],
                    span,
                }
            }
        })
        .collect()
}
