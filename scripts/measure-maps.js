#!/usr/bin/env node
// What docs/maps.md section 2 says, measured: the numbers of a "nearest places" list, the geometry of tiles, and in Chromium what a
// tile request carries, whether the page can bound the tile host, what the installed page's worker does with a tile, and what a
// screen reader gets. Not a check of the compiler. Outside the gate.
//   node scripts/measure-maps.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

setTimeout(() => { console.error('FAIL the measurement took more than 4 minutes'); process.exit(1); }, 240000).unref();
const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const say = (k, v) => console.log(`${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`);

// ---- 1. the list, in numbers -------------------------------------------------------------------------------------------------
let seed = 12345;
const rnd = () => { seed = (seed * 1664525 + 1013904223) % 4294967296; return seed / 4294967296; };
const RAD = Math.PI / 180;
const R = 6371.0088;
function haversine(a, b) {
  const dl = (b[0] - a[0]) * RAD; const dg = (b[1] - a[1]) * RAD;
  const h = Math.sin(dl / 2) ** 2 + Math.cos(a[0] * RAD) * Math.cos(b[0] * RAD) * Math.sin(dg / 2) ** 2;
  return 2 * R * Math.asin(Math.min(1, Math.sqrt(h)));
}
// what the language can do today: + - * / abs min, no trig, no square root. cos by its series at the user's latitude.
function cosSeries(lat) { const t = lat * 0.0174533; const t2 = t * t; return 1 - t2 / 2 + t2 * t2 / 24 - t2 * t2 * t2 / 720; }
function squared(user, p) {
  const c = cosSeries(user[0]); const d = Math.abs(p[1] - user[1]); const dg = Math.min(d, 360 - d);
  return (p[0] - user[0]) ** 2 + c * c * dg * dg;
}
function around(user, km) { // a place at a random distance (uniform in area) and bearing
  const d = km * Math.sqrt(rnd()) / R; const brg = rnd() * 2 * Math.PI; const l1 = user[0] * RAD; const g1 = user[1] * RAD;
  const l2 = Math.asin(Math.sin(l1) * Math.cos(d) + Math.cos(l1) * Math.sin(d) * Math.cos(brg));
  let g2 = g1 + Math.atan2(Math.sin(brg) * Math.sin(d) * Math.cos(l1), Math.cos(d) - Math.sin(l1) * Math.sin(l2));
  g2 = ((g2 / RAD + 540) % 360) - 180;
  return [l2 / RAD, g2];
}
const top = (places, key, n) => places.map((p, i) => [key(p), i]).sort((a, b) => a[0] - b[0] || a[1] - b[1]).slice(0, n).map((x) => x[1]);
let worstCos = 0;
for (let lat = 0; lat <= 75; lat += 0.5) { worstCos = Math.max(worstCos, Math.abs(cosSeries(lat) - Math.cos(lat * RAD))); }
say('cos by its series (4 terms): the largest error for latitudes 0 to 75', worstCos.toExponential(2));
for (const km of [10, 100, 1000]) {
  let sameSet = 0; let sameOrder = 0; let worstRel = 0; const N = 2000;
  for (let t = 0; t < N; t++) {
    const user = [(rnd() * 2 - 1) * 70, rnd() * 360 - 180];
    const places = Array.from({ length: 200 }, () => around(user, km));
    const exact = top(places, (p) => haversine(user, p), 5);
    const approx = top(places, (p) => squared(user, p), 5);
    if ([...exact].sort().join() === [...approx].sort().join()) { sameSet++; }
    if (exact.join() === approx.join()) { sameOrder++; }
    for (const p of places) {
      const e = haversine(user, p); if (e > 0.01) { worstRel = Math.max(worstRel, Math.abs(Math.sqrt(squared(user, p)) * 111.195 / e - 1)); }
    }
  }
  say(`nearest 5 of 200 places within ${km} km, ${N} random positions (latitude up to 70, any longitude), series cos and squared flat distance against haversine`,
    { sameSet: `${(100 * sameSet / N).toFixed(2)}%`, sameOrder: `${(100 * sameOrder / N).toFixed(2)}%`, worstDistanceError: `${(100 * worstRel).toFixed(2)}%` });
}
for (const [digits, label] of [[2, 'coarse'], [5, 'fine']]) {
  let changed = 0; const N = 2000; const f = 10 ** digits;
  for (let t = 0; t < N; t++) {
    const user = [(rnd() * 2 - 1) * 60, rnd() * 360 - 180];
    const places = Array.from({ length: 200 }, () => around(user, 10));
    const rounded = [Math.round(user[0] * f) / f, Math.round(user[1] * f) / f];
    if (top(places, (p) => haversine(user, p), 5).join() !== top(places, (p) => haversine(rounded, p), 5).join()) { changed++; }
  }
  say(`nearest 5 of 200 places within 10 km: the list differs when the position is rounded (${label}, ${digits} decimals)`, `${(100 * changed / N).toFixed(1)}% of ${N}`);
}

