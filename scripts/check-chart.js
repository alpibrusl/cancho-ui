#!/usr/bin/env node
// `Chart` (docs/charts.md, issue #77) in Chromium, in the mobile profile (Pixel 7, 412 px):
//  A. tests/chart_corpus.json, the file `cargo test` replays (crates/cancho-ui/tests/chart.rs): for each case the SVG's accessible name is
//     "<label>: <summary>" with exactly the summary replay reports, the caption is the label, and the geometry (the polyline's points,
//     each bar's x, y, width and height, the y ticks, the x labels) agrees with an ORACLE below that is written from the formulas of
//     docs/charts.md section 3 and shares no code with the page; the one case the corpus also gives by hand ([1, 2, 3]) is compared exactly;
//  B. the limits: 1,000 values are drawn, 1,500 draw the last 1,000, the data table lists the last 200, is collapsed and opens from the keyboard;
//  C. markup in a label stays text; D. contrast of the series colour (3:1), the text (4.5:1), in light and dark; E. 412, 360 and 320 px wide:
//     no horizontal scroll and a label no smaller than 10 px; F. the accessibility tree (an image named by the label).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-chart.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

// ---- the oracle: docs/charts.md section 3, written again here ----
function oracle(kind, all) {
  const vals = all.slice(Math.max(0, all.length - 1000));
  const n = vals.length;
  const out = { n, ticks: [], points: [], rects: [] };
  if (n === 0) { return out; }
  const lowest = Math.min(...vals), highest = Math.max(...vals);
  let lo, hi;
  if (kind === 'bar') { lo = Math.min(0, lowest); hi = Math.max(0, highest); if (lo === hi) { hi = lo + 1; } }
  else if (lowest === highest) { lo = lowest - 1; hi = highest + 1; }
  else { lo = lowest - 0.05 * (highest - lowest); hi = highest + 0.05 * (highest - lowest); }
  const raw = (hi - lo) / 4;
  let step = Infinity;
  for (let k = -12; k <= 12; k++) { for (const m of [1, 2, 5]) { const s = m * Math.pow(10, k); if (s >= raw * (1 - 1e-12) && s < step) { step = s; } } }
  for (let i = Math.ceil(lo / step - 1e-9); i <= Math.floor(hi / step + 1e-9); i++) { out.ticks.push(i * step); }
  const Y = (v) => 152 - ((v - lo) / (hi - lo)) * 142;
  if (kind === 'bar') {
    const slot = 270 / n;
    vals.forEach((v, i) => { out.rects.push([40 + i * slot + 0.15 * slot, Math.min(Y(v), Y(0)), 0.7 * slot, Math.abs(Y(v) - Y(0))]); });
  } else {
    vals.forEach((v, i) => { out.points.push([n === 1 ? 175 : 40 + (270 * i) / (n - 1), Y(v)]); });
  }
  return out;
}
const near = (a, b) => Math.abs(a - b) <= 0.011;
function roundText(x) { const r = Math.round(x * 100) / 100; return String(r); }

function lum(rgb) {
  const [r, g, b] = rgb.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function ratio(a, b) { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); }
function rgbOf(s) { const m = s.match(/\d+(\.\d+)?/g); return m.slice(0, 3).map(Number); }

