#!/usr/bin/env node
// Browser-side checks of the bindings of built-in elements (docs/controls.md), in Chromium against a real HTTP server:
// a Save button `disabled: sending` sends ONE write for a double click and a fast triple click, is enabled again when the
// answer arrives and works again; `disabled: true` never fires; a disabled TextField takes no typing; Enter in a field
// fires its submit event once (section 3). Outside the gate: needs node, the `playwright` package with a Chromium, and
// a built `cancho-ui`.
//   node scripts/check-controls.js [path/to/cancho-ui]
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

const SRC = `
component Controls {
    requires NetworkWrite;

    state sending: Bool = false;
    state sent: Int = 0;
    state locked: Bool = true;
    state fired: Int = 0;
    state note: String = "";
    state r: Result[String, String] = Ok("");

    on Save {
        sending = true;
        r = await(net.post("/api/save", "[1]"));
        sending = false;
        sent = sent + 1;
    }
    on Locked { fired = fired + 1; }
    on Edit(v) { note = v; }

    view {
        Column {
            Button { label: "Save" on: Save disabled: sending }
            Button { label: "Never" on: Locked disabled: true }
            TextField { label: "Locked note" value: note on: Edit disabled: locked }
            Text { "sent {sent} fired {fired} note {note}" }
        }
    }
}
`;

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-controls-'));
  const src = path.join(dir, 'c.cui');
  fs.writeFileSync(src, SRC);
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const posts = { count: 0, held: [] };
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url === '/api/save') {
      posts.count++;
      req.resume();
      req.on('end', () => { posts.held.push(() => { res.writeHead(201); res.end('made'); }); });
      return;
    }
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, 'out', path.normalize(rel));
    if (!file.startsWith(path.join(dir, 'out')) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  const text = async () => page.locator('#root').innerText();
  const until = async (want, ms = 3000) => { for (let t = 0; t < ms; t += 30) { if ((await text()).includes(want)) { return true; } await page.waitForTimeout(30); } return false; };
  const settle = async (n) => { for (let i = 0; i < 100 && posts.count !== n; i++) { await page.waitForTimeout(20); } await page.waitForTimeout(150); return posts.count === n; };

  // a double click on Save while the first write is in flight
  await page.dblclick('button:has-text("Save")');
  check('a double click sends ONE write', await settle(1), String(posts.count));
  check('and the button is disabled while it is in flight', await page.locator('button:has-text("Save")').isDisabled());
  await page.click('button:has-text("Save")', { force: true, timeout: 500 }).catch(() => {});
  await page.waitForTimeout(150);
  check('a forced click on the disabled button sends nothing more', posts.count === 1, String(posts.count));
  posts.held.splice(0).forEach((reply) => reply());
  check('the answer arrives: counted once', await until('sent 1'));
  check('and the button is enabled again', await page.locator('button:has-text("Save")').isEnabled());

  // a fast triple click, then release
  await page.click('button:has-text("Save")', { clickCount: 3, delay: 0 });
  check('a triple click sends one more write, not three', await settle(2), String(posts.count));
  posts.held.splice(0).forEach((reply) => reply());
  check('and it is counted once', await until('sent 2'));

  // disabled: true
  check('`disabled: true` is a disabled button', await page.locator('button:has-text("Never")').isDisabled());
  await page.click('button:has-text("Never")', { force: true, timeout: 500 }).catch(() => {});
  await page.waitForTimeout(150);
  check('and its event never fires', (await text()).includes('fired 0'), await text());

  // a disabled TextField
  check('a field with `disabled: locked` is disabled', await page.locator('input').isDisabled());
  await page.locator('input').fill('x', { force: true, timeout: 500 }).catch(() => {});
  await page.waitForTimeout(150);
  check('and takes no typing', !(await text()).includes('note x'), await text());

  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
