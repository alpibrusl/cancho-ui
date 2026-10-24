#!/usr/bin/env node
// Browser-side checks of `start` and `every` (docs/schedules.md section 7), in Chromium with a fake clock
// (`page.clock`): start runs once after the first render; a tick every period; a tick that finds a run in flight is
// skipped; a click counts as a run in flight; a hidden page runs no tick and gets one catch-up tick when shown
// (docs/schedules.md section 11); disposing a component stops its timers. Outside the gate: needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`. Timers in a background tab are not tested.
//   node scripts/check-schedules.js [path/to/cancho-ui]
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

// A check that waits for a page must not wait for ever (a spoiled page can leave a promise unsettled): it fails on its own after 4 minutes.
setTimeout(() => { console.error('FAIL the check took more than 4 minutes'); process.exit(1); }, 240000).unref();

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

// A component that pings the server on start and on every tick (the server counts), and a parent that can remove it.
const SRC = `
component Pinger {
    requires Network;

    state r: Result[String, String] = Ok("");

    start Ping;
    every 1000 Ping;

    on Ping { r = await(net.get("ping")); }

    view {
        Column {
            Text { "pinger" }
            Button { label: "Ping now" on: Ping }
        }
    }
}

component Page {
    state items: List[String] = ["x"];
    state label: String = "mounted";

    on Remove {
        items = [];
        label = "removed";
    }

    view {
        Column {
            Text { "{label}" }
            For {
                over: items
                as: i
                Pinger { }
            }
            Button { label: "Remove" on: Remove }
        }
    }
}
`;

