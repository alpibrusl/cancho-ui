#!/usr/bin/env node
// A measurement, not a gate (docs/prerender.md section 13): what a visitor does to the page before its script runs, and what is left after it. Run
// against one or more compilers, to compare them on the same program:
//   node scripts/measure-hydration.js label=path/to/cancho-ui [label=path ..]
// For each compiler the program is built with `build --page`, the size of `app.js` and `index.html` is read, the script is held back 1.5 s, and
// before it runs the visitor selects a sentence, opens a `details`, scrolls a code box and focuses a link. After the script, and after a click that
// changes the state, the table says what is left of each, and whether the static nodes are the very nodes the page had (`===`).
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const lines = Array.from({ length: 40 }, (_, i) => `line ${i + 1} of the code box`).join('\\n');
const program = `component Page {
    state n: Int = 0;
    on Up { n = n + 1; }
    view {
        Column {
            Heading { level: 1 "Hydration" }
            Rich { "Select " Strong { "this sentence" } " before the script runs." }
            Details { summary: "More" Text { "Hidden text." } }
            Code { max_height: 4 "${lines}" }
            Link { to: "https://a.example/" "A link" }
            Button { label: "Up" on: Up }
            Text { "Count: {n}" }
        }
    }
}
`;
const watchdog = setTimeout(() => { console.log('FAIL watchdog'); process.exit(1); }, 5 * 60 * 1000);
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const compilers = process.argv.slice(2).map((a) => { const [label, bin] = a.split('='); return { label, bin: path.resolve(bin) }; });
if (!compilers.length) { console.error('usage: node scripts/measure-hydration.js label=path/to/cancho-ui [label=path ..]'); process.exit(2); }

(async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'hydration-'));
  fs.writeFileSync(path.join(work, 'p.cui'), program);
  const browser = await chromium.launch();
  let hold = 0;
  const server = http.createServer((q, r) => {
    let f = path.join(work, 'site', decodeURIComponent(q.url.split('?')[0]));
    if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
    fs.readFile(f, (e, d) => {
      if (e) { r.statusCode = 404; r.end(); return; }
      r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript' }[path.extname(f)] || 'application/octet-stream');
      if (hold && f.endsWith('app.js')) { setTimeout(() => r.end(d), hold); } else { r.end(d); }
    });
  });
  await new Promise((ok) => server.listen(0, ok));
  const base = `http://localhost:${server.address().port}`;
  const rows = [];
  for (const c of compilers) {
    const out = path.join(work, 'site', c.label);
    const b = spawnSync(c.bin, ['build', path.join(work, 'p.cui'), '--page', '-o', out], { encoding: 'utf8' });
    if (b.status !== 0) { console.log(`FAIL ${c.label}: ${b.stderr}`); process.exit(1); }
    const bytes = { js: fs.statSync(path.join(out, 'app.js')).size, html: fs.statSync(path.join(out, 'index.html')).size };
    hold = 1500;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 412, height: 700 }, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.addInitScript(() => { window.__shifts = 0; new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__shifts += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); });
    await page.goto(`${base}/${c.label}/`, { waitUntil: 'commit' });
    await page.waitForSelector('h1');
    const before = await page.evaluate(() => {
      const root = document.getElementById('root');
      const p = root.querySelector('p');
      const range = document.createRange();
      range.selectNodeContents(p);
      const sel = getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
      const details = root.querySelector('details');
      details.open = true;
      const pre = root.querySelector('pre');
      pre.scrollTop = 40;
      root.querySelector('a').focus();
      window.__static = [...root.querySelectorAll('h1, p, strong, details, summary, pre, code, a')];
      return { buttons: root.querySelectorAll('button').length, selected: String(sel), scroll: pre.scrollTop, nodes: window.__static.length };
    });
    await page.waitForFunction(() => document.querySelectorAll('#root button').length > 0, null, { timeout: 8000 });
    await sleep(300);
    const probe = () => page.evaluate(() => {
      const root = document.getElementById('root');
      const now = [...root.querySelectorAll('h1, p, strong, details, summary, pre, code, a')];
      return {
        selected: String(getSelection()),
        open: root.querySelector('details').open,
        scroll: root.querySelector('pre').scrollTop,
        focus: document.activeElement === root.querySelector('a'),
        same: now.length === window.__static.length && now.every((n, i) => n === window.__static[i]),
        count: ([...root.querySelectorAll('span')].find((x) => x.textContent.startsWith('Count:')) || { textContent: '' }).textContent,
        shifts: Math.round(window.__shifts * 1000) / 1000,
      };
    });
    const afterScript = await probe();
    await page.locator('#root button').click();
    await sleep(200);
    const afterClick = await probe();
    rows.push({ label: c.label, bytes, before, afterScript, afterClick });
    await ctx.close();
    hold = 0;
  }
  await browser.close();
  server.close();
  fs.rmSync(work, { recursive: true, force: true });
  clearTimeout(watchdog);
  console.log('| compiler | app.js | index.html | after the script: selection | details open | pre scroll | link focus | static nodes the same | after a click: details open | pre scroll | static nodes the same | count | layout shift |');
  console.log('|---|---:|---:|---|---|---:|---|---|---|---:|---|---|---:|');
  for (const r of rows) {
    const a = r.afterScript, k = r.afterClick;
    console.log(`| ${r.label} | ${r.bytes.js} | ${r.bytes.html} | ${a.selected === r.before.selected ? 'kept' : 'LOST'} | ${a.open ? 'kept' : 'LOST'} | ${a.scroll} | ${a.focus ? 'kept' : 'LOST'} | ${a.same ? 'yes' : 'NO'} | ${k.open ? 'kept' : 'LOST'} | ${k.scroll} | ${k.same ? 'yes' : 'NO'} | ${k.count} | ${k.shifts} |`);
  }
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
