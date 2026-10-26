//! `Waffle` (docs/waffle.md): a grid of squares, one for each of `of:` things, `counted:` of them counted. The cells are written in the page's HTML as three
//! `<path>`s (counted, known, open), not a `<rect>` each: measured, 20 kB for the 1,099 cells of the cancho-www proof section against 70 kB and 1,103
//! elements. Everything is a literal, so the element is always static: it is in the prerendered HTML, with its text alternative and its table.

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;
use crate::media::Dom;

/// The most cells: at 20 bytes each, 100 kB of HTML.
pub const OF_MAX: i64 = 5000;
/// The most columns.
pub const COLUMNS_MAX: i64 = 100;
/// What the cells that are not counted are: `known` (outlined, a difference somebody listed) or `open` (hollow, not yet).
pub const RESTS: &[&str] = &["known", "open"];
/// The keys of a `Waffle` beside `label:`.
pub const KEYS: &[&str] = &["of", "counted", "columns", "rest"];
const CELL: i64 = 14;
const GAP: i64 = 3;

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

fn int_of(b: &IrBinding) -> Option<i64> {
    match &b.value {
        IrBindingValue::Const { expr, .. } => match **expr {
            IrExpr::IntLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    }
}

fn find<'a>(bindings: &'a [IrBinding], key: &str) -> Option<&'a IrBinding> {
    bindings.iter().find(|b| b.key == key)
}

fn string_of(b: &IrBinding) -> Option<&str> {
    match &b.value {
        IrBindingValue::String(s) => Some(s),
        _ => None,
    }
}

/// Checks one binding. `Ok(true)`: this module owns it and it is fine; `Ok(false)`: not its business.
pub fn check_binding(kind: &IrElementKind, b: &IrBinding) -> Result<bool, LowerError> {
    let waffle = matches!(kind, IrElementKind::Waffle);
    if !waffle {
        // `counted:` belongs to the fields as well; the other three keys belong to a Waffle alone
        if matches!(b.key.as_str(), "of" | "columns" | "rest") {
            return Err(refuse(
                "binding-unknown",
                format!("`{}:` is a binding of Waffle", b.key),
                b.span,
            ));
        }
        return Ok(false);
    }
    match b.key.as_str() {
        "of" => match int_of(b) {
            Some(n) if (1..=OF_MAX).contains(&n) => Ok(true),
            _ => Err(refuse(
                "waffle-of-invalid",
                format!("`of:` is a whole number written in the program, from 1 to {OF_MAX}: how many squares there are"),
                b.span,
            )),
        },
        "counted" => match int_of(b) {
            Some(n) if n >= 0 => Ok(true),
            _ => Err(refuse(
                "waffle-counted-invalid",
                "`counted:` is a whole number written in the program, 0 or more: how many squares are counted".to_string(),
                b.span,
            )),
        },
        "columns" => match int_of(b) {
            Some(n) if (1..=COLUMNS_MAX).contains(&n) => Ok(true),
            _ => Err(refuse(
                "waffle-columns-invalid",
                format!("`columns:` is a whole number written in the program, from 1 to {COLUMNS_MAX}"),
                b.span,
            )),
        },
        "rest" => match string_of(b) {
            Some(s) if RESTS.contains(&s) => Ok(true),
            _ => Err(refuse(
                "waffle-rest-invalid",
                format!("`rest:` is one of {} as a string literal: what the squares that are not counted are", RESTS.join(", ")),
                b.span,
            )),
        },
        other => Err(refuse(
            "binding-unknown",
            format!("`{other}:` is not a binding of Waffle; it takes label, {}", KEYS.join(", ")),
            b.span,
        )),
    }
}

