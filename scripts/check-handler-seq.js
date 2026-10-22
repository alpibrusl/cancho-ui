#!/usr/bin/env node
// Every case of tests/stmt_if_corpus.json (docs/handler-sequences.md) run in the emitted page, in Chromium: an `if` statement
// issues a request only when its condition holds, what follows an `await` inside a branch runs when the answer arrives, a block
// run from two handlers behaves as if written out, a superseded request abandons the rest of the handler, and a fault inside a
// branch undoes the writes since the last commit point. The requests the page makes are compared with the `issued` list that
// `cancho-ui test` derives for the same case (cargo test replays the same file: crates/cancho-ui/tests/handler_seq.rs).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-handler-seq.js [path/to/cancho-ui]
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
const lines = async (page) => (await page.locator('#root').innerText()).split('\n').map((s) => s.trim()).filter(Boolean);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const settle = (page) => page.waitForTimeout(80);

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-handler-seq-'));
  let served = root;
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(served, path.normalize(rel));
    if (!file.startsWith(served) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'stmt_if_corpus.json'), 'utf8'));

  let n = 0;
  for (const c of corpus) {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), c.source.join('\n') + '\n');
    const built = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (built.status !== 0) { check(c.why, false, `build failed: ${built.stderr}`); continue; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    const errors = [];
    const requests = []; // in the order the page made them: { url, route }
    await page.exposeFunction('__report', (m) => errors.push(String(m)));
    await page.addInitScript(() => {
      window.addEventListener('unhandledrejection', (ev) => { window.__report(ev.reason && ev.reason.message ? ev.reason.message : String(ev.reason)); });
      window.addEventListener('error', (ev) => { window.__report(ev.message); });
    });
    await page.route('**/q/**', (route) => { requests.push({ url: new URL(route.request().url()).pathname.slice(1), route }); });
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root *');
    check(`${c.why} [initial]`, same(await lines(page), c.initial), JSON.stringify(await lines(page)));
    let seen = 0;
    let i = 0;
    for (const s of c.steps) {
      const label = `${c.why} [step ${i++}: ${s.event ? s.event : `resolve #${s.resolve}`}]`;
      const errorsBefore = errors.length;
      if (s.event) {
        await page.click(`#root button[aria-label="${s.label}"]`);
      } else {
        const r = requests[s.resolve - 1];
        if (!r) { check(`${label}: the request was made`, false, `${requests.length} made`); continue; }
        await r.route.fulfill({ status: s.answer.status, contentType: 'text/plain', body: s.answer.body }).catch(() => {});
      }
      await settle(page);
      const made = requests.slice(seen).map((r) => r.url);
      seen = requests.length;
      check(`${label}: the requests issued`, same(made, s.issued), `page ${JSON.stringify(made)}, replay ${JSON.stringify(s.issued)}`);
      const raised = errors.slice(errorsBefore);
      if (s.fault) {
        check(`${label}: the fault`, raised.length === 1 && raised[0].startsWith(s.fault), JSON.stringify(raised));
      } else {
        check(`${label}: no error`, raised.length === 0, JSON.stringify(raised));
      }
      check(`${label}: the page`, same(await lines(page), s.texts), JSON.stringify(await lines(page)));
    }
    await page.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
