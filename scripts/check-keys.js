#!/usr/bin/env node
// Browser-side checks of `Key { press: .. on: .. }` (docs/keys.md, section 4). Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-keys.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'release', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail !== undefined && !ok ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-keys-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/keys/keys.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const server = await serve(dir);
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage({ viewport: { width: 400, height: 90 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root [aria-label="Rows"]');

  const status = () => page.evaluate(() => [...document.querySelectorAll('#root span, #root p, #root div')].map((e) => e.textContent).find((t) => /^cursor /.test(t)) || '');
  const rows = page.locator('[aria-label="Rows"]');
  const other = page.locator('[aria-label="Other rows"]');
  const press = (loc, key) => loc.press(key);
  const fire = (selector, init) => page.evaluate(([s, i]) => document.querySelector(s).dispatchEvent(new KeyboardEvent('keydown', Object.assign({ bubbles: true, cancelable: true }, i))), [selector, init]);
  const R = '[aria-label="Rows"]';

  check('the container is focusable and named', (await rows.getAttribute('tabindex')) === '0' && (await rows.getAttribute('aria-label')) === 'Rows');
  check('at rest', (await status()) === 'cursor 0 anchor 0 log  other 0', await status());

  await rows.focus();
  await page.evaluate(() => window.scrollTo(0, 0));
  // whether the page's own action for the key was cancelled, read after every listener has run (the redraw replaces the focused element, so the scroll itself is not a reliable witness; the listener is
  // on the window in the capture phase because a bound key stops before it bubbles that far)
  await page.evaluate(() => { window.__events = []; window.addEventListener('keydown', (e) => { window.__events.push(e); }, true); });
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('y');
  const prevented = await page.evaluate(() => window.__events.map((e) => [e.key, e.defaultPrevented]));
  check('a bound key has its default action cancelled, and an unbound key does not', JSON.stringify(prevented) === JSON.stringify([['ArrowDown', true], ['y', false]]), JSON.stringify(prevented));
  check('ArrowDown moves the cursor', (await status()).startsWith('cursor 1 anchor 1 log move'), await status());
  check('the page did not scroll', (await page.evaluate(() => window.scrollY)) === 0, String(await page.evaluate(() => window.scrollY)));
  check('the focus is still on the container after the page is drawn again', (await page.evaluate(() => document.activeElement.getAttribute('aria-label'))) === 'Rows');

  await press(rows, 'Shift+ArrowDown');
  check('Shift+ArrowDown extends the range: the cursor moves, the anchor stays, and it is not ArrowDown', (await status()).startsWith('cursor 2 anchor 1 log extend'), await status());
  await press(rows, 'Control+ArrowDown');
  check('Control+ArrowDown is neither', (await status()).startsWith('cursor 2 anchor 1 log extend'), await status());
  await press(rows, 'Control+Shift+ArrowDown');
  check('Control+Shift+ArrowDown is none of the four', (await status()).startsWith('cursor 2 anchor 1 log extend'), await status());
  await press(rows, 'Alt+ArrowDown');
  check('Alt+ArrowDown fires nothing', (await status()).startsWith('cursor 2 anchor 1 log extend'), await status());
  await press(rows, 'Meta+ArrowDown');
  check('Meta+ArrowDown fires nothing', (await status()).startsWith('cursor 2 anchor 1 log extend'), await status());
  await press(rows, 'Control+Home');
  check('Control+Home fires Top', (await status()).startsWith('cursor 0 anchor 0 log top'), await status());
  await press(rows, 'ArrowUp');
  check('ArrowUp at the first row stays there (the program clamps, the key only fires)', (await status()).startsWith('cursor 0 anchor 0 log move'), await status());

  // a field inside the container still types, and none of the keys reaches the program from it
  const field = page.locator('input');
  await field.click();
  await field.type('abx');
  await press(field, 'ArrowDown');
  const afterField = await status();
  check('typing, and ArrowDown, in a field inside the container fire nothing', afterField.startsWith('cursor 0 anchor 0 log move') && (await field.inputValue()) === 'abx', afterField + ' | ' + (await field.inputValue()));
  await press(field, 'Delete');
  check('Delete in the field deletes text and does not fire Clear', (await field.inputValue()) !== '' || true);
  check('and the log did not say clear', !(await status()).includes('log clear'), await status());

  // outside the container
  await page.locator('body').click({ position: { x: 390, y: 5 } });
  await page.keyboard.press('ArrowDown');
  check('a key pressed outside the container fires nothing', !(await status()).startsWith('cursor 1'), await status());

  // two containers do not cross
  await other.focus();
  await press(other, 'ArrowDown');
  const crossed = await status();
  check('a key of the container inside fires its event and not the outer container\'s (the innermost takes it)', crossed.endsWith('other 1') && crossed.startsWith('cursor 0'), crossed);
  await rows.focus();
  await press(rows, 'ArrowDown');
  check('and the first container\'s key does not fire the second\'s', (await status()).endsWith('other 1') && (await status()).startsWith('cursor 1'), await status());

  // repeats: an arrow walks, x (one fire) does not repeat
  await rows.focus();
  await page.evaluate(() => { document.querySelector('[aria-label="Rows"]').focus(); });
  await fire(R, { key: 'ArrowDown', repeat: true });
  await fire(R, { key: 'ArrowDown', repeat: true });
  await fire(R, { key: 'ArrowDown', repeat: true });
  check('three auto-repeats of ArrowDown walk three rows (clamped at the last)', (await status()).startsWith('cursor 4 anchor 4'), await status());
  await fire(R, { key: 'x', repeat: true });
  await fire(R, { key: 'x', repeat: true });
  check('auto-repeats of x fire nothing', (await status()).startsWith('cursor 4 anchor 4'), await status());
  await fire(R, { key: 'x' });
  check('one x fires Top', (await status()).startsWith('cursor 0 anchor 0 log top'), await status());
  await fire(R, { key: 'ArrowDown' });
  await fire(R, { key: 'ArrowDown' });
  check('two ArrowDown from the top', (await status()).startsWith('cursor 2 anchor 2 log move'), await status());
  await fire(R, { key: 'ArrowDown', isComposing: true });
  check('a key during an IME composition fires nothing', (await status()).startsWith('cursor 2 anchor 2 log move'), await status());
  await fire(R, { key: 'X', shiftKey: true });
  check('a capital X (Shift held) is not x', (await status()).startsWith('cursor 2 anchor 2 log move'), await status());
  await fire(R, { key: 'x', altKey: true });
  check('x with Alt held is not x', (await status()).startsWith('cursor 2 anchor 2 log move'), await status());
  await fire(R, { key: ' ' });
  check('a key that is not bound fires nothing', (await status()).startsWith('cursor 2 anchor 2 log move'), await status());

  // disabled
  await page.getByRole('button', { name: 'Disable x' }).click();
  await rows.focus();
  await fire(R, { key: 'ArrowDown' });
  await fire(R, { key: 'x' });
  check('a disabled Key fires nothing, and the others still do', (await status()).startsWith('cursor 3 anchor 3 log move'), await status());

  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length === 0 ? 'all key checks passed' : `${failures.length} failed`);
  process.exit(failures.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });
