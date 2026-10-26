#!/usr/bin/env node
// A measurement, not a gate (docs/data.md section 6): what `examples/ecosystem` (written from the page's own `data/ecosystem.json`) renders against the lists
// that the page's Python generator writes into `index.html`, in Chromium with JavaScript off (and on for the waffles, which the page draws by script).
//   node scripts/measure-ecosystem.js /path/to/cancho-www
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const www = path.resolve(process.argv[2] || '');
if (!fs.existsSync(path.join(www, 'index.html'))) { console.error('usage: node scripts/measure-ecosystem.js <path of a cancho-www clone>'); process.exit(2); }
const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'eco-'));
const b = spawnSync(BIN, ['build', path.join(ROOT, 'examples', 'ecosystem', 'ecosystem.cui'), '--page', '-o', path.join(work, 'ours')], { encoding: 'utf8' });
if (b.status !== 0) { console.error(b.stderr); process.exit(1); }
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => { if (e) { r.statusCode = 404; r.end(); return; } r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.css': 'text/css' }[path.extname(f)] || 'application/octet-stream'); r.end(d); });
    }).listen(0, () => ok(s));
  });
}
const norm = (s) => s.replace(/\s+/g, ' ').trim();
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
(async () => {
  const [theirs, ours] = [await serve(www), await serve(path.join(work, 'ours'))];
  const browser = await chromium.launch();
  const read = async (srv, js, fn, arg) => {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], javaScriptEnabled: js });
    const page = await ctx.newPage();
    await page.goto(`http://127.0.0.1:${srv.address().port}/`);
    await sleep(js ? 2500 : 300);
    const r = await page.evaluate(fn, arg);
    await ctx.close();
    return r;
  };
  // 1. the list of projects: the page's `<details id="map-fallback">`, ours the `Details`
  const listOf = (sel) => [...document.querySelectorAll(sel)].map((li) => ({ text: li.textContent.replace(/\s+/g, ' ').trim(), links: [...li.querySelectorAll('a')].map((a) => a.getAttribute('href')) }));
  const A = await read(theirs, false, listOf, '#map-fallback li');
  const B = await read(ours, false, listOf, '#ecosystem details > p');
  let same = 0; const diffs = [];
  A.forEach((a, i) => { const o = B[i]; if (o && norm(o.text) === norm(a.text) && JSON.stringify(o.links) === JSON.stringify(a.links)) { same++; } else { diffs.push(`${i}: ${JSON.stringify(a.text).slice(0, 90)} / ${JSON.stringify(o && o.text).slice(0, 90)}`); } });
  console.log(`projects list: page ${A.length} items, ours ${B.length}; identical text and links in ${same}${diffs.length ? '; first difference ' + diffs[0] : ''}`);
  // 2. paths
  const pathsOf = () => [...document.querySelectorAll('.paths .path, #paths article')].map((a) => ({ title: (a.querySelector('h3') || {}).textContent, entries: [...a.querySelectorAll('li, p')].length, links: [...a.querySelectorAll('a')].map((x) => x.getAttribute('href')), chips: [...a.querySelectorAll('.chip, .cui-chip')].map((c) => c.textContent.trim()) }));
  const PA = await read(theirs, false, pathsOf); const PB = await read(ours, false, pathsOf);
  console.log(`paths: page ${PA.length} articles with titles ${JSON.stringify(PA.map((p) => p.title))}; ours ${PB.length} with ${JSON.stringify(PB.map((p) => p.title))}`);
  const same2 = PA.every((p, i) => PB[i] && JSON.stringify(p.links) === JSON.stringify(PB[i].links) && JSON.stringify(p.chips) === JSON.stringify(PB[i].chips));
  console.log(`paths: the links, in order, and the stage chips are the same in all ${PA.length}: ${same2}${same2 ? '' : ` (page links ${PA.map((p) => p.links.length)}, ours ${PB.map((p) => p.links.length)}; chips ${PA.map((p) => p.chips.length)} / ${PB.map((p) => p.chips.length)})`}`);
  // 3. claims and evidence
  const claimsOf = () => { const li = [...document.querySelectorAll('.verif li')]; return { n: li.length, links: li.reduce((n, l) => n + l.querySelectorAll('a').length, 0), code: li.reduce((n, l) => n + l.querySelectorAll('code').length, 0), ticks: li.reduce((n, l) => n + (l.textContent.match(/`/g) || []).length, 0), text: li.map((l) => l.textContent.replace(/\s+/g, ' ').trim()) }; };
  const oursClaims = () => { const ps = [...document.querySelectorAll('#verifies p, #verifies > div p')]; const sec = document.getElementById('verifies'); const rich = [...sec.querySelectorAll('p')]; return { n: rich.length, links: sec.querySelectorAll('a').length, code: sec.querySelectorAll('code').length, ticks: (sec.textContent.match(/`/g) || []).length }; };
  const CA = await read(theirs, false, claimsOf); const CB = await read(ours, false, oursClaims);
  console.log(`claims: page ${CA.n} items with ${CA.links} links and ${CA.code} <code> elements; ours ${CB.n} paragraphs with ${CB.links} links, ${CB.code} <code> elements and ${CB.ticks} backticks left in the text (the page's generator turns each pair of backticks into <code>: ${CA.code} elements)`);
  // 4. structure: list semantics, chips, waffles
  const shape = () => ({ ul: document.querySelectorAll('main ul').length, li: document.querySelectorAll('main li').length, chips: document.querySelectorAll('.chip, .cui-chip').length, h3: document.querySelectorAll('main h3').length });
  const SA = await read(theirs, false, shape); const SB = await read(ours, false, shape);
  console.log(`structure with script off: page ul ${SA.ul}, li ${SA.li}, chips ${SA.chips}, h3 ${SA.h3}; ours ul ${SB.ul}, li ${SB.li}, chips ${SB.chips}, h3 ${SB.h3}`);
  const wf = () => ({ svgs: document.querySelectorAll('.waffle, .cui-waffle svg').length, counted: document.querySelectorAll('.waffle rect[data-pass="1"]').length + (document.querySelectorAll('.cui-wf-on').length ? [...document.querySelectorAll('.cui-wf-on')].reduce((n, p) => n + (p.getAttribute('d').match(/M/g) || []).length, 0) : 0), counts: [...document.querySelectorAll('[data-count] span, .cui-countup')].map((e) => e.textContent) });
  const WA0 = await read(theirs, false, wf); const WA1 = await read(theirs, true, wf); const WB = await read(ours, false, wf);
  console.log(`waffles: page with script off ${JSON.stringify(WA0)}; page with script on ${JSON.stringify(WA1)}; ours with script off ${JSON.stringify(WB)}`);
  await browser.close(); theirs.close(); ours.close();
  fs.rmSync(work, { recursive: true, force: true });
})();
