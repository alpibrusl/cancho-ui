//! Hand-written lexer for the Tier 1 supported subset.
//!
//! Token kinds follow `../../../spec/grammar/README.md` §9 and §10.
//! Phase 1 supports only what the parser needs; Phase 2 adds the
//! rest (`fn`, `let`, `match`, etc.) when the corresponding
//! syntactic forms land.
//!
//! The escape set is intentionally the same six Cancho Core uses:
//! `\n \r \t \\ \" \0`. No `\x` or `\u` — the source file is already
//! UTF-8.

use crate::span::{Position, Span};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords (the Phase 1 reserved set, see spec/grammar §2)
    Component,
    View,
    State,
    On,
    Requires,
    Let,
    Match,
    Await,
    If,
    Else,

    // Punctuation
    LBrace,   // {
    RBrace,   // }
    LParen,   // (
    RParen,   // )
    LBracket, // [
    RBracket, // ]
    Semi,     // ;
    Comma,    // ,
    Dot,      // .
    DotDot,   // :

    // Operators
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Eq,       // =
    EqEq,     // ==
    FatArrow, // =>
    BangEq,   // !=
    Lt,       // <
    LtEq,     // <=
    Gt,       // >
    GtEq,     // >=
    AmpAmp,   // &&
    PipePipe, // ||
    Bang,     // !

    // Literals and identifiers
    Ident(String),
    StringLit(String),
    IntLit(i64),
    FloatLit(f64),

    // End of file
    Eof,
}

impl Token {
    /// Returns true for the keyword tokens. Used by the parser to
    /// distinguish `Text` (a built-in primitive, used as an element
    /// kind) from any identifier.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Token::Component
                | Token::View
                | Token::State
                | Token::On
                | Token::Requires
                | Token::Let
                | Token::Match
                | Token::Await
                | Token::If
                | Token::Else
        )
    }
}

/// What the lexer writes for `\{` in a string literal until the parser turns it into a brace: a private-use character
/// (U+E000), so that `"\{"` is text and not the start of an interpolation (docs/language-design.md, decision 15).
pub const ESCAPED_OPEN: char = '\u{E000}';
/// What the lexer writes for `\}` (U+E001).
pub const ESCAPED_CLOSE: char = '\u{E001}';

#[derive(Debug, Clone, Error, PartialEq)]
pub enum LexError {
    #[error("unexpected character {ch:?} at line {line} column {column}")]
    UnexpectedChar { ch: char, line: u32, column: u32 },

    #[error("unterminated string literal starting at line {line} column {column}")]
    UnterminatedString { line: u32, column: u32 },

    #[error("unknown escape sequence \\{ch} at line {line} column {column}")]
    UnknownEscape { ch: char, line: u32, column: u32 },
}

/// A token with its source span, in token-stream order.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// Lex `source` into a stream of tokens, terminating with `Eof`.
/// Whitespace and the two comment forms (`// line` and `/* block */`)
/// are skipped.
pub fn lex(source: &str) -> Result<Vec<SpannedToken>, LexError> {
    let mut l = Lexer::new(source);
    let mut out = Vec::new();
    loop {
        l.skip_trivia()?;
        let pos = l.position();
        if l.peek().is_none() {
            out.push(SpannedToken {
                token: Token::Eof,
                span: Span::new(pos, pos),
            });
            return Ok(out);
        }
        let tok = l.next()?;
        let end = l.position();
        out.push(SpannedToken {
            token: tok,
            span: Span::new(pos, end),
        });
    }
}

