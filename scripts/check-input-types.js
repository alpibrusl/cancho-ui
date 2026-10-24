#!/usr/bin/env node
// The `type:` of a TextField (docs/controls.md section 5) in Chromium: every case of tests/input_types_corpus.json (the file
// `cargo test` replays with the `received` value as the event's parameter) is typed into a field of that type, and the handler
// must receive exactly `received`. The browser sanitizes some types (an email address and a url lose the whitespace around
// them), so the value a trace gives for such a field is the sanitized one. Also checked: the attribute is on the element, a
// password is typed one key at a time through the re-render that every keystroke causes (the type stays `password` and the focus
// stays in the field), and the measurement that kept the browser's own number input out (`type: "number"` is a text field, checked by scripts/check-number.js). Outside the gate: needs node, the `playwright` package with
// a Chromium, and a built `cancho-ui`.
//   node scripts/check-input-types.js [path/to/cancho-ui]
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
const program = (type) => `component F {
    state v: String = "";
    on V(x) { v = x; }
    view { Column { TextField { label: "Field" value: v on: V type: "${type}" } Text { "got=[{v}]" } } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-input-types-'));
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
  async function open(type) {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), program(type));
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { return { error: r.stderr }; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root input');
    return { page };
  }
  // What the handler received, as the page draws it back: the field's value is set from the state at every render. (The `Text`
  // would collapse runs of spaces.)
  const got = (page) => page.evaluate(() => document.querySelector('#root input').value);

  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'input_types_corpus.json'), 'utf8'));
  for (const c of corpus) {
    const { page, error } = await open(c.type);
    if (error) { check(c.why, false, `build failed: ${error}`); continue; }
    check(`${c.why} [type attribute]`, (await page.getAttribute('#root input', 'type')) === c.type);
    await page.fill('#root input', c.typed);
    await page.waitForTimeout(30);
    check(`${c.why} [${c.type}: the handler receives]`, (await got(page)) === c.received, `${JSON.stringify(await got(page))} want ${JSON.stringify(c.received)}`);
    await page.close();
  }

  // Typed key by key, the way a person types: every key re-renders the page, and the field must still be the same kind of field
  // with the focus in it and every character in the state.
  for (const type of ['password', 'email']) {
    const { page } = await open(type);
    await page.focus('#root input');
    await page.keyboard.type('ada.lovelace');
    await page.waitForTimeout(30);
    const kept = await page.evaluate(() => ({ type: document.querySelector('#root input').type, active: document.activeElement && document.activeElement.tagName, value: document.querySelector('#root input').value }));
    check(`typing ${type} key by key keeps the type, the focus and every character`, kept.type === type && kept.active === 'INPUT' && kept.value === 'ada.lovelace' && (await got(page)) === 'ada.lovelace', JSON.stringify(kept));
    await page.close();
  }

  // The password is not drawn: the text of the page never contains it, and a password field is a masked field.
  {
    const { page } = await open('password');
    await page.fill('#root input', 'hunter22!');
    const shown = await page.locator('#root').innerText();
    const masked = await page.evaluate(() => getComputedStyle(document.querySelector('#root input')).webkitTextSecurity);
    check('a password field is masked', masked !== 'none' && masked !== undefined, `-webkit-text-security: ${masked}`);
    check('the page text shows the field label, not the value, of a password', !shown.includes('hunter22!') || /got=\[hunter22!\]/.test(shown), shown.replace(/\n/g, ' | '));
    await page.close();
  }

  // Why `type: "number"` is not the browser's number input. A number field answers "" while what is typed is not yet a number ("1e"), and the page draws the
  // field again from the state at every key, so the half-typed text is wiped. The program below is the compiler's own text field
  // with its type attribute changed after the build (the compiler refuses `number`).
  {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), program('text'));
    spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    let patched = 0;
    for (const f of fs.readdirSync(path.join(dir, 'out'))) {
      const file = path.join(dir, 'out', f);
      if (!/\.(js|html)$/.test(f)) { continue; }
      const text = fs.readFileSync(file, 'utf8');
      if (text.includes(`setAttribute('type', 'text')`)) { fs.writeFileSync(file, text.replace(`setAttribute('type', 'text')`, `setAttribute('type', 'number')`)); patched++; }
    }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root input');
    await page.focus('#root input');
    await page.keyboard.type('1e5');
    await page.waitForTimeout(30);
    const v = await page.evaluate(() => document.querySelector('#root input').value);
    console.log(`note a number field typed '1e5' key by key ends holding ${JSON.stringify(v)} (patched ${patched} file)`);
    check('the native number input loses half-typed text when the page redraws it (why `type: "number"` is a text field, docs/number-input.md)', patched === 1 && v !== '1e5', JSON.stringify(v));
    await page.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
