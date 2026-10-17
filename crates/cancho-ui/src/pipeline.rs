//! The compile pipeline that ties lexer → parser → type-check →
//! IR lowering → JavaScript emission together. Phase 1 is the
//! minimum that produces a static component render.
//!
//! The diagnostic JSON shape follows `spec/diagnostics/README.md` §4:
//! stable field order, no embedded newlines in `message`, every
//! refusal has a code + severity + tier + subject + message + (when
//! applicable) expected/actual + remediation. Consumers match on `rule`
//! rather than parsing the message.

use std::fs;
use std::path::Path;

use cancho_ui_codegen_web::emit_module;
use cancho_ui_ir::lower::LowerError;
use cancho_ui_ir::{lower_file, serialize::canonical_json};
use cancho_ui_syntax::lexer::LexError;
use cancho_ui_syntax::parser::{parse, ParseError};
use cancho_ui_syntax::Span;
use cancho_ui_types::{check_file, TypeError};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("refused: {0}")]
    Refused(Diagnostic),

    #[error("environment: {0}")]
    Env(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub tier: Tier,
    pub subject: Subject,
    pub span: Option<SpanOut>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<Remediation>,
    pub related: Vec<Related>,
    pub notes: Vec<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Static,
    Tested,
    Runtime,
    Trust,
    Unknown,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Static => "static",
            Tier::Tested => "tested",
            Tier::Runtime => "runtime",
            Tier::Trust => "trust",
            Tier::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    Component { name: String },
    State { component: String, field: String },
    Event { name: String },
    Effect { name: String },
    Capability { name: String },
    Lifecycle { component: String, phase: String },
    Foreign { scope: String, symbol: String },
    Target { name: String },
    Accessibility { component: String, role: String },
    Ir { path: String },
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpanOut {
    pub file: String,
    pub start: PositionOut,
    pub end: PositionOut,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PositionOut {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Remediation {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize)]
pub struct Related {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<SpanOut>,
    pub message: String,
}

impl SpanOut {
    pub fn from_span(span: Span, file: &Path) -> Self {
        SpanOut {
            file: file.display().to_string(),
            start: PositionOut { line: span.start.line, column: span.start.column },
            end: PositionOut { line: span.end.line, column: span.end.column },
        }
    }
}

pub fn compile_to_ir(file: &Path) -> Result<cancho_ui_ir::ir::IrFile, PipelineError> {
    let source = fs::read_to_string(file)
        .map_err(|e| PipelineError::Env(format!("reading `{}`: {}", file.display(), e)))?;

    let tokens = cancho_ui_syntax::lexer::lex(&source)
        .map_err(|e| PipelineError::Refused(lex_to_diag(e, file)))?;

    let ast = parse(tokens).map_err(|e| PipelineError::Refused(parse_to_diag(e, file)))?;

    check_file(&ast).map_err(|e| PipelineError::Refused(check_to_diag(&e, file)))?;

    lower_file(&ast).map_err(|e| PipelineError::Refused(lower_to_diag(&e, file)))
}

fn check_to_diag(e: &cancho_ui_types::CheckError, file: &Path) -> Diagnostic {
    match e {
        cancho_ui_types::CheckError::Shape(t) => type_to_diag(t, file),
        cancho_ui_types::CheckError::CapabilityMissing {
            component,
            capability,
            call_path,
            span,
        } => {
            let (code, severity, tier, subject, message, span) = (
                "capability-missing",
                Severity::Error,
                Tier::Static,
                Subject::Capability { name: capability.clone() },
                format!(
                    "component `{}` performs `{}` which requires the `{}` capability, but `requires {}` is not declared",
                    component, call_path, capability, capability
                ),
                Some(*span),
            );
            Diagnostic {
                code: code.into(),
                severity,
                tier,
                subject,
                message,
                span: span.map(|s| SpanOut::from_span(s, file)),
                expected: None,
                actual: None,
                remediation: Some(Remediation {
                    description: format!("Add `requires {};` to the component header.", capability),
                    suggestion: None,
                    confidence: Confidence::High,
                }),
                related: Vec::new(),
                notes: Vec::new(),
            }
        }
        cancho_ui_types::CheckError::CapabilityUnused {
            component,
            capability,
            span,
        } => {
            let (code, severity, tier, subject, message, span) = (
                "capability-unused",
                Severity::Error,
                Tier::Static,
                Subject::Capability { name: capability.clone() },
                format!(
                    "component `{}` declares `requires {};` but no reachable effect uses it",
                    component, capability
                ),
                Some(*span),
            );
            Diagnostic {
                code: code.into(),
                severity,
                tier,
                subject,
                message,
                span: span.map(|s| SpanOut::from_span(s, file)),
                expected: None,
                actual: None,
                remediation: Some(Remediation {
                    description: format!(
                        "Either use `{}` in an event handler or view, or remove the `requires {};` declaration.",
                        capability_to_example_call(capability),
                        capability
                    ),
                    suggestion: None,
                    confidence: Confidence::High,
                }),
                related: Vec::new(),
                notes: Vec::new(),
            }
        }
    }
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
    let loc = match &d.span {
        Some(s) => format!("{}:{}:{}", s.file, s.start.line, s.start.column),
        None => file.display().to_string(),
    };
    format!("error[{}] at {}: {}", d.code, loc, d.message)
}

pub fn format_diagnostic_json(d: &Diagnostic) -> String {
    serde_json::to_string_pretty(d).expect("Diagnostic serialization is infallible")
}

fn lex_to_diag(e: LexError, file: &Path) -> Diagnostic {
    let (code, severity, tier, subject, message, span) = match &e {
        LexError::UnexpectedChar { ch, line, column } => (
            "lex-unexpected-char",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("unexpected character {:?}", ch),
            Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column + 1 },
            ),
        ),
        LexError::UnterminatedString { line, column } => (
            "lex-unterminated-string",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            "unterminated string literal".to_string(),
            Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column },
            ),
        ),
        LexError::UnknownEscape { ch, line, column } => (
            "lex-unknown-escape",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("unknown escape sequence \\{}", ch),
            Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column + 2 },
            ),
        ),
    };
    Diagnostic {
        code: code.into(),
        severity,
        tier,
        subject,
        message,
        span: Some(SpanOut::from_span(span, file)),
        expected: None,
        actual: None,
        remediation: None,
        related: Vec::new(),
        notes: Vec::new(),
    }
}

