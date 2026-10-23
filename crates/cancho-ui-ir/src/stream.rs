//! Streams (docs/streams.md): the `subscribe Event "url" [status Event];` declaration. This pass owns its rule tags; the machine
//! that connects, retries and pauses is in the page adapter and in `cancho_ui_replay`, which share the numbers below.

use cancho_ui_syntax::ast::{Component, Schedule};
use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The most characters of the address, as `net.get`'s and `router.go`'s are bounded (docs/streams.md section 3).
pub const URL_MAX: usize = 2000;

/// The most characters of one message (the `data` lines joined with `\n`): more fails the stream (docs/streams.md section 3).
pub const MESSAGE_MAX: usize = 65_536;

/// The wait before each reconnect in milliseconds: one entry per consecutive failure, the last for every later one. A connection
/// that reached `open` starts the list again (docs/streams.md section 3).
pub const BACKOFF_MS: [u32; 6] = [1000, 2000, 4000, 8000, 16000, 30000];

/// The five words the `status` event receives.
pub const STATUSES: [&str; 5] = ["connecting", "open", "retrying", "paused", "failed"];

/// Is `url` an address a `subscribe` may name? A path (`/api/stream`, `api/stream`) or an `http` / `https` URL with a host, in
/// printable ASCII with no space, no fragment, no backslash and no credentials, at most `URL_MAX` characters. Anything else,
/// a protocol-relative `//host/x` and every other scheme included, is refused: the page listens where a reviewer can read.
pub fn url_is_legal(url: &str) -> Result<(), &'static str> {
    if url.is_empty() {
        return Err("it is empty");
    }
    if url.len() > URL_MAX {
        return Err("it is longer than 2000 characters");
    }
    if !url
        .bytes()
        .all(|b| (0x21..=0x7e).contains(&b) && b != b'\\')
    {
        return Err("it holds a space, a backslash or a character outside printable ASCII");
    }
    if url.contains('#') {
        return Err("it has a fragment (`#`), which is never sent to a server");
    }
    if url.starts_with("//") {
        return Err("it is protocol-relative; write a path or a full `http(s)://` URL");
    }
    let before_slash = url.split(['/', '?']).next().unwrap_or("");
    if before_slash.contains(':') {
        let lower = url.to_ascii_lowercase();
        let Some(rest) = lower
            .strip_prefix("https://")
            .or_else(|| lower.strip_prefix("http://"))
        else {
            return Err("its scheme is not `http` or `https`");
        };
        let authority = rest.split(['/', '?']).next().unwrap_or("");
        if authority.contains('@') {
            return Err("it carries credentials (`user@`); a token goes in `Credential`");
        }
        if authority.is_empty() {
            return Err("it has no host");
        }
    }
    Ok(())
}

fn refused(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// Check that `event` is handled by the component with exactly one parameter, for the `what` it receives.
fn check_event(
    c: &Component,
    s: &Schedule,
    event: &str,
    what: &str,
    written: &str,
) -> Result<(), LowerError> {
    let Some(handler) = c.events.iter().find(|e| e.name == event) else {
        return Err(refused(
            "subscribe-event-unknown",
            format!(
                "component `{}`: `{written}` names an event the component does not handle; add `on {event}(x) {{ .. }}`",
                c.name
            ),
            s.span,
        ));
    };
    if handler.params.len() != 1 {
        return Err(refused(
            "subscribe-event-params",
            format!(
                "component `{}`: `{written}` runs `{event}` with {what}, so it takes exactly one parameter and it takes {}",
                c.name,
                handler.params.len()
            ),
            s.span,
        ));
    }
    Ok(())
}

/// Check one `subscribe` against its component and lower it. `earlier` are the schedules lowered before it.
pub fn lower_subscribe(
    c: &Component,
    s: &Schedule,
    earlier: &[IrSchedule],
) -> Result<IrSchedule, LowerError> {
    let Some(decl) = &s.stream else {
        return Err(refused(
            "subscribe-url-invalid",
            format!(
                "component `{}`: `subscribe {}` names no address; write `subscribe {} \"/api/stream\";`",
                c.name, s.event, s.event
            ),
            s.span,
        ));
    };
    check_event(
        c,
        s,
        &s.event,
        "each message's text",
        &format!("subscribe {}", s.event),
    )?;
    if let Some(status) = &decl.status {
        check_event(
            c,
            s,
            status,
            "the connection's status word",
            &format!("subscribe {} \"..\" status {status}", s.event),
        )?;
    }
    if let Err(why) = url_is_legal(&decl.url) {
        return Err(refused(
            "subscribe-url-invalid",
            format!(
                "component `{}`: `subscribe {} {:?}`: {why}; a stream's address is a path or an `http(s)://` URL (at most {URL_MAX} characters)",
                c.name, s.event, decl.url
            ),
            s.span,
        ));
    }
    if earlier.iter().any(|o| o.kind == IrScheduleKind::Subscribe) {
        return Err(refused(
            "subscribe-duplicate",
            format!(
                "component `{}`: `subscribe` is declared twice; a component listens to one stream",
                c.name
            ),
            s.span,
        ));
    }
    Ok(IrSchedule {
        kind: IrScheduleKind::Subscribe,
        event: s.event.clone(),
        stream: Some(IrStream {
            url: decl.url.clone(),
            status: decl.status.clone(),
        }),
        span: s.span,
    })
}

/// What a stream costs: a long-lived GET of the kind `net.get` is, so the `Network` capability, with the address as the
/// literal the allowed hosts are checked against (docs/streams.md section 3).
pub fn stream_effects(schedules: &[IrSchedule]) -> Vec<IrReachableEffect> {
    schedules
        .iter()
        .filter_map(|s| {
            let stream = s.stream.as_ref()?;
            (s.kind == IrScheduleKind::Subscribe).then(|| IrReachableEffect {
                capability: "Network".to_string(),
                path: vec!["subscribe".to_string()],
                literal_arg: Some(stream.url.clone()),
                span: s.span,
            })
        })
        .collect()
}
