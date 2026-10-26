#!/usr/bin/env node
// A measurement, not a gate (docs/motion.md section 1): what the real cancho-www page does with its reveals, its count-ups and its reduced-motion rules,
// in Chromium (Playwright; the `Pixel 7` profile is a viewport and a user agent, not a phone). The page is read, never changed: pass the path of a clone.
//   node scripts/measure-motion.js /path/to/cancho-www
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium, devices } = require('playwright');

const www = path.resolve(process.argv[2] || '');
if (!fs.existsSync(path.join(www, 'index.html'))) { console.error('usage: node scripts/measure-motion.js <path of a cancho-www clone>'); process.exit(2); }
const watchdog = setTimeout(() => { console.log('FAIL watchdog'); process.exit(1); }, 5 * 60 * 1000);
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.png': 'image/png' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, () => ok(s));
  });
}
const PIXEL = devices['Pixel 7'];
const DESK = { viewport: { width: 1280, height: 800 } };

async function scrollAll(page) {
  await page.evaluate(async () => {
    const h = document.documentElement.scrollHeight;
    for (let y = 0; y < h; y += 200) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 30)); }
    scrollTo(0, h);
    await new Promise((r) => setTimeout(r, 1500));
  });
}

(async () => {
  const server = await serve(www);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  const ctxOf = (opts) => browser.newContext(opts);

  // 1. with JavaScript off
  {
    const ctx = await ctxOf({ ...PIXEL, javaScriptEnabled: false });
    const page = await ctx.newPage();
    await page.goto(url);
    const r = await page.evaluate(() => ({ reveal: document.querySelectorAll('.reveal').length, hidden: [...document.querySelectorAll('.reveal')].filter((e) => getComputedStyle(e).opacity !== '1').length, counts: [...document.querySelectorAll('[data-count]')].map((e) => e.textContent.trim()) }));
    console.log(`JS off: ${r.reveal} .reveal elements, ${r.hidden} not fully opaque; the three count-ups read: ${JSON.stringify(r.counts)}`);
    await ctx.close();
  }
  // 2. JS on, the page's own script blocked (the head's inline script has already put `js` on <html>)
  {
    const ctx = await ctxOf({ ...PIXEL });
    const page = await ctx.newPage();
    await page.route('**/assets/site.js', (r) => r.abort());
    await page.goto(url);
    await sleep(500);
    const r = await page.evaluate(() => ({ js: document.documentElement.classList.contains('js'), hidden: [...document.querySelectorAll('.reveal')].filter((e) => getComputedStyle(e).opacity === '0').length, total: document.querySelectorAll('.reveal').length }));
    console.log(`site.js blocked (inline head script ran): <html class="js"> ${r.js}; ${r.hidden} of ${r.total} .reveal elements have opacity 0 and nothing will ever show them`);
    await ctx.close();
  }
  // 3. normal and reduced motion, two sizes: what is hidden before scrolling, what is left hidden after scrolling the whole page, running animations, layout shift
  for (const [name, opts] of [['Pixel 7', PIXEL], ['desktop 1280', DESK]]) {
    for (const reduce of [false, true]) {
      const ctx = await ctxOf({ ...opts, reducedMotion: reduce ? 'reduce' : 'no-preference' });
      const page = await ctx.newPage();
      await page.addInitScript(() => { window.__cls = 0; new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__cls += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); });
      await page.goto(url);
      await sleep(1500);
      const before = await page.evaluate(() => ({ hidden: [...document.querySelectorAll('.reveal')].filter((e) => getComputedStyle(e).opacity < 0.99).length, anim: document.getAnimations().filter((a) => a.playState === 'running').length }));
      await scrollAll(page);
      const after = await page.evaluate(() => ({ left: [...document.querySelectorAll('.reveal')].filter((e) => !e.classList.contains('in')).map((e) => `${e.className} ${Math.round(e.getBoundingClientRect().height)}px`), cls: window.__cls, vh: innerHeight, counts: [...document.querySelectorAll('[data-count] span')].map((e) => e.textContent) }));
      console.log(`${name}, reduced motion ${reduce}: before scrolling ${before.hidden} .reveal not opaque, ${before.anim} running animations; after scrolling the whole page ${after.left.length} .reveal never revealed${after.left.length ? ' (' + after.left.join('; ') + ', window ' + after.vh + 'px)' : ''}; layout shift ${after.cls.toFixed(4)}; count-ups end at ${JSON.stringify(after.counts)}`);
      await ctx.close();
    }
  }
  // 4. what a screen reader is given while a number counts up
  {
    const ctx = await ctxOf({ ...PIXEL, reducedMotion: 'no-preference' });
    const page = await ctx.newPage();
    await page.goto(url);
    await sleep(800);
    await page.evaluate(() => document.querySelector('[data-count]').scrollIntoView({ block: 'center' }));
    await sleep(500);
    const mid = await page.evaluate(() => document.querySelector('[data-count]').textContent.trim());
    const ax = await page.accessibility.snapshot({ root: await page.$('[data-count]'), interestingOnly: false });
    await sleep(2000);
    const end = await page.evaluate(() => document.querySelector('[data-count]').textContent.trim());
    console.log(`count-up read mid-way (0.5 s in): DOM text "${mid}", accessibility tree ${JSON.stringify(ax && ax.children ? ax.children.map((c) => c.name) : ax && ax.name)}; after 2.5 s: "${end}"`);
    await ctx.close();
  }
  await browser.close();
  server.close();
  clearTimeout(watchdog);
})();
