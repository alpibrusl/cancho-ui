//! Hand-written recursive-descent parser.
//!
//! Grammar (Tier 2 subset of `../../../spec/grammar/README.md`):
//!
//! ```text
//! file := [ module_decl ] { component_decl }
//! module_decl := "module" identifier ";"
//! component_decl := "component" identifier "{" component_member { component_member } "}"
//! component_member := state_decl | requires_decl | on_clause | view_clause
//! state_decl := "state" identifier ":" type "=" expr ";"
//! requires_decl := "requires" identifier ";"
//! on_clause := "on" identifier [ "(" [ identifier { "," identifier } ] ")" ] "{" stmt_block "}"
//! type := "Int" | "Float" | "Bool" | "String"
//! stmt_block := { stmt }
//! stmt := assign_stmt | expr_stmt
//! assign_stmt := identifier "=" expr ";"
//! expr_stmt := expr ";"
//!
//! expr := binary_expr
//! binary_expr := "+" unary_expr | "-" unary_expr | primary_expr
//! primary_expr := literal | identifier | primary_expr binop primary_expr | "(" expr ")"
//!
//! view_clause := "view" "{" view_node "}"
//! view_node := element_node | text_node
//! element_node := identifier "{" element_body "}"
//! element_body := [ "role" ":" role ] [ "label" ":" string_literal ]
//!                  { ( binding | view_node ) }
//! binding := identifier ":" (string_literal | "on" identifier | "role" role)
//! text_node := string_literal   // contains optional {expr} interpolations
//! ```

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

    #[error("invalid number literal {raw:?} at line {line} column {column}")]
    InvalidNumber { raw: String, line: u32, column: u32 },
}

