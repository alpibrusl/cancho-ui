//! Routing (docs/routing.md): the `route Event;` declaration, the `router.go` / `router.replace` builtins and the
//! `NavLink` element. This pass owns their rule tags; the shapes are in `lower.rs` and `controls.rs`.

use cancho_ui_syntax::ast::{Component, Schedule};
use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The most characters of a path (`router.go`, a `NavLink`'s literal `to:`), docs/routing.md section 4.
pub const PATH_MAX: usize = 2000;

/// Is `path` something a URL fragment can carry as a route? It starts with `/`, is at most `PATH_MAX` characters, and holds
/// only printable ASCII other than space, `#`, `"`, `<`, `>` and the backtick, so the page writes `#` + path and reads the same
/// path back. (Measured in Chromium: those five are percent-encoded in the fragment, every other printable ASCII
/// character, `%` included, comes back as written; docs/routing.md section 9.)
pub fn path_is_legal(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= PATH_MAX
        && path
            .bytes()
            .all(|b| (0x21..=0x7e).contains(&b) && !matches!(b, b'#' | b'"' | b'<' | b'>' | b'`'))
}

fn refused(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// Check one `route Event;` against its component and lower it. `earlier` are the schedules lowered before it.
pub fn lower_route(
    c: &Component,
    s: &Schedule,
    earlier: &[IrSchedule],
) -> Result<IrSchedule, LowerError> {
    let Some(handler) = c.events.iter().find(|e| e.name == s.event) else {
        return Err(refused(
            "route-event-unknown",
            format!(
                "component `{}`: `route {}` names an event the component does not handle; add `on {}(path) {{ .. }}`",
                c.name, s.event, s.event
            ),
            s.span,
        ));
    };
    if handler.params.len() != 1 {
        return Err(refused(
            "route-event-params",
            format!(
                "component `{}`: `route {}` runs `{}` with the path, so it takes exactly one parameter and it takes {}",
                c.name,
                s.event,
                s.event,
                handler.params.len()
            ),
            s.span,
        ));
    }
    if earlier.iter().any(|o| o.kind == IrScheduleKind::Route) {
        return Err(refused(
            "route-duplicate",
            format!(
                "component `{}`: `route` is declared twice; one event receives every path",
                c.name
            ),
            s.span,
        ));
    }
    Ok(IrSchedule {
        kind: IrScheduleKind::Route,
        event: s.event.clone(),
        span: s.span,
    })
}

/// What the URL costs: reading the fragment (`route`) and writing it (`router.go`, `router.replace`) is the `Router`
/// capability. A `route` is an effect site like a call, so the component must declare it (`capability-missing`).
pub fn route_effects(schedules: &[IrSchedule]) -> Vec<IrReachableEffect> {
    schedules
        .iter()
        .filter(|s| s.kind == IrScheduleKind::Route)
        .map(|s| IrReachableEffect {
            capability: "Router".to_string(),
            path: vec!["route".to_string()],
            literal_arg: None,
            span: s.span,
        })
        .collect()
}

/// The arity of `router.go(path)` and `router.replace(path)`: one, always.
pub fn check_router_call(name: &str, args: usize, span: Span) -> Result<(), LowerError> {
    if args != 1 {
        return Err(LowerError::BuiltinMisuse {
            name: format!("router.{name}"),
            detail: format!("exactly one argument: router.{name}(path)"),
            span,
        });
    }
    Ok(())
}
