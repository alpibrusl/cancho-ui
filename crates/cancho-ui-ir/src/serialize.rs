//! Canonical IR serialization. Phase 1 emits JSON; Phase 2 may
//! add a binary form. The shape mirrors
//! `../../../spec/ir-schema/README.md` §10.

use serde::Serialize;

use crate::ir::*;

pub fn schema_version() -> &'static str {
    "0.1.0"
}

pub fn canonical_json(ir: &IrFile) -> String {
    serde_json::to_string_pretty(ir).expect("IrFile serialization is infallible")
}

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
            start: IrPosition {
                line: s.start.line,
                column: s.start.column,
            },
            end: IrPosition {
                line: s.end.line,
                column: s.end.column,
            },
        }
    }
}

impl From<cancho_ui_syntax::span::Position> for IrPosition {
    fn from(p: cancho_ui_syntax::span::Position) -> Self {
        IrPosition {
            line: p.line,
            column: p.column,
        }
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
        let mut s = ser.serialize_struct("IrComponent", 7)?;
        s.serialize_field("id", &self.id.0)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("state", &self.state)?;
        s.serialize_field("requires", &self.requires)?;
        s.serialize_field("reachable_effects", &self.reachable_effects)?;
        s.serialize_field("events", &self.events)?;
        s.serialize_field("view", &self.view)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrReachableEffect {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrReachableEffect", 3)?;
        s.serialize_field("capability", &self.capability)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrStateField {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrStateField", 4)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("ty", &self.ty)?;
        s.serialize_field("initial", &self.initial)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrType {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrType::Int => ser.serialize_unit_variant("IrType", 0, "Int"),
            IrType::Float => ser.serialize_unit_variant("IrType", 1, "Float"),
            IrType::Bool => ser.serialize_unit_variant("IrType", 2, "Bool"),
            IrType::String => ser.serialize_unit_variant("IrType", 3, "String"),
            IrType::Result { ok, err } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrType::Result", 2)?;
                s.serialize_field("ok", ok.as_ref())?;
                s.serialize_field("err", err.as_ref())?;
                s.end()
            }
            IrType::Request { payload } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrType::Request", 1)?;
                s.serialize_field("payload", payload.as_ref())?;
                s.end()
            }
            IrType::Option { some } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrType::Option", 1)?;
                s.serialize_field("some", some.as_ref())?;
                s.end()
            }
            IrType::List { elem } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrType::List", 1)?;
                s.serialize_field("elem", elem.as_ref())?;
                s.end()
            }
        }
    }
}

impl Serialize for IrCapabilityRef {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrCapabilityRef", 3)?;
        s.serialize_field("name", &self.name)?;
        if !self.bounds.is_empty() {
            s.serialize_field("bounds", &self.bounds)?;
        }
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrEventHandler {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrEventHandler", 4)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("params", &self.params)?;
        s.serialize_field("body", &self.body)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrStmt {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrStmt::Assign {
                target,
                value,
                span,
            } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrStmt::Assign", 3)?;
                s.serialize_field("target", target)?;
                s.serialize_field("value", value)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrStmt::Expr { expr, span } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrStmt::Expr", 2)?;
                s.serialize_field("expr", expr)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
        }
    }
}

