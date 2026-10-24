#!/usr/bin/env node
// `Map` (docs/maps.md, issue #81 step two) in Chromium, in the mobile profile (Pixel 7: touch, 412 px). Every tile comes from a stand-in tile server on
// this machine (scripts/stand-in-tiles.js, scripts/tile-png.js) or from the example's own folder of stand-in tiles: nothing here asks a public tile
// provider for anything, and a request to any other host is refused by the check itself and fails it.
//  A. `__cui_map_plan` in the page against tests/map_corpus.json, the file `cargo test` runs on replay's;
//  B. examples/map as shipped (tiles from the page's own origin): exactly the nine tiles replay reports, no referrer, the picture named, the list beside
//     it and the attribution under it; a new centre asks for its nine tiles; a render that changes nothing asks for none; no tab stop and no overflow;
//  C. the same program with the tiles on another host, declared in `requires Network("host")`: the page carries a Content-Security-Policy, the tile
//     host sees the nine requests and no Referer, and an image from a host the program did not declare is never requested;
//  D. the tile host is gone: the list and the credit stay, and the picture says it is not available; one tile missing does not say so;
//  E. a coarse position (a bare `requires Location`) is not drawn finer than 1 km: at latitude 80 the zoom asked for is 12, not 13.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-map.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');
const tiles = require('./stand-in-tiles.js');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const J = JSON.stringify;
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
async function until(fn, ms = 4000) {
  for (let t = 0; t < ms; t += 25) { const v = await fn(); if (v) { return v; } await sleep(25); }
  return null;
}
const SOURCE = fs.readFileSync(path.join(ROOT, 'examples', 'map', 'map.cui'), 'utf8');
const TRACE = JSON.parse(fs.readFileSync(path.join(ROOT, 'examples', 'map', 'map.trace.json'), 'utf8'));
const planOf = (i) => TRACE.steps[i].expect.maps[0];
const SOL = planOf(0);
const AIRPORT = planOf(1);

