#!/usr/bin/env node
// Every case of tests/cond_corpus.json (docs/conditionals.md) run in the emitted page, in Chromium: `&&`, `||`, `!` and
// `if` give the answer cancho-ui test gives (the same file is replayed by `cargo test`), a branch that is not taken is not
// evaluated (a division by zero there does nothing), and one that is taken throws `replay-non-finite`, the page's form of
// replay's fault. Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-conditionals.js [path/to/cancho-ui]
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

const DEFAULTS = { a: 7, b: 0, t: true, f: false, s: 'x' };
function program(expr, state) {
  const v = { ...DEFAULTS, ...(state || {}) };
  return `component Cond {
    state a: Int = ${v.a};
    state b: Int = ${v.b};
    state t: Bool = ${v.t};
    state f: Bool = ${v.f};
    state s: String = ${JSON.stringify(v.s)};
    state out: String = "unset";
    on Go { out = ${expr}; }
    view { Column { Button { label: "Go" on: Go } Text { "out={out}" } } }
}
`;
}

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-cond-'));
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
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'cond_corpus.json'), 'utf8'));
  let n = 0;
  for (const e of corpus) {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), program(e.expr, e.state));
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { check(e.why, false, `build failed: ${r.stderr}`); continue; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    const errors = [];
    await page.exposeFunction('__report', (m) => errors.push(String(m)));
    await page.addInitScript(() => {
      window.addEventListener('unhandledrejection', (ev) => { window.__report(ev.reason && ev.reason.message ? ev.reason.message : String(ev.reason)); });
      window.addEventListener('error', (ev) => { window.__report(ev.message); });
    });
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root *');
    await page.click('text=Go');
    await page.waitForTimeout(30);
    const shown = (await page.locator('#root').innerText()).split('\n').find((l) => l.startsWith('out=')).slice(4);
    if (e.expect === 'fault') {
      check(`${e.expr} [${e.why}]`, errors.length === 1 && errors[0].startsWith('replay-non-finite') && shown === 'unset', `errors ${JSON.stringify(errors)}, shown ${shown}`);
    } else {
      const want = typeof e.expect === 'string' ? e.expect : JSON.stringify(e.expect);
      check(`${e.expr} [${e.why}]`, errors.length === 0 && shown === want, `errors ${JSON.stringify(errors)}, shown ${shown}, want ${want}`);
    }
    await page.close();
  }
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
