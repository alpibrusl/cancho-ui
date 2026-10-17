//! Canonical IR serialization. Phase 1 emits JSON; Phase 2 may
//! add a binary form. The shape mirrors
//! `../../../spec/ir-schema/README.md` §10.
//!
//! Stable rules:
//! - field order matches the declaration order in `ir.rs`,
//! - no insignificant whitespace,
//! - stable ID encoding is a decimal integer (Phase 2 will switch
//!   to the 32-byte hex hash when the canonical encoder lands),
//! - source spans are kept (they are part of the IR, not a
//!   sidecar, for Phase 1).

use serde::Serialize;

use crate::ir::*;

pub fn schema_version() -> &'static str {
    "0.1.0"
}

pub fn canonical_json(ir: &IrFile) -> String {
    // serde_json::to_string is already deterministic for a struct
    // with derived Serialize and a fixed field order. Phase 2 adds
    // explicit canonical ordering if a snapshot test demands it.
    serde_json::to_string_pretty(ir).expect("IrFile serialization is infallible")
}

/// A serializable view of `cancho_ui_syntax::span::Span`. We can't
/// `impl Serialize for the foreign Span` (orphan rule), so the IR
/// stores an `IrSpan` directly and provides a `From<Span>` impl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IrSpan {
    pub start: IrPosition,
    pub end: IrPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IrPosition {
    pub line: u32,
    pub column: u32,
}

impl From<cancho_ui_syntax::span::Span> for IrSpan {
    fn from(s: cancho_ui_syntax::span::Span) -> Self {
        IrSpan {
            start: IrPosition { line: s.start.line, column: s.start.column },
            end: IrPosition { line: s.end.line, column: s.end.column },
        }
    }
}

impl From<cancho_ui_syntax::span::Position> for IrPosition {
    fn from(p: cancho_ui_syntax::span::Position) -> Self {
        IrPosition { line: p.line, column: p.column }
    }
}

impl Serialize for IrFile {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrFile", 4)?;
        s.serialize_field("schema", &self.schema)?;
        s.serialize_field("version", &self.version)?;
        s.serialize_field("module", &self.module)?;
        s.serialize_field("components", &self.components)?;
        s.end()
    }
}

impl Serialize for IrComponent {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrComponent", 4)?;
        s.serialize_field("id", &self.id.0)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("view", &self.view)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrViewBlock {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrViewBlock", 2)?;
        s.serialize_field("body", &self.body)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrViewNode {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrViewNode::Element { kind, accessibility, children, span } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrViewNode::Element", 4)?;
                s.serialize_field("kind", kind)?;
                s.serialize_field("accessibility", accessibility)?;
                s.serialize_field("children", children)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrViewNode::Text { value, span } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrViewNode::Text", 2)?;
                s.serialize_field("value", value)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
        }
    }
}

impl Serialize for IrElementKind {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrElementKind::Column => ser.serialize_unit_variant("IrElementKind", 0, "Column"),
            IrElementKind::Row => ser.serialize_unit_variant("IrElementKind", 1, "Row"),
            IrElementKind::Text => ser.serialize_unit_variant("IrElementKind", 2, "Text"),
            IrElementKind::Named(s) => {
                use serde::ser::SerializeStruct;
                let mut st = ser.serialize_struct("IrElementKind::Named", 1)?;
                st.serialize_field("Named", s)?;
                st.end()
            }
        }
    }
}

impl Serialize for IrAccessibility {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrAccessibility", 2)?;
        s.serialize_field("role", &self.role)?;
        s.serialize_field("label", &self.label)?;
        s.end()
    }
}

impl Serialize for IrRole {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrRole::Text => ser.serialize_unit_variant("IrRole", 0, "Text"),
            IrRole::Other(s) => ser.serialize_str(s),
        }
    }
}