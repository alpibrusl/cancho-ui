//! Canonical IR serialization. Phase 1 emits JSON; Phase 2 may
//! add a binary form. The shape mirrors
//! `../../../spec/ir-schema/README.md` §10.

use serde::Serialize;

use crate::ir::*;

pub fn schema_version() -> &'static str {
    "0.1.0"
}

pub fn canonical_json(ir: &IrFile) -> String {
    pretty_json(ir)
}

/// Pretty JSON for a value of one of our derived `Serialize` types. Serializing those cannot fail (every map key is a
/// string); if it ever did, the text is a JSON object carrying the error instead of a panic (#62).
pub fn pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|e| {
        format!(
            "{{\"rule\": \"internal-json\", \"message\": {}}}",
            serde_json::Value::String(e.to_string())
        )
    })
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
        let mut s = ser.serialize_struct("IrFile", 5)?;
        s.serialize_field("schema", &self.schema)?;
        s.serialize_field("version", &self.version)?;
        s.serialize_field("module", &self.module)?;
        s.serialize_field("components", &self.components)?;
        if let Some(t) = &self.theme {
            s.serialize_field("theme", &crate::theme::to_json(t))?;
        }
        if !self.assets.is_empty() {
            s.serialize_field("assets", &self.assets)?;
        }
        if !self.islands.is_empty() {
            s.serialize_field("islands", &self.islands)?;
        }
        if let Some(p) = &self.page {
            s.serialize_field("page", &crate::page::to_json(p))?;
        }
        s.end()
    }
}

impl Serialize for IrIsland {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrIsland", 10)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("sealed", &self.sealed)?;
        if !self.sealed {
            s.serialize_field("authority", crate::island::OPEN_AUTHORITY)?;
        }
        s.serialize_field("label", &self.label)?;
        s.serialize_field("decorative", &self.decorative)?;
        s.serialize_field("width", &self.width)?;
        s.serialize_field("height", &self.height)?;
        s.serialize_field("scripts", &self.scripts)?;
        s.serialize_field("style", &self.style)?;
        let props: Vec<serde_json::Value> = self
            .props
            .iter()
            .map(|p| serde_json::json!({ "name": p.name, "type": p.ty.name() }))
            .collect();
        s.serialize_field("props", &props)?;
        let events: Vec<serde_json::Value> = self
            .events
            .iter()
            .map(|e| serde_json::json!({ "name": e.name, "type": e.ty.map(|t| t.name()) }))
            .collect();
        s.serialize_field("events", &events)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrIslandFile {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrIslandFile", 3)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("sha256", &self.sha256)?;
        s.serialize_field("bytes", &self.bytes)?;
        s.end()
    }
}

impl Serialize for IrAsset {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrAsset", 3)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("sha256", &self.sha256)?;
        s.serialize_field("bytes", &self.bytes)?;
        s.end()
    }
}

impl Serialize for IrComponent {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrComponent", 9)?;
        s.serialize_field("id", &self.id.0)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("state", &self.state)?;
        s.serialize_field("requires", &self.requires)?;
        s.serialize_field("schedules", &self.schedules)?;
        s.serialize_field("reachable_effects", &self.reachable_effects)?;
        s.serialize_field("events", &self.events)?;
        if !self.messages.is_empty() {
            s.serialize_field("messages", &self.messages)?;
        }
        s.serialize_field("view", &self.view)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrCatalog {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrCatalog", 3)?;
        s.serialize_field("lang", &self.lang)?;
        s.serialize_field("entries", &self.entries)?;
        s.serialize_field("span", &IrSpan::from(self.span))?;
        s.end()
    }
}

impl Serialize for IrMessage {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrMessage", 3)?;
        s.serialize_field("key", &self.key)?;
        if let Some(p) = &self.plural {
            s.serialize_field("plural", p)?;
        }
        let forms: Vec<(&str, String)> = self
            .forms
            .iter()
            .map(|(cat, parts)| (cat.as_str(), crate::messages::text_of(parts)))
            .collect();
        s.serialize_field("forms", &forms)?;
        s.end()
    }
}

impl Serialize for IrSchedule {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let every = matches!(self.kind, IrScheduleKind::Every(_));
        let extra = if every {
            1
        } else {
            self.stream
                .as_ref()
                .map_or(0, |t| 1 + usize::from(t.status.is_some()))
        };
        let mut s = ser.serialize_struct("IrSchedule", 3 + extra)?;
        s.serialize_field(
            "kind",
            match self.kind {
                IrScheduleKind::Start => "start",
                IrScheduleKind::Every(_) => "every",
                IrScheduleKind::Route => "route",
                IrScheduleKind::Subscribe => "subscribe",
            },
        )?;
        s.serialize_field("event", &self.event)?;
        if let Some(t) = &self.stream {
            s.serialize_field("url", &t.url)?;
            if let Some(status) = &t.status {
                s.serialize_field("status", status)?;
            }
        }
        if let IrScheduleKind::Every(ms) = self.kind {
            s.serialize_field("period_ms", &ms)?;
        }
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
            IrStmt::If {
                cond,
                then,
                else_,
                span,
            } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrStmt::If", 4)?;
                s.serialize_field("cond", cond)?;
                s.serialize_field("then", then)?;
                s.serialize_field("else", else_)?;
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
            IrExpr::If {
                cond,
                then,
                else_,
                span,
            } => {
                let mut s = ser.serialize_struct("IrExpr::If", 4)?;
                s.serialize_field("cond", cond.as_ref())?;
                s.serialize_field("then", then.as_ref())?;
                s.serialize_field("else", else_.as_ref())?;
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
            IrExpr::SortBy {
                list,
                item,
                key,
                span,
            } => {
                let mut s = ser.serialize_struct("IrExpr::SortBy", 4)?;
                s.serialize_field("list", list)?;
                s.serialize_field("item", item)?;
                s.serialize_field("key", key)?;
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
            IrBinOp::And => "And",
            IrBinOp::Or => "Or",
        };
        ser.serialize_str(s)
    }
}