// ---- 2. tiles, in numbers ----------------------------------------------------------------------------------------------------
for (const lat of [0, 40.4, 60, 75]) {
  const row = {};
  for (const z of [10, 12, 13, 14, 15, 16, 18]) {
    const lonDeg = 360 / 2 ** z; const latDeg = lonDeg * Math.cos(lat * RAD);
    row[`z${z}`] = `${(latDeg * 111.195).toFixed(2)} km x ${(lonDeg * 111.195 * Math.cos(lat * RAD)).toFixed(2)} km`;
  }
  say(`a tile (north-south x east-west) at latitude ${lat}`, row);
}
for (const lat of [0, 40.4, 60, 70, 75, 80]) {
  const fits = (digits, z) => 360 * Math.cos(lat * RAD) / 2 ** z >= 10 ** -digits;
  let coarse = 0; let fine = 0;
  for (let z = 0; z <= 22; z++) { if (fits(2, z)) { coarse = z; } if (fits(5, z)) { fine = z; } }
  say(`latitude ${lat}: the highest zoom whose tile is at least as large as the rounding cell`, { coarse2decimals: coarse, fine5decimals: fine });
}
{
  const view = { w: 412, h: 300 }; const T = 256;
  const sets = new Set(); let min = 99; let max = 0;
  for (let ox = 0; ox < T; ox++) {
    for (let oy = 0; oy < T; oy++) {
      const x0 = Math.floor((ox - view.w / 2) / T); const x1 = Math.floor((ox + view.w / 2 - 1) / T);
      const y0 = Math.floor((oy - view.h / 2) / T); const y1 = Math.floor((oy + view.h / 2 - 1) / T);
      sets.add(`${x0},${x1},${y0},${y1}`); const n = (x1 - x0 + 1) * (y1 - y0 + 1); min = Math.min(min, n); max = Math.max(max, n);
    }
  }
  say('a 412 x 300 view centred on the position, 256 px tiles, every offset of the position inside its tile', { tilesRequired: `${min} to ${max}`, distinctRequestSets: sets.size });
  say('the same view centred on the middle of the position\'s tile', { tilesRequired: (Math.floor((T / 2 + view.w / 2 - 1) / T) - Math.floor((T / 2 - view.w / 2) / T) + 1) * (Math.floor((T / 2 + view.h / 2 - 1) / T) - Math.floor((T / 2 - view.h / 2) / T) + 1), distinctRequestSets: 1 });
}

// ---- 3. what the language can do today ---------------------------------------------------------------------------------------
{
  const r = spawnSync(CANCHO_UI, ['test', path.join(__dirname, 'measure-maps', 'near.cui'), path.join(__dirname, 'measure-maps', 'near.trace.json')], { encoding: 'utf8', timeout: 60000 });
  say('scripts/measure-maps/near.cui, the nearest three of five places, written with the builtins that exist (split, take, skip, join, num, sort, min, abs), replayed', { exit: r.status, lastLine: (r.stdout || '').trim().split('\n').pop() });
  say('the length of that sort key, in characters', fs.readFileSync(path.join(__dirname, 'measure-maps', 'near.cui'), 'utf8').match(/sort\(places, r => (.*)\), 3\);/)[1].length);
}

