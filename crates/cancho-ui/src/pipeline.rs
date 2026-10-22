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

use cancho_ui_codegen_web::{EmitOptions, emit_module_with};
use cancho_ui_ir::lower::LowerError;
use cancho_ui_ir::{lower_file, serialize::canonical_json};
use cancho_ui_syntax::Span;
use cancho_ui_syntax::lexer::LexError;
use cancho_ui_syntax::parser::{ParseError, parse};
use cancho_ui_types::{TypeError, check_file_with_externals};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("refused: {0}")]
    Refused(Box<Diagnostic>),

    #[error("environment: {0}")]
    Env(String),
}

impl PipelineError {
    pub fn refused(d: Diagnostic) -> Self {
        PipelineError::Refused(Box::new(d))
    }
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

// The full vocabulary of spec/diagnostics/README.md is declared even where
// no check constructs it yet; a variant that is absent cannot be reported.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Static,
    Tested,
    Runtime,
    Trust,
    Unknown,
}

// See `Tier`: declared vocabulary of spec/diagnostics/README.md.
#[allow(dead_code)]
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

// The full vocabulary of spec/diagnostics/README.md is declared even where
// no check constructs it yet; a variant that is absent cannot be reported.
#[allow(dead_code)]
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
            start: PositionOut {
                line: span.start.line,
                column: span.start.column,
            },
            end: PositionOut {
                line: span.end.line,
                column: span.end.column,
            },
        }
    }
}

pub fn compile_to_ir(file: &Path) -> Result<cancho_ui_ir::ir::IrFile, PipelineError> {
    compile_inner(file).map(|(ir, _)| ir)
}

/// Tier 16: resolve `import <module>.<Component>;` declarations.
/// The imported component lives in the sibling file
/// `<module>.cui` (same directory as the importing file), which
/// must declare `module <module>;`. Resolution is recursive and
/// dependency-ordered: an imported module's own imports are
/// resolved first, a file already on the resolution stack is a
/// cycle (`import-cycle`), and the entry file comes last so the
/// merged IR's auto-mount stays the entry's own last component.
struct ResolvedFile {
    path: std::path::PathBuf,
    ast: cancho_ui_syntax::ast::File,
}

fn import_diag(
    code: &str,
    subject_name: String,
    message: String,
    span: cancho_ui_syntax::Span,
    file: &Path,
    remediation: Option<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        severity: Severity::Error,
        tier: Tier::Static,
        subject: Subject::Component { name: subject_name },
        message,
        span: Some(SpanOut::from_span(span, file)),
        expected: None,
        actual: None,
        remediation: remediation.map(|description| Remediation {
            description,
            suggestion: None,
            confidence: Confidence::High,
        }),
        related: Vec::new(),
        notes: Vec::new(),
    }
}