impl Serialize for IrUnOp {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(match self {
            IrUnOp::Neg => "Neg",
            IrUnOp::Not => "Not",
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
            IrViewNode::If {
                cond,
                children,
                span,
            } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrViewNode::If", 3)?;
                s.serialize_field("cond", cond)?;
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
            IrElementKind::TextArea => ser.serialize_unit_variant("IrElementKind", 9, "TextArea"),
            IrElementKind::Checkbox => ser.serialize_unit_variant("IrElementKind", 10, "Checkbox"),
            IrElementKind::Select => ser.serialize_unit_variant("IrElementKind", 11, "Select"),
            IrElementKind::FileField => {
                ser.serialize_unit_variant("IrElementKind", 13, "FileField")
            }
            IrElementKind::Chart => ser.serialize_unit_variant("IrElementKind", 14, "Chart"),
            IrElementKind::NavLink => ser.serialize_unit_variant("IrElementKind", 12, "NavLink"),
            IrElementKind::SortList => ser.serialize_unit_variant("IrElementKind", 15, "SortList"),
            IrElementKind::Map => ser.serialize_unit_variant("IrElementKind", 16, "Map"),
            IrElementKind::Heading => ser.serialize_unit_variant("IrElementKind", 17, "Heading"),
            IrElementKind::Section => ser.serialize_unit_variant("IrElementKind", 18, "Section"),
            IrElementKind::Header => ser.serialize_unit_variant("IrElementKind", 19, "Header"),
            IrElementKind::Nav => ser.serialize_unit_variant("IrElementKind", 20, "Nav"),
            IrElementKind::Main => ser.serialize_unit_variant("IrElementKind", 21, "Main"),
            IrElementKind::Footer => ser.serialize_unit_variant("IrElementKind", 22, "Footer"),
            IrElementKind::Aside => ser.serialize_unit_variant("IrElementKind", 23, "Aside"),
            IrElementKind::Article => ser.serialize_unit_variant("IrElementKind", 24, "Article"),
            IrElementKind::Link => ser.serialize_unit_variant("IrElementKind", 25, "Link"),
            IrElementKind::Details => ser.serialize_unit_variant("IrElementKind", 26, "Details"),
            IrElementKind::Figure => ser.serialize_unit_variant("IrElementKind", 27, "Figure"),
            IrElementKind::Caption => ser.serialize_unit_variant("IrElementKind", 28, "Caption"),
            IrElementKind::Rich => ser.serialize_unit_variant("IrElementKind", 29, "Rich"),
            IrElementKind::Strong => ser.serialize_unit_variant("IrElementKind", 30, "Strong"),
            IrElementKind::Em => ser.serialize_unit_variant("IrElementKind", 31, "Em"),
            IrElementKind::Small => ser.serialize_unit_variant("IrElementKind", 32, "Small"),
            IrElementKind::Reveal => ser.serialize_unit_variant("IrElementKind", 40, "Reveal"),
            IrElementKind::CountUp => ser.serialize_unit_variant("IrElementKind", 41, "CountUp"),
            IrElementKind::Waffle => ser.serialize_unit_variant("IrElementKind", 42, "Waffle"),
            IrElementKind::Chip => ser.serialize_unit_variant("IrElementKind", 43, "Chip"),
            IrElementKind::Break => ser.serialize_unit_variant("IrElementKind", 33, "Break"),
            IrElementKind::Summary => ser.serialize_unit_variant("IrElementKind", 34, "Summary"),
            IrElementKind::Code => ser.serialize_unit_variant("IrElementKind", 35, "Code"),
            IrElementKind::InlineCode => {
                ser.serialize_unit_variant("IrElementKind", 36, "InlineCode")
            }
            IrElementKind::Image => ser.serialize_unit_variant("IrElementKind", 37, "Image"),
            IrElementKind::Media => ser.serialize_unit_variant("IrElementKind", 38, "Media"),
            IrElementKind::Grid => ser.serialize_unit_variant("IrElementKind", 39, "Grid"),
            IrElementKind::Table => ser.serialize_unit_variant("IrElementKind", 5, "Table"),
            IrElementKind::TableRow => ser.serialize_unit_variant("IrElementKind", 6, "TableRow"),
            IrElementKind::Cell => ser.serialize_unit_variant("IrElementKind", 7, "Cell"),
            IrElementKind::HeaderCell => {
                ser.serialize_unit_variant("IrElementKind", 8, "HeaderCell")
            }
            IrElementKind::Island(i) => {
                ser.serialize_newtype_variant("IrElementKind", 40, "Island", &i.name)
            }
            IrElementKind::Named(s) => ser.serialize_str(s),
        }
    }
}

impl Serialize for IrAccessibility {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("IrAccessibility", 3)?;
        s.serialize_field("role", &self.role)?;
        s.serialize_field("label", &self.label)?;
        // Only a label that is an expression has this key, so the IR of a program with literal labels is unchanged.
        if let Some(e) = &self.label_expr {
            s.serialize_field("label_expr", e)?;
        }
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
            IrBindingValue::EventCall { name, args } => {
                use serde::ser::SerializeStruct;
                let mut s = ser.serialize_struct("IrBindingValue::EventCall", 2)?;
                s.serialize_field("event", name)?;
                s.serialize_field("args", args)?;
                s.end()
            }
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
