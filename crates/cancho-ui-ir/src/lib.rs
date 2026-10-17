//! cancho-ui-ir — Typed UI IR and AST → IR lowering.
//!
//! Phase 1 implements only the minimum needed for a static
//! `hello, world`:
//! - `IrFile`, `IrComponent`
//! - `IrViewNode::Element` and `IrViewNode::Text`
//! - `IrElementKind` (Column, Row, Text, Named)
//! - `Accessibility` (role + label)
//! - `lower_file` entry point
//!
//! Phase 2 adds `state`, `requires`, `event` handlers, the effect
//! row, and the canonical serialization described in
//! `../../../spec/ir-schema/README.md`.

pub mod ir;
pub mod lower;
pub mod serialize;

pub use ir::*;
pub use lower::{LowerError, lower_file};
pub use serialize::{canonical_json, schema_version};