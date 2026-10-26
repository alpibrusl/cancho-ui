#!/usr/bin/env node
// What the port costs against the page, in Chromium (DESKTOP CHROMIUM with the Pixel 7 profile and a desktop window, on this machine, 5 runs each, medians):
// bytes of the HTML and of the script (raw and gzip), requests and bytes by kind, frames, DOM nodes, JavaScript heap, first and largest contentful paint, layout shift,
// long tasks, load. Not part of the gate (it needs the reference clone, and timings are of this machine):
//   node scripts/measure-www-port.js <path to a cancho-www clone> [--write]       (--write rewrites docs/www-port/cost.md)
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const zlib = require('zlib');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const REF = path.resolve(process.argv[2] || '');
const WRITE = process.argv.includes('--write');
if (!process.argv[2] || !fs.existsSync(path.join(REF, 'index.html'))) { console.error('usage: node scripts/measure-www-port.js <cancho-www clone> [--write]'); process.exit(2); }
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'measure-'));
const out = path.join(work, 'port');
const b = spawnSync(path.join(ROOT, 'target', 'debug', 'cancho-ui'), ['build', 'docs/www-port/site/home.cui', '--page', '-o', out], { cwd: ROOT, encoding: 'utf8' });
if (b.status !== 0) { console.error(b.stderr + b.stdout); process.exit(1); }
const refSha = spawnSync('git', ['-C', REF, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.md': 'text/markdown', '.json': 'application/json', '.png': 'image/png', '.webp': 'image/webp', '.jpg': 'image/jpeg', '.woff2': 'font/woff2', '.mp4': 'video/mp4', '.webm': 'video/webm' };
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', TYPES[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, '127.0.0.1', () => ok(s));
  });
}
const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
const gz = (p) => zlib.gzipSync(fs.readFileSync(p), { level: 9 }).length;

