#!/usr/bin/env node
// Browser-side checks of `cancho-ui build --pwa` (docs/mobile-backend.md, section 7, layer 2). Outside the gate: it needs
// node, the `playwright` package with a Chromium, and a built `cancho-ui`. Prints what it measured and exits 1 on a
// failed check. Chromium only: nothing here says what iOS Safari does.
//   node scripts/check-pwa.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = {
  '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png',
  '.webmanifest': 'application/manifest+json',
};

function build(example, dir) {
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, example), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${example}: ${r.stderr}`); }
}

function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    // no-store: the browser's own HTTP cache must not answer, so an offline start proves the service worker's cache.
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

async function one(example, interact) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-pwa-'));
  build(example, dir);
  const bytes = fs.readdirSync(dir).reduce((n, f) => n + fs.statSync(path.join(dir, f)).size, 0);
  console.log(`\n== ${example}: ${fs.readdirSync(dir).length} files, ${bytes} bytes`);
  const server = await serve(dir);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const context = await browser.newContext({ ...devices['Pixel 5'], serviceWorkers: 'allow' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });

  await page.goto(url);
  await page.waitForSelector('#root *');
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload(); // the worker now controls the page
  await page.waitForSelector('#root *');

  const cdp = await context.newCDPSession(page);
  await cdp.send('Page.enable');
  await page.waitForTimeout(500);
  const installability = await cdp.send('Page.getInstallabilityErrors');
  check('Chromium reports no installability errors', installability.installabilityErrors.length === 0,
    JSON.stringify(installability.installabilityErrors));

  const manifest = await cdp.send('Page.getAppManifest');
  // An absent manifest is reported as an empty one with no errors, so require a URL and a name as well.
  const parsed = manifest.data ? JSON.parse(manifest.data) : {};
  check('the page has a manifest, with a name, and it parses with no errors',
    /manifest\.webmanifest$/.test(manifest.url) && typeof parsed.name === 'string' && parsed.name.length > 0 && (manifest.errors || []).length === 0,
    `${manifest.url} ${JSON.stringify(parsed.name)} ${JSON.stringify(manifest.errors || [])}`);

  const cacheNames = await page.evaluate(() => caches.keys());
  check('exactly one cancho-ui cache exists', cacheNames.length === 1 && cacheNames[0].startsWith('cancho-ui-'), cacheNames.join(','));

  const boxes = await page.evaluate(() => Array.from(document.querySelectorAll('button, input, textarea')).map((e) => {
    const r = e.getBoundingClientRect();
    return { tag: e.tagName, w: r.width, h: r.height };
  }));
  const small = boxes.filter((b) => b.w < 44 || b.h < 44);
  check(`every interactive element is at least 44 x 44 CSS px at a phone viewport (${boxes.length} found)`,
    boxes.length > 0 && small.length === 0, small.length ? JSON.stringify(small) : '');

  // Really offline: stop the server. (Playwright's setOffline does not reach a service worker's own requests.)
  server.closeAllConnections();
  await new Promise((done) => server.close(done));
  let offlineText = '';
  try {
    await page.reload();
    await page.waitForSelector('#root *', { timeout: 5000 });
    offlineText = await page.textContent('#root');
  } catch (e) {
    offlineText = '';
  }
  check('the page starts offline after the first load', offlineText.trim().length > 0, JSON.stringify(offlineText.trim().slice(0, 60)));
  if (interact && offlineText.trim().length > 0) {
    const result = await interact(page);
    check(result.label, result.ok, result.detail);
  }
  check('no script or console errors', errors.length === 0, errors.join(' | '));
  await browser.close();
}

(async () => {
  await one('examples/counter/counter.cui', async (page) => {
    const before = await page.textContent('#root');
    await page.getByRole('button').first().tap();
    const after = await page.textContent('#root');
    return { label: 'a tap works offline (the counter changed)', ok: before !== after, detail: `${JSON.stringify(before.trim())} -> ${JSON.stringify(after.trim())}` };
  });
  await one('examples/search/search.cui', null);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
