//! cancho-ui-replay — deterministic replay of an event trace over the typed IR.
//!
//! Pure and total: `replay` takes an IR and the text of a trace and returns a
//! report; it cannot fail, does no I/O and has no clock, network or JavaScript
//! runtime. The design, the trace format, what it computes and what it
//! deliberately does not claim are in `docs/replay.md`.

mod eval;
mod machine;
mod report;
mod router;
mod trace;
mod value;

use std::collections::BTreeMap;

use cancho_ui_ir::ir::{IrComponent, IrFile};
use cancho_ui_syntax::span::Span;
use serde_json::Value as Json;

use machine::{Machine, Status, StepEffects};
use trace::{Action, Step, Trace, TraceError, parse_trace};
use value::{Value, num_json, subset};

pub use eval::{
    JSON_ITEMS_MAX, arith, json_get, json_items, json_quote, math_builtin, num_of_text, page_count,
    sort_key_order_json, text_builtin_json,
};
pub use report::{
    Failure, FiredJson, HUMAN_VALUE_BOUND, IssuedJson, NOT_CLAIMED, REPORT_SCHEMA, REPORT_VERSION,
    RequestJson, SpanJson, StepReport, TestReport, TickJson, WriteJson, render_human,
    render_human_full,
};
pub use trace::{SCHEMA as TRACE_SCHEMA, VERSION as TRACE_VERSION};

fn failure(
    rule: &'static str,
    step: Option<usize>,
    path: String,
    message: String,
    file: &str,
    span: Option<Span>,
) -> Failure {
    Failure {
        rule,
        tier: "tested",
        step,
        path,
        message,
        expected: None,
        actual: None,
        line: None,
        column: None,
        span: span.map(|s| SpanJson::new(file, s)),
    }
}

fn from_trace_error(e: TraceError) -> Failure {
    Failure {
        rule: e.rule,
        tier: "tested",
        step: e.step,
        path: e.path,
        message: e.message,
        expected: None,
        actual: None,
        line: e.line,
        column: e.column,
        span: None,
    }
}

fn empty_report(file: &str, trace_name: &str, failures: Vec<Failure>) -> TestReport {
    TestReport {
        schema: REPORT_SCHEMA,
        version: REPORT_VERSION,
        file: file.to_string(),
        trace: trace_name.to_string(),
        component: None,
        result: "fail",
        steps_total: 0,
        steps_run: 0,
        checks: 0,
        failures,
        steps: Vec::new(),
        requests: Vec::new(),
        final_state: BTreeMap::new(),
        not_claimed: NOT_CLAIMED.to_vec(),
    }
}

/// Replay `trace_text` over the IR. `file` and `trace_name` label the report.
pub fn replay(ir: &IrFile, trace_text: &str, file: &str, trace_name: &str) -> TestReport {
    let trace = match parse_trace(trace_text) {
        Ok(t) => t,
        Err(e) => return empty_report(file, trace_name, vec![from_trace_error(e)]),
    };
    let comp = match &trace.component {
        Some(name) => ir.components.iter().find(|c| &c.name == name),
        None => ir.components.last(),
    };
    let Some(comp) = comp else {
        let msg = match &trace.component {
            Some(n) => format!("the file declares no component `{n}`"),
            None => "the file declares no component".to_string(),
        };
        let mut r = empty_report(
            file,
            trace_name,
            vec![failure(
                "trace-unknown-component",
                None,
                "component".into(),
                msg,
                file,
                None,
            )],
        );
        r.steps_total = trace.steps.len();
        return r;
    };
    run_trace(comp, &trace, file, trace_name)
}

/// The name of the `Credential` the component declares, if it does (docs/adapters.md section 18).
fn credential_name(comp: &IrComponent) -> Option<&str> {
    comp.requires
        .iter()
        .find(|r| r.name == "Credential")
        .and_then(|r| r.bounds.first())
        .map(String::as_str)
}

/// Does the component use the URL? Only then do the reports carry it.
fn routes_used(comp: &IrComponent) -> bool {
    comp.requires.iter().any(|r| r.name == "Router")
}

fn state_json(m: &Machine) -> BTreeMap<String, Json> {
    m.state
        .iter()
        .map(|(k, v)| (k.clone(), v.to_json()))
        .collect()
}