(async () => {
  const servers = { page: await serve(REF), port: await serve(out) };
  const browser = await chromium.launch();
  const rows = {};
  for (const [pname, opts] of [['Pixel 7', { ...devices['Pixel 7'] }], ['desktop', { viewport: { width: 1280, height: 900 } }]]) {
    for (const side of ['page', 'port']) {
      const runs = [];
      for (let i = 0; i < 5; i++) {
        const ctx = await browser.newContext({ ...opts, reducedMotion: 'no-preference', serviceWorkers: 'block' });
        const page = await ctx.newPage();
        const reqs = [];
        page.on('response', async (r) => { try { const b = await r.body(); reqs.push({ url: r.url(), type: path.extname(new URL(r.url()).pathname) || '.html', bytes: b.length }); } catch (e) { /* aborted */ } });
        await page.addInitScript(() => {
          window.__m = { cls: 0, lcp: 0, fcp: 0, long: 0, longMs: 0 };
          new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__m.cls += e.value; } } }).observe({ type: 'layout-shift', buffered: true });
          new PerformanceObserver((l) => { for (const e of l.getEntries()) { window.__m.lcp = e.startTime; } }).observe({ type: 'largest-contentful-paint', buffered: true });
          new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (e.name === 'first-contentful-paint') { window.__m.fcp = e.startTime; } } }).observe({ type: 'paint', buffered: true });
          window.__m.rocks24 = 0;
          new MutationObserver(() => { if (!window.__m.rocks24 && document.querySelectorAll('#ecosystem .rock').length >= 24) { window.__m.rocks24 = performance.now(); } }).observe(document, { childList: true, subtree: true });
          new PerformanceObserver((l) => { for (const e of l.getEntries()) { window.__m.long++; window.__m.longMs += e.duration - 50; } }).observe({ type: 'longtask', buffered: true });
        });
        await page.goto(`http://127.0.0.1:${servers[side].address().port}/`, { waitUntil: 'load' });
        await page.waitForTimeout(2500);
        const cdp = await ctx.newCDPSession(page);
        await cdp.send('Performance.enable');
        const pm = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));
        const m = await page.evaluate(() => ({ ...window.__m, frames: document.querySelectorAll('iframe').length, nodes: document.getElementsByTagName('*').length, load: performance.timing.loadEventEnd - performance.timing.navigationStart, dcl: performance.timing.domContentLoadedEventEnd - performance.timing.navigationStart }));
        const map = await page.evaluate(async () => {
          const eco = document.getElementById('ecosystem');
          const rock = eco.querySelector('.rock[data-id="cancho-web"]');
          const nodes = eco.getElementsByTagName('*').length;
          const rocks = eco.querySelectorAll('.rock').length;
          const t0 = performance.now();
          rock.dispatchEvent(new MouseEvent('click', { bubbles: true }));
          let t1 = null;
          for (let i = 0; i < 400 && t1 === null; i++) { const h = document.querySelector('#detail h3'); if (h && h.textContent.trim() === 'cancho-web') { t1 = performance.now(); } else { await new Promise((r) => requestAnimationFrame(r)); } }
          return { nodes, rocks, click: t1 === null ? -1 : t1 - t0, drawn: window.__m.rocks24 };
        });
        runs.push({ ...m, map, heap: pm.JSHeapUsedSize, requests: reqs.length, bytes: reqs.reduce((a, r) => a + r.bytes, 0), byType: reqs.reduce((a, r) => { a[r.type] = (a[r.type] || 0) + r.bytes; return a; }, {}) });
        await ctx.close();
      }
      rows[`${pname} ${side}`] = runs;
    }
  }
  await browser.close();
  Object.values(servers).forEach((s) => s.close());
  const f = (k) => (runs) => median(runs.map((r) => r[k]));
  const kb = (n) => `${(n / 1024).toFixed(1)} kB`;
  const md = [];
  md.push('# What the port costs against the page, measured', '');
  md.push(`> Generated by \`node scripts/measure-www-port.js <clone> --write\` against cancho-www \`${refSha}\`. Chromium (Playwright), this machine, local static server with no compression and no cache, 5 runs each, **medians**; the timings are this machine's and are good for comparing the two pages, not as numbers of a device or a network. Normal motion.`, '');
  md.push('## Files', '');
  const sizes = (dir, names) => names.map((n) => `${n}: ${kb(fs.statSync(path.join(dir, n)).size)} (gzip ${kb(gz(path.join(dir, n)))})`).join('; ');
  md.push(`* **page**: ${sizes(REF, ['index.html', 'assets/site.js', 'assets/map.js', 'assets/rocks.js', 'assets/scene.js', 'assets/agent-files.js', 'assets/site.css'])}`);
  md.push(`* **port**: ${sizes(out, ['index.html', 'app.js'])}`, '');
  md.push('## A load, 5 runs, medians', '', '| | requests | bytes | HTML + script | iframes | DOM nodes | JS heap | FCP | LCP | CLS | long tasks (ms over 50) | DOMContentLoaded | load |', '|---|---|---|---|---|---|---|---|---|---|---|---|---|');
  for (const [k, runs] of Object.entries(rows)) {
    const html = median(runs.map((r) => r.byType['.html'] || 0));
    const js = median(runs.map((r) => r.byType['.js'] || 0));
    md.push(`| ${k} | ${f('requests')(runs)} | ${kb(f('bytes')(runs))} | ${kb(html)} + ${kb(js)} | ${f('frames')(runs)} | ${f('nodes')(runs)} | ${kb(f('heap')(runs))} | ${f('fcp')(runs).toFixed(0)} ms | ${f('lcp')(runs).toFixed(0)} ms | ${f('cls')(runs).toFixed(4)} | ${f('long')(runs)} (${f('longMs')(runs).toFixed(0)}) | ${f('dcl')(runs).toFixed(0)} ms | ${f('load')(runs).toFixed(0)} ms |`);
  }
  md.push('', '## The map alone, medians of the same runs', '', 'The ecosystem section (`#ecosystem`): its elements, the rocks drawn, the time from the start of the navigation until the 24th rock exists, and the time from a click on a rock until the detail pane shows its name (in `requestAnimationFrame` steps, so 16 ms is the floor).', '', '| | elements in #ecosystem | rocks | the 24th rock exists at | click on a rock to its detail |', '|---|---|---|---|---|');
  for (const [k, runs] of Object.entries(rows)) {
    md.push(`| ${k} | ${median(runs.map((r) => r.map.nodes))} | ${median(runs.map((r) => r.map.rocks))} | ${median(runs.map((r) => r.map.drawn)).toFixed(0)} ms | ${median(runs.map((r) => r.map.click)).toFixed(0)} ms |`);
  }
  md.push('### Where the map\'s bytes are', '');
  md.push('* **the page**: `assets/map.js` ' + kb(fs.statSync(path.join(REF, 'assets', 'map.js')).size) + ', `assets/rocks.js` ' + kb(fs.statSync(path.join(REF, 'assets', 'rocks.js')).size) + ' (the hero uses it too), and two requests for `data/ecosystem.json` ' + kb(fs.statSync(path.join(REF, 'data', 'ecosystem.json')).size) + ' and `data/demos.json` ' + kb(fs.statSync(path.join(REF, 'data', 'demos.json')).size) + '; the map\'s rules are in `site.css`.');
  md.push('* **the port**: the island\'s own `islands/map.js` ' + kb(fs.statSync(path.join(ROOT, 'docs', 'www-port', 'site', 'islands', 'map.js')).size) + ' and `islands/map.css` ' + kb(fs.statSync(path.join(ROOT, 'docs', 'www-port', 'site', 'islands', 'map.css')).size) + ' (inside `app.js`), the `places` JSON the program hands the island (in the HTML), and the **program\'s own code for the detail pane**: the `For` over the 24 projects and the four trail panels are unrolled at build time (`data.items` is closed, so each row is emitted as node-building code in `app.js`). Measured once by removing the four trail panels from a copy of the program (not regenerated by this script): `app.js` 273.4 kB to 225.0 kB, so **the four trail panels are about 48 kB of the script**; the 24 project panels are most of the rest of the difference with B2a\'s 87.3 kB script (by subtraction, not by a second removal: the program does not build with the `For` over projects removed, `list-empty`).');
  md.push('');
  fs.writeFileSync(path.join(work, 'cost.md'), md.join('\n'));
  if (WRITE) { fs.writeFileSync(path.join(ROOT, 'docs', 'www-port', 'cost.md'), md.join('\n') + '\n'); }
  console.log(md.join('\n'));
})().catch((e) => { console.error(e); process.exit(1); });
