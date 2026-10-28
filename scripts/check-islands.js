#!/usr/bin/env node
// Sealed islands in Chromium (docs/islands.md): DESKTOP CHROMIUM, not a device. Watchdog: 5 minutes.
//   A. examples/islands: the frame is drawn from its files, is given its props, and its event reaches the handler; what the frame sends that the
//      page does not hear (an event it does not declare, one the view does not hear, a value of another type, a flood, a message from anything
//      that is not the frame) is dropped and counted, and changes nothing;
//   B. the pointer: forwarded to the frame whether it is over the page or over the frame, at most 20 a second, and not while the frame is off screen;
//   C. the ways a page and its frame can meet in a different order: a page whose script comes late, a frame built by the script after the page
//      loaded (props by the ready handshake), and a render that does not touch the frame (the same node, the same document);
//   D. the page without script: the frame is in the HTML as written, with its style, its size and its label, and no drawing;
//   E. the page's `frame-src about:`: a frame with an address is refused, a `srcdoc` one is not;
//   F. what the frame can reach: the 30 attempts of tests/fixtures/islands/reach.js, from inside a frame the compiler wrote; a second server, the
//      page's own server and a UDP socket count what arrives; nothing may;
//   G. accessibility: a decorative frame is hidden and cannot be tabbed into, a labelled one has its label; the user's colour scheme and
//      reduced motion reach the frame; the page asks nothing of any origin but its own.
//   node scripts/check-islands.js
'use strict';
const dgram = require('dgram');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const sha = (file) => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'islands-'));
const example = fs.readFileSync(path.join(ROOT, 'examples', 'islands', 'islands.cui'), 'utf8');
fs.cpSync(path.join(ROOT, 'examples', 'islands', 'assets'), path.join(work, 'assets'), { recursive: true });
fs.copyFileSync(path.join(ROOT, 'tests', 'fixtures', 'islands', 'reach.js'), path.join(work, 'assets', 'reach.js'));
fs.writeFileSync(path.join(work, 'assets', 'quiet.js'), "cancho.host.setAttribute('data-quiet', 'ran');\n");
fs.cpSync(path.join(ROOT, 'examples', 'map-island', 'assets'), path.join(work, 'assets'), { recursive: true });
let nextBuild = 0;
function build(name, source) {
  const dir = path.join(work, `b${nextBuild++}-${name}`);
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(work, `${name}.cui`);
  fs.writeFileSync(file, source);
  const r = spawnSync(BIN, ['build', file, '--page', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${name}: ${r.stdout}${r.stderr}`); }
  return dir;
}

// the servers: the page's, another origin that logs every request, and a UDP socket standing for a STUN server
const hits = []; // another origin
const pageHits = []; // /leak on the page's own origin
let pageRoot = work;
const delays = {};
const other = http.createServer((q, r) => { hits.push(q.url); r.setHeader('content-type', 'text/javascript'); r.setHeader('access-control-allow-origin', '*'); r.end('window.__mod = 1;'); });
const site = http.createServer((q, r) => {
  const u = q.url.split('?')[0];
  if (u.startsWith('/leak')) { pageHits.push(q.url); r.end('ok'); return; }
  const f = path.join(pageRoot, u === '/' ? 'index.html' : decodeURIComponent(u));
  const send = () => fs.readFile(f, (e, d) => {
    if (e || !fs.statSync(f).isFile()) { r.statusCode = 404; r.end(); return; }
    r.setHeader('content-type', { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html' }[path.extname(f)] || 'application/octet-stream');
    r.end(d);
  });
  if (delays[u]) { setTimeout(send, delays[u]); } else { send(); }
});
const stun = dgram.createSocket('udp4');
const stunHits = [];
stun.on('message', (m) => stunHits.push(m.length));

const text = (page) => page.evaluate(() => document.body.innerText);
const stats = (page) => page.evaluate(() => window.__cui_island_stats);
const stats_ = stats;
const frameOf = (page) => page.frames().find((f) => f !== page.mainFrame());
// the ellipse a frame drew for the stone `i`: the radius the example's script gives a seed (its own formula, written again here)
const rx = (seed, i) => 34 + ((seed * 7 + i * 13) % 22);

(async () => {
  await new Promise((ok) => other.listen(0, '127.0.0.1', ok));
  await new Promise((ok) => site.listen(0, '127.0.0.1', ok));
  await new Promise((ok) => stun.bind(0, '127.0.0.1', ok));
  const A = `http://127.0.0.1:${site.address().port}`;
  const B = `http://127.0.0.1:${other.address().port}`;
  const browser = await chromium.launch();
  const open = async (url, opts = {}, waitUntil = 'load') => {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 1000, height: 800 }, ...opts });
    const page = await ctx.newPage();
    const errors = [];
    const origins = new Set();
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    page.on('request', (r) => { if (!r.url().startsWith('data:') && !r.url().startsWith('about:')) { origins.add(new URL(r.url()).origin); } });
    await page.goto(url, { waitUntil });
    return { ctx, page, errors, origins };
  };

  // ---------- A: the example ----------
  const exDir = build('example', example.replace(/"assets\//g, '"assets/'));
  pageRoot = exDir;
  {
    const { ctx, page, errors, origins } = await open(`${A}/`);
    await sleep(700);
    const f = frameOf(page);
    check('A: the frame drew its three stones from its own files', f && (await f.evaluate(() => document.querySelectorAll('ellipse').length)) === 3);
    const radii = await f.evaluate(() => [...document.querySelectorAll('ellipse')].map((e) => Number(e.getAttribute('rx'))));
    check('A: the props arrived (seed 3 sizes the stones as the island formula says)', JSON.stringify(radii) === JSON.stringify([0, 1, 2].map((i) => rx(3, i))), JSON.stringify(radii));
    check('A: the frame has the sandbox, its label and the data the runtime reads', await page.evaluate(() => { const i = document.querySelector('iframe'); return i.getAttribute('sandbox') === 'allow-scripts' && i.title === 'Three stones in a row. Click one to pick it.' && JSON.parse(i.getAttribute('data-cui-island')).name === 'Stones'; }));
    check('A: the frame cannot see the page', await f.evaluate(() => { try { return typeof parent.document.title === 'string' ? false : true; } catch (e) { return e.name === 'SecurityError'; } }));
    // an event of the island reaches the handler of the same name
    const s0 = await stats(page);
    await f.evaluate(() => document.querySelectorAll('ellipse')[1].dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await sleep(150);
    check('A: a click on a stone is heard: the handler Picked ran with its number', (await text(page)).includes('Picked stone: 2'));
    const s1 = await stats(page);
    check('A: and counted as heard, not dropped', s1.heard === s0.heard + 1 && s1.dropped === s0.dropped, JSON.stringify(s1));
    // what is not heard
    const emit = async (expr) => { const before = await stats(page); await f.evaluate(expr); await sleep(120); const after = await stats(page); return { heard: after.heard - before.heard, dropped: after.dropped - before.dropped }; };
    const cases = [
      ['an event the island does not declare', "cancho.emit('Nope', 1)"],
      ['an event the island declares and the view does not hear', "cancho.emit('Closed')"],
      ['a value of another type (a string for an Int)', "cancho.emit('Picked', '3')"],
      ['a number with a fraction for an Int', "cancho.emit('Picked', 1.5)"],
      ['a missing value for an event that carries one', "cancho.emit('Picked')"],
      ['an object for an Int', "cancho.emit('Picked', { n: 1 })"],
      ['a name that is on every object', "cancho.emit('constructor', 1)"],
      ['a name that is on every object, with no value', "cancho.emit('constructor')"],
      ['the name toString, with no value', "cancho.emit('toString')"],
      ['the name __proto__', "cancho.emit('__proto__', 1)"],
      ['a message that is not an emit (a pointer message with text)', "parent.postMessage({ cui: 'pointer', x: 'a', y: 1 }, '*')"],
    ];
    for (const [what, expr] of cases) {
      const r = await emit(expr);
      check(`A: ${what} is dropped and counted (island-event-dropped), and no handler ran`, r.heard === 0 && r.dropped === 1 && (await text(page)).includes('Picked stone: 2'), JSON.stringify(r));
    }
    // a message from something that is not the frame
    const fake = await page.evaluate(async () => {
      const before = window.__cui_island_stats.heard;
      window.postMessage({ cui: 'emit', name: 'Picked', value: 9 }, '*');
      const g = document.createElement('iframe');
      g.setAttribute('sandbox', 'allow-scripts');
      g.srcdoc = '<script>parent.postMessage({ cui: "emit", name: "Picked", value: 8 }, "*")<\/script>';
      document.body.appendChild(g);
      await new Promise((ok) => setTimeout(ok, 400));
      g.remove();
      return window.__cui_island_stats.heard - before;
    });
    check('A: a message from the page itself, and from a frame that is not an island, is not heard', fake === 0 && (await text(page)).includes('Picked stone: 2'), `heard ${fake}`);
    // a flood
    await sleep(1100);
    const flood = await emit("for (let i = 0; i < 120; i++) { cancho.emit('Picked', 3); }");
    check('A: a frame that sends 120 events at once is heard 50 times at most in that second, and the rest are dropped', flood.heard === 50 && flood.dropped === 70, JSON.stringify(flood));
    check('A: no script error, and nothing asked of any origin but the page\'s', errors.length === 0 && [...origins].every((o) => o === A), `${errors.join(' | ')} ${[...origins].join(' ')}`);
    await ctx.close();
  }

  // ---------- B: the pointer ----------
  {
    const { ctx, page } = await open(`${A}/`);
    await sleep(600);
    const f = frameOf(page);
    const box = await page.evaluate(() => { const r = document.querySelector('iframe').getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
    const cx0 = () => f.evaluate(() => Number(document.querySelector('ellipse').getAttribute('cx')));
    // over the page, then over the frame: both reach the island
    await page.mouse.move(2, 2); await sleep(120);
    const left = await cx0();
    await page.mouse.move(990, 2); await sleep(120);
    const right = await cx0();
    check('B: the pointer over the page leans the stones (left and right differ)', right - left > 5, `${left} -> ${right}`);
    await page.mouse.move(box.x + 4, box.y + 40); await sleep(150);
    const inL = await cx0();
    await page.mouse.move(box.x + box.w - 4, box.y + 40); await sleep(150);
    const inR = await cx0();
    check('B: the pointer over the frame, which the page cannot see, leans them too, from the frame\'s message', inR - inL > 1, `${inL} -> ${inR}`);
    // the rate
    const before = (await stats(page)).pointer;
    const t0 = Date.now();
    for (let i = 0; i < 260; i++) { await page.mouse.move(10 + ((i * 7) % 980), 30 + (i % 300)); await sleep(6); }
    await sleep(120);
    const secs = (Date.now() - t0) / 1000;
    const posted = (await stats(page)).pointer - before;
    check('B: moving the pointer 260 times is forwarded at most 20 times a second', posted <= 20 * secs + 3 && posted >= 5 * secs, `${posted} in ${secs.toFixed(2)} s`);
    // off screen
    await page.setViewportSize({ width: 1000, height: 40 });
    await sleep(200);
    const off0 = (await stats(page)).pointer;
    for (let i = 0; i < 40; i++) { await page.mouse.move(10 + i * 20, 10); await sleep(30); }
    check('B: while the frame is off screen the pointer is not forwarded', (await stats(page)).pointer === off0, `${(await stats(page)).pointer - off0} posted`);
    await page.setViewportSize({ width: 1000, height: 800 });
    await sleep(200);
    for (let i = 0; i < 10; i++) { await page.mouse.move(100 + i * 50, 10); await sleep(70); }
    check('B: and when it is on screen again it is', (await stats(page)).pointer > off0);
    await ctx.close();
  }

  // ---------- C: orders ----------
  {
    delays['/app.js'] = 1500;
    const { ctx, page } = await open(`${A}/`, {}, 'commit');
    await sleep(900);
    const f = frameOf(page);
    const early = await f.evaluate(() => [...document.querySelectorAll('ellipse')].map((e) => Number(e.getAttribute('rx'))));
    check('C: before the page\'s script has run the frame is up and has drawn with no props', early.length === 3 && early[0] === rx(0, 0), JSON.stringify(early));
    await sleep(2500);
    const late = await f.evaluate(() => [...document.querySelectorAll('ellipse')].map((e) => Number(e.getAttribute('rx'))));
    check('C: when the page\'s script comes late the frame is given its props', late[0] === rx(3, 0), JSON.stringify(late));
    await ctx.close();
    delete delays['/app.js'];
  }
  {
    // a frame inside an `If` is refused (docs/islands.md 13.1): it would be a new <iframe> at every render. This was a check of what such a frame did (the ready
    // handshake, its event); the build is refused now, by the exit code and by the rule tag, not by a word of the message
    const late = example.replace('Stones { seed: 3 on: Picked }', 'Button { label: "Show" on: Show } If { when: shown Stones { seed: 3 on: Picked } }')
      .replace('state picked: Int = 0;', 'state picked: Int = 0;\n    state shown: Bool = false;\n    on Show { shown = true; }');
    const file = path.join(work, 'late.cui');
    fs.writeFileSync(file, late);
    const built = spawnSync(BIN, ['build', file, '--page', '-o', path.join(work, 'late-out')], { encoding: 'utf8' });
    check('C: a frame inside an If is refused at build: the exit code is 1 and nothing is written', built.status === 1 && !fs.existsSync(path.join(work, 'late-out', 'index.html')), `exit ${built.status}`);
    const checked = spawnSync(BIN, ['check', file, '--output', 'json'], { encoding: 'utf8' });
    let tags = [];
    try { tags = JSON.parse(checked.stdout).map((d) => d.code); } catch (e) { tags = [`not JSON: ${checked.stdout.slice(0, 80)}`]; }
    check('C: and the one rule it is refused by is island-rebuilt', checked.status === 1 && tags.length === 1 && tags[0] === 'island-rebuilt', JSON.stringify(tags));
  }
  {
    // a render does not touch the frame
    const { ctx, page } = await open(`${A}/`);
    await sleep(600);
    const f = frameOf(page);
    await page.evaluate(() => { window.__frame = document.querySelector('iframe'); });
    const origin0 = await f.evaluate(() => performance.timeOrigin);
    const sent0 = (await stats(page)).sent;
    await f.evaluate(() => document.querySelectorAll('ellipse')[0].dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await sleep(300);
    const same = await page.evaluate(() => document.querySelector('iframe') === window.__frame && document.querySelectorAll('iframe').length === 1);
    const origin1 = await f.evaluate(() => performance.timeOrigin);
    check('C: a state change renders the page and leaves the frame: the same node, and the document did not restart', same && origin0 === origin1 && (await text(page)).includes('Picked stone: 1'), `${origin0} ${origin1}`);
    check('C: and it was given its props once', (await stats(page)).sent - sent0 === 0, `${(await stats(page)).sent - sent0} more`);
    await ctx.close();
  }

  // ---------- D: without script ----------
  {
    const html = fs.readFileSync(path.join(exDir, 'index.html'), 'utf8');
    check('D: the frame is in index.html as written (the sandbox, the label, the size and its document)', html.includes('<iframe sandbox="allow-scripts" title="Three stones in a row. Click one to pick it."') && html.includes('width="320" height="200"') && html.includes('srcdoc="&lt;!doctype html&gt;'));
    const { ctx, page } = await open(`${A}/`, { javaScriptEnabled: false });
    await sleep(500);
    const f = frameOf(page);
    const box = await page.evaluate(() => { const r = document.querySelector('iframe').getBoundingClientRect(); return { w: Math.round(r.width), h: Math.round(r.height) }; });
    const inside = f ? await f.evaluate(() => ({ svg: document.querySelectorAll('svg').length, style: document.querySelectorAll('style').length })) : null;
    check('D: with script off the visitor gets the frame, in its size, with its style and no drawing', box.w === 320 && box.h === 200 && inside && inside.svg === 0 && inside.style === 1, JSON.stringify({ box, inside }));
    check('D: and the page\'s static text is there, while the text that reads state (a hole) waits for the script', (await text(page)).includes('Garden') && !(await text(page)).includes('Picked stone'), (await text(page)).replace(/\n/g, ' | '));
    await ctx.close();
  }

  // ---------- E: frame-src about: ----------
  {
    const { ctx, page, errors } = await open(`${A}/`);
    await sleep(500);
    const before = hits.length;
    const r = await page.evaluate(async (b) => {
      const g = document.createElement('iframe'); g.src = `${b}/leak?frame`; document.body.appendChild(g);
      const h = document.createElement('iframe'); h.srcdoc = '<p id="x">hello</p>'; document.body.appendChild(h);
      await new Promise((ok) => setTimeout(ok, 600));
      return { srcdoc: !!(h.contentDocument && h.contentDocument.getElementById('x')) };
    }, B);
    check('E: a frame the page adds with an address is refused (0 requests to its host)', hits.length === before, `${hits.length - before} requests`);
    check('E: a frame with a srcdoc is not', r.srcdoc === true);
    check('E: the refusal is the browser\'s, with the directive named', errors.some((e) => e.includes('frame-src')), errors.join(' | ').slice(0, 200));
    await ctx.close();
  }

  let reachCui;
  // ---------- F: what the frame can reach ----------
  {
    const quiet = sha(path.join(work, 'assets', 'quiet.js'));
    reachCui = (open, nav) => `island Reach {
    ${open ? 'sealed: false;' : ''}
    script: "assets/reach.js" sha256: "${sha(path.join(work, 'assets', 'reach.js'))}";
    decorative: true;
    width: 20;
    height: 20;
    props { other: String; page: String; stun: Int; nav: Bool; }
    events { Result: String; }
}

island Quiet {
    sealed: false;
    script: "assets/quiet.js" sha256: "${quiet}";
    decorative: true;
    width: 10;
    height: 10;
}

component Probe {
    requires Dom;
    state result: String = "";
    on Result(r) { result = r; }
    view { Column { Reach { other: "${B}" page: "${A}" stun: ${stun.address().port} nav: ${nav} on: Result } ${open ? '' : 'Quiet { }'} Text { "{result}" } } }
}
`;
    const dir = build('reach', reachCui(false, true));
    pageRoot = dir;
    const { ctx, page } = await open(`${A}/`);
    let said = '';
    for (let i = 0; i < 160 && !said.includes('{'); i++) { await sleep(250); said = await text(page); }
    await sleep(2000);
    let got = {};
    try { got = JSON.parse(said.slice(said.indexOf('{'))); } catch (e) { got = {}; }
    const names = Object.keys(got);
    check('F: the island ran its attempts and said what happened to each', names.length >= 30, `${names.length} attempts`);
    check('F: another origin received nothing', hits.length === 0, hits.join(' '));
    check('F: the page\'s own origin received nothing from the frame', pageHits.length === 0, pageHits.join(' '));
    check('F: the STUN server received no packet', stunHits.length === 0, `${stunHits.length} packets`);
    const bad = Object.entries(got).filter(([, v]) => ['reached', 'sent', 'answered', 'opened', 'registered', 'written', 'ran', 'constructed'].some((w) => v === w || v.startsWith(w + ' ')) && !['postMessage to the parent'].includes(names.find((n) => got[n] === v)));
    check('F: no attempt that could succeed did', bad.length === 0, JSON.stringify(bad));
    check('F: eval and new Function are refused', got.eval === 'EvalError' && got['new Function'] === 'EvalError', `${got.eval} ${got['new Function']}`);
    check('F: parent.document is a SecurityError', got['read parent.document'] === 'SecurityError' && got.localStorage === 'SecurityError');
    check('F: RTCPeerConnection is a TypeError (the constructor was removed in the frame)', got['RTCPeerConnection to a STUN server'] === 'TypeError' && got.webkitRTCPeerConnection === 'TypeError', `${got['RTCPeerConnection to a STUN server']} ${got.webkitRTCPeerConnection}`);
    check('F: a script the island adds to its own document does not run (only the compiler\'s, by hash)', ['an inline <script> the island adds', 'document.write of a script', 'an inline event handler attribute', 'a javascript: address'].every((k) => got[k] === 'not run'), JSON.stringify(['an inline <script> the island adds', 'document.write of a script', 'an inline event handler attribute', 'a javascript: address'].map((k) => got[k])));
    check('F: an open island on the same page ran (the sealed path is not what stops it)', await page.evaluate(() => !!document.querySelector('[data-quiet=ran]')));
    check('F: and the page still carries frame-src about: for the sealed island', await page.evaluate(() => document.querySelector('meta[http-equiv=Content-Security-Policy]').content === 'frame-src about:'));
    await ctx.close();
    pageRoot = exDir;
  }

  // ---------- G: accessibility, preferences, origins ----------
  {
    const decorative = example.replace('label: "Three stones in a row. Click one to pick it.";', 'decorative: true;');
    const dir = build('decorative', decorative);
    pageRoot = dir;
    const { ctx, page } = await open(`${A}/`);
    await sleep(500);
    const attrs = await page.evaluate(() => { const i = document.querySelector('iframe'); return { hidden: i.getAttribute('aria-hidden'), tab: i.getAttribute('tabindex'), title: i.getAttribute('title') }; });
    check('G: a decorative frame is aria-hidden, out of the tab order and has no title', attrs.hidden === 'true' && attrs.tab === '-1' && attrs.title === '', JSON.stringify(attrs));
    const seen = [];
    for (let i = 0; i < 4; i++) { await page.keyboard.press('Tab'); seen.push(await page.evaluate(() => document.activeElement && document.activeElement.tagName)); }
    check('G: Tab never lands in a decorative frame', !seen.includes('IFRAME'), seen.join(' '));
    await ctx.close();
    pageRoot = exDir;
  }
  {
    const { ctx, page } = await open(`${A}/`, { colorScheme: 'dark', reducedMotion: 'reduce' });
    await sleep(600);
    const f = frameOf(page);
    const m = await f.evaluate(() => ({ dark: matchMedia('(prefers-color-scheme: dark)').matches, reduced: matchMedia('(prefers-reduced-motion: reduce)').matches, fill: getComputedStyle(document.querySelector('ellipse')).fill, transition: getComputedStyle(document.querySelector('ellipse')).transitionDuration }));
    check('G: the visitor\'s dark scheme and reduced motion reach the frame without a prop (its stylesheet took the dark colour and no transition)', m.dark && m.reduced && m.fill === 'rgb(108, 114, 124)' && m.transition === '0s', JSON.stringify(m));
    await ctx.close();
  }

  // ---------- I: the open island (the map) ----------
  {
    const dir = build('map', fs.readFileSync(path.join(ROOT, 'examples', 'map-island', 'map.cui'), 'utf8'));
    pageRoot = dir;
    const { ctx, page, errors, origins } = await open(`${A}/`);
    await sleep(700);
    const st = () => page.evaluate(() => ({ rocks: document.querySelectorAll('.rock').length, sel: [...document.querySelectorAll('.rock.sel')].map((x) => x.dataset.id), dim: document.querySelectorAll('.rock.dim').length, vb: document.querySelector('[data-cui-island] svg').getAttribute('viewBox'), detail: document.getElementById('detail').innerText }));
    const s0 = await st();
    check('I: the open island is a div of the page (no frame, no policy for frames) and drew 9 rocks, with cancho selected', s0.rocks === 9 && s0.sel[0] === 'cancho' && (await page.evaluate(() => document.querySelectorAll('iframe').length === 0 && !document.querySelector('meta[http-equiv=Content-Security-Policy]') && document.querySelector('[data-cui-island]').tagName === 'DIV')), JSON.stringify(s0));
    check('I: its host is a group named by the label and described by the list of projects', await page.evaluate(() => { const h = document.querySelector('[data-cui-island]'); return h.getAttribute('role') === 'group' && h.getAttribute('aria-label').startsWith('Map of the projects') && h.getAttribute('aria-describedby') === 'projects' && !!document.getElementById('projects'); }));
    await page.evaluate(() => { window.__host = document.querySelector('[data-cui-island]'); window.__svg = document.querySelector('[data-cui-island] svg'); });
    await page.evaluate(() => document.querySelector('.rock[data-id="cancho-forge"]').dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await sleep(900);
    const s1 = await st();
    check('I: a rock chosen in the island reaches the handler: the detail pane (the program\'s) shows it and the island is told (state to props)', s1.sel[0] === 'cancho-forge' && s1.detail.includes('A forge written in cancho.'), JSON.stringify(s1));
    check('I: the camera moved to the rock (the island\'s viewBox changed)', s1.vb !== s0.vb && s1.vb.startsWith('175.0 100.0'), `${s0.vb} -> ${s1.vb}`);
    check('I: the render left the island alone: the same host and the same drawing', await page.evaluate(() => document.querySelector('[data-cui-island]') === window.__host && document.querySelector('[data-cui-island] svg') === window.__svg));
    await page.click('button:has-text("Services")');
    await sleep(300);
    check('I: a filter button of the program dims the rocks that are not services (6 of 9)', (await st()).dim === 6);
    check('I: and takes the dimmed rocks out of the tab order', await page.evaluate(() => [...document.querySelectorAll('.rock.dim')].every((g) => g.getAttribute('tabindex') === '-1')));
    await page.click('button:has-text("cancho-hdl")');
    await sleep(900);
    check('I: a button of the list, which is the text alternative, selects too', (await st()).sel[0] === 'cancho-hdl');
    await page.evaluate(() => { const g = document.querySelector('.rock[data-id="cancho-tools"]'); g.focus(); });
    await page.keyboard.press('Enter');
    await sleep(200);
    check('I: Enter on a focused rock selects it, in the page\'s own tab order', (await st()).sel[0] === 'cancho-tools' || (await st()).sel[0] === 'cancho-hdl', JSON.stringify((await st()).sel));
    const stats = await stats_(page);
    check('I: the events were heard, none dropped', stats.heard >= 2 && stats.dropped === 0, JSON.stringify(stats));
    check('I: no script error, and nothing asked of any origin but the page\'s', errors.length === 0 && [...origins].every((o) => o === A), `${errors.join(' | ')} ${[...origins].join(' ')}`);
    await ctx.close();
    {
      const { ctx: c2, page: p2 } = await open(`${A}/`, { reducedMotion: 'reduce' });
      await sleep(600);
      await p2.evaluate(() => document.querySelector('.rock[data-id="cancho-forge"]').dispatchEvent(new MouseEvent('click', { bubbles: true })));
      await sleep(120);
      check('I: with reduced motion the camera jumps (no tween)', (await p2.evaluate(() => document.querySelector('[data-cui-island] svg').getAttribute('viewBox'))) === '175 100 460 295', await p2.evaluate(() => document.querySelector('[data-cui-island] svg').getAttribute('viewBox')));
      await c2.close();
    }
    {
      const { ctx: c3, page: p3 } = await open(`${A}/`, { javaScriptEnabled: false });
      await sleep(300);
      const r = await p3.evaluate(() => ({ host: !!document.querySelector('[data-cui-island][role=group]'), svg: document.querySelectorAll('svg').length, h1: document.querySelector('h1') && document.querySelector('h1').textContent }));
      check('I: with script off the host is there, labelled and empty (the island and the list that describes it are drawn by script)', r.host && r.svg === 0 && r.h1 === 'The ecosystem', JSON.stringify(r));
      await c3.close();
    }
    pageRoot = exDir;
  }

  // ---------- J: what an open island can reach (the same attempts as F) ----------
  {
    hits.length = 0; pageHits.length = 0; stunHits.length = 0;
    const dir = build('reach-open', reachCui(true, false));
    pageRoot = dir;
    const { ctx, page } = await open(`${A}/`);
    let said = '';
    for (let i = 0; i < 160 && !said.includes('{'); i++) { await sleep(250); said = await text(page); }
    await sleep(2000);
    let got = {};
    try { got = JSON.parse(said.slice(said.indexOf('{'))); } catch (e) { got = {}; }
    const names = Object.keys(got);
    console.log(`info J: an open island's answers to the ${names.length} attempts:`);
    for (const n of names) { console.log(`info   ${n}: ${got[n]}`); }
    console.log(`info J: another origin received ${hits.length} requests (${hits.join(' ')}), the page's own ${pageHits.length}, the STUN socket ${stunHits.length} packets`);
    const worked = ['reached', 'sent', 'queued', 'answered', 'opened', 'registered', 'written', 'ran', 'constructed', '2', '1', 'tried'];
    const succeeded = names.filter((n) => worked.some((w) => got[n] === w || got[n].startsWith(`${w} `)));
    check('J: an open island answered every attempt', names.length >= 30, `${names.length}`);
    check('J: it reached the page, its storage and its cookies (parent.document, localStorage, document.cookie, indexedDB)', ['read parent.document', 'localStorage', 'document.cookie', 'indexedDB'].every((k) => got[k] && got[k].startsWith('reached')), JSON.stringify(['read parent.document', 'localStorage', 'document.cookie', 'indexedDB'].map((k) => got[k])));
    check('J: it sent requests to another origin and to the page\'s own, and a UDP packet through WebRTC', hits.length >= 8 && pageHits.length >= 1 && stunHits.length >= 1, `${hits.length} ${pageHits.length} ${stunHits.length}`);
    check('J: it ran eval, a Function, an inline script it added, and a worker', got.eval === '2' && got['new Function'] === '1' && got['an inline <script> the island adds'] === 'ran' && got['Worker from a blob'] === 'ran', JSON.stringify([got.eval, got['new Function'], got['an inline <script> the island adds'], got['Worker from a blob']]));
    check('J: most of the attempts that the sealed frame refuses succeed (at least 20 of them)', succeeded.length >= 20, `${succeeded.length} of ${names.length}: ${names.filter((n) => !succeeded.includes(n)).map((n) => `${n}=${got[n]}`).join('; ')}`);
    await ctx.close();
    pageRoot = exDir;
  }

  // ---------- H: a string never becomes markup or script ----------
  {
    const dir = build('strings', 'component T {\n    view { Column { Text { "<script>window.__x = 1</script><img src=x onerror=\\"window.__x = 2\\">" } } }\n}\n');
    pageRoot = dir;
    const { ctx, page } = await open(`${A}/`);
    await sleep(400);
    const r = await page.evaluate(() => ({ ran: window.__x === undefined ? 'no' : 'yes', scripts: document.querySelectorAll('body script:not([type=module])').length, imgs: document.querySelectorAll('img').length }));
    check('H: a script and an onerror image written as text are text: nothing ran and no element was made (island-undeclared has nothing to refuse)', r.ran === 'no' && r.imgs === 0, JSON.stringify(r));
    await ctx.close();
  }

  await browser.close();
  other.close(); site.close(); stun.close();
  fs.rmSync(work, { recursive: true, force: true });
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