fn run_trace(comp: &IrComponent, trace: &Trace, file: &str, trace_name: &str) -> TestReport {
    let mut report = empty_report(file, trace_name, Vec::new());
    report.component = Some(comp.name.clone());
    report.steps_total = trace.steps.len();
    let mut machine = match Machine::new(comp, trace.clock) {
        Ok(m) => m,
        Err(f) => {
            report.failures.push(failure(
                f.rule,
                None,
                String::new(),
                f.message,
                file,
                Some(f.span),
            ));
            return report;
        }
    };
    machine.router = router::Router::new(&trace.url);
    machine.storage = trace.storage.clone();
    machine.storage_quota = trace.storage_quota;
    // docs/adapters.md section 18: a trace lists only credentials the component declares.
    let declared = credential_name(comp);
    for given in &trace.credentials {
        if declared != Some(given.as_str()) {
            report.failures.push(failure(
                "trace-credential-unknown",
                None,
                "credentials".into(),
                match declared {
                    Some(d) => format!(
                        "the trace lists the credential `{given}`, but component `{}` declares `{d}`",
                        comp.name
                    ),
                    None => format!(
                        "the trace lists the credential `{given}`, but component `{}` declares no `Credential`",
                        comp.name
                    ),
                },
                file,
                None,
            ));
            return report;
        }
    }
    machine.credentials = trace.credentials.clone();
    for (i, step) in trace.steps.iter().enumerate() {
        match run_step(&mut machine, comp, step, i, file, &mut report) {
            Ok(()) => report.steps_run += 1,
            Err(f) => {
                report.failures.push(*f);
                break;
            }
        }
    }
    report.requests = machine
        .requests
        .iter()
        .map(|r| RequestJson {
            request: r.id,
            effect: r.effect.name(),
            args: r.args.iter().map(Value::to_json).collect(),
            due: r.due.map(num_json),
            status: if r.status == Status::Pending {
                "pending"
            } else {
                "resolved"
            },
            superseded: r.superseded,
            disposed: r.disposed,
        })
        .collect();
    report.final_state = state_json(&machine);
    report.result = if report.failures.is_empty() {
        "pass"
    } else {
        "fail"
    };
    report
}