(async () => {
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'chart_corpus.json'), 'utf8'));
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-chart-'));
  fs.writeFileSync(path.join(tmp, 'p.cui'), corpus.program.join('\n') + '\n');
  const r = spawnSync(CANCHO_UI, ['build', path.join(tmp, 'p.cui'), '--page', '-o', path.join(tmp, 'out'), '--name', 'charts'], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stdout + r.stderr); }
  const server = await new Promise((ok) => {
    const types = { '.html': 'text/html', '.js': 'text/javascript' };
    const s = http.createServer((req, res) => {
      const f = path.join(tmp, 'out', req.url.split('?')[0] === '/' ? 'index.html' : path.normalize(req.url.split('?')[0]));
      if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': types[path.extname(f)] || 'text/plain', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const open = async (opts = {}) => {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block', ...opts });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(`pageerror ${e}`));
    page.on('console', (m) => { if (m.type() === 'error') { problems.push(`console ${m.text()}`); } });
    await page.goto(url);
    await page.waitForSelector('#root figure');
    return { ctx, page, problems };
  };
  // everything about the two charts, read from the page
  const read = (page) => page.evaluate(() => {
    return Array.from(document.querySelectorAll('#root figure')).map((fig) => {
      const svg = fig.querySelector('svg');
      const pl = svg.querySelector('polyline');
      const texts = Array.from(svg.querySelectorAll('text')).map((t) => ({ s: t.textContent, x: +t.getAttribute('x'), y: +t.getAttribute('y'), a: t.getAttribute('text-anchor') }));
      const det = fig.querySelector('details');
      return {
        aria: svg.getAttribute('aria-label'), role: svg.getAttribute('role'), caption: (fig.querySelector('figcaption') || {}).textContent,
        points: pl ? pl.getAttribute('points') : null,
        rects: Array.from(svg.querySelectorAll('rect')).map((r) => ['x', 'y', 'width', 'height'].map((k) => +r.getAttribute(k))),
        texts, hasDetails: !!det, open: det ? det.open : null, summaryText: det ? det.querySelector('summary').textContent : null,
        rows: det ? det.querySelectorAll('tbody tr').length : 0, tableCaption: det && det.querySelector('caption') ? det.querySelector('caption').textContent : null,
        firstRow: det && det.querySelector('tbody tr') ? Array.from(det.querySelector('tbody tr').children).map((c) => c.textContent) : null,
        lastRow: det && det.querySelector('tbody tr:last-child') ? Array.from(det.querySelector('tbody tr:last-child').children).map((c) => c.textContent) : null,
      };
    });
  });

  // ---- A, B, C: the corpus ----
  {
    const t = await open();
    const { page } = t;
    let idx = 0;
    for (const k of corpus.cases) {
      if (k.button) { await page.getByRole('button', { name: k.button, exact: true }).tap(); await page.waitForTimeout(60); }
      const [line, bar] = await read(page);
      const tag = `case ${idx++} (${k.why})`;
      check(`${tag}: the line chart's accessible name`, line.role === 'img' && line.aria === `${corpus.labelLine}: ${k.line_summary}`, `${line.aria} vs ${corpus.labelLine}: ${k.line_summary}`);
      check(`${tag}: the bar chart's accessible name`, bar.aria === `${corpus.labelBar}: ${k.bar_summary}`, bar.aria);
      check(`${tag}: the captions are the labels`, line.caption === corpus.labelLine && bar.caption === corpus.labelBar);
      const n = Math.min(k.values.length, 1000);
      // geometry against the oracle
      const ol = oracle('line', k.values), ob = oracle('bar', k.values);
      let geo = true, why = '';
      if (n > 0) {
        const pts = line.points.split(' ').map((p) => p.split(',').map(Number));
        geo = pts.length === n && pts.every((p, i) => near(p[0], ol.points[i][0]) && near(p[1], ol.points[i][1]));
        if (!geo) { why = `line ${line.points.slice(0, 80)}`; }
        const okBars = bar.rects.length === n && bar.rects.every((r, i) => r.every((v, j) => near(v, ob.rects[i][j])));
        if (!okBars) { geo = false; why += ` bars ${JSON.stringify(bar.rects.slice(0, 2))} vs ${JSON.stringify(ob.rects.slice(0, 2))}`; }
        const ticks = (chart) => chart.texts.filter((x) => x.x === 36).map((x) => x.s);
        const wantL = ol.ticks.map(roundText), wantB = ob.ticks.map(roundText);
        if (JSON.stringify(ticks(line)) !== JSON.stringify(wantL)) { geo = false; why += ` line ticks ${ticks(line)} vs ${wantL}`; }
        if (JSON.stringify(ticks(bar)) !== JSON.stringify(wantB)) { geo = false; why += ` bar ticks ${ticks(bar)} vs ${wantB}`; }
      } else {
        geo = line.points === null && line.rects.length === 0 && line.texts.some((x) => x.s === 'No values');
      }
      check(`${tag}: geometry agrees with the oracle`, geo, why);
      // x labels: first, middle, last (positions when there are no labels)
      if (n > 0) {
        const xs = line.texts.filter((x) => x.y === 170).map((x) => x.s);
        const nameOf = (i) => { const kk = k.values.length - n + i; return k.labels[kk] !== undefined ? k.labels[kk] : String(kk + 1); };
        const want = n === 1 ? [nameOf(0)] : n === 2 ? [nameOf(0), nameOf(1)] : [nameOf(0), nameOf(Math.floor((n - 1) / 2)), nameOf(n - 1)];
        check(`${tag}: the x labels are the first, middle and last`, JSON.stringify(xs) === JSON.stringify(want), JSON.stringify(xs) + ' vs ' + JSON.stringify(want));
        const rows = Math.min(n, 200);
        check(`${tag}: the data table is collapsed and lists ${rows} rows`, line.hasDetails && line.open === false && line.rows === rows && line.summaryText === 'Data table', JSON.stringify({ open: line.open, rows: line.rows }));
        if (n > 200) { check(`${tag}: the table says it is the last 200`, line.tableCaption === `The last 200 of ${n} values.`, line.tableCaption); }
      } else {
        check(`${tag}: no table for no values`, !line.hasDetails);
      }
    }
    // the case given by hand
    await page.getByRole('button', { name: 'case 2', exact: true }).tap();
    await page.waitForTimeout(60);
    const [l2, b2] = await read(page);
    const hand = corpus.hand_geometry_case_2;
    check('by hand [1, 2, 3]: the line\'s points are exactly the numbers worked out from the formulas', l2.points === hand.line_points, l2.points);
    check('by hand [1, 2, 3]: the bars are exactly those numbers', JSON.stringify(b2.rects) === JSON.stringify(hand.bars), JSON.stringify(b2.rects));
    check('by hand [1, 2, 3]: the y ticks', JSON.stringify(l2.texts.filter((x) => x.x === 36).map((x) => x.s)) === JSON.stringify(hand.line_ticks) && JSON.stringify(b2.texts.filter((x) => x.x === 36).map((x) => x.s)) === JSON.stringify(hand.bar_ticks));
    // B. the limits and the cases with the most values
    const big = corpus.cases.findIndex((c) => c.why.startsWith('1500'));
    await page.getByRole('button', { name: `case ${big}`, exact: true }).tap();
    await page.waitForTimeout(100);
    const [l15, b15] = await read(page);
    check('1,500 values: the line has 1,000 points and the bars 1,000 rects', l15.points.split(' ').length === 1000 && b15.rects.length === 1000);
    check('1,500 values: the first row of the table is position 1,301 (the last 200 of the last 1,000), the last is 1,500', l15.firstRow[0] === '1301' && l15.lastRow[0] === '1500', JSON.stringify([l15.firstRow, l15.lastRow]));
    check('1,500 values: the value has its unit in the table', /°C$/.test(l15.lastRow[1]) && !/°C/.test(b15.lastRow[1]), JSON.stringify([l15.lastRow, b15.lastRow]));
    // the table opens from the keyboard
    await page.locator('#root figure').first().locator('summary').focus();
    await page.keyboard.press('Enter');
    await page.waitForTimeout(40);
    check('the data table opens with Enter on its summary', (await read(page))[0].open === true);
    await page.keyboard.press('Space');
    await page.waitForTimeout(40);
    check('and closes with Space', (await read(page))[0].open === false);
    const sbox = await page.locator('#root figure summary').first().boundingBox();
    check('the summary is at least 44 px high (touch target)', sbox && sbox.height >= 44, JSON.stringify(sbox));
    // C. markup stays text
    const mk = corpus.cases.findIndex((c) => c.why.startsWith('labels made of markup'));
    await page.getByRole('button', { name: `case ${mk}`, exact: true }).tap();
    await page.waitForTimeout(60);
    const safe = await page.evaluate(() => ({ script: window.__x === undefined, onerror: window.__y === undefined, imgs: document.querySelectorAll('img').length, scripts: document.querySelectorAll('#root script').length }));
    const [lm] = await read(page);
    check('markup in labels: nothing ran and nothing was created', safe.script && safe.onerror && safe.imgs === 0 && safe.scripts === 0, JSON.stringify(safe));
    check('markup in labels: it is in the table and on the axis as text', lm.firstRow[0] === '<script>window.__x=1</script>' && lm.texts.some((x) => x.s === '"><img src=x onerror=window.__y=1>' || x.s === '<script>window.__x=1</script>'), JSON.stringify(lm.firstRow));
    // F. the accessibility tree
    await page.getByRole('button', { name: 'case 2', exact: true }).tap();
    await page.waitForTimeout(60);
    const cdp = await t.ctx.newCDPSession(page);
    await cdp.send('Accessibility.enable');
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    const imgs = nodes.filter((x) => x.role && x.role.value === 'image' && !x.ignored);
    check('a11y: each chart is one image named by its label and summary', imgs.length === 2 && imgs[0].name.value === `Temperature: ${corpus.cases[2].line_summary}`, JSON.stringify(imgs.map((x) => x.name.value)));
    const svgKids = nodes.filter((x) => !x.ignored && x.role && ['graphics-symbol', 'polyline', 'StaticText'].includes(x.role.value) && /^[\d.]+$/.test((x.name && x.name.value) || '')).length;
    check('a11y: the axis numbers inside the image are not read separately', svgKids === 0, String(svgKids));
    check('corpus: no page or console error', t.problems.length === 0, t.problems.join('; '));
    await t.ctx.close();
  }

  // ---- D. contrast, light and dark ----
  for (const scheme of ['light', 'dark']) {
    const t = await open({ colorScheme: scheme });
    const { page } = t;
    await page.getByRole('button', { name: 'case 2', exact: true }).tap();
    await page.waitForTimeout(60);
    await page.locator('#root figure details summary').first().focus();
    await page.keyboard.press('Enter');
    const c = await page.evaluate(() => {
      const probe = document.createElement('div');
      probe.style.cssText = 'background:Canvas;color:CanvasText';
      document.body.appendChild(probe);
      const canvas = getComputedStyle(probe).backgroundColor;
      const figs = document.querySelectorAll('#root figure');
      return {
        canvas, line: getComputedStyle(figs[0].querySelector('polyline')).stroke, bar: getComputedStyle(figs[1].querySelector('rect')).fill,
        axisText: getComputedStyle(figs[0].querySelector('text')).fill, table: getComputedStyle(figs[0].querySelector('td')).color,
        axisLine: getComputedStyle(figs[0].querySelector('svg > line:last-of-type')).stroke,
      };
    });
    const bg = rgbOf(c.canvas);
    const r1 = ratio(rgbOf(c.line), bg), r2 = ratio(rgbOf(c.bar), bg), r3 = ratio(rgbOf(c.axisText), bg), r4 = ratio(rgbOf(c.table), bg);
    check(`${scheme}: the series colour on the page is at least 3:1 (line ${r1.toFixed(2)}, bars ${r2.toFixed(2)})`, r1 >= 3 && r2 >= 3, JSON.stringify(c));
    check(`${scheme}: the axis text and the table text are at least 4.5:1 (${r3.toFixed(2)}, ${r4.toFixed(2)})`, r3 >= 4.5 && r4 >= 4.5);
    check(`${scheme}: the series colour is the stylesheet's (${scheme === 'light' ? '#0550ae' : '#79c0ff'})`, c.line.replace(/\s/g, '') === (scheme === 'light' ? 'rgb(5,80,174)' : 'rgb(121,192,255)'), c.line);
    await t.ctx.close();
  }

  // ---- E. narrow screens ----
  {
    const t = await open();
    const { page } = t;
    await page.getByRole('button', { name: 'case 14', exact: true }).tap();
    for (const w of [412, 360, 320]) {
      await page.setViewportSize({ width: w, height: 800 });
      await page.waitForTimeout(80);
      const m = await page.evaluate(() => { const s = document.querySelector('#root svg').getBoundingClientRect(); return { svgW: s.width, over: document.documentElement.scrollWidth > innerWidth, font: 12 * s.width / 320 }; });
      check(`${w} px: no horizontal scroll, the chart fits (${Math.round(m.svgW)} px wide) and a label shows at ${m.font.toFixed(1)} px (at least 10)`, !m.over && m.svgW <= w && m.font >= 10, JSON.stringify(m));
    }
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.waitForTimeout(80);
    const wide = await page.evaluate(() => document.querySelector('#root svg').getBoundingClientRect().width);
    check('1280 px: the chart stops at 40rem (640 px)', wide <= 641, String(wide));
    check('no page or console error', t.problems.length === 0, t.problems.join('; '));
    await t.ctx.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
