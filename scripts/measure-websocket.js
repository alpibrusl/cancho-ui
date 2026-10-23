#!/usr/bin/env node
// What docs/streams.md section 8.3 says about WebSocket, measured in Chromium (Pixel 7 profile) against a minimal WebSocket server written
// here (the handshake and unmasked server frames of RFC 6455; no dependency). Not a check of the compiler: nothing in cancho-ui speaks
// WebSocket. It exists so that the sentences of the note that mention it are numbers, not memory. Outside the gate.
//   node scripts/measure-websocket.js
// The questions: (1) can the page choose a header of the handshake; (2) what does the handshake carry on its own; (3) do binary frames
// arrive, and as what; (4) can the page send; (5) does the browser reconnect after the server closes; (6) what does an installed service
// worker see of a WebSocket; (7) what does the page see when the server is not there.
'use strict';
const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { chromium, devices } = require('playwright');

const out = [];
function say(k, v) { out.push([k, v]); console.log(`${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`); }
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

// one server frame (FIN, no mask), payload under 65,536 bytes
function frame(opcode, payload) {
  const n = payload.length;
  const head = n < 126 ? Buffer.from([0x80 | opcode, n]) : Buffer.from([0x80 | opcode, 126, n >> 8, n & 255]);
  return Buffer.concat([head, payload]);
}
// the payload of one masked client frame, and its opcode
function readFrame(buf) {
  const op = buf[0] & 15;
  let n = buf[1] & 127;
  let at = 2;
  if (n === 126) { n = buf.readUInt16BE(2); at = 4; }
  const mask = buf.subarray(at, at + 4);
  const body = Buffer.from(buf.subarray(at + 4, at + 4 + n));
  for (let i = 0; i < n; i++) { body[i] ^= mask[i % 4]; }
  return { op, body };
}

