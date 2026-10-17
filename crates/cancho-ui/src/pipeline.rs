//! The compile pipeline that ties lexer → parser → type-check →
//! IR lowering → JavaScript emission together. Phase 1 is the
//! minimum that produces a static component render.

use std::fs;
use std::path::Path;

use cancho_ui_codegen_web::emit_module;
use cancho_ui_ir::lower::LowerError;
use cancho_ui_ir::{lower_file, serialize::canonical_json};
use cancho_ui_syntax::lexer::LexError;
use cancho_ui_syntax::parser::{parse, ParseError};
use cancho_ui_syntax::Span;
use cancho_ui_types::{check_file, TypeError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("refused: {0}")]
    Refused(Diagnostic),

    #[error("environment: {0}")]
    Env(String),
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<Span>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

pub fn compile_to_ir(file: &Path) -> Result<cancho_ui_ir::ir::IrFile, PipelineError> {
    let source = fs::read_to_string(file)
        .map_err(|e| PipelineError::Env(format!("reading `{}`: {}", file.display(), e)))?;

    let tokens = cancho_ui_syntax::lexer::lex(&source)
        .map_err(|e| PipelineError::Refused(lex_to_diag(e, file)))?;

    let ast = parse(tokens).map_err(|e| PipelineError::Refused(parse_to_diag(e, file)))?;

    check_file(&ast).map_err(|e| PipelineError::Refused(type_to_diag(&e)))?;

    lower_file(&ast).map_err(|e| PipelineError::Refused(lower_to_diag(&e)))
}

pub fn compile_to_js(file: &Path, target: Option<&str>) -> Result<String, PipelineError> {
    if let Some(t) = target {
        if t != "web" {
            return Err(PipelineError::Env(format!(
                "unsupported target `{}`; only `web` is supported in Phase 1",
                t
            )));
        }
    }
    let ir = compile_to_ir(file)?;
    Ok(emit_module(
        &ir,
        Some(file.to_string_lossy().as_ref()),
    ))
}

pub fn format_diagnostic(d: &Diagnostic, file: &Path) -> String {
    let loc = match d.span {
        Some(s) => format!("{}:{}:{}", file.display(), s.start.line, s.start.column),
        None => file.display().to_string(),
    };
    format!("error[{}] at {}: {}", d.code, loc, d.message)
}

fn lex_to_diag(e: LexError, _file: &Path) -> Diagnostic {
    let (code, message, span) = match &e {
        LexError::UnexpectedChar { ch, line, column } => (
            "lex-unexpected-char",
            format!("unexpected character {:?}", ch),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column + 1 },
            )),
        ),
        LexError::UnterminatedString { line, column } => (
            "lex-unterminated-string",
            "unterminated string literal".to_string(),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column },
            )),
        ),
        LexError::UnknownEscape { ch, line, column } => (
            "lex-unknown-escape",
            format!("unknown escape sequence \\{}", ch),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column + 2 },
            )),
        ),
    };
    Diagnostic { code: code.into(), message, span }
}

fn parse_to_diag(e: ParseError, _file: &Path) -> Diagnostic {
    let (code, message, span) = match &e {
        ParseError::Unexpected { expected, found, line, column } => (
            "parse-unexpected-token",
            format!(
                "expected {}, found {:?}",
                expected,
                token_name(found)
            ),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column + 1 },
            )),
        ),
        ParseError::UnexpectedEof { expected } => (
            "parse-unexpected-eof",
            format!("unexpected end of file; expected {}", expected),
            None,
        ),
        ParseError::InvalidNumber { raw, line, column } => (
            "parse-invalid-number",
            format!("invalid number literal: {}", raw),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column },
            )),
        ),
    };
    Diagnostic { code: code.into(), message, span }
}

fn token_name(t: &cancho_ui_syntax::Token) -> &'static str {
    use cancho_ui_syntax::Token;
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

fn type_to_diag(e: &TypeError) -> Diagnostic {
    let (code, message, span) = match e {
        TypeError::MissingView { name, span } => (
            "type-missing-view",
            format!("component `{}` has no `view` clause", name),
            Some(*span),
        ),
        TypeError::DuplicateComponent { name, .. } => (
            "type-duplicate-component",
            format!("component `{}` is declared more than once in this file", name),
            None,
        ),
        TypeError::ReservedComponentName { name, span } => (
            "name-reserved",
            format!("`{}` is a reserved word and cannot be used as a component name", name),
            Some(*span),
        ),
        TypeError::NotPascalCase { name, span } => (
            "type-not-pascal-case",
            format!(
                "component name `{}` is not PascalCase; component names must start with an uppercase letter",
                name
            ),
            Some(*span),
        ),
        TypeError::NotPascalCaseKind { name, span } => (
            "type-not-pascal-case-kind",
            format!(
                "element kind `{}` is not PascalCase; element kinds must match a built-in primitive or a PascalCase component name",
                name
            ),
            Some(*span),
        ),
    };
    Diagnostic { code: code.into(), message, span }
}

fn lower_to_diag(e: &LowerError) -> Diagnostic {
    let (code, message, span) = match e {
        LowerError::EmptySpan { span } => (
            "internal",
            "AST node has empty span (compiler bug)".to_string(),
            Some(*span),
        ),
    };
    Diagnostic { code: code.into(), message, span }
}

#[allow(dead_code)]
pub fn emit_ir_json(file: &Path) -> Result<String, PipelineError> {
    let ir = compile_to_ir(file)?;
    Ok(canonical_json(&ir))
}