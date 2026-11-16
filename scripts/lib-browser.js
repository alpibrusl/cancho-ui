// A browser for the checks that must run where `playwright` is not installed (docs/structure.md 12.0r): `playwright` when it is, else Chrome or Chromium found through $CHROME or the usual places,
// driven over the DevTools protocol (the WebSocket of node 22 or later; no package). `open(url)` gives a page with `evaluate(source)` (the value of the expression, a promise awaited), `problems` (page
// errors, console errors and the errors the browser logs, such as a policy violation or a request that failed) and `close()`. The pages are the mobile profile of a Pixel 7: 412 x 915, touch.
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');

function serve(dir) {
  const types = { '.html': 'text/html', '.js': 'text/javascript' };
  return new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const u = req.url.split('?')[0];
      const f = path.join(dir, u === '/' ? 'index.html' : path.normalize(u));
      if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': types[path.extname(f)] || 'text/plain', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
}

async function launch() {
  let playwright = null;
  try { playwright = require('playwright'); } catch (e) { /* not installed */ }
  if (playwright) {
    const browser = await playwright.chromium.launch({ args: ['--no-proxy-server'] });
    return {
      name: 'playwright',
      async open(url) {
        const ctx = await browser.newContext({ ...playwright.devices['Pixel 7'], serviceWorkers: 'block' });
        const page = await ctx.newPage();
        const problems = [];
        page.on('pageerror', (e) => problems.push(`pageerror ${e}`));
        page.on('console', (m) => { if (m.type() === 'error') { problems.push(`console ${m.text()}`); } });
        await page.goto(url);
        return { problems, evaluate: (src) => page.evaluate(src), close: () => ctx.close() };
      },
      close: () => browser.close(),
    };
  }
  const candidates = [process.env.CHROME, '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/Applications/Chromium.app/Contents/MacOS/Chromium', '/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'].filter(Boolean);
  const exe = candidates.find((c) => fs.existsSync(c));
  if (!exe) { throw new Error('no browser: install playwright, or set CHROME to a Chrome or Chromium'); }
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'check-photo-chrome-'));
  const proc = spawn(exe, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--no-proxy-server', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  const wsUrl = await new Promise((ok, no) => {
    let buf = '';
    proc.stderr.on('data', (d) => { buf += d; const m = /DevTools listening on (ws:\/\/\S+)/.exec(buf); if (m) { ok(m[1]); } });
    proc.on('exit', () => no(new Error('the browser ended before it listened')));
    setTimeout(() => no(new Error('the browser did not listen in 20 s')), 20000);
  });
  const port = new URL(wsUrl).port;
  return {
    name: 'chrome over the DevTools protocol',
    async open(url) {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
      const ws = new WebSocket(list.webSocketDebuggerUrl);
      await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no; });
      let next = 0;
      const waiting = new Map();
      const problems = [];
      let loaded = null;
      ws.onmessage = (m) => {
        const msg = JSON.parse(m.data);
        if (msg.id && waiting.has(msg.id)) { waiting.get(msg.id)(msg); waiting.delete(msg.id); return; }
        if (msg.method === 'Page.loadEventFired' && loaded) { loaded(); }
        if (msg.method === 'Runtime.exceptionThrown') { problems.push(`pageerror ${msg.params.exceptionDetails.text} ${msg.params.exceptionDetails.exception && msg.params.exceptionDetails.exception.description}`); }
        if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') { problems.push(`console ${msg.params.args.map((a) => a.value || a.description).join(' ')}`); }
        if (msg.method === 'Log.entryAdded' && msg.params.entry.level === 'error') { problems.push(`log ${msg.params.entry.text} ${msg.params.entry.url || ''}`); }
      };
      const send = (method, params) => new Promise((ok) => { const id = ++next; waiting.set(id, ok); ws.send(JSON.stringify({ id, method, params: params || {} })); });
      await send('Runtime.enable'); await send('Page.enable'); await send('Log.enable');
      await send('Emulation.setDeviceMetricsOverride', { width: 412, height: 915, deviceScaleFactor: 2.625, mobile: true });
      await send('Emulation.setTouchEmulationEnabled', { enabled: true });
      const done = new Promise((ok) => { loaded = ok; });
      await send('Page.navigate', { url });
      await done;
      return {
        problems,
        async evaluate(src) {
          const r = await send('Runtime.evaluate', { expression: src, awaitPromise: true, returnByValue: true });
          if (r.result.exceptionDetails) { throw new Error(`in the page: ${r.result.exceptionDetails.exception ? r.result.exceptionDetails.exception.description : r.result.exceptionDetails.text}`); }
          return r.result.result.value;
        },
        close: async () => { ws.close(); await fetch(`http://127.0.0.1:${port}/json/close/${list.id}`).catch(() => {}); },
      };
    },
    close: async () => { proc.kill(); },
  };
}


module.exports = { serve, launch };
