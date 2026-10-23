#!/usr/bin/env node
// A stand-in service for examples/stream (docs/streams.md): the page's own origin serves the built page and one stream. A test double,
// not a model of any real service.
//   GET /api/readings   a Server-Sent Events stream: `id: N` and `data: <reading>` every `behaviour.interval` ms (default 1000; 0 = only
//                       when `server.tick()` is called). The server keeps the last 100 readings; a request with `Last-Event-ID: N` is
//                       sent the readings after N first, so a page that reconnects loses nothing it can still be told about.
// `behaviour.down` destroys the connection of every /api call; `server.dropAll()` ends every open stream; `server.log.connections` has
// the `Last-Event-ID` and `Authorization` of each request.
//   node scripts/stream-server.js <built-page-dir> [port]
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

// reading N is 18 to 26, from N alone, so that a test can compute what it should see
function reading(n) { return 18 + ((n * 7) % 9); }

function createServer(dir, behaviour = {}, port = 0) {
  const log = { connections: [] };
  const history = [];
  const clients = new Set();
  let seq = 0;
  const send = (res, n) => res.write(`id: ${n}\ndata: ${reading(n)}\n\n`);
  const tick = () => {
    seq += 1;
    history.push(seq);
    if (history.length > 100) { history.shift(); }
    for (const res of clients) { send(res, seq); }
  };
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url.startsWith('/api/')) {
      if (behaviour.down) { req.socket.destroy(); return; }
      if (url === '/api/readings') {
        const last = req.headers['last-event-id'];
        log.connections.push({ lastEventId: last === undefined ? null : last, authorization: req.headers.authorization || null });
        res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store' });
        res.flushHeaders();
        if (last !== undefined && /^\d+$/.test(last)) {
          for (const n of history) { if (n > Number(last)) { send(res, n); } }
        }
        clients.add(res);
        res.on('close', () => clients.delete(res));
        return;
      }
      res.writeHead(404, { 'content-type': 'text/plain' });
      res.end('no such route');
      return;
    }
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  const interval = behaviour.interval === undefined ? 1000 : behaviour.interval;
  let timer = null;
  if (interval > 0) { timer = setInterval(tick, interval); }
  server.log = log;
  server.tick = tick;
  server.openStreams = () => clients.size;
  server.dropAll = () => { for (const res of [...clients]) { res.end(); } };
  server.on('close', () => { if (timer) { clearInterval(timer); } });
  return new Promise((ok) => server.listen(port, '127.0.0.1', () => ok(server)));
}

module.exports = { createServer, reading };

if (require.main === module) {
  const dir = path.resolve(process.argv[2] || '.');
  createServer(dir, {}, Number(process.argv[3] || 8080)).then((s) => console.log(`serving ${dir} and /api/readings on http://127.0.0.1:${s.address().port}/`));
}
