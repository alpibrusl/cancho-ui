//! `credential.set(name, value)` and `credential.clear(name)` (docs/bearer-token.md): the program holds a token it received, in the
//! component's `Credential`, in memory. This pass owns their rule tag; the call shapes are in `lower.rs`.

use crate::ir::*;
use crate::lower::LowerError;

/// The calls, with the number of arguments each takes.
pub const CALLS: &[(&str, usize)] = &[("set", 2), ("clear", 1)];

pub fn is_call(path: &[String]) -> bool {
    path.len() == 2 && path[0] == "credential" && CALLS.iter().any(|(n, _)| *n == path[1])
}

/// The arity of `credential.set(name, value)` and `credential.clear(name)`.
pub fn check_call(
    path: &[String],
    args: usize,
    span: cancho_ui_syntax::span::Span,
) -> Result<(), LowerError> {
    let Some((name, want)) = CALLS
        .iter()
        .find(|(n, _)| path.get(1).is_some_and(|p| p == n))
    else {
        return Ok(());
    };
    if args != *want {
        let form = if *name == "set" {
            "exactly two arguments: credential.set(\"name\", token)"
        } else {
            "exactly one argument: credential.clear(\"name\")"
        };
        return Err(LowerError::BuiltinMisuse {
            name: format!("credential.{name}"),
            detail: form.to_string(),
            span,
        });
    }
    Ok(())
}

/// The name a call names is a string literal and is the `Credential` the component declares: a call cannot choose between
/// credentials (`credential-ambiguous`), and what a reviewer reads in `requires` is what the program may set.
pub fn check_names(c: &IrComponent) -> Result<(), LowerError> {
    let declared = c
        .requires
        .iter()
        .find(|r| r.name == "Credential")
        .and_then(|r| r.bounds.first());
    for e in &c.reachable_effects {
        if !is_call(&e.path) {
            continue;
        }
        let ok = matches!((&e.literal_arg, declared), (Some(given), Some(want)) if given == want);
        if !ok {
            return Err(LowerError::ControlRefused {
                code: "credential-name-invalid",
                message: match (&e.literal_arg, declared) {
                    (None, _) => format!(
                        "`{}` takes the name of the credential as a string literal, like `credential.{}(\"api\"..)`",
                        e.path.join("."),
                        e.path[1]
                    ),
                    (Some(given), Some(want)) => format!(
                        "`{}(\"{given}\"..)` names a credential this component does not declare; it declares `Credential(\"{want}\")`",
                        e.path.join(".")
                    ),
                    (Some(given), None) => format!(
                        "`{}(\"{given}\"..)`: this component declares no `Credential`",
                        e.path.join(".")
                    ),
                },
                span: e.span,
            });
        }
    }
    Ok(())
}
