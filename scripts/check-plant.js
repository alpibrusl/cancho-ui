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
    if (url === '/admin/metrics') { res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(admin.metrics); return; }
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
  const admin = { metrics: METRICS, ready: '{"ready":true}\n', readyStatus: 200 };
  const server = await serve(dir, admin);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(url);
  await page.waitForSelector('#root *');
  check('before a refresh nothing is claimed', (await shown(page)).includes('ready: unknown') && (await shown(page)).includes('Routes (0)'), await shown(page));

  await page.click('text=Refresh');
  await page.waitForFunction(() => document.body.innerText.includes('status: updated'));
  const t = await shown(page);
  check('the status, readiness, uptime and sessions are read', ['status: updated', 'ready: true', 'uptime (ms): 86400000', 'sessions: 3'].every((x) => t.includes(x)), t);
  check('both routes are listed with their counters', t.includes('Routes (2)') && t.includes('api|  2xx: 4021|  5xx: 2|web|  2xx: 950|  5xx: 0'), t);
  check('both upstreams are listed with their circuit', t.includes('api|  circuit open: false|  failures: 2|web|  circuit open: true|  failures: 0'), t);

  // a gateway that is not ready: 503 comes back as an error with no body (docs/adapters.md section 9)
  admin.ready = '{"ready":false,"why":["upstream web: circuit open"]}\n';
  admin.readyStatus = 503;
  await page.click('text=Refresh');
  await page.waitForFunction(() => document.body.innerText.includes('ready: not ready (HTTP 503)'));
  check('a 503 on readiness is shown as not ready', (await shown(page)).includes('ready: not ready (HTTP 503)'), await shown(page));

  // a document that is not JSON: the defaults, not a crash
  admin.metrics = '<html>proxy error</html>';
  admin.readyStatus = 200;
  admin.ready = 'also not json';
  await page.click('text=Refresh');
  await page.waitForFunction(() => document.body.innerText.includes('status: updated') && document.body.innerText.includes('Routes (0)'));
  const bad = await shown(page);
  check('a document that is not JSON gives the defaults', bad.includes('ready: unknown') && bad.includes('uptime (ms): -') && bad.includes('sessions: -') && bad.includes('Routes (0)'), bad);

  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
