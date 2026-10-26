#!/usr/bin/env node
// A measurement, not a gate (docs/waffle.md section 1): what a waffle of the real page's size costs written as one <rect> per cell (as the page draws it)
// and as one <path> per state, in Chromium. The three waffles of the cancho-www proof section: 704 cells of 44 columns, 333 of 37, 62 of 16.
//   node scripts/measure-waffle.js
'use strict';
const zlib = require('zlib');
const { chromium } = require('playwright');

const CELL = 14, GAP = 3, R = 3;
const SPECS = [[704, 697, 44, 'known'], [333, 256, 37, 'empty'], [62, 62, 16, 'empty']];
function pos(i, cols) { return [(i % cols) * (CELL + GAP), Math.floor(i / cols) * (CELL + GAP)]; }
function rects(total, passed, cols, rest) {
  const w = cols * (CELL + GAP) - GAP, h = Math.ceil(total / cols) * (CELL + GAP) - GAP;
  let s = `<svg viewBox="0 0 ${w} ${h}" role="img" aria-label="${passed} of ${total}">`;
  for (let i = 0; i < total; i++) {
    const [x, y] = pos(i, cols);
    s += `<rect x="${x}" y="${y}" width="${CELL}" height="${CELL}" rx="${R}" class="${i < passed ? 'on' : rest === 'known' ? 'known' : 'off'}"/>`;
  }
  return s + '</svg>';
}
function square(x, y) { return `M${x + R} ${y}h${CELL - 2 * R}a${R} ${R} 0 0 1 ${R} ${R}v${CELL - 2 * R}a${R} ${R} 0 0 1 -${R} ${R}h-${CELL - 2 * R}a${R} ${R} 0 0 1 -${R} -${R}v-${CELL - 2 * R}a${R} ${R} 0 0 1 ${R} -${R}z`; }
function compact(x, y) { return `M${x} ${y}h${CELL}v${CELL}h-${CELL}z`; }
function pathsWith(sq) {
  return function (total, passed, cols, rest) {
  const w = cols * (CELL + GAP) - GAP, h = Math.ceil(total / cols) * (CELL + GAP) - GAP;
  const d = { on: '', known: '', off: '' };
  for (let i = 0; i < total; i++) { const [x, y] = pos(i, cols); d[i < passed ? 'on' : rest === 'known' ? 'known' : 'off'] += sq(x, y); }
  let s = `<svg viewBox="0 0 ${w} ${h}" role="img" aria-label="${passed} of ${total}">`;
  for (const k of ['on', 'known', 'off']) { if (d[k]) { s += `<path class="${k}" d="${d[k]}"/>`; } }
  return s + '</svg>';
  };
}
const paths = pathsWith(square);
const squares = pathsWith(compact);
// The colours the page's waffle cells are drawn in, against the page's background (palette of examples/theme, which is the page's `:root` and dark override).
function contrasts() {
  const { ratio } = require('./theme-oracle.js');
  const pal = { light: { bg: '#f7f4eb', accent: '#185fa5', warn_line: '#d9a441', line: '#e2ddd0', soft: '#efece1', muted: '#4f545b', warn: '#8a4b0a' }, dark: { bg: '#0b1016', accent: '#6ea3ee', warn_line: '#7e6126', line: '#222b36', soft: '#172029', muted: '#9aa5b4', warn: '#f1c577' } };
  for (const sch of ['light', 'dark']) {
    const p = pal[sch];
    console.log(`${sch}: the page's cells against bg: passed (accent) ${ratio(p.accent, p.bg).toFixed(2)}, known difference outline (warn_line) ${ratio(p.warn_line, p.bg).toFixed(2)}, empty cell stroke (line) ${ratio(p.line, p.bg).toFixed(2)}, empty cell fill (soft) ${ratio(p.soft, p.bg).toFixed(2)}; with text tokens instead: muted ${ratio(p.muted, p.bg).toFixed(2)}, warn ${ratio(p.warn, p.bg).toFixed(2)} (WCAG 1.4.11 asks 3:1 of a graphic that carries meaning)`);
  }
}
(async () => {
  contrasts();
  const browser = await chromium.launch();
  const css = '<style>svg{width:100%;height:auto;display:block}.on{fill:#185fa5}.off{fill:none;stroke:#4f545b;stroke-width:1}.known{fill:none;stroke:#8a4b0a;stroke-width:2}</style>';
  for (const [name, f] of [['one <rect> per cell', rects], ['one <path> per state, rounded corners', paths], ['one <path> per state, square corners', squares]]) {
    const html = SPECS.map((s) => f(...s)).join('\n');
    const doc = `<!doctype html><meta name=viewport content="width=device-width"><body>${css}${html}`;
    const ctx = await browser.newContext({ viewport: { width: 412, height: 800 } });
    const page = await ctx.newPage();
    const times = [];
    for (let i = 0; i < 15; i++) {
      await page.setContent('<!doctype html><body></body>');
      times.push(await page.evaluate((d) => { const t = performance.now(); document.body.innerHTML = d; document.body.getBoundingClientRect(); document.querySelectorAll('svg').forEach((s) => s.getBoundingClientRect()); return performance.now() - t; }, css + html));
    }
    times.sort((a, b) => a - b);
    const nodes = await page.evaluate(() => document.body.querySelectorAll('*').length);
    await page.setContent(doc);
    const ax = await page.accessibility.snapshot({ interestingOnly: false });
    let n = 0; (function w(x) { n++; (x.children || []).forEach(w); })(ax);
    console.log(`${name}: ${html.length} bytes, ${zlib.gzipSync(html).length} gzipped, ${nodes} elements, ${n} accessibility nodes, parse + layout median ${times[7].toFixed(1)} ms`);
    await ctx.close();
  }
  await browser.close();
})();
