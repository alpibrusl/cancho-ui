#!/usr/bin/env node
// The emitted `__cui_fin` and `__cui_add` against tests/arith_corpus.json (docs/semantics.md, "Arithmetic"), the same file
// the Rust model is checked against by `cargo test`, and a page in Chromium where `1 / 0` throws instead of showing
// `Infinity`. Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-arith.js [path/to/cancho-ui]
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

const SRC = `
component Calc {
    state n: Int = 7;
    state m: Int = 0;
    state s: String = "";
    state big: Int = 1000000000;

    on Divide { n = 100 / m; }
    on Guarded { s = "before"; n = 1 / m; s = "after"; }
    on Safe { m = 4; n = 100 / m + 1; s = "n=" + n; }
    on Grow { big = big * big; big = big * big; big = big * big; big = big * big; big = big * big; big = big * big; }
    view {
        Column {
            Button { label: "Divide" on: Divide }
            Button { label: "Guarded" on: Guarded }
            Button { label: "Safe" on: Safe }
            Button { label: "Grow" on: Grow }
            Text { "n {n}" }
            Text { "s {s}" }
            Text { "big {big}" }
        }
    }
}
`;

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-arith-'));
  const src = path.join(dir, 'p.cui');
  fs.writeFileSync(src, SRC);
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }

  const js = fs.readFileSync(path.join(dir, 'out', fs.readdirSync(path.join(dir, 'out')).find((f) => f.endsWith('.js') && f !== 'sw.js')), 'utf8');
  const m = js.match(/function __cui_fin[\s\S]*?\n}\nfunction __cui_add[\s\S]*?\n}\n/);
  if (!m) { throw new Error('the arithmetic helpers are not in the built program'); }
  const lib = new Function(m[0] + '\nreturn { fin: __cui_fin, add: __cui_add };')();
  const ops = {
    '+': (a, b) => lib.add(a, b),
    '-': (a, b) => lib.fin(a - b),
    '*': (a, b) => lib.fin(a * b),
    '/': (a, b) => lib.fin(a / b),
  };
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'arith_corpus.json'), 'utf8'));
  for (const e of corpus) {
    let got;
    try { got = ops[e.op](e.a, e.b); } catch (err) { got = String(err.message).startsWith('replay-non-finite') ? 'fault' : `error ${err.message}`; }
    const want = e.expect;
    check(`${e.a} ${e.op} ${e.b}: ${e.why}`, want === 'fault' ? got === 'fault' : Object.is(got, want), `got ${Object.is(got, -0) ? '-0' : got}`);
  }
  check('+ with a string is concatenation, unchecked', lib.add('a', 1) === 'a1' && lib.add(2, 'b') === '2b' && lib.add('', 'x') === 'x');

  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, 'out', path.normalize(rel));
    if (!file.startsWith(path.join(dir, 'out')) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  // A handler is an async function, so a throw is an unhandled rejection rather than an uncaught exception.
  const errors = [];
  await page.exposeFunction('__report', (m) => errors.push(String(m)));
  await page.addInitScript(() => {
    window.addEventListener('unhandledrejection', (ev) => { window.__report(ev.reason && ev.reason.message ? ev.reason.message : String(ev.reason)); });
    window.addEventListener('error', (ev) => { window.__report(ev.message); });
  });
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  const text = async () => page.locator('#root').innerText();
  await page.click('text=Divide');
  await page.waitForTimeout(50);
  check('100 / 0 on the page throws a replay-non-finite error', errors.length === 1 && errors[0].includes('replay-non-finite'), errors.join(' | '));
  check('and the page does not show Infinity', !(await text()).includes('Infinity') && (await text()).includes('n 7'), (await text()).split('\n').find((l) => l.startsWith('n ')));
  await page.click('text=Guarded');
  await page.waitForTimeout(50);
  check('a throw stops the handler: the statement after it does not run', errors.length === 2 && !(await text()).includes('s after'), (await text()).split('\n').find((l) => l.startsWith('s ')));
  await page.click('text=Grow');
  await page.waitForTimeout(50);
  check('an overflow by multiplication throws too', errors.length === 3 && !(await text()).includes('Infinity'), errors[2]);
  await page.click('text=Safe');
  check('the same arithmetic with a non-zero divisor runs', errors.length === 3 && (await text()).includes('s n=26') && (await text()).includes('n 26'), (await text()).split('\n').filter((l) => /^(n|s) /.test(l)).join(' / '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
