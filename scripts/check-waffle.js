#!/usr/bin/env node
// `Waffle` and `Chip` in Chromium (docs/waffle.md): DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   A. with JavaScript off, the squares of examples/waffle against an oracle written here from the formulas of the note (the position of each square,
//      the size of the drawing, how many are counted, outlined and hollow), the text alternative, the caption, the table (closed), one accessibility node
//      for the drawing;
//   B. a waffle the script draws (inside an If) is the same HTML as the one the page was written with;
//   C. colour: every kind of square and every tone of chip against the page, in both schemes, with and without a theme (3:1 for a graphic, 4.5:1 for
//      text), and what tells the kinds apart besides colour (fill, stroke width, border style, the words);
//   D. no horizontal scroll at 320 px, no layout shift, no animation, the same nodes after app.js ran, no request to another host, no console error.
//   node scripts/check-waffle.js
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');
const { ratio } = require('./theme-oracle.js');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const EXAMPLE = path.join(ROOT, 'examples', 'waffle');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'waffle-'));
function build(name, file) {
  const out = path.join(work, name);
  const r = spawnSync(BIN, ['build', file, '--page', '-o', out], { encoding: 'utf8', cwd: ROOT });
  if (r.status !== 0) { console.log(`FAIL build ${name}: ${r.stderr}${r.stdout}`); process.exit(1); }
  return out;
}
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, () => ok(s));
  });
}
const PIXEL = devices['Pixel 7'];
// the oracle: the squares of a waffle, from the note's formulas (a square is 14 wide, the gap is 3, row-major)
function oracle(of, counted, columns) {
  const sq = (i) => `M${(i % columns) * 17} ${Math.floor(i / columns) * 17}h14v14h-14z`;
  const on = []; const rest = [];
  for (let i = 0; i < of; i++) { (i < counted ? on : rest).push(sq(i)); }
  return { on: on.join(''), rest: rest.join(''), w: columns * 17 - 3, h: Math.ceil(of / columns) * 17 - 3 };
}
const THEME = `theme {
    light { bg: "#f7f4eb"; fg: "#121417"; muted: "#4f545b"; line: "#e2ddd0"; surface: "#ffffff"; soft: "#efece1"; ink: "#16283b"; accent: "#185fa5"; accent_fg: "#ffffff"; accent_soft: "#ebf3fc"; good: "#0f7b5f"; warn: "#8a4b0a"; warn_bg: "#f9efd9"; warn_line: "#d9a441"; bad: "#b91c1c"; bad_bg: "#fdeceb"; code_bg: "#12151a"; code_fg: "#d6dde8"; }
    dark { bg: "#0b1016"; fg: "#e9ecf1"; muted: "#9aa5b4"; line: "#222b36"; surface: "#121922"; soft: "#172029"; ink: "#0a121a"; accent: "#6ea3ee"; accent_fg: "#0a1220"; accent_soft: "#14233a"; good: "#4fd1a5"; warn: "#f1c577"; warn_bg: "#1c1810"; warn_line: "#7e6126"; bad: "#ff7a72"; bad_bg: "#2a1413"; code_bg: "#12151a"; code_fg: "#d6dde8"; }
}
`;
const hex = (c) => '#' + c.match(/[\d.]+/g).slice(0, 3).map((n) => Math.round(Number(n)).toString(16).padStart(2, '0')).join('');

