#!/usr/bin/env node
// Browser-side check of the installed ops-console started with the service DOWN (docs/adapters.md section 19, issue #44).
// The page is built with --pwa, loaded once with the stand-in service up (so the service worker is installed and the
// program has saved its list), then the server is STOPPED (Playwright's setOffline does not reach a service worker's
// requests) and the page is reloaded: a cold start with no network. Outside the gate: needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-offline.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');
const { createServer } = require('./ops-console-server.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-offline-'));
  const built = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/ops-console/ops-console.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (built.status !== 0) { throw new Error(built.stderr); }
  const server = await createServer(path.join(dir, 'out'), {});
  const port = server.address().port;
  const url = `http://localhost:${port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const context = await browser.newContext({ serviceWorkers: 'allow' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const texts = async () => page.evaluate(() => Array.from(document.querySelectorAll('#root span')).map((e) => e.textContent).concat(Array.from(document.querySelectorAll('#root tbody tr')).filter((tr) => tr.querySelector('td')).map((tr) => Array.from(tr.querySelectorAll('td')).filter((td) => !td.querySelector('button')).map((td) => td.textContent).join('  '))));

  await page.goto(url);
  await page.waitForFunction(() => /23 machines|of 23/.test(document.body.innerText), null, { timeout: 8000 });
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload();
  await page.waitForFunction(() => /of 23 machines/.test(document.body.innerText), null, { timeout: 8000 });
  const stored = await page.evaluate(() => localStorage.getItem('cui:ops-console:machines'));
  check('the list is saved in the page\'s storage after a successful read', typeof stored === 'string' && stored.split('\n').filter(Boolean).length === 23, `${stored ? stored.length : 0} bytes`);

  // The service goes away. A cold start has no network at all.
  server.closeAllConnections();
  await new Promise((done) => server.close(done));
  await page.reload();
  await page.waitForFunction(() => /status: /.test(document.body.innerText), null, { timeout: 8000 });
  await page.waitForFunction(() => !/status: loading/.test(document.body.innerText), null, { timeout: 8000 });
  const t = await texts();
  const status = t.find((x) => x.startsWith('status: ')) || '';
  const count = t.find((x) => /machines, page/.test(x)) || '';
  check('the installed page opens with the service down', t.length > 0, JSON.stringify(status));
  check('the status says the read failed (it is not shown as fresh)', /^status: error: .*showing the last saved list/.test(status), JSON.stringify(status));
  check('the last saved list is shown', /^5 of 23 machines|^23 of 23 machines/.test(count) || /of 23 machines/.test(count), JSON.stringify(count));
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
