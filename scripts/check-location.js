#!/usr/bin/env node
// Browser-side checks of `location.current` (docs/adapters.md, section 11). Outside the gate: needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`. Chromium only; the position is emulated.
//   node scripts/check-location.js [path/to/cancho-ui]
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

function build(source) {
  const srcDir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-location-src-'));
  const file = path.join(srcDir, 'where.cui');
  fs.writeFileSync(file, source);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-location-'));
  const r = spawnSync(CANCHO_UI, ['build', file, '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  return dir;
}

async function texts(page) {
  return page.evaluate(() => Array.from(document.querySelectorAll('#root *')).filter((e) => e.children.length === 0).map((e) => e.textContent).join(' | '));
}

async function press(page) {
  await page.click('button');
  await page.waitForFunction(() => !document.body.textContent.includes('status: locating'), null, { timeout: 15000 });
  return texts(page);
}

// A fresh browser context per case: permission and the emulated position are the context's.
async function run(browser, dir, options, init) {
  const server = await serve(dir);
  const context = await browser.newContext(options);
  if (init) { await context.addInitScript(init); }
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  const shown = await press(page);
  await context.close();
  server.close();
  return { shown, errors };
}

(async () => {
  const bare = fs.readFileSync(path.join(ROOT, 'examples/where/where.cui'), 'utf8');
  const fine = bare.replace('\n    requires Location;\n', '\n    requires Location("fine");\n');
  if (fine === bare) { throw new Error('the bound was not applied'); }
  const coarseDir = build(bare);
  const fineDir = build(fine);
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const here = { geolocation: { latitude: 40.416775, longitude: -3.703790 }, permissions: ['geolocation'] };

  let r = await run(browser, coarseDir, here);
  check('a bare `requires Location` gives 2 decimals', r.shown.includes('status: done') && r.shown.includes('40.42,-3.70') && !r.shown.includes('40.4167'), r.shown);
  check('no script errors', r.errors.length === 0, r.errors.join(' | '));
  r = await run(browser, fineDir, here);
  check('`Location("fine")` gives 5 decimals', r.shown.includes('40.41678,-3.70379'), r.shown);
  r = await run(browser, coarseDir, { geolocation: { latitude: -33.8688, longitude: 151.2093 }, permissions: ['geolocation'] });
  check('negative and positive longitudes keep their signs', r.shown.includes('-33.87,151.21'), r.shown);
  r = await run(browser, coarseDir, { geolocation: { latitude: 1, longitude: 2 } }); // permission not granted
  check('without the user\'s permission the answer is Err("permission denied")', r.shown.includes('status: error: permission denied') && !r.shown.includes('1.00'), r.shown);
  r = await run(browser, coarseDir, here, () => { Object.defineProperty(navigator, 'geolocation', { value: undefined, configurable: true }); });
  check('with no Geolocation API the answer is Err("unavailable")', r.shown.includes('status: error: unavailable'), r.shown);
  r = await run(browser, coarseDir, here, () => {
    Object.defineProperty(navigator, 'geolocation', { configurable: true, value: { getCurrentPosition: (ok, bad) => bad({ code: 2 }) } });
  });
  check('a position that cannot be determined is Err("position unavailable")', r.shown.includes('status: error: position unavailable'), r.shown);
  r = await run(browser, coarseDir, here, () => {
    Object.defineProperty(navigator, 'geolocation', { configurable: true, value: { getCurrentPosition: (ok, bad) => bad({ code: 3 }) } });
  });
  check('a timeout is Err("timeout")', r.shown.includes('status: error: timeout'), r.shown);
  r = await run(browser, coarseDir, here, () => {
    Object.defineProperty(navigator, 'geolocation', { configurable: true, value: { getCurrentPosition: () => { throw new Error('boom'); } } });
  });
  check('an adapter-side throw is Err("unavailable"), not a script error', r.shown.includes('status: error: unavailable') && r.errors.length === 0, `${r.shown} ${r.errors.join('|')}`);

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
