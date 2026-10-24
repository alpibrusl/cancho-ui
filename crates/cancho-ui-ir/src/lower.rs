//! AST → IR lowering + capability checking for the Tier 4 supported
//! subset.
//!
//! Lowering is structural: every AST node has a one-to-one IR
//! equivalent. `lower_file` produces an IR then runs the
//! capability pass against it (every reachable effect's
//! capability must be in the component's `requires` list, and
//! every declared capability must have at least one reachable
//! use). Errors from either pass come back through the same
//! `LowerError` enum.

use cancho_ui_syntax::ast::{
    BinOp, Binding, BindingValue, CapabilityRef, Component, ElementKind, EventHandler, Expr, File,
    MatchArm, Pattern, Role, ScheduleKind, StateField, Stmt, TextPart, TextValue, Type, UnOp,
    ViewNode,
};
use cancho_ui_syntax::span::{Position, Span};
use std::collections::BTreeSet;
use thiserror::Error;

use crate::ir::*;
use crate::serialize::schema_version;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum LowerError {
    #[error("internal: AST node has empty span")]
    EmptySpan { span: Span },

    #[error(
        "component `{component}` performs `{call_path}` which requires the `{capability}` capability, but `requires {capability}` is not declared; add `requires {capability};` to the component header"
    )]
    CapabilityMissing {
        component: String,
        capability: String,
        call_path: String,
        span: Span,
    },

    #[error(
        "component `{component}` declares `requires {capability};` but no reachable effect uses it"
    )]
    CapabilityUnused {
        component: String,
        capability: String,
        span: Span,
    },

    #[error(
        "component `{component}`: schedule `{schedule}` names the event `{event}`, which it does not handle"
    )]
    ScheduleEventUnknown {
        component: String,
        schedule: String,
        event: String,
        span: Span,
    },

    #[error(
        "component `{component}`: schedule `{schedule}` runs `{event}`, which takes parameters; a scheduled run has none to give"
    )]
    ScheduleEventParams {
        component: String,
        schedule: String,
        event: String,
        span: Span,
    },

    #[error(
        "component `{component}`: `every {period}` is not a period; write an integer from 100 to 86400000 (milliseconds)"
    )]
    SchedulePeriodInvalid {
        component: String,
        period: i64,
        span: Span,
    },

    #[error("component `{component}`: `{schedule}` is declared twice for `{event}`")]
    ScheduleDuplicate {
        component: String,
        schedule: String,
        event: String,
        span: Span,
    },

    #[error("component `{component}`: `requires {capability}(..)` is not a legal bound: {reason}")]
    NarrowingIllegal {
        component: String,
        capability: String,
        reason: String,
        span: Span,
    },

    /// A misuse of `requires Credential(..)` that is not a malformed bound (docs/adapters.md section 18): `rule` is
    /// `credential-ambiguous` or `credential-host-not-allowed`.
    #[error("component `{component}`: {detail}")]
    CredentialMisuse {
        rule: &'static str,
        component: String,
        detail: String,
        span: Span,
    },

    #[error(
        "component `{component}` fetches `{url}`, whose host `{host}` its `Network` does not allow; add `requires Network(\"{host}\");`"
    )]
    BoundViolated {
        component: String,
        host: String,
        url: String,
        span: Span,
    },

    #[error(
        "`match` on `{field}` in component `{component}` is not exhaustive; missing arm(s): {missing:?}"
    )]
    MatchNonExhaustive {
        component: String,
        field: String,
        missing: Vec<String>,
        span: Span,
    },

    #[error(
        "a lambda (`x => expr`) is only supported as the second argument of `filter(list, pred)` or `map(list, transform)`; remove the lambda or move it into a `filter`/`map` call"
    )]
    LambdaUnsupported { span: Span },

    /// `true` / `false` as a match arm pattern (docs/semantics.md, `pattern-literal-unsupported`): a pattern that is a
    /// bare name binds it and matches everything, so the arm that looks like a literal would always match.
    #[error(
        "`{name}` as a match pattern is a binding that matches everything, not the literal; write `_` for \"anything else\" or branch on a state flag"
    )]
    PatternLiteralUnsupported { name: String, span: Span },

    #[error(
        "`{name}` is not a function of this language; it would be emitted as a JavaScript syntax error"
    )]
    UnknownCall { name: String, span: Span },

    /// A refusal about `if`, `&&`, `||` or `!` (docs/conditionals.md): `code` is the rule tag (`if-else-required`,
    /// `if-effect-in-expression`, `condition-not-bool`, `if-branch-type-mismatch`).
    #[error("{message}")]
    ConditionRefused {
        code: &'static str,
        message: String,
        span: Span,
    },

    /// A refusal about the bindings of a built-in element (docs/controls.md): `code` is the rule tag
    /// (`binding-unknown`, `disabled-not-bool`).
    #[error("{message}")]
    ControlRefused {
        code: &'static str,
        message: String,
        span: Span,
    },

    /// A refusal about `block` and `run` (docs/handler-sequences.md): `code` is the rule tag (`block-unknown`,
    /// `block-duplicate`, `block-name-conflict`, `block-cycle`, `block-expansion-too-large`).
    #[error("{message}")]
    BlockRefused {
        code: &'static str,
        message: String,
        span: Span,
    },

    #[error(
        "`{name}` is misused here (expected {detail}); see the builtin call forms: `filter(list, x => cond)`, `map(list, x => body)`, `sort(list, x => key)`, `reverse(list)`, `contains(text, needle)`, `lower(text)`, `fold(text)`, `num(text, default)`, `take(list, n)`, `skip(list, n)`, `concat(a, b)`, `lines(text)`, `split(text, sep)`, `join(list, sep)`, `trim(text)`, `upper(text)`, `starts_with(text, prefix)`, `ends_with(text, suffix)`, `len(value)`, `storage.get(key, default)`, `storage.set(key, value)`, `net.get(url)`, `net.fetch(url)`, `net.post(url, body)`, `net.put(url, body)`, `net.delete(url)`, `json.get(text, path, default)`, `json.items(text, path)`, `json.quote(text)`"
    )]
    BuiltinMisuse {
        name: String,
        detail: String,
        span: Span,
    },

    #[error(
        "the lambda body must be pure: it may not `await(...)` or call effectful/suspending builtins — list operations run synchronously inside the render pass"
    )]
    PureLambdaRequired { span: Span },
}

pub fn lower_file(file: &File) -> Result<IrFile, LowerError> {
    let mut next_id = 0u32;
    let mut components = Vec::with_capacity(file.components.len());
    for c in &file.components {
        components.push(lower_component(c, &mut next_id)?);
    }
    if let Some(second) = file.themes.get(1) {
        return Err(LowerError::ControlRefused {
            code: "theme-duplicate",
            message: "a program has one `theme { .. }`; this is a second one in the same file"
                .to_string(),
            span: second.span,
        });
    }
    let theme = match file.themes.first() {
        Some(t) => Some(crate::theme::lower_theme(t)?),
        None => None,
    };
    let ir = IrFile {
        schema: SCHEMA.to_string(),
        version: schema_version().to_string(),
        module: file.module.as_ref().map(|m| m.name.clone()),
        components,
        theme,
    };
    crate::controls::check_controls(&ir)?;
    crate::conditions::check_conditions(&ir)?;
    check_capabilities(&ir)?;
    check_match_exhaustiveness(&ir)?;
    Ok(ir)
}

/// Tier 16: merge the dependency-ordered per-file IRs into one
/// module. Imported files come first and the entry last, so the
/// merged components' final entry — what the emitter auto-mounts
/// — is the entry file's last component, exactly what the file's
/// standalone build mounts today.
pub fn merge_ir_files(files: &[IrFile]) -> IrFile {
    let Some(last) = files.last() else {
        // Nothing to merge is an empty module (the callers always pass at least the entry file).
        return IrFile {
            schema: SCHEMA.to_string(),
            version: schema_version().to_string(),
            module: None,
            components: Vec::new(),
            theme: None,
        };
    };
    IrFile {
        schema: last.schema.clone(),
        version: last.version.clone(),
        module: last.module.clone(),
        components: files.iter().flat_map(|f| f.components.clone()).collect(),
        // one theme per program (the pipeline refuses two): the last file's, which is the entry's when it has one
        theme: files.iter().rev().find_map(|f| f.theme.clone()),
    }
}

