//! Reading a trace (docs/replay.md §2). The whole file is validated before
//! anything runs, so a malformed trace is refused as a value, with the JSON path
//! of the offending key; it never reaches the interpreter.

use serde_json::{Map, Value as Json};

use crate::value::Value;

pub const SCHEMA: &str = "cancho-ui-trace";
pub const VERSION: &str = "0.1.0";

/// A trace the format does not allow. `path` is a JSON path (`steps[3].event`);
/// a JSON syntax error carries the parser's line and column instead.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceError {
    pub rule: &'static str,
    pub path: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    /// The step the error is in, when it is in one.
    pub step: Option<usize>,
}

fn err(rule: &'static str, path: impl Into<String>, message: impl Into<String>) -> TraceError {
    TraceError {
        rule,
        path: path.into(),
        message: message.into(),
        line: None,
        column: None,
        step: None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Event {
        name: String,
        params: Vec<Value>,
    },
    /// `answer` is an `Ok(..)` or `Err(..)` value.
    Resolve {
        request: u64,
        answer: Value,
    },
    Advance(f64),
    Dispose,
    /// The page becomes hidden / visible again (docs/schedules.md section 11).
    Hide,
    Show,
    /// The user follows a link or types a URL: the fragment becomes this path (docs/routing.md section 5).
    Navigate(String),
    /// The browser's Back and Forward buttons.
    Back,
    Forward,
    /// The server answers the stream the page is waiting for (docs/streams.md section 4).
    Stream(StreamAct),
    /// A message arrives on the open stream; `id` is the event id the server sent with it, if any.
    Message {
        text: String,
        id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamAct {
    /// The connection is accepted as `text/event-stream`.
    Open,
    /// The connection ends (or could not be made): the page waits, then connects again.
    Drop,
    /// The answer is not a stream: a status other than 200, another content type, a redirect.
    Refuse,
}

impl StreamAct {
    pub fn as_str(self) -> &'static str {
        match self {
            StreamAct::Open => "open",
            StreamAct::Drop => "drop",
            StreamAct::Refuse => "refuse",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub action: Action,
    /// `state` expectations in key order: (field, expected value).
    pub state: Vec<(String, Value)>,
    /// `expect` expectations: (key, expected JSON).
    pub expect: Vec<(String, Json)>,
    /// `"fault": "replay-.."`: the step is expected to fault with this rule, and the trace goes on after it
    /// (docs/replay.md, "A step that faults").
    pub fault: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub component: Option<String>,
    pub clock: f64,
    /// The `Storage` store at the start: key to value (docs/adapters.md).
    pub storage: std::collections::BTreeMap<String, String>,
    /// The most UTF-16 units the persisted store may hold, keys (with the adapter's `cui:`) and values together; `None`
    /// is no limit. A `storage.set` that would pass it fails as a full `localStorage` does (docs/adapters.md section 20).
    pub storage_quota: Option<u64>,
    /// The names of the credentials the page supplied at mount (docs/adapters.md section 18); never their values.
    pub credentials: Vec<String>,
    /// The URL fragment the page loaded at, as a path (docs/routing.md section 5); `/` when the trace does not say.
    pub url: String,
    /// The browser's languages in order of preference, as `navigator.languages` (docs/i18n.md section 3); empty when the trace does not say.
    pub locales: Vec<String>,
    pub steps: Vec<Step>,
}

const EXPECT_KEYS: &[&str] = &[
    "outcome",
    "issued",
    "pending",
    "fired",
    "ticks",
    "storage",
    "url",
    "history",
    "routes",
    "credentials",
    "charts",
    "stream",
    "document",
    "sortlists",
];

pub fn parse_trace(text: &str) -> Result<Trace, TraceError> {
    let root: Json = serde_json::from_str(text).map_err(|e| TraceError {
        rule: "trace-malformed",
        path: String::new(),
        message: format!("not valid JSON: {e}"),
        line: Some(e.line()),
        column: Some(e.column()),
        step: None,
    })?;
    let obj = root.as_object().ok_or_else(|| {
        err(
            "trace-malformed",
            "",
            "a trace is a JSON object with `schema`, `version` and `steps`",
        )
    })?;
    allow_keys(
        obj,
        &[
            "schema",
            "version",
            "component",
            "clock",
            "storage",
            "storage_quota",
            "credentials",
            "url",
            "locales",
            "steps",
            "note",
        ],
        "",
    )?;
    match obj.get("schema").and_then(Json::as_str) {
        Some(SCHEMA) => {}
        _ => {
            return Err(err(
                "trace-schema-unsupported",
                "schema",
                format!("`schema` must be the string \"{SCHEMA}\""),
            ));
        }
    }
    match obj.get("version").and_then(Json::as_str) {
        Some(VERSION) => {}
        other => {
            return Err(err(
                "trace-schema-unsupported",
                "version",
                format!("this build reads trace version \"{VERSION}\", the trace says {other:?}"),
            ));
        }
    }
    let component = match obj.get("component") {
        None => None,
        Some(Json::String(s)) => Some(s.clone()),
        Some(_) => {
            return Err(err(
                "trace-malformed",
                "component",
                "`component` is a string",
            ));
        }
    };
    let clock = match obj.get("clock") {
        None => 0.0,
        Some(j) => non_negative(j, "clock")?,
    };
    let mut storage = std::collections::BTreeMap::new();
    match obj.get("storage") {
        None => {}
        Some(Json::Object(m)) => {
            for (k, v) in m {
                match v {
                    Json::String(s) => {
                        storage.insert(k.clone(), s.clone());
                    }
                    _ => {
                        return Err(err(
                            "trace-malformed",
                            format!("storage.{k}"),
                            "a stored value is a string",
                        ));
                    }
                }
            }
        }
        Some(_) => {
            return Err(err(
                "trace-malformed",
                "storage",
                "`storage` is an object of strings",
            ));
        }
    }
    let storage_quota = match obj.get("storage_quota") {
        None => None,
        Some(j) => match j.as_u64() {
            Some(n) => Some(n),
            None => {
                return Err(err(
                    "trace-bad-value",
                    "storage_quota",
                    "expected a non-negative integer: the most UTF-16 units the store may hold",
                ));
            }
        },
    };
    let mut credentials: Vec<String> = Vec::new();
    match obj.get("credentials") {
        None => {}
        Some(Json::Array(a)) => {
            for (i, v) in a.iter().enumerate() {
                match v {
                    Json::String(s) => credentials.push(s.clone()),
                    _ => {
                        return Err(err(
                            "trace-malformed",
                            format!("credentials[{i}]"),
                            "a credential is listed by its name, a string; its value is never in a trace",
                        ));
                    }
                }
            }
        }
        Some(_) => {
            return Err(err(
                "trace-malformed",
                "credentials",
                "`credentials` is a list of the names of the credentials the page supplied",
            ));
        }
    }
    let url = match obj.get("url") {
        None => "/".to_string(),
        Some(Json::String(p)) if cancho_ui_ir::routing::path_is_legal(p) => p.clone(),
        Some(_) => {
            return Err(err(
                "trace-navigate-invalid",
                "url",
                "`url` is the path the page loaded at: it starts with `/` and holds no space, `#`, `\"`, `<`, `>`, backtick or character outside printable ASCII",
            ));
        }
    };
    let mut locales: Vec<String> = Vec::new();
    match obj.get("locales") {
        None => {}
        Some(Json::Array(a)) => {
            for (i, v) in a.iter().enumerate() {
                match v {
                    Json::String(s) if locale_tag_is_legal(s) => locales.push(s.clone()),
                    _ => {
                        return Err(err(
                            "trace-locale-invalid",
                            format!("locales[{i}]"),
                            "a language is a tag like `es` or `es-MX` (2 or 3 letters, then parts of 1 to 8 letters or digits after `-` or `_`)",
                        ));
                    }
                }
            }
        }
        Some(_) => {
            return Err(err(
                "trace-locale-invalid",
                "locales",
                "`locales` is the list of languages the browser prefers, like `navigator.languages`",
            ));
        }
    }
    let steps_json = match obj.get("steps") {
        Some(Json::Array(a)) if !a.is_empty() => a,
        Some(Json::Array(_)) => {
            return Err(err(
                "trace-malformed",
                "steps",
                "`steps` is empty; a trace has at least one step",
            ));
        }
        _ => {
            return Err(err(
                "trace-malformed",
                "steps",
                "`steps` is a list of steps",
            ));
        }
    };
    let mut steps = Vec::with_capacity(steps_json.len());
    for (i, s) in steps_json.iter().enumerate() {
        steps.push(parse_step(s, i).map_err(|mut e| {
            e.step = Some(i);
            e
        })?);
    }
    // A step that names the rule it faults with asserts something too.
    let assertions: usize = steps
        .iter()
        .map(|s| s.state.len() + s.expect.len() + usize::from(s.fault.is_some()))
        .sum();
    if assertions == 0 {
        return Err(err(
            "trace-no-assertions",
            "steps",
            "no step has a `state` or an `expect`, so this trace can never fail and proves nothing",
        ));
    }
    Ok(Trace {
        component,
        clock,
        storage_quota,
        storage,
        credentials,
        url,
        locales,
        steps,
    })
}

fn locale_tag_is_legal(s: &str) -> bool {
    let mut parts = s.split(['-', '_']);
    let first = parts.next().unwrap_or("");
    (2..=3).contains(&first.len())
        && first.chars().all(|c| c.is_ascii_alphabetic())
        && parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn allow_keys(obj: &Map<String, Json>, allowed: &[&str], at: &str) -> Result<(), TraceError> {
    for k in obj.keys() {
        if !allowed.contains(&k.as_str()) {
            let path = if at.is_empty() {
                k.clone()
            } else {
                format!("{at}.{k}")
            };
            return Err(err(
                "trace-unknown-key",
                path,
                format!(
                    "`{k}` is not part of the trace format (allowed here: {})",
                    allowed.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

fn non_negative(j: &Json, path: &str) -> Result<f64, TraceError> {
    match j.as_f64() {
        Some(n) if n.is_finite() && n >= 0.0 => Ok(n),
        _ => Err(err(
            "trace-bad-value",
            path,
            "expected a finite, non-negative number of milliseconds",
        )),
    }
}

fn value_at(j: &Json, path: &str) -> Result<Value, TraceError> {
    Value::from_json(j).map_err(|why| err("trace-bad-value", path, why))
}

fn parse_step(j: &Json, i: usize) -> Result<Step, TraceError> {
    let at = format!("steps[{i}]");
    let obj = j
        .as_object()
        .ok_or_else(|| err("trace-malformed", &at, "a step is a JSON object"))?;
    allow_keys(
        obj,
        &[
            "event", "params", "resolve", "with", "advance", "dispose", "hide", "show", "navigate",
            "back", "forward", "stream", "message", "id", "state", "expect", "note", "fault",
        ],
        &at,
    )?;
    let actions: Vec<&str> = [
        "event", "resolve", "advance", "dispose", "hide", "show", "navigate", "back", "forward",
        "stream", "message",
    ]
    .into_iter()
    .filter(|k| obj.contains_key(*k))
    .collect();
    if actions.len() != 1 {
        return Err(err(
            "trace-bad-step",
            &at,
            format!(
                "a step has exactly one of `event`, `resolve`, `advance`, `dispose`, `hide`, `show`, `navigate`, `back`, `forward`, `stream`, `message`; this one has {}",
                actions.len()
            ),
        ));
    }
    // `params` belongs to `event`, `with` to `resolve`.
    if obj.contains_key("params") && actions[0] != "event" {
        return Err(err(
            "trace-bad-step",
            format!("{at}.params"),
            "`params` goes with `event`",
        ));
    }
    if obj.contains_key("with") && actions[0] != "resolve" {
        return Err(err(
            "trace-bad-step",
            format!("{at}.with"),
            "`with` goes with `resolve`",
        ));
    }
    if obj.contains_key("id") && actions[0] != "message" {
        return Err(err(
            "trace-bad-step",
            format!("{at}.id"),
            "`id` goes with `message`",
        ));
    }
    let action = match actions[0] {
        "stream" => match obj["stream"].as_str() {
            Some("open") => Action::Stream(StreamAct::Open),
            Some("drop") => Action::Stream(StreamAct::Drop),
            Some("refuse") => Action::Stream(StreamAct::Refuse),
            _ => {
                return Err(err(
                    "trace-bad-step",
                    format!("{at}.stream"),
                    "`stream` is what the server does: \"open\", \"drop\" or \"refuse\"",
                ));
            }
        },
        "message" => {
            let text = obj["message"].as_str().ok_or_else(|| {
                err(
                    "trace-bad-step",
                    format!("{at}.message"),
                    "`message` is the text of the message (a string)",
                )
            })?;
            let id = match obj.get("id") {
                None => None,
                Some(Json::String(i)) if !i.contains(['\0', '\n', '\r']) => Some(i.clone()),
                Some(_) => {
                    return Err(err(
                        "trace-bad-value",
                        format!("{at}.id"),
                        "`id` is a string with no NUL, newline or carriage return, as the event stream format allows",
                    ));
                }
            };
            Action::Message {
                text: text.to_string(),
                id,
            }
        }
        "event" => {
            let name = obj["event"].as_str().ok_or_else(|| {
                err(
                    "trace-bad-step",
                    format!("{at}.event"),
                    "`event` is an event name (a string)",
                )
            })?;
            let params = match obj.get("params") {
                None => Vec::new(),
                Some(Json::Array(a)) => a
                    .iter()
                    .enumerate()
                    .map(|(k, p)| value_at(p, &format!("{at}.params[{k}]")))
                    .collect::<Result<Vec<_>, _>>()?,
                Some(_) => {
                    return Err(err(
                        "trace-bad-step",
                        format!("{at}.params"),
                        "`params` is a list",
                    ));
                }
            };
            Action::Event {
                name: name.to_string(),
                params,
            }
        }
        "resolve" => {
            let request = obj["resolve"].as_u64().filter(|n| *n >= 1).ok_or_else(|| {
                err(
                    "trace-bad-step",
                    format!("{at}.resolve"),
                    "`resolve` is a request id (1, 2, 3, ... in the order started)",
                )
            })?;
            let with = obj.get("with").ok_or_else(|| {
                err(
                    "trace-bad-response",
                    format!("{at}.with"),
                    "`resolve` needs `with`: {\"Ok\": v} or {\"Err\": v}",
                )
            })?;
            let answer = value_at(with, &format!("{at}.with"))?;
            if !matches!(answer, Value::Ok(_) | Value::Err(_)) {
                return Err(err(
                    "trace-bad-response",
                    format!("{at}.with"),
                    "a scripted response is {\"Ok\": v} or {\"Err\": v}; `Cancelled` comes only from disposal",
                ));
            }
            Action::Resolve { request, answer }
        }
        "advance" => Action::Advance(non_negative(&obj["advance"], &format!("{at}.advance"))?),
        "navigate" => match &obj["navigate"] {
            Json::String(p) if cancho_ui_ir::routing::path_is_legal(p) => {
                Action::Navigate(p.clone())
            }
            _ => {
                return Err(err(
                    "trace-navigate-invalid",
                    format!("{at}.navigate"),
                    "`navigate` is a path: it starts with `/` and holds no space, `#`, `\"`, `<`, `>`, backtick or character outside printable ASCII (a fragment that is not a route, like `#main`, is not something the page's route event hears)",
                ));
            }
        },
        "back" | "forward" => {
            let key = actions[0];
            if obj[key] != Json::Bool(true) {
                return Err(err(
                    "trace-bad-step",
                    format!("{at}.{key}"),
                    format!("`{key}` is `true`"),
                ));
            }
            if key == "back" {
                Action::Back
            } else {
                Action::Forward
            }
        }
        "hide" | "show" => {
            let key = actions[0];
            if obj[key] != Json::Bool(true) {
                return Err(err(
                    "trace-bad-step",
                    format!("{at}.{key}"),
                    format!("`{key}` is `true`"),
                ));
            }
            if key == "hide" {
                Action::Hide
            } else {
                Action::Show
            }
        }
        _ => {
            if obj["dispose"] != Json::Bool(true) {
                return Err(err(
                    "trace-bad-step",
                    format!("{at}.dispose"),
                    "`dispose` is `true`",
                ));
            }
            Action::Dispose
        }
    };
    let fault = match obj.get("fault") {
        None => None,
        Some(Json::String(rule)) if rule.starts_with("replay-") => Some(rule.clone()),
        Some(_) => {
            return Err(err(
                "trace-bad-step",
                format!("{at}.fault"),
                "`fault` is the rule the step is expected to fault with, a string starting `replay-` (for example `\"replay-non-finite\"`)",
            ));
        }
    };
    let mut state = Vec::new();
    if let Some(s) = obj.get("state") {
        let m = s.as_object().ok_or_else(|| {
            err(
                "trace-malformed",
                format!("{at}.state"),
                "`state` is an object of field -> value",
            )
        })?;
        for (k, v) in m {
            state.push((k.clone(), value_at(v, &format!("{at}.state.{k}"))?));
        }
    }
    let mut expect = Vec::new();
    if let Some(e) = obj.get("expect") {
        let m = e.as_object().ok_or_else(|| {
            err(
                "trace-malformed",
                format!("{at}.expect"),
                "`expect` is an object",
            )
        })?;
        allow_keys(m, EXPECT_KEYS, &format!("{at}.expect"))?;
        for (k, v) in m {
            expect.push((k.clone(), v.clone()));
        }
    }
    Ok(Step {
        action,
        state,
        expect,
        fault,
    })
}
