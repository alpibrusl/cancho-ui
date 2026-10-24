#!/usr/bin/env node
// Comparing a value whose type the compiler cannot see (docs/comparisons.md section 6, issue #103) in Chromium: every case of
// tests/compare_unknown_corpus.json (the file `cargo test` replays) is clicked in a page built from the corpus's program for its place (an event
// parameter, a lambda binding, a `match` binding), and the page must answer what replay answers: the value, or a `replay-type-error` where replay
// faults. Also: the probe of the issue (a TextField given 7); a `For` binding in the view, which replay does not draw (page only); and that a
// comparison of known types stays the host's own operator.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. Given the path of an older compiler it reports how
// many cases that one answers differently (the measurement of the issue): `node scripts/check-compare-unknown.js <old cancho-ui> --report`.
//   node scripts/check-compare-unknown.js [path/to/cancho-ui] [--report]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();

const ROOT = path.join(__dirname, '..');
const args = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const REPORT = process.argv.includes('--report');
const CANCHO_UI = args[0] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const J = JSON.stringify;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-cmpu-'));
  let served = root;
  const server = await new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
      const f = path.join(served, path.normalize(rel));
      if (!f.startsWith(served) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  let n = 0;
  const build = (src) => {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), src);
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--page', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`build: ${r.stdout}${r.stderr}`); }
    return path.join(dir, 'out');
  };
  const open = async (dir) => {
    served = dir;
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e.message || e)));
    await page.goto(url);
    await page.waitForSelector('#root *');
    return { page, errors };
  };

  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'compare_unknown_corpus.json'), 'utf8'));

  // ---- the corpus, in the page ------------------------------------------------------------------------------------------------------------------
  const wrong = [];
  for (const ctx of Object.keys(corpus.programs)) {
    const { page, errors } = await open(build(corpus.programs[ctx]));
    for (const c of corpus.cases.filter((x) => x.ctx === ctx)) {
      errors.length = 0;
      const before = await page.evaluate(() => document.querySelector('#root span').textContent);
      await page.click(`button[aria-label="${c.event}"]`);
      await sleep(15);
      const after = await page.evaluate(() => document.querySelector('#root span').textContent);
      const got = errors.some((e) => e.includes('replay-type-error')) ? 'fault' : (errors.length ? `error: ${errors[0]}` : after);
      void before;
      if (got !== c.want) { wrong.push(`${ctx} ${J(c.left)} ${c.op} ${J(c.right)}: page ${J(got)}, replay ${J(c.want)}`); }
    }
    await page.close();
  }
  if (REPORT) {
    console.log(`${wrong.length} of ${corpus.cases.length} cases answer differently from replay with this compiler`);
    for (const w of wrong.slice(0, 12)) { console.log(`  ${w}`); }
    await browser.close(); server.close();
    process.exit(0);
  }
  check(`the page answers what replay answers for all ${corpus.cases.length} cases (event parameter, lambda binding, match binding)`, wrong.length === 0, wrong.slice(0, 5).join(' | '));

  // ---- the probe of the issue -------------------------------------------------------------------------------------------------------------------
  {
    const dir = build('component F {\n    state out: String = "";\n    on Param(x) { out = "" + (x <= 7) + " " + (x == 7); }\n    view { Column { TextField { label: "v" value: out on: Param } Text { "out={out}" } } }\n}\n');
    const { page, errors } = await open(dir);
    await page.fill('#root input', '7');
    await sleep(30);
    const out = await page.evaluate(() => document.querySelector('#root span').textContent);
    check('the probe (a TextField given 7, handler `x <= 7`) is a replay-type-error in the page, as in replay, and the state is unchanged', errors.some((e) => e.includes('replay-type-error')) && out === 'out=', J({ errors, out }));
    await page.close();
  }

  // ---- a For binding in the view: replay does not draw the view, so only the page can be asked ------------------------------------------------------
  {
    const dir = build('component V {\n    state nums: List[Int] = [3, 9];\n    state strs: List[String] = ["3", "9"];\n    state show: Bool = false;\n    on Toggle { show = !show; }\n    view { Column { Button { label: "Toggle" on: Toggle } For { over: nums as: n Text { "{n <= 5}" } } If { when: show For { over: strs as: s Text { "{s <= 5}" } } } } }\n}\n');
    const { page, errors } = await open(dir);
    const texts = await page.evaluate(() => [...document.querySelectorAll('#root span')].map((e) => e.textContent));
    check('a For binding over numbers is compared as numbers', J(texts) === J(['true', 'false']), J(texts));
    errors.length = 0;
    await page.click('text=Toggle');
    await sleep(30);
    check('a For binding over strings against a number is a replay-type-error in the page, not a coerced answer', errors.some((e) => e.includes('replay-type-error')), J(errors));
    await page.close();
  }

  // ---- known operands stay the host's own operator ---------------------------------------------------------------------------------------------------
  {
    const dir = build('component K {\n    state a: Int = 3;\n    state s: String = "b";\n    state out: String = "";\n    on Go { out = "" + (a <= 7) + (s < "c") + (a == 3) + (s != "z"); }\n    view { Column { Button { label: "Go" on: Go } Text { "{out}" } } }\n}\n');
    const js = fs.readFileSync(path.join(dir, 'app.js'), 'utf8');
    check('known operands carry no helper', !/function __cui_(lt|le|gt|ge|eq|ord)\(/.test(js));
    const { page } = await open(dir);
    await page.click('text=Go');
    await sleep(30);
    check('and answer as before', (await page.evaluate(() => [...document.querySelectorAll('#root span')].pop().textContent)) === 'truetruetruetrue');
    await page.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