impl Serialize for IrExpr {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        match self {
            IrExpr::IntLit { value, span } => {
                let mut s = ser.serialize_struct("IrExpr::IntLit", 2)?;
                s.serialize_field("value", value)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::FloatLit { value, span } => {
                let mut s = ser.serialize_struct("IrExpr::FloatLit", 2)?;
                s.serialize_field("value", value)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::StringLit { value, span } => {
                let mut s = ser.serialize_struct("IrExpr::StringLit", 2)?;
                s.serialize_field("value", value)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::BoolLit { value, span } => {
                let mut s = ser.serialize_struct("IrExpr::BoolLit", 2)?;
                s.serialize_field("value", value)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Ident { name, span } => {
                let mut s = ser.serialize_struct("IrExpr::Ident", 2)?;
                s.serialize_field("name", name)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Call { path, args, span } => {
                let mut s = ser.serialize_struct("IrExpr::Call", 3)?;
                s.serialize_field("path", path)?;
                s.serialize_field("args", args)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Binary { op, lhs, rhs, span } => {
                let mut s = ser.serialize_struct("IrExpr::Binary", 4)?;
                s.serialize_field("op", op)?;
                s.serialize_field("lhs", lhs)?;
                s.serialize_field("rhs", rhs)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Unary { op, operand, span } => {
                let mut s = ser.serialize_struct("IrExpr::Unary", 3)?;
                s.serialize_field("op", op)?;
                s.serialize_field("operand", operand)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Match {
                scrutinee,
                arms,
                span,
            } => {
                let mut s = ser.serialize_struct("IrExpr::Match", 3)?;
                s.serialize_field("scrutinee", scrutinee.as_ref())?;
                s.serialize_field("arms", arms)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Ok { value, span } => {
                let mut s = ser.serialize_struct("IrExpr::Ok", 2)?;
                s.serialize_field("value", value.as_ref())?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Err { reason, span } => {
                let mut s = ser.serialize_struct("IrExpr::Err", 2)?;
                s.serialize_field("reason", reason.as_ref())?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Await { request, span } => {
                let mut s = ser.serialize_struct("IrExpr::Await", 2)?;
                s.serialize_field("request", request.as_ref())?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Cancelled { span } => {
                let mut s = ser.serialize_struct("IrExpr::Cancelled", 1)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Some { value, span } => {
                let mut s = ser.serialize_struct("IrExpr::Some", 2)?;
                s.serialize_field("value", value.as_ref())?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::None { span } => {
                let mut s = ser.serialize_struct("IrExpr::None", 1)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::ListLit { items, span } => {
                let mut s = ser.serialize_struct("IrExpr::ListLit", 2)?;
                s.serialize_field("items", items)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Filter {
                list,
                item,
                cond,
                span,
            } => {
                let mut s = ser.serialize_struct("IrExpr::Filter", 4)?;
                s.serialize_field("list", list)?;
                s.serialize_field("item", item)?;
                s.serialize_field("cond", cond)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Map {
                list,
                item,
                body,
                span,
            } => {
                let mut s = ser.serialize_struct("IrExpr::Map", 4)?;
                s.serialize_field("list", list)?;
                s.serialize_field("item", item)?;
                s.serialize_field("body", body)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrExpr::Len { list, span } => {
                let mut s = ser.serialize_struct("IrExpr::Len", 2)?;
                s.serialize_field("list", list)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
        }
    }
}

impl Serialize for IrMatchArm {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrMatchArm", 3)?;
        s.serialize_field("pattern", &self.pattern)?;
        s.serialize_field("body", &self.body)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrPattern {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        match self {
            IrPattern::Wildcard { span } => {
                let mut s = ser.serialize_struct("IrPattern::Wildcard", 1)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrPattern::Ident { name, span } => {
                let mut s = ser.serialize_struct("IrPattern::Ident", 2)?;
                s.serialize_field("name", name)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrPattern::Variant {
                name,
                binding,
                span,
            } => {
                let mut s = ser.serialize_struct("IrPattern::Variant", 3)?;
                s.serialize_field("name", name)?;
                s.serialize_field("binding", binding)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrPattern::UnitVariant { name, span } => {
                let mut s = ser.serialize_struct("IrPattern::UnitVariant", 2)?;
                s.serialize_field("name", name)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
        }
    }
}

impl Serialize for IrBinOp {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let s = match self {
            IrBinOp::Add => "Add",
            IrBinOp::Sub => "Sub",
            IrBinOp::Mul => "Mul",
            IrBinOp::Div => "Div",
            IrBinOp::Eq => "Eq",
            IrBinOp::Ne => "Ne",
            IrBinOp::Lt => "Lt",
            IrBinOp::Le => "Le",
            IrBinOp::Gt => "Gt",
            IrBinOp::Ge => "Ge",
        };
        ser.serialize_str(s)
    }
}

impl Serialize for IrUnOp {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(match self {
            IrUnOp::Neg => "Neg",
        })
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
            IrViewNode::Element {
                kind,
                accessibility,
                bindings,
                children,
                span,
            } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrViewNode::Element", 5)?;
                s.serialize_field("kind", kind)?;
                s.serialize_field("accessibility", accessibility)?;
                s.serialize_field("bindings", bindings)?;
                s.serialize_field("children", children)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrViewNode::Text { parts, span } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrViewNode::Text", 2)?;
                s.serialize_field("parts", parts)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
            IrViewNode::For {
                over,
                binding,
                children,
                span,
            } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrViewNode::For", 4)?;
                s.serialize_field("over", over)?;
                s.serialize_field("binding", binding)?;
                s.serialize_field("children", children)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
        }
    }
}

impl Serialize for IrTextPart {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrTextPart::Literal(s) => ser.serialize_str(s),
            IrTextPart::Expr(e) => e.serialize(ser),
        }
    }
}

impl Serialize for IrElementKind {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrElementKind::Column => ser.serialize_unit_variant("IrElementKind", 0, "Column"),
            IrElementKind::Row => ser.serialize_unit_variant("IrElementKind", 1, "Row"),
            IrElementKind::Text => ser.serialize_unit_variant("IrElementKind", 2, "Text"),
            IrElementKind::Button => ser.serialize_unit_variant("IrElementKind", 3, "Button"),
            IrElementKind::TextField => ser.serialize_unit_variant("IrElementKind", 4, "TextField"),
            IrElementKind::Named(s) => ser.serialize_str(s),
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
        let s = match self {
            IrRole::Text => "text",
            IrRole::Heading => "heading",
            IrRole::Button => "button",
            IrRole::Other(s) => s.as_str(),
        };
        ser.serialize_str(s)
    }
}

impl Serialize for IrBinding {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrBinding", 3)?;
        s.serialize_field("key", &self.key)?;
        s.serialize_field("value", &self.value)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrBindingValue {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            IrBindingValue::String(s) => ser.serialize_str(s),
            IrBindingValue::EventRef(s) => ser.serialize_str(s),
            IrBindingValue::Role(r) => r.serialize(ser),
            IrBindingValue::StateRef(s) => ser.serialize_str(s),
            IrBindingValue::Const { expr, span } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrBindingValue::Const", 2)?;
                s.serialize_field("expr", expr)?;
                s.serialize_field("span", &IrSpan::from(*span))?;
                s.end()
            }
        }
    }
}