fn lower_component(c: &Component, next_id: &mut u32) -> Result<IrComponent, LowerError> {
    if c.span == Span::DUMMY {
        return Err(LowerError::EmptySpan { span: c.span });
    }
    let id = IrId(*next_id);
    *next_id += 1;
    let mut state = Vec::with_capacity(c.state.len());
    for s in &c.state {
        state.push(lower_state(s)?);
    }
    let requires = c.requires.iter().map(lower_capability).collect();
    let schedules = lower_schedules(c)?;
    let messages = crate::messages::lower_messages(c)?;
    let _messages_guard = crate::messages::enter(&messages);
    let mut events = Vec::with_capacity(c.events.len());
    let mut reachable_effects: Vec<IrReachableEffect> = Vec::new();
    // `run Name;` is replaced by the block's statements here, before anything else sees the handler (docs/handler-sequences.md).
    let expanded = crate::blocks::expand(c)?;
    for (e, body) in c.events.iter().zip(&expanded.handlers) {
        let (ir_event, effects) = lower_event(e, body)?;
        events.push(ir_event);
        reachable_effects.extend(effects);
    }
    // A block is checked on its own: a name in it is a state field, never a parameter of the handler that runs it.
    for body in &expanded.blocks {
        let (ir_body, _) = lower_stmts_with_effects(body)?;
        crate::conditions::check_block(&state, &ir_body)?;
    }
    reachable_effects.extend(crate::routing::route_effects(&schedules));
    reachable_effects.extend(crate::stream::stream_effects(&schedules));
    let (view_body, view_effects) = lower_view_node_with_effects(&c.view.body)?;
    reachable_effects.extend(view_effects);
    let view = IrViewBlock {
        body: view_body,
        span: c.view.span,
    };
    Ok(IrComponent {
        id,
        name: c.name.clone(),
        state,
        requires,
        schedules,
        reachable_effects,
        events,
        messages,
        view,
        span: c.span,
    })
}

/// The shortest and longest period of `every` (docs/schedules.md section 3), in milliseconds.
pub const MIN_PERIOD_MS: i64 = 100;
pub const MAX_PERIOD_MS: i64 = 86_400_000;

/// Check the component's schedules against its events and lower them.
fn lower_schedules(c: &Component) -> Result<Vec<IrSchedule>, LowerError> {
    let mut out: Vec<IrSchedule> = Vec::with_capacity(c.schedules.len());
    for s in &c.schedules {
        if s.kind == ScheduleKind::Route {
            let route = crate::routing::lower_route(c, s, &out)?;
            out.push(route);
            continue;
        }
        if s.kind == ScheduleKind::Subscribe {
            let stream = crate::stream::lower_subscribe(c, s, &out)?;
            out.push(stream);
            continue;
        }
        let text = match s.kind {
            ScheduleKind::Start => format!("start {}", s.event),
            ScheduleKind::Every(ms) => format!("every {ms} {}", s.event),
            ScheduleKind::Route | ScheduleKind::Subscribe => String::new(),
        };
        let Some(handler) = c.events.iter().find(|e| e.name == s.event) else {
            return Err(LowerError::ScheduleEventUnknown {
                component: c.name.clone(),
                schedule: text,
                event: s.event.clone(),
                span: s.span,
            });
        };
        if !handler.params.is_empty() {
            return Err(LowerError::ScheduleEventParams {
                component: c.name.clone(),
                schedule: text,
                event: s.event.clone(),
                span: s.span,
            });
        }
        let kind = match s.kind {
            ScheduleKind::Start | ScheduleKind::Route | ScheduleKind::Subscribe => {
                IrScheduleKind::Start
            }
            ScheduleKind::Every(ms) => {
                if !(MIN_PERIOD_MS..=MAX_PERIOD_MS).contains(&ms) {
                    return Err(LowerError::SchedulePeriodInvalid {
                        component: c.name.clone(),
                        period: ms,
                        span: s.span,
                    });
                }
                IrScheduleKind::Every(ms as u32)
            }
        };
        let same_kind = |k: &IrScheduleKind| {
            matches!(
                (k, &kind),
                (IrScheduleKind::Start, IrScheduleKind::Start)
                    | (IrScheduleKind::Every(_), IrScheduleKind::Every(_))
            )
        };
        if out.iter().any(|o| o.event == s.event && same_kind(&o.kind)) {
            return Err(LowerError::ScheduleDuplicate {
                component: c.name.clone(),
                schedule: text,
                event: s.event.clone(),
                span: s.span,
            });
        }
        out.push(IrSchedule {
            kind,
            event: s.event.clone(),
            stream: None,
            span: s.span,
        });
    }
    Ok(out)
}

fn lower_state(s: &StateField) -> Result<IrStateField, LowerError> {
    Ok(IrStateField {
        name: s.name.clone(),
        ty: lower_type(&s.ty),
        initial: lower_expr(&s.initial)?,
        span: s.span,
    })
}

fn lower_type(t: &Type) -> IrType {
    match t {
        Type::Int => IrType::Int,
        Type::Float => IrType::Float,
        Type::Bool => IrType::Bool,
        Type::String => IrType::String,
        Type::Result { ok, err } => IrType::Result {
            ok: Box::new(lower_type(ok)),
            err: Box::new(lower_type(err)),
        },
        Type::Request { payload } => IrType::Request {
            payload: Box::new(lower_type(payload)),
        },
        Type::Option { some } => IrType::Option {
            some: Box::new(lower_type(some)),
        },
        Type::List { elem } => IrType::List {
            elem: Box::new(lower_type(elem)),
        },
    }
}

fn lower_capability(c: &CapabilityRef) -> IrCapabilityRef {
    IrCapabilityRef {
        name: c.name.clone(),
        bounds: c.bounds.clone(),
        span: c.span,
    }
}

fn lower_event(
    e: &EventHandler,
    stmts: &[Stmt],
) -> Result<(IrEventHandler, Vec<IrReachableEffect>), LowerError> {
    let (body, effects) = lower_stmts_with_effects(stmts)?;
    Ok((
        IrEventHandler {
            name: e.name.clone(),
            params: e.params.clone(),
            body,
            span: e.span,
        },
        effects,
    ))
}

fn lower_stmts_with_effects(
    stmts: &[Stmt],
) -> Result<(Vec<IrStmt>, Vec<IrReachableEffect>), LowerError> {
    let mut out = Vec::with_capacity(stmts.len());
    let mut effects = Vec::new();
    for s in stmts {
        let (ir, mut e) = lower_stmt(s)?;
        out.push(ir);
        effects.append(&mut e);
    }
    Ok((out, effects))
}

