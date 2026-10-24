//! `Map` on the page (docs/maps.md): a `<figure>` with the 3 x 3 tiles around the tile of the centre, the places pinned over them, the list of
//! those places by distance, and the attribution. The tile addresses come from the program's `tiles:` and nowhere else; the compiler has no
//! default host. The steps that decide what is shown (`__cui_map_plan`) are those of `cancho_ui_replay::map_plan` in JavaScript;
//! `tests/map_corpus.json` and `scripts/check-map.js` hold both to the same cases.

use cancho_ui_ir::ir::*;

use crate::emit::{STATE_PREFIX, js_string};
use crate::expr::emit_expr_runtime;

/// Does a view hold a `Map`?
pub(crate) fn view_has_map(node: &IrViewNode) -> bool {
    cancho_ui_ir::map::view_has_map(node)
}

/// The statements that draw one `Map` into `parent_var`.
pub(crate) fn emit(
    indent: &str,
    var: &str,
    parent_var: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let label = match (&accessibility.label, &accessibility.label_expr) {
        (Some(l), _) => js_string(l),
        (None, Some(e)) => format!("String({})", emit_expr_runtime(e, params)),
        (None, None) => "\"\"".to_string(),
    };
    let get = |key: &str| bindings.iter().find(|b| b.key == key).map(|b| &b.value);
    let center = match get("center") {
        Some(IrBindingValue::StateRef(n)) => Some(format!("{STATE_PREFIX}{n}")),
        Some(IrBindingValue::String(s)) => Some(js_string(s)),
        _ => None,
    };
    let places = match get("places") {
        Some(IrBindingValue::StateRef(n)) => Some(format!("{STATE_PREFIX}{n}")),
        _ => None,
    };
    let zoom = match get("zoom") {
        Some(IrBindingValue::Const { expr, .. }) => match **expr {
            IrExpr::IntLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    };
    let tiles = match get("tiles") {
        Some(IrBindingValue::String(s)) => Some(js_string(s)),
        _ => None,
    };
    let attribution = match get("attribution") {
        Some(IrBindingValue::String(s)) => Some(js_string(s)),
        _ => None,
    };
    let (Some(center), Some(places), Some(zoom), Some(tiles), Some(attribution)) =
        (center, places, zoom, tiles, attribution)
    else {
        // lowering has refused a Map without them (`map-incomplete`); an empty figure keeps the module valid
        return format!(
            "{indent}const {var} = document.createElement('figure');\n{indent}{parent_var}.appendChild({var});\n"
        );
    };
    format!(
        "{indent}const {var} = __cui_map(root, {key}, {label}, {center}, {places}, {zoom}, {tiles}, {attribution}, typeof __cui_map_coarse !== 'undefined' && __cui_map_coarse, function (r, k) {{ return __cui_json_get(r, k, \"\"); }}, function (a, b) {{ return __cui_distance(a, b, -1); }});\n{indent}{parent_var}.appendChild({var});\n",
        key = js_string(var)
    )
}

pub(crate) const ADAPTER_JS: &str = r#"// Platform adapter: Map (docs/maps.md). The tiles are images of the program's tile host (declared in `requires Network`), asked for with no
// referrer; the picture is centred on the middle of the position's tile, so that the set of tiles asked for says no more than the tile; the
// list of places, nearest first, is what a screen reader and a keyboard get.
const __CUI_MAP_TILE = 256;
const __CUI_MAP_MAX_LISTED = 200;
const __CUI_MAP_MAX_PINS = 50;
const __CUI_MAP_MAX_LAT = 85.0511287798;
function __cui_map_zoom_cap(lat) {
    const a = Math.abs(lat);
    return a <= 76 ? 13 : (a <= 83 ? 12 : 11);
}
function __cui_map_tile(lat, lon, z) {
    const n = Math.pow(2, z);
    const phi = lat * Math.PI / 180;
    const x = Math.floor((lon + 180) / 360 * n);
    const y = Math.floor((1 - Math.log(Math.tan(phi) + 1 / Math.cos(phi)) / Math.PI) / 2 * n);
    return [Math.min(Math.max(x, 0), n - 1), Math.min(Math.max(y, 0), n - 1)];
}
function __cui_map_world(lat, lon, z) {
    const n = Math.pow(2, z) * __CUI_MAP_TILE;
    const phi = lat * Math.PI / 180;
    return [(lon + 180) / 360 * n, (1 - Math.log(Math.tan(phi) + 1 / Math.cos(phi)) / Math.PI) / 2 * n];
}
// What the map shows: { zoom, tiles (the addresses, once each), cells ([column, row, address] of the picture), list ([name, km text]), listed,
// total, pinned, pins ([x, y, n] in picture units), me ([x, y]) }.
function __cui_map_plan(center, rows, tpl, zoom, coarse, getj, dist) {
    const none = { zoom: zoom, tiles: [], cells: [], list: [], listed: 0, total: 0, pinned: 0, pins: [], me: null };
    const p = __cui_position(center);
    if (p === null || Math.abs(p[0]) > __CUI_MAP_MAX_LAT || !Array.isArray(rows)) { return none; }
    const lat = p[0];
    const lon = p[1];
    const z = coarse ? Math.min(zoom, __cui_map_zoom_cap(lat)) : zoom;
    const t = __cui_map_tile(lat, lon, z);
    const n = Math.pow(2, z);
    const tiles = [];
    const cells = [];
    for (let dy = -1; dy <= 1; dy++) {
        const y = t[1] + dy;
        if (y < 0 || y >= n) { continue; }
        for (let dx = -1; dx <= 1; dx++) {
            const x = ((t[0] + dx) % n + n) % n;
            const url = tpl.split('{z}').join(String(z)).split('{x}').join(String(x)).split('{y}').join(String(y));
            if (tiles.indexOf(url) < 0) { tiles.push(url); }
            cells.push([dx + 1, dy + 1, url]);
        }
    }
    const near = [];
    for (let i = 0; i < rows.length; i++) {
        const pos = getj(rows[i], 'pos');
        const d = dist(center, pos);
        const q = __cui_position(pos);
        if (d >= 0 && q !== null) { near.push([d, getj(rows[i], 'name'), q, i]); }
    }
    near.sort(function (a, b) { return a[0] - b[0] || a[3] - b[3]; });
    const side = 3 * __CUI_MAP_TILE;
    const ox = (t[0] - 1) * __CUI_MAP_TILE;
    const oy = (t[1] - 1) * __CUI_MAP_TILE;
    const world = n * __CUI_MAP_TILE;
    const c = __cui_map_world(lat, lon, z);
    const at = function (q) {
        const w = __cui_map_world(q[0], q[1], z);
        let dx = w[0] - c[0];
        if (dx > world / 2) { dx -= world; } else if (dx < -world / 2) { dx += world; }
        return [c[0] + dx - ox, w[1] - oy];
    };
    const pins = [];
    let pinned = 0;
    for (let i = 0; i < near.length && i < __CUI_MAP_MAX_PINS; i++) {
        const xy = at(near[i][2]);
        if (xy[0] >= 0 && xy[0] <= side && xy[1] >= 0 && xy[1] <= side) { pinned++; pins.push([xy[0], xy[1], i + 1]); }
    }
    const list = [];
    for (let i = 0; i < near.length && i < __CUI_MAP_MAX_LISTED; i++) {
        list.push([near[i][1], String(Math.round(near[i][0] * 10) / 10)]);
    }
    return { zoom: z, tiles: tiles, cells: cells, list: list, listed: list.length, total: near.length, pinned: pinned, pins: pins, me: at(p) };
}
function __cui_map_style() {
    if (typeof document === 'undefined' || document.getElementById('cui-map-style')) { return; }
    const s = document.createElement('style');
    s.id = 'cui-map-style';
    s.textContent = ':root { --cui-map-pin: #0550ae; --cui-map-fail: #d0d7de; }'
        + ' @media (prefers-color-scheme: dark) { :root { --cui-map-pin: #79c0ff; --cui-map-fail: #30363d; } }'
        + ' .cui-map { margin: 0; display: flex; flex-direction: column; gap: 0.5rem; }'
        + ' .cui-map-pic { position: relative; display: grid; grid-template-columns: repeat(3, 1fr); width: 100%; max-width: 40rem; aspect-ratio: 1; overflow: hidden; background: var(--cui-map-fail); }'
        + ' .cui-map-pic img { display: block; width: 100%; height: 100%; object-fit: fill; }'
        + ' .cui-map-pins { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }'
        + ' .cui-map-list { margin: 0; padding-inline-start: 1.5rem; }'
        + ' .cui-map-attr { font-size: 0.875rem; }';
    document.head.appendChild(s);
}
const __cui_map_cache = new WeakMap();
// The figure for a map. A page is rebuilt at every render; the figure is kept while its inputs are the same, so a render that has nothing to do
// with the map asks the tile host for nothing.
function __cui_map(root, key, label, center, rows, zoom, tpl, attribution, coarse, getj, dist) {
    __cui_map_style();
    const sig = JSON.stringify([label, center, rows, zoom, tpl, attribution, coarse]);
    let kept = __cui_map_cache.get(root);
    if (!kept) { kept = {}; __cui_map_cache.set(root, kept); }
    if (kept[key] && kept[key].sig === sig) { return kept[key].el; }
    const plan = __cui_map_plan(center, rows, tpl, zoom, coarse, getj, dist);
    const fig = document.createElement('figure');
    fig.className = 'cui-map';
    const pic = document.createElement('div');
    pic.className = 'cui-map-pic';
    pic.setAttribute('role', 'img');
    pic.setAttribute('aria-label', label);
    const note = document.createElement('p');
    note.setAttribute('role', 'status');
    note.hidden = true;
    let failed = 0;
    for (let i = 0; i < plan.cells.length; i++) {
        const c = plan.cells[i];
        const img = document.createElement('img');
        img.alt = '';
        img.setAttribute('referrerpolicy', 'no-referrer');
        img.setAttribute('decoding', 'async');
        img.draggable = false;
        img.width = __CUI_MAP_TILE;
        img.height = __CUI_MAP_TILE;
        img.style.gridColumn = String(c[0]);
        img.style.gridRow = String(c[1]);
        img.addEventListener('error', function () {
            img.style.visibility = 'hidden';
            failed++;
            if (failed === plan.cells.length) { note.textContent = 'The map is not available offline.'; note.hidden = false; }
        });
        img.src = c[2];
        pic.appendChild(img);
    }
    if (plan.me) {
        const ns = 'http://www.w3.org/2000/svg';
        const svg = document.createElementNS(ns, 'svg');
        svg.setAttribute('class', 'cui-map-pins');
        svg.setAttribute('viewBox', '0 0 768 768');
        svg.setAttribute('aria-hidden', 'true');
        svg.setAttribute('focusable', 'false');
        const mark = function (x, y, r, fill, stroke) {
            const c = document.createElementNS(ns, 'circle');
            c.setAttribute('cx', String(x));
            c.setAttribute('cy', String(y));
            c.setAttribute('r', String(r));
            c.setAttribute('fill', fill);
            c.setAttribute('stroke', stroke);
            c.setAttribute('stroke-width', '3');
            svg.appendChild(c);
        };
        for (let i = 0; i < plan.pins.length; i++) {
            const q = plan.pins[i];
            mark(q[0], q[1], 14, 'var(--cui-map-pin)', 'white');
            const t = document.createElementNS(ns, 'text');
            t.setAttribute('x', String(q[0]));
            t.setAttribute('y', String(q[1] + 5));
            t.setAttribute('text-anchor', 'middle');
            t.setAttribute('font-size', '14');
            t.setAttribute('font-weight', '700');
            t.setAttribute('fill', 'white');
            t.textContent = String(q[2]);
            svg.appendChild(t);
        }
        if (plan.me[0] >= 0 && plan.me[0] <= 768 && plan.me[1] >= 0 && plan.me[1] <= 768) { mark(plan.me[0], plan.me[1], 8, 'none', 'var(--cui-map-pin)'); }
        pic.appendChild(svg);
    }
    fig.appendChild(pic);
    fig.appendChild(note);
    if (plan.me === null) {
        const none = document.createElement('p');
        none.textContent = 'There is no position to show.';
        fig.appendChild(none);
    }
    const ol = document.createElement('ol');
    ol.className = 'cui-map-list';
    for (let i = 0; i < plan.list.length; i++) {
        const li = document.createElement('li');
        li.textContent = plan.list[i][0] + ', ' + plan.list[i][1] + ' km';
        ol.appendChild(li);
    }
    fig.appendChild(ol);
    if (plan.total > plan.listed) {
        const more = document.createElement('p');
        more.textContent = 'The first ' + plan.listed + ' of ' + plan.total + ' places are listed.';
        fig.appendChild(more);
    }
    const cap = document.createElement('figcaption');
    cap.className = 'cui-map-attr';
    cap.textContent = attribution;
    fig.appendChild(cap);
    kept[key] = { sig: sig, el: fig };
    return fig;
}
"#;
