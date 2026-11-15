#!/usr/bin/env node
// Browser-side checks of a sortable table header (docs/table-sort.md, section 3). Outside the gate: needs node, the `playwright` package with a Chromium, and a built
// `cancho-ui`. Chromium only.
//   node scripts/check-table-sort.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'release', 'cancho-ui');
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

// Hand-computed from rows [pump, valve, fan, compressor, pipe]: the lengths are 4, 5, 3, 10, 4, so pump and pipe tie, and the sort is stable.
const UNSORTED = ['pump', 'valve', 'fan', 'compressor', 'pipe'];
const NAME_ASC = ['compressor', 'fan', 'pipe', 'pump', 'valve'];
const NAME_DESC = ['valve', 'pump', 'pipe', 'fan', 'compressor'];
const LEN_ASC = ['fan', 'pump', 'pipe', 'valve', 'compressor'];
const LEN_DESC = ['compressor', 'valve', 'pipe', 'pump', 'fan'];

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-table-sort-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/table-sort/table-sort.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const server = await serve(dir);
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root table');

  const rows = () => page.evaluate(() => [...document.querySelectorAll('tbody tr')].slice(1).map((tr) => tr.children[0].textContent));
  const sortOf = () => page.evaluate(() => [...document.querySelectorAll('th')].map((th) => th.getAttribute('aria-sort')));
  const lens = () => page.evaluate(() => [...document.querySelectorAll('tbody tr')].slice(1).map((tr) => Number(tr.children[1].textContent)));

  check('at rest: the program\'s own order, and no header says it is sorted', JSON.stringify(await rows()) === JSON.stringify(UNSORTED) && (await sortOf()).every((s) => s === null), JSON.stringify([await rows(), await sortOf()]));

  // structure: each header is a th (scope col) holding exactly one button named like the column; aria-sort is on the th, never on the button
  const shape = await page.evaluate(() => [...document.querySelectorAll('th')].map((th) => ({ scope: th.getAttribute('scope'), kids: th.children.length, tag: th.children[0] && th.children[0].tagName, type: th.children[0] && th.children[0].getAttribute('type'), name: th.textContent, onButton: th.querySelector('[aria-sort]') !== null })));
  check('each header is a th scope=col with one button named like the column', shape.length === 2 && shape.every((s) => s.scope === 'col' && s.kids === 1 && s.tag === 'BUTTON' && s.type !== 'submit' && !s.onButton) && shape[0].name === 'name' && shape[1].name === 'length', JSON.stringify(shape));
  const snap = await page.locator('table').ariaSnapshot();
  check('the accessibility tree reads columnheader "name" holding button "name"', /columnheader "name"[\s\S]*button "name"/.test(snap) && /columnheader "length"[\s\S]*button "length"/.test(snap), snap);

  const tap = async (col) => { await page.click(`th:nth-child(${col}) button`); };
  // name: ascending, descending, unsorted, ascending
  await tap(1);
  const sortedSnap = await page.locator('table').ariaSnapshot();
  const arrow = await page.evaluate(() => getComputedStyle(document.querySelector('th button'), '::after').content);
  check('the arrow is drawn and is not in the name of the button', /button "name"/.test(sortedSnap) && !/[\u2191\u2193]/.test(sortedSnap) && /\u2191/.test(arrow), JSON.stringify([arrow, sortedSnap]));
  check('tap 1 on name: ascending', JSON.stringify(await rows()) === JSON.stringify(NAME_ASC) && JSON.stringify(await sortOf()) === JSON.stringify(['ascending', null]), JSON.stringify([await rows(), await sortOf()]));
  await tap(1);
  check('tap 2 on name: descending', JSON.stringify(await rows()) === JSON.stringify(NAME_DESC) && JSON.stringify(await sortOf()) === JSON.stringify(['descending', null]), JSON.stringify([await rows(), await sortOf()]));
  await tap(1);
  check('tap 3 on name: unsorted, and aria-sort is gone', JSON.stringify(await rows()) === JSON.stringify(UNSORTED) && (await sortOf()).every((s) => s === null), JSON.stringify([await rows(), await sortOf()]));
  await tap(1);
  // another column starts at ascending, and the first loses its aria-sort
  await tap(2);
  check('tap on length after name: length ascending, name no longer sorted; the tie keeps its order', JSON.stringify(await rows()) === JSON.stringify(LEN_ASC) && JSON.stringify(await sortOf()) === JSON.stringify([null, 'ascending']) && JSON.stringify(await lens()) === JSON.stringify([3, 4, 4, 5, 10]), JSON.stringify([await rows(), await sortOf()]));
  await tap(2);
  check('tap 2 on length: descending', JSON.stringify(await rows()) === JSON.stringify(LEN_DESC) && JSON.stringify(await sortOf()) === JSON.stringify([null, 'descending']), JSON.stringify([await rows(), await sortOf()]));

  // keyboard: Tab reaches each header button in document order, Enter and Space both sort
  await page.reload();
  await page.waitForSelector('#root table');
  await page.keyboard.press('Tab');
  let focus = await page.evaluate(() => document.activeElement.tagName + ':' + document.activeElement.textContent);
  if (focus.startsWith('DIV')) { await page.keyboard.press('Tab'); focus = await page.evaluate(() => document.activeElement.tagName + ':' + document.activeElement.textContent); }
  check('Tab reaches the first header button', focus === 'BUTTON:name', focus);
  await page.keyboard.press('Enter');
  const afterEnter = JSON.stringify(await rows()) === JSON.stringify(NAME_ASC);
  check('Enter sorts', afterEnter, JSON.stringify(await rows()));
  const stillFocused = await page.evaluate(() => document.activeElement.tagName + ':' + document.activeElement.textContent);
  check('the focus is still on a header button after the table is drawn again', stillFocused === 'BUTTON:name', stillFocused);
  await page.keyboard.press('Space');
  check('Space sorts', JSON.stringify(await rows()) === JSON.stringify(NAME_DESC), JSON.stringify(await rows()));
  await page.keyboard.press('Tab');
  const second = await page.evaluate(() => document.activeElement.tagName + ':' + document.activeElement.textContent);
  check('Tab reaches the second header button', second === 'BUTTON:length', second);

  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length === 0 ? 'all table-sort checks passed' : `${failures.length} failed`);
  process.exit(failures.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });
