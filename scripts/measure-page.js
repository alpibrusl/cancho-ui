#!/usr/bin/env node
// A measurement, not a gate (docs/page.md section 1): what the real cancho-www page does about its skip link, its sticky header and its in-page
// anchors, in Chromium (Playwright; the `Pixel 7` profile is a viewport and a user agent, not a phone). The page is read, never changed: pass the
// path of a clone.
//   node scripts/measure-page.js /path/to/cancho-www
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium, devices } = require('playwright');

const www = path.resolve(process.argv[2] || '');
if (!fs.existsSync(path.join(www, 'index.html'))) { console.error('usage: node scripts/measure-page.js <path of a cancho-www clone>'); process.exit(2); }
const watchdog = setTimeout(() => { console.log('FAIL watchdog'); process.exit(1); }, 5 * 60 * 1000);

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

// For every in-page link of the nav and the page: follow it and report how far below the header's bottom edge the target's top lands.
async function anchors(page) {
  return page.evaluate(async () => {
    const hdr = document.querySelector('header');
    const ids = [...new Set([...document.querySelectorAll('a[href^="#"]')].map((a) => a.getAttribute('href').slice(1)).filter((i) => i && document.getElementById(i)))];
    const rows = [];
    for (const id of ids) {
      location.hash = '';
      window.scrollTo(0, 0);
      location.hash = id;
      await new Promise((r) => setTimeout(r, 120));
      const t = document.getElementById(id).getBoundingClientRect().top;
      rows.push({ id, top: Math.round(t), header: Math.round(hdr.getBoundingClientRect().bottom), maxed: Math.round(window.scrollY) >= document.documentElement.scrollHeight - innerHeight - 1 });
    }
    return rows;
  });
}

(async () => {
  const server = await serve(www);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  for (const [name, ctxOpts] of [['desktop 1280', { viewport: { width: 1280, height: 800 } }], ['Pixel 7', { ...devices['Pixel 7'] }]]) {
    for (const padding of ['the page\'s own scroll-padding-top (5rem)', 'scroll-padding-top removed']) {
      const ctx = await browser.newContext({ ...ctxOpts, reducedMotion: 'reduce' });
      const page = await ctx.newPage();
      await page.route('**/*', (r) => (r.request().url().startsWith(url) ? r.continue() : r.abort()));
      await page.goto(url);
      await page.waitForTimeout(500);
      if (padding.includes('removed')) { await page.addStyleTag({ content: 'html { scroll-padding-top: 0 !important; }' }); }
      const hdr = await page.evaluate(() => { const h = document.querySelector('header'); const r = h.getBoundingClientRect(); return { height: Math.round(r.height), position: getComputedStyle(h).position, padTop: getComputedStyle(document.documentElement).scrollPaddingTop }; });
      const rows = await anchors(page);
      const hidden = rows.filter((r) => r.top < r.header && !r.maxed);
      console.log(`${name}, ${padding}: header ${hdr.height} px (${hdr.position}), ${rows.length} anchors, ${hidden.length} with the target's top above the header's bottom${hidden.length ? ' (' + hidden.map((r) => `#${r.id} top ${r.top} < ${r.header}`).join(', ') + ')' : ''}; tops: ${[...new Set(rows.filter((r) => !r.maxed).map((r) => r.top))].join(',')}`);
      await ctx.close();
    }
  }
  // the skip link: is it the first focusable element, where does Enter on it take the page, and where does the next Tab go
  for (const [name, ctxOpts] of [['desktop 1280', { viewport: { width: 1280, height: 800 } }], ['Pixel 7', { ...devices['Pixel 7'] }]]) {
    const ctx = await browser.newContext({ ...ctxOpts, reducedMotion: 'reduce' });
    const page = await ctx.newPage();
    await page.goto(url);
    await page.waitForTimeout(500);
    await page.keyboard.press('Tab');
    const first = await page.evaluate(() => { const a = document.activeElement; const r = a.getBoundingClientRect(); return { tag: a.tagName, cls: a.className, text: a.textContent.trim(), top: Math.round(r.top), visible: r.top >= 0 }; });
    await page.keyboard.press('Enter');
    await page.waitForTimeout(200);
    const after = await page.evaluate(() => ({ hash: location.hash, active: document.activeElement.tagName + (document.activeElement.id ? '#' + document.activeElement.id : ''), mainTop: Math.round(document.getElementById('main').getBoundingClientRect().top), tabindex: document.getElementById('main').getAttribute('tabindex') }));
    await page.keyboard.press('Tab');
    const next = await page.evaluate(() => { const a = document.activeElement; return { tag: a.tagName, text: (a.textContent || '').trim().slice(0, 30), inMain: !!a.closest('main'), inHeader: !!a.closest('header') }; });
    console.log(`${name}: the first Tab focuses <${first.tag} class="${first.cls}"> "${first.text}" at top ${first.top}; after Enter: hash ${after.hash}, active ${after.active}, main's top ${after.mainTop}, main tabindex ${after.tabindex}; the next Tab: <${next.tag}> "${next.text}" in main ${next.inMain}, in header ${next.inHeader}`);
    await ctx.close();
  }
  // keyboard focus under a sticky header: Tab down the page and report the links whose top edge is above the header's bottom after the browser scrolled them into view
  for (const padding of ['own scroll-padding-top', 'removed']) {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], reducedMotion: 'reduce' });
    const page = await ctx.newPage();
    await page.goto(url);
    await page.waitForTimeout(500);
    if (padding === 'removed') { await page.addStyleTag({ content: 'html { scroll-padding-top: 0 !important; }' }); }
    let obscured = 0; let n = 0;
    for (let i = 0; i < 120; i++) {
      await page.keyboard.press('Tab');
      const r = await page.evaluate(() => { const a = document.activeElement; const b = a.getBoundingClientRect(); const h = document.querySelector('header').getBoundingClientRect(); return { cover: !a.closest('header') && b.top < h.bottom && b.bottom > 0, inview: b.bottom > 0 && b.top < innerHeight }; });
      n++; if (r.cover) { obscured++; }
    }
    console.log(`Pixel 7, ${padding}: ${n} Tab presses, ${obscured} left the focused element under the header`);
    await ctx.close();
  }
  await browser.close();
  server.close();
  clearTimeout(watchdog);
})();
