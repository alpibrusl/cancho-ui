#!/usr/bin/env node
// Streams (docs/streams.md, issue #78) in Chromium, in the mobile profile, against stand-in servers the script drives:
//  N. the event-stream parser on its own (the `__cui_sse` the compiler emits, run in node): the lines of tests/stream_corpus.json's `wire`
//     in every way of cutting them in two, and 4,000 random streams cut at random places, against a parser written again here from the
//     format (it reads the whole text, sharing no code with the page); a message at the bound, one over it, a line over its bound; the
//     cost of reading 1 MiB and 8 MiB of tiny lines (linear, not quadratic);
//  A. tests/stream_corpus.json `cases`, the file `cargo test` replays (crates/cancho-ui/tests/stream.rs): the same steps, a real server
//     for the trace's server, a fake clock for `advance`: the page ends each step where replay does, waits exactly the replay's backoff,
//     sends the id replay holds, and makes the requests replay says;
//  B. `wire`: the same bytes, whole and one at a time, through a real response, a BOM and invalid UTF-8 among them;
//  C. what the page will not read: a hostile line of 8 MiB, a message that is too long, 4M comment lines (the cost);
//  D. authority: the host check at run time, Authorization to the host the Credential names and to no other, a credential not supplied,
//     anonymous until the program sets it, and what a second origin must answer (CORS) measured;
//  E. a page that starts hidden, two messages while the first handler waits on a request, a server that answers 204;
//  F. examples/stream built with --page against scripts/stream-server.js: readings, a drop, the id sent back, a hidden page;
//  G. the same built with --pwa: the service worker passes the stream through, and a server that is stopped is a retry, then back.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-stream.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const vm = require('vm');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');
const { createServer: exampleServer, reading } = require('./stream-server.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const CORPUS = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'stream_corpus.json'), 'utf8'));
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const MESSAGE_MAX = 65536;
const LINE_MAX = MESSAGE_MAX + 6;

const failures = [];
// a check that waits for something that never comes (a spoiled page) must end, not hang: the whole run takes about 45 s
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
async function until(fn, ms = 2500) {
  for (let t = 0; t < ms; t += 20) {
    const v = await fn();
    if (v) { return v; }
    await sleep(20);
  }
  return null;
}

function build(src, out, flags) {
  const r = spawnSync(CANCHO_UI, ['build', src, ...flags, '-o', out], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${src}: ${r.stdout}${r.stderr}`); }
}

// ---- a server the script drives ------------------------------------------------------------------------------------------------------
// `/stream` requests are held until the script answers them; every request is logged. `opts.cors` answers preflights.
function scriptable(dir, opts = {}) {
  const S = { conns: [], reqs: [], slow: [], preflights: [], opts, hits: 0 };
  const server = http.createServer((req, res) => {
    const url = req.url.split('?')[0];
    S.hits += 1;
    if (opts.cors) {
      res.setHeader('access-control-allow-origin', '*');
      res.setHeader('access-control-allow-headers', opts.allowHeaders || 'authorization, last-event-id');
      if (req.method === 'OPTIONS') {
        S.preflights.push({ url, requested: req.headers['access-control-request-headers'] || '' });
        res.writeHead(204);
        res.end();
        return;
      }
    }
    if (url === '/stream') {
      const c = { req, res, state: 'pending', closed: false, headers: req.headers };
      S.conns.push(c);
      S.reqs.push({ url, headers: req.headers });
      res.on('close', () => { c.closed = true; });
      return;
    }
    if (url === '/slow') {
      S.slow.push({ req, res });
      return;
    }
    // what a redirect would lead to: a perfectly good stream (a page that followed redirects would be reading it)
    if (url === '/elsewhere') { res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', 'access-control-allow-origin': '*' }); res.write('data: redirected\n\n'); return; }
    if (!dir) { res.writeHead(404); res.end(); return; }
    const rel = decodeURIComponent(url).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    // `connection: close`: the stream's request then goes out on a socket of its own. Chromium sends a request again, unasked, when a
    // socket it reused is reset before any answer; a stand-in that resets the request of a stream must not be hidden by that.
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store', connection: 'close' });
    fs.createReadStream(file).pipe(res);
  });
  S.server = server;
  S.reset = () => { S.conns.length = 0; S.reqs.length = 0; S.slow.length = 0; S.preflights.length = 0; };
  S.pending = () => S.conns.filter((c) => c.state === 'pending' && !c.closed);
  S.current = () => [...S.conns].reverse().find((c) => c.state === 'open' && !c.closed) || null;
  S.latest = () => S.conns[S.conns.length - 1] || null;
  // `connection: close` here too: a stream that ended must not leave a socket for the next request to reuse (see above)
  S.accept = (c) => { c.res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'close' }); c.res.flushHeaders(); c.state = 'open'; };
  // a write to a connection the page has just closed may never call back: the write ends when the connection closes, or after 3 s
  S.write = (c, data) => new Promise((ok) => {
    if (c.closed) { ok(); return; }
    c.res.once('close', ok);
    c.res.write(data, ok);
    setTimeout(ok, 3000).unref();
  });
  S.close = () => new Promise((ok) => { server.closeAllConnections(); server.close(ok); });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => { S.port = server.address().port; ok(S); }));
}

async function fresh(browser, url, opts = {}) {
  const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
  const page = await ctx.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e}`));
  if (opts.init) { await page.addInitScript(opts.init); }
  if (opts.clock !== false) {
    await page.clock.install({ time: 0 });
    // The clock keeps running in real time after install and `pauseAt` cannot go back, so a time read from the page and paused at a little
    // ahead (`now + 1`, or a retry of `now + 100 * k`) is a race (the same cause as check-schedules.js, #74). Pause at a time the clock cannot
    // have reached, before the page exists, so that only `runFor` moves time.
    await page.clock.pauseAt(60000);
  }
  await page.goto(url);
  await page.waitForSelector('#root *');
  return { ctx, page, problems };
}
// the text of the page's Text node
const readText = (page) => page.evaluate(() => {
  const el = [...document.querySelectorAll('#root *')].find((e) => e.children.length === 0 && /^link=/.test(e.textContent));
  return el ? el.textContent : null;
});
function parseText(t) {
  const m = t && t.match(/^link=(\S*) log=([\s\S]*)$/);
  return m ? { link: m[1], log: m[2] } : null;
}
const setVisibility = (page, state) => page.evaluate((st) => {
  Object.defineProperty(document, 'visibilityState', { value: st, configurable: true });
  document.dispatchEvent(new Event('visibilitychange'));
}, state);

