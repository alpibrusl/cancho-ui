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
    ast::{Accessibility, Binding, BindingValue, Component, ElementKind, Expr, File, Role, Type, ViewNode},
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

    /// Tier 12.`{name}` is not a component declared in this file,
    /// so it cannot be instantiated in a view.
    #[error("component kind `{name}` is not declared in this file; add a `component {name} {{ ... }}` or use a built-in primitive")]
    UnknownComponentKind { name: String, span: Span },

    /// Tier 12: a component instance is opaque — it owns its own
    /// state, view, and handlers; the parent configures nothing
    /// (properties arrive in a later tier).
    #[error("component instance `{name}` takes no bindings; it is opaque — its own state and handlers govern it (properties arrive in a later tier)")]
    InstanceTakesBindings { name: String, span: Span },

    /// Tier 13: property key is not a state field of the child.
    #[error("component instance `{component}` has no state field `{key}` to pass as a property; add `state {key}: ...` to `component {component}` or remove the binding")]
    PropertyUnknownKey { component: String, key: String, span: Span },

    /// Tier 13: prop value's literal kind does not match the
    /// child state field's declared type (or is not a constant).
    #[error("property `{key}` of `{component}` does not match the state field's declared type (string property or int/float/bool literal constants only)")]
    PropertyTypeMismatch { component: String, key: String, span: Span },

    /// Tier 14: dynamic property source is not a state field of
    /// the enclosing component (nor an enclosing `For` binding).
    #[error("`{key}` is not a state field of `{component}` (and not an enclosing `For` binding); a dynamic property value must be read from the parent's state")]
    PropertySourceUnknown { component: String, key: String, span: Span },

    /// Tier 13: the same property is passed twice on an instance.
    #[error("property `{key}` of `{component}` is passed twice on this instance")]
    DuplicatePropKey {
        component: String,
        key: String,
        first_span: Span,
        second_span: Span,
    },

    #[error("component instance `{name}` takes no children; its own view governs what renders")]
    InstanceTakesChildren { name: String, span: Span },

    #[error("component instance `{name}` takes no role/label of its own; the inner components carry their own accessibility")]
    InstanceTakesAccessibility { name: String, span: Span },

    #[error("component instance chain is cyclic: {} — the emitted mount would recurse forever", chain.join(" → "))]
    ComponentCycle { chain: Vec<String> },
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

    #[error(
        "a lambda (`x => expr`) is only supported as the second argument of `filter(list, pred)` or `map(list, transform)`"
    )]
    LambdaUnsupported { span: Span },

    #[error("`{name}` is misused here (expected {detail})")]
    BuiltinMisuse { name: String, detail: String, span: Span },

    #[error("the lambda body must be pure: it may not `await(...)` or call effectful/suspending builtins")]
    PureLambdaRequired { span: Span },
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
        Err(cancho_ui_ir::lower::LowerError::LambdaUnsupported { span }) => {
            Err(CheckError::LambdaUnsupported { span })
        }
        Err(cancho_ui_ir::lower::LowerError::BuiltinMisuse { name, detail, span }) => {
            Err(CheckError::BuiltinMisuse { name, detail, span })
        }
        Err(cancho_ui_ir::lower::LowerError::PureLambdaRequired { span }) => {
            Err(CheckError::PureLambdaRequired { span })
        }
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
    // Tier 12: a view may instantiate *other components from
    // this file* (same-file composition). Each instance must
    // exist, stay opaque (no bindings, no children, no own
    // accessibility), and the instance graph must not cycle.
    for c in &file.components {
        check_instances(c, file)?;
    }
    check_instance_cycle(file)?;
    Ok(())
}

