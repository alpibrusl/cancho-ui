//! `RouteMap` on the page (docs/routes.md): a `<figure>` holding an inline SVG of the located points as numbered pins, joined in list order by a
//! polyline, and a collapsed `<details>` with the same points as an ordered list. Without `tiles:` nothing is fetched: no host, no capability, and
//! the module is the one it always was. With `tiles:` the page also carries `TILES_JS` (section 9 of the docs), which lays the tiles of the
//! program's own host under the pins. The text the page writes (`__cui_routemap_describe`) is that of `cancho_ui_replay::routemap::describe` in JavaScript;
//! `tests/routemap_corpus.json` and `scripts/check-routemap.js` hold both to the same cases. Every string from the program is set with
//! `textContent` or `setAttribute`, never parsed as markup.

use cancho_ui_ir::ir::*;

use crate::emit::{STATE_PREFIX, js_string};
use crate::expr::emit_expr_runtime;

/// Does a view hold a `RouteMap`?
pub(crate) fn view_has_routemap(node: &IrViewNode) -> bool {
    cancho_ui_ir::routemap::view_has_routemap(node)
}

/// Does a view hold a `RouteMap` with `tiles:`?
pub(crate) fn view_has_tiles(node: &IrViewNode) -> bool {
    cancho_ui_ir::routemap::view_has_tiles(node)
}

/// The statements that draw one `RouteMap` into `parent_var`.
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
    let points = match get("points") {
        Some(IrBindingValue::StateRef(n)) => format!("{STATE_PREFIX}{n}"),
        // lowering has refused a RouteMap without them (`routemap-incomplete`); an empty list keeps the module valid
        _ => "[]".to_string(),
    };
    let route = match get("route") {
        Some(IrBindingValue::StateRef(n)) => format!("{STATE_PREFIX}{n} !== false"),
        Some(IrBindingValue::Const { expr, .. }) => match **expr {
            IrExpr::BoolLit { value, .. } => value.to_string(),
            _ => "true".to_string(),
        },
        _ => "true".to_string(),
    };
    let tiles = match get("tiles") {
        Some(IrBindingValue::String(s)) => Some(js_string(s)),
        _ => None,
    };
    let attribution = match get("attribution") {
        Some(IrBindingValue::String(s)) => js_string(s),
        _ => "\"\"".to_string(),
    };
    if let Some(tiles) = tiles {
        return format!(
            "{indent}const {var} = __cui_routemap_tiled(root, {key}, {label}, {points}, {route}, function (r, k) {{ return __cui_json_get(r, k, \"\"); }}, function (a, b) {{ return __cui_distance(a, b, -1); }}, {tiles}, {attribution});\n{indent}{parent_var}.appendChild({var});\n",
            key = js_string(var)
        );
    }
    format!(
        "{indent}const {var} = __cui_routemap({label}, {points}, {route}, function (r, k) {{ return __cui_json_get(r, k, \"\"); }}, function (a, b) {{ return __cui_distance(a, b, -1); }});\n{indent}{parent_var}.appendChild({var});\n"
    )
}

