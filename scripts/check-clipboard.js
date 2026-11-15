#!/usr/bin/env node
// Browser-side checks of `Button { copy: field }` (docs/clipboard.md, section 5). Outside the gate: needs node, the `playwright` package with a
// Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-clipboard.js [path/to/cancho-ui]
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

// The page's own use of the API is counted; the test reads back through the reference saved before the page runs.
const COUNT = () => {
  window.__reads = 0;
  window.__writes = [];
  if (navigator.clipboard) {
    window.__readback = navigator.clipboard.readText.bind(navigator.clipboard);
    const write = navigator.clipboard.writeText.bind(navigator.clipboard);
    navigator.clipboard.readText = () => { window.__reads += 1; return window.__readback(); };
    navigator.clipboard.writeText = (t) => { window.__writes.push(t.length); return write(t); };
  }
};

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-clipboard-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/clipboard/clipboard.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const server = await serve(dir);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const label = (page) => page.evaluate(() => document.querySelector('button').textContent);

  // 1. a normal browser, permission granted: nothing is written at load or at a render; a tap writes the field's text, and only then.
  {
    const context = await browser.newContext();
    await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: url.replace(/\/$/, '') });
    await context.addInitScript(COUNT);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('#root button');
    await page.evaluate(() => navigator.clipboard.writeText('sentinel'));
    await page.evaluate(() => { window.__writes = []; });
    await page.fill('input', 'hello');
    check('typing (a render) writes nothing', (await page.evaluate(() => window.__writes.length)) === 0 && (await page.evaluate(() => window.__readback())) === 'sentinel');
    check('the button is named', (await label(page)) === 'Copy', await label(page));
    await page.click('button');
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Copied');
    check('a tap puts the field\'s text on the system clipboard', (await page.evaluate(() => window.__readback())) === 'hello');
    check('the label says Copied, then goes back', true);
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Copy', null, { timeout: 4000 });
    await page.fill('input', 'tab\there');
    await page.click('button');
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Copied');
    check('after the field changes, a tap writes the new text', (await page.evaluate(() => window.__readback())) === 'tab\there');
    check('exactly two writes, one for each tap', (await page.evaluate(() => window.__writes.length)) === 2, JSON.stringify(await page.evaluate(() => window.__writes)));
    check('the page never read the clipboard', (await page.evaluate(() => window.__reads)) === 0);
    check('no script errors', errors.length === 0, errors.join(' | '));
    await context.close();
  }

  // 2. a text over the limit is refused, not cut: nothing is written and the label says so.
  {
    const context = await browser.newContext();
    await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: url.replace(/\/$/, '') });
    await context.addInitScript(COUNT);
    const page = await context.newPage();
    await page.goto(url);
    await page.waitForSelector('#root button');
    await page.evaluate(() => navigator.clipboard.writeText('sentinel'));
    await page.evaluate(() => { window.__writes = []; });
    await page.fill('input', 'x'.repeat(1000001));
    await page.click('button');
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Too long to copy');
    check('1,000,001 units: nothing written', (await page.evaluate(() => window.__writes.length)) === 0 && (await page.evaluate(() => window.__readback())) === 'sentinel');
    await page.fill('input', 'y'.repeat(1000000));
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Copy', null, { timeout: 4000 });
    await page.click('button');
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Copied');
    check('1,000,000 units: written whole', (await page.evaluate(() => window.__readback())).length === 1000000);
    await context.close();
  }

  // 3. a browser that rejects the write, and one with no clipboard API: the label says Not copied and nothing throws.
  for (const [name, script] of [
    ['a rejected write', () => { navigator.clipboard.writeText = () => Promise.reject(new Error('refused')); }],
    ['no clipboard API', () => { Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: true }); }],
  ]) {
    const context = await browser.newContext();
    await context.addInitScript(script);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('#root button');
    await page.fill('input', 'hello');
    await page.click('button');
    await page.waitForFunction(() => document.querySelector('button').textContent === 'Not copied');
    check(`${name}: the label says Not copied`, true);
    check(`${name}: no script errors`, errors.length === 0, errors.join(' | '));
    await context.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length === 0 ? 'all clipboard checks passed' : `${failures.length} failed`);
  process.exit(failures.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });
