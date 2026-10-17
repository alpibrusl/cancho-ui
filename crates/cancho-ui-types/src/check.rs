//! Phase 1 validation passes. Phase 1 is deliberately thin — it
//! checks that the AST is a well-formed Tier 1 file (one or more
//! components, each with a single `view` clause, no reserved words
//! used as component names, no duplicate component names in one
//! file). Phase 2 layers on the capability checker and the full
//! semantic model from `docs/semantics.md`.

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

    #[error("element kind `{name}` is not PascalCase; built-in primitives are PascalCase and user components must match")]
    NotPascalCaseKind { name: String, span: Span },
}

/// Run the Phase 1 validation pass over a parsed file. Returns the
/// first error encountered; Phase 2 will batch every independent
/// error per `spec/diagnostics/README.md`.
pub fn check_file(file: &File) -> Result<(), TypeError> {
    let mut seen: Vec<(&str, Span)> = Vec::new();
    for c in &file.components {
        check_component(c)?;

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

fn check_component(c: &Component) -> Result<(), TypeError> {
    check_view_node(&c.name, &c.view.body)
}

fn check_view_node(_component: &str, node: &ViewNode) -> Result<(), TypeError> {
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
            check_accessibility(accessibility);
            for c in children {
                check_view_node(_component, c)?;
            }
            for b in bindings {
                check_binding(b)?;
            }
        }
        ViewNode::Text { .. } => {}
    }
    Ok(())
}

fn check_binding(_b: &Binding) -> Result<(), TypeError> {
    // Phase 2 enforces binding types (e.g. `on:` must reference a
    // declared event; `role:` must be one of the built-in roles;
    // `label:` must be non-empty for interactive primitives). For
    // now, the parser accepts and the type checker passes through.
    Ok(())
}

fn check_accessibility(_acc: &Accessibility) {
    // Phase 1 records accessibility; Phase 2 enforces it. For now,
    // `role` and `label` are accepted at the parse level and
    // passed through to the IR.
    let _ = Role::Text;
}

/// The reserved-word set. Source: `spec/grammar/README.md` §2.
/// Names in this set are refused as component / element-kind /
/// state-field / capability names per the `name-reserved` rule.
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
        // `effect` is reserved at the language level (per the rule
        // catalog in `spec/diagnostics/README.md`) but is not a hard
        // keyword in the lexer, so it lexes as an identifier and
        // the reserved-word check fires here.
        let f = parse_ok("component effect { view { Text { \"hi\" } } }\n");
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, TypeError::ReservedComponentName { .. }));
    }

    #[test]
    fn lowercase_name_refused() {
        let f = parse_ok("component hello { view { Text { \"hi\" } } }\n");
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, TypeError::NotPascalCase { .. }));
    }

    #[test]
    fn duplicate_refused() {
        let f = parse_ok(
            "component X { view { Text { \"a\" } } }\ncomponent X { view { Text { \"b\" } } }\n",
        );
        let err = check_file(&f).unwrap_err();
        assert!(matches!(err, TypeError::DuplicateComponent { .. }));
    }
}