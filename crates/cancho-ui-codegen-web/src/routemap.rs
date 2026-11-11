//! `RouteMap` on the page (docs/routes.md): a `<figure>` holding an inline SVG of the located points as numbered pins, joined in list order by a
//! polyline, and a collapsed `<details>` with the same points as an ordered list. Nothing is fetched: there are no tiles, no host and no
//! capability. The text the page writes (`__cui_routemap_describe`) is that of `cancho_ui_replay::routemap::describe` in JavaScript;
//! `tests/routemap_corpus.json` and `scripts/check-routemap.js` hold both to the same cases. Every string from the program is set with
//! `textContent` or `setAttribute`, never parsed as markup.

use cancho_ui_ir::ir::*;

use crate::emit::{STATE_PREFIX, js_string};
use crate::expr::emit_expr_runtime;

/// Does a view hold a `RouteMap`?
pub(crate) fn view_has_routemap(node: &IrViewNode) -> bool {
    cancho_ui_ir::routemap::view_has_routemap(node)
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
