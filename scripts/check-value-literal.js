#!/usr/bin/env node
// A string literal `value:` on a TextField or TextArea (issue #140) in Chromium: every case of tests/value_literal_corpus.json (the file `cargo test`
// reads back from the IR, which is what replay reads) is built into a page, the page must load (`value: ""` used to be `.value = ;`, a syntax error), and
// both fields must hold exactly the text the corpus says. Also: the view is a function of the state, so a field with a literal value goes back to it at
// the next render, whatever was typed. A single-line <input> drops line feeds from the value it is given (the corpus says so with `input`); a <textarea> keeps them. Given an older compiler and `--report` it says how many cases that one fails to load.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-value-literal.js [path/to/cancho-ui] [--report]
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
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const show = (s) => JSON.stringify(s.length > 40 ? `${s.slice(0, 40)}... (${s.length} characters)` : s);

const program = (source) => `component F {
    state v: String = "";
    on V(x) { v = x; }
    view { Column { TextField { label: "F" value: "${source}" on: V } TextArea { label: "T" value: "${source}" on: V } Text { "{v}" } } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-vlit-'));
  let served = root;
  const server = await new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
      const f = path.join(served, path.normalize(rel));
      if (!f.startsWith(served) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : 'text/html', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'value_literal_corpus.json'), 'utf8'));
  let n = 0;
  const open = async (src) => {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), src);
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--page', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { return { error: `${r.stdout}${r.stderr}` }; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(`http://localhost:${server.address().port}/`);
    await sleep(150);
    return { page, errors };
  };

  const broken = [];
  for (const c of corpus.cases) {
    const { page, errors, error } = await open(program(c.source));
    if (error) { broken.push(`${c.why}: does not build`); continue; }
    const loaded = (await page.locator('#root input, #root textarea').count()) === 2 && errors.length === 0;
    const got = loaded ? await page.evaluate(() => [document.querySelector('#root input').value, document.querySelector('#root textarea').value]) : null;
    if (!loaded || got[0] !== (c.input !== undefined ? c.input : c.value) || got[1] !== c.value) { broken.push(`${c.why}: ${loaded ? show(got[0]) : `the page did not load (${errors[0] || 'no fields'})`}`); }
    await page.close();
  }
  if (REPORT) {
    console.log(`${broken.length} of ${corpus.cases.length} cases fail with this compiler`);
    broken.slice(0, 6).forEach((b) => console.log(`  ${b}`));
    await browser.close(); server.close(); process.exit(0);
  }
  check(`both fields hold exactly the text of the source in all ${corpus.cases.length} cases (an empty text, quotes, a backslash, line breaks, braces, a template-literal opening, markup, non-ASCII, U+2028, a NUL, 300 characters)`, broken.length === 0, broken.slice(0, 4).join(' | '));

  // the view is a function of the state: typed text does not outlive the next render
  {
    const { page, errors } = await open(program('fixed'));
    await page.focus('#root input');
    await page.keyboard.type('zz');
    await sleep(40);
    const after = await page.evaluate(() => ({ field: document.querySelector('#root input').value, area: document.querySelector('#root textarea').value, state: document.querySelector('#root span').textContent }));
    check('a field with a literal value goes back to it at the next render, the state keeps what was typed after it (the second key is typed after the restored text)', after.field === 'fixed' && after.area === 'fixed' && after.state === 'fixedz', JSON.stringify(after));
    check('no script error', errors.length === 0, errors.join(' | '));
    await page.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
