#!/usr/bin/env node
// Reveal, CountUp and `visible:` in Chromium (docs/motion.md): DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   A. with JavaScript off the page's HTML has every block, every final number and every section, none hidden, no state of the script in it;
//   B. a browser with no IntersectionObserver, and a script that never runs: nothing is hidden, the numbers are final;
//   C. normal motion: blocks below the window are hidden until seen, every block is seen by scrolling (a block four times taller than the window
//      too), and the layout shifts by 0 the whole way;
//   D. reduced motion: nothing is armed, nothing is hidden, no animation runs, no number counts, and the stylesheet alone shows an armed block;
//   E. a count-up: the text of the page is the final number before, during and after (DOM and accessibility tree), the frames are drawn by CSS
//      with an empty alternative text, and the frame is removed at the end; without the CSS feature nothing counts;
//   F. `visible:` and `hidden:`: an event when half the section is seen and when it is left, again on a second visit, one told before `app.js`
//      had run is not lost, a section taller than the window fires, and the page does not loop;
//   G. the static nodes are the same nodes after `app.js` ran; print shows every block; no request to another host, no console error.
//   node scripts/check-motion.js
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
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
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'motion-'));
let holdApp = 0;
function build(name, file) {
  const out = path.join(work, name);
  const r = spawnSync(BIN, ['build', file, '--page', '-o', out], { encoding: 'utf8', cwd: ROOT });
  if (r.status !== 0) { console.log(`FAIL build ${name}: ${r.stderr}${r.stdout}`); process.exit(1); }
  return out;
}
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      const send = () => fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
      if (f.endsWith('app.js') && holdApp) { setTimeout(send, holdApp); } else { send(); }
    }).listen(0, () => ok(s));
  });
}
const PIXEL = devices['Pixel 7'];
const DESK = { viewport: { width: 1280, height: 800 } };
const NOPREF = { reducedMotion: 'no-preference' };

