//! cancho-ui-types — type representation and Phase 1 validation.
//!
//! Phase 1 supports the minimum needed for a static `hello, world`:
//! - the four built-in primitives (`Column`, `Row`, `Text`),
//! - one string literal type (`String`),
//! - one role type (Text, Other),
//! - basic well-formedness checks.
//!
//! Phase 2 extends this with `Int`, `Float`, `Bool`, `List[T]`,
//! `Option[T]`, `Result[T, E]`, `Request[T]`, `Event(...)`,
//! `Capability(...)`, and the full capability/effect checker.

pub mod check;
pub mod types;

pub use check::{CheckError, ExternalComponent, TypeError, check_file, check_file_with_externals};
pub use types::{PrimitiveType, Type};