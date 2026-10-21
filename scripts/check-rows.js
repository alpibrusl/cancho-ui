#!/usr/bin/env node
// Row actions in Chromium (docs/row-actions.md, issue #41): a Button inside a For whose label is an expression and whose
// event carries the element it was drawn for. Builds examples/rows/rows.cui with 100 and with 1000 rows, clicks, and
// measures what a click that re-renders every row costs. Outside the gate: needs node, the `playwright` package with a
// Chromium, and a built `cancho-ui`.
//   node scripts/check-rows.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

// `rows` is the number of rows; `buttons` false draws the same rows as Text, the baseline for the cost.
function program(rows, buttons) {
  const names = ['<img src=x onerror=alert(1)>', 'a"b'];
  for (let i = 2; i < rows; i++) { names.push(`R${i}`); }
  const list = names.map((n) => JSON.stringify(n)).join(', ');
  const row = buttons
    ? 'Button { label: "Select " + n on: Pick(n, len(names) - 1) }'
    : 'Text { "Select " + n }';
  return `
component Rows {
    state names: List[String] = [${list}];
    state picked: String = "nothing";
    state clicks: Int = 0;
    on Pick(id, position) { picked = id + " at " + position; clicks = clicks + 1; }
    on Reverse { names = reverse(names); }
    on Bump { clicks = clicks + 1; }
    view {
        Column {
            Text { role: status "picked: {picked}" }
            Button { label: "Reverse" on: Reverse }
            Button { label: "Bump" on: Bump }
            For { over: names as: n ${row} }
            Text { "clicks: {clicks}" }
        }
    }
}
`;
}

async function open(browser, dir, name, src) {
  const d = path.join(dir, name);
  fs.mkdirSync(d, { recursive: true });
  fs.writeFileSync(path.join(d, 'p.cui'), src);
  const r = spawnSync(CANCHO_UI, ['build', path.join(d, 'p.cui'), '--pwa', '-o', path.join(d, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(d, 'out', path.normalize(rel));
    if (!file.startsWith(path.join(d, 'out')) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('dialog', (dlg) => { errors.push(`dialog: ${dlg.message()}`); dlg.dismiss(); });
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  return { page, server, errors };
}

// The median, in milliseconds, of `n` clicks on `selector` (each one re-renders every row), measured in the page.
async function clickCost(page, selector, n) {
  return page.evaluate(({ selector, n }) => {
    const el = () => document.querySelector(selector);
    const clicks = () => Number(/clicks: (\d+)/.exec(document.getElementById('root').innerText)[1]);
    const times = [];
    for (let i = 0; i < n; i++) {
      const before = clicks();
      const t0 = performance.now();
      el().click();
      times.push(performance.now() - t0);
      // The render is synchronous with the click, so the time above includes it: the text has already changed.
      if (clicks() !== before + 1) { throw new Error('the page had not re-rendered when the click returned'); }
    }
    times.sort((a, b) => a - b);
    return times[Math.floor(n / 2)];
  }, { selector, n });
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-rows-'));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // ---- 100 rows: what the page does
  const a = await open(browser, dir, 'rows100', program(100, true));
  const { page } = a;
  const text = async () => page.locator('#root').innerText();
  const selects = page.getByRole('button', { name: /^Select / });
  check('one button per row (100)', (await selects.count()) === 100, String(await selects.count()));
  check('a row is a button named by its element', (await page.getByRole('button', { name: 'Select R57', exact: true }).count()) === 1);
  check('the visible text is the accessible name', await page.evaluate(() => [...document.querySelectorAll('button')].filter((b) => b.textContent.startsWith('Select ')).every((b) => b.getAttribute('aria-label') === b.textContent)));
  await page.getByRole('button', { name: 'Select R57', exact: true }).click();
  check('clicking a row gives its event that row (element and position)', (await text()).includes('picked: R57 at 99'), (await text()).split('\n')[0]);
  check('and the handler ran once', (await text()).includes('clicks: 1'));
  await page.getByRole('button', { name: 'Select R3', exact: true }).click();
  check('a click after a re-render still names its own row', (await text()).includes('picked: R3 at 99') && (await text()).includes('clicks: 2'));
  await page.getByRole('button', { name: 'Reverse' }).click();
  await page.locator('button', { hasText: /^Select / }).first().click();
  check('after the list is reversed the first row is the former last (not the former first)', (await text()).includes('picked: R99 at 99'), (await text()).split('\n')[0]);
  await page.getByRole('button', { name: 'Select a"b', exact: true }).click();
  check('a quote in the element is neither lost nor an injection', (await text()).includes('picked: a"b at 99'));
  const hostile = '<img src=x onerror=alert(1)>';
  check('markup in the element is a button name and not markup', (await page.getByRole('button', { name: `Select ${hostile}`, exact: true }).count()) === 1 && (await page.locator('#root img').count()) === 0);
  await page.getByRole('button', { name: `Select ${hostile}`, exact: true }).click();
  check('and its click carries the text whole', (await text()).includes(`picked: ${hostile} at 99`));
  check('no script errors and no dialog (100 rows)', a.errors.length === 0, a.errors.join(' | '));
  const cost100 = await clickCost(page, 'button[aria-label="Bump"]', 30);
  a.server.close();

  // ---- the cost, with buttons and with the same rows as text
  const rows = {};
  for (const n of [100, 1000]) {
    for (const buttons of [true, false]) {
      const c = await open(browser, dir, `cost${n}${buttons ? 'b' : 't'}`, program(n, buttons));
      rows[`${n} ${buttons ? 'buttons' : 'text'}`] = await clickCost(c.page, 'button[aria-label="Bump"]', 30);
      check(`no script errors (${n} ${buttons ? 'buttons' : 'text'})`, c.errors.length === 0, c.errors.join(' | '));
      c.server.close();
    }
  }
  const report = Object.entries(rows).map(([k, v]) => `${k}: ${v.toFixed(2)} ms`).join(', ');
  console.log(`median of 30 clicks that re-render every row: ${report}`);
  check('1000 rows of buttons re-render in well under a frame budget of a slow page (< 250 ms)', rows['1000 buttons'] < 250, `${rows['1000 buttons'].toFixed(1)} ms`);
  void cost100;
  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
