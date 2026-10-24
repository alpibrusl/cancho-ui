#!/usr/bin/env node
// A tile server for trying a map without a tile provider: every /{z}/{x}/{y}.png is a stand-in tile (scripts/tile-png.js), and every request
// is logged to the console. Nothing here talks to the internet.
//   node scripts/stand-in-tiles.js [port]      (default 8099)
'use strict';
const http = require('http');
const { tilePng } = require('./tile-png.js');

function serve(port, onRequest) {
  const server = http.createServer((req, res) => {
    if (onRequest) { onRequest(req); }
    const m = /^\/(\d+)\/(\d+)\/(\d+)\.png(\?.*)?$/.exec(req.url);
    if (!m) { res.writeHead(404); res.end(); return; }
    const png = tilePng(Number(m[2]), Number(m[3]), Number(m[1]));
    res.writeHead(200, { 'content-type': 'image/png', 'content-length': png.length, 'cache-control': 'max-age=3600' });
    res.end(png);
  });
  return new Promise((ok) => server.listen(port, '127.0.0.1', () => ok(server)));
}
module.exports = { serve };

if (require.main === module) {
  const port = Number(process.argv[2] || 8099);
  serve(port, (req) => console.log(`${new Date().toISOString()} ${req.url}`)).then(() => console.log(`stand-in tiles on http://localhost:${port}/{z}/{x}/{y}.png`));
}
