//! Phase 1 / Tier 4 validation: type checking.
//!
//! Two responsibilities:
//! 1. **Shape** — PascalCase component names, reserved words, no
//!    duplicate components in one file.
//! 2. **Capability** — every reachable effect's capability must
//!    appear in the component's `requires` list; every declared
//!    capability must have at least one reachable use.
//!
//! The capability check is implemented in `cancho-ui-ir` because
//! it operates on the lowered IR (so `types` does not need to
//! depend on `ir`, and `ir` does not need to depend on `types` —
//! avoiding the cycle). `check_file` runs shape first, then calls
//! `lower_file` for the capability pass.

use cancho_ui_syntax::{
    ast::{Accessibility, Binding, Component, ElementKind, File, Role, ViewNode},
    Span,
};
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum TypeError {
    #[error("component `{name}` is declared but has no `view` clause")]
    MissingView { name: String, span: Span },

    #[error("duplicate component `{name}` in this file (declared at {first_span:?} and {second_span:?})")]
    DuplicateComponent {
        name: String,
        first_span: Span,
        second_span: Span,
    },

    #[error("component name `{name}` uses the reserved word `{name}`")]
    ReservedComponentName { name: String, span: Span },

    #[error("component name `{name}` is not PascalCase; component names must start with an uppercase letter")]
    NotPascalCase { name: String, span: Span },

    #[error("element kind `{name}` is not PascalCase; element kinds must match a built-in primitive or a PascalCase component name")]
    NotPascalCaseKind { name: String, span: Span },
}

/// What `check_file` can produce. The shape pass (PascalCase,
/// reserved words, duplicates) lives in this crate. The
/// capability pass lives in `cancho-ui-ir` (against the lowered
/// IR) and is exposed here so the type crate's tests can match
/// on either kind of refusal.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum CheckError {
    #[error(transparent)]
    Shape(TypeError),

    #[error("component `{component}` performs `{call_path}` which requires the `{capability}` capability, but `requires {capability}` is not declared")]
    CapabilityMissing {
        component: String,
        capability: String,
        call_path: String,
        span: Span,
    },

    #[error("component `{component}` declares `requires {capability};` but no reachable effect uses it")]
    CapabilityUnused {
        component: String,
        capability: String,
        span: Span,
    },

    #[error("`match` on `{field}` in component `{component}` is not exhaustive; missing arm(s): {missing:?}")]
    MatchNonExhaustive {
        component: String,
        field: String,
        missing: Vec<String>,
        span: Span,
    },
}

/// Run the shape pass + the capability pass. Returns the first
/// error. Phase 2 will batch every independent error per
/// `spec/diagnostics/README.md`.
pub fn check_file(file: &File) -> Result<(), CheckError> {
    check_shape(file).map_err(CheckError::Shape)?;
    match cancho_ui_ir::lower_file(file) {
        Ok(_) => Ok(()),
        Err(cancho_ui_ir::lower::LowerError::EmptySpan { .. }) => {
            Err(CheckError::Shape(TypeError::MissingView {
                name: "<file>".into(),
                span: file.span,
            }))
        }
        Err(cancho_ui_ir::lower::LowerError::CapabilityMissing {
            component,
            capability,
            call_path,
            span,
        }) => Err(CheckError::CapabilityMissing {
            component,
            capability,
            call_path,
            span,
        }),
        Err(cancho_ui_ir::lower::LowerError::CapabilityUnused {
            component,
            capability,
            span,
        }) => Err(CheckError::CapabilityUnused {
            component,
            capability,
            span,
        }),
        Err(cancho_ui_ir::lower::LowerError::MatchNonExhaustive {
            component,
            field,
            missing,
            span,
        }) => Err(CheckError::MatchNonExhaustive {
            component,
            field,
            missing,
            span,
        }),
    }
}

