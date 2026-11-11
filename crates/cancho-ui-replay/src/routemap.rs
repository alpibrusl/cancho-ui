//! `RouteMap` in replay (docs/routes.md): what the page lists and says about a trip. Replay does not draw, so it cannot see the geometry of the
//! SVG; it reports the counts, the length of the route, how wide the diagram is, the accessible list and the sentence the page writes as the
//! SVG's accessible name. The emitted `__cui_routemap_describe` is the same function; `tests/routemap_corpus.json` is checked against both.

use cancho_ui_ir::ir::{IrBindingValue, IrElementKind, IrExpr, IrViewNode};

use crate::eval::{json_get, math_builtin};
use crate::geo::parse_position;
use crate::machine::Machine;
use crate::value::{Value, js_number};

pub use cancho_ui_ir::routemap::{MAX_KIND, MAX_POINTS};

/// The edge of a tile in pixels.
const TILE: f64 = 256.0;
/// The latitude Web Mercator tiles reach; a latitude beyond it is drawn (and asked for) at the edge of the world.
const MAX_LAT: f64 = 85.051_128_779_8;
/// The zoom of a tiled `RouteMap` with one located place (or all at one position), and the grid around its tile.
pub const SINGLE_ZOOM: i64 = 13;
/// The highest and lowest zoom chosen for several places, and the most tiles across and down.
pub const MAX_ZOOM: i64 = 18;
pub const MIN_ZOOM: i64 = 1;
pub const MAX_GRID: i64 = 4;
/// Pixels of margin kept around the places inside the grid (more than a pin's radius at any size of the drawing).
pub const MARGIN: f64 = 32.0;

/// A position in pixels of the whole world at a zoom (Web Mercator, 256 px tiles); the latitude is clamped to the tiles' range.
fn world_px(lat: f64, lon: f64, zoom: i64) -> (f64, f64) {
    let s = 2f64.powi(zoom as i32) * TILE;
    let phi = lat.clamp(-MAX_LAT, MAX_LAT).to_radians();
    (
        (lon + 180.0) / 360.0 * s,
        (1.0 - (phi.tan() + 1.0 / phi.cos()).ln() / std::f64::consts::PI) / 2.0 * s,
    )
}

/// What a tiled `RouteMap` asks for: the zoom, the grid of tiles (`cols` x `rows` from tile `tx0`, `ty0`), the addresses in row-major order, and the
/// position of each located point in the drawing's pixels (the grid's own space, `cols * 256` wide and `rows * 256` high).
#[derive(Debug, Clone, PartialEq)]
pub struct TilePlan {
    pub zoom: i64,
    pub tx0: i64,
    pub ty0: i64,
    pub cols: i64,
    pub rows: i64,
    pub tiles: Vec<String>,
    pub px: Vec<(f64, f64)>,
}