struct Lexer {
    source: Vec<char>,
    /// Byte position into the original source, kept for error
    /// messages that need it.
    cursor: usize,
    line: u32,
    column: u32,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            cursor: 0,
            line: 1,
            column: 0,
        }
    }

    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.cursor).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.cursor += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 0;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') => {
                    // Could be `//` or `/*`.
                    let next = self.source.get(self.cursor + 1).copied();
                    match next {
                        Some('/') => {
                            // Line comment: skip to end of line.
                            self.bump();
                            self.bump();
                            while let Some(c) = self.peek() {
                                if c == '\n' {
                                    break;
                                }
                                self.bump();
                            }
                        }
                        Some('*') => {
                            // Block comment. Nesting matches
                            // spec/grammar §10.
                            self.bump();
                            self.bump();
                            let mut depth: u32 = 1;
                            while depth > 0 {
                                match (self.peek(), self.source.get(self.cursor + 1).copied()) {
                                    (Some('/'), Some('*')) => {
                                        self.bump();
                                        self.bump();
                                        depth += 1;
                                    }
                                    (Some('*'), Some('/')) => {
                                        self.bump();
                                        self.bump();
                                        depth -= 1;
                                    }
                                    (None, _) => {
                                        return Err(LexError::UnterminatedString {
                                            line: self.line,
                                            column: self.column,
                                        });
                                    }
                                    _ => {
                                        self.bump();
                                    }
                                }
                            }
                        }
                        _ => return Ok(()),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn next(&mut self) -> Result<Token, LexError> {
        // At the end of the source the next token is `Eof`; `lex` stops before asking, so this is the total form of
        // "nothing left" and not a panic (#62).
        let Some(c) = self.peek() else {
            return Ok(Token::Eof);
        };
        match c {
            '{' => {
                self.bump();
                Ok(Token::LBrace)
            }
            '}' => {
                self.bump();
                Ok(Token::RBrace)
            }
            '(' => {
                self.bump();
                Ok(Token::LParen)
            }
            ')' => {
                self.bump();
                Ok(Token::RParen)
            }
            '[' => {
                self.bump();
                Ok(Token::LBracket)
            }
            ']' => {
                self.bump();
                Ok(Token::RBracket)
            }
            ';' => {
                self.bump();
                Ok(Token::Semi)
            }
            ',' => {
                self.bump();
                Ok(Token::Comma)
            }
            '.' => {
                self.bump();
                Ok(Token::Dot)
            }
            ':' => {
                self.bump();
                Ok(Token::DotDot)
            }
            '+' => {
                self.bump();
                Ok(Token::Plus)
            }
            '-' => {
                self.bump();
                Ok(Token::Minus)
            }
            '*' => {
                self.bump();
                Ok(Token::Star)
            }
            '/' => {
                self.bump();
                Ok(Token::Slash)
            }
            '=' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    if self.peek() == Some('>') {
                        self.bump();
                        Ok(Token::FatArrow)
                    } else {
                        Ok(Token::EqEq)
                    }
                } else if self.peek() == Some('>') {
                    self.bump();
                    Ok(Token::FatArrow)
                } else {
                    Ok(Token::Eq)
                }
            }
            '!' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::BangEq)
                } else {
                    Ok(Token::Bang)
                }
            }
            '&' => {
                self.bump();
                if self.peek() == Some('&') {
                    self.bump();
                    Ok(Token::AmpAmp)
                } else {
                    Err(LexError::UnexpectedChar {
                        ch: '&',
                        line: self.line,
                        column: self.column,
                    })
                }
            }
            '|' => {
                self.bump();
                if self.peek() == Some('|') {
                    self.bump();
                    Ok(Token::PipePipe)
                } else {
                    Err(LexError::UnexpectedChar {
                        ch: '|',
                        line: self.line,
                        column: self.column,
                    })
                }
            }
            '<' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::LtEq)
                } else {
                    Ok(Token::Lt)
                }
            }
            '>' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token::GtEq)
                } else {
                    Ok(Token::Gt)
                }
            }
            '"' => self.lex_string(),
            c if c.is_ascii_digit() => self.lex_number(),
            c if is_ident_start(c) => self.lex_ident(),
            c => Err(LexError::UnexpectedChar {
                ch: c,
                line: self.line,
                column: self.column,
            }),
        }
    }

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        // Optional fractional part.
        if self.peek() == Some('.') {
            // Look ahead — if next-next is a digit, treat as float.
            let next = self.source.get(self.cursor + 1).copied();
            if next.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                s.push('.');
                self.bump();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                return Ok(Token::FloatLit(s.parse().map_err(|_| {
                    LexError::UnexpectedChar {
                        ch: '.',
                        line: self.line,
                        column: self.column,
                    }
                })?));
            }
        }
        Ok(Token::IntLit(s.parse().map_err(|_| {
            LexError::UnexpectedChar {
                ch: '?',
                line: self.line,
                column: self.column,
            }
        })?))
    }

    fn lex_string(&mut self) -> Result<Token, LexError> {
        let start = self.position();
        self.bump(); // opening "
        let mut value = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(LexError::UnterminatedString {
                        line: start.line,
                        column: start.column,
                    });
                }
                Some('"') => {
                    self.bump();
                    return Ok(Token::StringLit(value));
                }
                Some('\\') => {
                    self.bump();
                    let esc = self.peek().ok_or(LexError::UnterminatedString {
                        line: start.line,
                        column: start.column,
                    })?;
                    self.bump();
                    match esc {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '0' => value.push('\0'),
                        // `\{` and `\}` write a literal brace (see `ESCAPED_OPEN`): kept as private-use
                        // marks until the interpolation splitter, so that `"\{"` is not the start of an interpolation.
                        '{' => value.push(ESCAPED_OPEN),
                        '}' => value.push(ESCAPED_CLOSE),
                        _ => {
                            return Err(LexError::UnknownEscape {
                                ch: esc,
                                line: self.line,
                                column: self.column,
                            });
                        }
                    }
                }
                Some(c) => {
                    self.bump();
                    value.push(c);
                }
            }
        }
    }

    fn lex_ident(&mut self) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        Ok(match s.as_str() {
            "component" => Token::Component,
            "view" => Token::View,
            "state" => Token::State,
            "on" => Token::On,
            "requires" => Token::Requires,
            "let" => Token::Let,
            "match" => Token::Match,
            "await" => Token::Await,
            "if" => Token::If,
            "else" => Token::Else,
            // Note: `Text`, `Column`, `Row`, `Button` are
            // intentionally NOT keywords. They are element kinds
            // in the parser, and an agent or human is free to
            // define their own component with any of these names
            // (the parser's element-kind path is unambiguous
            // because the reserved-word rule applies at the
            // binding level, not the kind level — see
            // `spec/diagnostics/name_reserved`).
            _ => Token::Ident(s),
        })
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_keywords_and_idents() {
        let toks = lex("component Hello view").unwrap();
        assert_eq!(
            toks.iter().map(|t| &t.token).collect::<Vec<_>>(),
            vec![
                &Token::Component,
                &Token::Ident("Hello".into()),
                &Token::View,
                &Token::Eof,
            ]
        );
    }

    #[test]
    fn lexes_string_with_escapes() {
        let toks = lex(r#""hello\nworld""#).unwrap();
        assert_eq!(toks[0].token, Token::StringLit("hello\nworld".into()));
    }

    #[test]
    fn skips_line_and_block_comments() {
        let toks = lex("// hi\n/* block */\ncomponent X").unwrap();
        assert_eq!(toks[0].token, Token::Component);
        assert_eq!(toks[1].token, Token::Ident("X".into()));
    }

    #[test]
    fn nested_block_comments() {
        let toks = lex("/* outer /* inner */ outer */ component X").unwrap();
        assert_eq!(toks[0].token, Token::Component);
    }

    #[test]
    fn unknown_escape_refused() {
        let err = lex(r#""\x""#).unwrap_err();
        assert!(matches!(err, LexError::UnknownEscape { .. }));
    }

    #[test]
    fn next_at_the_end_of_the_source_is_eof_not_a_panic() {
        // #62: `next` used to `expect` a character.
        let mut l = Lexer::new("");
        assert_eq!(l.next(), Ok(Token::Eof));
        let mut l = Lexer::new("  ");
        let _ = l.skip_trivia();
        assert_eq!(l.next(), Ok(Token::Eof));
    }
}
