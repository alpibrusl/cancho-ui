//! Source positions. Every AST and IR node carries a `Span` so that
//! diagnostics and source maps can locate the offending construct.
//!
//! Phase 1 uses 1-indexed line and 0-indexed column (column is
//! counted in Unicode scalar values, matching what most editors display).
//! Phase 2 may add byte offsets if a tool needs them.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub const DUMMY: Position = Position { line: 0, column: 0 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub const DUMMY: Span = Span {
        start: Position { line: 0, column: 0 },
        end: Position { line: 0, column: 0 },
    };

    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}