pub(crate) const ADAPTER_JS: &str = r#"// Platform adapter: RouteMap (docs/routes.md). A diagram of positions, drawn from the program's own list with no tiles, no host and no capability:
// an equirectangular projection (longitude scaled by the cosine of the mean latitude) fitted to the bounding box. It draws no coastlines and no
// geography, and it does not treat the antimeridian specially. The list is the accessible form.
const __CUI_RM_MAX = 200;
const __CUI_RM_W = 600;
const __CUI_RM_H = 400;
const __CUI_RM_PAD = 40;
const __CUI_RM_EARTH_KM = 6371.0088;
function __cui_rm_tenth(x) { return String(Math.round(x * 10) / 10); }
function __cui_rm_across(km) {
    if (km >= 100) { return Math.round(km / 10) * 10; }
    if (km >= 10) { return Math.round(km); }
    return Math.round(km * 10) / 10;
}
function __cui_rm_kind(k) { return /^[A-Za-z0-9_-]{1,12}$/.test(k); }
// What the route map says: { count, total, located, onRoute, routeKm, acrossKm (null with no located point), list ([{ number|null, text, cls, x, y }]), pts, summary }
function __cui_routemap_describe(rows, route, getj, dist) {
    const total = Array.isArray(rows) ? rows.length : 0;
    const count = Math.min(total, __CUI_RM_MAX);
    const list = [];
    const pts = [];
    let prev = null;
    let routeKm = 0;
    for (let i = 0; i < count; i++) {
        const row = typeof rows[i] === 'string' ? rows[i] : '';
        let name = getj(row, 'name');
        if (name === '') { name = '(unnamed)'; }
        const kind = getj(row, 'kind');
        const kindPart = __cui_rm_kind(kind) ? ' (' + kind + ')' : '';
        const cls = kind === 'lodging' || kind === 'activity' || kind === 'transport' ? kind : 'other';
        const pos = getj(row, 'pos');
        const p = __cui_position(pos);
        if (p === null) {
            list.push({ number: null, text: name + kindPart + ', no position' });
        } else {
            const n = pts.length + 1;
            let tail = 'start';
            if (prev !== null) {
                const d = dist(prev, pos);
                routeKm += d;
                tail = __cui_rm_tenth(d) + ' km from the previous point';
            }
            prev = pos;
            pts.push({ lat: p[0], lon: p[1], n: n, cls: cls });
            list.push({ number: n, text: n + '. ' + name + kindPart + ', ' + tail });
        }
    }
    const located = pts.length;
    let acrossKm = null;
    let kx = 1;
    if (located > 0) {
        let latMin = Infinity, latMax = -Infinity, lonMin = Infinity, lonMax = -Infinity, sum = 0;
        for (let i = 0; i < located; i++) {
            latMin = Math.min(latMin, pts[i].lat); latMax = Math.max(latMax, pts[i].lat);
            lonMin = Math.min(lonMin, pts[i].lon); lonMax = Math.max(lonMax, pts[i].lon);
            sum += pts[i].lat;
        }
        const mean = sum / located;
        kx = Math.cos(mean * Math.PI / 180);
        const perDegree = __CUI_RM_EARTH_KM * Math.PI / 180;
        acrossKm = Math.hypot((lonMax - lonMin) * perDegree * kx, (latMax - latMin) * perDegree);
    }
    const onRoute = route ? located : 0;
    if (!route) { routeKm = 0; }
    let summary;
    if (count === 0) {
        summary = 'route map with no places';
    } else {
        summary = 'route map of ' + count + ' place' + (count === 1 ? '' : 's');
        if (located === 0) {
            summary += ', none with a position';
        } else {
            summary += route ? ', ' + onRoute + ' on the route' : ', route not drawn';
            if (count > located) { summary += ', ' + (count - located) + ' without a position'; }
            const about = __cui_rm_across(acrossKm);
            summary += about === 0 ? ', all at one position' : ', about ' + about + ' km across';
        }
        if (total > count) { summary += ', the first ' + count + ' of ' + total + ' shown'; }
    }
    return { count: count, total: total, located: located, onRoute: onRoute, routeKm: routeKm, acrossKm: acrossKm, kx: kx, list: list, pts: pts, summary: summary };
}
// The SVG coordinates of the located points: fitted to the padded box, centred; one place (or all at one position) sits in the middle.
function __cui_routemap_layout(d) {
    const out = [];
    if (d.located === 0) { return out; }
    let minx = Infinity, maxx = -Infinity, miny = Infinity, maxy = -Infinity;
    const xs = [], ys = [];
    for (let i = 0; i < d.located; i++) {
        const x = d.pts[i].lon * d.kx;
        const y = -d.pts[i].lat;
        xs.push(x); ys.push(y);
        minx = Math.min(minx, x); maxx = Math.max(maxx, x);
        miny = Math.min(miny, y); maxy = Math.max(maxy, y);
    }
    const bw = __CUI_RM_W - 2 * __CUI_RM_PAD;
    const bh = __CUI_RM_H - 2 * __CUI_RM_PAD;
    const dx = maxx - minx;
    const dy = maxy - miny;
    const eps = 1e-9;
    let scale = 0;
    if (dx > eps || dy > eps) { scale = Math.min(dx > eps ? bw / dx : Infinity, dy > eps ? bh / dy : Infinity); }
    const cx = (minx + maxx) / 2;
    const cy = (miny + maxy) / 2;
    for (let i = 0; i < d.located; i++) {
        out.push([__CUI_RM_W / 2 + (xs[i] - cx) * scale, __CUI_RM_H / 2 + (ys[i] - cy) * scale]);
    }
    return out;
}
function __cui_routemap(label, rows, route, getj, dist) {
    const d = __cui_routemap_describe(rows, route, getj, dist);
    const ns = 'http://www.w3.org/2000/svg';
    const el = function (tag, attrs) {
        const e = document.createElementNS(ns, tag);
        for (const k in attrs) { e.setAttribute(k, String(attrs[k])); }
        return e;
    };
    const fig = document.createElement('figure');
    fig.classList.add('cui-chart');
    fig.classList.add('cui-routemap');
    const svg = el('svg', { viewBox: '0 0 ' + __CUI_RM_W + ' ' + __CUI_RM_H, role: 'img', 'aria-label': label + ': ' + d.summary });
    svg.setAttribute('class', 'cui-routemap-svg');
    // everything inside is presentation only: the name and the list say it in words
    const art = el('g', { 'aria-hidden': 'true' });
    const lx = [0.25, 0.5, 0.75];
    for (let i = 0; i < 3; i++) {
        const x = __CUI_RM_PAD + lx[i] * (__CUI_RM_W - 2 * __CUI_RM_PAD);
        art.appendChild(el('line', { x1: x, y1: __CUI_RM_PAD, x2: x, y2: __CUI_RM_H - __CUI_RM_PAD, stroke: 'currentColor', 'stroke-opacity': 0.25, 'stroke-width': 1 }));
    }
    for (let i = 0; i < 2; i++) {
        const y = __CUI_RM_PAD + (i + 1) / 3 * (__CUI_RM_H - 2 * __CUI_RM_PAD);
        art.appendChild(el('line', { x1: __CUI_RM_PAD, y1: y, x2: __CUI_RM_W - __CUI_RM_PAD, y2: y, stroke: 'currentColor', 'stroke-opacity': 0.25, 'stroke-width': 1 }));
    }
    art.appendChild(el('rect', { x: __CUI_RM_PAD, y: __CUI_RM_PAD, width: __CUI_RM_W - 2 * __CUI_RM_PAD, height: __CUI_RM_H - 2 * __CUI_RM_PAD, fill: 'none', stroke: 'currentColor', 'stroke-opacity': 0.25, 'stroke-width': 1 }));
    const xy = __cui_routemap_layout(d);
    if (route && xy.length >= 2) {
        art.appendChild(el('polyline', { points: xy.map(function (q) { return Math.round(q[0] * 100) / 100 + ',' + Math.round(q[1] * 100) / 100; }).join(' '), fill: 'none', stroke: 'var(--cui-chart, #0550ae)', 'stroke-width': 3, 'stroke-linejoin': 'round', 'stroke-linecap': 'round', class: 'cui-rm-line' }));
    }
    const fills = { lodging: 'var(--cui-route-lodging, #0550ae)', activity: 'var(--cui-route-activity, #1a7f37)', transport: 'var(--cui-route-transport, #57606a)', other: 'var(--cui-route-other, CanvasText)' };
    for (let i = 0; i < xy.length; i++) {
        const p = d.pts[i];
        art.appendChild(el('circle', { cx: Math.round(xy[i][0] * 100) / 100, cy: Math.round(xy[i][1] * 100) / 100, r: 14, fill: fills[p.cls], stroke: 'var(--cui-route-ink, Canvas)', 'stroke-width': 2, class: 'cui-rm-pin cui-rm-' + p.cls }));
        const t = el('text', { x: Math.round(xy[i][0] * 100) / 100, y: Math.round(xy[i][1] * 100) / 100 + 4, 'text-anchor': 'middle', 'font-size': p.n >= 100 ? 10 : 13, 'font-weight': 700, fill: 'var(--cui-route-ink, Canvas)', class: 'cui-rm-num' });
        t.textContent = String(p.n);
        art.appendChild(t);
    }
    const note = el('text', { x: __CUI_RM_W - __CUI_RM_PAD, y: __CUI_RM_H - 14, 'text-anchor': 'end', 'font-size': 13, fill: 'currentColor', class: 'cui-rm-note' });
    if (d.located === 0) { note.textContent = d.count === 0 ? 'no places' : 'no positions'; }
    else {
        const about = __cui_rm_across(d.acrossKm);
        note.textContent = about === 0 ? 'all at one position' : 'about ' + about + ' km across';
    }
    art.appendChild(note);
    svg.appendChild(art);
    fig.appendChild(svg);
    const cap = document.createElement('figcaption');
    cap.textContent = label;
    fig.appendChild(cap);
    const det = document.createElement('details');
    const sum = document.createElement('summary');
    sum.textContent = 'List of places (' + d.count + ')';
    det.appendChild(sum);
    const ol = document.createElement('ol');
    ol.className = 'cui-rm-list';
    for (let i = 0; i < d.list.length; i++) {
        const li = document.createElement('li');
        li.textContent = d.list[i].text;
        ol.appendChild(li);
    }
    det.appendChild(ol);
    if (d.total > d.count) {
        const more = document.createElement('p');
        more.textContent = 'The first ' + d.count + ' of ' + d.total + ' places are drawn and listed.';
        det.appendChild(more);
    }
    fig.appendChild(det);
    return fig;
}
"#;

