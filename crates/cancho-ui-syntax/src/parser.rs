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
//! requires_decl := "requires" identifier [ "(" string_literal { "," string_literal } ")" ] ";"
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
use crate::lexer::{SpannedToken, Token};
use crate::span::Span;
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

fn unexpected(found: Token, span: Span, expected: &'static str) -> ParseError {
    ParseError::Unexpected {
        expected,
        found,
        line: span.start.line,
        column: span.start.column,
    }
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

/// A string that is static text (a label, an attribute, a property): a `{` in it would be shown as a brace and
/// never interpolated, which reads as a bug in the program, so it is refused. The dynamic part belongs in a
/// `Text` child (or, for a handler or a `match` arm, in a string expression, where `{x}` does interpolate).
fn static_text(value: String, span: Span) -> Result<String, ParseError> {
    if value.contains('{') {
        return Err(ParseError::Unexpected {
            expected: "static text without `{` (a label or attribute is not interpolated; put the dynamic part in a `Text` child)",
            found: Token::StringLit(value),
            line: span.start.line,
            column: span.start.column,
        });
    }
    Ok(unescape_braces(&value))
}

/// `\{` and `\}` in the source reach the parser as private-use marks (`ESCAPED_OPEN`, `ESCAPED_CLOSE`); here they become
/// the braces they stand for.
fn unescape_braces(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            crate::lexer::ESCAPED_OPEN => '{',
            crate::lexer::ESCAPED_CLOSE => '}',
            other => other,
        })
        .collect()
}

