#!/usr/bin/env node
// The Stopwatch gallery demo (examples/clock/clock.cui, issue #114) in Chromium, in the mobile profile, with the real clock: it reads
// "Elapsed: 0s" before Start (never an epoch-sized number); after Start it ticks on its own (the reading grows with nobody touching
// the page, between 0.8 and 3 s after about a second); Stop freezes it; Start resumes from the frozen reading, not from 0. Replay checks
// the same program with `cancho-ui test` (examples/clock/clock.trace.json, in the cargo gate through the examples check). Outside the
// gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-stopwatch.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-stopwatch-'));
  const built = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/clock/clock.cui'), '--page', '-o', dir], { encoding: 'utf8' });
  if (built.status !== 0) { throw new Error(built.stderr); }
  const server = http.createServer((req, res) => {
    const u = req.url.split('?')[0];
    const f = path.join(dir, u === '/' ? 'index.html' : path.normalize(u));
    if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : 'text/html', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root button');
  const reading = async () => {
    const t = await page.evaluate(() => document.body.innerText);
    const m = t.match(/Elapsed: (\S+)s/);
    return m ? Number(m[1]) : NaN;
  };
  const press = (name) => page.getByRole('button', { name }).tap();

  const before = await reading();
  check('before Start the reading is 0 seconds, not an epoch time', before === 0, `Elapsed: ${before}s`);
  await page.waitForTimeout(400);
  check('it stays 0 before Start (nothing ticks while stopped)', (await reading()) === 0);
  await press('Start');
  await page.waitForTimeout(1200);
  const running = await reading();
  check('after Start it ticks by itself (0.8 to 3 s after about 1.2 s)', running >= 0.8 && running <= 3, `Elapsed: ${running}s`);
  await page.waitForTimeout(500);
  const later = await reading();
  check('it keeps growing without a tap', later > running, `${running} then ${later}`);
  await press('Stop');
  const frozen = await reading();
  await page.waitForTimeout(500);
  check('Stop freezes the reading', (await reading()) === frozen, `Elapsed: ${frozen}s`);
  await press('Start');
  await page.waitForTimeout(300);
  const resumed = await reading();
  check('Start resumes from the frozen reading, not from 0', resumed >= frozen && resumed < frozen + 1.5, `${frozen} then ${resumed}`);
  check('no script error', errors.length === 0, errors.join('; '));

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
