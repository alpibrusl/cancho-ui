#!/usr/bin/env node
// Browser-side checks of the emitted module's embedding contract (docs/mobile-backend.md section 9), in Chromium:
// the default build mounts into #root on import; a `--no-auto-mount` build mounts nothing on import and can be mounted
// where and as often as the page wants (two independent instances); a non-ASCII string written as \uXXXX escapes is the
// same string on the page; and no built module has a byte above 126. Outside the gate: needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-embed.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const SRC = 'component Cafe {\n    state n: Int = 0;\n    state s: String = "café \u{1F600} —";\n    on Inc { n = n + 1; }\n    view { Column { Text { "{s}" } Text { "count {n}" } Button { label: "Über" on: Inc } } }\n}\n'.replace('\\u{1F600}', '\u{1F600}');

function build(dir, name, extra) {
  const src = path.join(dir, name + '.cui');
  fs.writeFileSync(src, SRC);
  const out = path.join(dir, name + '.js');
  const r = spawnSync(CANCHO_UI, ['build', src, '-o', out, ...extra], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  return out;
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-embed-'));
  const auto = build(dir, 'auto', []);
  const embed = build(dir, 'embed', ['--no-auto-mount']);
  for (const f of [auto, embed]) {
    const bytes = fs.readFileSync(f);
    const bad = [...bytes].findIndex((b) => !(b === 10 || b === 9 || (b >= 32 && b <= 126)));
    check(`${path.basename(f)} is ASCII`, bad === -1, bad === -1 ? '' : `byte ${bytes[bad]} at ${bad}`);
  }
  const html = (mod, body, script) => `<!doctype html><meta charset="utf-8"><body>${body}<script type="module">${script}</script>`;
  const pages = {
    '/auto.html': html('auto', '<div id="root"></div>', 'import "./auto.js";'),
    '/embed.html': html('embed', '<div id="root"></div><div id="left"></div><div id="right"></div>',
      'import { mount_Cafe } from "./embed.js"; window.imported = true; window.mount = (id) => mount_Cafe(document.getElementById(id));'),
  };
  const server = http.createServer((req, res) => {
    const url = req.url.split('?')[0];
    if (pages[url]) { res.writeHead(200, { 'content-type': 'text/html' }); res.end(pages[url]); return; }
    const f = path.join(dir, path.basename(url));
    if (fs.existsSync(f) && f.endsWith('.js')) { res.writeHead(200, { 'content-type': 'text/javascript' }); fs.createReadStream(f).pipe(res); return; }
    res.writeHead(404); res.end();
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const base = `http://localhost:${server.address().port}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const errors = [];

  const page = await browser.newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`${base}/auto.html`);
  await page.waitForSelector('#root *');
  const text = await page.locator('#root').innerText();
  check('the default build mounts into #root on import', text.includes('count 0'));
  check('a non-ASCII string written as escapes is the same string on the page', text.includes('café \u{1F600} —'), JSON.stringify(text));
  check('and so is a button label', (await page.locator('button').innerText()) === 'Über');

  const p2 = await browser.newPage();
  p2.on('pageerror', (e) => errors.push(String(e)));
  await p2.goto(`${base}/embed.html`);
  await p2.waitForFunction(() => window.imported === true);
  check('--no-auto-mount: importing mounts nothing', (await p2.locator('body').innerText()).trim() === '');
  await p2.evaluate(() => { window.mount('left'); window.mount('right'); });
  check('and it can be mounted twice, where the page wants', (await p2.locator('#left').innerText()).includes('count 0') && (await p2.locator('#right').innerText()).includes('count 0'));
  check('#root, which nobody asked for, stays empty', (await p2.locator('#root').innerHTML()) === '');
  await p2.locator('#left button').click();
  check('the two instances are independent', (await p2.locator('#left').innerText()).includes('count 1') && (await p2.locator('#right').innerText()).includes('count 0'));
  check('no script errors', errors.length === 0, errors.join(' | '));

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
