//! `Map` in replay (docs/maps.md): what the page asks the tile host for, and the list it writes. Replay does not draw, so it reports the
//! exact tile addresses, the zoom used, the list with its distances and how many pins fall in the picture. The emitted `__cui_map_plan` is the
//! same function; `tests/map_corpus.json` is checked against both.

use cancho_ui_ir::ir::{IrBindingValue, IrElementKind, IrViewNode};

use crate::eval::{json_get, math_builtin};
use crate::geo::{distance_km, parse_position};
use crate::machine::Machine;
use crate::value::{Value, js_number};

/// The most places listed, and the most pins drawn.
pub const MAX_LISTED: usize = 200;
pub const MAX_PINS: usize = 50;
/// The edge of a tile in pixels, and the tiles across and down the picture.
const TILE: f64 = 256.0;
const SPAN: i64 = 3;
/// The latitude web-mercator tiles reach.
const MAX_LAT: f64 = 85.051_128_779_8;

/// The highest zoom whose tile is at least as large as the rounding cell (0.01 degree) of a coarse position, by latitude: 13 up to 76, 12 up
/// to 83, 11 beyond (the tiles stop at 85.05).
pub fn coarse_zoom(lat: f64) -> i64 {
    let a = lat.abs();
    if a <= 76.0 {
        13
    } else if a <= 83.0 {
        12
    } else {
        11
    }
}