const textOf = (v) => (typeof v === 'string' ? v : v.map(([t, n]) => t.repeat(n)).join(''));
const sseOf = (text, id) => {
  const lines = textOf(text).split('\n').map((l) => `data: ${l}\n`);
  return (id === undefined ? '' : `id:${id === '' ? '' : ` ${id}`}\n`) + lines.join('') + '\n';
};

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-stream-'));

  // The corpus program, as a page (auto-mounted: the last component, `Page`, holds a `Feed` it can remove).
  const corpusSrc = path.join(tmp, 'corpus.cui');
  fs.writeFileSync(corpusSrc, CORPUS.program.join('\n') + '\n');
  const corpusDir = path.join(tmp, 'corpus');
  build(corpusSrc, corpusDir, ['--page']);
  const appJs = fs.readFileSync(path.join(corpusDir, 'app.js'), 'utf8');

  // ---- N. the parser alone -------------------------------------------------------------------------------------------------------------
  {
    const a = appJs.indexOf('function __cui_sse(');
    const b = appJs.indexOf('// cfg:');
    check('N: the emitted module carries the parser', a > 0 && b > a);
    const makeParser = new Function(`${appJs.slice(a, b)}; return __cui_sse;`)();
    // what the page's callbacks see, as a list
    const run = (chunks) => {
      const out = { messages: [], last: null, oversize: false };
      const p = makeParser({
        id: (v) => { out.last = v === '' ? null : v; },
        message: (t) => out.messages.push(t),
        oversize: () => { out.oversize = true; },
      });
      for (const c of chunks) { p.feed(c); }
      return out;
    };
    // the format, read again: the whole text at once, no state between chunks
    const oracle = (text) => {
      const lines = text.split(/\r\n|\r|\n/);
      lines.pop();
      const out = { messages: [], last: null };
      let data = '';
      let sawData = false;
      let idBuf = null;
      for (const l of lines) {
        if (l === '') {
          if (idBuf !== null) { out.last = idBuf === '' ? null : idBuf; }
          if (sawData) { out.messages.push(data); }
          data = ''; sawData = false;
          continue;
        }
        if (l[0] === ':') { continue; }
        const i = l.indexOf(':');
        const field = i < 0 ? l : l.slice(0, i);
        let value = i < 0 ? '' : l.slice(i + 1);
        if (value[0] === ' ') { value = value.slice(1); }
        if (field === 'data') { data = sawData ? `${data}\n${value}` : value; sawData = true; }
        else if (field === 'id' && !value.includes('\0')) { idBuf = value; }
      }
      return out;
    };
    let bad = 0;
    for (const w of CORPUS.wire) {
      const raw = w.raw.replace(/^﻿/, '');
      const want = { messages: w.messages, last: w.last_id };
      const same = (got) => JSON.stringify({ messages: got.messages, last: got.last }) === JSON.stringify(want);
      if (!same(run([raw]))) { bad++; console.log(`     whole: ${w.why}: ${JSON.stringify(run([raw]))}`); }
      for (let i = 1; i < raw.length; i++) {
        if (!same(run([raw.slice(0, i), raw.slice(i)]))) { bad++; console.log(`     cut at ${i}: ${w.why}`); break; }
      }
      if (!same(run([...raw]))) { bad++; console.log(`     one by one: ${w.why}`); }
    }
    check(`N: ${CORPUS.wire.length} wire cases come out as written whole, cut at every place, and one character at a time`, bad === 0, `${bad} wrong`);

    // random streams against the oracle
    let seed = 20261024;
    const rnd = (n) => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed % n; };
    const pieces = ['data: x', 'data:y', 'data', 'data: ', 'data:  two', ': note', 'id: 5', 'id:', 'id: 6\0', 'id: 77', 'event: e', 'retry: 3', 'zzz: q', 'x:', ':', 'data: hé😀', 'DATA: no', '', '', ''];
    const ends = ['\n', '\r\n', '\r'];
    let diffs = 0;
    let firstDiff = '';
    for (let n = 0; n < 4000; n++) {
      let text = '';
      const k = rnd(14);
      for (let i = 0; i < k; i++) { text += pieces[rnd(pieces.length)] + ends[rnd(3)]; }
      if (rnd(4) === 0) { text += pieces[rnd(pieces.length)]; }
      const chunks = [];
      for (let i = 0; i < text.length;) { const len = 1 + rnd(rnd(3) === 0 ? 12 : 3); chunks.push(text.slice(i, i + len)); i += len; }
      const got = run(chunks);
      const want = oracle(text);
      if (JSON.stringify([got.messages, got.last]) !== JSON.stringify([want.messages, want.last])) {
        diffs++;
        if (!firstDiff) { firstDiff = `${JSON.stringify(text)} -> ${JSON.stringify(got)} vs ${JSON.stringify(want)}`; }
      }
    }
    check('N: 4,000 random streams cut at random places read the same as the format read whole', diffs === 0, firstDiff);

    // the bounds
    const msg = (n) => `data: ${'a'.repeat(n)}\n\n`;
    check('N: a message of 65,536 characters is delivered', run([msg(MESSAGE_MAX)]).messages.length === 1 && !run([msg(MESSAGE_MAX)]).oversize);
    check('N: one of 65,537 is not, and the parser says so', run([msg(MESSAGE_MAX + 1)]).messages.length === 0 && run([msg(MESSAGE_MAX + 1)]).oversize);
    check('N: two data lines that together pass the bound are over it', run([`data: ${'a'.repeat(40000)}\ndata: ${'b'.repeat(25536)}\n\n`]).oversize);
    check('N: and at the bound they are not', !run([`data: ${'a'.repeat(40000)}\ndata: ${'b'.repeat(25535)}\n\n`]).oversize);
    check('N: a comment line of 65,542 characters is allowed', !run([`:${'c'.repeat(LINE_MAX - 1)}\ndata: x\n\n`]).oversize);
    check('N: one of 65,543 characters is over', run([`:${'c'.repeat(LINE_MAX)}\ndata: x\n\n`]).oversize);
    check('N: so is a line that never ends', run([`:${'c'.repeat(LINE_MAX)}`]).oversize);
    check('N: after oversize the parser delivers nothing more', (() => { const r = run([msg(MESSAGE_MAX + 1), 'data: late\n\n']); return r.messages.length === 0; })());
    const slowChunks = [];
    for (let i = 0; i < MESSAGE_MAX; i++) { slowChunks.push('a'); }
    const t0 = Date.now();
    const oneByOne = run(['data: ', ...slowChunks, '\n\n']);
    check('N: a message of 65,536 characters arriving one character at a time is delivered whole', oneByOne.messages.length === 1 && oneByOne.messages[0].length === MESSAGE_MAX, `${Date.now() - t0} ms`);

    // cost: tiny lines, 1 MiB and then 8 MiB; the second takes about eight times the first, not sixty-four
    const flood = (bytes) => {
      const unit = ': x\n';
      const chunk = unit.repeat(Math.floor(65536 / unit.length));
      const n = Math.floor(bytes / chunk.length);
      const p = makeParser({ id() { }, message() { }, oversize() { } });
      const start = process.hrtime.bigint();
      for (let i = 0; i < n; i++) { p.feed(chunk); }
      return Number(process.hrtime.bigint() - start) / 1e6;
    };
    flood(1 << 20);
    const t1 = flood(1 << 20);
    const t8 = flood(8 << 20);
    check('N: 8 MiB of comment lines is read in the time of eight 1 MiB (linear, not quadratic)', t8 < Math.max(t1 * 24, 200), `1 MiB ${t1.toFixed(0)} ms, 8 MiB ${t8.toFixed(0)} ms`);
    const t2 = Date.now();
    const many = 'data: x\n\n'.repeat(10000);
    const r10k = run([many]);
    check('N: 10,000 small messages in one chunk are 10,000 messages', r10k.messages.length === 10000, `${Date.now() - t2} ms`);
  }

  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // ---- A. the corpus, case by case ------------------------------------------------------------------------------------------------------
  const srv = await scriptable(corpusDir);
  const base = `http://localhost:${srv.port}/`;
  {
    let ci = 0;
    for (const c of CORPUS.cases) {
      ci++;
      const label = `A${ci}: ${c.why.slice(0, 90)}`;
      srv.reset();
      const { ctx, page, problems } = await fresh(browser, base);
      let failed = null;
      const wrong = (i, s, what) => { if (!failed) { failed = `step ${i} (${s.do}): ${what}`; } };
      for (const [i, s] of c.steps.entries()) {
        if (failed) { break; }
        const before = s.do === 'mount' ? 0 : srv.reqs.length;
        const target = srv.latest();
        if (s.do === 'mount') {
          if (!(await until(() => srv.pending().length === 1))) { wrong(i, s, 'the page made no request'); }
        } else if (s.do === 'open') {
          const p = await until(() => srv.pending()[0]);
          if (!p) { wrong(i, s, 'no request is waiting for an answer'); } else { srv.accept(p); }
        } else if (s.do === 'message') {
          const cur = await until(() => srv.current());
          if (!cur) { wrong(i, s, 'no open connection'); } else { await srv.write(cur, sseOf(s.arg, s.id)); }
        } else if (s.do === 'drop') {
          const p = await until(() => srv.pending()[0] || srv.current(), 300);
          if (!p) { wrong(i, s, 'no connection to drop'); } else if (p.state === 'pending' || s.how === 'reset') { p.req.socket.destroy(); } else { p.res.end(); }
        } else if (s.do === 'refuse') {
          const p = await until(() => srv.pending()[0]);
          if (!p) { wrong(i, s, 'no request is waiting for an answer'); }
          else if (s.how === '500') { p.res.writeHead(500, { 'content-type': 'text/event-stream' }); p.res.end('data: not a stream, whatever it says\n\n'); p.state = 'done'; }
          else if (s.how === 'type') { p.res.writeHead(200, { 'content-type': 'text/plain' }); p.res.end('data: x\n\n'); p.state = 'done'; }
          else if (s.how === 'redirect') { p.res.writeHead(302, { location: '/elsewhere' }); p.res.end(); p.state = 'done'; }
          else { wrong(i, s, `unknown refusal ${s.how}`); }
        } else if (s.do === 'advance') {
          await page.clock.runFor(s.ms);
        } else if (s.do === 'hide') {
          await setVisibility(page, 'hidden');
        } else if (s.do === 'show') {
          await setVisibility(page, 'visible');
        } else if (s.do === 'dispose') {
          await page.getByRole('button', { name: 'Remove' }).click();
        }
        await sleep(100);
        if (failed) { break; }
        const want = {};
        if (s.link !== undefined) { want.link = s.link; }
        if (s.log !== undefined) { want.log = s.log; }
        if (Object.keys(want).length || s.log_len !== undefined) {
          const ok = await until(async () => {
            const d = parseText(await readText(page));
            if (!d) { return false; }
            if (want.link !== undefined && d.link !== want.link) { return false; }
            if (want.log !== undefined && d.log !== want.log) { return false; }
            if (s.log_len !== undefined && d.log.length !== s.log_len) { return false; }
            return true;
          });
          if (!ok) {
            const d = parseText(await readText(page));
            wrong(i, s, `the page shows ${d ? `link=${d.link} log(${d.log.length})=${JSON.stringify(d.log.slice(0, 60))}` : 'nothing'}, wanted ${JSON.stringify(want)}${s.log_len !== undefined ? ` log length ${s.log_len}` : ''}`);
          }
        }
        if (s.requests !== undefined) {
          await until(() => srv.reqs.length >= s.requests, 1500);
          await sleep(150);
          if (srv.reqs.length !== s.requests) { wrong(i, s, `the server saw ${srv.reqs.length} requests, wanted ${s.requests}`); }
        }
        if (s.closed) {
          if (!(await until(() => target && target.closed))) { wrong(i, s, 'the connection was not closed'); }
        }
        if ('request_id' in s) {
          if (!(await until(() => srv.reqs.length > before))) { wrong(i, s, 'no new request was made'); }
          else {
            const got = srv.reqs[srv.reqs.length - 1].headers['last-event-id'];
            if ((got === undefined ? null : got) !== s.request_id) { wrong(i, s, `Last-Event-ID was ${JSON.stringify(got)}, wanted ${JSON.stringify(s.request_id)}`); }
          }
        }
      }
      if (!failed && problems.length) { failed = problems.join('; '); }
      check(label, !failed, failed || '');
      await ctx.close();
    }
  }

  // ---- B. the wire, through a real response ---------------------------------------------------------------------------------------------
  {
    const extra = [
      { why: 'invalid UTF-8 becomes U+FFFD', bytes: Buffer.concat([Buffer.from('data: '), Buffer.from([0xff, 0x41]), Buffer.from('\n\n')]), messages: ['�A'], last_id: null },
      { why: 'a BOM is dropped only at the start', bytes: Buffer.from('﻿data: a\n\n﻿data: b\n\n'), messages: ['a'], last_id: null },
    ];
    const all = [...CORPUS.wire.map((w) => ({ ...w, bytes: Buffer.from(w.raw, 'utf8') })), ...extra];
    for (const mode of ['whole', 'byte']) {
      let bad = 0;
      const first = [];
      for (const w of all) {
        srv.reset();
        const { ctx, page, problems } = await fresh(browser, base);
        const p = await until(() => srv.pending()[0]);
        if (!p) { bad++; first.push(`${w.why}: no request`); await ctx.close(); continue; }
        srv.accept(p);
        if (mode === 'whole') { await srv.write(p, w.bytes); }
        else { for (const b of w.bytes) { await srv.write(p, Buffer.from([b])); await sleep(1); } }
        const want = w.messages.map((m) => `${m}|`).join('');
        const ok = await until(async () => { const d = parseText(await readText(page)); return d && d.log === want && d.link === 'open'; }, 3000);
        let problem = ok ? '' : `shows ${JSON.stringify(parseText(await readText(page)))}`;
        if (ok && w.last_id !== undefined) {
          p.res.end();
          await until(async () => (parseText(await readText(page)) || {}).link === 'retrying');
          await page.clock.runFor(1000);
          if (!(await until(() => srv.reqs.length === 2))) { problem = 'no second request'; }
          else {
            const got = srv.reqs[1].headers['last-event-id'];
            if ((got === undefined ? null : got) !== w.last_id) { problem = `the next request carries ${JSON.stringify(got)}, wanted ${JSON.stringify(w.last_id)}`; }
          }
        }
        if (problem || problems.length) { bad++; first.push(`${w.why}: ${problem || problems.join('; ')}`); }
        await ctx.close();
      }
      check(`B: ${all.length} wire cases through a real response, ${mode === 'whole' ? 'each in one write' : 'one byte per write'}`, bad === 0, first.slice(0, 3).join(' | '));
    }
  }

  // ---- C. what the page will not read ----------------------------------------------------------------------------------------------------
  {
    // a line of 8 MiB that never ends: the page fails the stream after 65,543 characters and closes the connection
    srv.reset();
    const { ctx, page, problems } = await fresh(browser, base);
    const p = await until(() => srv.pending()[0]);
    srv.accept(p);
    let written = 0;
    const chunk = Buffer.from(':' + 'c'.repeat(65535));
    while (written < (8 << 20) && !p.closed) {
      await srv.write(p, chunk);
      written += chunk.length;
      await sleep(0);
      const d = parseText(await readText(page));
      if (d && d.link === 'failed') { break; }
    }
    const d = parseText(await readText(page));
    check('C: a line that never ends fails the stream', d && d.link === 'failed', JSON.stringify(d));
    check('C: after at most 2 MiB written the page had stopped reading and the connection is closed', (await until(() => p.closed)) && written <= (2 << 20), `${written} bytes written`);
    await sleep(200);
    check('C: and the page does not retry', srv.reqs.length === 1);
    check('C: the page stayed responsive', (await page.evaluate(() => 1 + 1)) === 2);
    check('C: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
  }
  {
    // many small data lines that add up to a message over the bound
    srv.reset();
    const { ctx, page } = await fresh(browser, base);
    const p = await until(() => srv.pending()[0]);
    srv.accept(p);
    const line = Buffer.from('data: ' + 'x'.repeat(63) + '\n');
    let n = 0;
    while (!p.closed && n < 4000) {
      await srv.write(p, line);
      n++;
      if (n % 100 === 0) { const d = parseText(await readText(page)); if (d && d.link === 'failed') { break; } }
    }
    const d = parseText(await readText(page));
    check('C: data lines that add up to more than the bound fail the stream', d && d.link === 'failed' && d.log === '', JSON.stringify(d && { link: d.link, log: d.log.length }));
    check('C: after about a thousand lines of 64 characters (65,536 characters), not 4,000', n <= 1100, `${n} lines written`);
    await ctx.close();
  }
  {
    // comments are not data: 4M comment lines are read and nothing happens
    srv.reset();
    const { ctx, page, problems } = await fresh(browser, base);
    const p = await until(() => srv.pending()[0]);
    srv.accept(p);
    const t0 = Date.now();
    const chunk = Buffer.from(': x\n'.repeat(16384));
    for (let i = 0; i < 64; i++) { await srv.write(p, chunk); }
    await srv.write(p, Buffer.from('data: after\n\n'));
    const ok = await until(async () => (parseText(await readText(page)) || {}).log === 'after|', 8000);
    check('C: 4 MiB of comment lines are read, and the message after them arrives', !!ok, `${Date.now() - t0} ms`);
    check('C: the stream is still open', (parseText(await readText(page)) || {}).link === 'open');
    check('C: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
  }

  // ---- D. authority ---------------------------------------------------------------------------------------------------------------------
  {
    const dir = path.join(tmp, 'cred');
    fs.mkdirSync(dir);
    const own = await scriptable(dir);
    const named = await scriptable(null, { cors: true });
    const unnamed = await scriptable(null, { cors: true });
    const namedHost = `localhost:${named.port}`;
    const unnamedHost = `localhost:${unnamed.port}`;
    const src = (url, extra = '', creds = true) => `
component Secured {
    requires Network("${namedHost}", "${unnamedHost}");
${creds ? `    requires Credential("api", "${namedHost}");` : ''}
    state link: String = "";
    state log: String = "";
    state ok: Bool = false;
    subscribe Got "${url}" status Link;
    on Got(t) { log = log + t + "|"; }
    on Link(w) { link = w; }
${extra}
    view { Column { Text { "link={link} log={log}" } Button { label: "Sign in" on: SignIn } } }
}
`.replace(/\n\s*\n/g, '\n');
    const withSignIn = (url, creds = true) => src(url, '    on SignIn { ok = credential.set("api", "tok-1"); }', creds);
    const noop = (url) => src(url, '    on SignIn { ok = false; }');
    const programs = {
      own: noop('/stream'),
      named: noop(`http://${namedHost}/stream`),
      unnamed: noop(`http://${unnamedHost}/stream`),
      anon: withSignIn('/stream'),
    };
    for (const [name, text] of Object.entries(programs)) {
      fs.writeFileSync(path.join(dir, `${name}.cui`), text);
      build(path.join(dir, `${name}.cui`), path.join(dir, `${name}.js`), ['--no-auto-mount']);
    }
    const page = (name, creds) => `<!doctype html><meta charset="utf-8"><div id="root"></div><script type="module">
import { mount_Secured } from './${name}.js';
mount_Secured(document.getElementById('root')${creds});
</script>`;
    const TOKEN = 'tok-3f9a-SECRET';
    const html = {
      'own-with.html': page('own', `, { credentials: { api: '${TOKEN}' } }`),
      'named-with.html': page('named', `, { credentials: { api: '${TOKEN}' } }`),
      'unnamed-with.html': page('unnamed', `, { credentials: { api: '${TOKEN}' } }`),
      'own-without.html': page('own', ''),
      'own-bad.html': page('own', `, { credentials: { api: 'has space' } }`),
      'anon.html': page('anon', ''),
    };
    for (const [f, t] of Object.entries(html)) { fs.writeFileSync(path.join(dir, f), t); }
    const url = (f) => `http://localhost:${own.port}/${f}`;
    const auth = (s) => s.reqs.map((r) => r.headers.authorization || null);

    // supplied: this origin, the named host, the unnamed host
    for (const [f, s, what, wantAuth] of [
      ['own-with.html', own, 'this origin', `Bearer ${TOKEN}`],
      ['named-with.html', named, 'the host the Credential names', `Bearer ${TOKEN}`],
      ['unnamed-with.html', unnamed, 'a host Network allows and the Credential does not name', null],
    ]) {
      own.reset(); named.reset(); unnamed.reset();
      const { ctx, page: pg, problems } = await fresh(browser, url(f));
      const p = await until(() => s.pending()[0]);
      check(`D: the stream to ${what} is requested`, !!p);
      if (p) {
        s.accept(p);
        await s.write(p, 'data: hi\n\n');
        const ok = await until(async () => (parseText(await readText(pg)) || {}).log === 'hi|');
        check(`D: and answered, to ${what}`, !!ok);
        check(`D: Authorization to ${what} is ${wantAuth === null ? 'absent' : 'the bearer token'}`, auth(s)[0] === wantAuth, JSON.stringify(auth(s)));
      }
      check(`D: no other server was asked for a stream (${what})`, [own, named, unnamed].filter((o) => o !== s).every((o) => o.reqs.length === 0), JSON.stringify([own, named, unnamed].map((o) => o.reqs.length)));
      const htmlOut = await pg.locator('#root').innerHTML();
      check(`D: the token is nowhere in what the component rendered (${what})`, !htmlOut.includes(TOKEN));
      // cross-origin, measured: Authorization needs a preflight and the preflight asked for that header; no header, no preflight
      if (s === named) { check('D: the preflight to the named host asked for the Authorization header', named.preflights.some((x) => /authorization/i.test(x.requested)), JSON.stringify(named.preflights)); }
      if (s === unnamed) { check('D: a request without the header needs no preflight (to the unnamed host)', unnamed.preflights.length === 0, JSON.stringify(unnamed.preflights)); }
      check(`D: no page error (${what})`, problems.length === 0, problems.join('; '));
      await ctx.close();
    }
    // not supplied, and a token that is not a plain token: failed before any request
    for (const [f, why] of [['own-without.html', 'not supplied'], ['own-bad.html', 'a token with a space']]) {
      own.reset();
      const { ctx, page: pg } = await fresh(browser, url(f));
      const ok = await until(async () => (parseText(await readText(pg)) || {}).link === 'failed');
      await sleep(200);
      check(`D: a credential ${why}: the stream is failed`, !!ok);
      check(`D: and the server saw no request for it`, own.reqs.length === 0, String(own.reqs.length));
      await ctx.close();
    }

    // anonymous until the program sets its token (docs/bearer-token.md): the first request has none, the next has it
    {
      own.reset();
      const { ctx, page: pg } = await fresh(browser, url('anon.html'));
      const p1 = await until(() => own.pending()[0]);
      check('D: a component that sets its own token connects without one', !!p1 && !p1.headers.authorization);
      own.accept(p1);
      await pg.getByRole('button', { name: 'Sign in' }).click();
      p1.res.end();
      await until(async () => (parseText(await readText(pg)) || {}).link === 'retrying');
      await pg.clock.runFor(1000);
      const p2 = await until(() => own.pending()[0]);
      check('D: after credential.set the next connection carries Bearer tok-1', !!p2 && p2.headers.authorization === 'Bearer tok-1', JSON.stringify(p2 && p2.headers.authorization));
      await ctx.close();
    }

    // the host check at run time: the same page with its address changed after the build to one the component does not allow
    {
      const far = await scriptable(null);
      const tampered = (await fsRead(path.join(dir, 'own.js'))).replace('url: "/stream"', `url: "http://localhost:${far.port}/stream"`);
      if (!tampered.includes(`localhost:${far.port}`)) { throw new Error('the address was not in the module'); }
      fs.writeFileSync(path.join(dir, 'far.js'), tampered);
      fs.writeFileSync(path.join(dir, 'far.html'), page('far', `, { credentials: { api: '${TOKEN}' } }`));
      own.reset();
      const { ctx, page: pg } = await fresh(browser, url('far.html'));
      const ok = await until(async () => (parseText(await readText(pg)) || {}).link === 'failed');
      await sleep(300);
      check('D: an address the component\'s Network does not allow is failed at run time', !!ok);
      check('D: and that host was never asked', far.hits === 0, String(far.hits));
      await ctx.close();
      for (const bad of ['ftp://localhost/s', 'javascript:alert(1)', 'http://']) {
        fs.writeFileSync(path.join(dir, 'bad.js'), (await fsRead(path.join(dir, 'own.js'))).replace('url: "/stream"', `url: ${JSON.stringify(bad)}`));
        fs.writeFileSync(path.join(dir, 'bad.html'), page('bad', `, { credentials: { api: '${TOKEN}' } }`));
        const r = await fresh(browser, url('bad.html'));
        const f = await until(async () => (parseText(await readText(r.page)) || {}).link === 'failed');
        check(`D: ${bad} is failed at run time`, !!f);
        await r.ctx.close();
      }
      await far.close();
    }

    // a second origin, measured: Last-Event-ID is a header too, so the server must allow it, or the page keeps retrying
    {
      const xdir = path.join(tmp, 'xorigin');
      fs.mkdirSync(xdir);
      const run = async (label, opts, wantReconnect) => {
        const xo = await scriptable(null, opts);
        const xs = `component X {\n    requires Network("localhost:${xo.port}");\n    state link: String = "";\n    state log: String = "";\n    subscribe Got "http://localhost:${xo.port}/stream" status Link;\n    on Got(t) { log = log + t + "|"; }\n    on Link(w) { link = w; }\n    view { Text { "link={link} log={log}" } }\n}\n`;
        fs.writeFileSync(path.join(xdir, `${label}.cui`), xs);
        build(path.join(xdir, `${label}.cui`), path.join(xdir, `${label}.js`), ['--no-auto-mount']);
        fs.writeFileSync(path.join(xdir, `${label}.html`), `<!doctype html><meta charset="utf-8"><div id="root"></div><script type="module">\nimport { mount_X } from './${label}.js';\nmount_X(document.getElementById('root'));\n</script>`);
        const o = await scriptable(xdir);
        const r = await fresh(browser, `http://localhost:${o.port}/${label}.html`);
        const p = await until(() => xo.pending()[0], 1500);
        let result = { first: !!p };
        if (p) {
          xo.accept(p);
          await xo.write(p, 'id: 5\ndata: a\n\n');
          result.got = !!(await until(async () => (parseText(await readText(r.page)) || {}).log === 'a|'));
          p.res.end();
          await until(async () => (parseText(await readText(r.page)) || {}).link === 'retrying');
          await r.page.clock.runFor(1000);
          const p2 = await until(() => xo.pending()[0], 1500);
          result.second = !!p2;
          result.header = p2 ? p2.headers['last-event-id'] || null : null;
          result.preflights = xo.preflights.map((x) => x.requested);
          result.link = (parseText(await readText(r.page)) || {}).link;
        } else {
          result.link = (parseText(await readText(r.page)) || {}).link;
        }
        await r.ctx.close();
        await o.close();
        await xo.close();
        return result;
      };
      const good = await run('xgood', { cors: true }, true);
      check('D: a second origin that allows Last-Event-ID: the first request needs no preflight, the reconnect asks for it and carries the id', good.first && good.got && good.second && good.header === '5' && good.preflights.length === 1 && /last-event-id/i.test(good.preflights[0]), JSON.stringify(good));
      const narrow = await run('xnarrow', { cors: true, allowHeaders: 'authorization' }, false);
      check('D: a second origin that does not allow Last-Event-ID: the reconnect is refused by the browser and the page stays retrying, never failed', narrow.first && narrow.got && !narrow.second && narrow.link !== 'failed', JSON.stringify(narrow));
      const none = await run('xnone', {}, false);
      check('D: a second origin with no CORS headers: the stream is never read; the page cannot tell it from a network error, and retries', none.first && !none.got && (none.link === 'retrying' || none.link === 'connecting'), JSON.stringify(none));
    }
    await own.close(); await named.close(); await unnamed.close();
  }

  // ---- E. the page starts hidden, a handler that waits, a 204 ---------------------------------------------------------------------------
  {
    srv.reset();
    const { ctx, page } = await fresh(browser, base, { init: "Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true });" });
    await sleep(300);
    check('E: a page that starts hidden shows paused', (parseText(await readText(page)) || {}).link === 'paused');
    check('E: and has asked for nothing', srv.reqs.length === 0, String(srv.reqs.length));
    await setVisibility(page, 'visible');
    check('E: shown, it connects', !!(await until(() => srv.pending().length === 1)));
    await ctx.close();
  }
  {
    srv.reset();
    const dir = path.join(tmp, 'slow');
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 's.cui'), 'component Slow {\n    requires Network;\n    state r: Result[String, String] = Ok("");\n    state seen: Int = 0;\n    subscribe Got "/stream";\n    on Got(t) {\n        seen = seen + 1;\n        r = await(net.get("/slow"));\n    }\n    view { Text { "seen={seen}" } }\n}\n');
    build(path.join(dir, 's.cui'), path.join(dir, 'out'), ['--page']);
    const s2 = await scriptable(path.join(dir, 'out'));
    const { ctx, page } = await fresh(browser, `http://localhost:${s2.port}/`);
    const p = await until(() => s2.pending()[0]);
    s2.accept(p);
    await s2.write(p, 'data: a\n\n');
    await until(() => s2.slow.length === 1);
    await s2.write(p, 'data: b\n\n');
    check('E: a message arriving while the first handler waits on a request runs its handler (two requests, two runs)', !!(await until(() => s2.slow.length === 2)) && (await page.evaluate(() => document.querySelector('#root').textContent)) === 'seen=2');
    await ctx.close();
    await s2.close();
  }
  {
    // 204 and 200 with no body are not streams
    srv.reset();
    const { ctx, page } = await fresh(browser, base);
    const p = await until(() => srv.pending()[0]);
    p.res.writeHead(204);
    p.res.end();
    p.state = 'done';
    check('E: a 204 is a refusal, not a stream', !!(await until(async () => (parseText(await readText(page)) || {}).link === 'failed')));
    await ctx.close();
  }
  await srv.close();

  // ---- F. examples/stream against its stand-in, as a page ----------------------------------------------------------------------------------
  const readingsSummary = (ns) => {
    const vs = ns.map(reading);
    const f = (x) => String(x);
    return `line chart of ${vs.length} values: minimum ${f(Math.min(...vs))} °C, maximum ${f(Math.max(...vs))} °C, last ${f(vs[vs.length - 1])} °C`;
  };
  const exText = async (page) => page.evaluate(() => document.querySelector('#root').innerText);
  const exCount = async (page) => {
    const m = (await exText(page)).match(/(\d+) readings received/);
    return m ? Number(m[1]) : null;
  };
  const exLink = async (page) => {
    const m = (await exText(page)).match(/Connection: (\w+)/);
    return m ? m[1] : null;
  };
  {
    const dir = path.join(tmp, 'ex-page');
    build(path.join(ROOT, 'examples', 'stream', 'stream.cui'), dir, ['--page']);
    const s = await exampleServer(dir, { interval: 0 });
    const { ctx, page, problems } = await fresh(browser, `http://localhost:${s.address().port}/`);
    check('F: the page connects', !!(await until(async () => (await exLink(page)) === 'open')));
    check('F: its first request has no Last-Event-ID', s.log.connections.length === 1 && s.log.connections[0].lastEventId === null, JSON.stringify(s.log.connections));
    s.tick(); s.tick(); s.tick();
    check('F: three readings arrive and are counted', !!(await until(async () => (await exCount(page)) === 3)));
    const aria = await page.evaluate(() => (document.querySelector('#root figure svg') || { getAttribute: () => null }).getAttribute('aria-label'));
    check('F: the text alternative of the chart is the one worked out from the readings the server sent', aria === `Latest readings: ${readingsSummary([1, 2, 3])}`, String(aria));
    s.dropAll();
    check('F: the server closed the stream: retrying', !!(await until(async () => (await exLink(page)) === 'retrying')));
    s.tick(); s.tick();
    await page.clock.runFor(999);
    await sleep(150);
    check('F: after 999 ms it has not asked again', s.log.connections.length === 1, String(s.log.connections.length));
    await page.clock.runFor(1);
    check('F: after 1,000 ms it asks again', !!(await until(() => s.log.connections.length === 2)));
    check('F: and sends the id of the last reading it had', s.log.connections[1] && s.log.connections[1].lastEventId === '3', JSON.stringify(s.log.connections[1]));
    check('F: the two readings it missed arrive: nothing lost that the server kept', !!(await until(async () => (await exCount(page)) === 5)));
    await setVisibility(page, 'hidden');
    check('F: hidden: paused, and the connection is closed', !!(await until(async () => (await exLink(page)) === 'paused' && s.openStreams() === 0)));
    s.tick();
    await page.clock.runFor(120000);
    await sleep(200);
    check('F: hidden for two minutes asks for nothing', s.log.connections.length === 2, String(s.log.connections.length));
    await setVisibility(page, 'visible');
    check('F: shown: connects at once with the last id, and receives the reading it missed', !!(await until(async () => s.log.connections.length === 3 && s.log.connections[2].lastEventId === '5' && (await exCount(page)) === 6)), JSON.stringify(s.log.connections));
    await page.getByRole('button', { name: 'Clear' }).click();
    check('F: Clear empties what the page holds', (await exCount(page)) === 0);
    // the connection state is a live region (and the readings are not)
    const live = await page.evaluate(() => [...document.querySelectorAll('#root [role=status], #root [aria-live]')].map((e) => e.textContent));
    check('F: only the connection state is a live region', live.length === 1 && /^Connection: /.test(live[0]), JSON.stringify(live));
    check('F: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
    s.closeAllConnections();
    await new Promise((ok) => s.close(ok));
  }

  // ---- G. through the service worker ------------------------------------------------------------------------------------------------------
  {
    const dir = path.join(tmp, 'ex-pwa');
    build(path.join(ROOT, 'examples', 'stream', 'stream.cui'), dir, ['--pwa']);
    let s = await exampleServer(dir, { interval: 0 });
    const port = s.address().port;
    const ctx = await browser.newContext({ ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(String(e)));
    await page.clock.install({ time: 0 });
    await page.clock.pauseAt(60000); // before the page exists: a time read back and paused at `now + 1` is a race
    await page.goto(`http://localhost:${port}/`);
    await page.waitForSelector('#root *');
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    check('G: the service worker controls the page', await page.evaluate(() => !!navigator.serviceWorker.controller));
    check('G: through the worker the stream opens', !!(await until(async () => (await exLink(page)) === 'open')));
    s.tick();
    check('G: a reading written after the headers arrives before the response ends', !!(await until(async () => (await exCount(page)) === 1)));
    s.tick(); s.tick();
    check('G: and so do the next', !!(await until(async () => (await exCount(page)) === 3)));
    // stop the server: the worker answers an error, the page sees a drop
    s.dropAll();
    s.closeAllConnections();
    await new Promise((ok) => s.close(ok));
    check('G: the server stopped: retrying', !!(await until(async () => (await exLink(page)) === 'retrying')));
    await page.clock.runFor(1000);
    check('G: still down after the first wait: retrying again, not failed', !!(await until(async () => (await exLink(page)) === 'retrying')));
    await page.clock.runFor(2000);
    await sleep(200);
    check('G: and after the second', (await exLink(page)) === 'retrying');
    const text = await exText(page);
    check('G: the page itself is still there (the shell comes from the cache)', /Live readings/.test(text) && /3 readings received/.test(text));
    s = await exampleServer(dir, { interval: 0 }, port);
    await page.clock.runFor(4000);
    check('G: the server is back: the page connects again', !!(await until(async () => (await exLink(page)) === 'open')));
    check('G: with the id of the last reading it had', s.log.connections.length === 1 && s.log.connections[0].lastEventId === '3', JSON.stringify(s.log.connections));
    s.tick();
    check('G: and readings flow again', !!(await until(async () => (await exCount(page)) === 4)));
    check('G: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
    s.closeAllConnections();
    await new Promise((ok) => s.close(ok));
  }

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);

  function fsRead(f) { return fs.readFileSync(f, 'utf8'); }
})().catch((e) => { console.error(e); process.exit(1); });