/// Run one step. `Err` is a failure that stops the replay (a trace error or a
/// runtime refusal); expectation failures are pushed to the report and the
/// replay goes on.
fn run_step(
    m: &mut Machine,
    comp: &IrComponent,
    step: &Step,
    i: usize,
    file: &str,
    report: &mut TestReport,
) -> Result<(), Box<Failure>> {
    let at = format!("steps[{i}]");
    let stop = |rule: &'static str, key: &str, message: String| {
        Box::new(failure(
            rule,
            Some(i),
            format!("{at}.{key}"),
            message,
            file,
            None,
        ))
    };
    // Expectations name real state fields, whatever the action is.
    for (field, _) in &step.state {
        if !m.state.contains_key(field) {
            return Err(stop(
                "trace-unknown-state-field",
                &format!("state.{field}"),
                format!(
                    "component `{}` declares no state field `{field}`",
                    comp.name
                ),
            ));
        }
    }
    let mut fx = StepEffects::default();
    // The component has mounted: its `start` events run before the first step, and are reported with it.
    if i == 0
        && let Err(f) = m.start(&mut fx)
    {
        return Err(Box::new(failure(
            f.rule,
            Some(i),
            at,
            f.message,
            file,
            Some(f.span),
        )));
    }
    let (action, detail, outcome) = match &step.action {
        Action::Event { name, params } => {
            if m.disposed {
                return Err(stop(
                    "trace-event-after-dispose",
                    "event",
                    format!("event `{name}` after `dispose`: no mounted view is left to fire it"),
                ));
            }
            let Some(h) = m.handler_index(name) else {
                return Err(stop(
                    "trace-unknown-event",
                    "event",
                    format!("component `{}` has no `on {name}` handler", comp.name),
                ));
            };
            if m.handler_params(h) != params.len() {
                return Err(stop(
                    "trace-params-arity",
                    "params",
                    format!(
                        "`on {name}` takes {} parameter(s), the step gives {}",
                        m.handler_params(h),
                        params.len()
                    ),
                ));
            }
            // A field with a `maxlength:` never reports a longer value: the browser stops the typing and the pasting. A trace
            // that gives one describes a page that does not exist (docs/controls.md section 6).
            if let (Some(Value::Str(typed)), Some(limit)) = (params.first(), m.input_limit(name))
                && typed.encode_utf16().count() > limit
            {
                return Err(stop(
                    "trace-value-too-long",
                    "params",
                    format!(
                        "`{name}` is fired by a field that allows {limit} characters (UTF-16 code units) and the step gives {}",
                        typed.encode_utf16().count()
                    ),
                ));
            }
            // A select reports one of its options, never anything else; a trace that gives another describes a page that does
            // not exist (docs/controls.md section 10).
            if let (Some(Value::Str(chosen)), Some(options)) =
                (params.first(), m.select_options(name))
                && !options.contains(chosen)
            {
                return Err(stop(
                    "trace-value-not-an-option",
                    "params",
                    format!(
                        "`{name}` is fired only by a Select, and {chosen:?} is not one of its options ({})",
                        options
                            .iter()
                            .map(|o| format!("{o:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            let detail = serde_json::json!({
                "name": name,
                "params": params.iter().map(Value::to_json).collect::<Vec<_>>(),
            });
            let out = m.dispatch(h, params.clone(), &mut fx);
            ("event", detail, out)
        }
        Action::Resolve { request, answer } => {
            let pending = u32::try_from(*request)
                .ok()
                .and_then(|id| m.request(id).map(|r| (id, r.status.clone(), r.effect)));
            let Some((id, status, effect)) = pending else {
                return Err(stop(
                    "trace-unknown-request",
                    "resolve",
                    format!(
                        "request #{request} was never started (started so far: {})",
                        m.requests.len()
                    ),
                ));
            };
            if status != Status::Pending {
                return Err(stop(
                    "trace-request-not-pending",
                    "resolve",
                    format!("request #{id} has already resolved"),
                ));
            }
            if !matches!(
                effect,
                value::Effect::NetSearch
                    | value::Effect::NetGet
                    | value::Effect::NetFetch
                    | value::Effect::NetPost
                    | value::Effect::NetPut
                    | value::Effect::NetDelete
                    | value::Effect::LocationCurrent
            ) {
                return Err(stop(
                    "trace-request-not-scriptable",
                    "resolve",
                    format!(
                        "request #{id} is a `{}`; timers resolve by advancing the clock, only `net.search`, `net.get`, `net.fetch`, `net.post`, `net.put`, `net.delete` and `location.current` are scripted",
                        effect.name()
                    ),
                ));
            }
            let out = m.resolve(id, answer.clone(), &mut fx);
            ("resolve", Json::from(id), out)
        }
        Action::Advance(ms) => {
            let out = m.advance(*ms, &mut fx);
            ("advance", num_json(*ms), out)
        }
        Action::Navigate(path) => {
            if m.disposed {
                return Err(stop(
                    "trace-event-after-dispose",
                    "navigate",
                    "`navigate` after `dispose`: no mounted page is left to hear it".to_string(),
                ));
            }
            (
                "navigate",
                Json::String(path.clone()),
                m.navigate(path, &mut fx),
            )
        }
        Action::Back | Action::Forward => {
            let forward = matches!(step.action, Action::Forward);
            let key = if forward { "forward" } else { "back" };
            if m.disposed {
                return Err(stop(
                    "trace-event-after-dispose",
                    key,
                    format!("`{key}` after `dispose`: no mounted page is left to hear it"),
                ));
            }
            let possible = if forward {
                m.router.can_forward()
            } else {
                m.router.can_back()
            };
            if !possible {
                return Err(stop(
                    "trace-history-empty",
                    key,
                    format!(
                        "the history has no entry to go {} to: the page is at entry {} of {}",
                        if forward { "forward" } else { "back" },
                        m.router.position + 1,
                        m.router.entries.len()
                    ),
                ));
            }
            (key, Json::Null, m.travel(forward, &mut fx))
        }
        Action::Dispose => ("dispose", Json::Null, Ok(m.dispose())),
        Action::Hide => {
            if !m.visible {
                return Err(stop(
                    "trace-bad-step",
                    "hide",
                    "the page is already hidden; the browser fires `visibilitychange` only when the state changes".to_string(),
                ));
            }
            ("hide", Json::Null, Ok(m.hide()))
        }
        Action::Show => {
            if m.visible {
                return Err(stop(
                    "trace-bad-step",
                    "show",
                    "the page is already visible; the browser fires `visibilitychange` only when the state changes".to_string(),
                ));
            }
            ("show", Json::Null, m.show(&mut fx))
        }
    };
    let outcome = match (outcome, &step.fault) {
        (Ok(o), None) => o,
        // The step said it would fault and did not: the test fails here, and the replay goes on (the step completed).
        (Ok(o), Some(rule)) => {
            report.failures.push(failure(
                "test-fault-missing",
                Some(i),
                format!("{at}.fault"),
                format!("step {i} was expected to fault with `{rule}` and completed"),
                file,
                fx.handler_span,
            ));
            o
        }
        // The expected fault: the machine has undone the segment's writes, and the replay goes on from there.
        (Err(f), Some(rule)) if f.rule == rule => machine::Outcome::Faulted,
        (Err(f), Some(rule)) => {
            let mut fl = failure(
                "test-fault-mismatch",
                Some(i),
                format!("{at}.fault"),
                format!(
                    "step {i} was expected to fault with `{rule}` and faulted with `{}`: {}",
                    f.rule, f.message
                ),
                file,
                Some(f.span),
            );
            fl.expected = Some(Json::String(rule.clone()));
            fl.actual = Some(Json::String(f.rule.to_string()));
            return Err(Box::new(fl));
        }
        (Err(f), None) => {
            return Err(Box::new(failure(
                f.rule,
                Some(i),
                at,
                f.message,
                file,
                Some(f.span),
            )));
        }
    };
    let step_report = StepReport {
        step: i,
        action,
        detail,
        outcome: outcome.as_str(),
        state: state_json(m),
        issued: fx
            .issued
            .iter()
            .filter_map(|id| m.request(*id))
            .map(|r| IssuedJson {
                request: r.id,
                effect: r.effect.name(),
                args: r.args.iter().map(Value::to_json).collect(),
                due: r.due.map(num_json),
                credential: match (credential_name(comp), r.effect) {
                    (
                        Some(name),
                        value::Effect::NetGet
                        | value::Effect::NetFetch
                        | value::Effect::NetPost
                        | value::Effect::NetPut
                        | value::Effect::NetDelete,
                    ) => Some(if m.credentials.iter().any(|c| c == name) {
                        name.to_string()
                    } else {
                        format!("{name} (not supplied)")
                    }),
                    _ => None,
                },
            })
            .collect(),
        pending: m.pending(),
        fired: fx
            .fired
            .iter()
            .map(|f| FiredJson {
                request: f.request,
                delivered: f.delivered.as_str(),
                outcome: f.outcome.as_str(),
            })
            .collect(),
        ticks: fx
            .ticks
            .iter()
            .map(|t| TickJson {
                event: t.event.clone(),
                at: num_json(t.at),
                ran: t.ran,
                catch_up: t.catch_up,
            })
            .collect(),
        writes: fx
            .writes
            .iter()
            .map(|w| WriteJson {
                field: w.field.clone(),
                span: SpanJson::new(file, w.span),
            })
            .collect(),
        clock: num_json(m.clock),
        storage: m.storage.clone(),
        routes: std::mem::take(&mut m.routes),
        url: routes_used(comp).then(|| m.router.current().to_string()),
        history: routes_used(comp).then(|| {
            serde_json::json!({
                "entries": m.router.entries,
                "position": m.router.position,
            })
        }),
    };
    check_expectations(m, comp, step, &step_report, &fx, i, file, report);
    report.steps.push(step_report);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_expectations(
    m: &Machine,
    comp: &IrComponent,
    step: &Step,
    sr: &StepReport,
    fx: &StepEffects,
    i: usize,
    file: &str,
    report: &mut TestReport,
) {
    for (field, expected) in &step.state {
        report.checks += 1;
        let actual = &m.state[field];
        if actual != expected {
            // The assignment that last wrote the field in this step; else the
            // field's declaration.
            let span = fx
                .writes
                .iter()
                .rev()
                .find(|w| &w.field == field)
                .map(|w| w.span)
                .or_else(|| comp.state.iter().find(|s| &s.name == field).map(|s| s.span));
            let mut f = failure(
                "test-state-mismatch",
                Some(i),
                format!("steps[{i}].state.{field}"),
                format!("after step {i}, `{field}` is not what the trace expects"),
                file,
                span,
            );
            f.expected = Some(expected.to_json());
            f.actual = Some(actual.to_json());
            report.failures.push(f);
        }
    }
    let actual_step = serde_json::to_value(sr).unwrap_or(Json::Null);
    for (key, expected) in &step.expect {
        report.checks += 1;
        // `ticks` and `routes` are absent from a step in which none fell due or none was heard: that is the empty list.
        let absent = if key == "ticks" || key == "routes" {
            Json::Array(Vec::new())
        } else {
            Json::Null
        };
        let actual = actual_step.get(key).cloned().unwrap_or(absent);
        if !subset(expected, &actual) {
            let mut f = failure(
                "test-expect-mismatch",
                Some(i),
                format!("steps[{i}].expect.{key}"),
                format!("in step {i}, `{key}` is not what the trace expects"),
                file,
                fx.handler_span,
            );
            f.expected = Some(expected.clone());
            f.actual = Some(actual);
            report.failures.push(f);
        }
    }
}
