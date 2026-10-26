/* Procedural granite. A seeded blob with a flat base (a boulder rests on a contact, it does not float),
   a highlight, a crack or two and some lichen. Same seed, same rock: the page is deterministic. */
(function () {
  var NS = 'http://www.w3.org/2000/svg';

  function rng(seed) {
    var s = seed >>> 0 || 1;
    return function () {
      s = (s + 0x6d2b79f5) >>> 0;
      var t = Math.imul(s ^ (s >>> 15), 1 | s);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  function hash(str) {
    var h = 2166136261;
    for (var i = 0; i < str.length; i++) { h ^= str.charCodeAt(i); h = Math.imul(h, 16777619); }
    return h >>> 0;
  }

  function el(name, attrs, parent) {
    var e = document.createElementNS(NS, name);
    for (var k in attrs) e.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(e);
    return e;
  }

  function outline(r, rx, ry, n, jitter, flat) {
    var pts = [];
    for (var i = 0; i < n; i++) {
      var a = (i / n) * Math.PI * 2;
      var k = 1 + (r() - 0.5) * 2 * jitter;
      pts.push([Math.cos(a) * rx * k, Math.min(Math.sin(a) * ry * k, ry * flat)]);
    }
    return pts;
  }

  function smooth(p) {
    var n = p.length, d = 'M' + p[0][0].toFixed(1) + ',' + p[0][1].toFixed(1);
    for (var i = 0; i < n; i++) {
      var p0 = p[(i - 1 + n) % n], p1 = p[i], p2 = p[(i + 1) % n], p3 = p[(i + 2) % n];
      d += 'C' + (p1[0] + (p2[0] - p0[0]) / 6).toFixed(1) + ',' + (p1[1] + (p2[1] - p0[1]) / 6).toFixed(1) + ' ' +
        (p2[0] - (p3[0] - p1[0]) / 6).toFixed(1) + ',' + (p2[1] - (p3[1] - p1[1]) / 6).toFixed(1) + ' ' +
        p2[0].toFixed(1) + ',' + p2[1].toFixed(1);
    }
    return d + 'Z';
  }

  /* Shared gradients and patterns, injected once so every page (and the share-card renderer) draws the same granite. */
  function ensureDefs() {
    if (document.getElementById('rocks-defs')) return;
    var svg = el('svg', { id: 'rocks-defs', class: 'defs', 'aria-hidden': 'true', focusable: 'false' }, document.body);
    var d = el('defs', {}, svg);
    var gr = el('linearGradient', { id: 'gr', x1: 0, y1: 0, x2: 1, y2: 1 }, d);
    [[0, 'var(--g1)'], [0.55, 'var(--g2)'], [1, 'var(--g3)']].forEach(function (s) { el('stop', { offset: s[0], style: 'stop-color:' + s[1] }, gr); });
    var sh = el('radialGradient', { id: 'shade', cx: 0.3, cy: 0.22, r: 0.95 }, d);
    [[0, 0], [0.55, 0], [1, 0.38]].forEach(function (s) { el('stop', { offset: s[0], 'stop-color': '#0a1424', 'stop-opacity': s[1] }, sh); });
    var halo = el('radialGradient', { id: 'halo' }, d);
    el('stop', { offset: 0.55, style: 'stop-color:var(--accent);stop-opacity:.28' }, halo);
    el('stop', { offset: 1, style: 'stop-color:var(--accent);stop-opacity:0' }, halo);
    // feldspar and mica: a tile of flecks, in the user space of whichever rock uses it
    var pat = el('pattern', { id: 'speck', width: 56, height: 56, patternUnits: 'userSpaceOnUse' }, d);
    var r = rng(11), cols = ['rgba(30,30,34,.38)', 'rgba(255,255,255,.55)', 'rgba(176,132,98,.5)', 'rgba(30,30,34,.25)'];
    for (var i = 0; i < 34; i++) {
      el('circle', { cx: (r() * 56).toFixed(1), cy: (r() * 56).toFixed(1), r: (0.5 + r() * 1.3).toFixed(2), fill: cols[i % cols.length] }, pat);
    }
  }

  /* rock(parent, {seed, x, baseY, rx, ry, shadow}) draws a boulder whose flat base sits on baseY. Returns the group. */
  function rock(parent, o) {
    var r = rng(typeof o.seed === 'string' ? hash(o.seed) : o.seed);
    var rx = o.rx, ry = o.ry, flat = 0.78;
    var g = el('g', { transform: 'translate(' + o.x + ',' + (o.baseY - ry * flat) + ')' }, parent);
    if (o.shadow) el('ellipse', { cx: rx * 0.12, cy: ry * flat, rx: rx * 1.08, ry: Math.max(4, ry * 0.13), class: 'rock-ground' }, g);
    var body = el('g', {}, g);
    var pts = outline(r, rx, ry, o.n || 12, o.jitter == null ? 0.13 : o.jitter, flat);
    var d = smooth(pts);
    el('path', { d: d, class: 'rock-shape' }, body);
    el('path', { d: d, class: 'rock-speck' }, body);
    el('path', { d: d, class: 'rock-shade' }, body);
    el('path', { d: d, class: 'rock-hl', transform: 'translate(' + (-rx * 0.1) + ',' + (-ry * 0.14) + ') scale(.78 .7)' }, body);
    var cracks = o.cracks == null ? 2 : o.cracks;
    for (var i = 0; i < cracks; i++) {
      var x0 = (r() - 0.5) * rx * 1.1, y0 = -ry * (0.2 + r() * 0.5);
      el('path', {
        d: 'M' + x0.toFixed(1) + ',' + y0.toFixed(1) + 'q' + ((r() - 0.5) * rx * 0.4).toFixed(1) + ',' + (ry * 0.3).toFixed(1) + ' ' +
          ((r() - 0.5) * rx * 0.5).toFixed(1) + ',' + (ry * (0.45 + r() * 0.3)).toFixed(1),
        class: 'rock-crack'
      }, body);
    }
    var lichen = o.lichen == null ? 3 : o.lichen;
    for (var j = 0; j < lichen; j++) {
      el('circle', { cx: ((r() - 0.5) * rx * 1.2).toFixed(1), cy: (-ry * (0.1 + r() * 0.6)).toFixed(1), r: (1.6 + r() * Math.min(5, rx * 0.06)).toFixed(1), class: 'rock-lichen' }, body);
    }
    return { g: g, body: body };
  }

  function ridge(parent, r, w, h, base, amp, cls) {
    var d = 'M0,' + h, x = 0;
    var y = base;
    d += 'L0,' + y.toFixed(0);
    while (x < w) {
      x += 30 + r() * 50;
      y = base - r() * amp;
      d += 'L' + Math.min(x, w).toFixed(0) + ',' + y.toFixed(0);
    }
    return el('path', { d: d + 'L' + w + ',' + h + 'Z', class: cls }, parent);
  }

  if (document.body) ensureDefs();
  window.Rocks = { ensureDefs: ensureDefs, outline: outline, smooth: smooth, rock: rock, ridge: ridge, rng: rng, hash: hash, el: el, NS: NS };
})();