/// The tile plan for the located points (`lat`, `lon`) and the address `template`; `None` with no located point (nothing is asked for).
/// One point, or all at one position: zoom 13 and the 3 x 3 tiles around its tile. Otherwise the highest zoom from 18 down to 1 at which the
/// tiles covering the points plus `MARGIN` pixels, clipped to the tile range, are at most 4 across and 4 down (zoom 1 always fits).
/// Longitude is not wrapped: the antimeridian is a limit of the drawing; a latitude beyond 85.05 is clamped.
pub fn tile_plan(points: &[(f64, f64)], template: &str) -> Option<TilePlan> {
    let first = *points.first()?;
    let single = points.iter().all(|p| *p == first);
    let cover = |z: i64| -> (i64, i64, i64, i64) {
        let n = 1i64 << z;
        let w: Vec<(f64, f64)> = points.iter().map(|p| world_px(p.0, p.1, z)).collect();
        let lo = |f: fn(&(f64, f64)) -> f64| w.iter().map(f).fold(f64::INFINITY, f64::min);
        let hi = |f: fn(&(f64, f64)) -> f64| w.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        let (x0, x1, y0, y1) = if single {
            let (x, y) = w[0];
            (x - TILE, x + TILE, y - TILE, y + TILE)
        } else {
            (
                lo(|p| p.0) - MARGIN,
                hi(|p| p.0) + MARGIN,
                lo(|p| p.1) - MARGIN,
                hi(|p| p.1) + MARGIN,
            )
        };
        let t = |v: f64| (v / TILE).floor() as i64;
        (
            t(x0).clamp(0, n - 1),
            t(x1).clamp(0, n - 1),
            t(y0).clamp(0, n - 1),
            t(y1).clamp(0, n - 1),
        )
    };
    let (zoom, (tx0, tx1, ty0, ty1)) = if single {
        (SINGLE_ZOOM, cover(SINGLE_ZOOM))
    } else {
        let mut chosen = None;
        for z in (MIN_ZOOM..=MAX_ZOOM).rev() {
            let c = cover(z);
            if c.1 - c.0 + 1 <= MAX_GRID && c.3 - c.2 + 1 <= MAX_GRID {
                chosen = Some((z, c));
                break;
            }
        }
        chosen.unwrap_or((MIN_ZOOM, cover(MIN_ZOOM)))
    };
    let mut tiles = Vec::new();
    for y in ty0..=ty1 {
        for x in tx0..=tx1 {
            tiles.push(
                template
                    .replace("{z}", &zoom.to_string())
                    .replace("{x}", &x.to_string())
                    .replace("{y}", &y.to_string()),
            );
        }
    }
    let px = points
        .iter()
        .map(|p| {
            let (x, y) = world_px(p.0, p.1, zoom);
            (x - tx0 as f64 * TILE, y - ty0 as f64 * TILE)
        })
        .collect();
    Some(TilePlan {
        zoom,
        tx0,
        ty0,
        cols: tx1 - tx0 + 1,
        rows: ty1 - ty0 + 1,
        tiles,
        px,
    })
}

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
    /// The located points, `(lat, lon)`, in route order.
    pub points: Vec<(f64, f64)>,
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
        points: located,
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
                        let tiles = match get("tiles").map(|b| &b.value) {
                            Some(IrBindingValue::String(t)) => Some(t.clone()),
                            _ => None,
                        };
                        let mut entry = serde_json::json!({
                            "label": label,
                            "count": d.count,
                            "total": d.total,
                            "located": d.located,
                            "on_route": d.on_route,
                            "km": crate::value::num_json(round(d.route_km * 10.0) / 10.0),
                            "across_km": d.across_km.map(|x| crate::value::num_json(across_rounded(x))),
                            "summary": d.summary,
                            "list": d.list.iter().map(|e| e.text.clone()).collect::<Vec<_>>(),
                        });
                        // with `tiles:` (and only then) the report says what the page asks the tile host for: replay cannot fetch a tile
                        if let Some(t) = tiles {
                            let plan = tile_plan(&d.points, &t);
                            entry["zoom"] = plan
                                .as_ref()
                                .map_or(serde_json::Value::Null, |p| serde_json::json!(p.zoom));
                            entry["grid"] = plan.as_ref().map_or(serde_json::json!([0, 0]), |p| {
                                serde_json::json!([p.cols, p.rows])
                            });
                            entry["tiles"] = serde_json::json!(
                                plan.as_ref().map(|p| p.tiles.clone()).unwrap_or_default()
                            );
                            if let Some(IrBindingValue::String(a)) =
                                get("attribution").map(|b| &b.value)
                            {
                                entry["attribution"] = serde_json::json!(a);
                            }
                        }
                        out.push(entry);
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

    const TILES_CORPUS: &str = include_str!("../../../tests/routemap_tiles_corpus.json");

    fn corpus_points(k: &serde_json::Value) -> Vec<(f64, f64)> {
        k["points"]
            .as_array()
            .expect("points")
            .iter()
            .map(|p| parse_position(p.as_str().expect("a text")).expect("a position"))
            .collect()
    }

    #[test]
    fn the_tile_plan_is_that_of_the_independent_corpus() {
        let c: serde_json::Value = serde_json::from_str(TILES_CORPUS).expect("JSON");
        let template = c["template"].as_str().expect("template");
        let cases = c["cases"].as_array().expect("cases");
        assert!(cases.len() >= 50, "the corpus shrank");
        for k in cases {
            let why = k["why"].as_str().expect("why");
            let plan = tile_plan(&corpus_points(k), template).expect("a plan");
            let w = &k["want"];
            assert_eq!(plan.zoom, w["zoom"].as_i64().expect("zoom"), "{why}: zoom");
            assert_eq!(
                [plan.cols, plan.rows],
                [
                    w["grid"][0].as_i64().unwrap(),
                    w["grid"][1].as_i64().unwrap()
                ],
                "{why}: grid"
            );
            let tiles: Vec<&str> = w["tiles"]
                .as_array()
                .expect("tiles")
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect();
            assert_eq!(plan.tiles, tiles, "{why}: tiles");
            for (q, e) in plan.px.iter().zip(w["px"].as_array().expect("px")) {
                assert!((q.0 - e[0].as_f64().unwrap()).abs() < 1e-6, "{why}: x");
                assert!((q.1 - e[1].as_f64().unwrap()).abs() < 1e-6, "{why}: y");
            }
        }
    }

    #[test]
    fn no_located_point_asks_for_no_tile() {
        assert!(tile_plan(&[], "https://t.example/{z}/{x}/{y}.png").is_none());
    }

    #[test]
    fn a_grid_is_at_most_four_tiles_each_way_and_one_place_is_three_by_three() {
        let c: serde_json::Value = serde_json::from_str(TILES_CORPUS).expect("JSON");
        for k in c["cases"].as_array().expect("cases") {
            let plan = tile_plan(&corpus_points(k), "/{z}/{x}/{y}").expect("a plan");
            assert!(plan.cols <= 4 && plan.rows <= 4 && plan.tiles.len() <= 16);
            assert!((1..=18).contains(&plan.zoom));
        }
        let one = tile_plan(&[(45.9, 6.9)], "/{z}/{x}/{y}").expect("a plan");
        assert_eq!((one.zoom, one.cols, one.rows), (13, 3, 3));
    }
}
