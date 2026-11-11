//! `RouteMap` in replay (docs/routes.md): what the page lists and says about a trip. Replay does not draw, so it cannot see the geometry of the
//! SVG; it reports the counts, the length of the route, how wide the diagram is, the accessible list and the sentence the page writes as the
//! SVG's accessible name. The emitted `__cui_routemap_describe` is the same function; `tests/routemap_corpus.json` is checked against both.

use cancho_ui_ir::ir::{IrBindingValue, IrElementKind, IrExpr, IrViewNode};

use crate::eval::{json_get, math_builtin};
use crate::geo::parse_position;
use crate::machine::Machine;
use crate::value::{Value, js_number};

pub use cancho_ui_ir::routemap::{MAX_KIND, MAX_POINTS};

/// The mean Earth radius in kilometres (the one `distance` uses).
const EARTH_KM: f64 = 6371.0088;

fn round(x: f64) -> f64 {
    math_builtin("round", &[x]).unwrap_or(0.0)
}

/// A distance in kilometres to a tenth, as the list writes it.
pub fn tenth_text(km: f64) -> String {
    js_number(round(km * 10.0) / 10.0).unwrap_or_else(|| "?".to_string())
}

/// "about N km across": 100 km and over to the nearest 10, 10 km and over to the nearest 1, under that to a tenth. `0` says the points are one place.
pub fn across_rounded(km: f64) -> f64 {
    if km >= 100.0 {
        round(km / 10.0) * 10.0
    } else if km >= 10.0 {
        round(km)
    } else {
        round(km * 10.0) / 10.0
    }
}