(async () => {
  const site = build('site', path.join(EXAMPLE, 'waffle.cui'));
  const server = await serve(site);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const themedSrc = path.join(work, 'themed.cui');
  fs.writeFileSync(themedSrc, THEME + fs.readFileSync(path.join(EXAMPLE, 'waffle.cui'), 'utf8'));
  const themed = await serve(build('themed', themedSrc));
  const turl = `http://127.0.0.1:${themed.address().port}/`;
  const browser = await chromium.launch();
  const external = [];
  async function open(base, opts, init) {
    const ctx = await browser.newContext(opts);
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    await page.route('**/*', (r) => { if (r.request().url().startsWith('http://127.0.0.1')) { r.continue(); } else { external.push(r.request().url()); r.abort(); } });
    await page.addInitScript(() => { window.__cls = 0; try { new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__cls += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); } catch (e) { /* none */ } });
    if (init) { await page.addInitScript(init); }
    await page.goto(base);
    await sleep(400);
    return { ctx, page, errors };
  }

  // ---- A. JavaScript off -----------------------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open(url, { ...PIXEL, javaScriptEnabled: false });
    const figs = await page.evaluate(() => [...document.querySelectorAll('figure.cui-waffle')].map((f) => ({
      label: f.querySelector('figcaption').textContent, aria: f.querySelector('svg').getAttribute('aria-label'), vb: f.querySelector('svg').getAttribute('viewBox'),
      paths: [...f.querySelectorAll('path')].map((p) => [p.getAttribute('class'), p.getAttribute('d')]), open: f.querySelector('details').open,
      rows: [...f.querySelectorAll('tbody tr')].map((r) => r.textContent), elements: f.querySelectorAll('*').length,
    })));
    const specs = [[704, 697, 44, 'known', 'Conformance tests that pass', '697 of 704, 7 known differences'], [333, 256, 37, 'open', 'Programs refused with the same rule', '256 of 333, 77 not yet'], [62, 62, 16, 'open', 'Named rules', '62 of 62']];
    check('A three waffles are in the page with no script', figs.length === 3);
    figs.forEach((f, i) => {
      const [of, counted, columns, rest, label, sum] = specs[i];
      const o = oracle(of, counted, columns);
      const got = Object.fromEntries(f.paths.map(([c, d]) => [c, d]));
      check(`A waffle ${i + 1}: every square is where the oracle puts it, counted and not (${of} squares, ${columns} columns)`, got['cui-wf-on'] === o.on && (counted === of ? !(`cui-wf-${rest}` in got) : got[`cui-wf-${rest}`] === o.rest) && f.vb === `0 0 ${o.w} ${o.h}`, `${f.vb} against 0 0 ${o.w} ${o.h}; counted ${(got['cui-wf-on'].match(/M/g) || []).length}`);
      check(`A waffle ${i + 1}: the text alternative, the caption and the closed table`, f.aria === `${label}: ${sum}` && f.label === label && f.open === false && f.rows[0] === `Counted${counted}` && f.rows[f.rows.length - 1] === `Squares${of}`, f.aria);
      check(`A waffle ${i + 1}: a drawing, a caption and a table in ${f.elements} elements, not one per square`, f.elements <= 20, String(f.elements));
    });
    const ax = await page.accessibility.snapshot({ root: await page.$('figure.cui-waffle'), interestingOnly: false });
    const imgs = []; (function w(n) { if (n.role === 'img' || n.role === 'image') { imgs.push(n.name); } (n.children || []).forEach(w); })(ax);
    check('A the accessibility tree has the drawing as one image named by the sentence', imgs.length === 1 && imgs[0] === 'Conformance tests that pass: 697 of 704, 7 known differences', JSON.stringify(imgs));
    check('A no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- B. a waffle the script draws is the same HTML --------------------------------------------------------------------------------------------------
  {
    const f = path.join(work, 'drawn.cui');
    const w = 'Waffle { label: "Conformance tests that pass" of: 704 counted: 697 columns: 44 rest: "known" }';
    fs.writeFileSync(f, `component D {\n state shown: Bool = false;\n on Show { shown = true; }\n view { Column { Text { "Static." } Button { label: "Show" on: Show } ${w} If { when: shown ${w} } } }\n}\n`);
    const ds = await serve(build('drawn', f));
    const { ctx, page } = await open(`http://127.0.0.1:${ds.address().port}/`, { ...PIXEL });
    await page.click('button');
    await sleep(300);
    const htmls = await page.evaluate(() => [...document.querySelectorAll('figure.cui-waffle')].map((e) => e.outerHTML));
    check('B the waffle the script draws is the HTML the page was written with, byte for byte', htmls.length === 2 && htmls[0] === htmls[1], `${htmls.length} figures`);
    const ns = await page.evaluate(() => [...document.querySelectorAll('figure.cui-waffle')].map((e) => e.querySelector('svg').namespaceURI + e.querySelector('path').namespaceURI));
    check('B and in the SVG namespace both times', ns.every((n) => n === 'http://www.w3.org/2000/svghttp://www.w3.org/2000/svg'), JSON.stringify(ns));
    await ctx.close();
    ds.close();
  }

  // ---- C. colour -------------------------------------------------------------------------------------------------------------------------------------
  for (const [name, base] of [['no theme', url], ['theme', turl]]) {
    for (const scheme of ['light', 'dark']) {
      const { ctx, page } = await open(base, { ...PIXEL, colorScheme: scheme });
      const c = await page.evaluate(() => {
        const cs = (e, p) => getComputedStyle(e)[p];
        const on = document.querySelector('.cui-wf-on'); const kn = document.querySelector('.cui-wf-known'); const op = document.querySelector('.cui-wf-open');
        const chips = Object.fromEntries([...document.querySelectorAll('.cui-chip')].map((e) => [e.className.split('cui-chip-')[1], { fg: cs(e, 'color'), bg: cs(e, 'backgroundColor'), border: cs(e, 'borderTopStyle'), text: e.textContent }]));
        return { page: cs(document.body, 'backgroundColor'), canvas: cs(document.documentElement, 'backgroundColor'), on: { fill: cs(on, 'fill'), stroke: cs(on, 'stroke') }, known: { fill: cs(kn, 'fill'), stroke: cs(kn, 'stroke'), w: cs(kn, 'strokeWidth') }, open: { fill: cs(op, 'fill'), stroke: cs(op, 'stroke'), w: cs(op, 'strokeWidth') }, chips };
      });
      const bg = c.page === 'rgba(0, 0, 0, 0)' ? (scheme === 'dark' ? '#121212' : '#ffffff') : hex(c.page);
      const r = { on: ratio(hex(c.on.fill), bg), known: ratio(hex(c.known.stroke), bg), open: ratio(hex(c.open.stroke), bg) };
      check(`C ${name}, ${scheme}: counted, known and open squares are at least 3:1 of the page (${r.on.toFixed(2)}, ${r.known.toFixed(2)}, ${r.open.toFixed(2)})`, r.on >= 3 && r.known >= 3 && r.open >= 3, JSON.stringify(c.on) + JSON.stringify(c.known) + JSON.stringify(c.open));
      if (name === 'theme') { check(`C theme, ${scheme}: the squares are the theme's accent and muted`, hex(c.on.fill) === (scheme === 'light' ? '#185fa5' : '#6ea3ee') && hex(c.open.stroke) === (scheme === 'light' ? '#4f545b' : '#9aa5b4'), `${hex(c.on.fill)} ${hex(c.open.stroke)}`); }
      if (scheme === 'light' && name === 'no theme') {
        check('C the kinds differ by more than colour: counted is filled, known is outlined 2 wide, open is outlined 1 wide', c.on.fill !== 'none' && c.known.fill === 'none' && c.open.fill === 'none' && c.known.w === '2px' && c.open.w === '1px', JSON.stringify([c.on, c.known, c.open]));
      }
      const bad = [];
      for (const [tone, v] of Object.entries(c.chips)) {
        const eff = v.bg === 'rgba(0, 0, 0, 0)' ? bg : hex(v.bg);
        const rr = ratio(hex(v.fg), eff);
        if (rr < 4.5) { bad.push(`${tone} ${rr.toFixed(2)}`); }
      }
      check(`C ${name}, ${scheme}: the four chips are 4.5:1 or more of what is behind them`, Object.keys(c.chips).length === 4 && bad.length === 0, bad.join(', '));
      if (scheme === 'light' && name === 'no theme') { check('C a chip says its message in words, and the quiet one has a dashed border', c.chips.quiet.border === 'dashed' && c.chips.good.border === 'solid' && c.chips.good.text === 'working' && c.chips.quiet.text === 'design', JSON.stringify(c.chips)); }
      await ctx.close();
    }
  }

  // ---- D. the page around them ---------------------------------------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ viewport: { width: 320, height: 640 } });
    const page = await ctx.newPage();
    await page.goto(url);
    await sleep(300);
    const o = await page.evaluate(() => ({ sw: document.documentElement.scrollWidth, cw: document.documentElement.clientWidth }));
    check('D no horizontal scroll at 320 px', o.sw <= o.cw, JSON.stringify(o));
    await ctx.close();
  }
  {
    const { ctx, page, errors } = await open(url, { ...PIXEL, reducedMotion: 'no-preference' });
    await page.evaluate(async () => { for (let y = 0; y < document.documentElement.scrollHeight; y += 200) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 20)); } });
    const r = await page.evaluate(() => ({ cls: window.__cls, anim: document.getAnimations().filter((a) => a.playState === 'running' && !(a.effect && a.effect.target && a.effect.target.closest && a.effect.target.closest('.cui-countup'))).length }));
    check('D the layout shifts by 0 and no animation runs of the waffles and chips', r.cls === 0 && r.anim === 0, JSON.stringify(r));
    check('D no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }
  {
    const holdSrv = await serve(site);
    const ctx = await browser.newContext(PIXEL);
    const page = await ctx.newPage();
    await page.goto(`http://127.0.0.1:${holdSrv.address().port}/`);
    await page.evaluate(() => { window.__n = [...document.querySelectorAll('figure.cui-waffle, .cui-chip, svg, path')]; });
    await sleep(800);
    const same = await page.evaluate(() => window.__n.length === 16 && window.__n.every((n) => n.isConnected));
    check('D the figures, drawings, paths and chips are the same nodes after app.js ran', same);
    holdSrv.close();
    await ctx.close();
  }
  check('no request went to another host', external.length === 0, external.join(', '));
  await browser.close();
  server.close();
  themed.close();
  clearTimeout(watchdog);
  fs.rmSync(work, { recursive: true, force: true });
  if (failures.length) { console.log(`FAIL ${failures.length}: ${failures.join('; ')}`); process.exit(1); }
  console.log('PASS');
})();
