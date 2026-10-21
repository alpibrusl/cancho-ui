#!/usr/bin/env node
// A stand-in for the plant's machine service, for examples/ops-console (docs/ops-console.md): the page's own origin
// serves the built page and these two routes. It is a test double, not a model of any real service.
//   GET  /api/machines      one JSON record per line: {"id","name","state","temp","setpoint"}
//   POST /api/setpoint      a JSON array [id, setpoint]: 201 applied, 409 the machine is locked, 400 malformed or
//                           unknown id, 503 when `behaviour.busy` is set
// `behaviour` is how a test makes the service misbehave: `slow` holds the next GET until `release()`, `down` destroys
// the connection, `busy` answers a POST with 503, `limit` answers only the first n machines, `extra` appends raw lines
// (a malformed record, a name with markup), `crlf` ends lines with \r\n.
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

function machines() {
  const names = ['Press 1', 'Press 2', 'Press 3', 'Lathe 1', 'Lathe 2', 'Lathe 3', 'Furnace A', 'Furnace B', 'Furnace C',
    'Mill 1', 'Mill 2', 'Mill 3', 'Mill 4', 'Welder 1', 'Welder 2', 'Welder 3', 'Welder 4', 'Conveyor 1', 'Conveyor 2',
    'Conveyor 3', 'Pump 1', 'Pump 2', 'Pump 3'];
  const states = ['running', 'idle', 'running', 'fault', 'running'];
  return names.map((name, i) => ({
    id: `M-${101 + i}`, name, state: states[i % states.length],
    // distinct, not in id order, with a negative and a fractional value: text order would put "9.5" after "100.25"
    temp: [71.5, 64, 100.25, 9.5, 88, -4.5, 640, 655.5, 610, 55, 58.25, 61, 49.5, 301, 295.5, 310, 288, 33, 35.5, 31, 22, 24.5, 21][i],
    setpoint: 0,
  }));
}

function createServer(dir, behaviour) {
  const data = machines();
  const log = { gets: 0, posts: [], aborted: 0, held: [] };
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url === '/api/machines' && req.method === 'GET') {
      log.gets++;
      if (behaviour.down) { req.socket.destroy(); return; }
      const reply = () => {
        const eol = behaviour.crlf ? '\r\n' : '\n';
        const lines = data.slice(0, behaviour.limit || data.length).map((m) => JSON.stringify(m)).concat(behaviour.extra || []);
        res.writeHead(200, { 'content-type': 'text/plain', 'cache-control': 'no-store' });
        res.end(lines.join(eol) + eol);
      };
      if (behaviour.slow) {
        behaviour.slow = false;
        req.on('close', () => { if (!res.writableEnded) { log.aborted++; } });
        log.held.push(reply);
        return;
      }
      reply();
      return;
    }
    if (url === '/api/setpoint' && req.method === 'POST') {
      let body = '';
      req.on('data', (c) => { body += c; });
      req.on('end', () => {
        log.posts.push({ body, type: req.headers['content-type'] });
        const answer = (status, text) => { res.writeHead(status, { 'content-type': 'text/plain' }); res.end(text); };
        if (behaviour.busy) { answer(503, 'busy, try again'); return; }
        let parsed;
        try { parsed = JSON.parse(body); } catch (e) { answer(400, 'not json'); return; }
        const m = Array.isArray(parsed) ? data.find((d) => d.id === parsed[0]) : null;
        if (!m || typeof parsed[1] !== 'number') { answer(400, 'unknown machine or setpoint'); return; }
        if (m.id === 'M-107') { answer(409, 'locked by maintenance'); return; }
        m.setpoint = parsed[1];
        m.temp = parsed[1];
        answer(201, 'applied');
      });
      return;
    }
    const rel = url.replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  server.log = log;
  server.release = () => log.held.splice(0).forEach((reply) => reply());
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

module.exports = { createServer, machines };

if (require.main === module) {
  const dir = path.resolve(process.argv[2] || '.');
  createServer(dir, {}).then((s) => console.log(`http://localhost:${s.address().port}/`));
}