fn lower_stmt(s: &Stmt) -> Result<(IrStmt, Vec<IrReachableEffect>), LowerError> {
    match s {
        Stmt::Assign {
            target,
            value,
            span,
        } => {
            let (v, effects) = lower_expr_with_effects(value)?;
            Ok((
                IrStmt::Assign {
                    target: target.clone(),
                    value: v,
                    span: *span,
                },
                effects,
            ))
        }
        Stmt::Expr { expr, span } => {
            let (e, effects) = lower_expr_with_effects(expr)?;
            Ok((
                IrStmt::Expr {
                    expr: e,
                    span: *span,
                },
                effects,
            ))
        }
        Stmt::If {
            cond,
            then,
            else_,
            span,
        } => {
            let (ir_cond, cond_effects) = lower_expr_with_effects(cond)?;
            if !cond_effects.is_empty() || ir_expr_suspends(&ir_cond) {
                return Err(LowerError::ConditionRefused {
                    code: "if-effect-in-expression",
                    message: "the condition of an `if` is pure: it may not call an effect (`net.*`, `storage.*`, `clock.*`, `location.*`), `sleep` or `await(..)`; read the answer into a state field first, then branch on the field".into(),
                    span: cond.span(),
                });
            }
            let (ir_then, mut effects) = lower_stmts_with_effects(then)?;
            let (ir_else, mut e) = lower_stmts_with_effects(else_)?;
            effects.append(&mut e);
            Ok((
                IrStmt::If {
                    cond: ir_cond,
                    then: ir_then,
                    else_: ir_else,
                    span: *span,
                },
                effects,
            ))
        }
        // Every `run` was replaced before lowering (`blocks::expand`); one that is left names no block.
        Stmt::Run { name, span } => Err(LowerError::BlockRefused {
            code: "block-unknown",
            message: format!("`run {name};` names a block this component does not declare"),
            span: *span,
        }),
    }
}

fn lower_expr(e: &Expr) -> Result<IrExpr, LowerError> {
    Ok(lower_expr_with_effects(e)?.0)
}