/// Appended after `ADAPTER_JS` only for a program with a `RouteMap` that has `tiles:` (a program without one gets the module it always had).
/// `__cui_routemap_tile_plan` is `cancho_ui_replay::routemap::tile_plan` in JavaScript; `tests/routemap_tiles_corpus.json` and
/// `scripts/check-routemap-tiles.js` hold both to the same cases, computed by `scripts/gen-routemap-tiles-corpus.py`.
pub(crate) const TILES_JS: &str = r#"// Platform adapter: RouteMap tiles (docs/routes.md section 9). The tiles are the program's own host's images (declared in `requires Network`), asked for
// with no referrer and not kept by the page; the drawing is in Web Mercator so that the pins sit on the tiles. Tile addresses come from `tiles:` and nowhere else.
const __CUI_RMT_TILE = 256;
const __CUI_RMT_MAX_LAT = 85.0511287798;
const __CUI_RMT_SINGLE_ZOOM = 13;
const __CUI_RMT_MAX_ZOOM = 18;
const __CUI_RMT_MIN_ZOOM = 1;
const __CUI_RMT_MAX_GRID = 4;
const __CUI_RMT_MARGIN = 32;
function __cui_rmt_world(lat, lon, z) {
    const s = Math.pow(2, z) * __CUI_RMT_TILE;
    const phi = Math.min(Math.max(lat, -__CUI_RMT_MAX_LAT), __CUI_RMT_MAX_LAT) * Math.PI / 180;
    return [(lon + 180) / 360 * s, (1 - Math.log(Math.tan(phi) + 1 / Math.cos(phi)) / Math.PI) / 2 * s];
}
// What a tiled route map asks for: { zoom, tx0, ty0, cols, rows, tiles (row-major addresses), px (each point in the grid's own pixels) }; null with no located point.
function __cui_routemap_tile_plan(pts, tpl) {
    if (pts.length === 0) { return null; }
    let single = true;
    for (let i = 1; i < pts.length; i++) { if (pts[i].lat !== pts[0].lat || pts[i].lon !== pts[0].lon) { single = false; } }
    const cover = function (z) {
        const n = Math.pow(2, z);
        let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
        for (let i = 0; i < pts.length; i++) {
            const w = __cui_rmt_world(pts[i].lat, pts[i].lon, z);
            x0 = Math.min(x0, w[0]); x1 = Math.max(x1, w[0]); y0 = Math.min(y0, w[1]); y1 = Math.max(y1, w[1]);
        }
        const m = single ? __CUI_RMT_TILE : __CUI_RMT_MARGIN;
        const t = function (v) { return Math.min(Math.max(Math.floor(v / __CUI_RMT_TILE), 0), n - 1); };
        return [t(x0 - m), t(x1 + m), t(y0 - m), t(y1 + m)];
    };
    let zoom = __CUI_RMT_SINGLE_ZOOM;
    let c = cover(zoom);
    if (!single) {
        zoom = __CUI_RMT_MIN_ZOOM;
        c = cover(zoom);
        for (let z = __CUI_RMT_MAX_ZOOM; z >= __CUI_RMT_MIN_ZOOM; z--) {
            const k = cover(z);
            if (k[1] - k[0] + 1 <= __CUI_RMT_MAX_GRID && k[3] - k[2] + 1 <= __CUI_RMT_MAX_GRID) { zoom = z; c = k; break; }
        }
    }
    const tiles = [];
    for (let y = c[2]; y <= c[3]; y++) {
        for (let x = c[0]; x <= c[1]; x++) {
            tiles.push(tpl.split('{z}').join(String(zoom)).split('{x}').join(String(x)).split('{y}').join(String(y)));
        }
    }
    const px = [];
    for (let i = 0; i < pts.length; i++) {
        const w = __cui_rmt_world(pts[i].lat, pts[i].lon, zoom);
        px.push([w[0] - c[0] * __CUI_RMT_TILE, w[1] - c[2] * __CUI_RMT_TILE]);
    }
    return { zoom: zoom, tx0: c[0], ty0: c[2], cols: c[1] - c[0] + 1, rows: c[3] - c[2] + 1, tiles: tiles, px: px };
}
const __cui_routemap_tile_cache = new WeakMap();
// The figure of a tiled route map. A page is rebuilt at every render; the figure is kept while its inputs are the same, so a render that has nothing
// to do with the map asks the tile host for nothing.
function __cui_routemap_tiled(root, key, label, rows, route, getj, dist, tpl, attribution) {
    const sig = JSON.stringify([label, rows, route, tpl, attribution]);
    let kept = __cui_routemap_tile_cache.get(root);
    if (!kept) { kept = {}; __cui_routemap_tile_cache.set(root, kept); }
    if (kept[key] && kept[key].sig === sig) { return kept[key].el; }
    const fig = __cui_routemap(label, rows, route, getj, dist);
    const d = __cui_routemap_describe(rows, route, getj, dist);
    const plan = __cui_routemap_tile_plan(d.pts, tpl);
    // with no located point nothing is asked for and the plain diagram stands
    if (plan !== null) {
        const ns = 'http://www.w3.org/2000/svg';
        const el = function (tag, attrs) {
            const e = document.createElementNS(ns, tag);
            for (const k in attrs) { e.setAttribute(k, String(attrs[k])); }
            return e;
        };
        const W = plan.cols * __CUI_RMT_TILE;
        const H = plan.rows * __CUI_RMT_TILE;
        // pins keep the size they have in the plain diagram: 14 of 600 across the longer side
        const u = Math.max(W, H) / 600;
        const r2 = function (v) { return Math.round(v * 100) / 100; };
        fig.classList.add('cui-routemap-tiled');
        const svg = fig.querySelector('svg');
        svg.setAttribute('viewBox', '0 0 ' + W + ' ' + H);
        svg.style.position = 'absolute';
        svg.style.left = '0';
        svg.style.top = '0';
        svg.style.width = '100%';
        svg.style.height = '100%';
        svg.style.maxWidth = 'none';
        svg.removeChild(svg.querySelector('g'));
        const art = el('g', { 'aria-hidden': 'true' });
        if (route && plan.px.length >= 2) {
            const line = plan.px.map(function (q) { return r2(q[0]) + ',' + r2(q[1]); }).join(' ');
            const layer = function (stroke, width, cls) {
                return el('polyline', { points: line, fill: 'none', stroke: stroke, 'stroke-width': r2(width * u), 'stroke-linejoin': 'round', 'stroke-linecap': 'round', class: cls });
            };
            // a light casing and an ink line under the series colour: the route reads on a light map and on a dark page
            art.appendChild(layer('#ffffff', 9, 'cui-rm-casing'));
            art.appendChild(layer('var(--cui-route-ink, Canvas)', 6, 'cui-rm-ink'));
            art.appendChild(layer('var(--cui-chart, #0550ae)', 3, 'cui-rm-line'));
        }
        const fills = { lodging: 'var(--cui-route-lodging, #0550ae)', activity: 'var(--cui-route-activity, #1a7f37)', transport: 'var(--cui-route-transport, #57606a)', other: 'var(--cui-route-other, CanvasText)' };
        for (let i = 0; i < plan.px.length; i++) {
            const p = d.pts[i];
            const x = r2(plan.px[i][0]);
            const y = r2(plan.px[i][1]);
            art.appendChild(el('circle', { cx: x, cy: y, r: r2(17.5 * u), fill: '#ffffff', 'fill-opacity': 0.92, class: 'cui-rm-halo' }));
            art.appendChild(el('circle', { cx: x, cy: y, r: r2(14 * u), fill: fills[p.cls], stroke: 'var(--cui-route-ink, Canvas)', 'stroke-width': r2(2 * u), class: 'cui-rm-pin cui-rm-' + p.cls }));
            const t = el('text', { x: x, y: r2(y + 4 * u), 'text-anchor': 'middle', 'font-size': r2((p.n >= 100 ? 10 : 13) * u), 'font-weight': 700, fill: 'var(--cui-route-ink, Canvas)', class: 'cui-rm-num' });
            t.textContent = String(p.n);
            art.appendChild(t);
        }
        svg.appendChild(art);
        // the tiles are a picture under the SVG: decorative, no referrer, not lazy, no CORS; the SVG's name and the list are the accessible form
        const wrap = document.createElement('div');
        wrap.className = 'cui-rm-pic';
        wrap.style.position = 'relative';
        wrap.style.overflow = 'hidden';
        wrap.style.width = '100%';
        wrap.style.maxWidth = (Math.min(1, W / H) * 40) + 'rem';
        wrap.style.aspectRatio = W + ' / ' + H;
        wrap.style.background = 'rgba(128, 128, 128, 0.14)';
        for (let i = 0; i < plan.tiles.length; i++) {
            const col = i % plan.cols;
            const row = Math.floor(i / plan.cols);
            const img = document.createElement('img');
            img.alt = '';
            img.setAttribute('referrerpolicy', 'no-referrer');
            img.setAttribute('decoding', 'async');
            img.setAttribute('loading', 'eager');
            img.draggable = false;
            img.style.position = 'absolute';
            img.style.display = 'block';
            img.style.left = (col * __CUI_RMT_TILE / W * 100) + '%';
            img.style.top = (row * __CUI_RMT_TILE / H * 100) + '%';
            img.style.width = (__CUI_RMT_TILE / W * 100) + '%';
            img.style.height = (__CUI_RMT_TILE / H * 100) + '%';
            img.addEventListener('error', function (ev) { ev.target.style.visibility = 'hidden'; });
            img.src = plan.tiles[i];
            wrap.appendChild(img);
        }
        fig.insertBefore(wrap, svg);
        wrap.appendChild(svg);
        // visible text under the drawing: the width, and the credit the tile provider asks for (set as text, never parsed)
        const about = __cui_rm_across(d.acrossKm);
        const note = document.createElement('p');
        note.className = 'cui-rm-note';
        note.style.margin = '0.25rem 0 0';
        note.style.fontSize = '0.88rem';
        note.textContent = about === 0 ? 'all at one position' : 'about ' + about + ' km across';
        const credit = document.createElement('p');
        credit.className = 'cui-rm-attr';
        credit.style.margin = '0';
        credit.style.fontSize = '0.88rem';
        credit.textContent = attribution;
        fig.insertBefore(note, wrap.nextSibling);
        fig.insertBefore(credit, note.nextSibling);
    }
    kept[key] = { sig: sig, el: fig };
    return fig;
}
"#;
