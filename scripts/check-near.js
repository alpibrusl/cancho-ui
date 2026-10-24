#!/usr/bin/env node
// `distance(a, b, default)` and the "places near me" page (docs/maps.md section 3.1) in Chromium:
//  A. __cui_distance in the page against tests/distance_corpus.json, the file `cargo test` runs on replay's (to 1e-9 km);
//  B. examples/near with the geolocation permission GRANTED: the five nearest places, in the order and with the distances the example's own
//     trace (checked by `cancho-ui test`) expects, for two positions;
//  C. the permission DENIED: the answer is "permission denied" and no list;
//  D. no Geolocation API at all: "unavailable" and no list; a position with an error code 2 and 3.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. The position is emulated: nothing
// here says what a phone's prompt or accuracy do.
//   node scripts/check-near.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const f = path.join(dir, path.normalize(rel));
    if (!f.startsWith(dir) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}
const shown = (page) => page.evaluate(() => [...document.querySelectorAll('#root *')].filter((e) => e.children.length === 0).map((e) => e.textContent).filter((t) => t && t !== 'Find places near me'));

// the lists the trace expects after each position, read from the trace (not copied here)
const trace = JSON.parse(fs.readFileSync(path.join(ROOT, 'examples', 'near', 'near.trace.json'), 'utf8'));
const expected = trace.steps.filter((s) => s.state && s.state.nearest && s.state.nearest.length).map((s) => ({ pos: s.state.pos, list: s.state.nearest }));

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-near-'));
  const r = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples', 'near', 'near.cui'), '--page', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build: ${r.stdout}${r.stderr}`); }
  const server = await serve(dir);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // ---- A. the function, against the corpus ----------------------------------------------------------------------------------------------
  {
    const js = fs.readFileSync(path.join(dir, 'app.js'), 'utf8');
    const a = js.indexOf('function __cui_position(');
    const e = js.indexOf('\n}\n', js.indexOf('function __cui_distance(')) + 3;
    check('A: the module carries distance', a > 0 && e > a);
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'distance_corpus.json'), 'utf8'));
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    const bad = await page.evaluate(({ src, cases }) => {
      const d = new Function(`${src}; return __cui_distance;`)();
      return cases.filter((c) => { const got = d(c.a, c.b, c.default); return c.want === 'default' ? !Object.is(got, c.default) : !(Math.abs(got - c.want) <= 1e-9); }).map((c) => c.why);
    }, { src: js.slice(a, e), cases: corpus.cases });
    check(`A: ${corpus.cases.length} cases of the corpus come out as written`, bad.length === 0, bad.join('; '));
    await ctx.close();
  }

  const open = async (options, init) => {
    const ctx = await browser.newContext(options);
    if (init) { await ctx.addInitScript(init); }
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('button');
    return { ctx, page, errors };
  };
  // wait for the answer the case expects (a condition, not a sleep): `done` and the five rows, or an `error: ..` line
  const press = async (page, rows = 0) => {
    await page.click('button');
    await page.waitForFunction((n) => { const t = [...document.querySelectorAll('#root *')].filter((e) => e.children.length === 0).map((e) => e.textContent); return (n ? t.includes('done') : t.some((x) => x.startsWith('error:'))) && t.length >= n + 2; }, rows, { timeout: 15000 });
  };

  // ---- B. granted -----------------------------------------------------------------------------------------------------------------------
  check('B: the trace expects two lists to compare with', expected.length === 2, String(expected.length));
  for (const want of expected) {
    const [lat, lon] = want.pos.split(',').map(Number);
    const { ctx, page, errors } = await open({ geolocation: { latitude: lat, longitude: lon }, permissions: ['geolocation'] });
    await press(page, 5);
    const got = await shown(page);
    check(`B: at ${want.pos} the page lists ${JSON.stringify(want.list[0])} first, and the five as the trace expects`, JSON.stringify(got.filter((t) => t !== 'done')) === JSON.stringify(want.list), JSON.stringify(got));
    check('B: and says done', got.includes('done'), JSON.stringify(got));
    check('B: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- C. denied ------------------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open({ geolocation: { latitude: 40.4, longitude: -3.7 }, permissions: [] });
    await press(page);
    const got = await shown(page);
    check('C: without the permission the answer is "permission denied" and there is no list', JSON.stringify(got) === JSON.stringify(['error: permission denied']), JSON.stringify(got));
    check('C: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- D. no API, and the other errors --------------------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open({}, () => { Object.defineProperty(navigator, 'geolocation', { value: undefined, configurable: true }); });
    await press(page);
    const got = await shown(page);
    check('D: with no Geolocation API the answer is "unavailable" and there is no list', JSON.stringify(got) === JSON.stringify(['error: unavailable']), JSON.stringify(got));
    check('D: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }
  for (const [code, reason] of [[2, 'position unavailable'], [3, 'timeout']]) {
    const { ctx, page } = await open({}, `Object.defineProperty(navigator, 'geolocation', { value: { getCurrentPosition: (ok, err) => err({ code: ${code} }) }, configurable: true });`);
    await press(page);
    const got = await shown(page);
    check(`D: an error code ${code} is "${reason}" and there is no list`, JSON.stringify(got) === JSON.stringify([`error: ${reason}`]), JSON.stringify(got));
    await ctx.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
