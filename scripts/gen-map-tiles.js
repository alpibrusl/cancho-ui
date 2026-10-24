#!/usr/bin/env node
// Writes the stand-in tiles `examples/map` and its gallery demo use: zoom 13 around the two centres of the example (Puerta del Sol and
// Barajas), as the page would ask for them (`cancho-ui test examples/map/map.cui examples/map/map.trace.json` reports the addresses).
//   node scripts/gen-map-tiles.js
'use strict';
const fs = require('fs');
const path = require('path');
const { tilePng } = require('./tile-png.js');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, 'examples', 'map', 'tiles');
const z = 13;
// the tile of a position, by the formula of docs/maps.md (the same as the page's)
function tile(lat, lon) {
  const n = 2 ** z; const phi = lat * Math.PI / 180;
  return [Math.floor((lon + 180) / 360 * n), Math.floor((1 - Math.log(Math.tan(phi) + 1 / Math.cos(phi)) / Math.PI) / 2 * n)];
}
let count = 0;
for (const [lat, lon] of [[40.41694, -3.70351], [40.4983, -3.5676]]) {
  const [cx, cy] = tile(lat, lon);
  for (let dy = -1; dy <= 1; dy++) {
    for (let dx = -1; dx <= 1; dx++) {
      const file = path.join(OUT, String(z), String(cx + dx), `${cy + dy}.png`);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, tilePng(cx + dx, cy + dy, z));
      count++;
    }
  }
}
console.log(`${count} stand-in tiles written under ${path.relative(ROOT, OUT)}`);