(async () => {
  const site = build('site', path.join(ROOT, 'examples', 'motion', 'motion.cui'));
  const server = await serve(site);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  const external = [];
  async function open(opts, query = '', init) {
    const ctx = await browser.newContext(opts);
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    await page.route('**/*', (r) => { if (r.request().url().startsWith('http://127.0.0.1')) { r.continue(); } else { external.push(r.request().url()); r.abort(); } });
    await page.addInitScript(() => { window.__cls = 0; try { new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__cls += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); } catch (e) { /* none */ } });
    if (init) { await page.addInitScript(init); }
    await page.goto(url + query);
    await sleep(400);
    return { ctx, page, errors };
  }
  const opacity = (page) => page.evaluate(() => [...document.querySelectorAll('.cui-reveal')].map((e) => Number(getComputedStyle(e).opacity)));
  const scrollAll = (page) => page.evaluate(async () => {
    const h = document.documentElement.scrollHeight;
    for (let y = 0; y < h; y += 150) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 25)); }
    await new Promise((r) => setTimeout(r, 1200));
  });

  // ---- A. JavaScript off -----------------------------------------------------------------------------------------------------------------
  {
    const html = await (await fetch(url)).text();
    check('A the served HTML has the three blocks, the final numbers and the section, and none of the script\'s own state', (html.match(/class="cui-reveal"/g) || []).length === 3 && /data-to="704" class="cui-countup">704</.test(html) && /data-to="333"[^>]*>333</.test(html) && /data-to="62"[^>]*>62</.test(html) && /data-cui-visible="Seen"/.test(html) && !/cui-armed|cui-in|data-frame/.test(html.split('<body>')[1].split('<script>')[0]), `${(html.match(/class="cui-reveal"/g) || []).length} blocks`);
    for (const [name, opts] of [['Pixel 7', PIXEL], ['desktop', DESK]]) {
      const { ctx, page, errors } = await open({ ...opts, javaScriptEnabled: false });
      const o = await opacity(page);
      const text = await page.evaluate(() => document.body.innerText);
      const vis = await page.evaluate(() => [...document.querySelectorAll('.cui-reveal')].every((e) => getComputedStyle(e).visibility === 'visible' && e.getBoundingClientRect().height > 20));
      check(`A ${name}, script off: every block is opaque and has a height, the text is there, the numbers are final`, o.length === 3 && o.every((x) => x === 1) && vis && /Conformance: 704 of 704, refusals 333\./.test(text) && /Tall line 100\./.test(text) && /Rules: 62/.test(text), JSON.stringify(o));
      check(`A ${name}, script off: no console error`, errors.length === 0, errors.join(' | '));
      await ctx.close();
    }
  }

  // ---- B. no IntersectionObserver ---------------------------------------------------------------------------------------------------------
  {
    const { ctx, page } = await open({ ...PIXEL, ...NOPREF }, '', () => { delete window.IntersectionObserver; });
    const o = await opacity(page);
    const r = await page.evaluate(() => ({ armed: document.querySelectorAll('.cui-armed').length, frame: document.querySelectorAll('[data-frame]').length, count: document.querySelector('.cui-countup').textContent, text: document.body.innerText.match(/Seen (\d+) times/)[1] }));
    check('B no IntersectionObserver: nothing armed, nothing hidden, the numbers final, the section told once', o.every((x) => x === 1) && r.armed === 0 && r.frame === 0 && r.count === '704' && r.text === '1', JSON.stringify(r));
    await ctx.close();
  }

  // ---- C. normal motion ---------------------------------------------------------------------------------------------------------------------
  for (const [name, opts] of [['Pixel 7', PIXEL], ['desktop', DESK]]) {
    const { ctx, page, errors } = await open({ ...opts, ...NOPREF });
    const before = await page.evaluate(() => ({ armed: document.querySelectorAll('.cui-reveal.cui-armed').length, hidden: [...document.querySelectorAll('.cui-reveal')].filter((e) => getComputedStyle(e).opacity === '0').length, vh: innerHeight, tall: document.getElementById('tall').getBoundingClientRect().height }));
    check(`C ${name}: the three blocks are armed and the ones below the window are hidden until seen`, before.armed === 3 && before.hidden >= 1, JSON.stringify(before));
    if (name === 'Pixel 7') { check('C Pixel 7: the tall block is more than four windows tall, so a quarter of it alone can never be inside the window', before.tall * 0.25 > before.vh, `${Math.round(before.tall)} px against ${before.vh}`); }
    await scrollAll(page);
    const after = await page.evaluate(() => ({ left: document.querySelectorAll('.cui-reveal:not(.cui-in)').length, o: [...document.querySelectorAll('.cui-reveal')].map((e) => Number(getComputedStyle(e).opacity)), cls: window.__cls }));
    check(`C ${name}: scrolling through the page shows every block, the tall one too`, after.left === 0 && after.o.every((x) => x === 1), JSON.stringify(after));
    check(`C ${name}: the layout shifts by 0 from load to the end`, after.cls === 0, String(after.cls));
    check(`C ${name}: no console error`, errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- C2. a block built by script is never hidden ------------------------------------------------------------------------------------------------
  {
    const f = path.join(work, 'built.cui');
    fs.writeFileSync(f, 'component B {\n state shown: Bool = false;\n on Show { shown = true; }\n view { Column { Text { "Static text so the page is hydrated." } Button { label: "Show" on: Show } If { when: shown Reveal { Text { "Built by script." } } } } }\n}\n');
    const bs = await serve(build('built', f));
    const ctx = await browser.newContext({ ...PIXEL, ...NOPREF });
    const page = await ctx.newPage();
    await page.goto(`http://127.0.0.1:${bs.address().port}/`);
    await sleep(300);
    await page.click('button');
    await sleep(300);
    const r = await page.evaluate(() => { const e = document.querySelector('.cui-reveal'); return { there: !!e, armed: e && e.classList.contains('cui-armed'), o: e && Number(getComputedStyle(e).opacity) }; });
    check('C a block that script builds is shown at once and never armed', r.there && r.armed === false && r.o === 1, JSON.stringify(r));
    await ctx.close();
    bs.close();
  }

  // ---- D. reduced motion --------------------------------------------------------------------------------------------------------------------
  for (const [name, opts] of [['Pixel 7', PIXEL], ['desktop', DESK]]) {
    const { ctx, page } = await open({ ...opts, reducedMotion: 'reduce' });
    await page.evaluate(() => document.getElementById('top').scrollIntoView());
    const seenFrame = [];
    for (let i = 0; i < 8; i++) { seenFrame.push(await page.evaluate(() => document.querySelectorAll('[data-frame]').length)); await sleep(250); }
    const r = await page.evaluate(() => ({ armed: document.querySelectorAll('.cui-armed').length, o: [...document.querySelectorAll('.cui-reveal')].map((e) => Number(getComputedStyle(e).opacity)), anim: document.getAnimations().filter((a) => a.playState === 'running').length, counts: [...document.querySelectorAll('.cui-countup')].map((e) => e.textContent) }));
    check(`D ${name}, reduced motion: nothing armed, nothing hidden, no animation running, no frame ever drawn, numbers final`, r.armed === 0 && r.o.every((x) => x === 1) && r.anim === 0 && seenFrame.every((n) => n === 0) && r.counts.join() === '704,333,62', JSON.stringify(r));
    // the stylesheet alone: arm every block by hand, as a script that mis-detected the setting would
    const forced = await page.evaluate(() => { document.querySelectorAll('.cui-reveal').forEach((e) => e.classList.add('cui-armed')); return [...document.querySelectorAll('.cui-reveal')].map((e) => Number(getComputedStyle(e).opacity)); });
    check(`D ${name}: an armed block is opaque under reduced motion by the stylesheet alone`, forced.every((x) => x === 1), JSON.stringify(forced));
    await ctx.close();
  }

  // ---- E. a count-up -----------------------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page } = await open({ ...PIXEL, ...NOPREF });
    const para = await page.$('#top p');
    const axText = async () => { const s = await page.accessibility.snapshot({ root: para, interestingOnly: false }); const out = []; (function w(n) { if (n.name && (n.role === 'StaticText' || n.role === 'text')) { out.push(n.name); } (n.children || []).forEach(w); })(s); return out.join(''); };
    const samples = [];
    for (const t of [0, 250, 450, 600, 900, 1500]) {
      await sleep(t === 0 ? 0 : 150);
      samples.push(await page.evaluate(() => { const e = document.querySelector('.cui-countup'); return { dom: e.textContent, frame: e.getAttribute('data-frame'), after: getComputedStyle(e, '::after').content, color: getComputedStyle(e).color }; }));
    }
    const ax = await axText();
    check('E the text of the page is the final number the whole time (DOM)', samples.every((s) => s.dom === '704'), JSON.stringify(samples.map((s) => s.dom)));
    const mid = samples.filter((s) => s.frame !== null);
    check('E while it counts a frame is drawn by CSS over the text, with an empty alternative text', mid.length > 0 && mid.every((s) => /\/ ""/.test(s.after) && s.color === 'rgba(0, 0, 0, 0)') && new Set(mid.map((s) => s.frame)).size > 1, JSON.stringify(mid.slice(0, 3)));
    check('E the accessibility tree reads the paragraph with the final numbers and no frame, mid-way', /^Conformance: 704 of 704, refusals 333\.$/.test(ax), ax);
    await sleep(1500);
    const end = await page.evaluate(() => { const e = document.querySelector('.cui-countup'); return { frame: e.getAttribute('data-frame'), style: e.getAttribute('style'), color: getComputedStyle(e).color, dom: e.textContent }; });
    check('E at the end the frame, the colour trick and the style are gone and the text is the number', end.frame === null && !end.style && end.color !== 'rgba(0, 0, 0, 0)' && end.dom === '704', JSON.stringify(end));
    await ctx.close();
    const nf = await open({ ...PIXEL, ...NOPREF }, '', () => { CSS.supports = () => false; });
    await sleep(800);
    const f = await nf.page.evaluate(() => ({ frames: document.querySelectorAll('[data-frame]').length, t: document.querySelector('.cui-countup').textContent }));
    check('E a browser without the alternative-text feature of `content` counts nothing and shows the number', f.frames === 0 && f.t === '704', JSON.stringify(f));
    await nf.ctx.close();
  }

  // ---- F. visible and hidden -----------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open({ ...PIXEL, ...NOPREF });
    const counts = () => page.evaluate(() => { const m = document.body.innerText.match(/Seen (\d+) times, left (\d+) times/); return m ? [Number(m[1]), Number(m[2])] : null; });
    check('F nothing is told while the section is below the window', JSON.stringify(await counts()) === '[0,0]', JSON.stringify(await counts()));
    await page.evaluate(() => document.getElementById('watched').scrollIntoView({ block: 'start' }));
    await sleep(500);
    check('F the section is told it was seen when half of it is', JSON.stringify(await counts()) === '[1,0]', JSON.stringify(await counts()));
    await page.evaluate(() => scrollTo(0, 0));
    await sleep(500);
    check('F and that it was left when it is out of sight', JSON.stringify(await counts()) === '[1,1]', JSON.stringify(await counts()));
    await page.evaluate(() => document.getElementById('watched').scrollIntoView({ block: 'start' }));
    await sleep(500);
    check('F a second visit is a second event', JSON.stringify(await counts()) === '[2,1]', JSON.stringify(await counts()));
    await sleep(1500);
    check('F the page does not loop: the counts hold', JSON.stringify(await counts()) === '[2,1]', JSON.stringify(await counts()));
    check('F no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
    // seen before app.js ran
    holdApp = 1500;
    const early = await open({ ...PIXEL, ...NOPREF }, '#watched');
    await sleep(2500);
    const c = await early.page.evaluate(() => document.body.innerText.match(/Seen (\d+) times/)[1]);
    check('F a section on the screen before app.js ran is told once when it has', c === '1', c);
    holdApp = 0;
    await early.ctx.close();
    // a section taller than the window
    const tall = path.join(work, 'tallvis.cui');
    const lines = Array.from({ length: 100 }, (_, i) => `Text { "Line ${i}. A sentence long enough to wrap on a phone and take real height." }`).join(' ');
    fs.writeFileSync(tall, `component T {\n state n: Int = 0;\n on Seen { n = n + 1; }\n view { Column { Section { id: "spacer" ${lines} } Section { id: "tall" visible: Seen ${lines} } Text { "Seen {n} times" } } }\n}\n`);
    const ts = await serve(build('tall', tall));
    const ctx2 = await browser.newContext({ ...PIXEL, ...NOPREF });
    const p2 = await ctx2.newPage();
    await p2.goto(`http://127.0.0.1:${ts.address().port}/`);
    await sleep(300);
    const info = await p2.evaluate(() => { const e = document.getElementById('tall'); const r = e.getBoundingClientRect(); const top = scrollY + r.top; scrollTo(0, top - innerHeight + innerHeight * 0.6); return { h: r.height, vh: innerHeight }; });
    await sleep(500);
    const n = await p2.evaluate(() => document.body.innerText.match(/Seen (\d+) times/)[1]);
    check('F a section more than four windows tall is told it was seen once half a window of it is', n === '1' && info.h * 0.5 > info.vh * 2, `${Math.round(info.h)} px, window ${info.vh}, seen ${n}`);
    await ctx2.close();
    ts.close();
  }

  // ---- G. nothing is touched, print, the network ------------------------------------------------------------------------------------------------
  {
    holdApp = 1500;
    const ctx = await browser.newContext({ ...PIXEL, ...NOPREF });
    const page = await ctx.newPage();
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    await page.evaluate(() => { window.__nodes = [...document.querySelectorAll('.cui-reveal, .cui-countup, #watched, #top')]; });
    await page.waitForFunction(() => /Seen \d/.test(document.body.innerText), null, { timeout: 10000 });
    const same = await page.evaluate(() => window.__nodes.length === 8 && window.__nodes.every((n) => n.isConnected && document.contains(n)));
    check('G the blocks, the numbers and the sections are the same nodes after app.js ran', same);
    holdApp = 0;
    await ctx.close();
    const pr = await open({ ...PIXEL, ...NOPREF });
    await pr.page.emulateMedia({ media: 'print' });
    const o = await opacity(pr.page);
    check('G in print every block is shown', o.every((x) => x === 1), JSON.stringify(o));
    await pr.ctx.close();
  }
  check('no request went to another host', external.length === 0, external.join(', '));
  await browser.close();
  server.close();
  clearTimeout(watchdog);
  fs.rmSync(work, { recursive: true, force: true });
  if (failures.length) { console.log(`FAIL ${failures.length}: ${failures.join('; ')}`); process.exit(1); }
  console.log('PASS');
})();