fn resolve_imports(
    file: &Path,
    ast: &cancho_ui_syntax::ast::File,
    stack: &mut Vec<std::path::PathBuf>,
) -> Result<Vec<ResolvedFile>, PipelineError> {
    let canon = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    if let Some(pos) = stack.iter().position(|p| *p == canon) {
        let mut chain: Vec<String> = stack[pos..]
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        chain.push(canon.display().to_string());
        let last = ast.imports.last();
        let (msg_span, via): (cancho_ui_syntax::Span, String) = match last {
            Some(i) => (
                i.span,
                format!(" via `import {}.{};`", i.module, i.component),
            ),
            None => (ast.span, String::new()),
        };
        let subject = match last {
            Some(i) => format!("{}.{}", i.module, i.component),
            None => "file".to_string(),
        };
        return Err(PipelineError::refused(import_diag(
            "import-cycle",
            subject,
            format!(
                "the import chain cycles: {}{} — a component cannot reach itself through its imports",
                chain.join(" -> "),
                via
            ),
            msg_span,
            file,
            None,
        )));
    }
    stack.push(canon.clone());
    let mut resolved: Vec<ResolvedFile> = Vec::new();
    for imp in &ast.imports {
        let module_path = match file.parent() {
            Some(dir) => dir.join(format!("{}.cui", imp.module)),
            None => std::path::PathBuf::from(format!("{}.cui", imp.module)),
        };
        let source = fs::read_to_string(&module_path).map_err(|_| {
            PipelineError::refused(import_diag(
                "import-missing-module",
                format!("{}.{}", imp.module, imp.component),
                format!(
                    "`import {}.{};` resolves to `{}`, which does not exist; an import's first segment must name a sibling `.cui` file that declares `module {};`",
                    imp.module, imp.component, module_path.display(), imp.module
                ),
                imp.span,
                file,
                Some(format!("Create `{} {{ ... }}` in a sibling `{}.cui`. Make sure it starts with `module {};`.", imp.component, imp.module, imp.module)),
            ))
        })?;
        let toks = cancho_ui_syntax::lexer::lex(&source)
            .map_err(|e| PipelineError::refused(lex_to_diag(e, &module_path)))?;
        let module_ast =
            parse(toks).map_err(|e| PipelineError::refused(parse_to_diag(e, &module_path)))?;
        // The file must declare its module name.
        match &module_ast.module {
            Some(m) if m.name == imp.module => {}
            _ => {
                return Err(PipelineError::refused(import_diag(
                    "import-missing-module",
                    format!("{}.{}", imp.module, imp.component),
                    format!(
                        "`{}` does not declare `module {};` — an imported file must declare the module its importers name",
                        module_path.display(),
                        imp.module
                    ),
                    imp.span,
                    file,
                    Some(format!(
                        "Add `module {};` at the top of `{}.`",
                        imp.module,
                        module_path.display()
                    )),
                )));
            }
        }
        if !module_ast
            .components
            .iter()
            .any(|c| c.name == imp.component)
        {
            return Err(PipelineError::refused(import_diag(
                "import-unknown-component",
                format!("{}.{}", imp.module, imp.component),
                format!(
                    "component `{}` does not exist in `{}` (whose components are: {})",
                    imp.component,
                    module_path.display(),
                    module_ast
                        .components
                        .iter()
                        .map(|c| c.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                imp.span,
                file,
                Some(format!(
                    "Import a component the module declares, or declare `component {} {{ ... }}` in `{}.`",
                    imp.component,
                    module_path.display()
                )),
            )));
        }
        // Recurse first: the module's own imports must be resolved
        // (and cycle-checked) before its interior check runs, so
        // the check can see components imported from sibling
        // modules the same way the entry file does.
        let mut deps = resolve_imports(&module_path, &module_ast, stack)?;
        // The imported file must itself be valid (its interior
        // refusals belong to its own path in diagnostics). Its
        // shape map includes exactly the components its own
        // `import` lines name.
        let inner_externals: Vec<cancho_ui_types::ExternalComponent> = module_ast
            .imports
            .iter()
            .filter_map(|inner| {
                let inner_file = match module_path.parent() {
                    Some(dir) => dir.join(format!("{}.cui", inner.module)),
                    None => std::path::PathBuf::from(format!("{}.cui", inner.module)),
                };
                let r = deps.iter().find(|r| r.path == inner_file)?;
                r.ast
                    .components
                    .iter()
                    .find(|c| c.name == inner.component)
                    .map(|c| cancho_ui_types::ExternalComponent {
                        name: c.name.clone(),
                        state: c
                            .state
                            .iter()
                            .map(|s| (s.name.clone(), s.ty.clone()))
                            .collect(),
                    })
            })
            .collect();
        check_file_with_externals(&module_ast, &inner_externals)
            .map_err(|e| PipelineError::refused(check_to_diag(&e, &module_path)))?;
        resolved.append(&mut deps);
        resolved.push(ResolvedFile {
            path: module_path,
            ast: module_ast,
        });
    }
    stack.pop();
    Ok(resolved)
}

fn compile_inner(
    file: &Path,
) -> Result<(cancho_ui_ir::ir::IrFile, std::path::PathBuf), PipelineError> {
    let source = fs::read_to_string(file)
        .map_err(|e| PipelineError::Env(format!("reading `{}`: {}", file.display(), e)))?;

    let tokens = cancho_ui_syntax::lexer::lex(&source)
        .map_err(|e| PipelineError::refused(lex_to_diag(e, file)))?;

    let ast = parse(tokens).map_err(|e| PipelineError::refused(parse_to_diag(e, file)))?;

    // Tier 16: resolve the entry's imports (dependency order,
    // checked with their own file paths) and hand the entry a
    // component-shape view of the imported modules.
    let resolved = resolve_imports(file, &ast, &mut Vec::new())?;

    // Dedupe: the same module imported twice resolves once.
    let mut unique: Vec<ResolvedFile> = Vec::new();
    for r in resolved {
        if unique.iter().any(|u| u.path == r.path) {
            continue;
        }
        unique.push(r);
    }
    let resolved: Vec<ResolvedFile> = unique;

    // Cross-file name collisions: a component name must be unique
    // across the whole merged module — the emitted JS otherwise
    // carries two `mount_<Name>` definitions and the second one
    // silently wins. `type-duplicate-component` governs the
    // same-file case; this is its cross-file twin.
    let mut seen: Vec<(String, std::path::PathBuf)> = Vec::new();
    let entry_as_resolved = ResolvedFile {
        path: file.to_path_buf(),
        ast: cancho_ui_syntax::ast::File {
            module: ast.module.clone(),
            imports: Vec::new(),
            components: ast.components.clone(),
            span: ast.span,
        },
    };
    let components: Vec<(&cancho_ui_syntax::ast::Component, std::path::PathBuf)> = resolved
        .iter()
        .chain(std::iter::once(&entry_as_resolved))
        .flat_map(|r| r.ast.components.iter().map(move |c| (c, r.path.clone())))
        .collect();
    for (c, path) in &components {
        if let Some((n, other)) = seen.iter().find(|(n, _)| *n == c.name) {
            let (n, other) = (n.clone(), other.clone());
            return Err(PipelineError::refused(import_diag(
                "component-name-collision",
                c.name.clone(),
                format!(
                    "component `{n}` is declared in both `{other}` and `{path}` — across an import graph every component name must be unique, because both would emit a `mount_{n}`",
                    n = n,
                    other = other.display(),
                    path = path.display(),
                ),
                c.span,
                file,
                Some(format!(
                    "Rename one of the two `component {n}` declarations (and the instantiations that use it)."
                )),
            )));
        }
        seen.push((c.name.clone(), path.clone()));
    }

    // Shape + capability checks over the entry, with exactly the
    // imported component shapes known: an entry may instantiate
    // what it imports by name — not everything reachable through
    // transitive module imports (those serve the imported
    // components' own views, checked with their file's own shape).
    let imports_dir = file.parent().map(|d| d.to_path_buf());
    let externals: Vec<cancho_ui_types::ExternalComponent> = ast
        .imports
        .iter()
        .filter_map(|imp| {
            let module_file = match &imports_dir {
                Some(dir) => dir.join(format!("{}.cui", imp.module)),
                None => std::path::PathBuf::from(format!("{}.cui", imp.module)),
            };
            let r = resolved.iter().find(|r| r.path == module_file)?;
            r.ast
                .components
                .iter()
                .find(|c| c.name == imp.component)
                .map(|c| cancho_ui_types::ExternalComponent {
                    name: c.name.clone(),
                    state: c
                        .state
                        .iter()
                        .map(|s| (s.name.clone(), s.ty.clone()))
                        .collect(),
                })
        })
        .collect();
    check_file_with_externals(&ast, &externals)
        .map_err(|e| PipelineError::refused(check_to_diag(&e, file)))?;

    let entry_ir = lower_file(&ast).map_err(|e| PipelineError::refused(lower_to_diag(&e, file)))?;
    let mut irs: Vec<cancho_ui_ir::ir::IrFile> = Vec::new();
    for r in &resolved {
        irs.push(
            lower_file(&r.ast).map_err(|e| PipelineError::refused(lower_to_diag(&e, &r.path)))?,
        );
    }
    irs.push(entry_ir);
    let merged = cancho_ui_ir::merge_ir_files(&irs);
    Ok((merged, file.to_path_buf()))
}

fn check_to_diag(e: &cancho_ui_types::CheckError, file: &Path) -> Diagnostic {
    match e {
        cancho_ui_types::CheckError::Shape(t) => type_to_diag(t, file),
        cancho_ui_types::CheckError::LambdaUnsupported { span } => {
            let (code, severity, tier, subject, message, span) = (
                "lambda-unsupported",
                Severity::Error,
                Tier::Static,
                Subject::Component { name: String::new() },
                "a lambda (`x => expr`) is only supported as the second argument of `filter(list, pred)` or `map(list, transform)`".to_string(),
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
                remediation: None,
                related: Vec::new(),
                notes: Vec::new(),
            }
        }
        cancho_ui_types::CheckError::ControlRefused {
            code,
            message,
            span,
        } => Diagnostic {
            code: (*code).into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Component {
                name: String::new(),
            },
            message: message.clone(),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: control_remediation(code),
                suggestion: None,
                confidence: Confidence::High,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::ConditionRefused {
            code,
            message,
            span,
        } => Diagnostic {
            code: (*code).into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Component {
                name: String::new(),
            },
            message: message.clone(),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: condition_remediation(code),
                suggestion: None,
                confidence: Confidence::High,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::PatternLiteralUnsupported { name, span } => Diagnostic {
            code: "pattern-literal-unsupported".into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Component {
                name: String::new(),
            },
            message: format!(
                "`{name}` as a match pattern is a binding that matches everything, not the literal"
            ),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: pattern_literal_remediation(),
                suggestion: None,
                confidence: Confidence::High,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::UnknownCall { name, span } => Diagnostic {
            code: "call-unknown".into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Component {
                name: String::new(),
            },
            message: format!("`{name}` is not a function of this language"),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: unknown_call_remediation(),
                suggestion: None,
                confidence: Confidence::High,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::BuiltinMisuse { name, detail, span } => {
            let (code, severity, tier, subject, message, span) = (
                "builtin-shape-mismatch",
                Severity::Error,
                Tier::Static,
                Subject::Component {
                    name: String::new(),
                },
                format!("`{}` is misused here (expected {})", name, detail),
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
                remediation: None,
                related: Vec::new(),
                notes: Vec::new(),
            }
        }
        cancho_ui_types::CheckError::Schedule {
            code,
            component,
            message,
            span,
        } => Diagnostic {
            code: (*code).into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Component {
                name: component.clone(),
            },
            message: message.clone(),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(schedule_remediation(code)),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::PureLambdaRequired { span } => {
            let (code, severity, tier, subject, message, span) = (
                "pure-lambda-required",
                Severity::Error,
                Tier::Static,
                Subject::Component { name: String::new() },
                "the lambda body must be pure: it may not `await(...)` or call effectful/suspending builtins".to_string(),
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
                remediation: None,
                related: Vec::new(),
                notes: Vec::new(),
            }
        }
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
                Subject::Capability {
                    name: capability.clone(),
                },
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
        cancho_ui_types::CheckError::NarrowingIllegal {
            component,
            capability,
            reason,
            span,
        } => Diagnostic {
            code: "capability-narrowing-illegal".into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Capability {
                name: capability.clone(),
            },
            message: format!(
                "component `{component}`: `requires {capability}(..)` is not a legal bound: {reason}"
            ),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: "Write a bound as the capability allows: `Network(\"host\"..)`, `Storage(\"ns\")`, `Location(\"coarse\" or \"fine\")` or `Credential(\"name\", \"host\"..)` (docs/adapters.md).".to_string(),
                suggestion: None,
                confidence: Confidence::Medium,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::CredentialMisuse {
            rule,
            component,
            detail,
            span,
        } => Diagnostic {
            code: (*rule).into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Capability {
                name: "Credential".to_string(),
            },
            message: format!("component `{component}`: {detail}"),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(credential_remediation(rule)),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::BoundViolated {
            component,
            host,
            url,
            span,
        } => Diagnostic {
            code: "capability-bound-violated".into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Capability {
                name: "Network".to_string(),
            },
            message: format!(
                "component `{component}` fetches `{url}`, whose host `{host}` its `Network` does not allow"
            ),
            span: Some(SpanOut::from_span(*span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: format!(
                    "If this component should reach `{host}`, declare `requires Network(\"{host}\");` (that widens its authority: review it); otherwise fetch a path on this site."
                ),
                suggestion: None,
                confidence: Confidence::Medium,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        },
        cancho_ui_types::CheckError::CapabilityUnused {
            component,
            capability,
            span,
        } => {
            let (code, severity, tier, subject, message, span) = (
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
        cancho_ui_types::CheckError::MatchNonExhaustive {
            component,
            field,
            missing,
            span,
        } => {
            let (code, severity, tier, subject, message, span) = (
                "match-non-exhaustive",
                Severity::Error,
                Tier::Static,
                Subject::Component {
                    name: component.clone(),
                },
                format!(
                    "`match` on `{}` in component `{}` is not exhaustive; missing arm(s): {}",
                    field,
                    component,
                    missing.join(", ")
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
                        "Add arm(s): {}. A `Cancelled` is required when the value comes from `await(...)` unless a `_` catch-all covers it.",
                        missing.join(", ")
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

pub fn compile_to_js(
    file: &Path,
    target: Option<&str>,
    options: &EmitOptions,
) -> Result<String, PipelineError> {
    if let Some(t) = target
        && t != "web"
    {
        return Err(PipelineError::Env(format!(
            "unsupported target `{}`; only `web` is supported in Phase 1",
            t
        )));
    }
    let ir = compile_to_ir(file)?;
    // docs/adapters.md section 18: a module that mounts itself has nowhere to receive a credential.
    if options.auto_mount
        && let Some((comp, req)) = ir.components.iter().find_map(|c| {
            c.requires
                .iter()
                .find(|r| r.name == "Credential")
                .map(|r| (c, r))
        })
    {
        let name = req.bounds.first().map(String::as_str).unwrap_or("");
        return Err(PipelineError::Refused(Box::new(Diagnostic {
            code: "credential-needs-mount-options".into(),
            severity: Severity::Error,
            tier: Tier::Static,
            subject: Subject::Capability {
                name: "Credential".to_string(),
            },
            message: format!(
                "component `{}` declares `Credential(\"{name}\")`, but a module that mounts itself is given no credential, so every request it makes would be refused",
                comp.name
            ),
            span: Some(SpanOut::from_span(req.span, file)),
            expected: None,
            actual: None,
            remediation: Some(Remediation {
                description: format!(
                    "Build with `--no-auto-mount` and mount the module from your page, giving it the token: `mount_{}(root, {{ credentials: {{ \"{name}\": token }} }})`. The installed web app (`--pwa`) mounts itself, so it cannot carry a credential.",
                    ir.components
                        .last()
                        .map_or(comp.name.as_str(), |l| l.name.as_str())
                ),
                suggestion: None,
                confidence: Confidence::High,
            }),
            related: Vec::new(),
            notes: Vec::new(),
        })));
    }
    Ok(emit_module_with(
        &ir,
        Some(file.to_string_lossy().as_ref()),
        options,
    ))
}

pub fn format_diagnostic(d: &Diagnostic, file: &Path) -> String {
    let loc = match &d.span {
        Some(s) => format!("{}:{}:{}", s.file, s.start.line, s.start.column),
        None => file.display().to_string(),
    };
    format!("error[{}] at {}: {}", d.code, loc, d.message)
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
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column + 1,
                },
            ),
        ),
        LexError::UnterminatedString { line, column } => (
            "lex-unterminated-string",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            "unterminated string literal".to_string(),
            Span::new(
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
            ),
        ),
        LexError::UnknownEscape { ch, line, column } => (
            "lex-unknown-escape",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("unknown escape sequence \\{}", ch),
            Span::new(
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column + 2,
                },
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
        ParseError::Unexpected {
            expected,
            found,
            line,
            column,
        } => (
            "parse-unexpected-token",
            Severity::Error,
            Tier::Static,
            Subject::Internal,
            format!("expected {}, found {:?}", expected, token_name(found)),
            Some(Span::new(
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column + 1,
                },
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
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
                cancho_ui_syntax::Position {
                    line: *line,
                    column: *column,
                },
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
        Token::Match => "`match`",
        Token::Await => "`await`",
        Token::If => "`if`",
        Token::Else => "`else`",
        Token::FatArrow => "`=>`",
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
            format!(
                "component `{}` is declared more than once in this file",
                name
            ),
            None,
        ),
        TypeError::ReservedComponentName { name, span } => (
            "name-reserved",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "`{}` is a reserved word and cannot be used as a component name",
                name
            ),
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
        TypeError::UnknownComponentKind { name, span } => (
            "component-kind-unknown",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "component kind `{}` is not declared in this file; add a `component {0} {{ ... }}` or use a built-in primitive",
                name
            ),
            Some(*span),
        ),
        TypeError::InstanceTakesBindings { name, span } => (
            "instance-opaque",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "component instance `{}` takes no bindings; it is opaque — its own state and handlers govern it (properties arrive in a later tier)",
                name
            ),
            Some(*span),
        ),
        TypeError::InstanceTakesChildren { name, span } => (
            "instance-opaque",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "component instance `{}` takes no children; its own view governs what renders",
                name
            ),
            Some(*span),
        ),
        TypeError::InstanceTakesAccessibility { name, span } => (
            "instance-opaque",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: name.clone() },
            format!(
                "component instance `{}` takes no role/label of its own; the inner components carry their own accessibility",
                name
            ),
            Some(*span),
        ),
        TypeError::PropertyUnknownKey {
            component,
            key,
            span,
        } => (
            "property-unknown-key",
            Severity::Error,
            Tier::Static,
            Subject::Component {
                name: component.clone(),
            },
            format!(
                "component instance `{component}` has no state field `{key}` to pass as a property",
                component = component,
                key = key
            ),
            Some(*span),
        ),
        TypeError::PropertyTypeMismatch {
            component,
            key,
            span,
        } => (
            "property-type-mismatch",
            Severity::Error,
            Tier::Static,
            Subject::Component {
                name: component.clone(),
            },
            format!(
                "property `{key}` of `{component}` does not match the state field's declared type (string property or int/float/bool literal constants only)",
                component = component,
                key = key
            ),
            Some(*span),
        ),
        TypeError::PropertySourceUnknown {
            component,
            key,
            span,
        } => (
            "property-source-unknown",
            Severity::Error,
            Tier::Static,
            Subject::Component {
                name: component.clone(),
            },
            format!(
                "`{key}` is not a state field of `{component}` (and not an enclosing `For` binding); a dynamic property value must be read from the parent's state",
                component = component,
                key = key
            ),
            Some(*span),
        ),
        TypeError::DuplicatePropKey { component, key, .. } => (
            "duplicate-property-key",
            Severity::Error,
            Tier::Static,
            Subject::Component {
                name: component.clone(),
            },
            format!(
                "property `{key}` of `{component}` is passed twice on this instance",
                component = component,
                key = key
            ),
            None,
        ),
        TypeError::ComponentCycle { chain } => (
            "component-cycle",
            Severity::Error,
            Tier::Static,
            Subject::Component {
                name: chain.first().cloned().unwrap_or_default(),
            },
            format!(
                "component instance chain is cyclic: {} — the emitted mount would recurse forever",
                chain.join(" → ")
            ),
            None,
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
        LowerError::NarrowingIllegal {
            component,
            capability,
            reason,
            span,
        } => (
            "capability-narrowing-illegal",
            Severity::Error,
            Tier::Static,
            Subject::Capability {
                name: capability.clone(),
            },
            format!(
                "component `{component}`: `requires {capability}(..)` is not a legal bound: {reason}"
            ),
            Some(*span),
            Some(Remediation {
                description: "Write a bound as the capability allows: `Network(\"host\"..)`, `Storage(\"ns\")`, `Location(\"coarse\" or \"fine\")` or `Credential(\"name\", \"host\"..)` (docs/adapters.md).".to_string(),
                suggestion: None,
                confidence: Confidence::Medium,
            }),
        ),
        LowerError::CredentialMisuse {
            rule,
            component,
            detail,
            span,
        } => (
            *rule,
            Severity::Error,
            Tier::Static,
            Subject::Capability {
                name: "Credential".to_string(),
            },
            format!("component `{component}`: {detail}"),
            Some(*span),
            Some(credential_remediation(rule)),
        ),
        LowerError::BoundViolated {
            component,
            host,
            url,
            span,
        } => (
            "capability-bound-violated",
            Severity::Error,
            Tier::Static,
            Subject::Capability {
                name: "Network".to_string(),
            },
            format!(
                "component `{component}` fetches `{url}`, whose host `{host}` its `Network` does not allow"
            ),
            Some(*span),
            Some(Remediation {
                description: format!(
                    "If this component should reach `{host}`, declare `requires Network(\"{host}\");` (that widens its authority: review it); otherwise fetch a path on this site."
                ),
                suggestion: None,
                confidence: Confidence::Medium,
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
        LowerError::MatchNonExhaustive {
            component,
            field,
            missing,
            span,
        } => (
            "match-non-exhaustive",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: component.clone() },
            format!(
                "`match` on `{}` in component `{}` is not exhaustive; missing arm(s): {}",
                field,
                component,
                missing.join(", ")
            ),
            Some(*span),
            Some(Remediation {
                description: format!(
                    "Add arm(s): {}. A `Cancelled` is required when the value comes from `await(...)` unless a `_` catch-all covers it.",
                    missing.join(", ")
                ),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
        LowerError::LambdaUnsupported { span } => (
            "lambda-unsupported",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            "a lambda (`x => expr`) is only supported as the second argument of `filter(list, pred)` or `map(list, transform)`".to_string(),
            Some(*span),
            None,
        ),
        LowerError::ControlRefused {
            code,
            message,
            span,
        } => (
            *code,
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            message.clone(),
            Some(*span),
            Some(Remediation {
                description: control_remediation(code),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
        LowerError::ConditionRefused {
            code,
            message,
            span,
        } => (
            *code,
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            message.clone(),
            Some(*span),
            Some(Remediation {
                description: condition_remediation(code),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
        LowerError::PatternLiteralUnsupported { name, span } => (
            "pattern-literal-unsupported",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            format!("`{name}` as a match pattern is a binding that matches everything, not the literal"),
            Some(*span),
            Some(Remediation {
                description: pattern_literal_remediation(),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
        LowerError::UnknownCall { name, span } => (
            "call-unknown",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            format!("`{name}` is not a function of this language"),
            Some(*span),
            Some(Remediation {
                description: unknown_call_remediation(),
                suggestion: None,
                confidence: Confidence::High,
            }),
        ),
        LowerError::BuiltinMisuse { name, detail, span } => (
            "builtin-shape-mismatch",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            format!("`{}` is misused here (expected {})", name, detail),
            Some(*span),
            None,
        ),
        e @ (LowerError::ScheduleEventUnknown { .. }
        | LowerError::ScheduleEventParams { .. }
        | LowerError::SchedulePeriodInvalid { .. }
        | LowerError::ScheduleDuplicate { .. }) => {
            let (code, component, span) = e.schedule_refusal().expect("a schedule refusal");
            (
                code,
                Severity::Error,
                Tier::Static,
                Subject::Component {
                    name: component.to_string(),
                },
                e.to_string(),
                Some(span),
                Some(schedule_remediation(code)),
            )
        }
        LowerError::PureLambdaRequired { span } => (
            "pure-lambda-required",
            Severity::Error,
            Tier::Static,
            Subject::Component { name: String::new() },
            "the lambda body must be pure: it may not `await(...)` or call effectful/suspending builtins".to_string(),
            Some(*span),
            None,
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

/// What to do about a refused schedule (docs/schedules.md section 3).
fn schedule_remediation(code: &str) -> Remediation {
    let description = match code {
        "schedule-event-unknown" => {
            "Name an event the component handles with `on Name { ... }`, or add the handler."
        }
        "schedule-event-params" => {
            "Schedule an event that takes no parameters; a scheduled run has none to give."
        }
        "schedule-period-invalid" => {
            "Write the period as an integer literal of milliseconds from 100 to 86400000."
        }
        _ => "Declare each of `start Event;` and `every <ms> Event;` once per event.",
    };
    Remediation {
        description: description.to_string(),
        suggestion: None,
        confidence: Confidence::High,
    }
}

fn unknown_call_remediation() -> String {
    "The language has a fixed set of functions: `filter`, `map`, `sort`, `len`, `reverse`, `contains`, `lower`, `num`, `take`, `skip`, `min`, `max`, `abs`, `floor`, `ceil`, `round`, `lines`, `split`, `join`, `trim`, `upper`, `starts_with`, `ends_with`, `json.get`, `json.items`, `storage.get`, `storage.set`, `clock.now`, `sleep`, `net.get`, `net.fetch`, `net.post`, `net.search`, `location.current`. There is no function of your own; write the arithmetic with `+`, `-`, `*`, `/` and the list builtins (`docs/ops-console.md`).".to_string()
}

fn credential_remediation(rule: &str) -> Remediation {
    let description = match rule {
        "credential-ambiguous" => {
            "Declare one `Credential` per component (`requires Credential(\"api\");`); a request cannot choose between two. Split the component if two services need two credentials."
        }
        _ => {
            "A credential is sent only to this site's own origin and to the hosts named after its name, and each of those must also be allowed by `requires Network(\"host\");`. Add that `Network` host (review the widening) or drop the host from the `Credential`."
        }
    };
    Remediation {
        description: description.to_string(),
        suggestion: None,
        confidence: Confidence::High,
    }
}

fn pattern_literal_remediation() -> String {
    "There are no literal patterns: a bare name in a `match` arm binds the value and matches everything, so `true => ..` would always win. Branch on a Bool with `if cond { a } else { b }` (docs/conditionals.md), or use `_` for the arm that means \"anything else\".".to_string()
}

/// What to change for a refusal about `if`, `&&`, `||` or `!` (docs/conditionals.md).
fn condition_remediation(code: &str) -> String {
    match code {
        "if-else-required" => "Write both sides: `if cond { a } else { b }`; `else if cond2 { c }` chains.".to_string(),
        "if-effect-in-expression" => "Keep the condition and the branches pure: compute with state, literals and the pure builtins. To issue a request only sometimes, write two events and let the view pick the button.".to_string(),
        "condition-not-bool" => "Use a Bool: a comparison (`n > 0`), a Bool state field, or `&&`, `||`, `!` of those. A number or a string is not a condition: write `n != 0` or `len(s) > 0`.".to_string(),
        "if-branch-type-mismatch" => "Both branches of an `if` must have the same type: two numbers, two strings, two lists. Convert one side or restructure.".to_string(),
        _ => "See docs/conditionals.md.".to_string(),
    }
}

/// What to change for a refusal about the bindings of a built-in element (docs/controls.md).
fn control_remediation(code: &str) -> String {
    match code {
        "binding-unknown" => "Use one of the keys of a built-in element: label, role, on, value, disabled, placeholder, submit. `disabled:` is for Button and TextField, `submit:` for TextField.".to_string(),
        "submit-event-invalid" => "Name an event of this component that takes no parameters: `submit: Apply` with `on Apply { ... }`.".to_string(),
        "disabled-not-bool" => "Write `disabled: true`, `disabled: false` or `disabled: sending` where `sending` is a `state sending: Bool`.".to_string(),
        _ => "See docs/controls.md.".to_string(),
    }
}

fn capability_to_example_call(capability: &str) -> &'static str {
    match capability {
        "Clock" => "clock.now()",
        "Network" => "net.search(query)",
        "NetworkWrite" => "net.post(url, body)",
        _ => "an effect requiring this capability",
    }
}

#[allow(dead_code)]
pub fn emit_ir_json(file: &Path) -> Result<String, PipelineError> {
    let ir = compile_to_ir(file)?;
    Ok(canonical_json(&ir))
}
