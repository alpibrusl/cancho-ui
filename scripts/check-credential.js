#!/usr/bin/env node
// Browser-side checks of `requires Credential(..)` (docs/adapters.md section 18), in Chromium against three real HTTP
// servers: this page's own origin, a second origin the component's `Network` allows AND its `Credential` names, and a
// third origin its `Network` allows but its `Credential` does not name. The credential must reach the first two as
// `Authorization: Bearer <token>`, and never the third, never a request made without the token being supplied, and never a
// blocked origin (which sees no request at all). Outside the gate: needs node, the `playwright` package with a Chromium,
// and a built `cancho-ui`.
//   node scripts/check-credential.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TOKEN = 'tok-3f9a-SECRET';
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const SRC = (named, unnamed) => `
component Console {
    requires Network("${named}", "${unnamed}");
    requires NetworkWrite;
    requires Credential("api", "${named}");

    state target: String = "/own";
    state shown: String = "idle";
    state r: Result[String, String] = Ok("");

    on Own { target = "/own"; }
    on Named { target = "http://${named}/named"; }
    on Unnamed { target = "http://${unnamed}/unnamed"; }
    on Go {
        r = await(net.fetch(target));
        shown = match r {
            Ok(reply) => { json.get(reply, "status", "?") + " " + json.get(reply, "body", "") }
            Err(e) => { "err " + e }
            Cancelled => { "cancelled" }
        };
    }
    on Post {
        r = await(net.post("/write", "[1]"));
        shown = match r {
            Ok(reply) => { "post " + json.get(reply, "status", "?") }
            Err(e) => { "err " + e }
            Cancelled => { "cancelled" }
        };
    }

    view {
        Column {
            Text { "{shown}" }
            Button { label: "to own" on: Own }
            Button { label: "to named" on: Named }
            Button { label: "to unnamed" on: Unnamed }
            Button { label: "Go" on: Go }
            Button { label: "Post" on: Post }
        }
    }
}
`;

function recorder(log, label, files) {
  return new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const url = req.url.split('?')[0];
      if (files && files[url]) {
        res.writeHead(200, { 'content-type': files[url].type, 'cache-control': 'no-store' });
        res.end(files[url].body);
        return;
      }
      const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization, content-type', 'access-control-allow-methods': 'GET, POST' };
      if (req.method === 'OPTIONS') { res.writeHead(204, cors); res.end(); return; }
      log.push({ server: label, method: req.method, url, auth: req.headers.authorization || null });
      res.writeHead(200, Object.assign({ 'content-type': 'text/plain' }, cors));
      res.end(label);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-credential-'));
  const log = [];
  const named = await recorder(log, 'named', null);
  const unnamed = await recorder(log, 'unnamed', null);
  const namedHost = `localhost:${named.address().port}`;
  const unnamedHost = `localhost:${unnamed.address().port}`;
  const src = path.join(dir, 'c.cui');
  fs.writeFileSync(src, SRC(namedHost, unnamedHost));
  const r = spawnSync(CANCHO_UI, ['build', src, '--no-auto-mount', '-o', path.join(dir, 'app.js')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr + r.stdout); }
  const page = (mountArgs) => `<!doctype html><meta charset="utf-8"><div id="root"></div><script type="module">
import { mount_Console } from './app.js';
mount_Console(document.getElementById('root')${mountArgs});
</script>`;
  const files = {
    '/app.js': { type: 'text/javascript', body: fs.readFileSync(path.join(dir, 'app.js')) },
    '/with.html': { type: 'text/html', body: page(`, { credentials: { api: '${TOKEN}' } }`) },
    '/without.html': { type: 'text/html', body: page('') },
    '/bad.html': { type: 'text/html', body: page(`, { credentials: { api: 'has space' } }`) },
    '/other.html': { type: 'text/html', body: page(`, { credentials: { other: '${TOKEN}' } }`) },
    '/inherit.html': { type: 'text/html', body: page(`, { credentials: Object.create({ api: '${TOKEN}' }) }`) },
  };
  const own = await recorder(log, 'own', files);
  const base = `http://localhost:${own.address().port}`;
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
  const since = (n) => log.slice(n);

  // 1. supplied: this origin, the named host, the unnamed (allowed by Network) host
  {
    const { page, shown } = await session('with.html');
    let n = log.length;
    await page.click('text=Go');
    check('supplied: a request to this origin is answered', await shown('200 own'));
    check('and carries the bearer token', since(n).some((l) => l.server === 'own' && l.url === '/own' && l.auth === `Bearer ${TOKEN}`), JSON.stringify(since(n)));
    n = log.length;
    await page.click('text=to named'); await page.click('text=Go');
    check('supplied: a request to the host the credential names is answered', await shown('200 named'));
    check('and carries the bearer token', since(n).some((l) => l.server === 'named' && l.method === 'GET' && l.auth === `Bearer ${TOKEN}`), JSON.stringify(since(n)));
    n = log.length;
    await page.click('text=to unnamed'); await page.click('text=Go');
    check('supplied: a host Network allows but the credential does not name is answered', await shown('200 unnamed'));
    check('and the token is NOT sent to it', since(n).filter((l) => l.server === 'unnamed').length > 0 && since(n).filter((l) => l.server === 'unnamed').every((l) => l.auth === null), JSON.stringify(since(n)));
    n = log.length;
    await page.click('text=Post');
    check('supplied: a write is answered', await shown('post 200'));
    check('and the write carries the token', since(n).some((l) => l.server === 'own' && l.method === 'POST' && l.auth === `Bearer ${TOKEN}`), JSON.stringify(since(n)));
    const html = await page.locator('#root').innerHTML();
    check('the token is nowhere in what the component rendered', !html.includes(TOKEN));
    await page.close();
  }
  // 2. not supplied: every request is refused before it is made
  for (const [file, why] of [['without.html', 'not supplied'], ['other.html', 'a different name supplied'], ['inherit.html', 'only an inherited property'], ['bad.html', 'a token with a space']]) {
    const { page, shown } = await session(file);
    const n = log.length;
    await page.click('text=Go');
    const want = file === 'bad.html' ? 'err blocked: credential api is not a plain token' : 'err blocked: no credential api';
    check(`${why}: the request is refused with a value`, await shown(want));
    await page.click('text=Post');
    check(`${why}: so is a write`, await shown(want));
    check(`${why}: and the server saw no request at all`, since(n).length === 0, JSON.stringify(since(n)));
    await page.close();
  }
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  named.close(); unnamed.close(); own.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
