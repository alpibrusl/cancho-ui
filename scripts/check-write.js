#!/usr/bin/env node
// Browser-side checks of `net.put` and `net.delete` (docs/adapters.md section 21), in Chromium against real HTTP servers:
// the method, the body, the JSON content type (PUT) and its absence (DELETE), this origin's cookie and the declared
// credential's bearer token arrive as stated; 204, 404 and 409 are answers; another origin is blocked and that server
// sees 0 requests; a redirect is refused and not followed; a 2 MiB reply is `too large`; a write superseded by a newer one
// is NOT aborted; a missing credential sends nothing; a stopped server is `network error`.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-write.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TOKEN = 'tok-77c1-SECRET';
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const SRC = (other) => `
component Writer {
    requires NetworkWrite;
    requires Credential("api");

    state note: String = "\\{\\"n\\":1\\}";
    state target: String = "/api/ok";
    state elsewhere: String = "${other}";
    state r: Result[String, String] = Ok("");
    state shown: String = "idle";

    on Put {
        r = await(net.put(target, note));
        shown = match r { Ok(reply) => { "put " + json.get(reply, "status", "?") + " " + json.get(reply, "body", "") } Err(e) => { "err " + e } Cancelled => { "cancelled" } };
    }
    on Del {
        r = await(net.delete(target));
        shown = match r { Ok(reply) => { "del " + json.get(reply, "status", "?") + " " + json.get(reply, "body", "") } Err(e) => { "err " + e } Cancelled => { "cancelled" } };
    }
    on PutElsewhere {
        r = await(net.put(elsewhere, note));
        shown = match r { Ok(reply) => { json.get(reply, "status", "?") } Err(e) => { "err " + e } Cancelled => { "cancelled" } };
    }
    on DelElsewhere {
        r = await(net.delete(elsewhere));
        shown = match r { Ok(reply) => { json.get(reply, "status", "?") } Err(e) => { "err " + e } Cancelled => { "cancelled" } };
    }
    on Ok { target = "/api/ok"; }
    on Gone { target = "/api/gone"; }
    on Conflict { target = "/api/conflict"; }
    on Down { target = "/api/down"; }
    on Redirect { target = "/api/redirect"; }
    on Big { target = "/api/big"; }
    on Slow { target = "/api/slow"; }

    view {
        Column {
            Text { "{shown}" }
            Button { label: "Put" on: Put }
            Button { label: "Del" on: Del }
            Button { label: "PutElsewhere" on: PutElsewhere }
            Button { label: "DelElsewhere" on: DelElsewhere }
            Button { label: "to ok" on: Ok }
            Button { label: "to gone" on: Gone }
            Button { label: "to conflict" on: Conflict }
            Button { label: "to down" on: Down }
            Button { label: "to redirect" on: Redirect }
            Button { label: "to big" on: Big }
            Button { label: "to slow" on: Slow }
        }
    }
}
`;

const PAGE = (args) => `<!doctype html><meta charset="utf-8"><div id="root"></div><script type="module">
import { mount_Writer } from './app.js';
mount_Writer(document.getElementById('root')${args});
</script>`;

function serve(files, log) {
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url.startsWith('/api/')) {
      let body = '';
      req.on('data', (c) => { body += c; });
      req.on('end', () => {
        log.push({ url, method: req.method, type: req.headers['content-type'], cookie: req.headers.cookie || '', auth: req.headers.authorization || null, body });
        if (url === '/api/ok') { res.writeHead(200, { 'content-type': 'text/plain' }); res.end('done'); }
        else if (url === '/api/gone') { res.writeHead(req.method === 'DELETE' ? 204 : 404); res.end(); }
        else if (url === '/api/conflict') { res.writeHead(409); res.end('stale'); }
        else if (url === '/api/down') { res.writeHead(503); res.end('down'); }
        else if (url === '/api/redirect') { res.writeHead(302, { location: '/api/ok' }); res.end(); }
        else if (url === '/api/big') { res.writeHead(200); res.end('x'.repeat(2 * 1048576)); }
        else if (url === '/api/slow') { setTimeout(() => { res.writeHead(200); res.end('slow'); }, 600); }
        else { res.writeHead(404); res.end(); }
      });
      return;
    }
    const f = files[url];
    if (!f) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': f.type, 'cache-control': 'no-store', 'set-cookie': 'sid=abc; Path=/' });
    res.end(f.body);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-write-'));
  const otherLog = [];
  const other = await new Promise((ok) => {
    const s = http.createServer((req, res) => { otherLog.push(req.method + ' ' + req.url); res.writeHead(200, { 'access-control-allow-origin': '*' }); res.end('other'); });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const src = path.join(dir, 'w.cui');
  fs.writeFileSync(src, SRC(`http://localhost:${other.address().port}/api/ok`));
  const r = spawnSync(CANCHO_UI, ['build', src, '--no-auto-mount', '-o', path.join(dir, 'app.js')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr + r.stdout); }
  const log = [];
  const server = await serve({
    '/app.js': { type: 'text/javascript', body: fs.readFileSync(path.join(dir, 'app.js')) },
    '/with.html': { type: 'text/html', body: PAGE(`, { credentials: { api: '${TOKEN}' } }`) },
    '/without.html': { type: 'text/html', body: PAGE('') },
  }, log);
  const base = `http://localhost:${server.address().port}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const errors = [];

  async function session(html) {
    const page = await browser.newPage();
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(`${base}/${html}`);
    await page.waitForSelector('#root *');
    const shown = async (want, ms = 3000) => {
      for (let t = 0; t < ms; t += 30) {
        if ((await page.locator('#root').innerText()).includes(want)) { return true; }
        await page.waitForTimeout(30);
      }
      return false;
    };
    return { page, shown };
  }

  {
    const { page, shown } = await session('with.html');
    await page.click('button:text-is("Put")');
    check('a PUT of this site is answered with its status and body', await shown('put 200 done'));
    const put = log[log.length - 1] || {};
    check('it is a PUT of the body', put.method === 'PUT' && put.body === '{"n":1}', JSON.stringify(put));
    check('with the JSON content type', /^application\/json/.test(put.type || ''), String(put.type));
    check('this origin\'s cookie', /sid=abc/.test(put.cookie || ''), String(put.cookie));
    check('and the declared credential', put.auth === `Bearer ${TOKEN}`, String(put.auth));

    await page.click('button:text-is("Del")');
    check('a DELETE of this site is answered', await shown('del 200 done'));
    const del = log[log.length - 1] || {};
    check('it is a DELETE with no body', del.method === 'DELETE' && del.body === '', JSON.stringify(del));
    check('and no content type', !del.type, String(del.type));
    check('this origin\'s cookie and the credential go with it', /sid=abc/.test(del.cookie) && del.auth === `Bearer ${TOKEN}`, JSON.stringify(del));

    await page.click('text=to gone');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Del")');
    check('a 204 is an answer with an empty body', await shown('del 204'));
    await page.click('button:text-is("Put")');
    check('a 404 is an answer, not an error', await shown('put 404 '));
    await page.click('text=to conflict');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Put")');
    check('a 409 is an answer, not an error', await shown('put 409 stale'));
    await page.click('text=to down');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Del")');
    check('a 503 is an answer, not an error', await shown('del 503 down'));

    const before = otherLog.length;
    await page.click('button:text-is("PutElsewhere")');
    check('a PUT to another origin is blocked before any request', await shown('err blocked: a write reaches only this site'));
    await page.click('text=to ok');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Del")');
    await shown('del 200 done');
    await page.click('button:text-is("DelElsewhere")');
    await shown('err blocked: a write reaches only this site');
    await page.waitForTimeout(200);
    check('and that server saw 0 requests, for PUT and DELETE', otherLog.length === before && before === 0, String(otherLog.length));

    await page.click('text=to redirect');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Put")');
    check('a redirect is refused', await shown('err blocked: redirect'));
    const okHits = log.filter((l) => l.url === '/api/ok').length;
    await page.waitForTimeout(150);
    check('and not followed', log.filter((l) => l.url === '/api/ok').length === okHits, `${okHits}`);

    await page.click('text=to big');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Del")');
    check('a 2 MiB reply is `too large`', await shown('err too large'));

    await page.click('text=to slow');
    await page.waitForTimeout(100);
    const slowBefore = log.filter((l) => l.url === '/api/slow').length;
    await page.click('button:text-is("Put")');
    await page.click('button:text-is("Put")');
    check('the newer write\'s answer is shown', await shown('put 200 slow'));
    await page.waitForTimeout(800);
    check('and the server received both writes, neither cut off', log.filter((l) => l.url === '/api/slow').length - slowBefore === 2);

    server.close();
    server.closeAllConnections();
    await page.click('text=to ok');
    await page.waitForTimeout(100);
    await page.click('button:text-is("Del")');
    check('with the server stopped the answer is `network error`', await shown('err network error', 5000));
  }

  {
    const log2 = [];
    const s2 = await serve({
      '/app.js': { type: 'text/javascript', body: fs.readFileSync(path.join(dir, 'app.js')) },
      '/without.html': { type: 'text/html', body: PAGE('') },
    }, log2);
    const page = await browser.newPage();
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(`http://localhost:${s2.address().port}/without.html`);
    await page.waitForSelector('#root *');
    await page.click('button:text-is("Put")');
    await page.waitForFunction(() => document.querySelector('#root').innerText.includes('err blocked: no credential api'), null, { timeout: 3000 }).catch(() => {});
    check('without the credential a PUT is refused', (await page.locator('#root').innerText()).includes('err blocked: no credential api'));
    await page.click('button:text-is("Del")');
    await page.waitForTimeout(200);
    check('and so is a DELETE, and the server saw 0 requests', log2.filter((l) => l.url.startsWith('/api/')).length === 0, String(log2.length));
    s2.close();
  }

  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  other.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
