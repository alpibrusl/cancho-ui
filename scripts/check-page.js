#!/usr/bin/env node
// The page-level primitives in Chromium (docs/page.md): DESKTOP CHROMIUM, not a device. Watchdog: 5 minutes.
//   A. the head of examples/page, in the browser: the title, the language and every <meta>/<link> against tests/fixtures/page/cancho-www-head.html
//      (the head cancho-www writes by hand), element by element and attribute by attribute;
//   B. the skip link: off the screen until it has focus, first in the tab order, visible and readable at 4.5:1 in both colour schemes when it has,
//      and Enter takes the page to the Main, below the sticky header; and the same with script off;
//   C. the sticky header at 1280, 768, 390 and 320 px: it stays at the top, `scroll-padding-top` is its height + 16 px, and every anchor lands
//      below it, with the header at its tallest (the wrapped nav of 320 px); a resize changes the padding and the anchors still clear it; and what
//      the 5rem of the stylesheet alone does is measured: with script off the header is not sticky at all;
//   D. a control that takes focus under a sticky header is scrolled out from under it (WCAG 2.2, 2.4.11 Focus Not Obscured), by Tab, at 320 px;
//   E. text of the program in the head (quotes, angle brackets, a token) cannot become markup or be replaced;
//   F. no request to any origin but the page's, no console error.
//   node scripts/check-page.js
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');
const { ratio } = require('./theme-oracle.js');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'page-'));
fs.mkdirSync(path.join(work, 'site'), { recursive: true });
let nextBuild = 0;
function build(source, id) {
  const name = id || `b${nextBuild++}`;
  const r = spawnSync(BIN, ['build', source, '--page', '-o', path.join(work, 'site', name)], { encoding: 'utf8', cwd: ROOT });
  if (r.status !== 0) { throw new Error(`build ${source}: ${r.stderr}${r.stdout}`); }
  return name;
}

