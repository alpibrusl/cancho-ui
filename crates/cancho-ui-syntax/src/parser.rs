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

mod data;
mod page;
mod view;

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

    #[error(
        "{what} are nested more than {max} deep at line {line} column {column}; split them up (docs/nesting.md)"
    )]
    TooDeep {
        what: &'static str,
        max: usize,
        line: u32,
        column: u32,
    },
}

/// How deep `if` statements may nest (a handler, a block and each branch each count one).
pub const MAX_STMT_DEPTH: usize = 32;

/// How deep expressions, view nodes and types may nest, counted together (docs/nesting.md): an operand of a parenthesis, a unary
/// operator, a call, a list literal, a `match` arm, an `if` expression, a lambda or an interpolation; a view node inside a view node;
/// a type argument. A chain of binary operators (`a + b + c + ..`) counts one for each operator. A deeper input is
/// `parse-nesting-too-deep`, not a stack overflow in a later pass.
pub const MAX_NEST_DEPTH: usize = 32;

/// The span of a statement.
pub fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Assign { span, .. }
        | Stmt::Expr { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Run { span, .. } => *span,
    }
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
    let mut parser = Parser {
        tokens,
        pos: 0,
        depth: 0,
        nest: 0,
    };
    let file = parser.parse_file()?;
    parser.expect(Token::Eof)?;
    Ok(file)
}

struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    /// How many statement blocks are open (`MAX_STMT_DEPTH`).
    depth: usize,
    /// How many expressions, view nodes and types are open (`MAX_NEST_DEPTH`).
    nest: usize,
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
fn interpolation_parts(value: &str, span: Span, nest: usize) -> Result<Vec<TextPart>, ParseError> {
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
                depth: 0,
                nest,
            };
            let expr = sub.parse_expr().map_err(|e| match e {
                // the depth is counted through the interpolations (`"{"{..}"}"`), and stays what it is
                too_deep @ ParseError::TooDeep { .. } => too_deep,
                e => ParseError::InvalidNumber {
                    raw: format!("{:?}", e),
                    line: current_line,
                    column: 0,
                },
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
    /// One more open expression, view node or type (`MAX_NEST_DEPTH`); the caller leaves with `self.nest -= 1`.
    fn enter_nest(&mut self) -> Result<(), ParseError> {
        if self.nest >= MAX_NEST_DEPTH {
            let at = self.peek().span.start;
            return Err(ParseError::TooDeep {
                what: "expressions, view nodes and types",
                max: MAX_NEST_DEPTH,
                line: at.line,
                column: at.column,
            });
        }
        self.nest += 1;
        Ok(())
    }

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
        let mut themes = Vec::new();
        let mut islands = Vec::new();
        let mut pages = Vec::new();
        let mut data = Vec::new();
        while !self.at_eof() {
            // Tier 16: `import <module>.<Component>;` — only at the
            // top level, only before components in practice but the
            // grammar does not force the order (the pipeline
            // resolves imports wherever they appear).
            if matches!(&self.peek().token, Token::Ident(s) if s == "import") {
                imports.push(self.parse_import()?);
            } else if matches!(&self.peek().token, Token::Ident(s) if s == "theme")
                && matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::LBrace)
                )
            {
                // `theme` is a keyword only here, at the top level and followed by `{` (docs/theme.md section 2).
                themes.push(self.parse_theme_decl()?);
            } else if matches!(&self.peek().token, Token::Ident(s) if s == "page")
                && matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::LBrace)
                )
            {
                // `page` is a keyword only here, at the top level and followed by `{` (docs/page.md section 3).
                pages.push(self.parse_page_decl()?);
            } else if matches!(&self.peek().token, Token::Ident(s) if s == "data")
                && matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::Ident(_))
                )
            {
                // `data` is a keyword only here, at the top level and followed by a name (docs/data.md section 2).
                data.push(self.parse_data_decl()?);
            } else if matches!(&self.peek().token, Token::Ident(s) if s == "island")
                && matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::Ident(_))
                )
            {
                // `island` is a keyword only here, at the top level and followed by a name (docs/islands.md section 4).
                islands.push(self.parse_island_decl()?);
            } else {
                components.push(self.parse_component()?);
            }
        }
        let end = self.peek().span.end;
        Ok(File {
            module,
            imports,
            components,
            themes,
            islands,
            pages,
            data,
            span: Span::new(start, end),
        })
    }

    /// `island Name { script: ".." sha256: ".."; style: ".." sha256: ".."; label: ".."; decorative: true; width: 640; height: 600;
    /// props { seed: Int; } events { Selected: Int; Closed; } }`; the current token is `island`.
    fn parse_island_decl(&mut self) -> Result<IslandDecl, ParseError> {
        let kw = self.bump();
        let (name, _) = self.bump_ident("the island's name")?;
        self.expect(Token::LBrace)?;
        let mut entries = Vec::new();
        while !matches!(self.peek().token, Token::RBrace) {
            let (key, key_span) = self.bump_ident(
                "`script`, `style`, `sealed`, `label`, `decorative`, `width`, `height`, `props` or `events`",
            )?;
            match key.as_str() {
                "script" | "style" => {
                    self.expect(Token::DotDot)?;
                    let (path, _) = self.bump_string_lit("the path of the file, as a string")?;
                    let (word, word_span) = self.bump_ident("`sha256`")?;
                    if word != "sha256" {
                        return Err(unexpected(Token::Ident(word), word_span, "`sha256`"));
                    }
                    self.expect(Token::DotDot)?;
                    let (sha256, _) =
                        self.bump_string_lit("the sha256 of the file, as a string")?;
                    let semi = self.expect(Token::Semi)?;
                    let span = Span::new(key_span.start, semi.span.end);
                    entries.push(if key == "script" {
                        IslandEntry::Script { path, sha256, span }
                    } else {
                        IslandEntry::Style { path, sha256, span }
                    });
                }
                "label" => {
                    self.expect(Token::DotDot)?;
                    let (text, _) = self.bump_string_lit("the label, as a string")?;
                    let semi = self.expect(Token::Semi)?;
                    entries.push(IslandEntry::Label {
                        text,
                        span: Span::new(key_span.start, semi.span.end),
                    });
                }
                "sealed" => {
                    self.expect(Token::DotDot)?;
                    let (word, word_span) = self.bump_ident("`true` or `false`")?;
                    let value = match word.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => {
                            return Err(unexpected(
                                Token::Ident(word),
                                word_span,
                                "`true` or `false`",
                            ));
                        }
                    };
                    let semi = self.expect(Token::Semi)?;
                    entries.push(IslandEntry::Sealed {
                        value,
                        span: Span::new(key_span.start, semi.span.end),
                    });
                }
                "decorative" => {
                    self.expect(Token::DotDot)?;
                    let (word, word_span) = self.bump_ident("`true` or `false`")?;
                    let value = match word.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => {
                            return Err(unexpected(
                                Token::Ident(word),
                                word_span,
                                "`true` or `false`",
                            ));
                        }
                    };
                    let semi = self.expect(Token::Semi)?;
                    entries.push(IslandEntry::Decorative {
                        value,
                        span: Span::new(key_span.start, semi.span.end),
                    });
                }
                "width" | "height" => {
                    self.expect(Token::DotDot)?;
                    let here = self.peek().clone();
                    let value = match here.token {
                        Token::IntLit(n) => {
                            self.bump();
                            n
                        }
                        other => {
                            return Err(unexpected(other, here.span, "a whole number of pixels"));
                        }
                    };
                    let semi = self.expect(Token::Semi)?;
                    entries.push(IslandEntry::Size {
                        key,
                        value,
                        span: Span::new(key_span.start, semi.span.end),
                    });
                }
                "props" => {
                    self.expect(Token::LBrace)?;
                    let mut items = Vec::new();
                    while !matches!(self.peek().token, Token::RBrace) {
                        let (n, n_span) = self.bump_ident("a prop's name")?;
                        self.expect(Token::DotDot)?;
                        let (t, _) = self.bump_ident("the prop's type")?;
                        let semi = self.expect(Token::Semi)?;
                        items.push((n, t, Span::new(n_span.start, semi.span.end)));
                    }
                    let close = self.expect(Token::RBrace)?;
                    entries.push(IslandEntry::Props {
                        items,
                        span: Span::new(key_span.start, close.span.end),
                    });
                }
                "events" => {
                    self.expect(Token::LBrace)?;
                    let mut items = Vec::new();
                    while !matches!(self.peek().token, Token::RBrace) {
                        let (n, n_span) = self.bump_ident("an event's name")?;
                        let t = if matches!(self.peek().token, Token::DotDot) {
                            self.bump();
                            Some(self.bump_ident("the event's value type")?.0)
                        } else {
                            None
                        };
                        let semi = self.expect(Token::Semi)?;
                        items.push((n, t, Span::new(n_span.start, semi.span.end)));
                    }
                    let close = self.expect(Token::RBrace)?;
                    entries.push(IslandEntry::Events {
                        items,
                        span: Span::new(key_span.start, close.span.end),
                    });
                }
                _ => {
                    return Err(unexpected(
                        Token::Ident(key),
                        key_span,
                        "`script`, `style`, `sealed`, `label`, `decorative`, `width`, `height`, `props` or `events`",
                    ));
                }
            }
        }
        let close = self.expect(Token::RBrace)?;
        Ok(IslandDecl {
            name,
            entries,
            span: Span::new(kw.span.start, close.span.end),
        })
    }

    /// `theme { light { key: "value"; .. } dark { .. } font role { key: "value"; .. } }`; the current token is `theme`.
    fn parse_theme_decl(&mut self) -> Result<ThemeDecl, ParseError> {
        let kw = self.bump();
        self.expect(Token::LBrace)?;
        let mut blocks = Vec::new();
        while !matches!(self.peek().token, Token::RBrace) {
            let (word, word_span) = self.bump_ident("`light`, `dark` or `font`")?;
            if word == "font" {
                let (role, _) = self.bump_ident("the font's role (body, mono, hand)")?;
                let (entries, close) = self.parse_theme_entries()?;
                blocks.push(ThemeBlock::Font(ThemeFont {
                    role,
                    entries,
                    span: Span::new(word_span.start, close.end),
                }));
            } else {
                let (entries, close) = self.parse_theme_entries()?;
                blocks.push(ThemeBlock::Scheme(ThemeScheme {
                    name: word,
                    entries,
                    span: Span::new(word_span.start, close.end),
                }));
            }
        }
        let close = self.expect(Token::RBrace)?;
        Ok(ThemeDecl {
            blocks,
            span: Span::new(kw.span.start, close.span.end),
        })
    }

    /// `{ key: "value"; .. }`, and the span of the closing brace.
    fn parse_theme_entries(&mut self) -> Result<(Vec<ThemeEntry>, Span), ParseError> {
        self.expect(Token::LBrace)?;
        let mut entries = Vec::new();
        while !matches!(self.peek().token, Token::RBrace) {
            let (key, key_span) = self.bump_ident("a token name")?;
            self.expect(Token::DotDot)?;
            let (value, _) = self.bump_string_lit("a string literal")?;
            let semi = self.expect(Token::Semi)?;
            entries.push(ThemeEntry {
                key,
                value,
                span: Span::new(key_span.start, semi.span.end),
            });
        }
        let close = self.expect(Token::RBrace)?;
        Ok((entries, close.span))
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
        let mut blocks = Vec::new();
        let mut messages = Vec::new();
        let mut view: Option<ViewBlock> = None;
        loop {
            match &self.peek().token {
                Token::RBrace => break,
                // `start` and `every` are keywords only here, at the start of a member (docs/schedules.md section 3).
                Token::Ident(word)
                    if word == "start"
                        || word == "every"
                        || word == "route"
                        || word == "subscribe" =>
                {
                    schedules.push(self.parse_schedule_decl()?)
                }
                // `block` is a keyword only here, followed by a name and `{` (docs/handler-sequences.md section 4).
                Token::Ident(word)
                    if word == "block"
                        && matches!(
                            self.tokens.get(self.pos + 1).map(|t| &t.token),
                            Some(Token::Ident(_))
                        ) =>
                {
                    blocks.push(self.parse_block_decl()?)
                }
                // `messages` is a keyword only here, followed by `{` (docs/i18n.md section 3).
                Token::Ident(word)
                    if word == "messages"
                        && matches!(
                            self.tokens.get(self.pos + 1).map(|t| &t.token),
                            Some(Token::LBrace)
                        ) =>
                {
                    self.parse_messages_decl(&mut messages)?
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
                        expected: "component member (`state`, `requires`, `start`, `every`, `route`, `subscribe`, `on`, `block`, `view`)",
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
            blocks,
            messages,
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
        self.enter_nest()?;
        let result = self.parse_type_inner();
        self.nest -= 1;
        result
    }

    fn parse_type_inner(&mut self) -> Result<Type, ParseError> {
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

    /// `start Event;`, `every <ms> Event;` or `route Event;` (docs/schedules.md section 3, docs/routing.md section 4).
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
        } else if matches!(&kw.token, Token::Ident(w) if w == "route") {
            ScheduleKind::Route
        } else if matches!(&kw.token, Token::Ident(w) if w == "subscribe") {
            ScheduleKind::Subscribe
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
        // docs/streams.md: `subscribe Event "url" [status Event];`
        let stream = if kind == ScheduleKind::Subscribe {
            let url_tok = self.bump();
            let url = match url_tok.token {
                Token::StringLit(s) => s,
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "the address to listen to (a string literal)",
                        found: other,
                        line: url_tok.span.start.line,
                        column: url_tok.span.start.column,
                    });
                }
            };
            let status = if matches!(&self.peek().token, Token::Ident(w) if w == "status") {
                self.bump();
                let tok = self.bump();
                match tok.token {
                    Token::Ident(s) => Some(s),
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "status event name (identifier)",
                            found: other,
                            line: tok.span.start.line,
                            column: tok.span.start.column,
                        });
                    }
                }
            } else {
                None
            };
            Some(StreamDecl { url, status })
        } else {
            None
        };
        let semi = self.expect(Token::Semi)?;
        Ok(Schedule {
            kind,
            event,
            stream,
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
        let stmts = self.parse_stmts()?;
        let end = self.peek().span.end;
        Ok(StmtBlock {
            stmts,
            span: Span::new(start, end),
        })
    }

    /// Statements up to (not including) the closing brace. Each open block counts one toward `MAX_STMT_DEPTH`, so a deeply
    /// nested input is a parse error, not a stack overflow.
    fn parse_stmts(&mut self) -> Result<Vec<Stmt>, ParseError> {
        if self.depth >= MAX_STMT_DEPTH {
            let at = self.peek().span.start;
            return Err(ParseError::TooDeep {
                what: "statements",
                max: MAX_STMT_DEPTH,
                line: at.line,
                column: at.column,
            });
        }
        self.depth += 1;
        let mut stmts = Vec::new();
        let result = loop {
            if matches!(self.peek().token, Token::RBrace) {
                break Ok(());
            }
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(e) => break Err(e),
            }
        };
        self.depth -= 1;
        result.map(|()| stmts)
    }

    /// `block Name { stmts }`; the word `block` is the current token.
    /// `messages { lang { key: "text"; key(n): one "..", other ".."; } .. }`; the current token is `messages`.
    fn parse_messages_decl(&mut self, out: &mut Vec<MessageCatalog>) -> Result<(), ParseError> {
        self.bump();
        self.expect(Token::LBrace)?;
        while !matches!(self.peek().token, Token::RBrace) {
            let (lang, lang_span) = self.bump_ident("language code")?;
            self.expect(Token::LBrace)?;
            let mut entries = Vec::new();
            while !matches!(self.peek().token, Token::RBrace) {
                let (key, key_span) = self.bump_ident("message key")?;
                let mut plural = None;
                if matches!(self.peek().token, Token::LParen) {
                    self.bump();
                    plural = Some(self.bump_ident("the count's name")?.0);
                    self.expect(Token::RParen)?;
                }
                self.expect(Token::DotDot)?;
                let mut forms = Vec::new();
                if plural.is_some() {
                    loop {
                        let (cat, _) =
                            self.bump_ident("plural category (one, few, many, other ..)")?;
                        let text = self.bump_string_lit("the text of the form")?.0;
                        forms.push((Some(cat), text));
                        if matches!(self.peek().token, Token::Comma) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                } else {
                    forms.push((None, self.bump_string_lit("the text of the message")?.0));
                }
                let semi = self.expect(Token::Semi)?;
                entries.push(MessageEntry {
                    key,
                    plural,
                    forms,
                    span: Span::new(key_span.start, semi.span.end),
                });
            }
            let close = self.expect(Token::RBrace)?;
            out.push(MessageCatalog {
                lang,
                entries,
                span: Span::new(lang_span.start, close.span.end),
            });
        }
        self.expect(Token::RBrace)?;
        Ok(())
    }

    fn parse_block_decl(&mut self) -> Result<BlockDecl, ParseError> {
        let kw = self.bump();
        let (name, _) = self.bump_ident("block name")?;
        self.expect(Token::LBrace)?;
        let body = self.parse_stmt_block()?;
        let close = self.expect(Token::RBrace)?;
        Ok(BlockDecl {
            name,
            body,
            span: Span::new(kw.span.start, close.span.end),
        })
    }

    /// `if cond { stmts }` with an optional `else { stmts }` or `else if ..`; the `if` is the current token.
    fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        let kw = self.expect(Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(Token::LBrace)?;
        let then = self.parse_stmts()?;
        let then_close = self.expect(Token::RBrace)?;
        let mut end = then_close.span.end;
        let mut else_ = Vec::new();
        if matches!(self.peek().token, Token::Else) {
            self.bump();
            if matches!(self.peek().token, Token::If) {
                if self.depth >= MAX_STMT_DEPTH {
                    let at = self.peek().span.start;
                    return Err(ParseError::TooDeep {
                        what: "statements",
                        max: MAX_STMT_DEPTH,
                        line: at.line,
                        column: at.column,
                    });
                }
                self.depth += 1;
                let nested = self.parse_if_stmt();
                self.depth -= 1;
                let nested = nested?;
                end = stmt_span(&nested).end;
                else_.push(nested);
            } else {
                self.expect(Token::LBrace)?;
                else_ = self.parse_stmts()?;
                end = self.expect(Token::RBrace)?.span.end;
            }
        }
        Ok(Stmt::If {
            cond,
            then,
            else_,
            span: Span::new(kw.span.start, end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        // A statement that starts with `if` is the statement form; `x = if c { 1 } else { 2 };` is the expression.
        if matches!(self.peek().token, Token::If) {
            return self.parse_if_stmt();
        }
        // `run Name;` (`run` is a word only here, followed by a name and `;`).
        let is_run = matches!(&self.peek().token, Token::Ident(w) if w == "run")
            && matches!(
                self.tokens.get(self.pos + 1).map(|t| &t.token),
                Some(Token::Ident(_))
            )
            && matches!(
                self.tokens.get(self.pos + 2).map(|t| &t.token),
                Some(Token::Semi)
            );
        if is_run {
            self.bump();
            let (name, _) = self.bump_ident("block name")?;
            let end_tok = self.expect(Token::Semi)?;
            return Ok(Stmt::Run {
                name,
                span: Span::new(start, end_tok.span.end),
            });
        }
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
        // each operator folded onto `left` is one more level of the tree: a chain of 100,000 `+` is as deep as 100,000 parentheses
        let mut links = 0;
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
            links += 1;
            if self.nest + links > MAX_NEST_DEPTH {
                let at = self.peek().span.start;
                return Err(ParseError::TooDeep {
                    what: "expressions, view nodes and types",
                    max: MAX_NEST_DEPTH,
                    line: at.line,
                    column: at.column,
                });
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
        self.enter_nest()?;
        let result = self.parse_unary_inner();
        self.nest -= 1;
        result
    }

    fn parse_unary_inner(&mut self) -> Result<Expr, ParseError> {
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
                // `else if` chains recurse here and not through `parse_unary`: they count as nesting too
                self.enter_nest()?;
                let nested = self.parse_if();
                self.nest -= 1;
                let nested = nested?;
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
                let parts = interpolation_parts(&s, tok.span, self.nest)?;
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
            depth: 0,
            nest: 0,
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
