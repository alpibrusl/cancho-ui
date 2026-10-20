#!/usr/bin/env node
// Browser-side check of examples/plant: the page reads a gateway-shaped admin document with net.get and takes it
// apart with json.get and json.items (docs/adapters.md section 14). Outside the gate: needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-plant.js [path/to/cancho-ui]
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

// The shape of cancho-gateway's GET /metrics (docs/observability.md), keys in the order the gateway writes them.
const METRICS = '{"v":1,"uptime_ms":86400000,"sessions_active":3,"le_ms":[1,2,5,10,25,50,100,250,500,1000,2500,5000,10000],' +
  '"routes":[{"name":"api","requests":{"none":0,"1xx":0,"2xx":4021,"3xx":0,"4xx":12,"5xx":2}},{"name":"web","requests":{"none":0,"1xx":0,"2xx":950,"3xx":4,"4xx":1,"5xx":0}}],' +
  '"upstreams":[{"name":"api","responses":4035,"retries":1,"failures":2,"pool_idle":3,"circuit_open":false},{"name":"web","responses":955,"retries":0,"failures":0,"pool_idle":4,"circuit_open":true}],' +
  '"log":{"dropped":0,"write_failures":0}}\n';

function serve(dir, admin) {
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url === '/admin/metrics') { admin.metricsRequests++; }
    if (url === '/admin/readyz' && admin.hold) { admin.held.push(() => { res.writeHead(admin.readyStatus, { 'content-type': 'application/json' }); res.end(admin.ready); }); return; }
    if (url === '/admin/metrics') { res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(admin.metrics); return; }
    if (url === '/admin/readyz' && admin.dead) { req.socket.destroy(); return; }
    if (url === '/admin/readyz') { res.writeHead(admin.readyStatus, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(admin.ready); return; }
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

async function shown(page) {
  return page.evaluate(() => Array.from(document.querySelectorAll('#root *')).filter((e) => e.children.length === 0).map((e) => e.textContent).join('|'));
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-plant-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/plant/plant.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  const admin = { metrics: METRICS, ready: '{"ready":true}\n', readyStatus: 200, metricsRequests: 0, hold: false, held: [] };
  const server = await serve(dir, admin);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  await page.clock.install({ time: 0 });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const settle = async (text) => { for (let i = 0; i < 100; i++) { if ((await page.evaluate(() => document.body.innerText)).includes(text)) { return true; } await page.waitForTimeout(50); } return false; };
  await page.goto(url);
  await page.waitForSelector('#root *');

  // `start Refresh`: the page reads the gateway when it opens, without a click
  check('the page reads the gateway by itself when it opens', await settle('status: updated'), await shown(page));
  const t = await shown(page);
  check('the status, readiness, uptime and sessions are read', ['status: updated', 'ready: true', 'uptime (ms): 86400000', 'sessions: 3'].every((x) => t.includes(x)), t);
  check('both routes are listed with their counters', t.includes('Routes (2)') && t.includes('api|  2xx: 4021|  5xx: 2|web|  2xx: 950|  5xx: 0'), t);
  check('both upstreams are listed with their circuit', t.includes('api|  circuit open: false|  failures: 2|web|  circuit open: true|  failures: 0'), t);
  check('one read of the metrics so far', admin.metricsRequests === 1, String(admin.metricsRequests));

  // `every 5000 Refresh`: a tick reads again, with no click
  admin.metrics = METRICS.replace('86400000', '90000000');
  await page.clock.runFor(5000);
  check('a tick after 5 seconds reads the gateway again', await settle('uptime (ms): 90000000') && admin.metricsRequests === 2, `${admin.metricsRequests} reads; ${await shown(page)}`);

  // a tick that falls due while a refresh is waiting is skipped, not started on top
  admin.hold = true;
  admin.metrics = METRICS.replace('86400000', '91000000');
  await page.clock.runFor(5000);
  await settle('status: loading');
  check('a refresh waiting on readiness is in flight', admin.metricsRequests === 3 && (await shown(page)).includes('status: loading'), `${admin.metricsRequests} reads; ${await shown(page)}`);
  await page.clock.runFor(5000);
  await page.waitForTimeout(300);
  check('the next tick, while that refresh waits, is skipped', admin.metricsRequests === 3, String(admin.metricsRequests));
  admin.hold = false;
  admin.held.splice(0).forEach((reply) => reply());
  check('the waiting refresh finishes when the gateway answers', await settle('status: updated') && (await shown(page)).includes('uptime (ms): 91000000'), await shown(page));
  await page.clock.runFor(5000);
  check('and the tick after that runs', await settle('status: updated') && admin.metricsRequests === 4, String(admin.metricsRequests));

  // a gateway that is not ready: 503 with its reasons in the body, which net.fetch keeps (docs/adapters.md section 15)
  admin.ready = '{"ready":false,"why":["upstream web: circuit open","log: a write failed in the last 10 s"]}\n';
  admin.readyStatus = 503;
  await page.click('text=Refresh');
  await settle('ready: false');
  const nr = await shown(page);
  check('a 503 on readiness is shown with its reasons', nr.includes('ready: false') && nr.includes('because: upstream web: circuit open') && nr.includes('because: log: a write failed in the last 10 s'), nr);

  // a transport error on readiness
  admin.dead = true;
  await page.click('text=Refresh');
  await settle('ready: unknown (network error)');
  check('a transport error on readiness is unknown, with the reason', (await shown(page)).includes('ready: unknown (network error)') && !(await shown(page)).includes('because'), await shown(page));
  admin.dead = false;

  // a document that is not JSON: the defaults, not a crash
  admin.metrics = '<html>proxy error</html>';
  admin.readyStatus = 200;
  admin.ready = 'also not json';
  await page.click('text=Refresh');
  await settle('Routes (0)');
  const bad = await shown(page);
  check('a document that is not JSON gives the defaults', bad.includes('ready: unknown') && bad.includes('uptime (ms): -') && bad.includes('sessions: -') && bad.includes('Routes (0)'), bad);

  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
