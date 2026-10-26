/* The ecosystem map, drawn like a walkers' map of La Pedriza: parchment, green forest patches, pink granite, dashed
   footpaths, a river, a road, handwritten names. Layout only: size, distance, footpaths and rivers mean nothing.
   The one line that does mean something is the blue arrow, "rests on". Reads data/ecosystem.json. */
(function () {
  'use strict';
  var R = window.Rocks, el = R.el;

  // [x, y, where the name goes]: s = under the rock; n, e, w = beside it, joined by a thin line
  var SPOTS = {
    cancho: [280, 505, 'n'], 'cancho-ui': [250, 710, 'e'],
    'cancho-forge': [405, 255, 'w'], 'cancho-cache': [560, 335, 's'], 'cancho-hooks': [705, 255, 'n'], 'cancho-gateway': [855, 335, 's'], 'cancho-mqtt': [995, 255, 'n'], 'cancho-dns': [1120, 335, 's'], 'cancho-oci': [1215, 245, 'n'],
    'cancho-schema': [560, 485, 's'], 'cancho-web': [745, 550, 's'], 'cancho-pg': [905, 485, 'n'], 'cancho-log': [1050, 550, 's'], 'cancho-stats': [1170, 465, 'n'], 'cancho-ml': [1200, 600, 's'], 'cancho-mqtt-client': [415, 615, 'w'],
    'cancho-tools': [535, 775, 'w'], 'cancho-table': [690, 775, 's'], 'cancho-client-gen': [855, 775, 's'], 'cancho-code': [610, 865, 's'], 'cancho-fetch': [775, 865, 's'],
    'cancho-robot': [1030, 775, 'n'], 'cancho-gpu': [1170, 775, 'n'], 'cancho-hdl': [1100, 865, 's']
  };
  var REGIONS = [['La Pedriza', 120, 205], ['Services', 420, 150], ['Libraries', 640, 392], ['Tools', 345, 872], ['Edge', 1190, 888]];
  var PARKING = [112, 652];
  var ROAD = 'M20,130 C50,300 15,470 52,600 C92,705 230,800 420,892';
  var RIVER = 'M96,110 C130,260 90,420 125,560 C160,690 300,790 470,860 C560,895 600,900 700,905';
  var FOREST = [[300, 470, 190, 110], [620, 330, 250, 90], [880, 500, 260, 110], [1090, 330, 150, 90], [620, 790, 230, 90], [1080, 790, 170, 90], [240, 690, 100, 60]];

  function blob(r, cx, cy, rx, ry, n, jitter) {
    return R.smooth(R.outline(r, rx, ry, n, jitter, 0.78).map(function (p) { return [p[0] + cx, p[1] + cy]; }));
  }

  /* One outcrop of granite: a main boulder (shaded, with a highlight) and a few pebbles around it. */
  function granite(parent, o) {
    var r = R.rng(R.hash(o.seed)), g = el('g', { class: 'granite' }, parent);
    var ry = o.ry * 0.78, cy = o.baseY - ry * 0.78;
    if (!o.design) el('ellipse', { cx: o.x + o.rx * 0.1, cy: o.baseY + 2, rx: o.rx * 1.1, ry: Math.max(4, ry * 0.16), class: 'g-shadow' }, g);
    var pebbles = o.big ? 8 : 2 + (R.hash(o.seed) % 4);
    for (var i = 0; i < pebbles; i++) {
      var a = r() * Math.PI * 2, d = o.rx * (1.05 + r() * 0.55), pr = 3 + r() * (o.big ? 9 : 6);
      el('path', { d: blob(r, o.x + Math.cos(a) * d, o.baseY - pr * 0.6 + Math.sin(a) * ry * 0.5, pr * 1.3, pr * 0.8, 8, 0.2), class: 'g-body g-pebble' }, g);
    }
    var d = blob(r, o.x, cy, o.rx, ry, o.big ? 15 : 11, o.big ? 0.08 : 0.15);
    el('path', { d: d, class: 'g-body' }, g);
    if (!o.design) {
      el('path', { d: d, class: 'g-shade' }, g);
      el('ellipse', { cx: o.x - o.rx * 0.3, cy: cy - ry * 0.42, rx: o.rx * 0.28, ry: ry * 0.14, class: 'g-light', transform: 'rotate(-12 ' + (o.x - o.rx * 0.3) + ' ' + (cy - ry * 0.42) + ')' }, g);
      for (var k = 0; k < (o.big ? 4 : 1); k++) {
        var x0 = o.x + (r() - 0.5) * o.rx * 0.9, y0 = cy - ry * (0.1 + r() * 0.35);
        el('path', { d: 'M' + x0.toFixed(1) + ',' + y0.toFixed(1) + 'q' + ((r() - 0.5) * o.rx * 0.3).toFixed(1) + ',' + (ry * 0.25).toFixed(1) + ' ' + ((r() - 0.5) * o.rx * 0.35).toFixed(1) + ',' + (ry * 0.5).toFixed(1), class: 'g-crack' }, g);
      }
    }
    return g;
  }

  /* A footpath between two places: a few wobbling waypoints through a smooth curve. */
  function trail(parent, r, a, b, cls) {
    var dx = b.x - a.x, dy = b.y - a.y, len = Math.hypot(dx, dy) || 1, ux = dx / len, uy = dy / len;
    var sx = a.x + ux * (a.rx || 20) * 0.9, sy = a.y - (a.ry || 10) * 0.4 + uy * (a.ry || 10) * 0.5;
    var ex = b.x - ux * (b.rx || 20) * 0.9, ey = b.y - (b.ry || 10) * 0.4 - uy * (b.ry || 10) * 0.4;
    var pts = [[sx, sy]];
    for (var i = 1; i <= 3; i++) {
      var t = i / 4, off = (r() - 0.5) * Math.min(60, len * 0.25);
      pts.push([sx + (ex - sx) * t - uy * off, sy + (ey - sy) * t + ux * off]);
    }
    pts.push([ex, ey]);
    var d = 'M' + pts[0][0].toFixed(1) + ',' + pts[0][1].toFixed(1);
    for (var j = 0; j < pts.length - 1; j++) {
      var p0 = pts[Math.max(0, j - 1)], p1 = pts[j], p2 = pts[j + 1], p3 = pts[Math.min(pts.length - 1, j + 2)];
      d += 'C' + (p1[0] + (p2[0] - p0[0]) / 6).toFixed(1) + ',' + (p1[1] + (p2[1] - p0[1]) / 6).toFixed(1) + ' ' + (p2[0] - (p3[0] - p1[0]) / 6).toFixed(1) + ',' + (p2[1] - (p3[1] - p1[1]) / 6).toFixed(1) + ' ' + p2[0].toFixed(1) + ',' + p2[1].toFixed(1);
    }
    return el('path', { d: d, class: cls }, parent);
  }

  var svg, pos, rocks = [], restLines = [];
  function draw(data) {
    svg = el('svg', { class: 'cmap', viewBox: '0 100 1300 810', width: '100%', focusable: 'false' }, cancho.host);
    var defs = el('defs', {}, svg);
    var gg = el('linearGradient', { id: 'gran', x1: 0, y1: 0, x2: 0.8, y2: 1 }, defs);
    [[0, 'var(--gr1)'], [0.55, 'var(--gr2)'], [1, 'var(--gr3)']].forEach(function (s) { el('stop', { offset: s[0], style: 'stop-color:' + s[1] }, gg); });
    var gs = el('radialGradient', { id: 'gran-shade', cx: 0.3, cy: 0.2, r: 1 }, defs);
    [[0, 0], [0.6, 0], [1, 0.32]].forEach(function (s) { el('stop', { offset: s[0], 'stop-color': '#3a1d14', 'stop-opacity': s[1] }, gs); });
    var blur = el('filter', { id: 'soft', x: '-30%', y: '-30%', width: '160%', height: '160%' }, defs);
    el('feGaussianBlur', { stdDeviation: 22 }, blur);
    var grain = el('filter', { id: 'paper', x: 0, y: 0, width: '100%', height: '100%' }, defs);
    el('feTurbulence', { type: 'fractalNoise', baseFrequency: 0.8, numOctaves: 2, seed: 4, result: 'n' }, grain);
    el('feColorMatrix', { in: 'n', type: 'matrix', values: '0 0 0 0 0.35  0 0 0 0 0.28  0 0 0 0 0.12  0 0 0 0.09 0' }, grain);

    // paper, with a little grain, and the forest as soft green patches
    el('rect', { x: 0, y: 100, width: 1300, height: 810, class: 'paper' }, svg);
    el('rect', { x: 0, y: 100, width: 1300, height: 810, filter: 'url(#paper)', class: 'grain' }, svg);
    var fg = el('g', { filter: 'url(#soft)', class: 'forest' }, svg);
    FOREST.forEach(function (f) { el('ellipse', { cx: f[0], cy: f[1], rx: f[2], ry: f[3] }, fg); });

    // river and road, each with a name along it
    el('path', { d: RIVER, class: 'river' }, svg);
    var road = el('g', { class: 'road' }, svg);
    el('path', { d: ROAD, class: 'road-edge' }, road);
    el('path', { id: 'road-path', d: ROAD, class: 'road-fill' }, road);
    el('textPath', { href: '#road-path', startOffset: '4%' }, el('text', { class: 'road-name' }, road)).textContent = 'github.com / alpibrusl';
    el('path', { id: 'river-path', d: RIVER, fill: 'none' }, svg);
    var rn = el('text', { class: 'river-name' }, svg);
    el('textPath', { href: '#river-path', startOffset: '17%' }, rn).textContent = 'Río Manzanares';

    REGIONS.forEach(function (r) { el('text', { x: r[1], y: r[2], class: 'region' }, svg).textContent = r[0]; });
    // a few houses, because there is a village at the bottom of every walk
    var vil = el('g', { class: 'village' }, svg), vr = R.rng(21);
    for (var h = 0; h < 14; h++) el('rect', { x: 95 + (h % 5) * 14 + vr() * 6, y: 812 + Math.floor(h / 5) * 14 + vr() * 5, width: 8 + vr() * 5, height: 7 + vr() * 4 }, vil);
    el('text', { x: 135, y: 803, class: 'name sub', style: 'text-anchor:middle' }, svg).textContent = 'Manzanares el Real';

    var na = el('g', { class: 'north', transform: 'translate(1262,705)' }, svg);
    el('path', { d: 'M0,26 L0,-16 M-7,-6 L0,-18 L7,-6', class: 'north-line' }, na);
    el('text', { x: 0, y: -24, style: 'text-anchor:middle' }, na).textContent = 'N';

    pos = {};
    var spare = 0;
    data.projects.forEach(function (p) {
      var big = p.id === 'cancho', s = SPOTS[p.id] || [100 + (spare++) * 120, 890, 's'];
      pos[p.id] = { p: p, x: s[0], y: s[1], dir: s[2], big: big, rx: big ? 118 : 46 + (R.hash(p.id) % 14), ry: big ? 88 : 34 + (R.hash(p.id + 'y') % 10) };
    });

    // green footpaths: each place to its two nearest neighbours (and the car park), purely decorative
    var tg = el('g', { class: 'trails' }, svg), tr = R.rng(5), ids = Object.keys(pos), seen = {};
    var nodes = ids.map(function (id) { return pos[id]; }).concat([{ x: PARKING[0] + 12, y: PARKING[1] + 30, rx: 12, ry: 8, id: 'P' }]);
    nodes.forEach(function (a) {
      nodes.filter(function (b) { return b !== a; })
        .sort(function (u, v) { return Math.hypot(u.x - a.x, (u.y - a.y) * 1.2) - Math.hypot(v.x - a.x, (v.y - a.y) * 1.2); })
        .slice(0, 2).forEach(function (b) {
          var key = [a.id || a.p.id, b.id || b.p.id].sort().join('|');
          if (!seen[key]) { seen[key] = 1; trail(tg, tr, a, b, 'trail'); }
        });
    });

    // the car park is this page
    var pk = el('a', { href: 'https://github.com/alpibrusl/cancho-www', class: 'park' }, svg);
    el('rect', { x: PARKING[0], y: PARKING[1], width: 24, height: 24, rx: 3 }, pk);
    el('text', { x: PARKING[0] + 12, y: PARKING[1] + 18, class: 'park-p', style: 'text-anchor:middle' }, pk).textContent = 'P';
    el('text', { x: PARKING[0] + 12, y: PARKING[1] - 8, class: 'name', style: 'text-anchor:middle' }, pk).textContent = 'cancho-www';
    el('text', { x: PARKING[0] + 12, y: PARKING[1] + 44, class: 'name sub', style: 'text-anchor:middle' }, pk).textContent = 'you are here';

    // the one meaningful line: rests on
    var lines = el('g', {}, svg);
    var mk = el('marker', { id: 'mk-rest', viewBox: '0 0 10 10', refX: 8, refY: 5, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' }, defs);
    el('path', { d: 'M0,0L10,5L0,10z', class: 'mk-rest' }, mk);
    data.projects.forEach(function (p) {
      (p.rests_on || []).forEach(function (t) {
        if (!pos[t]) return;
        var a = pos[p.id], b = pos[t];
        restLines.push(el('path', { d: 'M' + a.x + ',' + (a.y - a.ry * 1.2) + 'Q' + ((a.x + b.x) / 2) + ',' + (Math.min(a.y, b.y) - 110) + ' ' + b.x + ',' + (b.y - b.ry * 1.5), class: 'rests', 'marker-end': 'url(#mk-rest)', 'data-a': p.id, 'data-b': t }, lines));
      });
    });

    Object.keys(pos).sort(function (a, b) { return pos[a].y - pos[b].y; }).forEach(function (id) {
      var q = pos[id], p = q.p, design = p.stage === 'design';
      var g = el('g', { class: 'rock ' + (design ? 'design' : 'built'), tabindex: 0, role: 'button', 'aria-label': p.name + ', ' + p.stage + ': ' + p.summary, 'data-id': id, 'data-kind': p.kind }, svg);
      var inner = el('g', { class: 'inner' }, g);
      granite(inner, { seed: p.id, x: q.x, baseY: q.y, rx: q.rx, ry: q.ry, big: q.big, design: design });
      el('circle', { cx: q.x + q.rx * 0.62, cy: q.y - q.ry * 1.3, r: 5, class: 'pin ' + p.stage }, inner);
      var top = q.y - q.ry * 1.45, mid = q.y - q.ry * 0.55, cls = 'name' + (q.big ? ' big' : '');
      if (q.dir === 's') {
        el('text', { x: q.x, y: q.y + 21, class: cls, style: 'text-anchor:middle' }, g).textContent = p.name;
      } else if (q.dir === 'n') {
        el('text', { x: q.x, y: top - 14, class: cls, style: 'text-anchor:middle' }, g).textContent = p.name;
        el('path', { d: 'M' + q.x + ',' + (top - 9) + ' L' + q.x + ',' + (top + 2), class: 'leader' }, g);
      } else {
        var sgn = q.dir === 'e' ? 1 : -1, edge = q.x + sgn * q.rx * 1.02;
        el('text', { x: edge + sgn * 34, y: mid + 4, class: cls, style: 'text-anchor:' + (sgn > 0 ? 'start' : 'end') }, g).textContent = p.name;
        el('path', { d: 'M' + (edge + sgn * 30) + ',' + mid + ' L' + (edge + sgn * 2) + ',' + mid, class: 'leader' }, g);
      }
      g.addEventListener('click', function () { cancho.emit('Selected', id); });
      g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); cancho.emit('Selected', id); } });
      rocks.push(g);
    });

    // the legend: the glyphs of what a rock, a dotted outline, an arrow and a footpath mean (the words are the page's)
    var lg = document.createElement('div');
    lg.className = 'cmap-legend';
    lg.innerHTML = '<span><svg viewBox="0 0 30 18" aria-hidden="true"><path class="lg-fill" d="M4,15 Q1,6 10,4 Q22,2 25,11 Q26,15 21,15Z"/></svg>exists</span>' +
      '<span><svg viewBox="0 0 30 18" aria-hidden="true"><path class="lg-line" d="M4,15 Q1,6 10,4 Q22,2 25,11 Q26,15 21,15Z" stroke-dasharray="2 3" opacity=".8"/></svg>design, not built</span>' +
      '<span><svg viewBox="0 0 30 18" aria-hidden="true"><path d="M2,14 Q14,-2 25,12" fill="none" stroke="var(--cui-accent)" stroke-width="1.8" stroke-dasharray="4 3"/><path d="M28,13 L22,9.5 L22.5,15z" fill="var(--cui-accent)"/></svg>rests on</span>' +
      '<span><svg viewBox="0 0 30 18" aria-hidden="true"><path d="M2,9 Q10,2 16,10 T28,8" fill="none" stroke="var(--map-trail)" stroke-width="1.6" stroke-dasharray="5 4"/></svg>footpath (layout only)</span>';
    cancho.host.appendChild(lg);
  }

  /* what the program says is on the map now: the rock that is chosen, and what a filter leaves out (the same rules as the page's own filter) */
  function apply(p) {
    if (!svg) { return; }
    rocks.forEach(function (g) {
      var id = g.getAttribute('data-id'), q = pos[id].p;
      var dim = (p.kind !== 'all' && q.kind !== p.kind) || (p.only_built && q.stage === 'design');
      g.classList.toggle('sel', id === p.selected);
      g.classList.toggle('dim', dim);
      g.setAttribute('tabindex', dim ? -1 : 0);
    });
    restLines.forEach(function (l) {
      var dim = svg.querySelector('.rock[data-id="' + l.getAttribute('data-a') + '"]').classList.contains('dim') || svg.querySelector('.rock[data-id="' + l.getAttribute('data-b') + '"]').classList.contains('dim');
      l.classList.toggle('dim', dim);
    });
    svg.classList.toggle('filtered', p.kind !== 'all' || !!p.only_built);
  }

  var drawn = false;
  cancho.host.classList.add('cmap-host');
  cancho.onprops(function (p) {
    if (!drawn) {
      var list;
      try { list = JSON.parse(p.places); } catch (e) { list = []; }
      drawn = true;
      draw({ projects: Array.isArray(list) ? list : [] });
    }
    apply(p);
  });
})();
