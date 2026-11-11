#!/usr/bin/env node
// Writes the stand-in tiles `examples/route-tiles` and its gallery demo use: one picture for each tile address the example's trace expects
// (`cancho-ui test examples/route-tiles/route-tiles.cui examples/route-tiles/route-tiles.trace.json` checks the addresses against the page's own plan).
// Nothing here asks a tile provider for anything.
//   node scripts/gen-route-tiles.js
'use strict';
const fs = require('fs');
const path = require('path');
const { tilePng } = require('./tile-png.js');

const ROOT = path.join(__dirname, '..');
const DIR = path.join(ROOT, 'examples', 'route-tiles');
const trace = JSON.parse(fs.readFileSync(path.join(DIR, 'route-tiles.trace.json'), 'utf8'));
const addresses = new Set();
for (const step of trace.steps) {
  for (const m of (step.expect && step.expect.routemaps) || []) { for (const t of m.tiles || []) { addresses.add(t); } }
}
let count = 0;
for (const a of addresses) {
  const m = /^\.\/tiles\/(\d+)\/(\d+)\/(\d+)\.png$/.exec(a);
  if (!m) { throw new Error(`not an address of the example's folder: ${a}`); }
  const [z, x, y] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const file = path.join(DIR, 'tiles', String(z), String(x), `${y}.png`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, tilePng(x, y, z));
  count++;
}
console.log(`${count} stand-in tiles written under ${path.relative(ROOT, path.join(DIR, 'tiles'))}`);
