#!/usr/bin/env node
// Browser-side checks of the Storage adapter (docs/adapters.md, section 6). Outside the gate: needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-storage.js [path/to/cancho-ui]
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

async function shown(page) {
  return page.evaluate(() => Array.from(document.querySelectorAll('#root *')).map((e) => e.textContent).filter((t, i, a) => a.indexOf(t) === i).join('|'));
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-storage-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/notes/notes.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const server = await serve(dir);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // 1. a normal browser: a note survives a reload, under the prefix, and nothing else is written.
  {
    const context = await browser.newContext();
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('#root *');
    check('before any save the default is shown', (await shown(page)).includes('nothing yet'), await shown(page));
    await page.fill('input', 'buy milk');
    await page.click('button');
    check('after Save the note is shown at once', (await shown(page)).includes('buy milk'), await shown(page));
    const keys = await page.evaluate(() => Object.keys(window.localStorage));
    check('exactly one key is written, under the prefix cui:', keys.length === 1 && keys[0] === 'cui:note', JSON.stringify(keys));
    await page.reload();
    await page.waitForSelector('#root *');
    check('after a reload the note is still shown', (await shown(page)).includes('buy milk'), await shown(page));
    check('no script errors', errors.length === 0, errors.join(' | '));
    await context.close();
  }

  // 1b. a page's own key `note`, without the prefix, is not the program's `note`.
  {
    const context = await browser.newContext();
    await context.addInitScript(() => { window.localStorage.setItem('note', 'not mine'); });
    const page = await context.newPage();
    await page.goto(url);
    await page.waitForSelector('#root *');
    check('a key `note` without the prefix is not read as the program\'s `note`', !(await shown(page)).includes('not mine') && (await shown(page)).includes('nothing yet'), await shown(page));
    await context.close();
  }

  // 2. localStorage that throws (a private window, blocked site data, a full quota): the page still works for its lifetime.
  {
    const context = await browser.newContext();
    await context.addInitScript(() => {
      const boom = () => { throw new DOMException('blocked', 'SecurityError'); };
      Object.defineProperty(window, 'localStorage', { get: () => ({ getItem: boom, setItem: boom, removeItem: boom }) });
    });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('#root *');
    await page.fill('input', 'only this page');
    await page.click('button');
    check('with storage blocked, Save still shows the note for the life of the page', (await shown(page)).includes('only this page'), await shown(page));
    await page.reload();
    await page.waitForSelector('#root *');
    check('with storage blocked, a reload forgets it, as documented', (await shown(page)).includes('nothing yet'), await shown(page));
    check('with storage blocked, no script errors', errors.length === 0, errors.join(' | '));
    await context.close();
  }

  // 3. `requires Storage("notes")`: keys live under `cui:notes:`, and a key of the bare program is not visible.
  {
    const nsDir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-storage-ns-'));
    const src = fs.readFileSync(path.join(ROOT, 'examples/notes/notes.cui'), 'utf8').replace('requires Storage;', 'requires Storage("notes");');
    const nsFile = path.join(nsDir, 'notes.cui');
    fs.writeFileSync(nsFile, src);
    const b = spawnSync(CANCHO_UI, ['build', nsFile, '--pwa', '-o', path.join(nsDir, 'out')], { encoding: 'utf8' });
    if (b.status !== 0) { throw new Error(b.stderr); }
    const nsServer = await serve(path.join(nsDir, 'out'));
    const nsUrl = `http://localhost:${nsServer.address().port}/`;
    const context = await browser.newContext();
    await context.addInitScript(() => { window.localStorage.setItem('cui:note', 'the bare program\'s note'); });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(nsUrl);
    await page.waitForSelector('#root *');
    check('a namespaced program does not see the bare program\'s key cui:note', (await shown(page)).includes('nothing yet') && !(await shown(page)).includes('bare program'), await shown(page));
    await page.fill('input', 'in a namespace');
    await page.click('button');
    const keys = (await page.evaluate(() => Object.keys(window.localStorage))).sort();
    check('Save writes cui:notes:note and leaves cui:note alone', JSON.stringify(keys) === JSON.stringify(['cui:note', 'cui:notes:note']), JSON.stringify(keys));
    await page.reload();
    await page.waitForSelector('#root *');
    check('after a reload the namespaced note is shown', (await shown(page)).includes('in a namespace'), await shown(page));
    check('namespaced: no script errors', errors.length === 0, errors.join(' | '));
    await context.close();
    nsServer.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
