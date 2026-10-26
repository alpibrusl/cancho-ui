#!/usr/bin/env node
// What the port of the cancho-www page needed (docs/www-port/gaps.md), in Chromium: DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   A. with JavaScript off the page's HTML has the lists, the items a data file wrote, the `<code>` of the data text, the value of the data, every
//      link to a file; and every link to a file of the site answers 200 with the bytes of the file the program has (the build carried it);
//   B. `pressed:`: the accessibility tree says which button is pressed, a click and the keyboard move it, focus stays, the lists are the same nodes;
//   C. a sealed island that reads the state: it has the first value, a change reaches it, a hundred changes in a burst are one or two messages
//      and the last value, a text over 4096 characters is not sent, and the frame still cannot reach the page;
//   D. nothing asks another host, nothing asks for the data file, no console error, the lists are the nodes the HTML had once app.js has run.
//   node scripts/check-port.js
'use strict';
const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const EX = path.join(ROOT, 'examples', 'port');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'port-'));
let holdApp = 0;
function build() {
  const out = path.join(work, 'site');
  const r = spawnSync(BIN, ['build', path.join(EX, 'port.cui'), '--page', '-o', out], { encoding: 'utf8', cwd: ROOT });
  if (r.status !== 0) { console.log(`FAIL build: ${r.stderr}${r.stdout}`); process.exit(1); }
  return out;
}
const requests = [];
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      requests.push(q.url);
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      const send = () => fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.md': 'text/markdown', '.json': 'application/json' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
      if (f.endsWith('app.js') && holdApp) { setTimeout(send, holdApp); } else { send(); }
    }).listen(0, '127.0.0.1', () => ok(s));
  });
}
const PIXEL = devices['Pixel 7'];
const frameOf = (page) => page.frames().find((f) => f !== page.mainFrame());
const stats = (page) => page.evaluate(() => window.__cui_island_stats);

(async () => {
  const site = build();
  const server = await serve(site);
  const base = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch();

  // ---- A. JavaScript off ----------------------------------------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...PIXEL, javaScriptEnabled: false });
    const page = await ctx.newPage();
    await page.goto(`${base}/`);
    const r = await page.evaluate(() => ({
      ul: document.querySelectorAll('ul').length, ol: document.querySelectorAll('ol').length, li: document.querySelectorAll('li').length,
      code: [...document.querySelectorAll('code')].map((c) => c.textContent),
      items: [...document.querySelectorAll('#claims li')].map((li) => li.innerText.replace(/\s+/g, ' ').trim()),
      note: document.querySelector('p[data-style="note"]').textContent,
      links: [...document.querySelectorAll('a[href]')].map((a) => a.getAttribute('href')),
      buttons: document.querySelectorAll('button').length,
      rows: [...document.querySelectorAll('table tbody tr')].map((tr) => tr.textContent.slice(0, 20)),
      region: (document.querySelector('.cui-table-scroll') || {}).getAttribute && document.querySelector('.cui-table-scroll').getAttribute('aria-label'),
    }));
    check('A: a table whose rows a For over the data wrote is in the HTML with no script (1 header row and 2 rows, in a named region)', r.rows.length === 3 && r.rows[0] === 'Claim' && r.region === 'The claims, in a table', JSON.stringify(r.rows));
    check('A: the lists are in the HTML with no script: 2 bulleted, 1 numbered, 7 items', r.ul === 2 && r.ol === 1 && r.li === 7, JSON.stringify([r.ul, r.ol, r.li]));
    check('A: the items a For over the data wrote are there, each with its link', r.items.length === 2 && r.items[0].startsWith('A value is consumed exactly once. Refused as linear-value-unconsumed (19 fixtures). rule fixtures') && r.items[1].startsWith('Authority is derived, not declared: cancho authority lists what a program can reach.'), JSON.stringify(r.items));
    check('A: the backticks of the data text are <code> elements and no backtick is left', JSON.stringify(r.code) === JSON.stringify(['--output json', 'linear-value-unconsumed', 'cancho authority']) && r.items.every((t) => !t.includes('`')), JSON.stringify(r.code));
    check('A: a value of the data file is text in the page', r.note === 'Counted at cancho 1d4c44e, 2026-10-20.', r.note);
    check('A: the buttons are the script\'s (a button needs it): none without script', r.buttons === 0, String(r.buttons));
    const files = r.links.filter((h) => !/^https?:/.test(h) && !h.startsWith('#'));
    check('A: the page links to two files of the site', JSON.stringify(files.sort()) === JSON.stringify(['data/claims.json', 'notes.md']), JSON.stringify(files));
    for (const f of files) {
      const res = await ctx.request.get(`${base}/${f}`);
      const body = await res.body();
      check(`A: ${f} answers 200 with the bytes of the file the program has`, res.status() === 200 && sha(body) === sha(fs.readFileSync(path.join(EX, f))), `${res.status()} ${body.length} bytes`);
    }
    await ctx.close();
  }

  // ---- B. pressed, and the nodes of the lists --------------------------------------------------------------------------------------------------
  requests.length = 0;
  const ctx = await browser.newContext({ ...PIXEL, viewport: { width: 1000, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  const origins = new Set();
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
  page.on('request', (r) => { if (!r.url().startsWith('about:') && !r.url().startsWith('data:')) { origins.add(new URL(r.url()).origin); } });
  holdApp = 1200;
  await page.goto(`${base}/`, { waitUntil: 'domcontentloaded' });
  // the lists before app.js has run: marked, then compared with the nodes after
  const before = await page.evaluate(() => { window.__lists = [...document.querySelectorAll('ul, ol')]; return document.querySelectorAll('li').length; });
  await sleep(1800);
  holdApp = 0;
  const same = await page.evaluate(() => window.__lists.every((l) => l.isConnected) && window.__lists.length === 3);
  check('D: the lists are the nodes the HTML had once app.js has run (static, never rebuilt)', same && before === 7, `${before} items before, same nodes: ${same}`);
  const cdp = await ctx.newCDPSession(page);
  const axButtons = async () => (await cdp.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored && n.role && n.role.value === 'button')
    .map((n) => ({ name: n.name && n.name.value, pressed: (n.properties || []).filter((p) => p.name === 'pressed').map((p) => p.value.value)[0] }));
  let b = await axButtons();
  const get = (name) => b.find((x) => x.name === name);
  check('B: the accessibility tree says Facts is pressed and Claims is not', get('Facts') && get('Facts').pressed === 'true' && get('Claims') && get('Claims').pressed === 'false', JSON.stringify(b));
  check('B: a button with no pressed: is not a toggle button (the tree has no state for it)', get('Next') && get('Next').pressed === undefined, JSON.stringify(get('Next')));
  await page.click('button[aria-label="Claims"]');
  await sleep(120);
  b = await axButtons();
  check('B: a click moves it: Claims is pressed, Facts is not, and the status says so', get('Claims').pressed === 'true' && get('Facts').pressed === 'false' && (await page.innerText('[role=status]')) === 'Showing claims', JSON.stringify(b));
  await page.focus('button[aria-label="Facts"]');
  await page.keyboard.press('Space');
  await sleep(120);
  b = await axButtons();
  const focused = await page.evaluate(() => document.activeElement && document.activeElement.getAttribute('aria-label'));
  check('B: the keyboard moves it, and focus is still on the button', get('Facts').pressed === 'true' && get('Claims').pressed === 'false' && focused === 'Facts', `${JSON.stringify(b)} focus ${focused}`);
  const nodes = await page.evaluate(() => window.__lists.every((l) => l.isConnected));
  check('B: and the lists are still the same nodes after the buttons were drawn again', nodes);

  // ---- C. the island that reads the state --------------------------------------------------------------------------------------------------------
  const f = frameOf(page);
  const say = () => f.evaluate(() => ({ text: document.getElementById('out').textContent, seen: Number(document.getElementById('out').getAttribute('data-seen')) }));
  let s = await say();
  check('C: the frame has the first values of the state: step 0, a note of 10 characters', s.text === 'step 0, note 10 characters', JSON.stringify(s));
  const first = await f.evaluate(() => document.getElementById('out').getAttribute('data-first'));
  check('C: and the first message the frame was sent already had them (not only a later one)', first === 'step 0, note 10 characters', String(first));
  await page.click('button[aria-label="Next"]');
  await sleep(200);
  s = await say();
  check('C: a change of the state reaches the frame: step 1', s.text === 'step 1, note 10 characters', JSON.stringify(s));
  const sent0 = (await stats(page)).sent;
  const seen0 = s.seen;
  const t0 = Date.now();
  await page.evaluate(() => { const b = document.querySelector('button[aria-label="Next"]'); for (let i = 0; i < 100; i++) { b.click(); } });
  await sleep(400);
  s = await say();
  const sent = (await stats(page)).sent - sent0;
  check('C: a hundred changes in a burst arrive as the last value', s.text === 'step 101, note 10 characters', JSON.stringify(s));
  check('C: and as few messages (at most one in 50 ms, and the page drew 100 times)', sent >= 1 && sent <= 4 && s.seen - seen0 === sent, `${sent} sent, ${s.seen - seen0} heard by the frame, ${Date.now() - t0} ms`);
  const dropped0 = (await stats(page)).dropped;
  for (let i = 0; i < 8; i++) { await page.click('button[aria-label="Grow the note"]'); await sleep(70); }
  s = await say();
  check('C: a note that doubles to 2560 characters is sent', s.text === 'step 101, note 2560 characters', JSON.stringify(s));
  await page.click('button[aria-label="Grow the note"]');
  await sleep(200);
  s = await say();
  const st = await stats(page);
  check('C: at 5120 characters (over 4096) the text is not sent: the frame keeps the last value, and the drop is counted', s.text === 'step 101, note 2560 characters' && st.dropped === dropped0 + 1, `${JSON.stringify(s)} dropped ${st.dropped - dropped0}`);
  const reach = await f.evaluate(() => { try { return typeof parent.document.title === 'string' ? 'reached' : 'no'; } catch (e) { return e.name; } });
  check('C: the frame that reads the state still cannot reach the page', reach === 'SecurityError', reach);
  const attr = await page.evaluate(() => JSON.parse(document.querySelector('iframe').getAttribute('data-cui-island')));
  check('C: the page tells the runtime which props it gives again, with their types', JSON.stringify(attr.typed) === JSON.stringify({ note: 'String', step: 'Int' }), JSON.stringify(attr));

  // ---- D. nothing else ---------------------------------------------------------------------------------------------------------------------
  check('D: every request was to the page\'s own origin', origins.size === 1 && [...origins][0] === base, [...origins].join(' '));
  check('D: the data file was never asked for (the page has what it said)', !requests.some((u) => u.includes('claims.json')), requests.join(' '));
  check('D: no console error', errors.length === 0, errors.join(' | '));
  await ctx.close();
  await browser.close();
  server.close();
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log('FAIL', e); process.exit(1); });
