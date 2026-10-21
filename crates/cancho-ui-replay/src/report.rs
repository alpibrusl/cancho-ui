//! The report: its types, JSON shape and the human rendering
//! (docs/replay.md §7).

use std::collections::BTreeMap;

use cancho_ui_syntax::span::Span;
use serde::Serialize;
use serde_json::Value as Json;

pub const REPORT_SCHEMA: &str = "cancho-ui-test";
pub const REPORT_VERSION: &str = "0.1.0";

/// Printed in every report, so a `pass` is never read as a claim about these
/// (docs/replay.md §9).
pub const NOT_CLAIMED: &[&str] = &[
    "emitted_javascript_behaviour",
    "view_rendering",
    "timing_and_latency",
    "real_network",
    "javascript_event_loop",
    "child_component_instances",
    "cross_component_ordering",
    "properties",
    "foreign_boundaries",
    "exhaustiveness",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanJson {
    pub file: String,
    pub start: PosJson,
    pub end: PosJson,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PosJson {
    pub line: u32,
    pub column: u32,
}

impl SpanJson {
    pub fn new(file: &str, s: Span) -> SpanJson {
        SpanJson {
            file: file.to_string(),
            start: PosJson {
                line: s.start.line,
                column: s.start.column,
            },
            end: PosJson {
                line: s.end.line,
                column: s.end.column,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Failure {
    pub rule: &'static str,
    pub tier: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<usize>,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Json>,
    /// A JSON syntax error's position in the trace file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// The source that produced the actual value, where one is responsible.
    pub span: Option<SpanJson>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IssuedJson {
    pub request: u32,
    pub effect: &'static str,
    pub args: Vec<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<Json>,
    /// The credential the request carries, by name (never its value), when its component declares one;
    /// `name (not supplied)` when the trace does not list it (docs/adapters.md section 18).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FiredJson {
    pub request: u32,
    pub delivered: &'static str,
    pub outcome: &'static str,
}

/// An `every` tick that fell due in a step (docs/schedules.md section 6).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TickJson {
    pub event: String,
    pub at: Json,
    pub ran: bool,
    /// Present (and true) on the tick that ran when the page became visible again.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub catch_up: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WriteJson {
    pub field: String,
    pub span: SpanJson,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepReport {
    pub step: usize,
    pub action: &'static str,
    /// The event name, or the request id, or the milliseconds.
    pub detail: Json,
    pub outcome: &'static str,
    pub state: BTreeMap<String, Json>,
    pub issued: Vec<IssuedJson>,
    pub pending: Vec<u32>,
    pub fired: Vec<FiredJson>,
    /// The `every` ticks that fell due; absent when none did.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ticks: Vec<TickJson>,
    pub writes: Vec<WriteJson>,
    pub clock: Json,
    /// The `Storage` store after the step; absent when the trace never seeds or writes it.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub storage: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RequestJson {
    pub request: u32,
    pub effect: &'static str,
    pub args: Vec<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<Json>,
    /// `pending`, `resolved`; plus `superseded` / `disposed` flags.
    pub status: &'static str,
    pub superseded: bool,
    pub disposed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestReport {
    pub schema: &'static str,
    pub version: &'static str,
    pub file: String,
    pub trace: String,
    pub component: Option<String>,
    pub result: &'static str,
    pub steps_total: usize,
    pub steps_run: usize,
    pub checks: usize,
    pub failures: Vec<Failure>,
    pub steps: Vec<StepReport>,
    pub requests: Vec<RequestJson>,
    pub final_state: BTreeMap<String, Json>,
    pub not_claimed: Vec<&'static str>,
}

impl TestReport {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

fn compact(j: &Json) -> String {
    serde_json::to_string(j).unwrap_or_else(|_| "?".into())
}

/// The line-per-step form (docs/replay.md §7).
pub fn render_human(r: &TestReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("cancho-ui test {} {}\n", r.file, r.trace));
    match &r.component {
        Some(c) => out.push_str(&format!(
            "component {c}, {} step{}\n",
            r.steps_total,
            if r.steps_total == 1 { "" } else { "s" }
        )),
        None => out.push_str("no component replayed\n"),
    }
    for s in &r.steps {
        let desc = match s.action {
            "event" => {
                let params: Vec<String> = s
                    .detail
                    .get("params")
                    .and_then(Json::as_array)
                    .map(|a| a.iter().map(compact).collect())
                    .unwrap_or_default();
                format!(
                    "event {}({})",
                    s.detail.get("name").and_then(Json::as_str).unwrap_or("?"),
                    params.join(", ")
                )
            }
            "resolve" => format!("resolve #{}", compact(&s.detail)),
            "advance" => format!(
                "advance {}ms -> clock {}",
                compact(&s.detail),
                compact(&s.clock)
            ),
            "hide" => "hide".to_string(),
            "show" => "show".to_string(),
            _ => "dispose".to_string(),
        };
        let mut detail: Vec<String> = Vec::new();
        for w in &s.writes {
            if let Some(v) = s.state.get(&w.field) {
                detail.push(format!("{}={}", w.field, compact(v)));
            }
        }
        for i in &s.issued {
            let args: Vec<String> = i.args.iter().map(compact).collect();
            detail.push(match &i.credential {
                Some(c) => format!(
                    "issued #{} {}({}) [credential {c}]",
                    i.request,
                    i.effect,
                    args.join(", ")
                ),
                None => format!("issued #{} {}({})", i.request, i.effect, args.join(", ")),
            });
        }
        for f in &s.fired {
            detail.push(format!("#{} -> {}", f.request, f.delivered));
        }
        out.push_str(&format!(
            "  step {:<2} {:<34} {:<10} {}\n",
            s.step,
            desc,
            s.outcome,
            detail.join("  ")
        ));
    }
    if r.passed() {
        out.push_str(&format!(
            "PASS  {} step{}, {} check{}\n",
            r.steps_run,
            if r.steps_run == 1 { "" } else { "s" },
            r.checks,
            if r.checks == 1 { "" } else { "s" }
        ));
    } else {
        for f in &r.failures {
            out.push_str(&format!("FAIL  {}", f.rule));
            if let Some(n) = f.step {
                out.push_str(&format!(" at step {n}"));
            }
            out.push_str(&format!(
                " ({})\n",
                if f.path.is_empty() { "trace" } else { &f.path }
            ));
            out.push_str(&format!("      {}\n", f.message));
            if let (Some(e), Some(a)) = (&f.expected, &f.actual) {
                out.push_str(&format!(
                    "      expected {}\n      actual   {}\n",
                    compact(e),
                    compact(a)
                ));
            }
            if let (Some(l), Some(c)) = (f.line, f.column) {
                out.push_str(&format!("      at trace {l}:{c}\n"));
            }
            if let Some(sp) = &f.span {
                out.push_str(&format!(
                    "      source {}:{}:{}\n",
                    sp.file, sp.start.line, sp.start.column
                ));
            }
        }
        out.push_str(&format!(
            "FAIL  {} failure{}; {} of {} steps run, {} checks\n",
            r.failures.len(),
            if r.failures.len() == 1 { "" } else { "s" },
            r.steps_run,
            r.steps_total,
            r.checks
        ));
    }
    out
}