fn check_shape(file: &File) -> Result<(), TypeError> {
    let mut seen: Vec<(&str, Span)> = Vec::new();
    for c in &file.components {
        check_component_shape(c)?;

        if is_reserved_word(&c.name) {
            return Err(TypeError::ReservedComponentName {
                name: c.name.clone(),
                span: c.span,
            });
        }
        if !is_pascal_case(&c.name) {
            return Err(TypeError::NotPascalCase {
                name: c.name.clone(),
                span: c.span,
            });
        }

        for (other_name, other_span) in &seen {
            if *other_name == c.name {
                return Err(TypeError::DuplicateComponent {
                    name: c.name.clone(),
                    first_span: *other_span,
                    second_span: c.span,
                });
            }
        }
        seen.push((&c.name, c.span));
    }
    Ok(())
}

fn check_component_shape(c: &Component) -> Result<(), TypeError> {
    check_view_node_shape(&c.view.body)
}

fn check_view_node_shape(node: &ViewNode) -> Result<(), TypeError> {
    match node {
        ViewNode::Element { kind, accessibility, children, bindings, span } => {
            if let ElementKind::Named(name) = kind {
                if !is_pascal_case(name) {
                    return Err(TypeError::NotPascalCaseKind {
                        name: name.clone(),
                        span: *span,
                    });
                }
            }
            check_accessibility_shape(accessibility);
            for c in children {
                check_view_node_shape(c)?;
            }
            for b in bindings {
                check_binding_shape(b)?;
            }
        }
        ViewNode::Text { .. } => {}
        // Tier 8: `For` brings its own children into scope as a
        // loop binding; nothing to shape-check per-child yet.
        ViewNode::For { children, .. } => {
            for c in children {
                check_view_node_shape(c)?;
            }
        }
    }
    Ok(())
}

fn check_binding_shape(_b: &Binding) -> Result<(), TypeError> {
    Ok(())
}

fn check_accessibility_shape(_acc: &Accessibility) {
    let _ = Role::Text;
}

/// The reserved-word set. Source: `spec/grammar/README.md` §2.
pub fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "component"
            | "state"
            | "view"
            | "on"
            | "requires"
            | "effect"
            | "capability"
            | "foreign"
            | "module"
            | "import"
            | "pub"
            | "target"
            | "let"
            | "fn"
            | "if"
            | "else"
            | "match"
            | "return"
    )
}

fn is_pascal_case(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancho_ui_syntax::parser::parse;
    use cancho_ui_syntax::lexer::lex;

    fn parse_ok(src: &str) -> File {
        let toks = lex(src).expect("lex");
        parse(toks).expect("parse")
    }

    #[test]
    fn hello_world_checks_clean() {
        let f = parse_ok("component Hello { view { Text { \"hi\" } } }\n");
        check_file(&f).expect("check");
    }

    #[test]
    fn reserved_name_refused() {
        let f = parse_ok("component effect { view { Text { \"hi\" } } }\n");
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, CheckError::Shape(TypeError::ReservedComponentName { .. })));
    }

    #[test]
    fn lowercase_name_refused() {
        let f = parse_ok("component hello { view { Text { \"hi\" } } }\n");
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, CheckError::Shape(TypeError::NotPascalCase { .. })));
    }

    #[test]
    fn duplicate_refused() {
        let f = parse_ok(
            "component X { view { Text { \"a\" } } }\ncomponent X { view { Text { \"b\" } } }\n",
        );
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, CheckError::Shape(TypeError::DuplicateComponent { .. })));
    }

    #[test]
    fn capability_missing_refused() {
        let src = "\
component Ticker {
    state n: Int = 0;
    on Tick { n = clock.now(); }
    view { Text { \"hi\" } }
}
";
        let f = parse_ok(src);
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, CheckError::CapabilityMissing { .. }));
    }

    #[test]
    fn capability_unused_refused() {
        let src = "\
component Unused {
    requires Clock;
    view { Text { \"hi\" } }
}
";
        let f = parse_ok(src);
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, CheckError::CapabilityUnused { .. }));
    }

    #[test]
    fn capability_clock_check_passes() {
        let src = "\
component Ticker {
    requires Clock;
    state n: Int = 0;
    on Tick { n = clock.now(); }
    view { Text { \"hi\" } }
}
";
        let f = parse_ok(src);
        check_file(&f).expect("check");
    }
}