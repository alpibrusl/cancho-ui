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

    fn match_token(&mut self, _t: &Token) -> bool {
        // Reserved for future parser-extension use; kept private to
        // silence the dead-code warning without dropping the
        // utility. Phase 2 uses this when `match` becomes a keyword.
        false
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
        let ty = self.parse_type()?;
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

    /// Parse a type expression. Tier 6 supports plain type
    /// identifiers (`Int`, `Float`, `Bool`, `String`) and the
    /// generic `Result[T, E]`. Tier 7 adds `Request[T]`.
    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let start = self.peek().span;
        let name_tok = self.bump();
        let name = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "type",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        if name == "Result" {
            self.expect(Token::LBracket)?;
            let ok = self.parse_type()?;
            self.expect(Token::Comma)?;
            let err = self.parse_type()?;
            self.expect(Token::RBracket)?;
            return Ok(Type::Result {
                ok: Box::new(ok),
                err: Box::new(err),
            });
        }
        if name == "Request" {
            self.expect(Token::LBracket)?;
            let payload = self.parse_type()?;
            self.expect(Token::RBracket)?;
            return Ok(Type::Request {
                payload: Box::new(payload),
            });
        }
        Type::from_ident(&name).ok_or_else(|| ParseError::Unexpected {
            expected: "type (`Int`, `Float`, `Bool`, `String`, `Result[T, E]`, or `Request[T]`)",
            found: Token::Ident(name.clone()),
            line: start.start.line,
            column: start.start.column,
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
        if matches!(self.peek().token, Token::Match) {
            return self.parse_match();
        }
        if matches!(self.peek().token, Token::Await) {
            return self.parse_await();
        }
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

    fn parse_await(&mut self) -> Result<Expr, ParseError> {
    // `await(expr)` — awaits a `Request[T]`, returns the
    // resolved `RequestResult[T]`. The parens are required so
    // `await foo` (a missing-paren error) is unambiguous.
    let kw = self.expect(Token::Await)?;
    self.expect(Token::LParen)?;
    let request = self.parse_expr()?;
    let close = self.expect(Token::RParen)?;
    Ok(Expr::Await {
        request: Box::new(request),
        span: Span::new(kw.span.start, close.span.end),
    })
}

fn parse_match(&mut self) -> Result<Expr, ParseError> {
    let kw = self.expect(Token::Match)?;
    let scrutinee = self.parse_expr()?;
    self.expect(Token::LBrace)?;
    let mut arms = Vec::new();
    while !matches!(self.peek().token, Token::RBrace) {
        let pattern = self.parse_pattern()?;
        self.expect(Token::FatArrow)?;
        // The arm body is a single expression, optionally wrapped
        // in braces (`{ expr }`). The braces are just for visual
        // grouping — they don't introduce a new scope. Tier 6
        // doesn't have block expressions with statements; for
        // that, Phase 2 lands.
        let (_open_brace, body) = if matches!(self.peek().token, Token::LBrace) {
            let open = self.bump();
            let inner = self.parse_expr()?;
            let _close = self.expect(Token::RBrace)?;
            (Some((open, _close)), inner)
        } else {
            (None, self.parse_expr()?)
        };
        let arm_span_start = pattern.span().start;
        let arm_span_end = body.span().end;
        arms.push(MatchArm {
            pattern,
            body,
            span: Span::new(arm_span_start, arm_span_end),
        });
        // A trailing comma is allowed but not required — the next
        // arm can be on a new line. Whitespace separates arms.
        if matches!(self.peek().token, Token::Comma) {
            self.bump();
        }
    }
    let close = self.expect(Token::RBrace)?;
    Ok(Expr::Match {
        scrutinee: Box::new(scrutinee),
        arms,
        span: Span::new(kw.span.start, close.span.end),
    })
}

fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
    let start = self.peek().span.start;
    if matches!(&self.peek().token, Token::Ident(s) if s == "_") {
        self.bump();
        return Ok(Pattern::Wildcard { span: Span::new(start, start) });
    }
    let name_tok = self.bump();
    let name = match name_tok.token {
        Token::Ident(s) => s,
        other => {
            return Err(ParseError::Unexpected {
                expected: "pattern (identifier, `_`, or `Name(binding)`)",
                found: other,
                line: name_tok.span.start.line,
                column: name_tok.span.start.column,
            });
        }
    };
    // `Cancelled` is a known zero-payload variant. Other bare
    // identifiers are still bindings (`Ident` pattern); the
    // match arm body decides whether to use them.
    if name == "Cancelled" && !matches!(self.peek().token, Token::LParen) {
        return Ok(Pattern::UnitVariant {
            name,
            span: name_tok.span,
        });
    }
    // `Name(binding)` — a constructor pattern.
    if matches!(self.peek().token, Token::LParen) {
        self.bump();
        let binding_tok = self.bump();
        let binding = match binding_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "identifier in `Name(binding)` pattern",
                    found: other,
                    line: binding_tok.span.start.line,
                    column: binding_tok.span.start.column,
                });
            }
        };
        let close = self.expect(Token::RParen)?;
        return Ok(Pattern::Variant {
            name,
            binding,
            span: Span::new(name_tok.span.start, close.span.end),
        });
    }
    Ok(Pattern::Ident {
        name,
        span: name_tok.span,
    })
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
            Token::Ident(_) => {
                // A path: `ident (.ident)*` optionally followed by
                // `(args)` as a builtin call. Bare single-segment
                // paths are identifiers; multi-segment paths must
                // be calls (`clock.now`, not just `clock.now`).
                let first_tok = self.bump();
                let first_name = match &first_tok.token {
                    Token::Ident(s) => s.clone(),
                    _ => unreachable!(),
                };                let mut path = vec![first_name];
                while matches!(self.peek().token, Token::Dot) {
                    self.bump();
                    let seg = self.bump();
                    let seg_name = match seg.token {
                        Token::Ident(s) => s,
                        other => {
                            return Err(ParseError::Unexpected {
                                expected: "identifier after `.`",
                                found: other,
                                line: seg.span.start.line,
                                column: seg.span.start.column,
                            });
                        }
                    };
                    path.push(seg_name);
                }
                if matches!(self.peek().token, Token::LParen) {
                    let call_start = first_tok.span.start;
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.peek().token, Token::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek().token, Token::Comma) {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    let call_end = self.expect(Token::RParen)?.span.end;
                    // `Ok(value)` and `Err(reason)` are constructor
                    // expressions for the `Result` type. They look
                    // like calls with a single-segment path, so we
                    // detect them here and lift them to dedicated
                    // AST variants so the type checker and IR can
                    // see them as constructors.
                    if path.len() == 1 && path[0] == "Ok" {
                        if args.len() != 1 {
                            return Err(ParseError::Unexpected {
                                expected: "exactly one argument to `Ok(...)`",
                                found: Token::LParen,
                                line: call_start.line,
                                column: call_start.column,
                            });
                        }
                        let v = args.into_iter().next().unwrap();
                        return Ok(Expr::Ok {
                            value: Box::new(v),
                            span: Span::new(call_start, call_end),
                        });
                    }
                    if path.len() == 1 && path[0] == "Err" {
                        if args.len() != 1 {
                            return Err(ParseError::Unexpected {
                                expected: "exactly one argument to `Err(...)`",
                                found: Token::LParen,
                                line: call_start.line,
                                column: call_start.column,
                            });
                        }
                        let v = args.into_iter().next().unwrap();
                        return Ok(Expr::Err {
                            reason: Box::new(v),
                            span: Span::new(call_start, call_end),
                        });
                    }
                    // `Cancelled` is a zero-payload variant
                    // produced by the runtime when a request is
                    // dropped; the language does not expose it as a
                    // constructor expression. If the user writes
                    // `Cancelled(...)`, treat it as a regular call
                    // — but a `Cancelled` token is unusual, so this
                    // is rare.
                    if path.len() == 1 && path[0] == "Cancelled" && args.is_empty() {
                        return Ok(Expr::Cancelled {
                            span: Span::new(call_start, call_end),
                        });
                    }
                    Ok(Expr::Call {
                        path,
                        args,
                        span: Span::new(call_start, call_end),
                    })
                } else if path.len() == 1 {
                    Ok(Expr::Ident {
                        name: path.into_iter().next().unwrap(),
                        span: first_tok.span,
                    })
                } else {
                    Err(ParseError::Unexpected {
                        expected: "`(` (call) or `;` (statement boundary)",
                        found: self.peek().token.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    })
                }
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
        // Text body: either a string literal (with `{expr}`
        // interpolations) or a general expression whose result
        // becomes the text. Tier 6 supports both because
        // `Text { match result { ... } }` needs the expression
        // form to render a `Result` value conditionally.
        if matches!(self.peek().token, Token::StringLit(_)) {
            let tok = self.bump();
            let value = match tok.token {
                Token::StringLit(s) => s,
                _ => unreachable!(),
            };
            let mut parts = Vec::new();
            let mut buf = String::new();
            let current_line = tok.span.start.line;
            let mut chars = value.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '{' {
                    if !buf.is_empty() {
                        parts.push(TextPart::Literal(std::mem::take(&mut buf)));
                    }
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
                    chars.next();
                    if depth != 0 {
                        return Err(ParseError::Unexpected {
                            expected: "closing `}` in interpolation",
                            found: Token::Eof,
                            line: current_line,
                            column: 0,
                        });
                    }
                    let interp_tokens = crate::lexer::lex(&expr_src).map_err(|e| {
                        ParseError::InvalidNumber {
                            raw: format!("{:?}", e),
                            line: current_line,
                            column: 0,
                        }
                    })?;
                    let mut sub = Parser { tokens: interp_tokens, pos: 0 };
                    let expr = sub.parse_expr().map_err(|e| ParseError::InvalidNumber {
                        raw: format!("{:?}", e),
                        line: current_line,
                        column: 0,
                    })?;
                    parts.push(TextPart::Expr { expr, span: tok.span });
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
        } else {
            // The body is a general expression. Render it as a Text
            // with a single Expr part.
            let start = self.peek().span.start;
            let expr = self.parse_expr()?;
            let end = expr.span().end;
            Ok(ViewNode::Text {
                value: TextValue::Interpolated(vec![TextPart::Expr {
                    expr,
                    span: Span::new(start, end),
                }]),
                span: Span::new(start, end),
            })
        }
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
            "TextField" => ElementKind::TextField,
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
                    self.bump();
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
                }
                Token::Ident(name) if name == "label" => {
                    let _kw = self.bump();
                    let _ = _kw;
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
                }
                Token::RBrace => break,
                Token::StringLit(_) => break,
                Token::Ident(_) => break,
                Token::On => break,
                Token::Match => break,
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
            // After accessibility, expect a string literal
            // (with `{expr}` interpolations) OR a general
            // expression whose result becomes the text. Tier 6
            // supports the expression form so `Text { match
            // result { ... } }` works.
            match &self.peek().token {
                Token::StringLit(_) | Token::Ident(_) | Token::IntLit(_) | Token::FloatLit(_)
                | Token::LParen | Token::Minus | Token::Match => {
                    children.push(self.parse_text_node()?);
                }
                Token::RBrace => {
                    let span = self.peek().span;
                    children.push(ViewNode::Text {
                        value: TextValue::Interpolated(vec![TextPart::Literal(String::new())]),
                        span,
                    });
                }
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "string literal (text content) or expression",
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
            "value" => {
                // `value: <ident>` — bind to a state field, or
                // `value: <string>` for a literal initial value.
                let val_tok = self.bump();
                match val_tok.token {
                    Token::Ident(s) => Ok(BindingValue::StateRef(s)),
                    Token::StringLit(s) => Ok(BindingValue::String(s)),
                    other => Err(ParseError::Unexpected {
                        expected: "state field name (identifier) or string literal",
                        found: other,
                        line: val_tok.span.start.line,
                        column: val_tok.span.start.column,
                    }),
                }
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

fn token_kind_name(t: &Token) -> &'static str {
    match t {
        Token::Component => "`component`",
        Token::View => "`view`",
        Token::State => "`state`",
        Token::On => "`on`",
        Token::Requires => "`requires`",
        Token::Let => "`let`",
        Token::Match => "`match`",
        Token::Await => "`await`",
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
        Token::FatArrow => "`=>`",
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