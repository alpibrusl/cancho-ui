//! cancho-ui-syntax — lexer, parser, and AST.
//!
//! Phase 0 / Tier 1 implements only the minimum needed for a static
//! `hello, world` component to round-trip through the compiler.
//! The grammar here is a strict subset of
//! `../../../spec/grammar/README.md` §3 and §5.
//!
//! Every source position is preserved on AST nodes so the IR and
//! the web backend can produce source-mapped diagnostics later
//! (Phase 2).

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod span;

pub use ast::*;
pub use lexer::{LexError, Token};
pub use parser::ParseError;
pub use span::{Position, Span};

/// The supported source-language version for Phase 1.
pub const SCHEMA_VERSION: &str = "cancho-ui-source";
pub const SOURCE_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::lexer::lex;

    #[test]
    fn lex_fat_arrow() {
        let toks = lex("=>").unwrap();
        assert!(matches!(toks[0].token, super::Token::FatArrow));
        assert!(matches!(toks[1].token, super::Token::Eof));
    }
}