(async () => {
  const log = { upgrades: [], received: [], plainRequests: [], upgradeCount: 0 };
  let onSocket = () => { };
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'measure-ws-'));
  fs.writeFileSync(path.join(dir, 'sw.js'), "self.addEventListener('fetch', (e) => { self.clients.matchAll().then((cs) => cs.forEach((c) => c.postMessage({ fetch: e.request.url }))); });\nself.addEventListener('install', () => self.skipWaiting());\nself.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));\n");
  fs.writeFileSync(path.join(dir, 'index.html'), '<!doctype html><meta charset="utf-8"><title>ws</title><body>ok</body>');
  const server = http.createServer((req, res) => {
    log.plainRequests.push(req.url);
    const f = req.url === '/sw.js' ? 'sw.js' : 'index.html';
    res.writeHead(200, { 'content-type': f === 'sw.js' ? 'text/javascript' : 'text/html', 'cache-control': 'no-store', connection: 'close' });
    res.end(fs.readFileSync(path.join(dir, f)));
  });
  server.on('upgrade', (req, socket) => {
    log.upgradeCount += 1;
    log.upgrades.push({ url: req.url, headers: req.headers });
    const accept = crypto.createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    const protocol = (req.headers['sec-websocket-protocol'] || '').split(',')[0].trim();
    socket.write(['HTTP/1.1 101 Switching Protocols', 'Upgrade: websocket', 'Connection: Upgrade', `Sec-WebSocket-Accept: ${accept}`, protocol ? `Sec-WebSocket-Protocol: ${protocol}` : ''].filter(Boolean).join('\r\n') + '\r\n\r\n');
    socket.on('data', (d) => { if (req.url !== '/live') { return; } let at = 0; while (at < d.length) { try { const f = readFrame(d.subarray(at)); log.received.push(f); at += 2 + 4 + f.body.length + (f.body.length >= 126 ? 2 : 0); } catch (e) { break; } } });
    onSocket(socket, req.url);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const origin = `http://localhost:${port}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();
  await page.goto(`${origin}/`);

  // (1) the constructor: what it takes
  say('WebSocket.length (the arguments the constructor requires)', await page.evaluate(() => WebSocket.length));
  say('WebSocket.prototype own properties', await page.evaluate(() => Object.getOwnPropertyNames(WebSocket.prototype).sort()));
  // (2) what the handshake carries when the page only gives a URL and a protocol
  let sock = null;
  onSocket = (s, url) => { if (url === '/live') { sock = s; } };
  await page.evaluate((u) => {
    window.events = [];
    window.ws = new WebSocket(u, ['chat.v1']);
    window.ws.onopen = () => window.events.push('open');
    window.ws.onmessage = (e) => window.events.push(['message', typeof e.data === 'string' ? 'string' : e.data.constructor.name, typeof e.data === 'string' ? e.data : (e.data.size || e.data.byteLength)]);
    window.ws.onclose = (e) => window.events.push(['close', e.code, e.wasClean]);
    window.ws.onerror = () => window.events.push('error');
  }, `ws://localhost:${port}/live`);
  for (let i = 0; i < 100 && !sock; i++) { await sleep(20); }
  const h = log.upgrades[0] ? log.upgrades[0].headers : {};
  say('handshake headers the page did not choose', Object.keys(h).sort());
  say('handshake carries Authorization', 'authorization' in h);
  say('handshake Origin', h.origin);
  say('subprotocol the page asked for, as the server saw it', h['sec-websocket-protocol']);
  // a header through the API: there is none; the URL's userinfo is the only thing that looks like one
  const userinfo = await page.evaluate((u) => { try { const w = new WebSocket(u); w.onerror = () => { }; return 'constructed'; } catch (e) { return `${e.name}: ${e.message}`; } }, `ws://user:secret@localhost:${port}/auth`);
  await sleep(300);
  const authUpgrade = log.upgrades.find((u) => u.url === '/auth');
  say('a URL with user:secret@', { constructor: userinfo, authorizationHeaderSent: authUpgrade ? authUpgrade.headers.authorization || null : 'no handshake' });

  // (3) text and binary frames
  sock.write(frame(1, Buffer.from('hello')));
  sock.write(frame(2, Buffer.from([1, 2, 3, 4, 5, 6, 7, 8])));
  await sleep(200);
  say('events after a text frame of 5 bytes and a binary frame of 8 bytes', await page.evaluate(() => window.events));
  await page.evaluate(() => { window.ws.binaryType = 'arraybuffer'; });
  sock.write(frame(2, Buffer.from([9, 9, 9])));
  await sleep(150);
  say('binaryType "arraybuffer": the next binary frame arrives as', (await page.evaluate(() => window.events)).slice(-1)[0]);
  // (4) the page sends
  await page.evaluate(() => { window.ws.send('from the page'); window.ws.send(new Uint8Array([7, 7])); });
  await sleep(200);
  say('the server received (opcode, bytes)', log.received.map((f) => [f.op, f.body.length <= 16 ? f.body.toString('latin1') : f.body.length]));

  // (5) the server closes: does the browser connect again?
  const before = log.upgradeCount;
  sock.end(frame(8, Buffer.from([0x03, 0xe8])));
  await sleep(5000);
  say('events after the server closed', (await page.evaluate(() => window.events)).slice(-2));
  say('handshakes in the 5 s after the close, beyond the ones the page asked for', log.upgradeCount - before);

  // (6) a service worker: does its fetch event see a WebSocket?
  await page.evaluate(async () => { await navigator.serviceWorker.register('/sw.js'); await navigator.serviceWorker.ready; });
  await page.reload();
  await page.evaluate(() => { window.sw = []; navigator.serviceWorker.addEventListener('message', (e) => window.sw.push(e.data.fetch)); });
  await page.evaluate((u) => { window.ws2 = new WebSocket(u); }, `ws://localhost:${port}/through-sw`);
  await sleep(500);
  await page.evaluate(() => fetch('/probe').catch(() => 0));
  await sleep(300);
  const seen = await page.evaluate(() => window.sw);
  say('requests the worker saw (a fetch to /probe was made on purpose)', seen);
  say('the worker saw the WebSocket', seen.some((u) => u.includes('through-sw')));

  // (7) the server is not there
  const dead = http.createServer();
  await new Promise((ok) => dead.listen(0, '127.0.0.1', ok));
  const deadPort = dead.address().port;
  await new Promise((ok) => dead.close(ok));
  const t0 = Date.now();
  const result = await page.evaluate((u) => new Promise((ok) => { const w = new WebSocket(u); const ev = []; w.onerror = () => ev.push('error'); w.onclose = (e) => { ev.push(['close', e.code, e.wasClean]); ok(ev); }; }), `ws://localhost:${deadPort}/`);
  say('no server: events', result);
  say('no server: ms until the page knows', Date.now() - t0);

  await browser.close();
  server.closeAllConnections();
  server.close();
  fs.writeFileSync(path.join(os.tmpdir(), 'measure-websocket.json'), JSON.stringify(out, null, 1));
})().catch((e) => { console.error(e); process.exit(1); });
