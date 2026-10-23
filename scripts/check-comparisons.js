#!/usr/bin/env node
// Every accepted case of tests/compare_corpus.json (docs/comparisons.md) run in the emitted page, in Chromium: `==`, `!=`, `<`,
// `<=`, `>` and `>=` between the pairs of types the compiler accepts give the answer `cancho-ui test` gives (the same file is
// replayed by `cargo test`: crates/cancho-ui/tests/comparisons.rs), and every refused case is refused by `cancho-ui build` too.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-comparisons.js [path/to/cancho-ui]
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
const program = (expr) => `component Cmp {
    state a: Int = 7;
    state f: Float = 7.5;
    state s: String = "7";
    state t: Bool = true;
    state l: List[String] = ["x"];
    state r: Result[String, String] = Ok("7");
    state reply: String = "\\{\\"status\\":200,\\"body\\":\\"ok\\"\\}";
    state out: String = "unset";
    on Go { out = "" + (${expr}); }
    view { Column { Button { label: "Go" on: Go } Text { "out={out}" } } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-cmp-'));
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
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'compare_corpus.json'), 'utf8'));
  let n = 0;
  for (const e of corpus.accepted) {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), program(e.expr));
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { check(e.why, false, `build failed: ${r.stderr}`); continue; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (x) => errors.push(String(x)));
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root *');
    await page.click('text=Go');
    await page.waitForTimeout(30);
    const shown = ((await page.locator('#root').innerText()).match(/out=(.*)/) || [])[1];
    check(`${e.expr} [${e.why}]`, errors.length === 0 && shown === String(e.expect), `shown ${shown}, want ${e.expect}, errors ${JSON.stringify(errors)}`);
    await page.close();
  }
  for (const e of corpus.refused) {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), program(e.expr));
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '-o', path.join(dir, 'out.html')], { encoding: 'utf8' });
    check(`${e.expr} is refused by build [${e.why}]`, r.status !== 0 && /comparison-type-mismatch/.test(r.stderr + r.stdout), `${r.status}`);
  }
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