/// Parse `tokens` (the output of `lex`) into a `File` AST.
pub fn parse(tokens: Vec<SpannedToken>) -> Result<File, ParseError> {
    let mut parser = Parser { tokens, pos: 0 };
    let file = parser.parse_file()?;
    parser.expect(Token::Eof)?;
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

    fn match_token(&mut self, t: &Token) -> bool {
        if std::mem::discriminant(&self.peek().token) == std::mem::discriminant(t) {
            self.bump();
            true
        } else {
            false
        }
    }

    // --- Top-level ---

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
        Ok(ModuleDecl { name, span: kw.span })
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
        let mut state = Vec::new();
        let mut requires = Vec::new();
        let mut events = Vec::new();
        let mut view: Option<ViewBlock> = None;
        loop {
            match &self.peek().token {
                Token::RBrace => break,
                Token::State => state.push(self.parse_state_decl()?),
                Token::Requires => requires.push(self.parse_requires_decl()?),
                Token::On => events.push(self.parse_on_clause()?),
                Token::View => {
                    if view.is_some() {
                        return Err(ParseError::Unexpected {
                            expected: "single `view` clause per component",
                            found: Token::View,
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                    view = Some(self.parse_view_clause()?);
                }
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "component member (`state`, `requires`, `on`, `view`)",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        }
        let view = view.ok_or_else(|| ParseError::Unexpected {
            expected: "view clause",
            found: Token::RBrace,
            line: self.peek().span.start.line,
            column: self.peek().span.start.column,
        })?;
        let close = self.expect(Token::RBrace)?;
        Ok(Component {
            name,
            state,
            requires,
            events,
            view,
            span: Span::new(kw.span.start, close.span.end),
        })
    }

    fn parse_state_decl(&mut self) -> Result<StateField, ParseError> {
        let kw = self.expect(Token::State)?;
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "state field name",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        self.expect(Token::DotDot)?;
        let ty_tok = self.bump();
        let ty = match ty_tok.token {
            Token::Ident(s) => Type::from_ident(&s).ok_or_else(|| ParseError::Unexpected {
                expected: "type (`Int`, `Float`, `Bool`, `String`)",
                found: Token::Ident(s),
                line: ty_tok.span.start.line,
                column: ty_tok.span.start.column,
            })?,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "type",
                    found: other,
                    line: ty_tok.span.start.line,
                    column: ty_tok.span.start.column,
                });
            }
        };
        self.expect(Token::Eq)?;
        let initial = self.parse_expr()?;
        let end_tok = self.expect(Token::Semi)?;
        Ok(StateField {
            name,
            ty,
            initial,
            span: Span::new(kw.span.start, end_tok.span.end),
        })
    }

    fn parse_requires_decl(&mut self) -> Result<CapabilityRef, ParseError> {
        let kw = self.expect(Token::Requires)?;
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "capability name",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        let end_tok = self.expect(Token::Semi)?;
        Ok(CapabilityRef {
            name,
            span: Span::new(kw.span.start, end_tok.span.end),
        })
    }

    fn parse_on_clause(&mut self) -> Result<EventHandler, ParseError> {
        let kw = self.expect(Token::On)?;
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "event handler name",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        let mut params = Vec::new();
        if matches!(self.peek().token, Token::LParen) {
            self.bump(); // (
            if !matches!(self.peek().token, Token::RParen) {
                loop {
                    let p = self.bump();
                    let p_name = match p.token {
                        Token::Ident(s) => s,
                        other => {
                            return Err(ParseError::Unexpected {
                                expected: "parameter name",
                                found: other,
                                line: p.span.start.line,
                                column: p.span.start.column,
                            });
                        }
                    };
                    params.push(p_name);
                    if matches!(self.peek().token, Token::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(Token::RParen)?;
        }
        self.expect(Token::LBrace)?;
        let body = self.parse_stmt_block()?;
        let close = self.expect(Token::RBrace)?;
        Ok(EventHandler {
            name,
            params,
            body,
            span: Span::new(kw.span.start, close.span.end),
        })
    }

    fn parse_stmt_block(&mut self) -> Result<StmtBlock, ParseError> {
        let start = self.peek().span.start;
        let mut stmts = Vec::new();
        while !matches!(self.peek().token, Token::RBrace) {
            stmts.push(self.parse_stmt()?);
        }
        let end = self.peek().span.end;
        Ok(StmtBlock { stmts, span: Span::new(start, end) })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        // An assignment is `identifier = expr;`.
        if let Token::Ident(_) = &self.peek().token {
            if matches!(self.tokens.get(self.pos + 1).map(|t| &t.token), Some(Token::Eq)) {
                let name_tok = self.bump();
                let name = match name_tok.token {
                    Token::Ident(s) => s,
                    _ => unreachable!(),
                };
                self.expect(Token::Eq)?;
                let value = self.parse_expr()?;
                let end_tok = self.expect(Token::Semi)?;
                return Ok(Stmt::Assign {
                    target: name,
                    value,
                    span: Span::new(name_tok.span.start, end_tok.span.end),
                });
            }
        }
        let expr = self.parse_expr()?;
        let end_tok = self.expect(Token::Semi)?;
        Ok(Stmt::Expr {
            expr,
            span: Span::new(start, end_tok.span.end),
        })
    }

    // --- Expressions (Tier 2: arithmetic + comparison, no calls / fns) ---

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_expr(0)
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let (op, prec) = match &self.peek().token {
                Token::EqEq => (BinOp::Eq, 4),
                Token::BangEq => (BinOp::Ne, 4),
                Token::Lt => (BinOp::Lt, 4),
                Token::LtEq => (BinOp::Le, 4),
                Token::Gt => (BinOp::Gt, 4),
                Token::GtEq => (BinOp::Ge, 4),
                Token::Plus => (BinOp::Add, 5),
                Token::Minus => (BinOp::Sub, 5),
                Token::Star => (BinOp::Mul, 6),
                Token::Slash => (BinOp::Div, 6),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let right = self.parse_binary_expr(prec + 1)?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        if matches!(self.peek().token, Token::Minus) {
            self.bump();
            let operand = self.parse_unary()?;
            let span = Span::new(start, operand.span().end);
            // Use the operand end for the trailing; the unary operator
            // returns the operand's type. For unary minus on
            // integers, the JS emitter emits `0 - x`.
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let tok = self.peek().clone();
        match tok.token {
            Token::IntLit(v) => {
                self.bump();
                Ok(Expr::IntLit { value: v, span: tok.span })
            }
            Token::FloatLit(v) => {
                self.bump();
                Ok(Expr::FloatLit { value: v, span: tok.span })
            }
            Token::StringLit(s) => {
                self.bump();
                Ok(Expr::StringLit { value: s, span: tok.span })
            }
            Token::Ident(s) if s == "true" || s == "false" => {
                self.bump();
                Ok(Expr::BoolLit { value: s == "true", span: tok.span })
            }
            Token::Ident(s) => {
                self.bump();
                Ok(Expr::Ident { name: s, span: tok.span })
            }
            Token::LParen => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            other => Err(ParseError::Unexpected {
                expected: "expression",
                found: other,
                line: tok.span.start.line,
                column: tok.span.start.column,
            }),
        }
    }

    // --- View block ---

    fn parse_view_clause(&mut self) -> Result<ViewBlock, ParseError> {
        let kw = self.expect(Token::View)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_view_node()?;
        let close = self.expect(Token::RBrace)?;
        Ok(ViewBlock {
            body,
            span: Span::new(kw.span.start, close.span.end),
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
        // Parse `{expr}` interpolations from the raw string.
        let mut parts = Vec::new();
        let mut buf = String::new();
        let mut chars = value.chars().peekable();
        let mut current_line = tok.span.start.line;
        while let Some(c) = chars.next() {
            if c == '{' {
                if !buf.is_empty() {
                    parts.push(TextPart::Literal(std::mem::take(&mut buf)));
                }
                // Read until the matching `}`.
                let mut expr_src = String::new();
                let mut depth = 1;
                while let Some(&nc) = chars.peek() {
                    if nc == '{' { depth += 1; }
                    if nc == '}' {
                        depth -= 1;
                        if depth == 0 { break; }
                    }
                    expr_src.push(nc);
                    chars.next();
                }
                chars.next(); // consume the closing }
                if depth != 0 {
                    return Err(ParseError::Unexpected {
                        expected: "closing `}` in interpolation",
                        found: Token::Eof,
                        line: current_line,
                        column: 0,
                    });
                }
                // Parse the expression in a mini-recursive-descent.
                let interp_tokens = crate::lexer::lex(&expr_src).map_err(|e| {
                    ParseError::InvalidNumber {
                        raw: format!("{:?}", e),
                        line: current_line,
                        column: 0,
                    }
                })?;
                let mut sub = Parser {
                    tokens: interp_tokens,
                    pos: 0,
                };
                let expr = sub.parse_expr().map_err(|e| ParseError::InvalidNumber {
                    raw: format!("{:?}", e),
                    line: current_line,
                    column: 0,
                })?;
                parts.push(TextPart::Expr { expr, span: tok.span });
                let _ = current_line;
            } else {
                buf.push(c);
            }
        }
        if !buf.is_empty() {
            parts.push(TextPart::Literal(buf));
        }
        Ok(ViewNode::Text {
            value: TextValue::Interpolated(parts),
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
            "Button" => ElementKind::Button,
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
        let mut bindings: Vec<Binding> = Vec::new();
        let mut children = Vec::new();

        loop {
            match &self.peek().token {
                Token::Ident(name) if name == "role" => {
                    let kw = self.bump();
                    self.expect(Token::DotDot)?;
                    let role_tok = self.bump();
                    let role = match role_tok.token {
                        Token::Ident(s) => parse_role(&s),
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
                Token::Ident(name) if name == "label" => {
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
                Token::RBrace => break,
                Token::StringLit(_) => break,
                Token::Ident(_) => break,
                Token::On => break,
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "accessibility key, binding, view node, or `}`",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        }

        // Body. `Text` is special: its body is a single string literal
        // (the text content, with `{expr}` interpolations). Other
        // elements take bindings (label:, on:, ...) and children.
        let kind_is_text = matches!(kind, ElementKind::Text);
        if kind_is_text {
            // After accessibility, expect a single string literal.
            match &self.peek().token {
                Token::StringLit(_) => {
                    children.push(self.parse_text_node()?);
                }
                Token::RBrace => {
                    // Empty body — render an empty text node.
                    let span = self.peek().span;
                    children.push(ViewNode::Text {
                        value: TextValue::Interpolated(vec![TextPart::Literal(String::new())]),
                        span,
                    });
                }
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "string literal (text content)",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        } else {
            // Bindings and children. To distinguish a binding
            // (`label: "x"`) from a child element (`Text { ... }`),
            // look ahead one token: bindings have `:` next, elements
            // have `{` next.
            while !matches!(self.peek().token, Token::RBrace) {
                match &self.peek().token {
                    Token::Ident(name) if name == "role" || name == "label" => {
                        return Err(ParseError::Unexpected {
                            expected: "binding or view node (accessibility keys must come first)",
                            found: Token::Ident(name.to_string()),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                    Token::Ident(_) | Token::On => {
                        // Lookahead: `{` next -> child element,
                        // `:` next -> binding. (`on:` is a binding
                        // even though `on` is a keyword for event
                        // handlers at the component-member level.)
                        let is_element = matches!(
                            self.tokens.get(self.pos + 1).map(|t| &t.token),
                            Some(Token::LBrace)
                        );
                        if is_element {
                            children.push(self.parse_element_node()?);
                        } else {
                            let key_tok = self.bump();
                            let key = match key_tok.token {
                                Token::Ident(s) => s,
                                Token::On => "on".to_string(),
                                _ => unreachable!(),
                            };
                            self.expect(Token::DotDot)?;
                            let value = self.parse_binding_value(&key, key_tok.span)?;
                            let span = Span::new(key_tok.span.start, self.peek().span.end);
                            bindings.push(Binding { key, value, span });
                        }
                    }
                    Token::StringLit(_) => children.push(self.parse_text_node()?),
                    _ => return Err(ParseError::Unexpected {
                        expected: "binding or view node",
                        found: self.peek().token.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    }),
                }
            }
        }
        let close = self.bump(); // }
        Ok(ViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span: Span::new(name_tok.span.start, close.span.end),
        })
    }

    fn parse_binding_value(&mut self, key: &str, _span: crate::span::Span) -> Result<BindingValue, ParseError> {
        // Currently the only supported binding values are:
        //   - string literal (e.g. `label: "Increment"`),
        //   - `on: EventName` (event reference),
        //   - `role: heading` etc.
        // Phase 2 adds typed expressions.
        match key {
            "on" => {
                let name_tok = self.bump();
                let name = match name_tok.token {
                    Token::Ident(s) => s,
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "event name",
                            found: other,
                            line: name_tok.span.start.line,
                            column: name_tok.span.start.column,
                        });
                    }
                };
                Ok(BindingValue::EventRef(name))
            }
            "role" => {
                let role_tok = self.bump();
                let role = match role_tok.token {
                    Token::Ident(s) => parse_role(&s),
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "role identifier",
                            found: other,
                            line: role_tok.span.start.line,
                            column: role_tok.span.start.column,
                        });
                    }
                };
                Ok(BindingValue::Role(role))
            }
            _ => {
                // Default: string literal.
                let lit_tok = self.bump();
                let s = match lit_tok.token {
                    Token::StringLit(s) => s,
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "string literal",
                            found: other,
                            line: lit_tok.span.start.line,
                            column: lit_tok.span.start.column,
                        });
                    }
                };
                Ok(BindingValue::String(s))
            }
        }
    }
}

fn parse_role(s: &str) -> Role {
    match s {
        "text" => Role::Text,
        "heading" => Role::Heading,
        "button" => Role::Button,
        other => Role::Other(other.to_string()),
    }
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::Ident { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. } => *span,
        }
    }
}

fn token_kind_name(t: &Token) -> &'static str {
    match t {
        Token::Component => "`component`",
        Token::View => "`view`",
        Token::State => "`state`",
        Token::On => "`on`",
        Token::Requires => "`requires`",
        Token::Let => "`let`",
        Token::LBrace => "`{`",
        Token::RBrace => "`}`",
        Token::LParen => "`(`",
        Token::RParen => "`)`",
        Token::LBracket => "`[`",
        Token::RBracket => "`]`",
        Token::Semi => "`;`",
        Token::Comma => "`,`",
        Token::Dot => "`.`",
        Token::DotDot => "`:`",
        Token::Plus => "`+`",
        Token::Minus => "`-`",
        Token::Star => "`*`",
        Token::Slash => "`/`",
        Token::Eq => "`=`",
        Token::EqEq => "`==`",
        Token::BangEq => "`!=`",
        Token::Lt => "`<`",
        Token::LtEq => "`<=`",
        Token::Gt => "`>`",
        Token::GtEq => "`>=`",
        Token::Ident(_) => "identifier",
        Token::StringLit(_) => "string literal",
        Token::IntLit(_) => "integer literal",
        Token::FloatLit(_) => "float literal",
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
    fn parses_state_decl() {
        let f = parse_source(
            "component C { state count: Int = 0; view { Text { \"x\" } } }\n",
        )
        .expect("parse");
        assert_eq!(f.components[0].state.len(), 1);
        assert_eq!(f.components[0].state[0].name, "count");
        assert_eq!(f.components[0].state[0].ty, Type::Int);
    }

    #[test]
    fn parses_on_clause_with_arithmetic() {
        let f = parse_source(
            "component C { state x: Int = 0; on Inc { x = x + 1; } view { Text { \"x\" } } }\n",
        )
        .expect("parse");
        assert_eq!(f.components[0].events.len(), 1);
        assert_eq!(f.components[0].events[0].body.stmts.len(), 1);
    }

    #[test]
    fn parses_string_interpolation() {
        let f = parse_source(
            "component C { state x: Int = 0; view { Text { \"Count: {x}\" } } }\n",
        )
        .expect("parse");
        // The view body is a Text element whose single child is the
        // interpolated text node.
        match &f.components[0].view.body {
            ViewNode::Element { kind, children, .. } => {
                assert!(matches!(kind, ElementKind::Text));
                assert_eq!(children.len(), 1);
                match &children[0] {
                    ViewNode::Text { value: TextValue::Interpolated(parts), .. } => {
                        assert_eq!(parts.len(), 2);
                        assert!(matches!(&parts[0], TextPart::Literal(s) if s == "Count: "));
                        assert!(matches!(&parts[1], TextPart::Expr { .. }));
                    }
                    _ => panic!("expected Text"),
                }
            }
            _ => panic!("expected Text element"),
        }
    }
}