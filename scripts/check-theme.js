#!/usr/bin/env node
// `theme { .. }` in Chromium (docs/theme.md section 8): DESKTOP CHROMIUM with the Pixel 7 profile, not a device. The example
// (examples/theme) is built with `build --page` and loaded in light and in dark; then every colour token of both schemes is set to four
// extreme values in turn, and each program that still compiles is built and loaded. Checked in the browser:
//   - the page makes no request to another host (every request is counted; the server answers only the page's own files), and the font is
//     requested from this origin and is the file in the program's directory, byte for byte;
//   - the font is applied (`document.fonts.check`, the computed `font-family` of the body);
//   - the colours drawn are the colours of the scheme the browser is in (body, text, a button, the three tones), and the contrast of what is
//     drawn is at least 4.5:1 in both schemes, computed by the oracle's formula (scripts/theme-oracle.js), not the compiler's;
//   - whatever a token is set to, a button and a field are at least 44 x 44 CSS px and a focused button has a 3 px solid outline in the accent.
// Watchdog: 5 minutes. Outside the gate.
//   node scripts/check-theme.js
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');
const { ratio } = require('./theme-oracle.js');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'theme_corpus.json'), 'utf8'));
const FONT = 'fonts/jetbrains-mono-latin-wght-normal.woff2';
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const rgb = (hex) => { const n = parseInt(hex.slice(1), 16); return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`; };
const hexOf = (css) => { const m = /rgba?\((\d+), (\d+), (\d+)/.exec(css); return m ? '#' + [1, 2, 3].map((i) => Number(m[i]).toString(16).padStart(2, '0')).join('') : css; };

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'theme-'));
fs.mkdirSync(path.join(work, 'src', 'fonts'), { recursive: true });
fs.copyFileSync(path.join(ROOT, 'examples/theme', FONT), path.join(work, 'src', FONT));
const fontBytes = fs.readFileSync(path.join(work, 'src', FONT));
const sha = require('crypto').createHash('sha256').update(fontBytes).digest('hex');
const APP = fs.readFileSync(path.join(ROOT, 'examples/theme/theme.cui'), 'utf8').split('component Palette')[1];

function source(palette) {
  const scheme = (n) => `    ${n} {\n${corpus.tokens.map((t) => `        ${t}: "${palette[n][t]}";`).join('\n')}\n    }\n`;
  return `theme {\n${scheme('light')}${scheme('dark')}    font body {\n        family: "JetBrains Mono";\n        file: "${FONT}";\n        sha256: "${sha}";\n        licence: "OFL-1.1";\n        weight: "100 800";\n    }\n}\n\ncomponent Palette${APP}`;
}
let nextBuild = 0;
function build(palette) {
  const id = `b${nextBuild++}`;
  const file = path.join(work, 'src', `${id}.cui`);
  fs.writeFileSync(file, source(palette));
  const r = spawnSync(BIN, ['build', file, '--page', '-o', path.join(work, 'site', id)], { encoding: 'utf8', cwd: work });
  return { id, code: r.status, err: r.stderr + r.stdout };
}

(async () => {
  const requests = [];
  const server = http.createServer((q, r) => {
    requests.push(q.url);
    let f = path.join(work, 'site', decodeURIComponent(q.url.split('?')[0]));
    if (f.endsWith(path.sep) || (fs.existsSync(f) && fs.statSync(f).isDirectory())) { f = path.join(f, 'index.html'); }
    fs.readFile(f, (e, d) => {
      if (e) { r.statusCode = 404; r.end(); return; }
      const types = { '.html': 'text/html', '.js': 'text/javascript', '.woff2': 'font/woff2' };
      r.setHeader('content-type', types[path.extname(f)] || 'application/octet-stream');
      r.end(d);
    });
  });
  await new Promise((ok) => server.listen(0, ok));
  const base = `http://localhost:${server.address().port}`;
  const browser = await chromium.launch();

  // 1. the example, in both schemes
  const main = build(corpus.palette);
  check('the example builds with the site palette', main.code === 0, main.err.slice(0, 200));
  for (const scheme of ['light', 'dark']) {
    requests.length = 0;
    const foreign = [];
    const ctx = await browser.newContext({ ...devices['Pixel 7'], colorScheme: scheme, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    page.on('request', (r) => { if (!r.url().startsWith(base)) { foreign.push(r.url()); } });
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    const fontRes = page.waitForResponse((r) => r.url().endsWith('.woff2'), { timeout: 10000 }).catch(() => null);
    await page.goto(`${base}/${main.id}/`);
    await page.getByRole('button', { name: 'Click' }).waitFor();
    const res = await fontRes;
    await sleep(500);
    const p = corpus.palette[scheme];
    check(`${scheme}: no request to another host`, foreign.length === 0, foreign.join(' '));
    check(`${scheme}: the server saw only the page, its script and the font`, requests.every((u) => /^\/b\d+\/(index\.html|app\.js|fonts\/[\w.-]+\.woff2)?$/.test(u) || u === '/favicon.ico'), JSON.stringify(requests));
    check(`${scheme}: the font was requested from this origin and is the program's file`, !!res && res.status() === 200 && Buffer.compare(await res.body(), fontBytes) === 0);
    check(`${scheme}: the font is applied`, await page.evaluate(() => document.fonts.check('16px "JetBrains Mono"') && getComputedStyle(document.body).fontFamily.includes('JetBrains Mono')));
    const drawn = await page.evaluate(() => {
      const css = (el, prop) => getComputedStyle(el)[prop];
      const q = (sel) => document.querySelector(sel);
      return {
        body: [css(document.body, 'backgroundColor'), css(document.body, 'color')],
        button: [css(q('button'), 'backgroundColor'), css(q('button'), 'color'), css(q('button'), 'borderTopColor')],
        field: [css(q('input'), 'backgroundColor'), css(q('input'), 'color')],
        ok: [css(q('[data-tone="ok"]'), 'color'), css(q('[data-tone="ok"]'), 'backgroundColor')],
        warn: [css(q('[data-tone="warn"]'), 'color'), css(q('[data-tone="warn"]'), 'backgroundColor')],
        error: [css(q('[data-tone="error"]'), 'color'), css(q('[data-tone="error"]'), 'backgroundColor')],
        scheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
      };
    });
    check(`${scheme}: the browser is in ${scheme}`, drawn.scheme === scheme);
    check(`${scheme}: the page is drawn in the colours of the ${scheme} block`,
      drawn.body[0] === rgb(p.bg) && drawn.body[1] === rgb(p.fg) && drawn.button[0] === rgb(p.surface) && drawn.button[1] === rgb(p.fg) && drawn.button[2] === rgb(p.muted)
      && drawn.field[0] === rgb(p.surface) && drawn.ok[0] === rgb(p.good) && drawn.warn[0] === rgb(p.warn) && drawn.warn[1] === rgb(p.warn_bg) && drawn.error[0] === rgb(p.bad) && drawn.error[1] === rgb(p.bad_bg),
      JSON.stringify(drawn));
    const pairs = [['body text', drawn.body[1], drawn.body[0]], ['button text', drawn.button[1], drawn.button[0]], ['field text', drawn.field[1], drawn.field[0]],
      ['ok tone on the page', drawn.ok[0], drawn.body[0]], ['warn tone', drawn.warn[0], drawn.warn[1]], ['error tone', drawn.error[0], drawn.error[1]]];
    for (const [name, fg, bg] of pairs) {
      const r = ratio(hexOf(fg), hexOf(bg));
      check(`${scheme}: ${name} is drawn at ${r.toFixed(2)}:1, at least 4.5`, r >= 4.5);
    }
    check(`${scheme}: no script error`, errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // 2. every token of both schemes at four extreme values: whatever compiles keeps its targets and its ring
  let built = 0, refused = 0, wrong = [];
  for (const scheme of ['light', 'dark']) {
    const c2 = await browser.newContext({ ...devices['Pixel 7'], colorScheme: scheme, serviceWorkers: 'block' });
    const pg = await c2.newPage();
    for (const token of corpus.tokens) {
      for (const value of ['#000000', '#ffffff', '#ff00ff', '#808080']) {
        const palette = JSON.parse(JSON.stringify(corpus.palette));
        palette[scheme][token] = value;
        const b = build(palette);
        if (b.code !== 0) { refused += 1; if (!b.err.includes('theme-contrast')) { wrong.push(`${scheme}.${token}=${value}: ${b.err.slice(0, 120)}`); } continue; }
        built += 1;
        await pg.goto(`${base}/${b.id}/`);
        await pg.getByRole('button', { name: 'Click' }).waitFor();
        const m = await pg.evaluate(() => {
          const out = { small: [] };
          for (const el of document.querySelectorAll('button, input')) { const r = el.getBoundingClientRect(); if (r.height < 44 || r.width < 44) { out.small.push(el.tagName + ' ' + r.width + 'x' + r.height); } }
          document.querySelector('button').focus({ focusVisible: true });
          const s = getComputedStyle(document.querySelector('button'));
          out.ring = [s.outlineStyle, s.outlineWidth, s.outlineColor];
          return out;
        });
        await pg.keyboard.press('Tab');
        const ring = await pg.evaluate(() => { const s = getComputedStyle(document.activeElement); return [s.outlineStyle, s.outlineWidth, s.outlineColor, document.activeElement.tagName]; });
        const accent = rgb(palette[scheme].accent);
        const ok = m.small.length === 0 && ring[0] === 'solid' && ring[1] === '3px' && ring[2] === accent;
        if (!ok) { wrong.push(`${scheme}.${token}=${value}: small ${m.small.join(',')} ring ${ring.join(' ')}`); }
      }
    }
    await c2.close();
  }
  check(`${built} themes that compile, of ${built + refused} (every token of both schemes at 4 values), keep 44 px targets and a 3 px solid accent focus ring`, wrong.length === 0 && built > 40, wrong.slice(0, 3).join(' | '));
  check('the others were refused by `theme-contrast` and nothing else', wrong.every((w) => !w.includes('refused')) && refused > 20, `${refused} refused`);
  await browser.close();
  server.close();
  fs.rmSync(work, { recursive: true, force: true });
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