/// Named kinds in `c`'s view must be declared components of the
/// same file. Tier 13: an instance may pass *properties* —
/// constant initial values for the child's state fields — but is
/// otherwise opaque (no `on:` handlers, no `role:`/`label:`, no
/// children).
fn check_instances(c: &Component, file: &File) -> Result<(), TypeError> {
    fn is_const(value: &BindingValue, ty: &Type) -> bool {
        // A prop value must be a literal whose kind matches the
        // target state field's declared type. String bindings
        // (`title: "Ada"`) parse as `String` and count as string
        // properties; int/float/bool literals lift to `Prop`.
        match value {
            BindingValue::String(_) => matches!(ty, Type::String),
            BindingValue::Prop(Expr::IntLit { .. }) => matches!(ty, Type::Int),
            BindingValue::Prop(Expr::FloatLit { .. }) => matches!(ty, Type::Float),
            BindingValue::Prop(Expr::BoolLit { .. }) => matches!(ty, Type::Bool),
            _ => false,
        }
    }

    fn walk(
        node: &ViewNode,
        file: &File,
        parent: &Component,
        locals: &mut Vec<String>,
    ) -> Result<(), TypeError> {
        match node {
            ViewNode::Element { kind, bindings, children, accessibility, span } => {
                if let ElementKind::Named(name) = kind {
                    let target = file
                        .components
                        .iter()
                        .find(|d| d.name == *name);
                    if target.is_none() {
                        return Err(TypeError::UnknownComponentKind {
                            name: name.clone(),
                            span: *span,
                        });
                    }
                    if !children.is_empty() {
                        return Err(TypeError::InstanceTakesChildren {
                            name: name.clone(),
                            span: *span,
                        });
                    }
                    if accessibility.role.is_some() || accessibility.label.is_some() {
                        return Err(TypeError::InstanceTakesAccessibility {
                            name: name.clone(),
                            span: *span,
                        });
                    }
                    let target = target.unwrap();
                    let mut seen_keys: Vec<(&str, Span)> = Vec::new();
                    for b in bindings {
                        match &b.value {
                            // `on:` — the child owns its handlers;
                            // the parent does not wire them. A
                            // `role:` binding value is the same
                            // category (the instance carries no
                            // role of its own).
                            BindingValue::EventRef(_) | BindingValue::Role(_) => {
                                return Err(TypeError::InstanceTakesBindings {
                                    name: name.clone(),
                                    span: b.span,
                                });
                            }
                            BindingValue::StateRef(source) => {
                                // Tier 14: a *dynamic property* —
                                // the value is read from the
                                // parent's state (or an enclosing
                                // `For` binding) at each render.
                                // The source must exist; the child
                                // field's declared type must match
                                // the source's when the source is
                                // typed (a `For` binding's element
                                // type is not tracked yet, so a
                                // local source skips the type
                                // match).
                                let field = match target.state.iter().find(|s| s.name == b.key) {
                                    Some(f) => f,
                                    None => {
                                        return Err(TypeError::PropertyUnknownKey {
                                            component: name.clone(),
                                            key: b.key.clone(),
                                            span: b.span,
                                        });
                                    }
                                };
                                let is_local = locals.iter().any(|l| l == source);
                                if !is_local {
                                    let ps = parent
                                        .state
                                        .iter()
                                        .find(|s| &s.name == source);
                                    let ps = match ps {
                                        Some(f) => f,
                                        None => {
                                            return Err(TypeError::PropertySourceUnknown {
                                                component: parent.name.clone(),
                                                key: source.clone(),
                                                span: b.span,
                                            });
                                        }
                                    };
                                    if ps.ty != field.ty {
                                        return Err(TypeError::PropertyTypeMismatch {
                                            component: name.clone(),
                                            key: b.key.clone(),
                                            span: b.span,
                                        });
                                    }
                                }
                                for (k, s) in &seen_keys {
                                    if *k == b.key.as_str() {
                                        return Err(TypeError::DuplicatePropKey {
                                            component: name.clone(),
                                            key: b.key.clone(),
                                            first_span: *s,
                                            second_span: b.span,
                                        });
                                    }
                                }
                                seen_keys.push((b.key.as_str(), b.span));
                            }
                            BindingValue::String(_) | BindingValue::Prop(_) => {
                                let field = match target.state.iter().find(|s| s.name == b.key) {
                                    Some(f) => f,
                                    None => {
                                        return Err(TypeError::PropertyUnknownKey {
                                            component: name.clone(),
                                            key: b.key.clone(),
                                            span: b.span,
                                        });
                                    }
                                };
                                if !is_const(&b.value, &field.ty) {
                                    return Err(TypeError::PropertyTypeMismatch {
                                        component: name.clone(),
                                        key: b.key.clone(),
                                        span: b.span,
                                    });
                                }
                                for (k, s) in &seen_keys {
                                    if *k == b.key.as_str() {
                                        return Err(TypeError::DuplicatePropKey {
                                            component: name.clone(),
                                            key: b.key.clone(),
                                            first_span: *s,
                                            second_span: b.span,
                                        });
                                    }
                                }
                                seen_keys.push((b.key.as_str(), b.span));
                            }
                        }
                    }
                }
                for ch in children {
                    walk(ch, file, parent, locals)?;
                }
            }
            ViewNode::For { binding, children, .. } => {
                // The `as:` binding is in scope for dynamic
                // property values inside the body.
                locals.push(binding.clone());
                for ch in children {
                    walk(ch, file, parent, locals)?;
                }
                locals.pop();
            }
            ViewNode::Text { .. } => {}
        }
        Ok(())
    }
    let mut locals: Vec<String> = Vec::new();
    walk(&c.view.body, file, c, &mut locals)
}

