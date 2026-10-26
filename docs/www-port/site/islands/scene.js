/* The hero scene: sun, Sierra ridges, Scots pines, a pile of boulders. Drawn from seeds, so the same every time.
   Used by the page (animated) and by scripts/make_og.js (still, for the share card). */
(function () {
  'use strict';
  var R = window.Rocks, el = R.el;

  /* A pino silvestre: bare trunk, flat crown. */
  function pine(parent, x, y, h, r) {
    var g = el('g', { class: 'pine' }, parent);
    el('rect', { x: x - h * 0.03, y: y - h * 0.62, width: h * 0.06, height: h * 0.62 }, g);
    [[0, -0.74, 0.3, 0.12], [-0.12, -0.66, 0.2, 0.09], [0.13, -0.68, 0.21, 0.09]].forEach(function (c) {
      el('ellipse', { cx: x + c[0] * h * (0.9 + r() * 0.3), cy: y + c[1] * h, rx: c[2] * h, ry: c[3] * h }, g);
    });
  }

  function vulture(parent, x, y, s) {
    return el('path', { d: 'M' + x + ',' + y + 'q' + 7 * s + ',' + -6 * s + ' ' + 14 * s + ',0q' + 7 * s + ',' + -6 * s + ' ' + 14 * s + ',0', class: 'bird' }, parent);
  }

  function draw(svg) {
    var r = R.rng(7);
    var layer = function (k) { return el('g', { class: 'layer', style: '--k:' + k }, svg); };

    var far = layer(6);
    el('circle', { cx: 330, cy: 235, r: 250, fill: 'url(#halo)' }, far);
    el('g', { class: 'sun-wrap' }, far).appendChild(el('circle', { cx: 330, cy: 235, r: 175, class: 'sun' }));
    var clouds = el('g', { class: 'clouds' }, far);
    [[120, 120, 70, 9], [500, 170, 56, 7], [440, 90, 44, 6]].forEach(function (c, i) {
      el('ellipse', { cx: c[0], cy: c[1], rx: c[2], ry: c[3], class: 'cloud', style: '--i:' + i }, clouds);
    });
    vulture(el('g', { class: 'birds b1' }, far), 80, 150, 1);
    vulture(el('g', { class: 'birds b2' }, far), 460, 110, 0.7);

    R.ridge(layer(8), r, 640, 600, 380, 60, 'ridge0');
    R.ridge(layer(12), r, 640, 600, 410, 70, 'ridge1');
    var mid = layer(18);
    R.ridge(mid, r, 640, 600, 462, 56, 'ridge2');
    for (var i = 0; i < 26; i++) { pine(mid, 12 + i * 25 + r() * 14, 470 + r() * 60, 30 + r() * 26, r); }
    el('path', { d: 'M-40,600 L-40,560 Q160,548 320,556 T680,552 L680,600Z', class: 'ground' }, layer(24));

    var fg = layer(34);
    [
      // seed, x, baseY, rx, ry, delay (the bottom of the pile first), tilt
      ['yelmo', 330, 560, 250, 205, 0.1, -3],
      ['left', 150, 448, 92, 66, 0.55, -8],
      ['right', 520, 486, 82, 58, 0.7, 7],
      ['top', 318, 262, 84, 66, 1.05, 9],
      ['cap', 332, 162, 40, 32, 1.35, -12],
      ['f1', 78, 566, 72, 46, 0.9, 6],
      ['f2', 540, 572, 88, 48, 0.8, -5],
      ['f3', 462, 578, 38, 26, 1.2, 10]
    ].forEach(function (o) {
      var big = o[3] > 150;
      var inner = el('g', { class: 'drop', style: '--d:' + o[5] + 's;--r:' + o[6] + 'deg' }, el('g', {}, fg));
      R.rock(inner, { seed: o[0], x: o[1], baseY: o[2], rx: o[3], ry: o[4], n: big ? 16 : 12, jitter: big ? 0.07 : 0.13, cracks: big ? 5 : 2, lichen: big ? 9 : 3, shadow: true });
    });
    [[196, 566, 30], [40, 572, 24], [612, 566, 26], [440, 568, 18]].forEach(function (s, k) {
      var g = el('g', { class: 'drop', style: '--d:' + (1.5 + k * 0.1) + 's' }, fg);
      [[0, 0, 1], [-0.7, 3, 0.8], [0.7, 4, 0.75]].forEach(function (c) { el('circle', { cx: s[0] + c[0] * s[2], cy: s[1] - s[2] * 0.5 + c[1], r: s[2] * c[2], class: 'shrub' }, g); });
    });
    return svg;
  }

  window.Scene = { draw: draw };
})();
