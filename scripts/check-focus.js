#!/usr/bin/env node
// Browser-side check that focus and the text selection survive a re-render (docs/semantics.md, Question 9). Outside the
// gate: it needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. Exits 1 on a failed check.
//   node scripts/check-focus.js [path/to/cancho-ui]
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

function build(example) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-focus-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, example), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${example}: ${r.stderr}`); }
  return dir;
}

function serve(dir) {
  const server = http.createServer((req, res) => {
    const file = path.join(dir, req.url === '/' ? 'index.html' : path.normalize(req.url.split('?')[0]));
    if (!file.startsWith(dir) || !fs.existsSync(file)) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

async function open(browser, example) {
  const server = await serve(build(example));
  const page = await browser.newPage();
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  return { page, server };
}

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // A text field keeps focus while the user types: every keystroke is a state change and a rebuild.
  let { page, server } = await open(browser, 'examples/search/search.cui');
  await page.click('input');
  await page.keyboard.type('ada', { delay: 100 });
  let got = await page.evaluate(() => ({ value: document.querySelector('input').value, focused: document.activeElement.tagName }));
  check('typing "ada" into a TextField gives "ada" and focus stays in it', got.value === 'ada' && got.focused === 'INPUT', JSON.stringify(got));

  // The caret stays where it was: typed in the middle, not thrown to the end.
  await page.keyboard.press('Home');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.type('x', { delay: 100 });
  got = await page.evaluate(() => { const i = document.querySelector('input'); return { value: i.value, caret: i.selectionStart, end: i.selectionEnd }; });
  check('a character typed mid-text lands there and the caret follows it', got.value === 'axda' && got.caret === 2 && got.end === 2, JSON.stringify(got));

  // A selection survives too.
  await page.keyboard.press('Home');
  await page.keyboard.down('Shift');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.up('Shift');
  await page.keyboard.type('Q', { delay: 50 });
  got = await page.evaluate(() => document.querySelector('input').value);
  check('typing over a selection replaces it', got === 'Qda', JSON.stringify(got));
  await page.close(); server.close();

  // A button keeps focus when it is activated from the keyboard.
  ({ page, server } = await open(browser, 'examples/counter/counter.cui'));
  await page.keyboard.press('Tab');
  for (let i = 0; i < 3; i++) { await page.keyboard.press('Enter'); }
  got = await page.evaluate(() => ({ text: document.getElementById('root').innerText.split('\n')[0], focused: document.activeElement.getAttribute('aria-label') }));
  check('Enter three times on a focused Button counts three and focus stays on it', got.text === 'Count: 3' && got.focused === 'Increment', JSON.stringify(got));
  await page.close(); server.close();

  // Inside a nested component.
  ({ page, server } = await open(browser, 'examples/dashboard/dashboard.cui'));
  await page.keyboard.press('Tab');
  for (let i = 0; i < 3; i++) { await page.keyboard.press('Enter'); }
  got = await page.evaluate(() => ({ text: document.getElementById('root').innerText.split('\n')[0], focused: document.activeElement.getAttribute('aria-label') }));
  check('the same on a Button of the page that hosts a child component', got.text === 'dashboard (3)' && got.focused === 'bump page', JSON.stringify(got));
  await page.close(); server.close();

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