fn lower_expr_with_effects(e: &Expr) -> Result<(IrExpr, Vec<IrReachableEffect>), LowerError> {
    match e {
        Expr::IntLit { value, span } => Ok((
            IrExpr::IntLit {
                value: *value,
                span: *span,
            },
            vec![],
        )),
        Expr::FloatLit { value, span } => Ok((
            IrExpr::FloatLit {
                value: *value,
                span: *span,
            },
            vec![],
        )),
        Expr::StringLit { value, span } => Ok((
            IrExpr::StringLit {
                value: value.clone(),
                span: *span,
            },
            vec![],
        )),
        Expr::BoolLit { value, span } => Ok((
            IrExpr::BoolLit {
                value: *value,
                span: *span,
            },
            vec![],
        )),
        Expr::Ident { name, span } => Ok((
            IrExpr::Ident {
                name: name.clone(),
                span: *span,
            },
            vec![],
        )),
        Expr::Lambda { span, .. } => {
            // `filter`/`map` consume their lambda before the
            // generic call path sees it; a lambda anywhere else
            // has no meaning (Tier 11 has no first-class
            // function values).
            Err(LowerError::LambdaUnsupported { span: *span })
        }
        Expr::Call { path, args, span } => {
            // Tier 11 call forms: `filter(list, x => cond)`,
            // `map(list, x => body)`, `len(value)`. They are pure
            // (no capability); the generic call path below does
            // not see them.
            if path.len() == 2 && path[0] == "location" && path[1] == "current" && !args.is_empty()
            {
                return Err(LowerError::BuiltinMisuse {
                    name: "location.current".to_string(),
                    detail: "no arguments: location.current()".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "net" && path[1] == "fetch" && args.len() != 1 {
                // docs/adapters.md section 15: one argument, the URL.
                return Err(LowerError::BuiltinMisuse {
                    name: "net.fetch".to_string(),
                    detail: "exactly one argument: net.fetch(url)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "net" && path[1] == "post" && args.len() != 2 {
                // docs/adapters.md section 17: the URL and the body.
                return Err(LowerError::BuiltinMisuse {
                    name: "net.post".to_string(),
                    detail: "exactly two arguments: net.post(url, body)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "net" && path[1] == "put" && args.len() != 2 {
                // docs/adapters.md section 21: the URL and the body.
                return Err(LowerError::BuiltinMisuse {
                    name: "net.put".to_string(),
                    detail: "exactly two arguments: net.put(url, body)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "net" && path[1] == "delete" && args.len() != 1 {
                // docs/adapters.md section 21: the URL only.
                return Err(LowerError::BuiltinMisuse {
                    name: "net.delete".to_string(),
                    detail: "exactly one argument: net.delete(url)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "net" && path[1] == "get" && args.len() != 1 {
                // docs/adapters.md section 9: one argument, the URL.
                return Err(LowerError::BuiltinMisuse {
                    name: "net.get".to_string(),
                    detail: "exactly one argument: net.get(url)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "json" && path[1] == "items" && args.len() != 2 {
                return Err(LowerError::BuiltinMisuse {
                    name: "json.items".to_string(),
                    detail: "exactly two arguments: json.items(text, path)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "json" && path[1] == "quote" && args.len() != 1 {
                // docs/language-design.md decision 15: the text to write as a JSON string.
                return Err(LowerError::BuiltinMisuse {
                    name: "json.quote".to_string(),
                    detail: "exactly one argument: json.quote(text)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "json" && path[1] == "get" && args.len() != 3 {
                // docs/language-design.md decision 13: text, path and default, always.
                return Err(LowerError::BuiltinMisuse {
                    name: "json.get".to_string(),
                    detail: "exactly three arguments: json.get(text, path, default)".to_string(),
                    span: *span,
                });
            }
            if path.len() == 2 && path[0] == "storage" && (path[1] == "get" || path[1] == "set") {
                // docs/adapters.md: two arguments, always. A missing one would reach the browser as
                // `undefined`, so it is refused here.
                if args.len() != 2 {
                    let form = if path[1] == "get" {
                        "exactly two arguments: storage.get(key, default)"
                    } else {
                        "exactly two arguments: storage.set(key, value)"
                    };
                    return Err(LowerError::BuiltinMisuse {
                        name: format!("storage.{}", path[1]),
                        detail: form.to_string(),
                        span: *span,
                    });
                }
            }
            crate::credential::check_call(path, args.len(), *span)?;
            if crate::messages::is_call(path) {
                crate::messages::check_call(path, args, *span)?;
            }
            if path.len() == 2 && path[0] == "router" && (path[1] == "go" || path[1] == "replace") {
                crate::routing::check_router_call(&path[1], args.len(), *span)?;
            }
            if path.len() == 1 && path[0] == "filter" {
                if args.len() != 2 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "filter".to_string(),
                        detail: "exactly two arguments: filter(list, x => cond)".to_string(),
                        span: *span,
                    });
                }
                let list = Box::new(lower_expr_pure(&args[0])?);
                let (item, cond_body, cond_span) = match &args[1] {
                    Expr::Lambda { param, body, .. } => {
                        let lowered = lower_expr_pure(body)?;
                        (param.clone(), lowered, body.span())
                    }
                    _ => {
                        return Err(LowerError::BuiltinMisuse {
                            name: "filter".to_string(),
                            detail: "second argument must be a lambda: x => cond".to_string(),
                            span: args[1].span(),
                        });
                    }
                };
                return Ok((
                    IrExpr::Filter {
                        list,
                        item,
                        cond: Box::new(cond_body),
                        span: cond_span,
                    },
                    vec![],
                ));
            }
            if path.len() == 1 && path[0] == "map" {
                if args.len() != 2 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "map".to_string(),
                        detail: "exactly two arguments: map(list, x => body)".to_string(),
                        span: *span,
                    });
                }
                let list = Box::new(lower_expr_pure(&args[0])?);
                let (item, mapper_body, mapper_span) = match &args[1] {
                    Expr::Lambda { param, body, .. } => {
                        let lowered = lower_expr_pure(body)?;
                        (param.clone(), lowered, body.span())
                    }
                    _ => {
                        return Err(LowerError::BuiltinMisuse {
                            name: "map".to_string(),
                            detail: "second argument must be a lambda: x => body".to_string(),
                            span: args[1].span(),
                        });
                    }
                };
                return Ok((
                    IrExpr::Map {
                        list,
                        item,
                        body: Box::new(mapper_body),
                        span: mapper_span,
                    },
                    vec![],
                ));
            }
            if path.len() == 1 && path[0] == "sort" {
                if args.len() != 2 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "sort".to_string(),
                        detail: "exactly two arguments: sort(list, x => key)".to_string(),
                        span: *span,
                    });
                }
                let list = Box::new(lower_expr_pure(&args[0])?);
                let (item, key, key_span) = match &args[1] {
                    Expr::Lambda { param, body, .. } => {
                        (param.clone(), lower_expr_pure(body)?, body.span())
                    }
                    _ => {
                        return Err(LowerError::BuiltinMisuse {
                            name: "sort".to_string(),
                            detail: "second argument must be a lambda: x => key".to_string(),
                            span: args[1].span(),
                        });
                    }
                };
                return Ok((
                    IrExpr::SortBy {
                        list,
                        item,
                        key: Box::new(key),
                        span: key_span,
                    },
                    vec![],
                ));
            }
            for (name, arity, form) in PURE_BUILTINS {
                if path.len() == 1 && path[0] == *name && args.len() != *arity {
                    return Err(LowerError::BuiltinMisuse {
                        name: (*name).to_string(),
                        detail: (*form).to_string(),
                        span: *span,
                    });
                }
            }
            if path.len() == 1 && path[0] == "len" {
                if args.len() != 1 {
                    return Err(LowerError::BuiltinMisuse {
                        name: "len".to_string(),
                        detail: "exactly one argument: len(value)".to_string(),
                        span: *span,
                    });
                }
                return Ok((
                    IrExpr::Len {
                        list: Box::new(lower_expr_pure(&args[0])?),
                        span: *span,
                    },
                    vec![],
                ));
            }
            let mut lowered_args = Vec::with_capacity(args.len());
            let mut effects = Vec::new();
            for a in args {
                let (ir, e) = lower_expr_with_effects(a)?;
                lowered_args.push(ir);
                effects.extend(e);
            }
            // the arguments are lowered first, so that a lambda in a call that is not `filter`/`map`/`sort` stays
            // `lambda-unsupported`
            if !is_known_call(path) {
                return Err(LowerError::UnknownCall {
                    name: path.join("."),
                    span: *span,
                });
            }
            if let Some(capability) = builtin_capability(path) {
                effects.push(IrReachableEffect {
                    capability: capability.to_string(),
                    path: path.clone(),
                    literal_arg: match args.first() {
                        Some(Expr::StringLit { value, .. }) => Some(value.clone()),
                        _ => None,
                    },
                    span: *span,
                });
            }
            Ok((
                IrExpr::Call {
                    path: path.clone(),
                    args: lowered_args,
                    span: *span,
                },
                effects,
            ))
        }
        Expr::Binary { op, lhs, rhs, span } => {
            let (l, mut le) = lower_expr_with_effects(lhs)?;
            let (r, mut re) = lower_expr_with_effects(rhs)?;
            le.append(&mut re);
            Ok((
                IrExpr::Binary {
                    op: lower_binop(*op),
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                    span: *span,
                },
                le,
            ))
        }
        Expr::Unary { op, operand, span } => {
            let (o, e) = lower_expr_with_effects(operand)?;
            Ok((
                IrExpr::Unary {
                    op: lower_unop(*op),
                    operand: Box::new(o),
                    span: *span,
                },
                e,
            ))
        }
        Expr::If {
            cond,
            then,
            else_,
            span,
        } => {
            let Some(else_) = else_ else {
                return Err(LowerError::ConditionRefused {
                    code: "if-else-required",
                    message: "an `if` expression has a value on both sides: write `if cond { a } else { b }` (the statement form, `if cond { x = a; }` at the start of a statement, may omit `else`)".into(),
                    span: *span,
                });
            };
            let (ir_cond, mut effects) = lower_expr_with_effects(cond)?;
            let (ir_then, mut e) = lower_expr_with_effects(then)?;
            effects.append(&mut e);
            let (ir_else, mut e) = lower_expr_with_effects(else_)?;
            effects.append(&mut e);
            if !effects.is_empty()
                || ir_expr_suspends(&ir_cond)
                || ir_expr_suspends(&ir_then)
                || ir_expr_suspends(&ir_else)
            {
                return Err(LowerError::ConditionRefused {
                    code: "if-effect-in-expression",
                    message: "an `if` expression is pure: its condition and branches may not call an effect (`net.*`, `storage.*`, `clock.*`, `location.*`), `sleep` or `await(..)`; to issue a request only sometimes use the statement form at the start of a statement: `if cond { r = await(net.get(url)); }`".into(),
                    span: *span,
                });
            }
            Ok((
                IrExpr::If {
                    cond: Box::new(ir_cond),
                    then: Box::new(ir_then),
                    else_: Box::new(ir_else),
                    span: *span,
                },
                effects,
            ))
        }
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => {
            let (ir_scrutinee, mut effects) = lower_expr_with_effects(scrutinee)?;
            let mut ir_arms = Vec::with_capacity(arms.len());
            for MatchArm {
                pattern,
                body,
                span: arm_span,
            } in arms
            {
                if let Pattern::Ident { name, span } = pattern
                    && (name == "true" || name == "false")
                {
                    return Err(LowerError::PatternLiteralUnsupported {
                        name: name.clone(),
                        span: *span,
                    });
                }
                let ir_pattern = lower_pattern(pattern);
                let (ir_body, mut e) = lower_expr_with_effects(body)?;
                effects.append(&mut e);
                ir_arms.push(IrMatchArm {
                    pattern: ir_pattern,
                    body: ir_body,
                    span: *arm_span,
                });
            }
            Ok((
                IrExpr::Match {
                    scrutinee: Box::new(ir_scrutinee),
                    arms: ir_arms,
                    span: *span,
                },
                effects,
            ))
        }
        Expr::Ok { value, span } => {
            let (ir_value, e) = lower_expr_with_effects(value)?;
            Ok((
                IrExpr::Ok {
                    value: Box::new(ir_value),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Err { reason, span } => {
            let (ir_reason, e) = lower_expr_with_effects(reason)?;
            Ok((
                IrExpr::Err {
                    reason: Box::new(ir_reason),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Await { request, span } => {
            let (ir_request, e) = lower_expr_with_effects(request)?;
            Ok((
                IrExpr::Await {
                    request: Box::new(ir_request),
                    span: *span,
                },
                e,
            ))
        }
        Expr::Cancelled { span } => Ok((IrExpr::Cancelled { span: *span }, Vec::new())),
        Expr::Some { value, span } => {
            let (ir_value, e) = lower_expr_with_effects(value)?;
            Ok((
                IrExpr::Some {
                    value: Box::new(ir_value),
                    span: *span,
                },
                e,
            ))
        }
        Expr::None { span } => Ok((IrExpr::None { span: *span }, Vec::new())),
        Expr::ListLit { items, span } => {
            let mut ir_items = Vec::with_capacity(items.len());
            let mut effects = Vec::new();
            for item in items {
                let (ir, mut e) = lower_expr_with_effects(item)?;
                effects.append(&mut e);
                ir_items.push(ir);
            }
            Ok((
                IrExpr::ListLit {
                    items: ir_items,
                    span: *span,
                },
                effects,
            ))
        }
    }
}

fn lower_pattern(p: &Pattern) -> IrPattern {
    match p {
        Pattern::Wildcard { span } => IrPattern::Wildcard { span: *span },
        Pattern::Ident { name, span } => IrPattern::Ident {
            name: name.clone(),
            span: *span,
        },
        Pattern::Variant {
            name,
            binding,
            span,
        } => IrPattern::Variant {
            name: name.clone(),
            binding: binding.clone(),
            span: *span,
        },
        Pattern::UnitVariant { name, span } => IrPattern::UnitVariant {
            name: name.clone(),
            span: *span,
        },
    }
}

/// Phase 1 / Tier 4 hard-codes the capability for known builtin
/// calls. Phase 2 will replace this with a registered vocabulary
/// (`spec/security-model.md`) and a richer `capability-missing`
/// permission set.
fn builtin_capability(path: &[String]) -> Option<&'static str> {
    if path.len() == 2 && path[0] == "clock" && path[1] == "now" {
        Some("Clock")
    } else if path.len() == 2
        && path[0] == "net"
        && (path[1] == "search" || path[1] == "get" || path[1] == "fetch")
    {
        // Tier 9: `net.search(query)` is the asynchronous search
        // effect. `await(net.search(q))` resolves to Ok | Err |
        // Cancelled with an Ok payload of `List[String]`.
        Some("Network")
    } else if path.len() == 2
        && path[0] == "net"
        && (path[1] == "post" || path[1] == "put" || path[1] == "delete")
    {
        // docs/adapters.md sections 17 and 21: a write is a stronger authority than a read, so it is its own capability,
        // and `PUT` and `DELETE` are writes of the same kind as `POST`.
        Some("NetworkWrite")
    } else if path.len() == 2 && path[0] == "location" && path[1] == "current" {
        // docs/adapters.md section 11: `await(location.current())` resolves to Ok("lat,lon") | Err | Cancelled.
        Some("Location")
    } else if path.len() == 2 && path[0] == "storage" && (path[1] == "get" || path[1] == "set") {
        // docs/adapters.md: `storage.get(key, default)` and `storage.set(key, value)` read and
        // write a persistent key-value store, synchronously. Both need `Storage`.
        Some("Storage")
    } else if crate::messages::is_call(path) {
        // docs/i18n.md: the texts, formats and plurals depend on the language in use, which `Locale` is the authority to read and change.
        Some("Locale")
    } else if crate::credential::is_call(path) {
        // docs/bearer-token.md: the program holds a token it received, in the component's `Credential`.
        Some("Credential")
    } else if path.len() == 2 && path[0] == "router" && (path[1] == "go" || path[1] == "replace") {
        // docs/routing.md section 4: `router.go(path)` and `router.replace(path)` write the URL fragment, synchronously.
        Some("Router")
    } else if path.len() == 1 && path[0] == "sleep" {
        // Tier 7: `sleep(ms)` is a pure-runtime wait. It does
        // not need a user-declared capability; the runtime
        // scheduler is its capability.
        None
    } else {
        None
    }
}

fn lower_view_node_with_effects(
    node: &ViewNode,
) -> Result<(IrViewNode, Vec<IrReachableEffect>), LowerError> {
    match node {
        ViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span,
        } => {
            let mut lowered_children = Vec::with_capacity(children.len());
            let mut effects = Vec::new();
            for c in children {
                let (n, mut e) = lower_view_node_with_effects(c)?;
                lowered_children.push(n);
                effects.append(&mut e);
            }
            if matches!(kind, ElementKind::Table) {
                lowered_children = crate::table::table_rows(lowered_children)?;
            }
            let mut lowered_bindings = Vec::with_capacity(bindings.len());
            for b in bindings {
                lowered_bindings.push(lower_binding(b, Some(kind), &mut effects)?);
            }
            Ok((
                IrViewNode::Element {
                    kind: lower_kind(kind),
                    accessibility: lower_accessibility(accessibility, &mut effects)?,
                    bindings: lowered_bindings,
                    children: lowered_children,
                    span: *span,
                },
                effects,
            ))
        }
        ViewNode::Text { value, span } => {
            let mut effects = Vec::new();
            let parts = match value {
                TextValue::Interpolated(parts) => {
                    let mut lowered = Vec::with_capacity(parts.len());
                    for p in parts {
                        match p {
                            TextPart::Literal(s) => lowered.push(IrTextPart::Literal(s.clone())),
                            TextPart::Expr { expr, span } => {
                                let _ = span;
                                let (ir, mut e) = lower_expr_with_effects(expr)?;
                                effects.append(&mut e);
                                lowered.push(IrTextPart::Expr(ir))
                            }
                        }
                    }
                    lowered
                }
            };
            Ok((IrViewNode::Text { parts, span: *span }, effects))
        }
        ViewNode::For {
            over,
            binding,
            children,
            span,
        } => {
            // Tier 11: `over` is a general (pure) expression — a
            // state list, or a derived one via `filter(list, n =>
            // ...)`. Purity matters less here than in a predicate
            // (a suspending `over` cannot even render), so the
            // same rule applies: no effects, no suspends.
            let ir_over = lower_expr_pure(over)?;
            let mut lowered_children = Vec::with_capacity(children.len());
            let mut effects = Vec::new();
            for ch in children {
                let (n, mut e) = lower_view_node_with_effects(ch)?;
                lowered_children.push(n);
                effects.append(&mut e);
            }
            Ok((
                IrViewNode::For {
                    over: ir_over,
                    binding: binding.clone(),
                    children: lowered_children,
                    span: *span,
                },
                effects,
            ))
        }
        ViewNode::If {
            cond,
            children,
            span,
        } => {
            // The condition is evaluated at each render, so it is as pure as a `For`'s `over`.
            let ir_cond = lower_expr_pure(cond)?;
            let mut lowered_children = Vec::with_capacity(children.len());
            let mut effects = Vec::new();
            for ch in children {
                let (n, mut e) = lower_view_node_with_effects(ch)?;
                lowered_children.push(n);
                effects.append(&mut e);
            }
            Ok((
                IrViewNode::If {
                    cond: ir_cond,
                    children: lowered_children,
                    span: *span,
                },
                effects,
            ))
        }
    }
}

fn lower_kind(k: &ElementKind) -> IrElementKind {
    match k {
        ElementKind::Column => IrElementKind::Column,
        ElementKind::Row => IrElementKind::Row,
        ElementKind::Table => IrElementKind::Table,
        ElementKind::Text => IrElementKind::Text,
        ElementKind::Button => IrElementKind::Button,
        ElementKind::TextField => IrElementKind::TextField,
        ElementKind::TextArea => IrElementKind::TextArea,
        ElementKind::Checkbox => IrElementKind::Checkbox,
        ElementKind::Select => IrElementKind::Select,
        ElementKind::FileField => IrElementKind::FileField,
        ElementKind::Chart => IrElementKind::Chart,
        ElementKind::NavLink => IrElementKind::NavLink,
        ElementKind::SortList => IrElementKind::SortList,
        ElementKind::Map => IrElementKind::Map,
        ElementKind::Heading => IrElementKind::Heading,
        ElementKind::Section => IrElementKind::Section,
        ElementKind::Header => IrElementKind::Header,
        ElementKind::Nav => IrElementKind::Nav,
        ElementKind::Main => IrElementKind::Main,
        ElementKind::Footer => IrElementKind::Footer,
        ElementKind::Aside => IrElementKind::Aside,
        ElementKind::Article => IrElementKind::Article,
        ElementKind::Link => IrElementKind::Link,
        ElementKind::Details => IrElementKind::Details,
        ElementKind::Figure => IrElementKind::Figure,
        ElementKind::Caption => IrElementKind::Caption,
        ElementKind::Named(s) => IrElementKind::Named(s.clone()),
    }
}

fn lower_accessibility(
    acc: &cancho_ui_syntax::ast::Accessibility,
    effects: &mut Vec<IrReachableEffect>,
) -> Result<IrAccessibility, LowerError> {
    let label_expr = match &acc.label_expr {
        Some(e) => {
            let (ir, mut e) = lower_expr_with_effects(e)?;
            effects.append(&mut e);
            Some(Box::new(ir))
        }
        None => None,
    };
    Ok(IrAccessibility {
        role: acc.role.as_ref().map(lower_role),
        label: acc.label.clone(),
        label_expr,
    })
}

fn lower_role(r: &Role) -> IrRole {
    match r {
        Role::Text => IrRole::Text,
        Role::Heading => IrRole::Heading,
        Role::Button => IrRole::Button,
        Role::Other(s) => IrRole::Other(s.clone()),
    }
}

fn lower_binding(
    b: &Binding,
    kind: Option<&ElementKind>,
    effects: &mut Vec<IrReachableEffect>,
) -> Result<IrBinding, LowerError> {
    let value = match &b.value {
        BindingValue::String(s) => {
            // Tier 13: on a component instance, a string binding
            // value is a *string property* (`title: "Ada"`) — a
            // const initializer for the child's state field — not
            // an HTML attribute (the instance is opaque). On
            // primitives it stays the plain attribute value.
            match kind {
                Some(ElementKind::Named(_)) => IrBindingValue::Const {
                    expr: Box::new(IrExpr::StringLit {
                        value: s.clone(),
                        span: b.span,
                    }),
                    span: b.span,
                },
                _ => IrBindingValue::String(s.clone()),
            }
        }
        BindingValue::EventRef(name) => IrBindingValue::EventRef(name.clone()),
        BindingValue::EventCall { name, args } => {
            // Evaluated at render time, like a `Text` part: an effect it reaches is the component's (a `storage.get`).
            let mut lowered = Vec::with_capacity(args.len());
            for a in args {
                let (ir, mut e) = lower_expr_with_effects(a)?;
                effects.append(&mut e);
                lowered.push(ir);
            }
            IrBindingValue::EventCall {
                name: name.clone(),
                args: lowered,
            }
        }
        BindingValue::Role(r) => IrBindingValue::Role(lower_role(r)),
        BindingValue::StateRef(name) => {
            // Tier 14: on a component instance, a `StateRef` value
            // is a *dynamic property* — the value is a runtime
            // read of the parent component's state (or an
            // enclosing `For` binding), evaluated on every parent
            // render. On primitives it stays the TextField-style
            // state binding.
            match kind {
                Some(ElementKind::Named(_)) => IrBindingValue::Const {
                    expr: Box::new(IrExpr::Ident {
                        name: name.clone(),
                        span: b.span,
                    }),
                    span: b.span,
                },
                _ => IrBindingValue::StateRef(name.clone()),
            }
        }
        BindingValue::Prop(expr) => {
            // Tier 13: a component-instance property. The value is
            // a constant (the parser only produces literal exprs
            // here; the shape pass enforces it against the child's
            // state field anyway). Lower it pure.
            let ir = lower_expr_pure(expr)?;
            IrBindingValue::Const {
                expr: Box::new(ir),
                span: b.span,
            }
        }
    };
    Ok(IrBinding {
        key: b.key.clone(),
        value,
        span: b.span,
    })
}

fn lower_binop(op: BinOp) -> IrBinOp {
    match op {
        BinOp::Add => IrBinOp::Add,
        BinOp::Sub => IrBinOp::Sub,
        BinOp::Mul => IrBinOp::Mul,
        BinOp::Div => IrBinOp::Div,
        BinOp::Eq => IrBinOp::Eq,
        BinOp::Ne => IrBinOp::Ne,
        BinOp::Lt => IrBinOp::Lt,
        BinOp::Le => IrBinOp::Le,
        BinOp::Gt => IrBinOp::Gt,
        BinOp::Ge => IrBinOp::Ge,
        BinOp::And => IrBinOp::And,
        BinOp::Or => IrBinOp::Or,
    }
}

fn lower_unop(op: UnOp) -> IrUnOp {
    match op {
        UnOp::Neg => IrUnOp::Neg,
        UnOp::Not => IrUnOp::Not,
    }
}

/// Walk the lowered IR's `reachable_effects` against the
/// component's `requires`. Refuses on `CapabilityMissing` and
/// `CapabilityUnused`.
/// `name` or `name:port`: letters, digits, `-` and `.` (no scheme, path, wildcard or empty label), lower-cased by
/// the caller. The form `URL.host` takes in a browser (docs/adapters.md 10.1).
fn host_is_legal(host: &str) -> bool {
    let (name, port) = match host.split_once(':') {
        Some((n, p)) => (n, Some(p)),
        None => (host, None),
    };
    let name_ok = !name.is_empty()
        && !name.starts_with(['.', '-'])
        && !name.ends_with(['.', '-'])
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    let port_ok = port.is_none_or(|p| {
        !p.is_empty() && p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit()) && p != "0"
    });
    name_ok && port_ok
}

/// The host (with a non-default port) a string literal URL names, or `None` for a relative one. Only `http://`,
/// `https://` and `//` literals are read; anything else is the runtime guard's.
fn literal_host(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    let (rest, default_port) = if let Some(r) = lower.strip_prefix("https://") {
        (r, ":443")
    } else if let Some(r) = lower.strip_prefix("http://") {
        (r, ":80")
    } else {
        (lower.strip_prefix("//")?, "")
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = if !default_port.is_empty() {
        host.strip_suffix(default_port).unwrap_or(host)
    } else {
        host
    };
    Some(host.to_string())
}

/// A `Storage` namespace (docs/adapters.md section 12): it becomes part of a `localStorage` key, so it holds no
/// `:` (which would let one namespace spell another's keys) and nothing that needs escaping.
fn namespace_is_legal(ns: &str) -> bool {
    let b = ns.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn check_bounds_legal(c: &IrComponent) -> Result<(), LowerError> {
    for req in &c.requires {
        if req.name == "Credential" && req.bounds.is_empty() {
            return Err(LowerError::NarrowingIllegal {
                component: c.name.clone(),
                capability: req.name.clone(),
                reason: "a `Credential` has a name: `requires Credential(\"api\");`".to_string(),
                span: req.span,
            });
        }
        if req.bounds.is_empty() {
            continue;
        }
        if req.name == "Location" {
            let legal =
                req.bounds.len() == 1 && matches!(req.bounds[0].as_str(), "coarse" | "fine");
            if !legal {
                return Err(LowerError::NarrowingIllegal {
                    component: c.name.clone(),
                    capability: req.name.clone(),
                    reason: "a `Location` bound is exactly one of \"coarse\" (2 decimals, the default) or \"fine\" (5 decimals)".to_string(),
                    span: req.span,
                });
            }
            continue;
        }
        if req.name == "Credential" {
            // docs/adapters.md section 18: a name (the first bound), then the extra hosts it may be sent to.
            if !namespace_is_legal(&req.bounds[0]) {
                return Err(LowerError::NarrowingIllegal {
                    component: c.name.clone(),
                    capability: req.name.clone(),
                    reason: "a `Credential` is named by its first bound: 1 to 32 lower-case letters, digits and `-`, starting with a letter or digit".to_string(),
                    span: req.span,
                });
            }
            for b in &req.bounds[1..] {
                if !host_is_legal(&b.to_ascii_lowercase()) {
                    return Err(LowerError::NarrowingIllegal {
                        component: c.name.clone(),
                        capability: req.name.clone(),
                        reason: format!(
                            "`{b}` is not a host; after the name, a `Credential` takes hosts as `name` or `name:port`"
                        ),
                        span: req.span,
                    });
                }
            }
            continue;
        }
        if req.name == "Storage" {
            let legal = req.bounds.len() == 1 && namespace_is_legal(&req.bounds[0]);
            if !legal {
                return Err(LowerError::NarrowingIllegal {
                    component: c.name.clone(),
                    capability: req.name.clone(),
                    reason: "a `Storage` bound is exactly one namespace: 1 to 32 lower-case letters, digits and `-`, starting with a letter or digit".to_string(),
                    span: req.span,
                });
            }
            continue;
        }
        if req.name != "Network" {
            return Err(LowerError::NarrowingIllegal {
                component: c.name.clone(),
                capability: req.name.clone(),
                reason: format!(
                    "`{}` takes no bound yet; only `Network(\"host\")`, `Location(\"coarse\" or \"fine\")`, `Storage(\"ns\")` and `Credential(\"name\", \"host\"..)` do",
                    req.name
                ),
                span: req.span,
            });
        }
        for b in &req.bounds {
            if !host_is_legal(&b.to_ascii_lowercase()) {
                return Err(LowerError::NarrowingIllegal {
                    component: c.name.clone(),
                    capability: req.name.clone(),
                    reason: format!(
                        "`{b}` is not a host; write `name` or `name:port` (letters, digits, `-` and `.`; no scheme, path or wildcard)"
                    ),
                    span: req.span,
                });
            }
        }
    }
    Ok(())
}

fn check_capabilities(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
        check_bounds_legal(c)?;
        let declared: BTreeSet<String> = c.requires.iter().map(|r| r.name.clone()).collect();
        let allowed: BTreeSet<String> = c
            .requires
            .iter()
            .filter(|r| r.name == "Network")
            .flat_map(|r| r.bounds.iter().map(|b| b.to_ascii_lowercase()))
            .collect();
        for eff in &c.reachable_effects {
            if !declared.contains(&eff.capability) {
                return Err(LowerError::CapabilityMissing {
                    component: c.name.clone(),
                    capability: eff.capability.clone(),
                    call_path: eff.path.join("."),
                    span: eff.span,
                });
            }
            // A write reaches this page's own origin only (sections 17 and 21): no host is ever allowed for it.
            let is_post = eff.capability == "NetworkWrite";
            if (eff.path == ["net", "get"]
                || eff.path == ["net", "fetch"]
                || eff.path == ["subscribe"]
                || is_post)
                && let Some(url) = &eff.literal_arg
                && let Some(host) = literal_host(url)
                && (is_post || !allowed.contains(&host))
            {
                return Err(LowerError::BoundViolated {
                    component: c.name.clone(),
                    host,
                    url: url.clone(),
                    span: eff.span,
                });
            }
        }
        // docs/bearer-token.md: a call names the credential the component declares.
        crate::credential::check_names(c)?;
        // docs/adapters.md section 18: one credential per component, and the hosts it names are hosts `Network` allows.
        let credentials: Vec<_> = c
            .requires
            .iter()
            .filter(|r| r.name == "Credential")
            .collect();
        if credentials.len() > 1 {
            return Err(LowerError::CredentialMisuse {
                rule: "credential-ambiguous",
                component: c.name.clone(),
                detail: format!(
                    "declares {} credentials; a request cannot choose between them, so a component declares one `Credential`",
                    credentials.len()
                ),
                span: credentials[1].span,
            });
        }
        for cred in &credentials {
            for host in &cred.bounds[1..] {
                if !allowed.contains(&host.to_ascii_lowercase()) {
                    return Err(LowerError::CredentialMisuse {
                        rule: "credential-host-not-allowed",
                        component: c.name.clone(),
                        detail: format!(
                            "`Credential(\"{}\", \"{host}\")` names a host its `Network` does not allow; add `requires Network(\"{host}\");` or drop the host",
                            cred.bounds[0]
                        ),
                        span: cred.span,
                    });
                }
            }
        }
        for req in &c.requires {
            // A credential is used by any request the component can make (`net.get`, `net.fetch`, `net.post`, `net.put`,
            // `net.delete`, and the connection of a `subscribe`).
            let used = if req.name == "Credential" {
                c.reachable_effects.iter().any(|e| {
                    e.path == ["subscribe"]
                        || e.path.len() == 2
                            && e.path[0] == "net"
                            && matches!(
                                e.path[1].as_str(),
                                "get" | "fetch" | "post" | "put" | "delete"
                            )
                })
            } else {
                // a `Map` asks its tile host for images: that is the use of `Network` it declares (docs/maps.md)
                c.reachable_effects.iter().any(|e| e.capability == req.name)
                    || (req.name == "Network" && crate::map::view_has_map(&c.view.body))
            };
            if !used {
                return Err(LowerError::CapabilityUnused {
                    component: c.name.clone(),
                    capability: req.name.clone(),
                    span: req.span,
                });
            }
        }
    }
    Ok(())
}

// Marker so Position is referenced (avoids dead-code warnings if
// Position is only used by error variants in LowerError).
#[allow(dead_code)]
const _POSITION: Position = Position { line: 0, column: 0 };

/// Phase 2: the `match-non-exhaustive` check.
///
/// A `match` whose scrutinee is a state field of type
/// `Result[T, E]` must cover every constructor the runtime can
/// produce, unless a catch-all arm (`_`, or a bare identifier —
/// Tier 6 semantics: it binds the whole value, so it matches
/// anything) is present. Required constructors:
///
/// - `{Ok, Err}` for a plain `Result` field,
/// - `{Ok, Err, Cancelled}` when the field is ever assigned from
///   an `await(...)` expression (an awaited `Request[T]`
///   resolves to all three) or when an arm matches `Cancelled`.
///
/// A `match` on anything other than a state field identifier is
/// not statically resolved here; the runtime throw (Tier 6)
/// remains the safety net for those. Phase 2's full type
/// inference will widen this check.
fn check_match_exhaustiveness(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
        let state_types: BTreeMap<String, &IrType> =
            c.state.iter().map(|s| (s.name.clone(), &s.ty)).collect();
        let await_driven = await_driven_fields(c);

        for m in component_matches(c) {
            check_one_match(&c.name, m, &state_types, &await_driven)?;
        }
    }
    Ok(())
}

use std::collections::BTreeMap;

/// Every `IrExpr::Match` reachable from the component's event
/// handlers and its view (including nested inside arm bodies).
fn component_matches(c: &IrComponent) -> Vec<&IrExpr> {
    let mut out: Vec<&IrExpr> = Vec::new();
    for e in &c.events {
        for s in &e.body {
            stmt_matches(s, &mut out);
        }
    }
    view_matches(&c.view.body, &mut out);
    out
}

fn stmt_matches<'a>(s: &'a IrStmt, out: &mut Vec<&'a IrExpr>) {
    match s {
        IrStmt::Assign { value, .. } => expr_matches(value, out),
        IrStmt::Expr { expr, .. } => expr_matches(expr, out),
        IrStmt::If {
            cond, then, else_, ..
        } => {
            expr_matches(cond, out);
            for t in then.iter().chain(else_) {
                stmt_matches(t, out);
            }
        }
    }
}

fn expr_matches<'a>(e: &'a IrExpr, out: &mut Vec<&'a IrExpr>) {
    if matches!(e, IrExpr::Match { .. }) {
        out.push(e);
    }
    for child in e.children() {
        expr_matches(child, out);
    }
}

fn view_matches<'a>(node: &'a IrViewNode, out: &mut Vec<&'a IrExpr>) {
    match node {
        IrViewNode::Element { children, .. } => {
            for ch in children {
                view_matches(ch, out);
            }
        }
        IrViewNode::Text { parts, .. } => {
            for p in parts {
                if let IrTextPart::Expr(e) = p {
                    expr_matches(e, out);
                }
            }
        }
        IrViewNode::For { over, children, .. } => {
            expr_matches(over, out);
            for ch in children {
                view_matches(ch, out);
            }
        }
        IrViewNode::If { cond, children, .. } => {
            expr_matches(cond, out);
            for ch in children {
                view_matches(ch, out);
            }
        }
    }
}

/// The state fields that are ever assigned from an
/// `await(...)` expression. These hold `Ok | Err | Cancelled`
/// at runtime even when their declared type says `Result[T, E]`.
fn await_driven_fields(c: &IrComponent) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for e in &c.events {
        for s in IrStmt::leaves(&e.body) {
            match s {
                IrStmt::Assign { target, value, .. } if expr_contains_await(value) => {
                    set.insert(target.clone());
                }
                _ => {}
            }
        }
    }
    set
}

/// Every call the emitters know how to write (`sleep`, the two-segment adapters, and `PURE_BUILTINS`; `filter`, `map`,
/// `sort` and `len` are lowered before this point). A call to anything else has no emission: the web emitter would
/// write `x = /* unsupported call */;`, a JavaScript syntax error that stops the whole page, so it is refused here
/// (`call-unknown`, docs/ops-console.md).
fn is_known_call(path: &[String]) -> bool {
    match path {
        [one] => {
            one == "sleep" || one == "t" || PURE_BUILTINS.iter().any(|(name, _, _)| name == one)
        }
        [a, b] => matches!(
            (a.as_str(), b.as_str()),
            ("clock", "now")
                | ("storage", "get" | "set")
                | ("router", "go" | "replace")
                | ("credential", "set" | "clear")
                | ("json", "get" | "items" | "quote")
                | ("location", "current")
                | ("fmt", "number" | "percent" | "date" | "time")
                | ("locale", "current" | "set")
                | (
                    "net",
                    "search" | "get" | "fetch" | "post" | "put" | "delete"
                )
        ),
        _ => false,
    }
}

/// The pure builtins that are plain calls (docs/lists.md): name, argument count, the form shown on a misuse.
const PURE_BUILTINS: &[(&str, usize, &str)] = &[
    ("reverse", 1, "exactly one argument: reverse(list)"),
    (
        "contains",
        2,
        "exactly two arguments: contains(text, needle)",
    ),
    ("lower", 1, "exactly one argument: lower(text)"),
    ("fold", 1, "exactly one argument: fold(text)"),
    ("num", 2, "exactly two arguments: num(text, default)"),
    ("take", 2, "exactly two arguments: take(list, n)"),
    ("skip", 2, "exactly two arguments: skip(list, n)"),
    ("concat", 2, "exactly two arguments: concat(a, b)"),
    ("move", 3, "exactly three arguments: move(list, from, to)"),
    (
        "distance",
        3,
        "exactly three arguments: distance(a, b, default)",
    ),
    ("min", 2, "exactly two arguments: min(a, b)"),
    ("max", 2, "exactly two arguments: max(a, b)"),
    ("abs", 1, "exactly one argument: abs(n)"),
    ("floor", 1, "exactly one argument: floor(n)"),
    ("ceil", 1, "exactly one argument: ceil(n)"),
    ("round", 1, "exactly one argument: round(n)"),
    ("lines", 1, "exactly one argument: lines(text)"),
    ("split", 2, "exactly two arguments: split(text, sep)"),
    ("join", 2, "exactly two arguments: join(list, sep)"),
    ("trim", 1, "exactly one argument: trim(text)"),
    ("upper", 1, "exactly one argument: upper(text)"),
    (
        "starts_with",
        2,
        "exactly two arguments: starts_with(text, prefix)",
    ),
    (
        "ends_with",
        2,
        "exactly two arguments: ends_with(text, suffix)",
    ),
];

/// Lower a Tier 11 list-operation operand. Filter predicates,
/// map bodies, and list arguments must be pure: no effects (the
/// capability check) and no suspends (`await`, `sleep`) — the
/// list operation runs synchronously inside the render pass.
fn lower_expr_pure(e: &Expr) -> Result<IrExpr, LowerError> {
    let (ir, effects) = lower_expr_with_effects(e)?;
    if !effects.is_empty() {
        return Err(LowerError::PureLambdaRequired { span: e.span() });
    }
    if ir_expr_suspends(&ir) {
        return Err(LowerError::PureLambdaRequired { span: e.span() });
    }
    Ok(ir)
}

/// True when the (already lowered) expression can suspend: an
/// `await(...)`, a `sleep(ms)` call, or anything carrying one.
pub fn ir_expr_suspends(e: &IrExpr) -> bool {
    match e {
        IrExpr::Await { .. } => true,
        IrExpr::Call { path, args, .. } => {
            (path.len() == 1 && path[0] == "sleep") || args.iter().any(ir_expr_suspends)
        }
        IrExpr::Binary { lhs, rhs, .. } => ir_expr_suspends(lhs) || ir_expr_suspends(rhs),
        IrExpr::Unary { operand, .. } => ir_expr_suspends(operand),
        IrExpr::If {
            cond, then, else_, ..
        } => ir_expr_suspends(cond) || ir_expr_suspends(then) || ir_expr_suspends(else_),
        IrExpr::Ok { value, .. } | IrExpr::Some { value, .. } => ir_expr_suspends(value),
        IrExpr::Err { reason, .. } => ir_expr_suspends(reason),
        IrExpr::ListLit { items, .. } => items.iter().any(ir_expr_suspends),
        IrExpr::Filter { list, cond, .. } => ir_expr_suspends(list) || ir_expr_suspends(cond),
        IrExpr::Map { list, body, .. } => ir_expr_suspends(list) || ir_expr_suspends(body),
        IrExpr::SortBy { list, key, .. } => ir_expr_suspends(list) || ir_expr_suspends(key),
        IrExpr::Len { list, .. } => ir_expr_suspends(list),
        IrExpr::Match {
            scrutinee, arms, ..
        } => ir_expr_suspends(scrutinee) || arms.iter().any(|a| ir_expr_suspends(&a.body)),
        _ => false,
    }
}

fn expr_contains_await(e: &IrExpr) -> bool {
    match e {
        IrExpr::Await { .. } => true,
        IrExpr::Binary { lhs, rhs, .. } => expr_contains_await(lhs) || expr_contains_await(rhs),
        IrExpr::Unary { operand, .. } => expr_contains_await(operand),
        IrExpr::If {
            cond, then, else_, ..
        } => expr_contains_await(cond) || expr_contains_await(then) || expr_contains_await(else_),
        IrExpr::Call { args, .. } => args.iter().any(expr_contains_await),
        IrExpr::Ok { value, .. } => expr_contains_await(value),
        IrExpr::Err { reason, .. } => expr_contains_await(reason),
        IrExpr::Some { value, .. } => expr_contains_await(value),
        IrExpr::Match {
            scrutinee, arms, ..
        } => expr_contains_await(scrutinee) || arms.iter().any(|a| expr_contains_await(&a.body)),
        _ => false,
    }
}

fn check_one_match(
    component_name: &str,
    m: &IrExpr,
    state_types: &BTreeMap<String, &IrType>,
    await_driven: &BTreeSet<String>,
) -> Result<(), LowerError> {
    let (scrutinee, arms, span) = match m {
        IrExpr::Match {
            scrutinee,
            arms,
            span,
        } => (scrutinee, arms, *span),
        _ => return Ok(()),
    };
    let field = match scrutinee.as_ref() {
        IrExpr::Ident { name, .. } => name,
        _ => return Ok(()),
    };
    let ty = match state_types.get(field) {
        Some(t) => *t,
        None => return Ok(()),
    };
    // Tier 8: `Result` needs {Ok, Err}, `Option` needs
    // {Some, None}. `List` has no patterns and is not matched.
    let mut required: Vec<&str> = match ty {
        IrType::Result { .. } => vec!["Ok", "Err"],
        IrType::Option { .. } => vec!["Some", "None"],
        IrType::Int
        | IrType::Float
        | IrType::Bool
        | IrType::String
        | IrType::List { .. }
        | IrType::Request { .. } => return Ok(()),
    };

    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut catch_all = false;
    for arm in arms {
        match &arm.pattern {
            IrPattern::Wildcard { .. } | IrPattern::Ident { .. } => catch_all = true,
            IrPattern::Variant { name, .. } | IrPattern::UnitVariant { name, .. } => {
                covered.insert(name.clone());
            }
        }
    }

    // An awaited value resolves to all three Result constructors
    // even though the declared type says `Result`, so treat the
    // match as tri-variant. The presence of a `Cancelled` arm is
    // the programmer announcing the same thing.
    let tri_variant = await_driven.contains(field) || covered.contains("Cancelled");
    if tri_variant {
        required.push("Cancelled");
    }
    let missing: Vec<String> = required
        .iter()
        .filter(|v| !covered.contains(**v))
        .map(|v| v.to_string())
        .collect();

    if !catch_all && !missing.is_empty() {
        return Err(LowerError::MatchNonExhaustive {
            component: component_name.to_string(),
            field: field.clone(),
            missing,
            span,
        });
    }
    Ok(())
}
