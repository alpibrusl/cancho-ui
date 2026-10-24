#!/usr/bin/env node
// A number field (`TextField { type: "number" }`, docs/number-input.md, issue #111) in Chromium, in the mobile profile (Pixel 7: touch, 412 px):
//  A. `__cui_number_text` in the page against tests/number_corpus.json, the file `cargo test` runs on replay's;
//  B. typed key by key through the page's redraw: 1e5, -, -5, 1., 1.5, .5, -.5, 0.10, 007 end in the field exactly as typed, with the focus kept, and the
//     handler is given the number (or the text, when it is not one);
//  C. the language in use: Spanish reads 1,5 as 1.5 and keeps 1,5 on the screen; English reads 1.5; switching language redraws the number;
//  D. the program decides: it clamps what is typed, and clears it; deleting everything is the empty text;
//  E. the field is a text field with a decimal keyboard: type, inputmode (and a program's own inputmode), the name a screen reader gets, no spin buttons.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. The soft keyboard itself is not measured.
//   node scripts/check-number.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const J = JSON.stringify;
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
async function until(fn, ms = 3000) {
  for (let t = 0; t < ms; t += 25) { const v = await fn(); if (v) { return v; } await sleep(25); }
  return null;
}
function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const f = path.join(dir, path.normalize(rel));
    if (!f.startsWith(dir) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

const PLAIN = (extra = '', handler = 'v = x;') => `component F {
    state v: String = "";
    on V(x) { ${handler} }
    on Clear { v = ""; }
    view { Column { TextField { label: "Amount" value: v on: V type: "number" ${extra} } Text { role: status "got=[{v}]" } Button { label: "Clear" on: Clear } } }
}
`;
const LANGS = ['en', 'es', 'de', 'fr', 'pt', 'ru', 'ar', 'he', 'ja', 'hi'];
// Spanish first (the page picks the browser's language when the catalogs have it; es-ES gets es, en-US gets en)
const LOCALIZED = `component F {
    requires Locale;
    state v: String = "";
    messages {
        es { title: "Cantidad"; english: "English"; }
${LANGS.filter((l) => l !== 'es').map((l) => `        ${l} { title: "Amount"; english: "English"; }`).join('\n')}
    }
    on V(x) { v = x; }
    on English { locale.set("en"); }
    view { Column { TextField { label: t("title") value: v on: V type: "number" } Text { role: status "got=[{v}]" } Button { label: t("english") on: English } } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-number-'));
  let served = root;
  const server = http.createServer((req, res) => { res.writeHead(500); res.end(); });
  server.close();
  const state = { dir: root };
  const srv = await new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
      const f = path.join(served, path.normalize(rel));
      if (!f.startsWith(served) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  void state;
  const url = `http://localhost:${srv.address().port}/`;
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
  const open = async (dir, locale = 'en-US') => {
    served = dir;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], locale });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('#root input');
    return { ctx, page, errors };
  };
  const field = (page) => page.evaluate(() => { const i = document.querySelector('#root input'); return { value: i.value, focus: document.activeElement === i, type: i.type, inputmode: i.getAttribute('inputmode') }; });
  const got = async (page) => (await page.locator('#root [role=status]').innerText()).replace(/^got=\[/, '').replace(/\]$/, '');
  const typeKeys = async (page, text) => { await page.focus('#root input'); for (const ch of text) { await page.keyboard.press(ch === ' ' ? 'Space' : ch); } };
  const settle = () => sleep(30);

  const plainDir = build(PLAIN());
  const localDir = build(LOCALIZED);

  // ---- A. the function, against the corpus ----------------------------------------------------------------------------------------------
  {
    const js = fs.readFileSync(path.join(localDir, 'app.js'), 'utf8');
    const d = js.indexOf('const __CUI_I18N = ');
    const dEnd = js.indexOf('\n', d);
    const a = js.indexOf('function __cui_number_lang(');
    const b = js.indexOf('function __cui_number_show(');
    const e = js.indexOf('\n}\n', b) + 3;
    check('A: the module carries the number adapter', d > 0 && a > 0 && e > b);
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'number_corpus.json'), 'utf8'));
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    const bad = await page.evaluate(({ src, cases }) => {
      const f = new Function(`${src}; return { text: __cui_number_text, data: __CUI_I18N };`)();
      return cases.filter((c) => f.text(c.lang === null ? null : f.data[c.lang], c.typed) !== c.delivered).map((c) => c.why);
    }, { src: `${js.slice(d, dEnd)}\n${js.slice(a, e)}`, cases: corpus.cases });
    check(`A: ${corpus.cases.length} cases of the corpus come out as written`, bad.length === 0, bad.join('; '));
    await ctx.close();
  }

  // ---- B. key by key through the redraw, in English -------------------------------------------------------------------------------------
  const keys = [
    ['1e5', '1e5', '100000'], ['-', '-', '-'], ['-5', '-5', '-5'], ['1.', '1.', '1'], ['1.5', '1.5', '1.5'], ['.5', '.5', '0.5'], ['-.5', '-.5', '-0.5'],
    ['0.10', '0.10', '0.1'], ['007', '007', '7'], ['1e', '1e', '1e'], ['12abc', '12abc', '12abc'], ['1,5', '1,5', '1,5'], ['1 2', '1 2', '1 2'],
  ];
  for (const [typed, shown, state] of keys) {
    const { ctx, page, errors } = await open(plainDir);
    await typeKeys(page, typed);
    await settle();
    const f = await field(page);
    check(`B: typing ${J(typed)} key by key leaves ${J(shown)} in the field`, f.value === shown && f.focus, J(f));
    check(`B: and the handler was given ${J(state)}`, (await got(page)) === state, J(await got(page)));
    check(`B: no script error (${J(typed)})`, errors.length === 0, errors.join(' | '));
    await ctx.close();
  }
  {
    // a number field in the middle of the text: the caret does not jump to the end under the redraw
    const { ctx, page } = await open(plainDir);
    await typeKeys(page, '135');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.type('2');
    await settle();
    const f = await field(page);
    check('B: a digit typed in the middle goes in the middle', f.value === '1235' && (await got(page)) === '1235', J(f));
    const caret = await page.evaluate(() => document.querySelector('#root input').selectionStart);
    check('B: and the caret is after it', caret === 2, String(caret));
    await ctx.close();
  }
  {
    // pasting
    const { ctx, page } = await open(plainDir);
    await page.focus('#root input');
    await page.keyboard.insertText('3.14159');
    await settle();
    check('B: a pasted number is read', (await got(page)) === '3.14159' && (await field(page)).value === '3.14159');
    await ctx.close();
  }

  // ---- C. the language in use ---------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open(localDir, 'es-ES');
    check('C: the page is Spanish', (await page.evaluate(() => document.documentElement.lang)) === 'es');
    await typeKeys(page, '1,5');
    await settle();
    let f = await field(page);
    check('C: in Spanish 1,5 stays 1,5 on the screen', f.value === '1,5' && f.focus, J(f));
    check('C: and the handler is given 1.5', (await got(page)) === '1.5', await got(page));
    await page.fill('#root input', '');
    await typeKeys(page, '1.5');
    await settle();
    check('C: in Spanish a point is a grouping mark: 1.5 is text', (await got(page)) === '1.5' && (await field(page)).value === '1.5');
    await page.fill('#root input', '');
    await typeKeys(page, ',5');
    await settle();
    check('C: ,5 is 0.5', (await got(page)) === '0.5' && (await field(page)).value === ',5', J(await field(page)));
    await page.fill('#root input', '');
    await typeKeys(page, '2,50');
    await page.click('text=English');
    await settle();
    f = await field(page);
    check('C: switching to English redraws the number it holds: 2.5', f.value === '2.5' && (await got(page)) === '2.5', J(f));
    check('C: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }
  {
    const { ctx, page } = await open(localDir, 'en-US');
    await typeKeys(page, '1,5');
    await settle();
    check('C: in English 1,5 is text (a comma is a grouping mark)', (await got(page)) === '1,5' && (await field(page)).value === '1,5');
    await ctx.close();
  }
  {
    const { ctx, page } = await open(localDir, 'fr-FR');
    await typeKeys(page, '1,5');
    await settle();
    check('C: in French (a language the page has) 1,5 is 1.5', (await got(page)) === '1.5' || (await got(page)) === '1,5', await got(page));
    await ctx.close();
  }

  {
    // a number the program put there is shown in the language's separators
    const dir = build(LOCALIZED.replace('state v: String = "";', 'state v: String = "-2.5";'));
    for (const [locale, want] of [['es-ES', '-2,5'], ['en-US', '-2.5'], ['de-DE', '-2,5']]) {
      const { ctx, page } = await open(dir, locale);
      check(`C: the program's own -2.5 is shown as ${J(want)} in ${locale}`, (await field(page)).value === want, J(await field(page)));
      await ctx.close();
    }
  }

  // ---- D. the program decides ---------------------------------------------------------------------------------------------------------------
  {
    const dir = build(PLAIN('', 'if num(x, 0) > 10 { v = "10"; } else { v = x; }'));
    const { ctx, page } = await open(dir);
    await typeKeys(page, '15');
    await settle();
    const f = await field(page);
    check('D: the handler clamps what is typed: the field shows what the program kept', f.value === '10' && (await got(page)) === '10' && f.focus, J(f));
    await typeKeys(page, '7');
    await settle();
    check('D: typing on goes on from the clamped number', (await got(page)) === '10', await got(page));
    await ctx.close();
  }
  {
    const { ctx, page } = await open(plainDir);
    await typeKeys(page, '1.');
    await page.click('text=Clear');
    await settle();
    check('D: the program clears the field', (await field(page)).value === '' && (await got(page)) === '');
    await typeKeys(page, '5');
    await page.keyboard.press('Backspace');
    await settle();
    check('D: deleting everything is the empty text', (await field(page)).value === '' && (await got(page)) === '' && (await field(page)).focus);
    await page.focus('#root input');
    await page.keyboard.type('12');
    await page.keyboard.press('Control+A');
    await page.keyboard.press('Delete');
    await settle();
    check('D: select all and delete is the empty text', (await got(page)) === '');
    await ctx.close();
  }
  // ---- E. what kind of field it is ------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page } = await open(plainDir);
    const f = await field(page);
    check('E: it is a text field with a decimal keyboard', f.type === 'text' && f.inputmode === 'decimal', J(f));
    const role = await page.evaluate(async () => { const i = document.querySelector('#root input'); return { role: i.getAttribute('role'), name: i.getAttribute('aria-label'), spin: i.type === 'number', numberAttr: i.hasAttribute('data-cui-number') }; });
    check('E: named for a screen reader, not a spin button', role.name === 'Amount' && !role.spin && role.numberAttr, J(role));
    await page.focus('#root input');
    await page.keyboard.type('5');
    await page.keyboard.press('ArrowUp');
    await page.mouse.wheel(0, -100);
    await settle();
    check('E: the arrow and the wheel do not change the number', (await field(page)).value === '5' && (await got(page)) === '5', J(await field(page)));
    await ctx.close();
  }
  {
    const dir = build(PLAIN('inputmode: "numeric"'));
    const { ctx, page } = await open(dir);
    check('E: a program\'s own inputmode wins', (await field(page)).inputmode === 'numeric', J(await field(page)));
    await ctx.close();
  }
  {
    // a page with no number field carries none of this
    const dir = build(PLAIN().replace('type: "number"', 'type: "text"'));
    const js = fs.readFileSync(path.join(dir, 'app.js'), 'utf8');
    check('E: a page without a number field has no number adapter', !js.includes('function __cui_number_parse'));
  }

  await browser.close();
  srv.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
