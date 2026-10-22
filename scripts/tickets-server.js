#!/usr/bin/env node
// A stand-in service for examples/tickets (docs/tickets.md): the page's own origin serves the built page and these routes. A test
// double, not a model of any real service. The session is a cookie (`sid`, HttpOnly, SameSite=Strict): the page never sees a token.
//   POST /api/login    [user, password]        200 and Set-Cookie, or 401        (the only user: ada / hunter22!)
//   POST /api/logout                           200 and the cookie is cleared
//   GET  /api/orders   (needs the cookie)      one JSON record per line: {"id","title","status","note"}; 401 without it
//   POST /api/order    [id, status, note]      200 saved; 400 unknown id, status not open/doing/done, or a note over 200 characters; 401
// `behaviour.down` destroys the connection of every /api call; `behaviour.slow` holds the next login until `release()`.
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

function orders() {
  return [
    { id: 'T-1', title: 'Replace the filter on Press 2', status: 'open', note: '' },
    { id: 'T-2', title: 'Calibrate the furnace probe', status: 'doing', note: 'probe ordered' },
    { id: 'T-3', title: 'Oil the conveyor', status: 'done', note: 'every month' },
    { id: 'T-4', title: 'Inspect the weld on Lathe 3', status: 'open', note: '' },
  ];
}

function createServer(dir, behaviour) {
  const data = orders();
  const sessions = new Set();
  const log = { logins: [], saves: [], gets: 0, orderRequests: 0 };
  const held = [];
  const cookieOf = (req) => (req.headers.cookie || '').split(';').map((c) => c.trim()).find((c) => c.startsWith('sid='));
  const signedIn = (req) => { const c = cookieOf(req); return !!c && sessions.has(c.slice(4)); };
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    const answer = (status, text, headers = {}) => { res.writeHead(status, { 'content-type': 'text/plain', 'cache-control': 'no-store', ...headers }); res.end(text); };
    if (url.startsWith('/api/')) {
      if (behaviour.down) { req.socket.destroy(); return; }
      let body = '';
      req.on('data', (c) => { body += c; });
      req.on('end', () => {
        if (url === '/api/login' && req.method === 'POST') {
          const go = () => {
            let p;
            try { p = JSON.parse(body); } catch (e) { answer(400, 'not json'); return; }
            log.logins.push(p);
            if (Array.isArray(p) && p[0] === 'ada' && p[1] === 'hunter22!') {
              const sid = 'S' + (sessions.size + 1) + Math.random().toString(16).slice(2, 8);
              sessions.add(sid);
              answer(200, 'welcome', { 'set-cookie': `sid=${sid}; HttpOnly; SameSite=Strict; Path=/` });
            } else { answer(401, 'wrong user name or password'); }
          };
          if (behaviour.slow) { behaviour.slow = false; held.push(go); return; }
          go();
          return;
        }
        if (url === '/api/logout' && req.method === 'POST') {
          const c = cookieOf(req);
          if (c) { sessions.delete(c.slice(4)); }
          answer(200, 'bye', { 'set-cookie': 'sid=; Max-Age=0; Path=/' });
          return;
        }
        if (url === '/api/orders') { log.orderRequests++; }
        if (!signedIn(req)) { answer(401, 'sign in first'); return; }
        if (url === '/api/orders' && req.method === 'GET') {
          log.gets++;
          answer(200, data.map((o) => JSON.stringify(o)).join('\n') + '\n');
          return;
        }
        if (url === '/api/order' && req.method === 'POST') {
          let p;
          try { p = JSON.parse(body); } catch (e) { answer(400, 'not json'); return; }
          log.saves.push(p);
          const o = Array.isArray(p) ? data.find((d) => d.id === p[0]) : null;
          if (!o) { answer(400, 'unknown order'); return; }
          if (!['open', 'doing', 'done'].includes(p[1])) { answer(400, 'status must be open, doing or done'); return; }
          if (typeof p[2] !== 'string' || p[2].length > 200) { answer(400, 'note too long'); return; }
          o.status = p[1];
          o.note = p[2];
          answer(200, 'saved');
          return;
        }
        answer(404, 'no such route');
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
  server.release = () => held.splice(0).forEach((go) => go());
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

module.exports = { createServer, orders };