fn parse_to_diag(e: ParseError, file: &Path) -> Diagnostic {
    let (code, severity, tier, subject, message, span) = match &e {
        ParseError::Unexpected { expected, found, line, column } => (
            "parse-unexpected-token",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("expected {}, found {:?}", expected, token_name(found)),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column + 1 },
            )),
        ),
        ParseError::UnexpectedEof { expected } => (
            "parse-unexpected-eof",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("unexpected end of file; expected {}", expected),
            None,
        ),
        ParseError::InvalidNumber { raw, line, column } => (
            "parse-invalid-number",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("invalid number literal: {}", raw),
            Some(Span::new(
                cancho_ui_syntax::Position { line: *line, column: *column },
                cancho_ui_syntax::Position { line: *line, column: *column },
            )),
        ),
    };
    Diagnostic {
        code: code.into(),
        severity,
        tier,
        subject,
        message,
        span: span.map(|s| SpanOut::from_span(s, file)),
        expected: None,
        actual: None,
        remediation: None,
        related: Vec::new(),
        notes: Vec::new(),
    }
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

fn type_to_diag(e: &TypeError, file: &Path) -> Diagnostic {
    let (code, severity, tier, subject, message, span) = match e {
        TypeError::MissingView { name, span } => (
            "type-missing-view",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!("component `{}` has no `view` clause", name),
            Some(*span),
        ),
        TypeError::DuplicateComponent { name, .. } => (
            "type-duplicate-component",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!("component `{}` is declared more than once in this file", name),
            None,
        ),
        TypeError::ReservedComponentName { name, span } => (
            "name-reserved",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!("`{}` is a reserved word and cannot be used as a component name", name),
            Some(*span),
        ),
        TypeError::NotPascalCase { name, span } => (
            "type-not-pascal-case",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "component name `{}` is not PascalCase; component names must start with an uppercase letter",
                name
            ),
            Some(*span),
        ),
        TypeError::NotPascalCaseKind { name, span } => (
            "type-not-pascal-case-kind",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "element kind `{}` is not PascalCase; element kinds must match a built-in primitive or a PascalCase component name",
                name
            ),
            Some(*span),
        ),
    };
    Diagnostic {
        code: code.into(),
        severity,
        tier,
        subject,
        message,
        span: span.map(|s| SpanOut::from_span(s, file)),
        expected: None,
        actual: None,
        remediation: None,
        related: Vec::new(),
        notes: Vec::new(),
    }
}

fn lower_to_diag(e: &LowerError, file: &Path) -> Diagnostic {
    let (code, severity, tier, subject, message, span, remediation) = match e {
        LowerError::EmptySpan { span } => (
            "internal",
            Severity::Error,
            Tier::Unknown,
            Subject::Internal,
            "AST node has empty span (compiler bug)".to_string(),
            Some(*span),
            None,
        ),
        LowerError::CapabilityMissing {
            component,
            capability,
            call_path,
            span,
        } => (
            "capability-missing",
            Severity::Error,
            Tier::Static,
            Subject::Capability {
                name: capability.clone(),
            },
            format!(
                "component `{}` performs `{}` which requires the `{}` capability, but `requires {}` is not declared",
                component, call_path, capability, capability
            ),
            Some(*span),
            Some(Remediation {
                description: format!("Add `requires {};` to the component header.", capability),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
        LowerError::CapabilityUnused {
            component,
            capability,
            span,
        } => (
            "capability-unused",
            Severity::Error,
            Tier::Static,
            Subject::Capability {
                name: capability.clone(),
            },
            format!(
                "component `{}` declares `requires {};` but no reachable effect uses it",
                component, capability
            ),
            Some(*span),
            Some(Remediation {
                description: format!(
                    "Either use `{}` in an event handler or view, or remove the `requires {};` declaration.",
                    capability_to_example_call(capability),
                    capability
                ),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
    };
    Diagnostic {
        code: code.into(),
        severity,
        tier,
        subject,
        message,
        span: span.map(|s| SpanOut::from_span(s, file)),
        expected: None,
        actual: None,
        remediation,
        related: Vec::new(),
        notes: Vec::new(),
    }
}

fn capability_to_example_call(capability: &str) -> &'static str {
    match capability {
        "Clock" => "clock.now()",
        _ => "an effect requiring this capability",
    }
}

#[allow(dead_code)]
pub fn emit_ir_json(file: &Path) -> Result<String, PipelineError> {
    let ir = compile_to_ir(file)?;
    Ok(canonical_json(&ir))
}