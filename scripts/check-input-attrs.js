#!/usr/bin/env node
// `autocomplete:`, `inputmode:`, `maxlength:` and `name:` of a TextField (docs/controls.md section 6) in Chromium, from
// tests/input_attrs_corpus.json (`cargo test` compiles and replays the same file): every accepted binding is an attribute of the
// element with its value; the browser cuts what is typed, set as a whole or pasted at `maxlength` (counted in UTF-16 code units)
// so the handler receives `received`, and the page draws that back; a value set from the program is not cut. Also checked: the
// sign-in form of examples/tickets carries the attributes a password manager reads. Outside the gate: needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-input-attrs.js [path/to/cancho-ui]
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
const program = (bindings) => `component F {
    state v: String = "";
    state u: String = "";
    on V(x) { v = x; }
    on Fill { v = "set from the program, longer than any limit"; }
    view { Column { TextField { label: "Field" value: v on: V ${bindings} } Button { label: "Fill" on: Fill } } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-input-attrs-'));
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
  let n = 0;
  async function open(source, cwd) {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), source);
    const r = spawnSync(CANCHO_UI, ['build', cwd ? source : path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8', cwd: cwd || undefined });
    if (r.status !== 0) { return { error: r.stderr }; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root input');
    return { page };
  }
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'input_attrs_corpus.json'), 'utf8'));

  for (const c of corpus.accepted) {
    const { page, error } = await open(program(c.binding));
    if (error) { check(c.binding, false, `build failed: ${error}`); continue; }
    const got = await page.getAttribute('#root input', c.attr);
    check(`${c.binding} is the attribute ${c.attr}`, got === c.value, JSON.stringify(got));
    await page.close();
  }

  for (const c of corpus.typing) {
    const { page, error } = await open(program(`maxlength: ${c.maxlength}`));
    if (error) { check(c.why, false, `build failed: ${error}`); continue; }
    await page.focus('#root input');
    if (c.how === 'type') { await page.keyboard.type(c.typed); } else if (c.how === 'paste') { await page.keyboard.insertText(c.typed); } else { await page.fill('#root input', c.typed); }
    await page.waitForTimeout(30);
    const value = await page.inputValue('#root input');
    check(`${c.why} [${c.how} ${JSON.stringify(c.typed)} into maxlength ${c.maxlength}]`, value === c.received, `${JSON.stringify(value)} want ${JSON.stringify(c.received)}`);
    await page.close();
  }

  // A value the program sets is drawn whole: maxlength limits a person's typing, not the state.
  {
    const { page } = await open(program('maxlength: 5'));
    await page.click('button[aria-label="Fill"]');
    await page.waitForTimeout(30);
    const value = await page.inputValue('#root input');
    check('a value set by the program is not cut at maxlength', value === 'set from the program, longer than any limit', JSON.stringify(value));
    await page.close();
  }

  // The sign-in form of examples/tickets.
  {
    const { page, error } = await open('examples/tickets/tickets.cui', ROOT);
    if (error) {
      check('examples/tickets builds', false, error);
    } else {
      const attrs = await page.$$eval('#root input', (is) => is.map((i) => ({ label: i.getAttribute('aria-label'), type: i.type, autocomplete: i.getAttribute('autocomplete'), name: i.getAttribute('name') })));
      const user = attrs.find((a) => a.label === 'User name');
      const pass = attrs.find((a) => a.label === 'Password');
      check('the sign-in form names its fields for a password manager', !!user && !!pass && user.autocomplete === 'username' && user.name === 'username' && pass.autocomplete === 'current-password' && pass.name === 'password' && pass.type === 'password', JSON.stringify(attrs));
    }
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
