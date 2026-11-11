#!/usr/bin/env node
// `RouteMap` (docs/routes.md) against the page's own functions, in Node (no browser, no `playwright`):
//  A. tests/routemap_corpus.json, the file `cargo test` replays (crates/cancho-ui/tests/routemap.rs): for each case the page's
//     `__cui_routemap_describe` gives the same counts, route length, width, accessible list and sentence as replay, to the expected values the corpus holds
//     (computed by scripts/gen-routemap-corpus.py, which shares no code with either);
//  B. the geometry of `__cui_routemap_layout` against an ORACLE written again below from docs/routes.md section 4: fitted to the padded box, centred,
//     longitude scaled by the cosine of the mean latitude, a single point in the middle, every point inside the box.
// The DOM the page builds from them (the SVG, its name, the pins, the collapsed list) is looked at in a browser: docs/routes.md section 8.
//   node scripts/check-routemap.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  if (!ok) { console.log(`FAIL ${label}${detail ? `: ${detail}` : ''}`); failures.push(label); }
  return ok;
}

// build a page of a program with a RouteMap and cut the page's functions out of the module
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'routemap-'));
const src = path.join(tmp, 'r.cui');
fs.writeFileSync(src, 'component T {\n    state rows: List[String] = [];\n    view { Column { RouteMap { label: "T" points: rows } } }\n}\n');
const out = path.join(tmp, 'r.js');
const built = spawnSync(CANCHO_UI, ['build', src, '-o', out], { encoding: 'utf8' });
if (built.status !== 0) { console.error(built.stdout + built.stderr); process.exit(2); }
const js = fs.readFileSync(out, 'utf8');
function fn(name) {
  const at = js.indexOf(`function ${name}(`);
  if (at < 0) { throw new Error(`no ${name} in the built module`); }
  let depth = 0, i = js.indexOf('{', at);
  for (; i < js.length; i++) { if (js[i] === '{') { depth++; } else if (js[i] === '}') { depth--; if (depth === 0) { break; } } }
  return js.slice(at, i + 1);
}
const consts = js.split('\n').filter((l) => /^const __CUI_RM_/.test(l)).join('\n');
const page = new Function(`${consts}\n${['__cui_json_depth', '__cui_json_at', '__cui_json_get', '__cui_position', '__cui_distance', '__cui_rm_tenth', '__cui_rm_across', '__cui_rm_kind', '__cui_routemap_describe', '__cui_routemap_layout'].map(fn).join('\n')}\nreturn { describe: __cui_routemap_describe, layout: __cui_routemap_layout };`)();
const jget = new Function(`${fn('__cui_json_depth')}\n${fn('__cui_json_at')}\n${fn('__cui_json_get')}\nreturn __cui_json_get;`)();
const dist = new Function(`${fn('__cui_position')}\n${fn('__cui_distance')}\nreturn __cui_distance;`)();
const describe = (rows, route) => page.describe(rows, route, (r, k) => jget(r, k, ''), (a, b) => dist(a, b, -1));

const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'routemap_corpus.json'), 'utf8'));
let n = 0;
for (const k of corpus.cases) {
  const d = describe(k.rows, k.route);
  const w = k.want;
  n += 6;
  check(`${k.why}: summary`, d.summary === w.summary, `${d.summary} / ${w.summary}`);
  check(`${k.why}: counts`, d.count === w.count && d.total === w.total && d.located === w.located && d.onRoute === w.on_route, JSON.stringify([d.count, d.total, d.located, d.onRoute]));
  check(`${k.why}: route length`, Math.round(d.routeKm * 10) / 10 === w.km, `${d.routeKm} / ${w.km}`);
  check(`${k.why}: width`, (d.acrossKm === null ? null : d.acrossKm >= 100 ? Math.round(d.acrossKm / 10) * 10 : d.acrossKm >= 10 ? Math.round(d.acrossKm) : Math.round(d.acrossKm * 10) / 10) === w.across_km, `${d.acrossKm} / ${w.across_km}`);
  check(`${k.why}: list`, JSON.stringify(d.list.map((e) => e.text)) === JSON.stringify(w.list));
  // B. geometry
  const xy = page.layout(d);
  check(`${k.why}: one coordinate per located point`, xy.length === d.located);
  if (xy.length === 0) { continue; }
  const lat = d.pts.map((p) => p.lat), lon = d.pts.map((p) => p.lon);
  const kx = Math.cos((lat.reduce((a, b) => a + b, 0) / lat.length) * Math.PI / 180);
  const X = lon.map((v) => v * kx), Y = lat.map((v) => -v);
  const dx = Math.max(...X) - Math.min(...X), dy = Math.max(...Y) - Math.min(...Y);
  const scale = dx < 1e-9 && dy < 1e-9 ? 0 : Math.min(dx > 1e-9 ? 520 / dx : Infinity, dy > 1e-9 ? 320 / dy : Infinity);
  const cx = (Math.max(...X) + Math.min(...X)) / 2, cy = (Math.max(...Y) + Math.min(...Y)) / 2;
  let ok = true;
  xy.forEach((q, i) => {
    const ex = 300 + (X[i] - cx) * scale, ey = 200 + (Y[i] - cy) * scale;
    if (Math.abs(q[0] - ex) > 1e-6 || Math.abs(q[1] - ey) > 1e-6) { ok = false; }
    if (q[0] < 40 - 1e-6 || q[0] > 560 + 1e-6 || q[1] < 40 - 1e-6 || q[1] > 360 + 1e-6) { ok = false; }
  });
  n += 2;
  check(`${k.why}: layout matches the oracle and stays in the box`, ok);
  if (d.located === 1 || scale === 0) { check(`${k.why}: one place is in the middle`, xy.every((q) => q[0] === 300 && q[1] === 200)); n++; }
  else {
    const w2 = Math.max(...xy.map((q) => q[0])) - Math.min(...xy.map((q) => q[0]));
    const h2 = Math.max(...xy.map((q) => q[1])) - Math.min(...xy.map((q) => q[1]));
    check(`${k.why}: the longer side fills the box`, Math.abs(Math.max(w2 / 520, h2 / 320) - 1) < 1e-6, `${w2} x ${h2}`);
    n++;
  }
}
// the anchor: northern Spain to Barcelona by hand. mean lat 41.5, kx = cos(41.5 deg) = 0.7490; lon -3.7 .. 2.17 -> x spans 4.396, lat 40.42..41.39 -> y spans 0.97
const anchor = describe(['{"name":"a","pos":"40.4168,-3.7038"}', '{"name":"b","pos":"41.3874,2.1686"}'], true);
const axy = page.layout(anchor);
check('anchor: Madrid and Barcelona are 520 wide (the box is the width), Barcelona is right and above', Math.abs(axy[1][0] - axy[0][0] - 520) < 1e-6 && axy[1][1] < axy[0][1], JSON.stringify(axy));
n += 1;
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`${failures.length === 0 ? 'ok' : 'FAIL'}: ${n} checks, ${failures.length} failed`);
process.exit(failures.length === 0 ? 0 : 1);
