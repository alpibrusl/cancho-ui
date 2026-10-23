#!/usr/bin/env node
// How an installed page updates (docs/mobile-backend.md section 5 and 9, issue #67 item 1). Two builds of one program are served
// from the same URL, one after the other. What the browser does is MEASURED here, not assumed: which version a reload shows, what
// is waiting, which caches exist, and what a fresh tab shows once every old tab is closed. Outside the gate: needs node, the
// `playwright` package with a Chromium and a built `cancho-ui`. Chromium only.
//   node scripts/check-update.js [path/to/cancho-ui]
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
// a check that waits for something that never comes must end, not hang; nothing below waits for this long on purpose
const CEILING_MS = 60000;
setTimeout(() => { console.error('FAIL the run took more than 3 minutes: something never arrived'); process.exit(1); }, 180000).unref();
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const program = (v) => `component App {\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view { Column { Text { role: status "this is version ${v}" } Button { label: "Go" on: Go } } }\n}\n`;

function build(v, dir) {
  const src = path.join(dir + '.src', 'app.cui');
  fs.mkdirSync(path.dirname(src), { recursive: true });
  fs.writeFileSync(src, program(v));
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', dir, '--name', 'App'], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
}

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-update-'));
  const v1 = path.join(tmp, 'v1');
  const v2 = path.join(tmp, 'v2');
  const site = path.join(tmp, 'site');
  build('one', v1);
  build('two', v2);
  const publish = (from) => { fs.rmSync(site, { recursive: true, force: true }); fs.cpSync(from, site, { recursive: true }); };
  publish(v1);
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(site, path.normalize(rel));
    if (!file.startsWith(site) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const context = await browser.newContext({ serviceWorkers: 'allow' });
  const text = (p) => p.locator('#root').innerText();
  const caches = (p) => p.evaluate(async () => (await caches.keys()).sort());
  const state = (p) => p.evaluate(async () => { const r = await navigator.serviceWorker.getRegistration(); return { active: !!(r && r.active), waiting: !!(r && r.waiting), installing: !!(r && r.installing) }; });

  const page1 = await context.newPage();
  const errors = [];
  page1.on('pageerror', (e) => errors.push(String(e)));
  await page1.goto(url);
  await page1.evaluate(() => navigator.serviceWorker.ready);
  await page1.reload();
  check('version one is installed and shown', (await text(page1)).includes('version one'), (await text(page1)).replace(/\n/g, ' | '));
  const cachesOne = await caches(page1);
  check('one cache exists, named for the content', cachesOne.length === 1 && /^cancho-ui-[0-9a-f]{16}$/.test(cachesOne[0]), JSON.stringify(cachesOne));

  // the site is replaced by version two; the browser is asked to look for an update (it also does so on navigation)
  publish(v2);
  // Wait for the condition, not for a time (#97). The registration is kept in the page so that the predicate is a plain function: a predicate
  // written `async () => { await getRegistration(); .. }` returns a Promise, which Playwright counts as true at once, so the "wait" returned
  // after about 100 ms and the check passed or failed on whether the worker had finished installing by then (measured: under load it had
  // not in 4 runs of 10; unloaded it had in 10 of 10). The ceiling only turns a hang into a failure: no step depends on it.
  await page1.evaluate(async () => { window.__reg = await navigator.serviceWorker.getRegistration(); await window.__reg.update(); });
  await page1.waitForFunction(() => !!window.__reg.waiting, null, { timeout: CEILING_MS });
  const s2 = await state(page1);
  const cachesTwo = await caches(page1);
  console.log(`     after the update check: ${JSON.stringify(s2)}, caches ${JSON.stringify(cachesTwo)}`);
  check('the new version is installed and WAITING (no skipWaiting)', s2.active && s2.waiting, JSON.stringify(s2));
  check('both versions are cached while one waits (the old one is not yet deleted)', cachesTwo.length === 2 && cachesTwo.includes(cachesOne[0]), JSON.stringify(cachesTwo));
  check('the open tab still shows version one (a page never runs against two versions)', (await text(page1)).includes('version one'));

  await page1.reload();
  const afterReload = await text(page1);
  console.log(`     the same tab after a reload shows: ${JSON.stringify(afterReload.replace(/\n/g, ' | '))}`);
  const afterReloadState = await state(page1);
  console.log(`     registration after a reload: ${JSON.stringify(afterReloadState)}`);

  const page2 = await context.newPage();
  await page2.goto(url);
  const second = await text(page2);
  console.log(`     a second tab opened while the first is open shows: ${JSON.stringify(second.replace(/\n/g, ' | '))}`);
  check('a second tab opened meanwhile also shows version one (the waiting worker is not used)', second.includes('version one'));

  await page1.close();
  await page2.close();
  await new Promise((done) => setTimeout(done, 1500));
  const page3 = await context.newPage();
  await page3.goto(url);
  await page3.waitForTimeout(500);
  const third = await text(page3);
  const s3 = await state(page3);
  const cachesThree = await caches(page3);
  console.log(`     a fresh tab after every old tab was closed shows: ${JSON.stringify(third.replace(/\n/g, ' | '))}; ${JSON.stringify(s3)}; caches ${JSON.stringify(cachesThree)}`);
  check('after every old tab is closed, a fresh tab shows version two', third.includes('version two'));
  check('and only the new cache is left (activate deleted the old one)', cachesThree.length === 1 && cachesThree[0] !== cachesOne[0], JSON.stringify(cachesThree));
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
