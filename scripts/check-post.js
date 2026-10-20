#!/usr/bin/env node
// Browser-side checks of `net.post` (docs/adapters.md section 17), in Chromium against real HTTP servers: a POST of this
// site carries the body, the JSON content type and this origin's cookie; any status is an answer (201, 409, 503); another
// origin is blocked and that server sees 0 requests; a redirect is refused; a reply over 1 MiB is `too large`; a write
// superseded by a newer one is NOT aborted (the server sees both complete); a stopped server is `network error`.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-post.js [path/to/cancho-ui]
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

const SRC = (other) => `
component Writer {
    requires NetworkWrite;

    state note: String = "[1,2]";
    state target: String = "/api/ok";
    state elsewhere: String = "${other}";
    state r: Result[String, String] = Ok("");
    state shown: String = "idle";

    on Save {
        r = await(net.post(target, note));
        shown = match r { Ok(reply) => { json.get(reply, "status", "?") + " " + json.get(reply, "body", "") } Err(e) => { "err " + e } Cancelled => { "cancelled" } };
    }
    on SaveElsewhere {
        r = await(net.post(elsewhere, note));
        shown = match r { Ok(reply) => { json.get(reply, "status", "?") } Err(e) => { "err " + e } Cancelled => { "cancelled" } };
    }
    on Ok { target = "/api/ok"; }
    on Conflict { target = "/api/conflict"; }
    on Down { target = "/api/down"; }
    on Redirect { target = "/api/redirect"; }
    on Big { target = "/api/big"; }
    on Slow { target = "/api/slow"; }

    view {
        Column {
            Text { "{shown}" }
            Button { label: "Save" on: Save }
            Button { label: "Elsewhere" on: SaveElsewhere }
            Button { label: "to ok" on: Ok }
            Button { label: "to conflict" on: Conflict }
            Button { label: "to down" on: Down }
            Button { label: "to redirect" on: Redirect }
            Button { label: "to big" on: Big }
            Button { label: "to slow" on: Slow }
        }
    }
}
`;

function serve(dir, log) {
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url.startsWith('/api/')) {
      let body = '';
      req.on('data', (c) => { body += c; });
      req.on('end', () => {
        log.push({ url, method: req.method, type: req.headers['content-type'], cookie: req.headers.cookie || '', body });
        if (url === '/api/ok') { res.writeHead(201, { 'content-type': 'text/plain' }); res.end('made'); }
        else if (url === '/api/conflict') { res.writeHead(409); res.end('exists'); }
        else if (url === '/api/down') { res.writeHead(503); res.end('down'); }
        else if (url === '/api/redirect') { res.writeHead(302, { location: '/api/ok' }); res.end(); }
        else if (url === '/api/big') { res.writeHead(200); res.end('x'.repeat(2 * 1048576)); }
        else if (url === '/api/slow') { setTimeout(() => { res.writeHead(201); res.end('slow'); }, 600); }
        else { res.writeHead(404); res.end(); }
      });
      return;
    }
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store', 'set-cookie': 'sid=abc; Path=/' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-post-'));
  const otherLog = [];
  const other = await new Promise((ok) => {
    const s = http.createServer((req, res) => { otherLog.push(req.method + ' ' + req.url); res.writeHead(200, { 'access-control-allow-origin': '*' }); res.end('other'); });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const src = path.join(dir, 'w.cui');
  fs.writeFileSync(src, SRC(`http://localhost:${other.address().port}/api/ok`));
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const log = [];
  const server = await serve(path.join(dir, 'out'), log);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(url);
  await page.waitForSelector('#root *');
  const shown = async (want, ms = 3000) => {
    for (let t = 0; t < ms; t += 30) {
      const text = await page.locator('#root').innerText();
      if (text.includes(want)) { return true; }
      await page.waitForTimeout(30);
    }
    return false;
  };

  await page.click('text=Save');
  check('a POST of this site is answered with its status and body', await shown('201 made'));
  const first = log[log.length - 1] || {};
  check('it is a POST of the body', first.method === 'POST' && first.body === "[1,2]", JSON.stringify(first));
  check('with the JSON content type', /^application\/json/.test(first.type || ''), String(first.type));
  check('and this origin\'s cookie', /sid=abc/.test(first.cookie || ''), String(first.cookie));

  await page.click('text=to conflict');
  await page.click('text=Save');
  check('a 409 is an answer, not an error', await shown('409 exists'));
  await page.click('text=to down');
  await page.click('text=Save');
  check('a 503 is an answer, not an error', await shown('503 down'));

  const before = otherLog.length;
  await page.click('text=Elsewhere');
  check('another origin is blocked before any request', await shown('err blocked: a write reaches only this site'));
  await page.waitForTimeout(200);
  check('and that server saw 0 requests', otherLog.length === before && before === 0, String(otherLog.length));

  await page.click('text=to redirect');
  await page.click('text=Save');
  check('a redirect is refused', await shown('err blocked: redirect'));
  const redirectHits = log.filter((l) => l.url === '/api/redirect').length;
  check('and not followed: /api/ok saw only the first POST', log.filter((l) => l.url === '/api/ok').length === 1, `redirect hits ${redirectHits}`);

  await page.click('text=to big');
  await page.click('text=Save');
  check('a 2 MiB reply is `too large`', await shown('err too large'));

  // a write is not aborted when a newer one supersedes it: the server completes both
  await page.click('text=to slow');
  await page.click('text=Save');
  await page.click('text=Save');
  check('the newer write\'s answer is shown', await shown('201 slow'));
  await page.waitForTimeout(800);
  check('and the server received both writes, neither cut off', log.filter((l) => l.url === '/api/slow').length === 2, String(log.filter((l) => l.url === '/api/slow').length));

  server.close();
  server.closeAllConnections();
  await page.click('text=to ok');
  await page.click('text=Save');
  check('with the server stopped the answer is `network error`', await shown('err network error', 5000));
  check('no script errors', errors.length === 0, errors.join(' | '));

  await browser.close();
  other.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
