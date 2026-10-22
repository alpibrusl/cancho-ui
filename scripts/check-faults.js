#!/usr/bin/env node
// Every case of tests/fault_corpus.json (docs/semantics.md, "A handler that faults"; issue #59) run in the emitted page, in
// Chromium: when a statement of a handler throws, the writes made since the last commit point are undone, the page is
// redrawn from the committed state and the handler stops; the writes before an await stay; an event that ran while the
// handler was suspended is not undone. The same file is replayed by `cargo test` (cancho-ui test with `"fault"` steps).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-faults.js [path/to/cancho-ui]
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

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-faults-'));
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'fault_corpus.json'), 'utf8'));
  const built = {};
  for (const [name, source] of Object.entries(corpus.programs)) {
    const dir = path.join(root, name);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), source);
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stderr}`); }
    built[name] = path.join(dir, 'out');
  }
  let served = null;
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(served, path.normalize(rel));
    // A missing file answers after 250 ms, so that a request is still pending when the next step runs (replay holds it
    // pending until a `resolve` step).
    if (!file.startsWith(served) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { setTimeout(() => { res.writeHead(404); res.end(); }, 250); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  for (const c of corpus.cases) {
    served = built[c.program];
    const page = await browser.newPage();
    const errors = [];
    await page.exposeFunction('__report', (m) => errors.push(String(m)));
    await page.addInitScript(() => {
      window.addEventListener('unhandledrejection', (ev) => { window.__report(ev.reason && ev.reason.message ? ev.reason.message : String(ev.reason)); });
      window.addEventListener('error', (ev) => { window.__report(ev.message); });
    });
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root *');
    const shown = async () => {
      const line = (await page.locator('#root').innerText()).split('\n').find((l) => l.startsWith('a='));
      const out = {};
      for (const m of line.matchAll(/(\w+)=(.*?)(?= \w+=|$)/g)) { out[m[1]] = m[2]; }
      return out;
    };
    let ok = true;
    const why = [];
    let i = 0;
    for (const step of c.steps) {
      const before = errors.length;
      if (step.event) { await page.getByRole('button', { name: step.event, exact: true }).click(); await page.waitForTimeout(40); }
      else if (step.advance) { await page.waitForTimeout(step.advance + 120); }
      else if (step.resolve) { await page.waitForTimeout(450); }
      const now = errors.slice(before);
      if (step.fault) {
        const want = c.js_fault || step.fault;
        if (now.length !== 1 || !now[0].startsWith(want)) { ok = false; why.push(`step ${i}: expected one ${want} error, got ${JSON.stringify(now)}`); }
      } else if (now.length !== 0) { ok = false; why.push(`step ${i}: unexpected ${JSON.stringify(now)}`); }
      // The screen first (a handler that throws must still leave the page showing the state it keeps), then, after the
      // no-op event `Probe` has redrawn from the real state, again (a screen that happens to look right is not enough).
      for (const phase of ['screen', 'state']) {
        if (phase === 'screen' && step.shows) {
          const lines = (await page.locator('#root').innerText()).split('\n');
          if (!lines.includes(step.shows)) { ok = false; why.push(`step ${i}: the page does not show ${step.shows} (${JSON.stringify(lines.filter((l) => l.startsWith('r=')))})`); }
        }
        if (phase === 'state') {
          await page.getByRole('button', { name: 'Probe', exact: true }).click();
          await page.waitForTimeout(30);
        }
        const state = await shown();
        for (const [k, v] of Object.entries(step.state || {})) {
          if (!(k in state) && typeof v === 'object') { continue; } // a Result is not printed: `shows` checks it
          if (state[k] !== String(v)) { ok = false; why.push(`step ${i} (${phase}): ${k} is ${state[k]}, expected ${v}`); }
        }
      }
      i++;
    }
    check(c.why, ok, why.join('; '));
    await page.close();
  }
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
