#!/usr/bin/env node
// Browser-side checks of `Table { sticky: .. max_height: .. }` and `Text { numeric: true }` (docs/table-static.md, section 5). Outside the gate: needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-table-static.js [path/to/cancho-ui] [--src file.cui]   (--src: a mutant's source instead of the generated one)
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const args = process.argv.slice(2);
const srcAt = args.indexOf('--src');
const SRC = srcAt >= 0 ? args.splice(srcAt, 2)[1] : null;
const CANCHO_UI = args[0] || path.join(ROOT, 'target', 'release', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail !== undefined && !ok ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

// 40 rows and 6 columns: wider and taller than the box, so both scrolls are real.
function generated() {
  const head = Array.from({ length: 6 }, (_, c) => `Text { role: columnheader${c > 0 ? ' numeric: true' : ''} "c${c}" }`).join(' ');
  const rows = Array.from({ length: 40 }, (_, r) => `Row { ${Array.from({ length: 6 }, (_, c) => (c === 0 ? `Text { "r${r}" }` : `Text { numeric: true "${(r * 7919 + c * 104729) % 100000}.${(r + c) % 100}" }`)).join(' ')} }`).join('\n            ');
  return `component T {\n    view {\n        Table {\n            label: "T"\n            sticky: "both"\n            max_height: 10\n            Row { ${head} }\n            ${rows}\n        }\n    }\n}\n`;
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-table-static-'));
  const file = path.join(dir, 't.cui');
  fs.writeFileSync(file, SRC ? fs.readFileSync(SRC, 'utf8') : generated());
  const out = path.join(dir, 'out');
  const r = spawnSync(CANCHO_UI, ['build', file, '--pwa', '-o', out], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const server = await serve(out);
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage({ viewport: { width: 280, height: 700 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root table');

  const box = '.cui-table-scroll';
  const rect = (sel) => page.evaluate((s) => { const b = document.querySelector(s).getBoundingClientRect(); return { top: b.top, left: b.left, right: b.right, bottom: b.bottom }; }, sel);
  const scrollBox = async (top, left) => page.evaluate(([t, l]) => { const e = document.querySelector('.cui-table-scroll'); e.scrollTop = t; e.scrollLeft = l; }, [top, left]);
  const geo = () => page.evaluate(() => { const e = document.querySelector('.cui-table-scroll'); return { ch: e.clientHeight, sh: e.scrollHeight, cw: e.clientWidth, sw: e.scrollWidth, font: parseFloat(getComputedStyle(document.documentElement).fontSize) }; });

  const g = await geo();
  check('the box is 10rem high and scrolls down', Math.abs(g.ch - 10 * g.font) <= 20 && g.sh > g.ch + 100, JSON.stringify(g));
  check('and scrolls sideways (the checks below are not vacuous)', g.sw > g.cw + 60, JSON.stringify(g));

  // 1. at rest, then scrolled: the header row stays at the top of the box and the first column at its left edge
  const headTop0 = (await rect(`${box} thead th, ${box} tr:first-child th`)).top;
  const boxRect = await rect(box);
  await scrollBox(200, 120);
  const moved = await page.evaluate(() => { const e = document.querySelector('.cui-table-scroll'); return { top: e.scrollTop, left: e.scrollLeft }; });
  check('the box did scroll, down and right', moved.top > 100 && moved.left > 50, JSON.stringify(moved));
  const th0 = await page.evaluate(() => { const e = document.querySelector('.cui-table-scroll'); const b = e.getBoundingClientRect(); const h = document.querySelector('.cui-table-scroll th:nth-child(3)').getBoundingClientRect(); return { dy: h.top - b.top - e.clientTop, bottom: h.bottom }; });
  check('scrolled down: the header is exactly at the top of the box', Math.abs(th0.dy) <= 0.5, JSON.stringify(th0));
  const col = await page.evaluate(() => { const e = document.querySelector('.cui-table-scroll'); const b = e.getBoundingClientRect(); const rows = document.querySelectorAll('.cui-table-scroll tbody tr'); const c = rows[20].querySelector('th, td').getBoundingClientRect(); return { dx: c.left - b.left - e.clientLeft, w: c.width }; });
  check('scrolled right: the first column is exactly at the left edge of the box', Math.abs(col.dx) <= 0.5, JSON.stringify(col));
  const corner = await page.evaluate(() => { const e = document.querySelector('.cui-table-scroll'); const b = e.getBoundingClientRect(); const c = document.querySelector('.cui-table-scroll th:first-child').getBoundingClientRect(); const top = document.elementFromPoint(c.left + 3, c.top + 3); return { dx: c.left - b.left - e.clientLeft, dy: c.top - b.top - e.clientTop, ontop: top && top.closest('th') === document.querySelector('.cui-table-scroll th:first-child') }; });
  check('the corner cell stays in both and is on top of the cells that scroll under it', Math.abs(corner.dx) <= 0.5 && Math.abs(corner.dy) <= 0.5 && corner.ontop, JSON.stringify(corner));
  const under = await page.evaluate(() => { const b = document.querySelector('.cui-table-scroll').getBoundingClientRect(); const first = document.querySelector('.cui-table-scroll th:first-child').getBoundingClientRect(); const ths = [...document.querySelectorAll('.cui-table-scroll th')].filter((h) => { const r = h.getBoundingClientRect(); return r.left > first.right + 2 && r.right < b.right - 2; }); if (ths.length === 0) { return null; } const h = ths[0]; const c = h.getBoundingClientRect(); const e = document.elementFromPoint(c.left + 3, c.top + 3); return !!e && e.closest('th') === h; });
  check('the sticky header cells hide the rows that scroll under them (the topmost element there is the cell)', under === true, String(under));
  const opaque = await page.evaluate(() => ['.cui-table-scroll th:nth-child(2)', '.cui-table-scroll th:first-child', '.cui-table-scroll tbody tr:nth-child(5) td:first-child, .cui-table-scroll tbody tr:nth-child(5) th:first-child'].map((s) => getComputedStyle(document.querySelector(s)).backgroundColor));
  check('the sticky cells have a solid background (rows scrolling under them do not show through)', opaque.every((c) => /^rgb\(/.test(c) || /, 1\)$/.test(c)), JSON.stringify(opaque));

  // 2. numeric cells: right-aligned and tabular; a plain cell is not
  const align = await page.evaluate(() => { const n = document.querySelector('.cui-table-scroll td.cui-num'); const p = document.querySelector('.cui-table-scroll tbody tr td:first-child, .cui-table-scroll tbody tr th:first-child'); const s = getComputedStyle(n); return { num: s.textAlign, tab: s.fontVariantNumeric, plain: getComputedStyle(p).textAlign }; });
  check('a numeric cell is aligned to the end with tabular digits', (align.num === 'end' || align.num === 'right') && align.tab.includes('tabular-nums'), JSON.stringify(align));
  check('a plain cell is not', align.plain !== 'end' && align.plain !== 'right', align.plain);
  const right = await page.evaluate(() => [...document.querySelectorAll('.cui-table-scroll tbody tr')].slice(0, 5).map((tr) => { const c = tr.children[1]; const r = document.createRange(); r.selectNodeContents(c); return Math.round(c.getBoundingClientRect().right - r.getBoundingClientRect().right); }));
  check('the numbers end at the same distance from the cell edge', right.every((x) => Math.abs(x - right[0]) <= 1), JSON.stringify(right));

  // 3. the accessibility tree is the table it was: roles, header cells, and no extra focus stop but the scroll box
  const tree = await page.evaluate(() => ({ tables: document.querySelectorAll('table').length, th: document.querySelectorAll('th').length, rows: document.querySelectorAll('tr').length, focus: document.querySelectorAll('.cui-table-scroll[tabindex="0"]').length, hidden: document.querySelectorAll('[aria-hidden]').length }));
  check('one table, 41 rows, 6 header cells, one focus stop, nothing hidden', tree.tables === 1 && tree.rows === 41 && tree.th >= 6 && tree.focus === 1 && tree.hidden === 0, JSON.stringify(tree));
  check('no script errors', errors.length === 0, errors.join(' | '));
  void headTop0; void boxRect;

  await browser.close();
  server.close();
  console.log(failures.length === 0 ? 'all table-static checks passed' : `${failures.length} failed`);
  process.exit(failures.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });
