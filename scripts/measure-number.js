#!/usr/bin/env node
// What docs/number-input.md section 2 says, measured in Chromium (Pixel 7 profile: touch, 412 px): what a number field holds key by key, what its
// value is while the text is not a number, how the page's redraw (the whole view is rebuilt after every state change) treats it, what the
// browser's locale does, and what a text field with `inputmode` and `num(text, default)` gives. Not a check of the compiler. Outside the gate.
// The soft keyboard itself is not measured: headless Chromium has none (section 6 of the note says so).
//   node scripts/measure-number.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

setTimeout(() => { console.error('FAIL the measurement took more than 4 minutes'); process.exit(1); }, 240000).unref();
const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const say = (k, v) => console.log(`${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`);
const J = JSON.stringify;

const PROGRAM = (type, extra = '') => `component F {
    state v: String = "";
    on V(x) { v = x; }
    view { Column { TextField { label: "Field" value: v on: V type: "${type}" ${extra} } Text { "got=[{v}]" } } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'measure-number-'));
  let served = root;
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(served, path.normalize(rel));
    if (!file.startsWith(served) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': { '.html': 'text/html', '.js': 'text/javascript' }[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const url = `http://localhost:${server.address().port}/`;
  let n = 0;

  // the compiler's own text field, its type attribute changed after the build (the compiler refuses `number`): the redraw is the real one
  const build = (type, patch, extra) => {
    const dir = path.join(root, `case${n++}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), PROGRAM(type, extra));
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--page', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(r.stderr); }
    if (patch) {
      for (const f of fs.readdirSync(path.join(dir, 'out'))) {
        const file = path.join(dir, 'out', f);
        if (/\.(js|html)$/.test(f)) { fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace(`setAttribute('type', '${type}')`, `setAttribute('type', '${patch}')`)); }
      }
    }
    return path.join(dir, 'out');
  };
  const state = (page) => page.evaluate(() => { const i = document.querySelector('#root input'); return { value: i.value, bad: i.validity.badInput, n: i.valueAsNumber, focus: document.activeElement === i }; });
  const open = async (dir, options) => {
    served = dir;
    const ctx = await browser.newContext(options || { ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    await page.goto(url);
    await page.waitForSelector('#root input');
    return { ctx, page };
  };

  // ---- 1. the problem, reproduced: key by key through the page's redraw, and the same in a page that never redraws ---------------------------
  const numberDir = build('text', 'number');
  const plain = '<!doctype html><input id=i type=number>';
  for (const text of ['1e5', '-', '-5', '1.', '1.5', '.5', '-.5', '1e', '1e+', '12abc', '1,5', '+1', ' 1', '0.10', '007', '1e400']) {
    const { ctx, page } = await open(numberDir);
    await page.focus('#root input');
    const trail = [];
    for (const ch of text) { await page.keyboard.press(ch === ' ' ? 'Space' : ch); trail.push(await page.evaluate(() => { const i = document.querySelector('#root input'); return `${i.value}${i.validity.badInput ? '!' : ''}`; })); }
    const s = await state(page);
    const g = await page.locator('#root').innerText();
    await ctx.close();
    const c2 = await browser.newContext({ ...devices['Pixel 7'] });
    const p2 = await c2.newPage();
    await p2.setContent(plain);
    await p2.focus('#i');
    const t2 = [];
    for (const ch of text) { await p2.keyboard.press(ch === ' ' ? 'Space' : ch); t2.push(await p2.evaluate(() => `${i.value}${i.validity.badInput ? '!' : ''}`)); }
    const s2 = await p2.evaluate(() => ({ value: i.value, bad: i.validity.badInput, n: i.valueAsNumber }));
    await c2.close();
    say(`typed ${J(text)} | through the redraw: after each key (! = badInput) ${J(trail)}, ends holding ${J(s.value)} badInput ${s.bad}, the page says ${J(g.replace(/\n/g, ' '))} | a page that never redraws: ${J(t2)} ends ${J(s2.value)} badInput ${s2.bad} valueAsNumber ${s2.n}`, '');
  }

  // ---- 2. what the browser accepts, by locale (the browser's own, not the page's) -----------------------------------------------------------
  for (const locale of ['en-US', 'es-ES', 'de-DE', 'fr-FR', 'ar-EG', 'hi-IN']) {
    const c = await browser.newContext({ ...devices['Pixel 7'], locale });
    const p = await c.newPage();
    await p.setContent(plain);
    const row = { navigator: await p.evaluate(() => navigator.language), intl: await p.evaluate(() => (1234.5).toLocaleString()) };
    for (const text of ['1,5', '1.5', '1.234,5', '٣', '１２']) {
      await p.focus('#i');
      await p.fill('#i', '');
      await p.keyboard.type(text);
      row[text] = await p.evaluate(() => `${i.value}${i.validity.badInput ? '!' : ''}`);
    }
    say(`locale ${locale}: typing into a number field (value, ! = badInput)`, row);
    await c.close();
  }

  // ---- 3. the attributes that pick the keyboard, and the other ways a number field changes ------------------------------------------------------
  {
    const c = await browser.newContext({ ...devices['Pixel 7'] });
    const p = await c.newPage();
    await p.setContent('<input id=n type=number><input id=t type=text inputmode=decimal><input id=m type=text inputmode=numeric pattern="[0-9]*"><input id=f type=number step=any>');
    say('inputMode property: type=number / text+decimal / text+numeric', await p.evaluate(() => [n.inputMode, t.inputMode, m.inputMode]));
    await p.focus('#n');
    await p.keyboard.type('5');
    await p.keyboard.press('ArrowUp');
    const up = await p.evaluate(() => n.value);
    await p.mouse.move(100, 20);
    await p.mouse.wheel(0, -100);
    const wheel = await p.evaluate(() => n.value);
    say('a number field holding 5: ArrowUp gives, then the mouse wheel over it (focused) gives', [up, wheel]);
    await p.fill('#n', '1.5');
    say('1.5 in a field with the default step (1): stepMismatch / valid', await p.evaluate(() => [n.validity.stepMismatch, n.checkValidity()]));
    await p.fill('#f', '1.5');
    say('1.5 with step=any: stepMismatch', await p.evaluate(() => f.validity.stepMismatch));
    await p.focus('#t');
    await p.keyboard.type('1,5 abc');
    say('a text field with inputmode=decimal holds what was typed', await p.evaluate(() => t.value));
    await c.close();
  }

  // ---- 4. the text field today: inputmode and num(text, default) ----------------------------------------------------------------------------
  {
    const dir = build('text', null, 'inputmode: "decimal"');
    const { ctx, page } = await open(dir);
    await page.focus('#root input');
    await page.keyboard.type('1,5');
    const s = await state(page);
    say('the compiler\'s text field with inputmode: "decimal", typed 1,5 key by key through the redraw', { value: s.value, inputmode: await page.getAttribute('#root input', 'inputmode'), focus: s.focus });
    await ctx.close();
  }

  await browser.close();
  server.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
