//! Hand-written recursive-descent parser for the Tier 1 subset.
//!
//! Grammar (subset of `../../../spec/grammar/README.md`):
//!
//! ```text
//! file        := [ module_decl ] { component_decl }
//! module_decl := "module" identifier ";"
//! component_decl := "component" identifier "{" view_clause "}"
//! view_clause := "view" "{" view_node "}"
//! view_node   := element_node | text_node
//! element_node := ident [ "{" [ accessibility_block ] { view_node } "}" ]
//!                | ident "{" "}"
//! text_node   := string_literal
//! accessibility_block := "role" ":" identifier
//!                     | "label" ":" string_literal
//! ```
//!
//! Phase 1 accepts exactly this grammar and refuses everything
//! else. Phase 2 adds `state`, `requires`, `on`, slots, `each`,
//! `if`/`else`, etc.

use crate::ast::*;
use crate::span::Span;
use crate::lexer::{SpannedToken, Token};
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum ParseError {
    #[error("unexpected token {found:?} at line {line} column {column}; expected {expected}")]
    Unexpected {
        expected: &'static str,
        found: Token,
        line: u32,
        column: u32,
    },

    #[error("unexpected end of file; expected {expected}")]
    UnexpectedEof { expected: &'static str },
}

/// Parse `tokens` (the output of `lex`) into a `File` AST.
pub fn parse(tokens: Vec<SpannedToken>) -> Result<File, ParseError> {
    let mut p = Parser { tokens, pos: 0 };
    let file = p.parse_file()?;
    p.expect(Token::Eof)?;
    Ok(file)
}

struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &SpannedToken {
        &self.tokens[self.pos]
    }

    fn bump(&mut self) -> SpannedToken {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().token, Token::Eof)
    }

    fn expect(&mut self, want: Token) -> Result<SpannedToken, ParseError> {
        let here = self.peek().clone();
        if here.token == want {
            Ok(self.bump())
        } else {
            Err(ParseError::Unexpected {
                expected: token_kind_name(&want),
                found: here.token,
                line: here.span.start.line,
                column: here.span.start.column,
            })
        }
    }

    fn parse_file(&mut self) -> Result<File, ParseError> {
        let start = self.peek().span.start;
        let module = if matches!(&self.peek().token, Token::Ident(s) if s == "module") {
            Some(self.parse_module()?)
        } else {
            None
        };
        let mut components = Vec::new();
        while !self.at_eof() {
            components.push(self.parse_component()?);
        }
        let end = self.peek().span.end;
        Ok(File {
            module,
            components,
            span: Span::new(start, end),
        })
    }

    fn parse_module(&mut self) -> Result<ModuleDecl, ParseError> {
        // We already peeked and confirmed the keyword.
        let kw = self.bump();
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "module name (identifier)",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        self.expect(Token::Semi)?;
        Ok(ModuleDecl {
            name,
            span: kw.span,
        })
    }

    fn parse_component(&mut self) -> Result<Component, ParseError> {
        let kw = self.expect(Token::Component)?;
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "component name (identifier)",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        self.expect(Token::LBrace)?;
        let view = self.parse_view_clause()?;
        self.expect(Token::RBrace)?;
        Ok(Component {
            name,
            view,
            span: Span::new(kw.span.start, self.peek().span.end),
        })
    }

    fn parse_view_clause(&mut self) -> Result<ViewBlock, ParseError> {
        let kw = self.expect(Token::View)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_view_node()?;
        self.expect(Token::RBrace)?;
        Ok(ViewBlock {
            body,
            span: Span::new(kw.span.start, self.peek().span.end),
        })
    }

    fn parse_view_node(&mut self) -> Result<ViewNode, ParseError> {
        match &self.peek().token {
            Token::StringLit(_) => self.parse_text_node(),
            Token::Ident(_) => self.parse_element_node(),
            other => Err(ParseError::Unexpected {
                expected: "view node (string literal or element)",
                found: other.clone(),
                line: self.peek().span.start.line,
                column: self.peek().span.start.column,
            }),
        }
    }

    fn parse_text_node(&mut self) -> Result<ViewNode, ParseError> {
        let tok = self.bump();
        let value = match tok.token {
            Token::StringLit(s) => s,
            _ => unreachable!("parse_text_node called without string"),
        };
        Ok(ViewNode::Text {
            value: TextValue::Literal(value),
            span: tok.span,
        })
    }

    fn parse_element_node(&mut self) -> Result<ViewNode, ParseError> {
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            _ => unreachable!("parse_element_node called without ident"),
        };
        let kind = match name.as_str() {
            "Column" => ElementKind::Column,
            "Row" => ElementKind::Row,
            "Text" => ElementKind::Text,
            other => ElementKind::Named(other.to_string()),
        };

        if !matches!(self.peek().token, Token::LBrace) {
            return Err(ParseError::Unexpected {
                expected: "{",
                found: self.peek().token.clone(),
                line: self.peek().span.start.line,
                column: self.peek().span.start.column,
            });
        }
        let open = self.bump(); // {
        let mut accessibility = Accessibility { role: None, label: None, span: open.span };
        let mut children = Vec::new();

        loop {
            match &self.peek().token {
                Token::Ident(s) if s == "role" => {
                    let kw = self.bump();
                    self.expect(Token::DotDot)?;
                    let role_tok = self.bump();
                    let role = match role_tok.token {
                        Token::Ident(s) => match s.as_str() {
                            "text" => Role::Text,
                            other => Role::Other(other.to_string()),
                        },
                        other => {
                            return Err(ParseError::Unexpected {
                                expected: "role identifier",
                                found: other,
                                line: role_tok.span.start.line,
                                column: role_tok.span.start.column,
                            });
                        }
                    };
                    accessibility.role = Some(role);
                    accessibility.span = Span::new(accessibility.span.start, role_tok.span.end);
                    let _ = kw;
                }
                Token::Ident(s) if s == "label" => {
                    let kw = self.bump();
                    self.expect(Token::DotDot)?;
                    let label_tok = self.bump();
                    let label = match label_tok.token {
                        Token::StringLit(s) => s,
                        other => {
                            return Err(ParseError::Unexpected {
                                expected: "label string literal",
                                found: other,
                                line: label_tok.span.start.line,
                                column: label_tok.span.start.column,
                            });
                        }
                    };
                    accessibility.label = Some(label);
                    accessibility.span = Span::new(accessibility.span.start, label_tok.span.end);
                    let _ = kw;
                }
                Token::RBrace | Token::StringLit(_) | Token::Ident(_) => break,
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "accessibility key, view node, or `}`",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        }

        while !matches!(self.peek().token, Token::RBrace) {
            children.push(self.parse_view_node()?);
        }
        let close = self.bump(); // }
        Ok(ViewNode::Element {
            kind,
            accessibility,
            children,
            span: Span::new(name_tok.span.start, close.span.end),
        })
    }
}