/// The tile of a position at a zoom: column and row.
pub fn tile_of(lat: f64, lon: f64, zoom: i64) -> (i64, i64) {
    let n = 2f64.powi(zoom as i32);
    let phi = lat.to_radians();
    let x = ((lon + 180.0) / 360.0 * n).floor();
    let y = ((1.0 - (phi.tan() + 1.0 / phi.cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor();
    let max = n - 1.0;
    (x.clamp(0.0, max) as i64, y.clamp(0.0, max) as i64)
}

/// A position in pixels of the whole world at a zoom.
fn world_px(lat: f64, lon: f64, zoom: i64) -> (f64, f64) {
    let n = 2f64.powi(zoom as i32) * TILE;
    let phi = lat.to_radians();
    (
        (lon + 180.0) / 360.0 * n,
        (1.0 - (phi.tan() + 1.0 / phi.cos()).ln() / std::f64::consts::PI) / 2.0 * n,
    )
}

fn km_text(km: f64) -> String {
    let r = math_builtin("round", &[km * 10.0]).unwrap_or(0.0) / 10.0;
    js_number(r).unwrap_or_else(|| "?".to_string())
}

#[derive(Debug, PartialEq)]
pub struct Plan {
    pub zoom: i64,
    pub tiles: Vec<String>,
    pub list: Vec<(String, String)>,
    pub listed: usize,
    pub total: usize,
    pub pinned: usize,
}

/// What a map shows for `center` ("lat,lon"), the rows of `places` (JSON texts with `name` and `pos`), at `zoom` (lowered to the rounding cell
/// of a coarse position when `coarse`), from the tile address `template`. A centre that is not a position, or is beyond the tiles
/// (85.05 degrees), has no tiles and no list.
pub fn plan(center: &str, rows: &[String], template: &str, zoom: i64, coarse: bool) -> Plan {
    let empty = Plan {
        zoom,
        tiles: vec![],
        list: vec![],
        listed: 0,
        total: 0,
        pinned: 0,
    };
    let Some((lat, lon)) = parse_position(center) else {
        return empty;
    };
    if lat.abs() > MAX_LAT {
        return empty;
    }
    let z = if coarse {
        zoom.min(coarse_zoom(lat))
    } else {
        zoom
    };
    let (cx, cy) = tile_of(lat, lon, z);
    let n = 1i64 << z;
    let mut tiles: Vec<String> = Vec::new();
    for dy in -(SPAN / 2)..=(SPAN / 2) {
        let y = cy + dy;
        if !(0..n).contains(&y) {
            continue;
        }
        for dx in -(SPAN / 2)..=(SPAN / 2) {
            let x = (cx + dx).rem_euclid(n);
            let url = template
                .replace("{z}", &z.to_string())
                .replace("{x}", &x.to_string())
                .replace("{y}", &y.to_string());
            if !tiles.contains(&url) {
                tiles.push(url);
            }
        }
    }
    // the readable places, nearest first (a stable sort: equal distances keep the list's order)
    let mut near: Vec<(f64, String, f64, f64)> = Vec::new();
    for row in rows {
        let pos = json_get(row, "pos", "");
        let d = distance_km(center, &pos, -1.0);
        if d >= 0.0
            && let Some((plat, plon)) = parse_position(&pos)
        {
            near.push((d, json_get(row, "name", ""), plat, plon));
        }
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total = near.len();
    let (ox, oy) = ((cx - SPAN / 2) as f64 * TILE, (cy - SPAN / 2) as f64 * TILE);
    let side = SPAN as f64 * TILE;
    let (cpx, _) = world_px(lat, lon, z);
    let world = n as f64 * TILE;
    let pinned = near
        .iter()
        .take(MAX_PINS)
        .filter(|p| {
            let (px, py) = world_px(p.2, p.3, z);
            // the picture may wrap around the date line: take the copy of the world nearest the centre
            let mut dx = px - cpx;
            if dx > world / 2.0 {
                dx -= world;
            } else if dx < -world / 2.0 {
                dx += world;
            }
            let x = cpx + dx - ox;
            let y = py - oy;
            (0.0..=side).contains(&x) && (0.0..=side).contains(&y)
        })
        .count();
    let list: Vec<(String, String)> = near
        .iter()
        .take(MAX_LISTED)
        .map(|p| (p.1.clone(), km_text(p.0)))
        .collect();
    Plan {
        zoom: z,
        tiles,
        listed: list.len(),
        list,
        total,
        pinned,
    }
}

/// One `Map` that is in the page now.
struct Shown {
    label: String,
    center: String,
    plan: Plan,
}

impl Machine<'_> {
    fn maps(&self) -> Vec<Shown> {
        fn walk(m: &Machine<'_>, node: &IrViewNode, out: &mut Vec<Shown>) {
            match node {
                IrViewNode::Element {
                    kind,
                    accessibility,
                    bindings,
                    children,
                    ..
                } => {
                    if matches!(kind, IrElementKind::Map) {
                        let get = |key: &str| bindings.iter().find(|b| b.key == key);
                        let center = match get("center").map(|b| &b.value) {
                            Some(IrBindingValue::String(s)) => Some(s.clone()),
                            Some(IrBindingValue::StateRef(n)) => match m.state.get(n) {
                                Some(Value::Str(s)) => Some(s.clone()),
                                _ => None,
                            },
                            _ => None,
                        };
                        let rows: Vec<String> = match get("places").map(|b| &b.value) {
                            Some(IrBindingValue::StateRef(n)) => match m.state.get(n) {
                                Some(Value::List(items)) => items
                                    .iter()
                                    .filter_map(|v| match v {
                                        Value::Str(s) => Some(s.clone()),
                                        _ => None,
                                    })
                                    .collect(),
                                _ => vec![],
                            },
                            _ => vec![],
                        };
                        let template = match get("tiles").map(|b| &b.value) {
                            Some(IrBindingValue::String(s)) => Some(s.clone()),
                            _ => None,
                        };
                        let zoom = match get("zoom").map(|b| &b.value) {
                            Some(IrBindingValue::Const { expr, .. }) => match **expr {
                                cancho_ui_ir::ir::IrExpr::IntLit { value, .. } => Some(value),
                                _ => None,
                            },
                            _ => None,
                        };
                        let label = match (&accessibility.label, &accessibility.label_expr) {
                            (Some(l), _) => Some(l.clone()),
                            (None, Some(e)) => match m.eval_view(e) {
                                Some(Value::Str(s)) => Some(s),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let (Some(center), Some(template), Some(zoom), Some(label)) =
                            (center, template, zoom, label)
                        {
                            let coarse = m.comp.requires.iter().any(|r| {
                                r.name == "Location"
                                    && (r.bounds.is_empty()
                                        || r.bounds.iter().all(|b| b == "coarse"))
                            });
                            let plan = plan(&center, &rows, &template, zoom, coarse);
                            out.push(Shown {
                                label,
                                center,
                                plan,
                            });
                        }
                    }
                    for c in children {
                        walk(m, c, out);
                    }
                }
                IrViewNode::For { children, .. } => {
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
                IrViewNode::Text { .. } => {}
            }
        }
        let mut out = Vec::new();
        walk(self, &self.comp.view.body, &mut out);
        out
    }

    /// The `maps` of a step report: `[{label, center, zoom, tiles, list, listed, total, pinned}]` (docs/maps.md section 3.2).
    pub fn maps_report(&self) -> Vec<serde_json::Value> {
        self.maps()
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "label": s.label,
                    "center": s.center,
                    "zoom": s.plan.zoom,
                    "tiles": s.plan.tiles,
                    "list": s.plan.list.iter().map(|(n, k)| serde_json::json!({"name": n, "km": k})).collect::<Vec<_>>(),
                    "listed": s.plan.listed,
                    "total": s.plan.total,
                    "pinned": s.plan.pinned,
                })
            })
            .collect()
    }
}
