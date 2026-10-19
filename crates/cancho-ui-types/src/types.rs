//! Type representation for the Tier 1 subset.
//!
//! Phase 1 supports:
//!   - `String` — the only "value" type a static component needs.
//!   - `Unit`   — the type of a view body that has no data, for
//!     forward compatibility with stateful components.
//!
//! Phase 2 adds `Int`, `Float`, `Bool`, `List[T]`, `Option[T]`,
//! `Result[T, E]`, `Request[T]`, `Event(...)`, `Capability(...)`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    String,
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    String,
    Unit,
}

impl Type {
    pub fn primitive(&self) -> PrimitiveType {
        match self {
            Type::String => PrimitiveType::String,
            Type::Unit => PrimitiveType::Unit,
        }
    }
}