/// What a `Waffle` needs to be complete.
pub fn check_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Waffle) {
        return Ok(());
    }
    let label_ok = accessibility
        .label
        .as_deref()
        .is_some_and(|l| !l.trim().is_empty());
    if !label_ok
        || !["of", "counted", "columns"]
            .iter()
            .all(|k| find(bindings, k).is_some())
    {
        return Err(refuse(
            "waffle-incomplete",
            "a Waffle needs `label:` (a string literal: its name, and the caption), `of:`, `counted:` and `columns:`: `Waffle { label: \"tests pass\" of: 704 counted: 697 columns: 44 }`".to_string(),
            span,
        ));
    }
    if !children.is_empty() {
        return Err(refuse(
            "waffle-has-children",
            "a Waffle has no content of its own: its squares, its caption and its table are made from its bindings".to_string(),
            span,
        ));
    }
    let (of, value) = (
        find(bindings, "of").and_then(int_of).unwrap_or(0),
        find(bindings, "counted").and_then(int_of).unwrap_or(0),
    );
    if value > of {
        return Err(refuse(
            "waffle-counted-invalid",
            format!("`counted: {value}` counts more squares than `of: {of}` has"),
            find(bindings, "counted").map_or(span, |b| b.span),
        ));
    }
    Ok(())
}

/// `(of, value, columns, rest is known)` of a checked Waffle.
fn spec(bindings: &[IrBinding]) -> (i64, i64, i64, bool) {
    (
        find(bindings, "of").and_then(int_of).unwrap_or(1),
        find(bindings, "counted").and_then(int_of).unwrap_or(0),
        find(bindings, "columns").and_then(int_of).unwrap_or(1),
        find(bindings, "rest").and_then(string_of) == Some("known"),
    )
}

/// The one sentence that is the text alternative, and the caption's second half: `697 of 704, 7 known differences`.
pub fn summary(bindings: &[IrBinding]) -> String {
    let (of, value, _, known) = spec(bindings);
    let rest = of - value;
    let mut s = format!("{value} of {of}");
    if rest > 0 {
        s.push_str(&format!(
            ", {rest} {}",
            if known {
                "known differences"
            } else {
                "not yet"
            }
        ));
    }
    s
}

/// The `<figure>` a Waffle becomes. Call it on an element that `check_complete` accepted.
pub fn dom(accessibility: &IrAccessibility, bindings: &[IrBinding]) -> Dom {
    let (of, value, columns, known) = spec(bindings);
    let label = accessibility.label.clone().unwrap_or_default();
    let rows = (of + columns - 1) / columns;
    let (w, h) = (columns * (CELL + GAP) - GAP, rows * (CELL + GAP) - GAP);
    let (mut counted, mut rest) = (String::new(), String::new());
    for i in 0..of {
        let (x, y) = ((i % columns) * (CELL + GAP), (i / columns) * (CELL + GAP));
        let d = if i < value { &mut counted } else { &mut rest };
        d.push_str(&format!("M{x} {y}h{CELL}v{CELL}h-{CELL}z"));
    }
    let mut svg = Dom::element("svg")
        .attr("viewBox", format!("0 0 {w} {h}"))
        .attr("role", "img")
        .attr("aria-label", format!("{label}: {}", summary(bindings)));
    if !counted.is_empty() {
        svg.children.push(
            Dom::element("path")
                .attr("class", "cui-wf-on")
                .attr("d", counted),
        );
    }
    if !rest.is_empty() {
        svg.children.push(
            Dom::element("path")
                .attr("class", if known { "cui-wf-known" } else { "cui-wf-open" })
                .attr("d", rest),
        );
    }
    let mut caption = Dom::element("figcaption");
    caption.text = Some(label);
    let mut table = Dom::element("table");
    let row = |a: &str, n: i64| {
        let mut tr = Dom::element("tr");
        let mut th = Dom::element("th").attr("scope", "row");
        th.text = Some(a.to_string());
        let mut td = Dom::element("td");
        td.text = Some(n.to_string());
        tr.children.push(th);
        tr.children.push(td);
        tr
    };
    let mut body = Dom::element("tbody");
    body.children.push(row("Counted", value));
    if of > value {
        body.children.push(row(
            if known {
                "Known differences"
            } else {
                "Not yet"
            },
            of - value,
        ));
    }
    body.children.push(row("Squares", of));
    table.children.push(body);
    let mut summary_el = Dom::element("summary");
    summary_el.text = Some("Data table".to_string());
    let mut details = Dom::element("details");
    details.children.push(summary_el);
    details.children.push(table);
    let mut figure = Dom::element("figure").attr("class", "cui-chart cui-waffle");
    figure.children.push(svg);
    figure.children.push(caption);
    figure.children.push(details);
    figure
}
