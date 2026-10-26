#!/usr/bin/env node
// What the map island of the port can reach (docs/www-port/README.md, docs/islands.md section 12): the 30 attempts of tests/fixtures/islands/reach.js are added, as one more script, to the
// REAL map island of docs/www-port/site/home.cui (a copy of the program in a scratch directory: the page's own `rocks.js` and `map.js` run first, then the battery), and run in
// Chromium against two local servers (the page's origin and "somewhere else") and a UDP socket standing for a STUN server. An open island is plain JavaScript in the page's origin:
// the check says so with numbers, and does not say it is anything else. It also runs the same battery in the hero's sealed frame? No: that is scripts/check-islands.js F, on a
// frame the compiler wrote; here the point is the one open island of the port. DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   node scripts/check-www-reach.js
'use strict';
const crypto = require('crypto');
const dgram = require('dgram');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const sha = (file) => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');

(async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'wwwreach-'));
  const site = path.join(work, 'site');
  fs.cpSync(path.join(ROOT, 'docs', 'www-port', 'site'), site, { recursive: true });
  fs.copyFileSync(path.join(ROOT, 'tests', 'fixtures', 'islands', 'reach.js'), path.join(site, 'islands', 'reach.js'));
  const hits = [];
  const pageHits = [];
  const other = http.createServer((q, r) => { hits.push(q.url); r.setHeader('content-type', 'text/javascript'); r.setHeader('access-control-allow-origin', '*'); r.end('window.__mod = 1;'); });
  const stun = dgram.createSocket('udp4');
  const stunHits = [];
  stun.on('message', (m) => stunHits.push(m.length));
  await new Promise((ok) => other.listen(0, '127.0.0.1', ok));
  await new Promise((ok) => stun.bind(0, '127.0.0.1', ok));
  let root = null;
  const srv = http.createServer((q, r) => {
    const u = q.url.split('?')[0];
    if (u.startsWith('/leak')) { pageHits.push(q.url); r.end('ok'); return; }
    let f = path.join(root, u === '/' ? 'index.html' : decodeURIComponent(u));
    fs.readFile(f, (e, d) => {
      if (e || !fs.statSync(f).isFile()) { r.statusCode = 404; r.end(); return; }
      r.setHeader('content-type', { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.json': 'application/json', '.md': 'text/markdown', '.woff2': 'font/woff2' }[path.extname(f)] || 'application/octet-stream');
      r.end(d);
    });
  });
  await new Promise((ok) => srv.listen(0, '127.0.0.1', ok));
  const A = `http://127.0.0.1:${srv.address().port}`;
  const B = `http://127.0.0.1:${other.address().port}`;

  // the same program with the battery as one more script of the map island, and the three props and the event it needs
  let src = fs.readFileSync(path.join(site, 'home.cui'), 'utf8');
  const swap = (from, to) => { if (src.split(from).length !== 2) { throw new Error(`not once: ${from.slice(0, 60)}`); } src = src.replace(from, to); };
  swap('    style: "islands/map.css"', `    script: "islands/reach.js" sha256: "${sha(path.join(site, 'islands', 'reach.js'))}";\n    style: "islands/map.css"`);
  swap('props { places: String; selected: String; kind: String; only_built: Bool; }', 'props { places: String; selected: String; kind: String; only_built: Bool; other: String; page: String; stun: Int; nav: Bool; }');
  swap('events { Selected: String; }', 'events { Selected: String; Result: String; }');
  swap('    state selected: String = "cancho";', '    state selected: String = "cancho";\n    state reach: String = "";');
  swap('    on Selected(id) { selected = id; }', '    on Selected(id) { selected = id; }\n    on Result(r) { reach = r; }');
  swap('only_built: only on: Selected text: "map-fallback" }', `only_built: only on: Selected on: Result other: "${B}" page: "${A}" stun: ${stun.address().port} nav: false text: "map-fallback" }`);
  swap('                    Rich { style: "note" "{len(filter(', '                    Text { role: status "{reach}" }\n                    Rich { style: "note" "{len(filter(');
  fs.writeFileSync(path.join(site, 'home.cui'), src);
  root = path.join(work, 'out');
  const b = spawnSync(BIN, ['build', path.join(site, 'home.cui'), '--page', '-o', root], { encoding: 'utf8', cwd: ROOT });
  check('the port builds with the battery as one more script of its map island', b.status === 0, (b.stderr + b.stdout).slice(0, 200));
  if (b.status !== 0) { process.exit(1); }

  const browser = await chromium.launch();
  const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 1000, height: 800 } });
  const page = await ctx.newPage();
  await page.goto(`${A}/`, { waitUntil: 'load' });
  let said = '';
  for (let i = 0; i < 160 && !said.includes('{'); i++) { await sleep(250); said = await page.evaluate(() => (document.querySelector('#ecosystem [role=status]') || { textContent: '' }).textContent); }
  await sleep(2000);
  let got = {};
  try { got = JSON.parse(said.slice(said.indexOf('{'))); } catch (e) { got = {}; }
  const names = Object.keys(got);
  console.log(`info the map island's answers to the ${names.length} attempts:`);
  for (const n of names) { console.log(`info   ${n}: ${got[n]}`); }
  console.log(`info another origin received ${hits.length} requests, the page's own origin ${pageHits.length}, the STUN socket ${stunHits.length} packets`);
  const worked = ['reached', 'sent', 'queued', 'answered', 'opened', 'registered', 'written', 'ran', 'constructed', '2', '1', 'tried'];
  const succeeded = names.filter((n) => worked.some((w) => got[n] === w || got[n].startsWith(`${w} `)));
  check('the battery ran in the real map island: it answered every attempt', names.length >= 30, String(names.length));
  check('it reached the page, its storage and its cookies (parent.document, localStorage, document.cookie, indexedDB)', ['read parent.document', 'localStorage', 'document.cookie', 'indexedDB'].every((k) => got[k] && got[k].startsWith('reached')), JSON.stringify(['read parent.document', 'localStorage', 'document.cookie', 'indexedDB'].map((k) => got[k])));
  check('it sent requests to another origin and to the page\'s own, and a UDP packet through WebRTC', hits.length >= 8 && pageHits.length >= 1 && stunHits.length >= 1, `${hits.length} ${pageHits.length} ${stunHits.length}`);
  check('it ran eval, a Function, an inline script it added, and a worker', got.eval === '2' && got['new Function'] === '1' && got['an inline <script> the island adds'] === 'ran' && got['Worker from a blob'] === 'ran', JSON.stringify([got.eval, got['new Function'], got['an inline <script> the island adds'], got['Worker from a blob']]));
  check('most of the attempts that a sealed frame refuses succeed here (at least 20 of them)', succeeded.length >= 20, `${succeeded.length} of ${names.length}: the rest: ${names.filter((n) => !succeeded.includes(n)).map((n) => `${n}=${got[n]}`).join('; ')}`);
  // and what the compiler says about it
  const eff = JSON.parse(spawnSync(BIN, ['inspect', 'effects', path.join(ROOT, 'docs', 'www-port', 'site', 'home.cui'), 'Home'], { encoding: 'utf8', cwd: ROOT }).stdout);
  const open = eff.islands.filter((i) => i.sealed === false);
  check('inspect effects names exactly one open island, the map, as UNBOUNDED, and `requires` has Dom', open.length === 1 && open[0].name === 'Places' && /UNBOUNDED/.test(open[0].authority) && eff.requires.some((r) => r.name === 'Dom'), JSON.stringify(open.map((o) => [o.name, o.authority])));
  check('and the other 6 declared islands (15 frames on the page) are sealed', eff.islands.filter((i) => i.sealed === true).length === 6, `${eff.islands.length} islands declared`);
  await browser.close();
  stun.close(); other.close(); srv.close();
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log('FAIL', e); process.exit(1); });
