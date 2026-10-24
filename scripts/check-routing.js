#!/usr/bin/env node
// Routing (docs/routing.md) in Chromium, in the mobile profile (Pixel 7: touch, 412 px, Android UA), on a stand-in for a static host that
// answers an unknown path with 404 and serves the page under a prefix, as GitHub Pages does:
//  A. tests/routing_corpus.json, the file `cargo test` replays (crates/cancho-ui/tests/routing.rs): the route event hears the same paths, the
//     program ends in the same state, and the page's own URL agrees (the fragment, and how many entries each step added);
//  B. the page as a visitor meets it (examples/routes built with --page): a deep link mounts the detail, a reload stays on it, Back from a detail
//     returns to the list, a link can be followed with the keyboard, the link is an anchor of this document;
//  C. the same page built with --pwa and its service worker: a deep link and a reload work with the server stopped, and the path form of the same
//     URL (/app/items/valve) is a 404 online (why the route is a fragment, docs/routing.md section 2).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-routing.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const PREFIX = '/app/';
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

function build(src, dir, flag) {
  const r = spawnSync(CANCHO_UI, ['build', src, flag, '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${src}: ${r.stdout}${r.stderr}`); }
}

// A static host: files under PREFIX, 404 (an HTML body) for anything else.
function host(dir) {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
  const seen = [];
  const server = http.createServer((req, res) => {
    const u = req.url.split('?')[0];
    seen.push(u);
    const rel = u.startsWith(PREFIX) ? decodeURIComponent(u.slice(PREFIX.length)) : null;
    const f = rel === null ? null : path.join(dir, rel === '' ? 'index.html' : path.normalize(rel));
    if (f && f.startsWith(dir) && fs.existsSync(f) && fs.statSync(f).isFile()) {
      res.writeHead(200, { 'content-type': types[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    } else {
      res.writeHead(404, { 'content-type': 'text/html' });
      res.end('<h1>404 File not found</h1>');
    }
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ server, seen, port: server.address().port })));
}

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-routing-'));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // ---- A. the corpus ----
  {
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'routing_corpus.json'), 'utf8'));
    const dir = path.join(tmp, 'corpus');
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), corpus.program.join('\n') + '\n');
    build(path.join(dir, 'p.cui'), path.join(dir, 'out'), '--page');
    const h = await host(path.join(dir, 'out'));
    const base = `http://localhost:${h.port}${PREFIX}`;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(`pageerror ${e}`));
    page.on('console', (m) => { if (m.type() === 'error') { problems.push(`console ${m.text()}`); } });
    page.on('response', (r) => { if (r.status() >= 400) { problems.push(`${r.status()} ${r.url()}`); } });
    await page.goto(`${base}#${corpus.load}`);
    await page.waitForSelector('#root button');
    const read = async () => {
      const t = await page.evaluate(() => document.body.innerText);
      const m = t.match(/path=\[(.*)\] log=(\S*) ok=(\w+)/);
      return m ? { path: m[1], log: m[2], ok: m[3] === 'true', hash: await page.evaluate(() => location.hash), len: await page.evaluate(() => history.length) } : null;
    };
    let len = (await read()).len;
    let i = 0;
    for (const s of corpus.steps) {
      const label = `corpus ${i++} (${s.do}): ${s.why}`;
      const before = problems.length;
      if (s.do === 'click') {
        await page.getByRole('link', { name: s.arg, exact: true }).tap();
      } else if (s.do === 'go' || s.do === 'replace') {
        await page.getByRole('textbox', { name: 'Target' }).fill(s.arg);
        await page.getByRole('button', { name: s.do === 'go' ? 'Go' : 'Replace', exact: true }).tap();
      } else if (s.do === 'two') {
        await page.getByRole('button', { name: 'Two', exact: true }).tap();
      } else if (s.do === 'back') {
        await page.goBack();
      } else if (s.do === 'forward') {
        await page.goForward();
      } else if (s.do === 'hash') {
        await page.goto(`${base}${s.arg}`);
      } else if (s.do === 'fragment') {
        await page.evaluate((f) => { location.hash = f; }, s.arg);
      }
      await page.waitForTimeout(60);
      const d = await read();
      const want = { log: s.log, path: s.path, hash: s.hash };
      if (s.ok !== undefined) { want.ok = s.ok; }
      const got = { log: d.log, path: d.path, hash: d.hash };
      if (s.ok !== undefined) { got.ok = d.ok; }
      const same = JSON.stringify(got) === JSON.stringify(want);
      const grew = d.len - len;
      len = d.len;
      const grewOk = s.grew === undefined || grew === s.grew;
      check(label, same && grewOk && problems.length === before,
        same && grewOk ? '' : `${JSON.stringify(got).slice(0, 300)} vs ${JSON.stringify(want).slice(0, 300)}${grewOk ? '' : ` history grew by ${grew}, wanted ${s.grew}`} ${problems.slice(before).join('; ')}`);
    }
    check('corpus: no 4xx, failed request, console or page error', problems.length === 0, problems.join('; '));
    await ctx.close();
    h.server.close();
  }

  // ---- B. the page as a visitor meets it ----
  {
    const dir = path.join(tmp, 'routes-page');
    build(path.join(ROOT, 'examples/routes/routes.cui'), dir, '--page');
    const h = await host(dir);
    const base = `http://localhost:${h.port}${PREFIX}`;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(`pageerror ${e}`));
    page.on('console', (m) => { if (m.type() === 'error') { problems.push(`console ${m.text()}`); } });
    const heading = async () => page.evaluate(() => (document.querySelector('[role=heading]') || {}).textContent);
    await page.goto(`${base}#/items/valve`);
    await page.waitForSelector('#root [role=heading]');
    check('B: a deep link mounts the detail', (await heading()) === 'Machine valve', await heading());
    await page.reload();
    await page.waitForSelector('#root [role=heading]');
    check('B: a reload stays on the detail', (await heading()) === 'Machine valve' && page.url().endsWith('#/items/valve'), `${await heading()} ${page.url()}`);
    await page.getByRole('link', { name: 'All machines' }).tap();
    await page.waitForTimeout(60);
    check('B: the link leads to the list', (await heading()) === 'Machines', await heading());
    await page.goBack();
    await page.waitForTimeout(60);
    check('B: Back returns to the detail', (await heading()) === 'Machine valve', await heading());
    await page.goForward();
    await page.waitForTimeout(60);
    check('B: Forward returns to the list', (await heading()) === 'Machines', await heading());
    // the link is an anchor of this document, with a name, followed with the keyboard
    const href = await page.getByRole('link', { name: 'pump' }).getAttribute('href');
    check('B: a NavLink is an <a> whose href is the fragment', href === '#/items/pump', href);
    await page.getByRole('link', { name: 'pump' }).focus();
    await page.keyboard.press('Enter');
    await page.waitForTimeout(60);
    check('B: Enter on a focused link follows it', (await heading()) === 'Machine pump', await heading());
    await page.goto(`${base}#/items/lamp`);
    await page.waitForTimeout(60);
    check('B: a path with no page is the program\'s to draw', (await heading()) === 'Not found', await heading());
    check('B: no console or page error', problems.length === 0, problems.join('; '));
    await ctx.close();
    h.server.close();
  }

  // ---- C. offline, with the service worker, on a host that 404s unknown paths ----
  {
    const dir = path.join(tmp, 'routes-pwa');
    build(path.join(ROOT, 'examples/routes/routes.cui'), dir, '--pwa');
    const h = await host(dir);
    const base = `http://localhost:${h.port}${PREFIX}`;
    const ctx = await browser.newContext({ ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    await page.goto(`${base}#/items/gauge`);
    await page.waitForSelector('#root [role=heading]');
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    check('C: the service worker controls the page', await page.evaluate(() => !!navigator.serviceWorker.controller));
    const online = await page.goto(`${base}items/gauge`);
    check('C: the path form of the URL is a 404 online (the host knows no such file)', online.status() === 404, String(online.status()));
    await page.goto(`${base}#/items/gauge`);
    await page.waitForSelector('#root [role=heading]');
    h.server.closeAllConnections(); // before waiting for `close`, which calls back only when no request is in flight
    await new Promise((ok) => h.server.close(ok));
    await page.goto(`${base}#/items/pump`);
    await page.waitForSelector('#root [role=heading]', { timeout: 5000 });
    check('C: a deep link mounts with the server stopped', (await page.evaluate(() => document.querySelector('[role=heading]').textContent)) === 'Machine pump');
    await page.reload();
    await page.waitForSelector('#root [role=heading]', { timeout: 5000 });
    check('C: a reload on the detail works with the server stopped', page.url().endsWith('#/items/pump') && (await page.evaluate(() => document.querySelector('[role=heading]').textContent)) === 'Machine pump');
    await page.getByRole('link', { name: 'All machines' }).tap();
    await page.waitForTimeout(60);
    check('C: a link works offline', (await page.evaluate(() => document.querySelector('[role=heading]').textContent)) === 'Machines');
    await ctx.close();
  }

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
