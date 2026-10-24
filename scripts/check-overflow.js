#!/usr/bin/env node
// No page scrolls sideways on a narrow phone (alpibrusl/cancho-ui#144). Every gallery demo is compiled with `build --page` into a temporary
// directory (so a change to the baseline stylesheet in the compiler is what is measured, not a file on the disk) and opened in Chromium with the
// `Pixel 7` profile at 320, 360, 375, 380 and 412 px wide. DESKTOP CHROMIUM, not a phone: it measures the layout, not any device. For each
// width, after the page has settled and again 1 s later (a page can change after it loads), the page's scroll width must not exceed its client
// width. A failure names the demo, the width, the overflow in px and the widest elements that stick out. Part of a table that scrolls inside its
// own box is not a page overflow, and is not counted here. Watchdog: 5 minutes.
//   node scripts/check-overflow.js
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { chromium, devices } = require('playwright');
const { loadManifest, buildDemos, ROOT } = require('./build-gallery.js');

const WIDTHS = [320, 360, 375, 380, 412];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.json': 'application/json', '.webmanifest': 'application/manifest+json' };
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

(async () => {
  const m = loadManifest();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'overflow-'));
  buildDemos(m, path.join(ROOT, 'target', 'debug', 'cancho-ui'), dir);
  const server = http.createServer((q, r) => {
    let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
    if (f.endsWith(path.sep) || (fs.existsSync(f) && fs.statSync(f).isDirectory())) { f = path.join(f, 'index.html'); }
    fs.readFile(f, (e, d) => {
      if (e) { r.statusCode = 404; r.end(); return; }
      r.setHeader('content-type', TYPES[path.extname(f)] || 'application/octet-stream');
      r.end(d);
    });
  });
  await new Promise((ok) => server.listen(0, ok));
  const port = server.address().port;
  const browser = await chromium.launch();
  let measured = 0;
  for (const t of m.supported) {
    for (const w of WIDTHS) {
      const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: w, height: 800 }, serviceWorkers: 'block' });
      const page = await ctx.newPage();
      await page.goto(`http://localhost:${port}/${t.id}/`);
      await sleep(1200);
      const worst = { px: 0, detail: '' };
      for (let i = 0; i < 2; i++) {
        const o = await page.evaluate(() => {
          const d = document.documentElement;
          const out = [...document.querySelectorAll('body *')].map((e) => ({ e, r: e.getBoundingClientRect() }))
            .filter((x) => x.r.right > d.clientWidth + 1 && x.r.width > 0)
            .sort((a, b) => b.r.right - a.r.right).slice(0, 3)
            .map((x) => `${x.e.tagName.toLowerCase()}${x.e.className ? '.' + String(x.e.className).split(' ')[0] : ''} right=${Math.round(x.r.right)}`);
          return { px: d.scrollWidth - d.clientWidth, widest: out };
        });
        if (o.px > worst.px) { worst.px = o.px; worst.detail = o.widest.join('; '); }
        if (i === 0) { await sleep(1000); }
      }
      measured += 1;
      check(`${t.id} at ${w} px has no horizontal page overflow`, worst.px <= 0, worst.px > 0 ? `${worst.px} px (${worst.detail})` : '');
      await ctx.close();
    }
  }
  // The ops console's table scrolls inside its own box at 320 px, and that box is a region a keyboard can reach and scroll.
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 320, height: 800 }, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.goto(`http://localhost:${port}/ops-console/`);
    await sleep(1200);
    const box = page.getByRole('region', { name: 'Machines', exact: true });
    check('ops-console at 320 px: the table sits in a region named like the table', (await box.count()) === 1 && (await page.getByRole('table', { name: 'Machines', exact: true }).count()) === 1);
    const o = await box.evaluate((e) => ({ scrolls: e.scrollWidth > e.clientWidth, tab: e.getAttribute('tabindex'), over: getComputedStyle(e).overflowX }));
    check('ops-console at 320 px: the box scrolls sideways and is focusable (tabindex 0)', o.scrolls && o.tab === '0' && o.over === 'auto', JSON.stringify(o));
    await box.focus();
    await page.keyboard.press('ArrowRight');
    await sleep(300);
    const left = await box.evaluate((e) => e.scrollLeft);
    check('ops-console at 320 px: the right arrow scrolls the focused box', left > 0, `scrollLeft ${left}`);
    const rows = await page.getByRole('row').count();
    check('ops-console at 320 px: the table keeps its rows in the accessibility tree', rows >= 5, `${rows} rows`);
    await ctx.close();
  }
  await browser.close();
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} of ${measured} measurements failed` : `\nall ${measured} measurements passed (${m.supported.length} demos x ${WIDTHS.length} widths)`);
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