// ---- 4. Chromium -------------------------------------------------------------------------------------------------------------
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
function tileServer(log, extra) {
  const s = http.createServer((req, res) => {
    log.push({ url: req.url, headers: req.headers });
    res.writeHead(200, { 'content-type': 'image/png', 'content-length': PNG.length, ...extra });
    res.end(PNG);
  });
  return new Promise((ok) => s.listen(0, '127.0.0.1', () => ok(s)));
}
function pageServer(dir, headers) {
  const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
  const s = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store', ...headers });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => s.listen(0, '127.0.0.1', () => ok(s)));
}
const pick = (h) => ({ referer: h.referer || null, 'user-agent': (h['user-agent'] || '').replace(/Chrome\/[\d.]+/, 'Chrome/x'), accept: h.accept, 'sec-fetch-site': h['sec-fetch-site'], 'sec-fetch-mode': h['sec-fetch-mode'], cookie: h.cookie || null, origin: h.origin || null });
const loadImg = (page, src, attrs) => page.evaluate(([s, a]) => new Promise((ok) => {
  const t0 = performance.now(); const i = document.createElement('img');
  for (const k in a) { i.setAttribute(k, a[k]); }
  i.onload = () => ok({ loaded: true, w: i.naturalWidth, ms: Math.round(performance.now() - t0) });
  i.onerror = () => ok({ loaded: false, w: i.naturalWidth, ms: Math.round(performance.now() - t0) });
  i.src = s; document.body.appendChild(i);
}), [src, attrs || {}]);

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'maps-'));
  const b = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/where/where.cui'), '--pwa', '-o', dir], { encoding: 'utf8' });
  if (b.status !== 0) { console.error(b.stderr); process.exit(1); }
  const logA = []; const logB = []; const tilesA = await tileServer(logA, { 'cache-control': 'max-age=3600' }); const tilesB = await tileServer(logB, {});
  const hostA = `localhost:${tilesA.address().port}`; const hostB = `localhost:${tilesB.address().port}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();

  // (a) what a tile request carries
  const site = await pageServer(dir, {}); const origin = `http://127.0.0.1:${site.address().port}`;
  await page.goto(`${origin}/index.html?pos=40.42,-3.70#places`);
  logA.length = 0;
  await loadImg(page, `http://${hostA}/14/8000/6000.png`);
  await loadImg(page, `http://${hostA}/14/8001/6000.png`, { referrerpolicy: 'no-referrer' });
  await loadImg(page, `http://${hostA}/14/8002/6000.png`, { referrerpolicy: 'origin' });
  say('a tile request from an img, the page at <origin>/index.html?pos=40.42,-3.70#places (default policy)', pick(logA[0].headers));
  say('the same with referrerpolicy="no-referrer" (referer)', logA[1].headers.referer || null);
  say('the same with referrerpolicy="origin" (referer)', logA[2].headers.referer || null);
  say('the tile URL is the only place the position goes (x, y at zoom 14 for 40.42,-3.70)', (() => { const n = 2 ** 14; return `${Math.floor((-3.70 + 180) / 360 * n)}/${Math.floor((1 - Math.log(Math.tan(40.42 * RAD) + 1 / Math.cos(40.42 * RAD)) / Math.PI) / 2 * n)}`; })());

  // (b) can the page bound the tile host? Without a policy, and with one in a meta tag.
  logA.length = 0; logB.length = 0;
  say('no policy: an img from a host the program never declared', await loadImg(page, `http://${hostB}/1/0/0.png`));
  say('no policy: that host saw the request', logB.length === 1);
  const csp = `<!doctype html><meta http-equiv="Content-Security-Policy" content="img-src 'self' data: ${hostA}"><body>`;
  fs.writeFileSync(path.join(dir, 'csp.html'), csp);
  await page.goto(`${origin}/csp.html`);
  await page.evaluate(() => { window.violations = []; document.addEventListener('securitypolicyviolation', (e) => window.violations.push(e.violatedDirective + ' ' + e.blockedURI)); });
  logA.length = 0; logB.length = 0;
  say('<meta> img-src naming one host: an img from that host', await loadImg(page, `http://${hostA}/2/0/0.png`));
  say('<meta> img-src naming one host: an img from another host', await loadImg(page, `http://${hostB}/2/0/0.png`));
  say('<meta> img-src: requests the declared host saw, the other host saw', [logA.length, logB.length]);
  say('<meta> img-src: the violation the page can hear', await page.evaluate(() => window.violations));
  say('<meta> img-src: a CSS background-image from the other host', await page.evaluate(([u]) => new Promise((ok) => { const d = document.createElement('div'); d.style.cssText = `width:10px;height:10px;background-image:url(${u})`; document.body.appendChild(d); setTimeout(() => ok(true), 300); }), [`http://${hostB}/css.png`]).then(() => logB.some((e) => e.url === '/css.png') ? 'loaded' : 'blocked'));

  // (c) the installed page's worker
  await page.goto(`${origin}/index.html`);
  await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
  await page.reload();
  say('the worker controls the page', await page.evaluate(() => !!navigator.serviceWorker.controller));
  logA.length = 0;
  say('online, through the worker: a tile from another host', await loadImg(page, `http://${hostA}/10/1/1.png`));
  say('the tile host saw it', logA.length === 1);
  await ctx.setOffline(true);
  say('offline, a tile never seen', await loadImg(page, `http://${hostA}/10/1/2.png`));
  say('offline, the tile seen online, asked for again with another query string (a different URL)', await loadImg(page, `http://${hostA}/10/1/1.png?again`));
  await ctx.setOffline(false);
  await new Promise((ok) => tilesA.close(ok));
  tilesA.closeAllConnections();
  say('the tile host stopped (not an emulated offline mode), the same tile again with max-age=3600 (the browser\'s HTTP cache)', await loadImg(page, `http://${hostA}/10/1/1.png`));
  say('the tile host stopped, a tile never seen', await loadImg(page, `http://${hostA}/10/1/3.png`));
  const tilesA2 = await new Promise((ok) => { const s2 = http.createServer((req, res) => { res.writeHead(200, { 'content-type': 'image/png', 'content-length': PNG.length }); res.end(PNG); }); s2.listen(0, '127.0.0.1', () => ok(s2)); });
  const quota = await page.evaluate(async ([u]) => {
    const est = async () => (await navigator.storage.estimate()).usage;
    const c = await caches.open('measure');
    const before = await est();
    for (let i = 0; i < 3; i++) { await c.put(`${u}/o${i}`, await fetch(`${u}/o${i}`, { mode: 'no-cors' })); }
    const opaque = await est();
    return { before, opaque, perOpaqueTile: Math.round((opaque - before) / 3) };
  }, [`http://localhost:${tilesA2.address().port}`]);
  say('the storage a tile costs when the page keeps it with cache.put as an opaque (no-cors) response, bytes (the tile is 70 bytes)', quota);
  const corsServer = await tileServer([], { 'access-control-allow-origin': '*' });
  const corsHost = `http://localhost:${corsServer.address().port}`;
  const quota2 = await page.evaluate(async ([u]) => {
    const est = async () => (await navigator.storage.estimate()).usage;
    const c = await caches.open('measure2');
    const before = await est();
    for (let i = 0; i < 3; i++) { await c.put(`${u}/c${i}`, await fetch(`${u}/c${i}`)); }
    const after = await est();
    return { before, after, perTile: Math.round((after - before) / 3) };
  }, [corsHost]);
  say('the same when the tile host allows CORS (a readable response), bytes', quota2);

  // (d) what a screen reader gets
  const ax = async (html) => {
    await page.setContent(html);
    const cdp = await ctx.newCDPSession(page);
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    const live = nodes.filter((n) => !n.ignored);
    await cdp.detach();
    return { nodes: live.length, roles: live.map((n) => n.role && n.role.value).filter((r) => r !== 'none').join(' ') };
  };
  const tiles = (attrs) => Array.from({ length: 9 }, (_, i) => `<img src="http://${hostA}/z/${i}.png" ${attrs} width="256" height="256">`).join('');
  const list = '<ul><li>Sol, 0.3 km</li><li>Retiro, 1.2 km</li><li>Atocha, 1.9 km</li></ul>';
  say('9 tiles with no alt attribute, then a list of 3', await ax(`<div>${tiles('')}</div>${list}`));
  say('9 tiles with alt="" , then a list of 3', await ax(`<div>${tiles('alt=""')}</div>${list}`));
  say('9 tiles in a role="img" box named "Map of places near you", then a list of 3', await ax(`<div role="img" aria-label="Map of places near you">${tiles('alt=""')}</div>${list}`));
  say('the same with the tiles aria-hidden and the marker pins as an svg with aria-hidden', await ax(`<div role="img" aria-label="Map of places near you"><div aria-hidden="true">${tiles('alt=""')}<svg aria-hidden="true" width="10" height="10"><circle r="4"/></svg></div></div>${list}`));

  await browser.close();
  for (const s of [site, tilesB, tilesA2, corsServer]) { s.close(); }
  fs.rmSync(dir, { recursive: true, force: true });
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
