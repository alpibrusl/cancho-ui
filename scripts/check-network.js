#!/usr/bin/env node
// Browser-side checks of `net.get` (docs/adapters.md, section 9). Outside the gate: needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-network.js [path/to/cancho-ui]
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

// The app's own origin: the built page, and the routes the checks fetch.
const slow = { opened: 0, closed: 0 };
function mainServer(dir) {
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url === '/data.txt') { res.writeHead(200, { 'content-type': 'text/plain' }); res.end('hello from the server'); return; }
    if (url === '/redirect.txt') { res.writeHead(302, { location: '/data.txt' }); res.end(); return; }
    if (url === '/big.txt') { res.writeHead(200, { 'content-type': 'text/plain' }); res.end('x'.repeat(2 * 1024 * 1024)); return; }
    if (url === '/slow.txt') { slow.opened += 1; req.on('close', () => { slow.closed += 1; }); return; } // never answered
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end('no'); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

// Another origin that counts every request it receives; the program must never cause one.
function otherServer() {
  const hits = { n: 0 };
  const server = http.createServer((req, res) => {
    hits.n += 1;
    res.writeHead(200, { 'access-control-allow-origin': '*' });
    res.end('secret');
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ server, hits })));
}

async function status(page) {
  return page.evaluate(() => {
    const texts = Array.from(document.querySelectorAll('#root *')).filter((e) => e.children.length === 0).map((e) => e.textContent);
    return texts;
  });
}

async function load(page, url, waitMs) {
  await page.fill('input', url);
  await page.click('button');
  await page.waitForFunction((n) => !document.body.textContent.includes('status: loading'), null, { timeout: waitMs || 5000 });
  return (await status(page)).join(' | ');
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-network-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/fetch/fetch.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const main = await mainServer(dir);
  const other = await otherServer();
  const base = `http://localhost:${main.address().port}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`${base}/`);
  await page.waitForSelector('#root *');

  let shown = await load(page, 'data.txt');
  check('a same-origin file is fetched and shown', shown.includes('status: done') && shown.includes('hello from the server'), shown);
  shown = await load(page, 'missing.txt');
  check('a 404 is an Err("HTTP 404"), and the old body is gone', shown.includes('status: error: HTTP 404') && !shown.includes('hello'), shown);
  shown = await load(page, `http://127.0.0.1:${other.server.address().port}/secret.txt`);
  check('another origin is blocked', shown.includes('is neither this site nor an allowed host'), shown);
  check('and no request reached that origin', other.hits.n === 0, `hits: ${other.hits.n}`);
  shown = await load(page, 'http://');
  check('a string that is not a URL is blocked', shown.includes('error: blocked: not a URL'), shown);
  shown = await load(page, 'redirect.txt');
  check('a redirect is refused', shown.includes('error: blocked: redirect'), shown);
  shown = await load(page, 'big.txt');
  check('a body over 1 MiB is "too large"', shown.includes('error: too large'), shown);
  // a newer request supersedes the pending one, and the browser is told to stop the old one (docs/adapters.md 9.5)
  {
    const opened = slow.opened;
    const closed = slow.closed;
    await page.fill('input', 'slow.txt');
    await page.click('button');
    for (let i = 0; i < 50 && slow.opened === opened; i++) { await page.waitForTimeout(20); }
    check('a slow request reaches the server and waits', slow.opened === opened + 1 && slow.closed === closed, `${slow.opened - opened} opened, ${slow.closed - closed} closed`);
    shown = await load(page, 'data.txt');
    for (let i = 0; i < 50 && slow.closed === closed; i++) { await page.waitForTimeout(20); }
    check('the newer request is shown', shown.includes('status: done') && shown.includes('hello from the server'), shown);
    check('and the superseded one was aborted: its connection closed long before the 10 s timeout', slow.closed === closed + 1, `${slow.closed - closed} closed`);
  }
  const t0 = Date.now();
  shown = await load(page, 'slow.txt', 15000);
  const took = Date.now() - t0;
  check('a reply that never comes is a timeout after about 10 s', shown.includes('error: timeout') && took > 9000 && took < 13000, `${took} ms, ${shown}`);

  // Really offline: stop the server (the service worker's own fetch then fails too).
  main.closeAllConnections();
  await new Promise((done) => main.close(done));
  shown = await load(page, 'data.txt');
  check('with the server gone the answer is "network error"', shown.includes('error: network error'), shown);
  check('no script errors', errors.length === 0, errors.join(' | '));

  // `requires Network("host")` (docs/adapters.md section 10): the same program with the second server's host allowed.
  {
    const allowed = await otherServer();
    const third = await otherServer();
    const host = `127.0.0.1:${allowed.server.address().port}`;
    const source = fs.readFileSync(path.join(ROOT, 'examples/fetch/fetch.cui'), 'utf8').replace('\n    requires Network;\n', `\n    requires Network("${host}");\n`);
    if (!source.includes(host)) { throw new Error('the bound was not applied'); }
    const sourceFile = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'check-network-src-')), 'fetch.cui');
    fs.writeFileSync(sourceFile, source);
    const dir2 = fs.mkdtempSync(path.join(os.tmpdir(), 'check-network-b-'));
    const r2 = spawnSync(CANCHO_UI, ['build', sourceFile, '--pwa', '-o', dir2], { encoding: 'utf8' });
    if (r2.status !== 0) { throw new Error(r2.stderr); }
    const main2 = await mainServer(dir2);
    const page2 = await (await browser.newContext()).newPage();
    await page2.goto(`http://localhost:${main2.address().port}/`);
    await page2.waitForSelector('#root *');
    let t = await load(page2, `http://${host}/secret.txt`);
    check('a host named in requires Network(..) is fetched', t.includes('status: done') && t.includes('secret'), t);
    check('and that server saw exactly that one request', allowed.hits.n === 1, `hits: ${allowed.hits.n}`);
    t = await load(page2, `http://127.0.0.1:${third.server.address().port}/secret.txt`);
    check('another host is still blocked', t.includes('is neither this site nor an allowed host'), t);
    check('and no request reached it', third.hits.n === 0, `hits: ${third.hits.n}`);
    t = await load(page2, `HTTP://127.0.0.1:${allowed.server.address().port}/Secret.txt`);
    check('the scheme and host are compared case-insensitively', t.includes('status: done'), t);
    t = await load(page2, 'data.txt');
    check('this site is still reachable', t.includes('status: done') && t.includes('hello from the server'), t);
    allowed.server.close();
    third.server.close();
    main2.closeAllConnections();
    await new Promise((done) => main2.close(done));
  }

  await browser.close();
  other.server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
