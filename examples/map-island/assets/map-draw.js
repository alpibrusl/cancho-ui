// The drawing half of a map of projects, as an open island (docs/islands.md section 12). It draws; the program (map.cui) owns everything else: the
// filter buttons, the count, the detail pane and the list that is the map's text alternative.
// Props: places (a JSON text keyed by id: { name, kind, stage, rests_on }), selected, kind ("all" or a kind), only_built.
// It says `Selected` with an id when a rock is chosen. If `window.Rocks` (a granite drawing library) is loaded it draws rocks with it, and ellipses if not.
(function () {
    'use strict';
    var NS = 'http://www.w3.org/2000/svg', W = 1300, H = 810, TOP = 100;
    // where each place sits on the sheet (layout only: distance means nothing); a place with no spot goes in the bottom row
    var SPOTS = { cancho: [280, 505], 'cancho-ui': [250, 710], 'cancho-forge': [405, 255], 'cancho-cache': [560, 335], 'cancho-hooks': [705, 255],
        'cancho-schema': [560, 485], 'cancho-tools': [535, 775], 'cancho-robot': [1030, 775], 'cancho-hdl': [1100, 865] };
    var host = cancho.host, places = {}, ids = [], rocks = {}, svg, lines = [];
    var reduce = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;

    function el(name, attrs, parent) {
        var e = document.createElementNS(NS, name);
        for (var k in attrs) { e.setAttribute(k, attrs[k]); }
        if (parent) { parent.appendChild(e); }
        return e;
    }
    function hash(s) { var h = 2166136261; for (var i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }

    function spot(id, n) { return SPOTS[id] || [120 + (n % 9) * 130, 890]; }

    function build(json) {
        try { places = JSON.parse(json); } catch (e) { places = {}; }
        ids = Object.keys(places);
        host.textContent = '';
        svg = el('svg', { viewBox: '0 ' + TOP + ' ' + W + ' ' + (H - TOP + 100), width: '100%', 'aria-hidden': 'true', focusable: 'false' }, host);
        el('rect', { x: 0, y: TOP, width: W, height: H, class: 'paper' }, svg);
        // footpaths: each place to its nearest neighbour (decoration)
        var tg = el('g', { class: 'trails' }, svg), at = ids.map(function (id, n) { return spot(id, n); });
        at.forEach(function (a, i) {
            var best = -1, bd = 1e9;
            at.forEach(function (b, j) { var d = Math.hypot(a[0] - b[0], (a[1] - b[1]) * 1.2); if (j !== i && d < bd) { bd = d; best = j; } });
            if (best >= 0) { el('path', { d: 'M' + a[0] + ',' + a[1] + 'Q' + (a[0] + at[best][0]) / 2 + ',' + (Math.max(a[1], at[best][1]) + 40) + ' ' + at[best][0] + ',' + at[best][1], class: 'trail' }, tg); }
        });
        // the one line that means something: rests on
        var lg = el('g', {}, svg);
        el('marker', { id: 'mk', viewBox: '0 0 10 10', refX: 8, refY: 5, markerWidth: 7, markerHeight: 7, orient: 'auto' }, el('defs', {}, svg)).appendChild(el('path', { d: 'M0,0L10,5L0,10z', class: 'mk' }));
        lines = [];
        ids.forEach(function (id, n) {
            (places[id].rests_on || []).forEach(function (t) {
                if (!places[t]) { return; }
                var a = spot(id, n), b = spot(t, ids.indexOf(t));
                lines.push({ a: id, b: t, node: el('path', { d: 'M' + a[0] + ',' + (a[1] - 50) + 'Q' + (a[0] + b[0]) / 2 + ',' + (Math.min(a[1], b[1]) - 110) + ' ' + b[0] + ',' + (b[1] - 60), class: 'rests', 'marker-end': 'url(#mk)' }, lg) });
            });
        });
        rocks = {};
        ids.slice().sort(function (x, y) { return spot(x, 0)[1] - spot(y, 0)[1]; }).forEach(function (id) {
            var n = ids.indexOf(id), s = spot(id, n), p = places[id], big = id === 'cancho';
            var g = el('g', { class: 'rock ' + (p.stage === 'design' ? 'design' : 'built'), tabindex: 0, role: 'button', 'aria-label': p.name + ', ' + p.stage, 'data-id': id }, svg);
            var rx = big ? 118 : 46 + (hash(id) % 14), ry = big ? 88 : 34 + (hash(id + 'y') % 10);
            if (window.Rocks && window.Rocks.rock) { window.Rocks.rock(el('g', {}, g), { seed: id, x: s[0], baseY: s[1], rx: rx, ry: ry, n: 11, cracks: 1, lichen: 0, jitter: 0.1 }); }
            else { el('ellipse', { cx: s[0], cy: s[1] - ry * 0.6, rx: rx, ry: ry * 0.8, class: 'plain' }, g); }
            el('text', { x: s[0], y: s[1] + 21, class: 'name' + (big ? ' big' : ''), 'text-anchor': 'middle' }, g).textContent = p.name;
            g.addEventListener('click', function () { cancho.emit('Selected', id); });
            g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); cancho.emit('Selected', id); } });
            rocks[id] = g;
        });
    }

    // the camera: the whole sheet, or a rock; eased, and not at all for a visitor who asked for less motion
    function vbTo(to, ms) {
        var from = svg.getAttribute('viewBox').split(' ').map(Number), t0 = null;
        if (reduce || !ms) { svg.setAttribute('viewBox', to.join(' ')); return; }
        (function f(t) {
            if (t0 === null) { t0 = t; }
            var k = Math.min(1, (t - t0) / ms), e = k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2;
            svg.setAttribute('viewBox', from.map(function (v, i) { return (v + (to[i] - v) * e).toFixed(1); }).join(' '));
            if (k < 1) { requestAnimationFrame(f); }
        })(performance.now());
    }

    function show(p) {
        var dim = {};
        ids.forEach(function (id) {
            var q = places[id];
            dim[id] = (p.kind !== 'all' && q.kind !== p.kind) || (p.only_built && q.stage === 'design');
            rocks[id].classList.toggle('dim', dim[id]);
            rocks[id].classList.toggle('sel', id === p.selected);
            rocks[id].setAttribute('tabindex', dim[id] ? -1 : 0);
        });
        lines.forEach(function (l) { l.node.classList.toggle('dim', dim[l.a] || dim[l.b]); });
        if (rocks[p.selected]) {
            var s = spot(p.selected, ids.indexOf(p.selected));
            vbTo([Math.max(0, s[0] - 230), Math.max(TOP, s[1] - 230), 460, 295], 700);
        } else {
            vbTo([0, TOP, W, H - TOP + 100], 500);
        }
    }

    var built = null;
    cancho.onprops(function (p) {
        if (built !== p.places) { built = p.places; build(p.places); }
        show(p);
    });
})();