(async () => {
  const server = http.createServer((q, r) => {
    let f = path.join(work, 'site', decodeURIComponent(q.url.split('?')[0]));
    if (f.endsWith(path.sep) || (fs.existsSync(f) && fs.statSync(f).isDirectory())) { f = path.join(f, 'index.html'); }
    fs.readFile(f, (e, d) => {
      if (e) { r.statusCode = 404; r.end(); return; }
      r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.md': 'text/markdown', '.png': 'image/png' }[path.extname(f)] || 'application/octet-stream');
      r.end(d);
    });
  });
  await new Promise((ok) => server.listen(0, ok));
  const base = `http://localhost:${server.address().port}`;
  const browser = await chromium.launch();
  const example = build('examples/page/page.cui', 'example');
  const origins = new Set();
  const errors = [];
  async function open(url, opts = {}) {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, ...opts });
    const page = await ctx.newPage();
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('request', (rq) => { if (!/^(data|about|blob):/.test(rq.url())) { origins.add(new URL(rq.url()).origin); } });
    await page.goto(url);
    await sleep(opts.javaScriptEnabled === false ? 100 : 400);
    return { ctx, page };
  }

  // ---------- A. the head ----------
  {
    const { ctx, page } = await open(`${base}/${example}/`);
    const real = fs.readFileSync(path.join(ROOT, 'tests', 'fixtures', 'page', 'cancho-www-head.html'), 'utf8');
    const FIRST = `(() => { const t = document.title; const els = [...document.head.children].filter((e) => e.tagName === 'META' || e.tagName === 'LINK').filter((e) => !['viewport'].includes(e.getAttribute('name')) && e.getAttribute('charset') === null && e.getAttribute('rel') !== 'stylesheet').map((e) => [e.tagName, ...[...e.attributes].map((a) => a.name + '=' + a.value)].join(' ')); return { t, els }; })()`;
    const mine = await page.evaluate(FIRST);
    const ref = await ctx.newPage();
    await ref.setContent(`<!doctype html><html><head>${real}</head><body></body></html>`);
    const want = await ref.evaluate(FIRST);
    check('A: the title is the real page\'s', mine.t === want.t && mine.t.length > 0, mine.t);
    check(`A: ${want.els.length} meta and link elements, the same as the real head, in the same order, attribute by attribute`, JSON.stringify(mine.els) === JSON.stringify(want.els), mine.els.length === want.els.length ? '' : `${mine.els.length} against ${want.els.length}`);
    check('A: <html lang> is en', await page.evaluate(() => document.documentElement.lang) === 'en');
    const files = await page.evaluate(async () => { const out = {}; for (const f of ['logo-mark.png', 'logo.png', 'agents.md', 'data/ecosystem.json']) { out[f] = (await fetch(f)).status; } return out; });
    check('A: the icons and the alternates the page names are served beside it', Object.values(files).every((s) => s === 200), JSON.stringify(files));
    await ctx.close();
  }

  // ---------- B. the skip link ----------
  for (const [scheme, js] of [['light', true], ['dark', true], ['light', false]]) {
    const { ctx, page } = await open(`${base}/${example}/`, { colorScheme: scheme, javaScriptEnabled: js });
    const tag = `${scheme}${js ? '' : ', script off'}`;
    const off = await page.evaluate(() => { const a = document.querySelector('a.cui-link-skip'); const r = a.getBoundingClientRect(); return { bottom: r.bottom, text: a.textContent, href: a.getAttribute('href'), first: document.querySelector('#root a') === a }; });
    check(`B (${tag}): the skip link is the first link of the page and is above the screen until it has focus`, off.first && off.bottom <= 0 && off.text === 'Skip to content' && off.href === '#main', JSON.stringify(off));
    await page.keyboard.press('Tab');
    const on = await page.evaluate(() => { const a = document.activeElement; const r = a.getBoundingClientRect(); const cs = getComputedStyle(a); return { skip: a.classList.contains('cui-link-skip'), top: r.top, height: r.height, fg: cs.color, bg: cs.backgroundColor }; });
    const hex = (c) => '#' + c.match(/\d+/g).slice(0, 3).map((n) => Number(n).toString(16).padStart(2, '0')).join('');
    const cr = ratio(hex(on.fg), hex(on.bg));
    check(`B (${tag}): the first Tab lands on it and shows it, 44 px high, at ${cr.toFixed(2)}:1`, on.skip && on.top >= 0 && on.height >= 44 && cr >= 4.5, JSON.stringify(on));
    await page.keyboard.press('Enter');
    await sleep(js ? 600 : 100);
    const after = await page.evaluate(() => { const m = document.getElementById('main'); const h = document.querySelector('header.cui-sticky').getBoundingClientRect(); return { hash: location.hash, mainTop: Math.round(m.getBoundingClientRect().top), headerBottom: Math.round(h.bottom) }; });
    check(`B (${tag}): Enter goes to the Main, below the header`, after.hash === '#main' && after.mainTop >= after.headerBottom - 1, JSON.stringify(after));
    await ctx.close();
  }

  // ---------- C. the sticky header ----------
  const ANCHORS = ['try', 'trust', 'verifies', 'ecosystem', 'demo', 'agents', 'main', 'top'];
  const measure = async (page, id) => {
    await page.evaluate((i) => { location.hash = ''; window.scrollTo(0, 0); }, id);
    await sleep(120);
    await page.evaluate((i) => { location.hash = '#' + i; }, id);
    await sleep(900);
    return page.evaluate((i) => { const e = document.getElementById(i); const h = document.querySelector('header.cui-sticky').getBoundingClientRect(); return { top: Math.round(e.getBoundingClientRect().top), headerBottom: Math.round(h.bottom), maxScroll: document.documentElement.scrollHeight - innerHeight - scrollY }; }, id);
  };
  for (const width of [1280, 768, 390, 320]) {
    const { ctx, page } = await open(`${base}/${example}/`, { viewport: { width, height: 800 } });
    const s0 = await page.evaluate(() => { window.scrollTo(0, 1800); return null; });
    await sleep(150);
    const st = await page.evaluate(() => { const h = document.querySelector('header.cui-sticky'); const r = h.getBoundingClientRect(); return { pos: getComputedStyle(h).position, top: r.top, height: Math.round(r.height), pad: getComputedStyle(document.documentElement).scrollPaddingTop }; });
    check(`C (${width}): the header is sticky and stays at the top after the page is scrolled 1800 px; it is ${st.height} px tall`, st.pos === 'sticky' && st.top === 0, JSON.stringify(st));
    check(`C (${width}): scroll-padding-top is its height + 16 px (${st.height + 16})`, parseFloat(st.pad) === st.height + 16, st.pad);
    const bad = [];
    for (const id of ANCHORS) { const m = await measure(page, id); if (m.top < m.headerBottom - 1) { bad.push(`${id}: ${m.top} < ${m.headerBottom}`); } else if (m.top > m.headerBottom + 40 && m.maxScroll > 1) { bad.push(`${id}: ${m.top} is far below ${m.headerBottom}`); } }
    check(`C (${width}): every one of ${ANCHORS.length} anchors lands just below the header`, bad.length === 0, bad.join('; '));
    await ctx.close();
  }
  {
    const { ctx, page } = await open(`${base}/${example}/`, { viewport: { width: 1280, height: 800 } });
    const h1 = await page.evaluate(() => Math.round(document.querySelector('header.cui-sticky').getBoundingClientRect().height));
    await page.setViewportSize({ width: 320, height: 800 });
    await sleep(400);
    const h2 = await page.evaluate(() => Math.round(document.querySelector('header.cui-sticky').getBoundingClientRect().height));
    const pad = await page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop));
    check(`C: a resize from 1280 to 320 px changes the header from ${h1} to ${h2} px and the padding follows (${pad})`, h2 > h1 && pad === h2 + 16);
    const m = await measure(page, 'trust');
    check('C: and the anchor still clears it after the resize', m.top >= m.headerBottom - 1, JSON.stringify(m));
    await ctx.close();
  }
  for (const width of [1280, 320]) {
    const { ctx, page } = await open(`${base}/${example}/`, { viewport: { width, height: 800 }, javaScriptEnabled: false });
    const h = await page.evaluate(() => Math.round(document.querySelector('header.cui-sticky').getBoundingClientRect().height));
    await page.goto(`${base}/${example}/#trust`);
    await sleep(500);
    const m = await page.evaluate(() => { const e = document.getElementById('trust'); return { top: Math.round(e.getBoundingClientRect().top), headerBottom: Math.round(document.querySelector('header.cui-sticky').getBoundingClientRect().bottom), pad: getComputedStyle(document.documentElement).scrollPaddingTop }; });
    const pos = await page.evaluate(() => getComputedStyle(document.querySelector('header.cui-sticky')).position);
    const clear = m.top >= m.headerBottom - 1;
    console.log(`info C (script off, ${width}): the header is ${h} px and ${pos}; the anchor lands at ${m.top} against the header's bottom ${m.headerBottom}`);
    check(`C (script off, ${width}): the header is not sticky without script (its height is measured by the script, and 5rem alone hid an anchor under an 88 px header), so no anchor is under it`, pos === 'static' && clear, `${pos} ${JSON.stringify(m)}`);
    await ctx.close();
  }

  // ---------- D. focus not obscured ----------
  {
    const { ctx, page } = await open(`${base}/${example}/`, { viewport: { width: 320, height: 640 } });
    const seen = [];
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press('Tab');
      await sleep(60);
      const r = await page.evaluate(() => { const a = document.activeElement; const h = document.querySelector('header.cui-sticky'); if (!a || a === document.body) { return null; } const ar = a.getBoundingClientRect(); return { inHeader: h.contains(a), top: Math.round(ar.top), bottom: Math.round(ar.bottom), headerBottom: Math.round(h.getBoundingClientRect().bottom), text: (a.textContent || '').slice(0, 20) }; });
      if (r) { seen.push(r); }
    }
    const body = seen.filter((r) => !r.inHeader);
    // the skip link is drawn over the header (z-index 20) and is the one control that is meant to be there
    const hidden = body.filter((r) => r.top < r.headerBottom - 1 && r.text !== 'Skip to content');
    check(`D: tabbing through the page at 320 px focused ${body.length} controls outside the header and none was under it`, body.length >= 7 && hidden.length === 0, hidden.map((r) => `${r.text}@${r.top}<${r.headerBottom}`).join('; '));
    await ctx.close();
  }

  // ---------- E. hostile text ----------
  {
    const src = path.join(work, 'hostile.cui');
    fs.writeFileSync(src, `page {
    title: "<\\"x\\"><script>window.__x = 1</script> @TITLE@ @PAGE_META@";
    description: "A description that is long enough to be one: more than fifty characters, with <b>markup</b> & \\"quotes\\" and @LANG@.";
    canonical: "https://example.org/";
}
component C {
    view { Main { id: "main" Text { "x" } } }
}
`);
    const id = build(src, 'hostile');
    const { ctx, page } = await open(`${base}/${id}/`);
    const r = await page.evaluate(() => ({ x: window.__x, titles: document.querySelectorAll('title').length, title: document.title, desc: document.querySelector('meta[name=description]').content, og: document.querySelector('meta[property="og:title"]').content, lang: document.documentElement.lang, scripts: document.head.querySelectorAll('script').length }));
    check('E: a title with quotes, angle brackets, a script and tokens is text: one <title>, nothing ran, the tokens untouched', r.x === undefined && r.titles === 1 && r.title === '<"x"><script>window.__x = 1</script> @TITLE@ @PAGE_META@' && r.og === r.title && r.lang === 'en', JSON.stringify(r));
    check('E: the description keeps its markup as text and the token in it', r.desc.includes('<b>markup</b> & "quotes" and @LANG@'), r.desc);
    await ctx.close();
  }

  // ---------- F ----------
  check('F: no request to any origin but the page\'s', [...origins].every((o) => o === base), [...origins].join(' '));
  check('F: no console error or script error', errors.length === 0, errors.slice(0, 3).join(' | '));

  await browser.close();
  server.close();
  clearTimeout(watchdog);
  fs.rmSync(work, { recursive: true, force: true });
  if (failures.length) { console.log(`\n${failures.length} check(s) failed`); process.exit(1); }
  console.log('\nall checks passed');
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
