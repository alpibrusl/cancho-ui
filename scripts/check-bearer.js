#!/usr/bin/env node
// A bearer token the program receives and sends (docs/bearer-token.md, issue #83) in Chromium, against a stand-in service that answers
// a token in the body of a login and refuses any other request without `Authorization: Bearer <that token>`:
//  A. examples/bearer built with --page (auto-mounted: it needs no token from the page): a wrong password holds no token; a good one is
//     sent on the next GET and on the POST that signs out; the token is in no storage, cookie, window property or serialized DOM; a
//     reload forgets it;
//  B. a variant with a second origin: the token goes to hosts the declaration names and to no other; a value that is not a plain token
//     (a space, empty, 4,097 characters) is answered false and nothing is stored; after `credential.clear`, and after a reload, a request goes
//     out without an Authorization header (a component that sets its own token is anonymous until it holds one).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-bearer.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

// The service: /api/login, /api/me, /api/logout under `dir`'s page; `log` records every request's Authorization header.
function service(dir) {
  const issued = new Set();
  const log = [];
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
  const server = http.createServer((req, res) => {
    const u = req.url.split('?')[0];
    const auth = req.headers.authorization || null;
    if (u.startsWith('/api/')) {
      let body = '';
      req.on('data', (c) => { body += c; });
      req.on('end', () => {
        log.push({ path: u, auth });
        const bearer = auth && auth.startsWith('Bearer ') ? auth.slice(7) : null;
        const send = (status, text, type = 'application/json') => { res.writeHead(status, { 'content-type': type, 'cache-control': 'no-store' }); res.end(text); };
        if (u === '/api/login') {
          let p;
          try { p = JSON.parse(body); } catch (e) { send(400, '"not json"'); return; }
          if (Array.isArray(p) && p[0] === 'ada' && p[1] === 'hunter22!') {
            const t = 't-' + Math.random().toString(16).slice(2, 12);
            issued.add(t);
            send(200, JSON.stringify({ token: t }));
          } else { send(401, '"wrong user name or password"'); }
          return;
        }
        if (u === '/api/logout') { if (bearer) { issued.delete(bearer); } send(200, '"bye"'); return; }
        if (u === '/api/me') { if (bearer && issued.has(bearer)) { send(200, '{"user":"ada"}'); } else { send(401, '"no"'); } return; }
        send(404, '"?"');
      });
      return;
    }
    const f = path.join(dir, u === '/' ? 'index.html' : path.normalize(u));
    if (!f.startsWith(dir) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': types[path.extname(f)] || 'text/plain', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ server, log, port: server.address().port, issued })));
}
// A second origin that only records what it is sent.
function other() {
  const log = [];
  const server = http.createServer((req, res) => {
    log.push({ path: req.url, auth: req.headers.authorization || null });
    res.writeHead(200, { 'content-type': 'application/json', 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization' });
    if (req.method === 'OPTIONS') { res.end(); return; }
    res.end('{"ok":true}');
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ server, log, port: server.address().port })));
}

const variant = (second, named) => `component V {
    requires Network("localhost:${second}");
    requires NetworkWrite;
    requires Credential("api"${named ? `, "localhost:${second}"` : ''});

    state user: String = "";
    state pass: String = "";
    state shown: String = "idle";
    state login: Result[String, String] = Ok("");
    state r: Result[String, String] = Ok("");
    state out: Result[String, String] = Ok("");
    state w: Bool = true;

    on User(v) { user = v; }
    on Pass(v) { pass = v; }
    on SignIn {
        login = await(net.post("api/login", "[" + json.quote(user) + "," + json.quote(pass) + "]"));
        w = match login {
            Ok(reply) => { credential.set("api", json.get(json.get(reply, "body", ""), "token", "")) }
            Err(_) => { false }
            Cancelled => { false }
        };
        login = Ok("");
        shown = if w { "signed in" } else { "refused" };
    }
    on Probe {
        r = await(net.fetch("api/me"));
        shown = match r {
            Ok(x) => { "me " + json.get(x, "status", "?") }
            Err(e) => { "err " + e }
            Cancelled => { "cancelled" }
        };
    }
    on Other {
        r = await(net.fetch("http://localhost:${second}/other"));
        shown = match r {
            Ok(x) => { "other " + json.get(x, "status", "?") }
            Err(e) => { "err " + e }
            Cancelled => { "cancelled" }
        };
    }
    on Space { w = credential.set("api", "has space"); shown = "space " + w; }
    on Empty { w = credential.set("api", ""); shown = "empty " + w; }
    on Long { w = credential.set("api", "${'x'.repeat(4097)}"); shown = "long " + w; }
    on Out {
        out = await(net.post("api/logout", "[]"));
        w = credential.clear("api");
        shown = "cleared " + w;
    }
    view {
        Column {
            Text { role: status "{shown}" }
            TextField { label: "User name" value: user on: User }
            TextField { label: "Password" value: pass on: Pass type: "password" }
            Button { label: "Sign in" on: SignIn }
            Button { label: "Probe" on: Probe }
            Button { label: "Other" on: Other }
            Button { label: "Space" on: Space }
            Button { label: "Empty" on: Empty }
            Button { label: "Long" on: Long }
            Button { label: "Out" on: Out }
        }
    }
}
`;

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-bearer-'));
  const build = (name, file) => {
    const dir = path.join(tmp, name);
    const r = spawnSync(CANCHO_UI, ['build', file, '--page', '-o', dir, '--name', name], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stdout}${r.stderr}`); }
    return dir;
  };
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const text = (page) => page.evaluate(() => document.body.innerText);

  // ---- A. the example ----
  {
    const svc = await service(build('bearer', path.join(ROOT, 'examples/bearer/bearer.cui')));
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => { if (m.type() === 'error' && !/401/.test(m.text())) { errors.push(m.text()); } });
    await page.goto(`http://localhost:${svc.port}/`);
    await page.waitForSelector('#root button');
    await page.getByRole('textbox', { name: 'User name' }).fill('ada');
    await page.getByRole('textbox', { name: 'Password' }).fill('wrong-pass');
    await page.getByRole('button', { name: 'Sign in' }).tap();
    await page.waitForFunction(() => /Wrong user name or password\./.test(document.body.innerText), null, { timeout: 5000 });
    check('A: a wrong password holds no token: the service never saw a request that asked for one', svc.log.every((l) => l.auth === null) && svc.log.length === 1, JSON.stringify(svc.log));
    await page.getByRole('textbox', { name: 'Password' }).fill('hunter22!');
    await page.getByRole('button', { name: 'Sign in' }).tap();
    await page.waitForFunction(() => /Hello ada/.test(document.body.innerText), null, { timeout: 5000 });
    const token = [...svc.issued][0];
    const me = svc.log.filter((l) => l.path === '/api/me');
    check('A: the sign-in\'s token is sent on the next GET, as a Bearer header', me.length === 1 && me[0].auth === `Bearer ${token}`, JSON.stringify(me));
    await page.getByRole('button', { name: 'Ask again' }).tap();
    await page.waitForTimeout(150);
    check('A: and on a later one', svc.log.filter((l) => l.path === '/api/me').length === 2 && svc.log.filter((l) => l.path === '/api/me').every((l) => l.auth === `Bearer ${token}`));
    const where = await page.evaluate((T) => {
      const hits = [];
      for (const k of Object.getOwnPropertyNames(window)) { try { const v = window[k]; if (typeof v === 'string' && v.includes(T)) { hits.push(`window.${k}`); } } catch (e) { /* a getter that throws */ } }
      for (let i = 0; i < localStorage.length; i++) { if (localStorage.getItem(localStorage.key(i)).includes(T)) { hits.push('localStorage'); } }
      for (let i = 0; i < sessionStorage.length; i++) { if (sessionStorage.getItem(sessionStorage.key(i)).includes(T)) { hits.push('sessionStorage'); } }
      if (document.cookie.includes(T)) { hits.push('document.cookie'); }
      if (document.documentElement.outerHTML.includes(T)) { hits.push('the DOM'); }
      return hits;
    }, token);
    check('A: the token is in no storage, cookie, window property or the serialized DOM', where.length === 0, JSON.stringify(where));
    await page.getByRole('button', { name: 'Sign out' }).tap();
    await page.waitForFunction(() => /You are signed out\./.test(document.body.innerText), null, { timeout: 5000 });
    const logout = svc.log.filter((l) => l.path === '/api/logout');
    check('A: the POST that signs out carries the token, then it is forgotten', logout.length === 1 && logout[0].auth === `Bearer ${token}`, JSON.stringify(logout));
    await page.getByRole('textbox', { name: 'User name' }).fill('ada');
    await page.getByRole('textbox', { name: 'Password' }).fill('hunter22!');
    await page.getByRole('button', { name: 'Sign in' }).tap();
    await page.waitForFunction(() => /Hello ada/.test(document.body.innerText), null, { timeout: 5000 });
    await page.reload();
    await page.waitForSelector('#root button');
    check('A: a reload forgets the token: the sign-in screen is back', (await text(page)).includes('Sign in') && !(await text(page)).includes('Hello ada'));
    check('A: no page error', errors.length === 0, errors.join('; '));
    await ctx.close();
    svc.server.close();
  }

  // ---- B. the variant with a second origin ----
  for (const named of [false, true]) {
    const o = await other();
    const dir = path.join(tmp, `variant-${named}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'v.cui'), variant(o.port, named));
    const svc = await service(build(`variant-${named}`, path.join(dir, 'v.cui')));
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    const tag = `B(${named ? 'host named' : 'host not named'})`;
    const press = async (name) => { await page.getByRole('button', { name, exact: true }).tap(); await page.waitForTimeout(250); };
    await page.goto(`http://localhost:${svc.port}/`);
    await page.waitForSelector('#root button');
    await press('Probe');
    const lastMe = () => svc.log.filter((l) => l.path === '/api/me').pop();
    check(`${tag}: before any sign-in a request goes out without an Authorization header (the sign-in itself needs that), and is refused`, (await text(page)).includes('me 401') && lastMe().auth === null, `${(await text(page)).split('\n')[0]} ${JSON.stringify(svc.log)}`);
    for (const [name, word] of [['Space', 'space false'], ['Empty', 'empty false'], ['Long', 'long false']]) {
      await press(name);
      check(`${tag}: ${name} (not a plain token) is answered false`, (await text(page)).includes(word), (await text(page)).split('\n')[0]);
    }
    await press('Probe');
    check(`${tag}: and stored nothing: a request still carries no header`, (await text(page)).includes('me 401') && lastMe().auth === null, JSON.stringify(svc.log));
    await page.getByRole('textbox', { name: 'User name' }).fill('ada');
    await page.getByRole('textbox', { name: 'Password' }).fill('hunter22!');
    await press('Sign in');
    check(`${tag}: a good sign-in holds the token`, (await text(page)).includes('signed in'));
    await press('Other');
    const seen = o.log.filter((l) => l.path === '/other' && l.auth !== undefined);
    const token = [...svc.issued][0];
    if (named) {
      check(`${tag}: the second origin the credential names receives it`, seen.some((l) => l.auth === `Bearer ${token}`), JSON.stringify(o.log));
    } else {
      check(`${tag}: the second origin the credential does not name receives no Authorization`, seen.length >= 1 && seen.every((l) => l.auth === null), JSON.stringify(o.log));
    }
    await press('Probe');
    check(`${tag}: this origin receives it`, (await text(page)).includes('me 200') && svc.log.filter((l) => l.path === '/api/me').pop().auth === `Bearer ${token}`);
    await press('Out');
    await press('Probe');
    check(`${tag}: after credential.clear a request carries no header and is refused`, (await text(page)).includes('me 401') && lastMe().auth === null, `${(await text(page)).split('\n')[0]} ${JSON.stringify(lastMe())}`);
    await press('Space');
    await press('Probe');
    check(`${tag}: and a refused value does not bring it back`, (await text(page)).includes('me 401') && lastMe().auth === null);
    await page.getByRole('textbox', { name: 'User name' }).fill('ada');
    await page.getByRole('textbox', { name: 'Password' }).fill('hunter22!');
    await press('Sign in');
    await page.reload();
    await page.waitForSelector('#root button');
    await press('Probe');
    check(`${tag}: after a reload a request carries no header again (the token is not persisted)`, (await text(page)).includes('me 401') && lastMe().auth === null);
    check(`${tag}: no page error`, errors.length === 0, errors.join('; '));
    await ctx.close();
    svc.server.close();
    o.server.close();
  }

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
