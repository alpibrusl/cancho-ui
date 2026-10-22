#!/usr/bin/env node
// A full localStorage (docs/adapters.md section 20, issue #66). Two parts, outside the gate; needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`:
//   1. the emitted Storage adapter over a FAKE localStorage whose quota rule is the replay model's, against
//      tests/storage_quota_corpus.json (the same file `cargo test` replays);
//   2. the real Chromium quota: the page's storage is filled until it throws, then a save is refused, the older value is kept,
//      the page keeps working, and it reads back what it last wrote.
//   node scripts/check-storage-quota.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const SRC = `
component Keep {
    requires Storage;
    state saved: Bool = true;
    state got: String = "";
    state text: String = "";
    on Set(v) { text = v; }
    on Save { saved = storage.set("note", text); got = storage.get("note", "none"); }
    on Read { got = storage.get("note", "none"); }
    view {
        Column {
            TextField { label: "Note" value: text on: Set }
            Button { label: "Save" on: Save }
            Button { label: "Read" on: Read }
            Text { "saved: {saved}" }
            Text { "got: {got}" }
        }
    }
}
`;

function build(dir) {
  const src = path.join(dir, 'keep.cui');
  fs.writeFileSync(src, SRC);
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const out = path.join(dir, 'out');
  const js = fs.readFileSync(path.join(out, fs.readdirSync(out).find((f) => f.endsWith('.js') && f !== 'sw.js')), 'utf8');
  return { out, js };
}

// The adapter's source, cut out of the built program.
function adapterOf(js) {
  const from = js.indexOf('var __cui_storage_memory');
  const setAt = js.indexOf('function __cui_storage_set', from);
  const to = js.indexOf('\n}\n', setAt) + 3;
  if (from < 0 || setAt < 0 || to < 3) { throw new Error('the storage adapter is not in the built program'); }
  return js.slice(from, to);
}

function fakeLocalStorage(seed, quota) {
  const m = new Map(Object.entries(seed).map(([k, v]) => ['cui:' + k, v]));
  const units = (k, v) => k.length + v.length;
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => {
      let others = 0;
      for (const [kk, vv] of m) { if (kk !== k) { others += units(kk, vv); } }
      if (quota !== null && others + units(k, String(v)) > quota) { throw new Error('QuotaExceededError'); }
      m.set(k, String(v));
    },
    dump: () => Object.fromEntries(Array.from(m).map(([k, v]) => [k.slice(4), v])),
  };
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-storage-quota-'));
  const { out, js } = build(dir);

  // 1. the corpus against the emitted adapter
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'storage_quota_corpus.json'), 'utf8'));
  const adapter = adapterOf(js);
  for (const c of corpus.cases) {
    const ls = fakeLocalStorage(c.seed, c.quota);
    const lib = new Function('window', `${adapter}\nreturn { get: __cui_storage_get, set: __cui_storage_set };`)({ localStorage: ls });
    let ok = true;
    let why = '';
    for (const op of c.ops) {
      if (op.set) {
        const got = lib.set(op.set[0], op.set[1], '');
        if (got !== op.ok) { ok = false; why = `set ${JSON.stringify(op.set)} answered ${got}`; }
      } else {
        const got = lib.get(op.get[0], 'none', '');
        if (got !== op.is) { ok = false; why = `get ${JSON.stringify(op.get)} answered ${JSON.stringify(got)}`; }
      }
    }
    if (JSON.stringify(ls.dump()) !== JSON.stringify(c.stored)) { ok = false; why += ` store ${JSON.stringify(ls.dump())}`; }
    check(`adapter: ${c.why}`, ok, why);
  }

  // 2. the real Chromium quota
  const server = http.createServer((req, res) => {
    const f = path.join(out, req.url === '/' ? 'index.html' : req.url.split('?')[0]);
    if (!f.startsWith(out) || !fs.existsSync(f)) { res.writeHead(404); res.end(); return; }
    const type = f.endsWith('.js') ? 'text/javascript' : f.endsWith('.html') ? 'text/html' : f.endsWith('.json') || f.endsWith('.webmanifest') ? 'application/json' : 'application/octet-stream';
    res.writeHead(200, { 'content-type': type });
    res.end(fs.readFileSync(f));
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await (await browser.newContext()).newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(url);
  const body = async () => page.locator('#root').innerText();
  await page.fill('input', 'the older note');
  await page.click('text=Save');
  check('before the quota: the save persists', (await body()).includes('saved: true') && (await body()).includes('got: the older note'), (await body()).replace(/\n/g, ' | '));
  const filled = await page.evaluate(() => {
    let n = 0;
    for (let chunk = 1 << 20; chunk >= 1; chunk = chunk >> 1) {
      for (;;) {
        try { localStorage.setItem('filler' + n, 'x'.repeat(chunk)); n++; } catch (e) { break; }
      }
    }
    return n;
  });
  check('the storage is full (a one-unit write throws)', await page.evaluate(() => { try { localStorage.setItem('probe', 'x'); localStorage.removeItem('probe'); return false; } catch (e) { return true; } }), `${filled} filler keys`);
  await page.fill('input', 'a newer note that cannot be stored');
  await page.click('text=Save');
  check('the save is refused and the page says so (saved: false)', (await body()).includes('saved: false'), (await body()).replace(/\n/g, ' | '));
  check('the page reads back what it last wrote, not the older persisted note', (await body()).includes('got: a newer note that cannot be stored'), (await body()).replace(/\n/g, ' | '));
  check('the older note is still in the store', (await page.evaluate(() => localStorage.getItem('cui:note'))) === 'the older note');
  await page.click('text=Read');
  check('the page keeps working after the refusal', (await body()).includes('got: a newer note that cannot be stored'));
  await page.reload();
  await page.click('text=Read');
  check('after a reload only the persisted note is left', (await body()).includes('got: the older note'), (await body()).replace(/\n/g, ' | '));
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