/// A component cannot contain (transitively) an instance of
/// itself — the emitted mount would recurse forever at runtime,
/// so the cycle is refused at compile time. References to
/// undeclared names are already refused by the instance walk;
/// the DFS treats them as leaves.
fn check_instance_cycle(file: &File) -> Result<(), TypeError> {
    const VISITING: u8 = 1;
    const DONE: u8 = 2;

    /// Which components does the named component's view
    /// instantiate? (May contain duplicates; the DFS tolerates.)
    fn adjacency<'a>(name: &str, file: &'a File) -> Vec<String> {
        let mut uses = Vec::new();
        for c in file.components.iter().filter(|d| d.name == name) {
            component_uses(c, &mut uses);
        }
        uses
    }

    fn visit(
        name: &str,
        file: &File,
        color: &mut std::collections::HashMap<String, u8>,
        chain: &mut Vec<String>,
    ) -> Result<(), TypeError> {
        match color.get(name).copied() {
            Some(VISITING) => {
                chain.push(name.to_string());
                return Err(TypeError::ComponentCycle { chain: chain.clone() });
            }
            // A fresh node: mark as used by seed first DFS roots.
            None => {
                color.insert(name.to_string(), 0);
            }
            Some(DONE) => return Ok(()),
            _ => {}
        }
        if file.components.iter().any(|d| d.name == name) {
            color.insert(name.to_string(), VISITING);
            chain.push(name.to_string());
            for dep in adjacency(name, file) {
                if visit(&dep, file, color, chain).is_err() {
                    return Err(TypeError::ComponentCycle {
                        chain: std::mem::take(chain),
                    });
                }
            }
            chain.pop();
            color.insert(name.to_string(), DONE);
        }
        Ok(())
    }

    let mut color: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
    let mut chain: Vec<String> = Vec::new();
    for c in &file.components {
        visit(&c.name, file, &mut color, &mut chain)?;
    }
    Ok(())
}

/// The components `c`'s view instantiates, in source order
/// (duplicates included; the DFS tolerates them).
fn component_uses(c: &Component, out: &mut Vec<String>) {
    fn walk(node: &ViewNode, out: &mut Vec<String>) {
        match node {
            ViewNode::Element { kind, children, .. } => {
                if let ElementKind::Named(name) = kind {
                    out.push(name.clone());
                }
                for ch in children {
                    walk(ch, out);
                }
            }
            ViewNode::For { children, .. } => {
                for ch in children {
                    walk(ch, out);
                }
            }
            ViewNode::Text { .. } => {}
        }
    }
    walk(&c.view.body, out);
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