function serve(dir, hits) {
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url === '/ping') {
      hits.count++;
      if (hits.hold) { hits.held.push(() => { res.writeHead(200); res.end('pong'); }); req.on('close', () => { if (!res.writableEnded) { hits.aborted++; } }); return; }
      res.writeHead(200, { 'cache-control': 'no-store' });
      res.end('pong');
      return;
    }
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-schedules-'));
  const src = path.join(dir, 'p.cui');
  fs.writeFileSync(src, SRC);
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const hits = { count: 0, hold: false, held: [], aborted: 0 };
  const server = await serve(path.join(dir, 'out'), hits);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  await page.clock.install({ time: 0 });
  // Playwright's clock keeps running in real time after install, and `pauseAt` cannot go back: reading `Date.now()` in the page
  // and pausing at `now + 1` failed with "Cannot fast-forward to the past" whenever more than 1 ms passed in between (1 run in 36
  // with six busy loops on four cores, #74). Pause at a time the clock cannot have reached yet, before the page exists, and move
  // time with `runFor` only.
  await page.clock.pauseAt(60000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const settle = async (n) => { for (let i = 0; i < 100; i++) { if (hits.count === n) { await page.waitForTimeout(100); return hits.count === n; } await page.waitForTimeout(30); } return false; };

  await page.goto(url);
  await page.waitForSelector('#root *');
  check('start runs once, after the first render', await settle(1), String(hits.count));

  // The clock is paused (above), so that only `runFor` moves time. A window of 1000 ms holds exactly one tick, wherever the first one falls.
  await page.waitForTimeout(1200);
  check('time stands still when paused: no tick without runFor', await settle(1), String(hits.count));
  await page.clock.runFor(1000);
  check('one tick in a window of one period', await settle(2), String(hits.count));
  // (a run is in flight until the server answers, so each window waits for it: a tick that finds one running is skipped)
  for (let k = 3; k <= 5; k++) {
    await page.clock.runFor(1000);
    if (!(await settle(k))) { break; }
  }
  check('three more ticks, one window each', await settle(5), String(hits.count));

  // a run in flight: the tick is skipped
  hits.hold = true;
  await page.clock.runFor(1000);
  check('a tick starts a run, which the server holds', await settle(6), String(hits.count));
  await page.clock.runFor(3000);
  await page.waitForTimeout(200);
  check('ticks while that run waits are skipped', hits.count === 6, String(hits.count));
  hits.hold = false;
  hits.held.splice(0).forEach((reply) => reply());
  await page.waitForTimeout(300);
  await page.clock.runFor(1000);
  check('the tick after the run finishes runs', await settle(7), String(hits.count));

  // a run started by a click is in flight too: the ticks that fall due while it waits are skipped
  hits.hold = true;
  await page.click('text=Ping now');
  check('a click starts a run, which the server holds', await settle(8), String(hits.count));
  await page.clock.runFor(2000);
  await page.waitForTimeout(200);
  check('ticks while the click\'s run waits are skipped', hits.count === 8, String(hits.count));
  hits.hold = false;
  hits.held.splice(0).forEach((reply) => reply());
  await page.waitForTimeout(300);

  // hidden pages (section 11): no tick while hidden, one catch-up tick when shown, the remaining time kept when none was due
  const setVisibility = (state) => page.evaluate((st) => {
    Object.defineProperty(document, 'visibilityState', { value: st, configurable: true });
    document.dispatchEvent(new Event('visibilitychange'));
  }, state);
  await setVisibility('hidden');
  await page.clock.runFor(5000);
  await page.waitForTimeout(200);
  check('a hidden page runs no tick', hits.count === 8, String(hits.count));
  await setVisibility('visible');
  check('shown again: one catch-up tick for the five that were missed', await settle(9), String(hits.count));
  await page.clock.runFor(999);
  await page.waitForTimeout(200);
  check('the next tick is a period after the catch-up, not before', hits.count === 9, String(hits.count));
  await page.clock.runFor(1);
  check('and then it comes', await settle(10), String(hits.count));
  await page.clock.runFor(400);
  await setVisibility('hidden');
  await page.clock.runFor(100);
  await setVisibility('visible');
  await page.waitForTimeout(200);
  check('hidden for less than the time left: no catch-up tick', hits.count === 10, String(hits.count));
  await page.clock.runFor(499);
  await page.waitForTimeout(200);
  check('the schedule kept the time it had left', hits.count === 10, String(hits.count));
  await page.clock.runFor(1);
  check('and ticks when that is over', await settle(11), String(hits.count));
  // pagehide pauses, a persisted pageshow resumes (the back-forward cache)
  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true })));
  await page.clock.runFor(3000);
  await page.waitForTimeout(200);
  check('pagehide pauses the ticks', hits.count === 11, String(hits.count));
  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true })));
  check('a persisted pageshow resumes them with one catch-up tick', await settle(12), String(hits.count));
  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: false })));
  await page.waitForTimeout(200);
  check('a pageshow that is not persisted changes nothing', hits.count === 12, String(hits.count));

  // disposal aborts the request in flight (docs/adapters.md 9.5): the server sees the connection close
  hits.hold = true;
  await page.click('text=Ping now');
  check('a click starts a run, which the server holds', await settle(13), String(hits.count));
  check('nothing was aborted so far', hits.aborted === 0, String(hits.aborted));
  await page.click('text=Remove');
  for (let i = 0; i < 50 && hits.aborted === 0; i++) { await page.waitForTimeout(20); }
  check('removing the component aborts the request it had in flight', hits.aborted === 1, String(hits.aborted));
  hits.hold = false;
  hits.held.splice(0);

  // disposal: the parent removes the component; no tick runs afterwards
  await page.click('text=Remove');
  await page.waitForTimeout(100);
  const before = hits.count;
  await page.clock.runFor(10000);
  await page.waitForTimeout(300);
  check('a removed component stops ticking', hits.count === before, `${before} -> ${hits.count}`);
  await setVisibility('hidden');
  await setVisibility('visible');
  await page.waitForTimeout(200);
  check('and no longer listens to the page being shown', hits.count === before, `${before} -> ${hits.count}`);
  check('no script errors', errors.length === 0, errors.join(' | '));

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
