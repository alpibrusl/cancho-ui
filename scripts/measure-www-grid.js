#!/usr/bin/env node
// A measurement, not a gate (docs/structure.md section 10.4): does `Grid { min }` and `Grid { min weights }` give the columns that three
// grids of the cancho-www home page have? The page is read, never changed: pass the path of a clone.
//   node scripts/measure-www-grid.js /path/to/cancho-www
// For each width the page's grid is measured (the width of its content box, its column gap, its columns: the distinct left edges), then a Grid of ours is put in a box of the same width with the
// same gap and measured the same way. Prints a table and the number of widths at which the column count differs.
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const www = path.resolve(process.argv[2] || '');
if (!fs.existsSync(path.join(www, 'index.html'))) { console.error('usage: node scripts/measure-www-grid.js <path of a cancho-www clone>'); process.exit(2); }
const watchdog = setTimeout(() => { console.log('FAIL watchdog'); process.exit(1); }, 5 * 60 * 1000);

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'wwwgrid-'));
const prog = `component C {
    view {
        Column {
            Grid { min: 19 gap: 3 weights: [1, 1.75] Column { Text { "a" } } Column { Text { "b" } } }
            Grid { min: 22 gap: 3 Column { Text { "a" } } Column { Text { "b" } } }
            Grid { min: 17 gap: 3 Column { Text { "a" } } Column { Text { "b" } } Column { Text { "c" } } }
        }
    }
}
`;
fs.writeFileSync(path.join(work, 'g.cui'), prog);
const b = spawnSync(BIN, ['build', path.join(work, 'g.cui'), '--page', '-o', path.join(work, 'ours')], { encoding: 'utf8' });
if (b.status !== 0) { console.error(b.stderr); process.exit(1); }

function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, () => ok(s));
  });
}

(async () => {
  const [sw, so] = [await serve(www), await serve(path.join(work, 'ours'))];
  const browser = await chromium.launch();
  const live = await (await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block', viewport: { width: 1024, height: 900 } })).newPage();
  const ours = await (await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block', viewport: { width: 1024, height: 900 } })).newPage();
  await live.goto(`http://localhost:${sw.address().port}/`);
  await ours.goto(`http://localhost:${so.address().port}/`);
  await ours.waitForSelector('.cui-grid');
  const widths = [320, 360, 375, 412, 480, 600, 768, 900, 940, 949, 950, 959, 960, 1024, 1280, 1440];
  const measure = (sel) => `(() => { const g = document.querySelector(${JSON.stringify(sel)}); const cs = getComputedStyle(g); return { W: g.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight), gap: parseFloat(cs.columnGap) || 0, kids: [...g.children].map((c) => { const r = c.getBoundingClientRect(); return { left: Math.round(r.left * 10) / 10, top: Math.round(r.top), w: Math.round(r.width * 10) / 10 }; }) }; })()`;
  const grids = [['.try article', 0, 'try: 1fr 1.75fr from 60rem'], ['.videos', 1, 'videos: auto-fit 22rem'], ['.gallery', 2, 'gallery: auto-fit 17rem']];
  let different = 0;
  const widthDiff = {};
  for (const w of widths) {
    await live.setViewportSize({ width: w, height: 900 });
    await ours.setViewportSize({ width: w, height: 900 });
    await live.waitForTimeout(150);
    for (const [sel, i, label] of grids) {
      const l = await live.evaluate(measure(sel));
      // ours: the same box width and gap
      await ours.evaluate(([i, W, gap]) => { const g = document.querySelectorAll('.cui-grid')[i]; g.parentElement.style.width = '100%'; g.style.width = W + 'px'; g.style.gap = gap + 'px'; g.style.maxWidth = 'none'; }, [i, l.W, l.gap]);
      const o = await ours.evaluate((i) => { const g = document.querySelectorAll('.cui-grid')[i]; return { W: g.getBoundingClientRect().width, kids: [...g.children].map((c) => { const r = c.getBoundingClientRect(); return { left: Math.round(r.left * 10) / 10, top: Math.round(r.top), w: Math.round(r.width * 10) / 10 }; }) }; }, i);
      const cols = (m) => new Set(m.kids.map((k) => Math.round(k.left))).size;
      const same = cols(l) === cols(o);
      if (!same) { different += 1; }
      const dw = Math.max(...l.kids.map((k, j) => Math.abs(k.w - o.kids[j].w)));
      widthDiff[label] = Math.max(widthDiff[label] || 0, same ? dw : 0);
      console.log(`${String(w).padStart(5)}  ${label.padEnd(30)} box ${l.W.toFixed(0).padStart(5)}  gap ${l.gap.toFixed(1).padStart(5)}  page ${cols(l)} col ${l.kids.map((k) => k.w.toFixed(0)).join('+').padEnd(14)}  ours ${cols(o)} col ${o.kids.map((k) => k.w.toFixed(0)).join('+').padEnd(14)} ${same ? '' : '<-- differs'}`);
    }
  }
  console.log(`\ncolumn count differs at ${different} of ${widths.length * grids.length} (width, grid) pairs`);
  for (const [k, v] of Object.entries(widthDiff)) { console.log(`largest difference in a column's width, where the counts agree: ${k}: ${v.toFixed(1)} px`); }
  await browser.close(); sw.close(); so.close();
  fs.rmSync(work, { recursive: true, force: true });
  clearTimeout(watchdog);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
