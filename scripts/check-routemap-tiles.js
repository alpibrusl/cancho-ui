#!/usr/bin/env node
// `RouteMap` with `tiles:` (docs/routes.md section 9) against the page's own functions, in Node (no browser, no network):
//  A. tests/routemap_tiles_corpus.json, which `cargo test` also replays against the replay's tile plan: for each case the page's `__cui_routemap_tile_plan`
//     gives the zoom, the grid, the exact tile addresses and the pixel of every point that scripts/gen-routemap-tiles-corpus.py computed (it shares no code
//     with either, and writes the Mercator ordinate another way);
//  B. properties of the plan that hold for every case, written again here: at most 4 x 4 tiles, zoom 18..1, every point inside the grid, one place is
//     3 x 3 at zoom 13 (clipped at the edge of the world), no address that is not the template's;
//  C. negative controls: no located point asks for nothing; the plan does not wrap the antimeridian; a polar latitude is clamped, not refused.
// The DOM the page builds from the plan (the images, the SVG over them, the credit) is looked at in a browser: docs/routes.md section 9.
//   node scripts/check-routemap-tiles.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
let n = 0;
function check(label, ok, detail) {
  n++;
  if (!ok) { console.log(`FAIL ${label}${detail ? `: ${detail}` : ''}`); failures.push(label); }
  return ok;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'routemap-tiles-'));
const src = path.join(tmp, 'r.cui');
fs.writeFileSync(src, 'component T {\n    requires Network("tiles.example");\n    state rows: List[String] = [];\n    view { Column { RouteMap { label: "T" points: rows tiles: "https://tiles.example/{z}/{x}/{y}.png" attribution: "credit" } } }\n}\n');
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
const consts = js.split('\n').filter((l) => /^const __CUI_RMT_/.test(l)).join('\n');
const plan = new Function(`${consts}\n${fn('__cui_rmt_world')}\n${fn('__cui_routemap_tile_plan')}\nreturn __cui_routemap_tile_plan;`)();
const pts = (list) => list.map((t) => { const [la, lo] = t.split(',').map(Number); return { lat: la, lon: lo }; });

const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'routemap_tiles_corpus.json'), 'utf8'));
for (const k of corpus.cases) {
  const p = plan(pts(k.points), corpus.template);
  const w = k.want;
  check(`${k.why}: zoom`, p.zoom === w.zoom, `${p.zoom} / ${w.zoom}`);
  check(`${k.why}: grid`, p.cols === w.grid[0] && p.rows === w.grid[1], `${p.cols}x${p.rows} / ${w.grid}`);
  check(`${k.why}: tiles`, JSON.stringify(p.tiles) === JSON.stringify(w.tiles), `${p.tiles.length} tiles`);
  check(`${k.why}: pixels`, p.px.length === w.px.length && p.px.every((q, i) => Math.abs(q[0] - w.px[i][0]) < 1e-6 && Math.abs(q[1] - w.px[i][1]) < 1e-6), JSON.stringify(p.px));
  // B. properties
  check(`${k.why}: at most 4 x 4 tiles`, p.cols <= 4 && p.rows <= 4 && p.tiles.length === p.cols * p.rows && p.tiles.length <= 16);
  check(`${k.why}: zoom within 1..18`, p.zoom >= 1 && p.zoom <= 18);
  const W = p.cols * 256, H = p.rows * 256;
  check(`${k.why}: every point is inside the grid`, p.px.every((q) => q[0] >= -1e-6 && q[0] <= W + 1e-6 && q[1] >= -1e-6 && q[1] <= H + 1e-6), JSON.stringify(p.px));
  check(`${k.why}: only the template's addresses`, p.tiles.every((t) => /^https:\/\/tiles\.example\/\d+\/\d+\/\d+\.png$/.test(t)));
  const same = k.points.every((t) => t === k.points[0]);
  if (same) { check(`${k.why}: one place is zoom 13`, p.zoom === 13); }
}
// C. negative controls and anchors
check('no located point asks for nothing', plan([], corpus.template) === null);
const one = plan(pts(['45.9237,6.8694']), corpus.template);
// Chamonix at zoom 13: x = (6.8694 + 180) / 360 * 8192 = 4252.3, tile 4252, y = 2916 (asinh form, by hand); the tile grid starts one tile to the left
check('anchor: one place is the middle tile of 3 x 3 at zoom 13', one.zoom === 13 && one.cols === 3 && one.rows === 3 && one.tx0 === 4251 && one.tiles[4] === 'https://tiles.example/13/4252/2916.png', JSON.stringify([one.tx0, one.tiles[4]]));
const wide = plan(pts(['-17.7134,178.0650', '-16.69,-179.8']), corpus.template);
check('the antimeridian is not wrapped: two places 2 degrees apart span the world', wide.zoom === 2 && wide.cols === 4, JSON.stringify([wide.zoom, wide.cols]));
const polar = plan(pts(['89.9,10']), corpus.template);
check('a polar latitude is clamped to the first row of tiles', polar.ty0 === 0 && polar.px[0][1] >= 0 && polar.px[0][1] < 1e-3 + 256, JSON.stringify(polar.px));
const south = plan(pts(['-90,0']), corpus.template);
check('the south pole is clamped to the last row', south.ty0 + south.rows === 8192 && south.px[0][1] <= south.rows * 256 + 1e-6);
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`${failures.length === 0 ? 'ok' : 'FAIL'}: ${n} checks, ${failures.length} failed`);
process.exit(failures.length === 0 ? 0 : 1);
