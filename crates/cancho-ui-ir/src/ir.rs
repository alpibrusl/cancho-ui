//! Typed UI IR — the in-memory shape that the web backend and
//! every cancho-ui tool reads. Mirrors `../../../spec/ir-schema/README.md`,
//! restricted to the Tier 1 supported subset.

use cancho_ui_syntax::span::Span;

pub const SCHEMA: &str = "cancho-ui-ir";

#[derive(Debug, Clone, PartialEq)]
pub struct IrFile {
    pub schema: String,
    pub version: String,
    pub module: Option<String>,
    pub components: Vec<IrComponent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrComponent {
    pub id: IrId,
    pub name: String,
    pub view: IrViewBlock,
    pub span: Span,
}

/// A stable identifier derived from the declaration's content.
/// Phase 2 hashes the canonical text; Phase 1 uses a placeholder
/// counter so snapshots stay deterministic across runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct IrViewBlock {
    pub body: IrViewNode,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrViewNode {
    Element {
        kind: IrElementKind,
        accessibility: IrAccessibility,
        children: Vec<IrViewNode>,
        span: Span,
    },
    Text {
        value: String,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrElementKind {
    Column,
    Row,
    Text,
    Named(String),
}

impl IrElementKind {
    /// The string the JS emitter uses as the DOM tag. Phase 1
    /// uses semantic HTML; Phase 2 may extend this with custom
    /// elements.
    pub fn tag_name(&self) -> &'static str {
        match self {
            IrElementKind::Column => "div",
            IrElementKind::Row => "div",
            IrElementKind::Text => "span",
            // User components render as `<div>` for Tier 1; Phase 2
            // emits custom elements (`<x-counter>`) when a name is
            // known and stable.
            IrElementKind::Named(_) => "div",
        }
    }

    /// Returns true for layout primitives that need a class to
    /// distinguish vertical vs horizontal layout. The CSS is
    /// minimal in Phase 1; Phase 2 introduces a stylesheet.
    pub fn layout_class(&self) -> Option<&'static str> {
        match self {
            IrElementKind::Column => Some("cui-col"),
            IrElementKind::Row => Some("cui-row"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrAccessibility {
    pub role: Option<IrRole>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrRole {
    Text,
    Other(String),
}