fn token_kind_name(t: &Token) -> &'static str {
    match t {
        Token::Component => "`component`",
        Token::View => "`view`",
        Token::LBrace => "`{`",
        Token::RBrace => "`}`",
        Token::LParen => "`(`",
        Token::RParen => "`)`",
        Token::Semi => "`;`",
        Token::Comma => "`,`",
        Token::DotDot => "`:`",
        Token::Ident(_) => "identifier",
        Token::StringLit(_) => "string literal",
        Token::Eof => "end of file",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_source(src: &str) -> Result<File, ParseError> {
        let toks = lex(src).expect("lex");
        parse(toks)
    }

    #[test]
    fn parses_hello_world() {
        let f = parse_source(
            "component Hello {\n    view {\n        Text { \"hello, world\" }\n    }\n}\n",
        )
        .expect("parse");
        assert_eq!(f.components.len(), 1);
        assert_eq!(f.components[0].name, "Hello");
    }

    #[test]
    fn parses_nested_elements_with_accessibility() {
        let src = r#"
            component Greeting {
                view {
                    Column {
                        Text {
                            role: heading
                            label: "hi"
                            "the body"
                        }
                    }
                }
            }
        "#;
        let f = parse_source(src).expect("parse");
        match &f.components[0].view.body {
            ViewNode::Element { kind, accessibility, children, .. } => {
                assert!(matches!(kind, ElementKind::Column));
                assert!(accessibility.role.is_none());
                assert!(accessibility.label.is_none());
                assert_eq!(children.len(), 1);
            }
            _ => panic!("expected Column"),
        }
    }

    #[test]
    fn parses_module_decl() {
        let f = parse_source("module app;\ncomponent X { view { Text { \"hi\" } } }\n").expect("parse");
        assert_eq!(f.module.as_ref().unwrap().name, "app");
        assert_eq!(f.components.len(), 1);
    }

    #[test]
    fn unterminated_brace_refused() {
        let err = parse_source("component X { view { Text { \"hi\" ").unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedEof { .. } | ParseError::Unexpected { .. }));
    }
}