/// A `kind` is 1 to 12 letters, digits, `-` or `_`; anything else is no kind (it is not drawn in a colour of its own and not listed).
pub fn valid_kind(kind: &str) -> bool {
    let n = kind.chars().count();
    (1..=MAX_KIND).contains(&n)
        && kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// One row of the accessible list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The pin number, over the located points; `None` for a row with no position.
    pub number: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Described {
    /// Rows considered (at most 200, the first ones) and rows given.
    pub count: usize,
    pub total: usize,
    pub located: usize,
    pub on_route: usize,
    /// The length of the route in kilometres (the legs between consecutive located points; 0 when the route is not drawn).
    pub route_km: f64,
    /// The diagonal of the bounding box in the drawing's projection, in kilometres; `None` with no located point.
    pub across_km: Option<f64>,
    pub list: Vec<Entry>,
    pub summary: String,
}

fn haversine(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (pa, pb) = (a.0.to_radians(), b.0.to_radians());
    let h = ((pb - pa) / 2.0).sin().powi(2)
        + pa.cos() * pb.cos() * ((b.1 - a.1).to_radians() / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.sqrt().min(1.0).asin()
}

/// What a route map of these rows (JSON texts with `name`, `pos` and an optional `kind`) says, `route` being whether the polyline is drawn.
pub fn describe(rows: &[String], route: bool) -> Described {
    let total = rows.len();
    let rows = &rows[..total.min(MAX_POINTS)];
    let count = rows.len();
    let mut list = Vec::new();
    let mut located: Vec<(f64, f64)> = Vec::new();
    let mut route_km = 0.0;
    for row in rows {
        let name = json_get(row, "name", "");
        let name = if name.is_empty() {
            "(unnamed)".to_string()
        } else {
            name
        };
        let kind = json_get(row, "kind", "");
        let kind_part = if valid_kind(&kind) {
            format!(" ({kind})")
        } else {
            String::new()
        };
        match parse_position(&json_get(row, "pos", "")) {
            Some(p) => {
                let n = located.len() + 1;
                let tail = match located.last() {
                    None => "start".to_string(),
                    Some(prev) => {
                        let d = haversine(*prev, p);
                        route_km += d;
                        format!("{} km from the previous point", tenth_text(d))
                    }
                };
                located.push(p);
                list.push(Entry {
                    number: Some(n),
                    text: format!("{n}. {name}{kind_part}, {tail}"),
                });
            }
            None => list.push(Entry {
                number: None,
                text: format!("{name}{kind_part}, no position"),
            }),
        }
    }
    let n_located = located.len();
    let across_km = (n_located > 0).then(|| {
        let lat_min = located.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let lat_max = located
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let lon_min = located.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let lon_max = located
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let mean = located.iter().map(|p| p.0).sum::<f64>() / n_located as f64;
        let per_degree = EARTH_KM * std::f64::consts::PI / 180.0;
        let w = (lon_max - lon_min) * per_degree * mean.to_radians().cos();
        let h = (lat_max - lat_min) * per_degree;
        w.hypot(h)
    });
    let on_route = if route { n_located } else { 0 };
    if !route {
        route_km = 0.0;
    }
    let summary = if count == 0 {
        "route map with no places".to_string()
    } else {
        let mut s = format!(
            "route map of {count} place{}",
            if count == 1 { "" } else { "s" }
        );
        if n_located == 0 {
            s.push_str(", none with a position");
        } else {
            if route {
                s.push_str(&format!(", {on_route} on the route"));
            } else {
                s.push_str(", route not drawn");
            }
            if count > n_located {
                s.push_str(&format!(", {} without a position", count - n_located));
            }
            let about = across_rounded(across_km.unwrap_or(0.0));
            if about == 0.0 {
                s.push_str(", all at one position");
            } else {
                s.push_str(&format!(
                    ", about {} km across",
                    js_number(about).unwrap_or_else(|| "?".to_string())
                ));
            }
        }
        if total > count {
            s.push_str(&format!(", the first {count} of {total} shown"));
        }
        s
    };
    Described {
        count,
        total,
        located: n_located,
        on_route,
        route_km,
        across_km,
        list,
        summary,
    }
}

impl Machine<'_> {
    /// The `routemaps` of a step report: `[{label, count, total, located, on_route, km, across_km, summary, list}]` for each `RouteMap` in the page now
    /// (docs/routes.md section 5). One under an `If` that is false is not in the page; a `RouteMap` is never inside a `For` (`routemap-in-loop`).
    pub fn routemaps_report(&self) -> Vec<serde_json::Value> {
        fn walk(m: &Machine<'_>, node: &IrViewNode, out: &mut Vec<serde_json::Value>) {
            match node {
                IrViewNode::Element {
                    kind,
                    accessibility,
                    bindings,
                    children,
                    ..
                } => {
                    if matches!(kind, IrElementKind::RouteMap) {
                        let get = |key: &str| bindings.iter().find(|b| b.key == key);
                        let rows: Vec<String> = match get("points").map(|b| &b.value) {
                            Some(IrBindingValue::StateRef(n)) => match m.state.get(n) {
                                Some(Value::List(items)) => items
                                    .iter()
                                    .map(|v| match v {
                                        Value::Str(s) => s.clone(),
                                        _ => String::new(),
                                    })
                                    .collect(),
                                _ => vec![],
                            },
                            _ => vec![],
                        };
                        let route = match get("route").map(|b| &b.value) {
                            None => true,
                            Some(IrBindingValue::StateRef(n)) => {
                                !matches!(m.state.get(n), Some(Value::Bool(false)))
                            }
                            Some(IrBindingValue::Const { expr, .. }) => {
                                !matches!(**expr, IrExpr::BoolLit { value: false, .. })
                            }
                            _ => true,
                        };
                        let label = match (&accessibility.label, &accessibility.label_expr) {
                            (Some(l), _) => l.clone(),
                            (None, Some(e)) => match m.eval_view(e) {
                                Some(Value::Str(s)) => s,
                                _ => String::new(),
                            },
                            _ => String::new(),
                        };
                        let d = describe(&rows, route);
                        out.push(serde_json::json!({
                            "label": label,
                            "count": d.count,
                            "total": d.total,
                            "located": d.located,
                            "on_route": d.on_route,
                            "km": crate::value::num_json(round(d.route_km * 10.0) / 10.0),
                            "across_km": d.across_km.map(|x| crate::value::num_json(across_rounded(x))),
                            "summary": d.summary,
                            "list": d.list.iter().map(|e| e.text.clone()).collect::<Vec<_>>(),
                        }));
                    }
                    for c in children {
                        walk(m, c, out);
                    }
                }
                IrViewNode::If { cond, children, .. } => {
                    if m.condition_holds(cond) {
                        for c in children {
                            walk(m, c, out);
                        }
                    }
                }
                IrViewNode::For { .. } | IrViewNode::Text { .. } => {}
            }
        }
        let mut out = Vec::new();
        walk(self, &self.comp.view.body, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, pos: &str, kind: &str) -> String {
        format!("{{\"name\":\"{name}\",\"pos\":\"{pos}\",\"kind\":\"{kind}\"}}")
    }

    #[test]
    fn no_rows_say_so() {
        let d = describe(&[], true);
        assert_eq!(d.summary, "route map with no places");
        assert_eq!((d.count, d.located, d.on_route), (0, 0, 0));
        assert!(d.across_km.is_none());
    }

    #[test]
    fn one_point_is_at_one_position_and_has_no_leg() {
        let d = describe(&[row("A", "45.9237,6.8694", "lodging")], true);
        assert_eq!(
            d.summary,
            "route map of 1 place, 1 on the route, all at one position"
        );
        assert_eq!(d.list[0].text, "1. A (lodging), start");
        assert_eq!(d.route_km, 0.0);
    }

    #[test]
    fn the_same_point_twice_is_a_zero_leg() {
        let r = row("A", "45,6", "");
        let d = describe(&[r.clone(), r], true);
        assert_eq!(d.list[1].text, "2. A, 0 km from the previous point");
        assert!(d.summary.ends_with("all at one position"), "{}", d.summary);
    }

    #[test]
    fn a_row_without_a_position_is_listed_and_not_numbered() {
        let rows = [
            row("A", "45,6", "lodging"),
            row("B", "nonsense", "activity"),
            row("C", "46,7", "toolongkindname"),
        ];
        let d = describe(&rows, true);
        assert_eq!(d.located, 2);
        assert_eq!(d.list[1].text, "B (activity), no position");
        assert!(d.list[2].text.starts_with("2. C, "), "{}", d.list[2].text);
        assert!(
            d.summary
                .starts_with("route map of 3 places, 2 on the route, 1 without a position, about "),
            "{}",
            d.summary
        );
    }

    #[test]
    fn route_false_draws_no_route() {
        let rows = [row("A", "45,6", ""), row("B", "46,7", "")];
        let d = describe(&rows, false);
        assert_eq!((d.on_route, d.route_km), (0, 0.0));
        assert!(d.summary.contains("route not drawn"), "{}", d.summary);
    }

    #[test]
    fn at_most_200_points() {
        let rows: Vec<String> = (0..250)
            .map(|i| row("P", &format!("{},{}", 40.0 + i as f64 * 0.01, 3.0), ""))
            .collect();
        let d = describe(&rows, true);
        assert_eq!((d.count, d.total, d.located), (200, 250, 200));
        assert!(
            d.summary.ends_with(", the first 200 of 250 shown"),
            "{}",
            d.summary
        );
    }

    #[test]
    fn rounding_of_the_width() {
        assert_eq!(across_rounded(424.0), 420.0);
        assert_eq!(across_rounded(14.5), 15.0);
        assert_eq!(across_rounded(3.16), 3.2);
        assert_eq!(across_rounded(0.04), 0.0);
    }
}