// a server for a built page and anything beside it; `log` records the requests
function serve(dir, log) {
  const server = http.createServer((req, res) => {
    log.push({ url: req.url, headers: req.headers });
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const f = path.join(dir, path.normalize(rel));
    if (!f.startsWith(dir) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}
function build(src, root, name, withTiles) {
  const dir = path.join(root, name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'p.cui'), src);
  const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--page', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${name}: ${r.stdout}${r.stderr}`); }
  if (withTiles) { fs.cpSync(path.join(ROOT, 'examples', 'map', 'tiles'), path.join(dir, 'out', 'tiles'), { recursive: true }); }
  return path.join(dir, 'out');
}
// the tile addresses of a plan, as the paths a server sees
const pathsOf = (urls, prefix) => urls.map((u) => prefix + u.replace(/^\.\/tiles\//, '').replace(/^https?:\/\/[^/]+\//, ''));

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-map-'));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const stray = []; // every request that was not to a server of this check

  // ---- A. the plan, against the corpus ----------------------------------------------------------------------------------------------------
  const sameDir = build(SOURCE, root, 'same', true);
  {
    const js = fs.readFileSync(path.join(sameDir, 'app.js'), 'utf8');
    const a = js.indexOf('const __CUI_MAP_TILE');
    const e = js.indexOf('function mount_');
    check('A: the module carries the map adapter and the helpers it calls', a > 0 && e > a && js.includes('function __cui_distance(') && js.includes('function __cui_json_get('));
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'map_corpus.json'), 'utf8'));
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    const bad = await page.evaluate(({ src, cases }) => {
      const f = new Function(`${src}; return { plan: __cui_map_plan, getj: __cui_json_get, dist: __cui_distance };`)();
      const out = [];
      for (const c of cases) {
        const p = f.plan(c.center, c.rows, c.template, c.zoom, c.coarse, function (r, k) { return f.getj(r, k, ''); }, function (x, y) { return f.dist(x, y, -1); });
        const w = c.want;
        const same = p.zoom === w.zoom && JSON.stringify(p.tiles) === JSON.stringify(w.tiles)
          && JSON.stringify(p.list.map(function (l) { return { name: l[0], km: l[1] }; })) === JSON.stringify(w.list)
          && p.listed === w.listed && p.total === w.total && p.pinned === w.pinned;
        if (!same) { out.push(c.why); }
      }
      return out;
    }, { src: js.slice(a, e), cases: corpus.cases });
    check(`A: ${corpus.cases.length} cases of the corpus come out as written`, bad.length === 0, bad.join('; '));
    await ctx.close();
  }

  // a context that refuses every host this check does not run, and says so
  const open = async (url, hosts, options = {}) => {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], ...options });
    await ctx.route('**/*', (route) => {
      const u = new URL(route.request().url());
      if (hosts.includes(u.host) || u.protocol === 'data:') { route.continue(); } else { stray.push(route.request().url()); route.abort(); }
    });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('figure.cui-map');
    return { ctx, page, errors };
  };
  const shown = (page) => page.evaluate(() => {
    const f = document.querySelector('figure.cui-map');
    const pic = f.querySelector('[role=img]');
    const cap = f.querySelector('figcaption');
    const svg = f.querySelector('svg');
    const note = f.querySelector('[role=status]');
    return {
      imgs: [...f.querySelectorAll('img')].map((i) => ({ alt: i.getAttribute('alt'), policy: i.getAttribute('referrerpolicy'), src: i.getAttribute('src') })),
      list: [...f.querySelectorAll('ol li')].map((l) => l.textContent),
      name: pic ? pic.getAttribute('aria-label') : null,
      credit: cap ? cap.textContent : '',
      hiddenSvg: svg ? svg.getAttribute('aria-hidden') : null,
      pins: f.querySelectorAll('svg circle').length,
      note: note && note.hidden === false ? note.textContent : '',
    };
  });
  const listOf = (plan) => plan.list.map((l) => `${l.name}, ${l.km} km`);

  // ---- B. the example as shipped ---------------------------------------------------------------------------------------------------------------
  {
    const log = [];
    const server = await serve(sameDir, log);
    const host = `127.0.0.1:${server.address().port}`;
    const { ctx, page, errors } = await open(`http://${host}/`, [host]);
    await sleep(300);
    const tileReqs = () => log.filter((r) => r.url.startsWith('/tiles/')).map((r) => r.url);
    check('B: the page asked its own folder for exactly the nine tiles replay reports', J(tileReqs().sort()) === J(pathsOf(SOL.tiles, '/tiles/').sort()), J(tileReqs()));
    check('B: and for no Referer', log.filter((r) => r.url.startsWith('/tiles/')).every((r) => r.headers.referer === undefined), J(log.filter((r) => r.url.startsWith('/tiles/') && r.headers.referer).length));
    let s = await shown(page);
    check('B: the picture is one image named by its label', s.name === 'Places near the centre' && s.hiddenSvg === 'true', J(s.name));
    check('B: nine tiles, each with an empty alt and no referrer policy of its own', s.imgs.length === 9 && s.imgs.every((i) => i.alt === '' && i.policy === 'no-referrer'), J(s.imgs.slice(0, 2)));
    check('B: the list is the one replay reports, nearest first', J(s.list) === J(listOf(SOL)), J(s.list));
    check('B: the credit is under the map', s.credit.startsWith('Stand-in tiles'), s.credit);
    check(`B: ${SOL.pinned} pins are drawn, and the centre is marked`, s.pins === SOL.pinned + 1, String(s.pins));
    const role = await page.evaluate(() => { const f = document.querySelector('figure.cui-map'); return { figure: f.tagName, imgRole: (f.querySelector('[role=img]') || {}).tagName, tabbable: [...f.querySelectorAll('a, button, input, [tabindex]')].length }; });
    check('B: the map has no tab stop of its own', role.tabbable === 0, J(role));
    check('B: no overflow at 412 px', await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth), String(await page.evaluate(() => document.documentElement.scrollWidth)));
    // a new centre: its nine tiles
    log.length = 0;
    await page.click('text=Centre on Barajas');
    await until(async () => (await shown(page)).list[0] === listOf(AIRPORT)[0]);
    await sleep(300);
    s = await shown(page);
    check('B: another centre asks for the nine tiles around it', J(tileReqs().sort()) === J(pathsOf(AIRPORT.tiles, '/tiles/').sort()), J(tileReqs()));
    check('B: and the list changes with it', J(s.list) === J(listOf(AIRPORT)), J(s.list));
    log.length = 0;
    await page.click('text=Centre on Barajas');
    await sleep(300);
    check('B: a render that changes nothing about the map asks for no tile', tileReqs().length === 0, J(tileReqs()));
    // the keyboard: two buttons and then out of the page
    await page.evaluate(() => document.activeElement && document.activeElement.blur());
    const stops = [];
    for (let i = 0; i < 4; i++) { await page.keyboard.press('Tab'); stops.push(await page.evaluate(() => (document.activeElement && (document.activeElement.getAttribute('aria-label') || document.activeElement.tagName)) || '')); }
    const labelled = stops.filter((x) => x !== 'BODY' && x !== 'HTML' && x !== '');
    check('B: the keyboard visits the two buttons, in order, and the map adds no stop', J(labelled) === J(['Centre on Puerta del Sol', 'Centre on Barajas']), J(stops));
    check('B: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
    server.close();
  }

  // ---- C. the tiles on another host, declared ----------------------------------------------------------------------------------------------------
  {
    const tileLog = [];
    const tileServer = await tiles.serve(0, (req) => tileLog.push(req));
    const otherLog = [];
    const other = await tiles.serve(0, (req) => otherLog.push(req));
    const tilePort = tileServer.address().port;
    const src = SOURCE.replace('requires Network;', `requires Network("localhost:${tilePort}");`).replace('"./tiles/{z}/{x}/{y}.png"', `"http://localhost:${tilePort}/{z}/{x}/{y}.png"`);
    const dir = build(src, root, 'cross', false);
    const log = [];
    const server = await serve(dir, log);
    const host = `127.0.0.1:${server.address().port}`;
    const tileHost = `localhost:${tilePort}`;
    const otherHost = `localhost:${other.address().port}`;
    const html = fs.readFileSync(path.join(dir, 'index.html'), 'utf8');
    check('C: the page carries a Content-Security-Policy that names the declared tile host', html.includes(`<meta http-equiv="Content-Security-Policy" content="img-src 'self' data: http://${tileHost}">`), (html.match(/<meta http-equiv[^>]*>/) || [''])[0]);
    check('C: and it is the first thing after the charset', html.indexOf('Content-Security-Policy') < html.indexOf('<style>') && html.indexOf('Content-Security-Policy') < html.indexOf('<title>'));
    const { ctx, page, errors } = await open(`http://${host}/`, [host, tileHost, otherHost]);
    await sleep(300);
    check('C: the tile host saw the nine requests of the plan', J(tileLog.map((r) => r.url).sort()) === J(pathsOf(SOL.tiles, '/').sort()), J(tileLog.map((r) => r.url)));
    check('C: with no Referer, and no cookie', tileLog.every((r) => r.headers.referer === undefined && r.headers.cookie === undefined));
    check('C: the page shows the nine tiles of the declared host', J((await shown(page)).imgs.map((i) => i.src).sort()) === J(SOL.tiles.map((u) => u.replace('./tiles/', `http://${tileHost}/`)).sort()), J((await shown(page)).imgs.length));
    // an image from a host the program did not declare is not requested at all
    const blocked = await page.evaluate((u) => new Promise((ok) => {
      window.violations = [];
      document.addEventListener('securitypolicyviolation', (e) => window.violations.push(e.violatedDirective));
      const i = new Image();
      i.onload = () => ok({ loaded: true });
      i.onerror = () => ok({ loaded: false, violations: window.violations.length });
      i.src = u;
      document.body.appendChild(i);
    }), `http://${otherHost}/1/0/0.png`);
    await sleep(150);
    check('C: an image from a host the program did not declare is blocked by the browser', blocked.loaded === false && otherLog.length === 0, J({ blocked, seenByOther: otherLog.length }));
    check('C: and the page hears the violation', blocked.violations === 1, J(blocked));
    const declared = await page.evaluate((u) => new Promise((ok) => { const i = new Image(); i.onload = () => ok(true); i.onerror = () => ok(false); i.src = u; document.body.appendChild(i); }), `http://${tileHost}/2/0/0.png`);
    check('C: an image from the declared host still loads', declared === true);
    check('C: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();

    // ---- D. the tile host is gone ------------------------------------------------------------------------------------------------------------------
    const gone = await open(`http://${host}/`, [host, tileHost, otherHost]);
    await gone.ctx.close();
    await new Promise((ok) => tileServer.close(ok));
    tileServer.closeAllConnections();
    const off = await open(`http://${host}/`, [host, tileHost, otherHost]);
    await until(async () => (await shown(off.page)).note !== '');
    const s = await shown(off.page);
    check('D: with the tile host gone the picture says it is not available', s.note === 'The map is not available offline.', J(s.note));
    check('D: and the list and the credit are still there', J(s.list) === J(listOf(SOL)) && s.credit.length > 0, J(s.list));
    check('D: no script error', off.errors.length === 0, off.errors.join(' | '));
    await off.ctx.close();
    server.close();
    other.close();

    // one tile missing does not say the map is gone
    const half = [];
    const halfServer = await tiles.serve(0, (req) => half.push(req.url));
    const halfPort = halfServer.address().port;
    halfServer.removeAllListeners('request');
    const { tilePng } = require('./tile-png.js');
    halfServer.on('request', (req, res) => {
      const m = /^\/(\d+)\/(\d+)\/(\d+)\.png/.exec(req.url);
      if (m && m[3] === String(SOL.tiles[0].split('/').pop().replace('.png', ''))) { res.writeHead(404); res.end(); return; }
      const png = tilePng(Number(m[2]), Number(m[3]), Number(m[1]));
      res.writeHead(200, { 'content-type': 'image/png', 'content-length': png.length });
      res.end(png);
    });
    const dir2 = build(SOURCE.replace('requires Network;', `requires Network("localhost:${halfPort}");`).replace('"./tiles/{z}/{x}/{y}.png"', `"http://localhost:${halfPort}/{z}/{x}/{y}.png"`), root, 'half', false);
    const server2 = await serve(dir2, []);
    const host2 = `127.0.0.1:${server2.address().port}`;
    const p2 = await open(`http://${host2}/`, [host2, `localhost:${halfPort}`]);
    await sleep(500);
    const s2 = await shown(p2.page);
    check('D: a tile that is missing leaves the map and says nothing', s2.note === '' && s2.imgs.length === 9, J(s2.note));
    await p2.ctx.close();
    server2.close();
    halfServer.close();
  }

  // ---- E. a coarse position is not drawn finer than 1 km ------------------------------------------------------------------------------------------
  {
    const src = SOURCE.replace('requires Network;', 'requires Network;\n    requires Location;\n    state found: Result[String, String] = Ok("");\n    on Find { found = await(location.current()); }')
      .replace('state here: String = "40.41694,-3.70351";', 'state here: String = "80,10";');
    const dir = build(src, root, 'coarse', true);
    const log = [];
    const server = await serve(dir, log);
    const host = `127.0.0.1:${server.address().port}`;
    const { ctx, page } = await open(`http://${host}/`, [host]);
    await sleep(300);
    const asked = log.filter((r) => r.url.startsWith('/tiles/')).map((r) => r.url);
    check('E: at latitude 80 a coarse map asks for zoom 12, not the 13 written', asked.length > 0 && asked.every((u) => u.startsWith('/tiles/12/')), J(asked.slice(0, 2)));
    await ctx.close();
    server.close();
  }

  check('no request left this machine (the check refused any host that is not one of its own)', stray.length === 0, J(stray));
  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