/// Split the text of a string literal into literal runs and `{expr}` interpolations. A `{` always starts an
/// interpolation, in a `Text` body and in a string expression alike; `\{` and `\}` write a literal brace, and anything left
/// in the interpolation after the expression (`"{\"a\":1}"`) is a parse error, not silently dropped.
fn interpolation_parts(value: &str, span: Span) -> Result<Vec<TextPart>, ParseError> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let current_line = span.start.line;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if !buf.is_empty() {
                parts.push(TextPart::Literal(std::mem::take(&mut buf)));
            }
            let mut expr_src = String::new();
            let mut depth = 1;
            while let Some(&nc) = chars.peek() {
                if nc == '{' {
                    depth += 1;
                }
                if nc == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                match nc {
                    crate::lexer::ESCAPED_OPEN => expr_src.push_str("\\{"),
                    crate::lexer::ESCAPED_CLOSE => expr_src.push_str("\\}"),
                    other => expr_src.push(other),
                }
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
            let interp_tokens =
                crate::lexer::lex(&expr_src).map_err(|e| ParseError::InvalidNumber {
                    raw: format!("{:?}", e),
                    line: current_line,
                    column: 0,
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
            if !sub.at_eof() {
                let left = sub.peek().clone();
                return Err(ParseError::Unexpected {
                    expected: "the end of the interpolation after one expression (to write a literal brace, use `\\{` and `\\}`)",
                    found: left.token,
                    line: span.start.line,
                    column: span.start.column,
                });
            }
            parts.push(TextPart::Expr { expr, span });
        } else {
            buf.push(match c {
                crate::lexer::ESCAPED_OPEN => '{',
                crate::lexer::ESCAPED_CLOSE => '}',
                other => other,
            });
        }
    }
    if !buf.is_empty() {
        parts.push(TextPart::Literal(buf));
    }
    Ok(parts)
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

    /// Consume the next token as an identifier, or refuse. The checked match and the extraction are one operation, so
    /// there is no state in which a token was "checked" and then is not an identifier (#62).
    fn bump_ident(&mut self, expected: &'static str) -> Result<(String, Span), ParseError> {
        let here = self.peek().clone();
        match here.token {
            Token::Ident(name) => {
                self.bump();
                Ok((name, here.span))
            }
            other => Err(unexpected(other, here.span, expected)),
        }
    }

    /// Consume the next token as a string literal, or refuse.
    fn bump_string_lit(&mut self, expected: &'static str) -> Result<(String, Span), ParseError> {
        let here = self.peek().clone();
        match here.token {
            Token::StringLit(value) => {
                self.bump();
                Ok((value, here.span))
            }
            other => Err(unexpected(other, here.span, expected)),
        }
    }

    /// Consume a binding key: an identifier, or `on` (a keyword at member level, a key before `:` in a view).
    fn bump_binding_key(&mut self) -> Result<(String, Span), ParseError> {
        let here = self.peek().clone();
        match here.token {
            Token::Ident(name) => {
                self.bump();
                Ok((name, here.span))
            }
            Token::On => {
                self.bump();
                Ok(("on".to_string(), here.span))
            }
            other => Err(unexpected(other, here.span, "binding key")),
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
        let mut imports = Vec::new();
        let mut components = Vec::new();
        while !self.at_eof() {
            // Tier 16: `import <module>.<Component>;` — only at the
            // top level, only before components in practice but the
            // grammar does not force the order (the pipeline
            // resolves imports wherever they appear).
            if matches!(&self.peek().token, Token::Ident(s) if s == "import") {
                imports.push(self.parse_import()?);
            } else {
                components.push(self.parse_component()?);
            }
        }
        let end = self.peek().span.end;
        Ok(File {
            module,
            imports,
            components,
            span: Span::new(start, end),
        })
    }

    fn parse_import(&mut self) -> Result<ImportDecl, ParseError> {
        let kw = self.bump(); // `import`
        let module_tok = self.bump();
        let module = match module_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "module name (identifier) after `import`",
                    found: other,
                    line: module_tok.span.start.line,
                    column: module_tok.span.start.column,
                });
            }
        };
        self.expect(Token::Dot)?;
        let comp_tok = self.bump();
        let component = match comp_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "component name (identifier) after the dot",
                    found: other,
                    line: comp_tok.span.start.line,
                    column: comp_tok.span.start.column,
                });
            }
        };
        let semi = self.expect(Token::Semi)?;
        let span = Span::new(kw.span.start, semi.span.end);
        Ok(ImportDecl {
            module,
            component,
            span,
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
        let mut state = Vec::new();
        let mut requires = Vec::new();
        let mut schedules = Vec::new();
        let mut events = Vec::new();
        let mut view: Option<ViewBlock> = None;
        loop {
            match &self.peek().token {
                Token::RBrace => break,
                // `start` and `every` are keywords only here, at the start of a member (docs/schedules.md section 3).
                Token::Ident(word) if word == "start" || word == "every" => {
                    schedules.push(self.parse_schedule_decl()?)
                }
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
                        expected: "component member (`state`, `requires`, `start`, `every`, `on`, `view`)",
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
            schedules,
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
        if name == "Option" {
            // `Option[T]`
            self.expect(Token::LBracket)?;
            let some = self.parse_type()?;
            self.expect(Token::RBracket)?;
            return Ok(Type::Option {
                some: Box::new(some),
            });
        }
        if name == "List" {
            // `List[T]`
            self.expect(Token::LBracket)?;
            let elem = self.parse_type()?;
            self.expect(Token::RBracket)?;
            return Ok(Type::List {
                elem: Box::new(elem),
            });
        }
        Type::from_ident(&name).ok_or_else(|| ParseError::Unexpected {
            expected: "type (`Int`, `Float`, `Bool`, `String`, `Result[T, E]`, or `Request[T]`)",
            found: Token::Ident(name.clone()),
            line: start.start.line,
            column: start.start.column,
        })
    }

    /// `start Event;` or `every <ms> Event;` (docs/schedules.md section 3).
    fn parse_schedule_decl(&mut self) -> Result<Schedule, ParseError> {
        let kw = self.bump();
        let every = matches!(&kw.token, Token::Ident(w) if w == "every");
        let kind = if every {
            let tok = self.bump();
            match tok.token {
                Token::IntLit(ms) => ScheduleKind::Every(ms),
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "a period in milliseconds (an integer literal)",
                        found: other,
                        line: tok.span.start.line,
                        column: tok.span.start.column,
                    });
                }
            }
        } else {
            ScheduleKind::Start
        };
        let name_tok = self.bump();
        let event = match name_tok.token {
            Token::Ident(s) => s,
            other => {
                return Err(ParseError::Unexpected {
                    expected: "event name (identifier)",
                    found: other,
                    line: name_tok.span.start.line,
                    column: name_tok.span.start.column,
                });
            }
        };
        let semi = self.expect(Token::Semi)?;
        Ok(Schedule {
            kind,
            event,
            span: Span::new(kw.span.start, semi.span.end),
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
        let mut bounds = Vec::new();
        if self.peek().token == Token::LParen {
            self.bump();
            loop {
                let tok = self.bump();
                match tok.token {
                    Token::StringLit(s) => bounds.push(s),
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "a string literal bound (a capability bound is written in the source, not computed)",
                            found: other,
                            line: tok.span.start.line,
                            column: tok.span.start.column,
                        });
                    }
                }
                if self.peek().token == Token::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect(Token::RParen)?;
        }
        let end_tok = self.expect(Token::Semi)?;
        Ok(CapabilityRef {
            name,
            bounds,
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
        Ok(StmtBlock {
            stmts,
            span: Span::new(start, end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        // An assignment is `identifier = expr;`.
        let is_assign = matches!(self.peek().token, Token::Ident(_))
            && matches!(
                self.tokens.get(self.pos + 1).map(|t| &t.token),
                Some(Token::Eq)
            );
        if is_assign {
            let (name, name_span) = self.bump_ident("assignment target")?;
            self.expect(Token::Eq)?;
            let value = self.parse_expr()?;
            let end_tok = self.expect(Token::Semi)?;
            return Ok(Stmt::Assign {
                target: name,
                value,
                span: Span::new(name_span.start, end_tok.span.end),
            });
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

    /// Is the token after the current one a binary operator (so the current string literal starts an expression)?
    fn next_is_binary_operator(&self) -> bool {
        matches!(
            self.tokens.get(self.pos + 1).map(|t| &t.token),
            Some(
                Token::Plus
                    | Token::Minus
                    | Token::Star
                    | Token::Slash
                    | Token::EqEq
                    | Token::BangEq
                    | Token::Lt
                    | Token::LtEq
                    | Token::Gt
                    | Token::GtEq
                    | Token::AmpAmp
                    | Token::PipePipe
            )
        )
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let (op, prec) = match &self.peek().token {
                Token::PipePipe => (BinOp::Or, 2),
                Token::AmpAmp => (BinOp::And, 3),
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
        if matches!(self.peek().token, Token::If) {
            return self.parse_if();
        }
        // `[a, b, c]` — a `List[T]` literal. Tier 8.
        if matches!(self.peek().token, Token::LBracket) {
            return self.parse_list_lit();
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
        if matches!(self.peek().token, Token::Bang) {
            self.bump();
            let operand = self.parse_unary()?;
            let span = Span::new(start, operand.span().end);
            return Ok(Expr::Unary {
                op: UnOp::Not,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_primary()
    }

    /// `if cond { a } else { b }` and `if cond { a } else if cond2 { b } else { c }` (docs/conditionals.md). A branch is
    /// one expression in braces, as a `match` arm is. A missing `else` parses (as `None`) so that lowering refuses it
    /// with `if-else-required` rather than as a parse error.
    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        let kw = self.expect(Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(Token::LBrace)?;
        let then = self.parse_expr()?;
        let then_close = self.expect(Token::RBrace)?;
        let mut end = then_close.span.end;
        let else_ = if matches!(self.peek().token, Token::Else) {
            self.bump();
            if matches!(self.peek().token, Token::If) {
                let nested = self.parse_if()?;
                end = nested.span().end;
                Some(Box::new(nested))
            } else {
                self.expect(Token::LBrace)?;
                let body = self.parse_expr()?;
                let close = self.expect(Token::RBrace)?;
                end = close.span.end;
                Some(Box::new(body))
            }
        } else {
            None
        };
        Ok(Expr::If {
            cond: Box::new(cond),
            then: Box::new(then),
            else_,
            span: Span::new(kw.span.start, end),
        })
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

    fn parse_list_lit(&mut self) -> Result<Expr, ParseError> {
        // `[a, b, c]` — a `List[T]` literal. Items are plain
        // expressions; trailing commas are not accepted.
        let open = self.expect(Token::LBracket)?;
        let mut items = Vec::new();
        loop {
            if matches!(self.peek().token, Token::RBracket) {
                break;
            }
            items.push(self.parse_expr()?);
            if matches!(self.peek().token, Token::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        let close = self.expect(Token::RBracket)?;
        Ok(Expr::ListLit {
            items,
            span: Span::new(open.span.start, close.span.end),
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
            return Ok(Pattern::Wildcard {
                span: Span::new(start, start),
            });
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
        // `Cancelled` and `None` are known zero-payload variants. Other
        // bare identifiers are still bindings (`Ident` pattern); the
        // match arm body decides whether to use them.
        if (name == "Cancelled" || name == "None") && !matches!(self.peek().token, Token::LParen) {
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
                Ok(Expr::IntLit {
                    value: v,
                    span: tok.span,
                })
            }
            Token::FloatLit(v) => {
                self.bump();
                Ok(Expr::FloatLit {
                    value: v,
                    span: tok.span,
                })
            }
            Token::StringLit(s) => {
                self.bump();
                let parts = interpolation_parts(&s, tok.span)?;
                if !parts.iter().any(|p| matches!(p, TextPart::Expr { .. })) {
                    // No interpolation: the literal text, with `\{` and `\}` already braces.
                    let value: String = parts
                        .iter()
                        .map(|p| match p {
                            TextPart::Literal(t) => t.as_str(),
                            TextPart::Expr { .. } => "",
                        })
                        .collect();
                    return Ok(Expr::StringLit {
                        value,
                        span: tok.span,
                    });
                }
                // `"a {x} b"` in an expression is `("a " + x) + " b"`: the same text a `Text` would show
                // (JavaScript's `+` on a string converts the other side the way `String(..)` does). It starts
                // from a string so that `"{a}{b}"` concatenates and does not add.
                let mut items: Vec<Expr> = Vec::new();
                if matches!(parts.first(), Some(TextPart::Expr { .. })) {
                    items.push(Expr::StringLit {
                        value: String::new(),
                        span: tok.span,
                    });
                }
                for part in parts {
                    items.push(match part {
                        TextPart::Literal(value) => Expr::StringLit {
                            value,
                            span: tok.span,
                        },
                        TextPart::Expr { expr, .. } => expr,
                    });
                }
                let mut it = items.into_iter();
                // `interpolation_parts` always returns at least one part; an empty text would be the empty string.
                let mut acc = it.next().unwrap_or(Expr::StringLit {
                    value: String::new(),
                    span: tok.span,
                });
                for rhs in it {
                    acc = Expr::Binary {
                        op: BinOp::Add,
                        lhs: Box::new(acc),
                        rhs: Box::new(rhs),
                        span: tok.span,
                    };
                }
                Ok(acc)
            }
            Token::Ident(s) if s == "true" || s == "false" => {
                self.bump();
                Ok(Expr::BoolLit {
                    value: s == "true",
                    span: tok.span,
                })
            }
            Token::Ident(_) => {
                // A path: `ident (.ident)*` optionally followed by
                // `(args)` as a builtin call. Bare single-segment
                // paths are identifiers; multi-segment paths must
                // be calls (`clock.now`, not just `clock.now`).
                let (first_name, first_span) = self.bump_ident("identifier")?;
                let mut path = vec![first_name];
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
                    let call_start = first_span.start;
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.peek().token, Token::RParen) {
                        loop {
                            let arg = self.parse_expr()?;
                            // `x => body` — a lambda argument. The
                            // head must be a bare identifier; the
                            // lowering refuses lambdas outside the
                            // `filter`/`map` call forms
                            // (`lambda-unsupported`).
                            if matches!(self.peek().token, Token::FatArrow) {
                                let param = match &arg {
                                    Expr::Ident { name, .. } => name.clone(),
                                    other => {
                                        let s = other.span();
                                        return Err(ParseError::Unexpected {
                                            expected: "identifier before `=>` (lambda parameter)",
                                            found: Token::FatArrow,
                                            line: s.start.line,
                                            column: s.start.column,
                                        });
                                    }
                                };
                                self.bump(); // =>
                                let body = self.parse_expr()?;
                                let span = Span::new(arg.span().start, body.span().end);
                                args.push(Expr::Lambda {
                                    param,
                                    body: Box::new(body),
                                    span,
                                });
                            } else {
                                args.push(arg);
                            }
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
                        let Ok([v]) = <[Expr; 1]>::try_from(args) else {
                            return Err(ParseError::Unexpected {
                                expected: "exactly one argument to `Ok(...)`",
                                found: Token::LParen,
                                line: call_start.line,
                                column: call_start.column,
                            });
                        };
                        return Ok(Expr::Ok {
                            value: Box::new(v),
                            span: Span::new(call_start, call_end),
                        });
                    }
                    if path.len() == 1 && path[0] == "Err" {
                        let Ok([v]) = <[Expr; 1]>::try_from(args) else {
                            return Err(ParseError::Unexpected {
                                expected: "exactly one argument to `Err(...)`",
                                found: Token::LParen,
                                line: call_start.line,
                                column: call_start.column,
                            });
                        };
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
                    // `Some(value)` — Tier 8. `Option[T]` value
                    // constructor, lifted here (single-segment
                    // path with exactly one argument), exactly like
                    // `Ok(...)` above.
                    if path.len() == 1 && path[0] == "Some" {
                        let Ok([v]) = <[Expr; 1]>::try_from(args) else {
                            return Err(ParseError::Unexpected {
                                expected: "exactly one argument to `Some(...)`",
                                found: Token::LParen,
                                line: call_start.line,
                                column: call_start.column,
                            });
                        };
                        return Ok(Expr::Some {
                            value: Box::new(v),
                            span: Span::new(call_start, call_end),
                        });
                    }
                    // `None` — Tier 8. The zero-payload `Option`
                    // constructor. `None(...)` with arguments is
                    // refused; you cannot construct a payloaded
                    // None.
                    if path.len() == 1 && path[0] == "None" {
                        if !args.is_empty() {
                            return Err(ParseError::Unexpected {
                                expected: "`None` takes no arguments",
                                found: Token::LParen,
                                line: call_start.line,
                                column: call_start.column,
                            });
                        }
                        return Ok(Expr::None {
                            span: Span::new(call_start, call_end),
                        });
                    }
                    Ok(Expr::Call {
                        path,
                        args,
                        span: Span::new(call_start, call_end),
                    })
                } else {
                    match <[String; 1]>::try_from(path) {
                        Ok([name]) => {
                            // A bare `None` is the zero-payload `Option`
                            // constructor, not an identifier binding.
                            if name == "None" {
                                return Ok(Expr::None { span: first_span });
                            }
                            Ok(Expr::Ident {
                                name,
                                span: first_span,
                            })
                        }
                        Err(_) => Err(ParseError::Unexpected {
                            expected: "`(` (call) or `;` (statement boundary)",
                            found: self.peek().token.clone(),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        }),
                    }
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
            Token::Ident(_) => {
                // `For { ... }` — Tier 8 row-repetition node. (The
                // name `For` is therefore not usable as a component
                // kind; it is reserved for this shape.)
                let is_for = matches!(&self.peek().token, Token::Ident(s) if s == "For");
                let followed_by_brace = matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::LBrace)
                );
                if is_for && followed_by_brace {
                    let kw_tok = self.bump();
                    return self.parse_for_node(kw_tok.span);
                }
                self.parse_element_node()
            }
            other => Err(ParseError::Unexpected {
                expected: "view node (string literal or element)",
                found: other.clone(),
                line: self.peek().span.start.line,
                column: self.peek().span.start.column,
            }),
        }
    }

    /// `For { over: list as: item ... }` — Tier 8 row repetition.
    /// `over:` names the state-field list; `as:` names the loop
    /// binding visible inside the children. Children are ordinary
    /// view nodes. The `For` token is already consumed by the
    /// caller; `kw_span` is its span.
    fn parse_for_node(&mut self, kw_span: crate::span::Span) -> Result<ViewNode, ParseError> {
        self.expect(Token::LBrace)?;
        let mut over: Option<Expr> = None;
        let mut binding: Option<String> = None;
        loop {
            if let Token::Ident(name) = &self.peek().token {
                let key = name.clone();
                if key == "over" || key == "as" {
                    let key_tok = self.bump();
                    self.expect(Token::DotDot)?;
                    // Tier 11: `over:` is a general (pure)
                    // expression — a state list, or a derived one
                    // via `filter`. `as:` stays a binding name.
                    let val = if key == "over" {
                        self.parse_expr()?
                    } else {
                        let val_tok = self.bump();
                        match val_tok.token {
                            Token::Ident(s) => Expr::Ident {
                                name: s,
                                span: val_tok.span,
                            },
                            other => {
                                return Err(ParseError::Unexpected {
                                    expected: "binding name after `as:`",
                                    found: other,
                                    line: val_tok.span.start.line,
                                    column: val_tok.span.start.column,
                                });
                            }
                        }
                    };
                    if key == "over" {
                        over = Some(val);
                    } else {
                        let val_name = match val {
                            Expr::Ident { name, .. } => name,
                            other => {
                                let s = other.span();
                                return Err(ParseError::Unexpected {
                                    expected: "binding name after `as:`",
                                    found: Token::Eof,
                                    line: s.start.line,
                                    column: s.start.column,
                                });
                            }
                        };
                        binding = Some(val_name);
                    }
                    let _ = key_tok;
                    continue;
                }
            }
            break;
        }
        let over = over.ok_or(ParseError::Unexpected {
            expected: "`over: <state field name>` in For block",
            found: Token::Eof,
            line: kw_span.start.line,
            column: kw_span.start.column,
        })?;
        let binding = binding.ok_or(ParseError::Unexpected {
            expected: "`as: <binding name>` in For block",
            found: Token::Eof,
            line: kw_span.start.line,
            column: kw_span.start.column,
        })?;
        let mut children = Vec::new();
        // Children: only elements (`Child { ... }`) and string/text
        // bodies. No further bindings after `over:`/`as:`.
        loop {
            match &self.peek().token {
                Token::RBrace => break,
                Token::StringLit(_) => children.push(self.parse_text_node()?),
                Token::Ident(_) => {
                    // Lookahead `{` next -> child element.
                    let is_element = matches!(
                        self.tokens.get(self.pos + 1).map(|t| &t.token),
                        Some(Token::LBrace)
                    );
                    if !is_element {
                        return Err(ParseError::Unexpected {
                            expected: "child element (`Name { ... }`) or text",
                            found: self.peek().token.clone(),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                    children.push(self.parse_element_node()?);
                }
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "child element, text, or `}`",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        }
        let close = self.bump(); // }
        Ok(ViewNode::For {
            over,
            binding,
            children,
            span: Span::new(kw_span.start, close.span.end),
        })
    }

    fn parse_text_node(&mut self) -> Result<ViewNode, ParseError> {
        // Text body: either a string literal (with `{expr}`
        // interpolations) or a general expression whose result
        // becomes the text. Tier 6 supports both because
        // `Text { match result { ... } }` needs the expression
        // form to render a `Result` value conditionally.
        // A string literal followed by `+` is the start of a concatenation, an expression like any other
        // (`Text { "ready: " + ready }`); on its own it is the interpolated text.
        let concatenation = matches!(
            self.tokens.get(self.pos + 1).map(|t| &t.token),
            Some(Token::Plus)
        );
        if matches!(self.peek().token, Token::StringLit(_)) && !concatenation {
            let (value, tok_span) = self.bump_string_lit("string literal")?;
            let parts = interpolation_parts(&value, tok_span)?;
            Ok(ViewNode::Text {
                value: TextValue::Interpolated(parts),
                span: tok_span,
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
        let (name, name_span) = self.bump_ident("element name")?;
        // `For { ... }` can appear anywhere a normal element can
        // (including as a child of another element). Route it to
        // the Tier 8 loop parser. The name token is already
        // consumed here.
        if name == "For" {
            return self.parse_for_node(name_span);
        }
        let kind = match name.as_str() {
            "Column" => ElementKind::Column,
            "Row" => ElementKind::Row,
            "Table" => ElementKind::Table,
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
        let mut accessibility = Accessibility {
            role: None,
            label: None,
            label_expr: None,
            span: open.span,
        };
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
                    // A lone string literal is the static label (a `{` in it is refused); anything else is an
                    // expression, evaluated at each render (docs/row-actions.md section 1).
                    let lone_literal = matches!(self.peek().token, Token::StringLit(_))
                        && !self.next_is_binary_operator();
                    if lone_literal {
                        let (s, label_span) = self.bump_string_lit("label string literal")?;
                        accessibility.label = Some(static_text(s, label_span)?);
                        accessibility.span = Span::new(accessibility.span.start, label_span.end);
                    } else {
                        match &self.peek().token {
                            Token::StringLit(_)
                            | Token::Ident(_)
                            | Token::IntLit(_)
                            | Token::FloatLit(_)
                            | Token::LParen => {}
                            other => {
                                return Err(ParseError::Unexpected {
                                    expected: "label string literal or expression",
                                    found: other.clone(),
                                    line: self.peek().span.start.line,
                                    column: self.peek().span.start.column,
                                });
                            }
                        }
                        let expr = self.parse_expr()?;
                        accessibility.span = Span::new(accessibility.span.start, expr.span().end);
                        accessibility.label_expr = Some(expr);
                    }
                }
                Token::Ident(name) if name == "tone" => {
                    // `tone: "error"` or `tone: outcome_tone` (a String state field): a status text's tone
                    // (docs/presentation.md section 3). It is a binding like the others; it may come first because
                    // a `Text` has no other place to put one.
                    let key_tok = self.bump();
                    self.expect(Token::DotDot)?;
                    let value = self.parse_binding_value("tone", &kind)?;
                    let span = Span::new(key_tok.span.start, self.tokens[self.pos - 1].span.end);
                    bindings.push(Binding {
                        key: "tone".to_string(),
                        value,
                        span,
                    });
                }
                Token::RBrace => break,
                Token::StringLit(_) => break,
                Token::Ident(_) => break,
                Token::On => break,
                Token::Match | Token::If => break,
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
                Token::StringLit(_)
                | Token::Ident(_)
                | Token::IntLit(_)
                | Token::FloatLit(_)
                | Token::LParen
                | Token::Minus
                | Token::Match
                | Token::If => {
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
                            let (key, key_span) = self.bump_binding_key()?;
                            self.expect(Token::DotDot)?;
                            let value = self.parse_binding_value(&key, &kind)?;
                            let span = Span::new(key_span.start, self.peek().span.end);
                            bindings.push(Binding { key, value, span });
                        }
                    }
                    Token::StringLit(_) => children.push(self.parse_text_node()?),
                    _ => {
                        return Err(ParseError::Unexpected {
                            expected: "binding or view node",
                            found: self.peek().token.clone(),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                }
            }
        }
        let close = self.bump(); // }
        Ok(ViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span: Span::new(name_span.start, close.span.end),
        })
    }

    fn parse_binding_value(
        &mut self,
        key: &str,
        kind: &ElementKind,
    ) -> Result<BindingValue, ParseError> {
        // Currently the only supported binding values are:
        //   - string literal (e.g. `label: "Increment"`),
        //   - `on: EventName` (event reference),
        //   - `role: heading` etc.
        // Phase 2 adds typed expressions.
        match key {
            "on" | "submit" => {
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
                // `on: Pick(m)`: the arguments are expressions of the view (docs/row-actions.md section 2).
                if key == "on" && matches!(self.peek().token, Token::LParen) {
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
                    self.expect(Token::RParen)?;
                    return Ok(BindingValue::EventCall { name, args });
                }
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
            "tone" => {
                // A literal (`"error"`) or the name of a String state field.
                let tok = self.bump();
                match tok.token {
                    Token::StringLit(s) => Ok(BindingValue::String(static_text(s, tok.span)?)),
                    Token::Ident(s) => Ok(BindingValue::StateRef(s)),
                    other => Err(ParseError::Unexpected {
                        expected: "`\"ok\"`, `\"warn\"`, `\"error\"` or the name of a String state field",
                        found: other,
                        line: tok.span.start.line,
                        column: tok.span.start.column,
                    }),
                }
            }
            "value" => {
                // `value: <ident>` — bind to a state field, or
                // `value: <string>` for a literal initial value.
                let val_tok = self.bump();
                match val_tok.token {
                    Token::Ident(s) => Ok(BindingValue::StateRef(s)),
                    Token::StringLit(s) => Ok(BindingValue::String(static_text(s, val_tok.span)?)),
                    other => Err(ParseError::Unexpected {
                        expected: "state field name (identifier) or string literal",
                        found: other,
                        line: val_tok.span.start.line,
                        column: val_tok.span.start.column,
                    }),
                }
            }
            _ => {
                // Default: string literal, or a constant initial
                // value for a component-instance property (Tier
                // 13): an int/float/bool literal lifts to `Prop`.
                // String literals stay `String` (plain labels
                // everywhere else); the shape pass interprets a
                // `String` value on an instance as a string
                // property. Tier 14: on an instance, a bare
                // identifier is a *dynamic property* — the value
                // is read from the parent component's state (or an
                // enclosing `For` binding) at each render.
                let lit_tok = self.bump();
                match lit_tok.token {
                    Token::StringLit(s) => Ok(BindingValue::String(static_text(s, lit_tok.span)?)),
                    Token::IntLit(v) => Ok(BindingValue::Prop(Expr::IntLit {
                        value: v,
                        span: lit_tok.span,
                    })),
                    Token::FloatLit(v) => Ok(BindingValue::Prop(Expr::FloatLit {
                        value: v,
                        span: lit_tok.span,
                    })),
                    Token::Ident(k) if k == "true" || k == "false" => {
                        Ok(BindingValue::Prop(Expr::BoolLit {
                            value: k == "true",
                            span: lit_tok.span,
                        }))
                    }
                    Token::Ident(s)
                        if matches!(kind, ElementKind::Named(_)) || key == "disabled" =>
                    {
                        // `disabled: sending` names a Bool state field (docs/controls.md section 1).
                        Ok(BindingValue::StateRef(s))
                    }
                    other => Err(ParseError::Unexpected {
                        expected: "string literal (or a constant property value: int/float/bool literal)",
                        found: other,
                        line: lit_tok.span.start.line,
                        column: lit_tok.span.start.column,
                    }),
                }
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
        Token::If => "`if`",
        Token::Else => "`else`",
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
        Token::AmpAmp => "`&&`",
        Token::PipePipe => "`||`",
        Token::Bang => "`!`",
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
        let f = parse_source("component C { state count: Int = 0; view { Text { \"x\" } } }\n")
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
        let f =
            parse_source("component C { state x: Int = 0; view { Text { \"Count: {x}\" } } }\n")
                .expect("parse");
        // The view body is a Text element whose single child is the
        // interpolated text node.
        match &f.components[0].view.body {
            ViewNode::Element { kind, children, .. } => {
                assert!(matches!(kind, ElementKind::Text));
                assert_eq!(children.len(), 1);
                match &children[0] {
                    ViewNode::Text {
                        value: TextValue::Interpolated(parts),
                        ..
                    } => {
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

    fn parser_over(src: &str) -> Parser {
        Parser {
            tokens: lex(src).expect("lex"),
            pos: 0,
        }
    }

    #[test]
    fn the_token_helpers_refuse_the_wrong_token_instead_of_panicking() {
        // #62: each used to be a checked match followed by an `unreachable!` extraction.
        let mut p = parser_over("42");
        assert!(matches!(
            p.bump_ident("identifier"),
            Err(ParseError::Unexpected {
                expected: "identifier",
                ..
            })
        ));
        assert!(matches!(
            p.bump_string_lit("string literal"),
            Err(ParseError::Unexpected {
                expected: "string literal",
                ..
            })
        ));
        assert!(matches!(
            p.bump_binding_key(),
            Err(ParseError::Unexpected {
                expected: "binding key",
                ..
            })
        ));
        // nothing was consumed by a refusal
        assert_eq!(p.pos, 0);

        let mut p = parser_over("name \"s\" on");
        assert_eq!(
            p.bump_ident("identifier").map(|x| x.0),
            Ok("name".to_string())
        );
        assert_eq!(
            p.bump_string_lit("string").map(|x| x.0),
            Ok("s".to_string())
        );
        assert_eq!(p.bump_binding_key().map(|x| x.0), Ok("on".to_string()));
        // at the end of the tokens every helper refuses, none panics
        assert!(p.bump_ident("identifier").is_err());
        assert!(p.bump_string_lit("string literal").is_err());
        assert!(p.bump_binding_key().is_err());
    }

    #[test]
    fn a_dotted_path_that_is_not_a_call_is_refused() {
        let src = "component C { state n: Int = 0; on Go { n = a.b; } view { Text { \"x\" } } }\n";
        assert!(parse_source(src).is_err());
    }
}
