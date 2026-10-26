#!/usr/bin/env node
// The page-level elements in Chromium (docs/page.md): DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   A. the head, read with JAVASCRIPT OFF: the title, the language, every <meta> and <link> the page declares, the card's size against the
//      bytes of the file (read here, not by the compiler), every file the head names served with a 200, and no request to another host;
//   B. the skip link: the first focusable element of the document (with and without script), outside the window until it has the focus,
//      inside it and above the sticky header while it has it, Enter reaches the Main below the header, and the next Tab is inside the Main;
//   C. the sticky header and the anchors: it stays at the top while the page scrolls, `--cui-header-h` is its height, `scroll-padding-top`
//      is that and one rem, every in-page link and a page opened at a fragment put the target below the header (script on and off), and
//      Shift+Tab from the end of the page never leaves a focused element under it;
//   D. a header that wraps (320 px wide, fourteen links) is taller than 4rem: with the script the anchors are still below it;
//   E. the static head and the skip link are not touched when the script arrives (the same nodes, held back 1.5 s);
//   F. with a theme the skip link and the header take its colours at 4.5:1 or more in both schemes; a program with no sticky header has
//      no room reserved.
//   node scripts/check-page.js
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');
const { ratio } = require('./theme-oracle.js');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const EXAMPLE = path.join(ROOT, 'examples', 'page');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'page-'));
let holdApp = 0;

function build(name, file, cwd) {
  const out = path.join(work, name);
  const r = spawnSync(BIN, ['build', file, '--page', '-o', out], { encoding: 'utf8', cwd });
  if (r.status !== 0) { console.log(`FAIL build ${name}: ${r.stderr}${r.stdout}`); process.exit(1); }
  return out;
}
function serve(dir, seen) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      seen.push(q.url);
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      const send = () => fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.png': 'image/png', '.md': 'text/markdown' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
      if (f.endsWith('app.js') && holdApp) { setTimeout(send, holdApp); } else { send(); }
    }).listen(0, () => ok(s));
  });
}

const PIXEL = { ...devices['Pixel 7'], reducedMotion: 'reduce' };
const TOP = (id) => `(() => { const e = document.getElementById(${JSON.stringify(id)}); return e.getBoundingClientRect().top; })()`;
const geometry = () => ({ header: document.querySelector('.cui-sticky').getBoundingClientRect().bottom, max: document.documentElement.scrollHeight - innerHeight });

// every in-page link: follow it and report how far below the header the target's top lands
async function anchors(page, ids) {
  const rows = [];
  for (const id of ids) {
    await page.evaluate(() => { location.hash = ''; scrollTo(0, 0); });
    await page.evaluate((i) => { location.hash = i; }, id);
    await sleep(80);
    rows.push(await page.evaluate((i) => {
      const g = { header: document.querySelector('.cui-sticky').getBoundingClientRect().bottom, max: document.documentElement.scrollHeight - innerHeight };
      return { id: i, top: document.getElementById(i).getBoundingClientRect().top, header: g.header, atEnd: scrollY >= g.max - 1 };
    }, id));
  }
  return rows;
}
const hidden = (rows) => rows.filter((r) => r.top < r.header - 0.5 && !r.atEnd);

(async () => {
  const site = build('site', path.join(EXAMPLE, 'page.cui'), ROOT);
  const seen = [];
  const server = await serve(site, seen);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  const external = [];
  async function open(opts, query = '') {
    const ctx = await browser.newContext(opts);
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    await page.route('**/*', (r) => { if (r.request().url().startsWith('http://127.0.0.1')) { r.continue(); } else { external.push(r.request().url()); r.abort(); } });
    await page.goto(url + query);
    await sleep(300);
    return { ctx, page, errors };
  }

  // ---- A. the head, with JavaScript off ---------------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open({ ...PIXEL, javaScriptEnabled: false });
    const head = await page.evaluate(() => ({
      lang: document.documentElement.lang, title: document.title,
      meta: Object.fromEntries([...document.querySelectorAll('meta[name], meta[property]')].map((m) => [m.getAttribute('name') || m.getAttribute('property'), m.content])),
      links: [...document.querySelectorAll('link[rel]')].map((l) => ({ rel: l.rel, href: l.getAttribute('href'), type: l.type, title: l.title })),
    }));
    check('A the language and the title are the page\'s, with no script', head.lang === 'en' && head.title === 'Boulder: a page written in cancho-ui', `${head.lang} / ${head.title}`);
    const png = fs.readFileSync(path.join(EXAMPLE, 'card.png'));
    const want = { w: png.readUInt32BE(16), h: png.readUInt32BE(20) };
    check('A the card\'s size is the file\'s (read here, from the PNG header)', head.meta['og:image:width'] === String(want.w) && head.meta['og:image:height'] === String(want.h), `${head.meta['og:image:width']}x${head.meta['og:image:height']} against ${want.w}x${want.h}`);
    check('A the card, the canonical address and og:url are absolute and agree', head.meta['og:image'] === 'https://example.org/boulder/card.png' && head.meta['og:url'] === 'https://example.org/boulder/' && head.links.some((l) => l.rel === 'canonical' && l.href === head.meta['og:url']), `${head.meta['og:image']}`);
    check('A the description is the page\'s and the card has its own', /^A page whose head is declared/.test(head.meta.description) && head.meta['og:description'] === 'The head of a document, declared in the program.' && head.meta['og:title'] === head.title && head.meta['og:type'] === 'website' && head.meta['twitter:card'] === 'summary_large_image' && !!head.meta['og:image:alt'] && head.meta['color-scheme'] === 'light dark');
    check('A icons and alternates are in the head', head.links.some((l) => l.rel === 'icon' && l.type === 'image/png') && head.links.some((l) => l.rel === 'apple-touch-icon') && head.links.filter((l) => l.rel === 'alternate').map((l) => `${l.type} ${l.href} ${l.title}`).join('|') === 'text/markdown agents.md Boulder, as an agent reads it|application/json data/projects.json The projects as data');
    let bad = [];
    for (const l of head.links) { if (!/^https?:/.test(l.href)) { const res = await page.request.get(url + l.href); if (res.status() !== 200) { bad.push(`${l.href} ${res.status()}`); } } }
    check('A every file the head names is served', bad.length === 0, bad.join(', '));
    const text = await page.evaluate(() => document.body.innerText);
    check('A the page is readable with no script and says what is missing', /Paragraph 1/.test(text) && /Some parts of this page need JavaScript/.test(text));
    check('A no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- B. the skip link --------------------------------------------------------------------------------------------------------------
  for (const [name, opts] of [['Pixel 7', PIXEL], ['Pixel 7, no script', { ...PIXEL, javaScriptEnabled: false }], ['desktop 1280', { viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' }]]) {
    const { ctx, page, errors } = await open(opts);
    const first = await page.evaluate(() => {
      const sel = 'a[href], button, input, select, textarea, summary, [tabindex]:not([tabindex="-1"])';
      const all = [...document.querySelectorAll(sel)];
      const skip = document.querySelector('.cui-skip');
      const r = skip.getBoundingClientRect();
      return { firstIsSkip: all[0] === skip, hrefTarget: skip.getAttribute('href'), offscreen: r.bottom <= 0, targetIsMain: document.querySelector(skip.getAttribute('href')).tagName === 'MAIN' };
    });
    check(`B ${name}: the skip link is the first focusable element, goes to the Main, and is outside the window until focused`, first.firstIsSkip && first.targetIsMain && first.offscreen, JSON.stringify(first));
    await page.keyboard.press('Tab');
    const focused = await page.evaluate(() => {
      const a = document.activeElement; const r = a.getBoundingClientRect(); const h = document.querySelector('.cui-sticky').getBoundingClientRect();
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return { cls: a.className, inside: r.top >= 0 && r.left >= 0 && r.right <= innerWidth && r.bottom <= innerHeight, above: hit === a, overHeader: r.top < h.bottom, h: r.height };
    });
    check(`B ${name}: the first Tab focuses it, inside the window, above the header, 44 px high`, focused.cls === 'cui-skip' && focused.inside && focused.above && focused.h >= 44, JSON.stringify(focused));
    await page.keyboard.press('Enter');
    await sleep(100);
    const after = await page.evaluate(() => ({ hash: location.hash, active: document.activeElement.tagName, mainTop: document.getElementById('main').getBoundingClientRect().top, header: document.querySelector('.cui-sticky').getBoundingClientRect().bottom, outline: getComputedStyle(document.getElementById('main')).outlineStyle }));
    await page.keyboard.press('Tab');
    const next = await page.evaluate(() => ({ inMain: !!document.activeElement.closest('main'), inHeader: !!document.activeElement.closest('header') }));
    check(`B ${name}: Enter reaches the Main below the header and the next Tab is inside it`, after.hash === '#main' && after.mainTop >= after.header - 0.5 && next.inMain && !next.inHeader, JSON.stringify({ after, next }));
    if (name === 'Pixel 7') { check('B the Main that was started from is the active element and draws no outline', after.active === 'MAIN' && after.outline === 'none', `${after.active} ${after.outline}`); }
    check(`B ${name}: no console error`, errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- C. the sticky header and the anchors -----------------------------------------------------------------------------------------
  const ids = ['top', 'why', 'how', 'anchors', 'keys', 'end'];
  for (const [name, opts] of [['Pixel 7', PIXEL], ['desktop 1280', { viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' }]]) {
    const { ctx, page } = await open(opts);
    const s = await page.evaluate(() => { scrollTo(0, 1500); const h = document.querySelector('.cui-sticky'); return { pos: getComputedStyle(h).position, top: h.getBoundingClientRect().top, hh: h.offsetHeight, v: getComputedStyle(document.documentElement).getPropertyValue('--cui-header-h'), pad: parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop), rem: parseFloat(getComputedStyle(document.documentElement).fontSize) }; });
    check(`C ${name}: the header is sticky and stays at the top of the window`, s.pos === 'sticky' && s.top === 0, JSON.stringify(s));
    check(`C ${name}: --cui-header-h is its height and the padding is that and one rem`, s.v === `${s.hh}px` && Math.abs(s.pad - (s.hh + s.rem)) < 0.5, `${s.v} ${s.hh} ${s.pad}`);
    const rows = await anchors(page, ids);
    check(`C ${name}: ${ids.length} in-page links put their target below the header`, hidden(rows).length === 0, rows.map((r) => `${r.id} ${Math.round(r.top)}/${Math.round(r.header)}`).join(' '));
    // Shift+Tab from the end of the page: a focused element is not left under the header (the skip link is above it on purpose)
    await page.evaluate(() => { scrollTo(0, document.documentElement.scrollHeight); document.body.focus(); });
    let under = 0; let stops = 0;
    for (let i = 0; i < 20; i++) {
      await page.keyboard.press('Shift+Tab');
      const r = await page.evaluate(() => { const a = document.activeElement; const b = a.getBoundingClientRect(); return { under: !a.closest('.cui-sticky') && !a.classList.contains('cui-skip') && a !== document.body && b.top < document.querySelector('.cui-sticky').getBoundingClientRect().bottom && b.bottom > 0, ok: a !== document.body }; });
      if (r.ok) { stops++; } if (r.under) { under++; }
    }
    check(`C ${name}: Shift+Tab through the page leaves none of ${stops} focused elements under the header`, under === 0 && stops >= 8, `${under} of ${stops}`);
    await ctx.close();
  }
  for (const [name, opts] of [['script on', PIXEL], ['script off', { ...PIXEL, javaScriptEnabled: false }]]) {
    const { ctx, page } = await open(opts, '#anchors');
    await sleep(300);
    const r = await page.evaluate(() => ({ top: document.getElementById('anchors').getBoundingClientRect().top, header: document.querySelector('.cui-sticky').getBoundingClientRect().bottom }));
    check(`C a page opened at #anchors, ${name}: the section is below the header`, r.top >= r.header - 0.5 && r.top < r.header + 40, JSON.stringify(r));
    if (name === 'script off') {
      const rows = await anchors(page, ids);
      check('C with the script off the 4rem fallback holds for a header that fits in it', hidden(rows).length === 0, rows.map((x) => `${x.id} ${Math.round(x.top)}/${Math.round(x.header)}`).join(' '));
    }
    await ctx.close();
  }

  // ---- D. a header that wraps ---------------------------------------------------------------------------------------------------------
  {
    const nav = Array.from({ length: 14 }, (_, i) => `Link { to: "#s${i}" "Section ${i}" }`).join(' ');
    const secs = Array.from({ length: 14 }, (_, i) => `Section { id: "s${i}" Heading { level: 2 "Section ${i}" } ${Array.from({ length: 5 }, (_, j) => `Text { "Paragraph ${j} of section ${i}, long enough to take a few lines on a narrow window." }`).join(' ')} }`).join('\n');
    const f = path.join(work, 'tall.cui');
    fs.writeFileSync(f, `component Tall {\n view { Column {\n Header { sticky: true Nav { label: "Sections" ${nav} } }\n Main { id: "main" ${secs} }\n } }\n}\n`);
    const tall = build('tall', f, ROOT);
    const s2 = await serve(tall, []);
    const u2 = `http://127.0.0.1:${s2.address().port}/`;
    const ids2 = Array.from({ length: 14 }, (_, i) => `s${i}`);
    const res = {};
    for (const js of [true, false]) {
      const ctx = await browser.newContext({ viewport: { width: 320, height: 640 }, reducedMotion: 'reduce', javaScriptEnabled: js });
      const page = await ctx.newPage();
      await page.goto(u2);
      await sleep(300);
      const h = await page.evaluate(() => document.querySelector('.cui-sticky').offsetHeight);
      const rows = await anchors(page, ids2);
      res[js] = { h, hidden: hidden(rows).length };
      await ctx.close();
    }
    check('D the header is taller than 4rem at 320 px (64 px)', res[true].h > 64, `${res[true].h} px`);
    check('D with the script no target is under the taller header', res[true].hidden === 0, `${res[true].hidden} of ${ids2.length}`);
    console.log(`info D the same page with the script off (the 4rem fallback, a header of ${res[false].h} px): ${res[false].hidden} of ${ids2.length} targets under the header`);
    check('D without the script the fallback is what the note says: a taller header hides targets', res[false].hidden > 0, `${res[false].hidden}`);
    s2.close();
  }

  // ---- E. the static head and the skip link are not touched when the script arrives ------------------------------------------------------
  {
    holdApp = 1500;
    const ctx = await browser.newContext(PIXEL);
    const page = await ctx.newPage();
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    await page.evaluate(() => { window.__skip = document.querySelector('.cui-skip'); window.__head = document.head.innerHTML; window.__hdr = document.querySelector('.cui-sticky'); window.__main = document.getElementById('main'); });
    await page.waitForFunction(() => !!document.querySelector('#root button'), null, { timeout: 10000 });
    const same = await page.evaluate(() => ({ skip: window.__skip === document.querySelector('.cui-skip') && window.__skip.isConnected, head: window.__head.replace(/<style id="cui-theme">[\s\S]*?<\/style>/, '') === document.head.innerHTML.replace(/<style id="cui-theme">[\s\S]*?<\/style>/, ''), hdr: window.__hdr === document.querySelector('.cui-sticky'), main: window.__main === document.getElementById('main') }));
    check('E the skip link, the header, the Main and the head are the same nodes after the script ran', same.skip && same.hdr && same.main && same.head, JSON.stringify(same));
    holdApp = 0;
    await ctx.close();
  }

  // ---- F. a theme, and a program with no sticky header --------------------------------------------------------------------------------
  {
    const theme = fs.readFileSync(path.join(ROOT, 'examples', 'theme', 'theme.cui'), 'utf8').match(/theme \{[\s\S]*?\n    font body[\s\S]*?\n    \}\n/)[0].replace(/    font body[\s\S]*$/, '}\n');
    const f = path.join(EXAMPLE, 'themed.cui');
    fs.writeFileSync(f, fs.readFileSync(path.join(EXAMPLE, 'page.cui'), 'utf8') + '\n' + theme);
    let themed;
    try { themed = build('themed', f, ROOT); } finally { fs.rmSync(f, { force: true }); }
    const s3 = await serve(themed, []);
    const u3 = `http://127.0.0.1:${s3.address().port}/`;
    const tokens = { light: { bg: '#f7f4eb', accent: '#185fa5', accent_fg: '#ffffff' }, dark: { bg: '#0b1016', accent: '#6ea3ee', accent_fg: '#0a1220' } };
    const hex = (c) => '#' + c.match(/\d+/g).slice(0, 3).map((n) => Number(n).toString(16).padStart(2, '0')).join('');
    for (const scheme of ['light', 'dark']) {
      const ctx = await browser.newContext({ ...PIXEL, colorScheme: scheme });
      const page = await ctx.newPage();
      await page.goto(u3);
      await sleep(200);
      await page.keyboard.press('Tab');
      const c = await page.evaluate(() => { const s = getComputedStyle(document.activeElement); return { bg: s.backgroundColor, fg: s.color, hdr: getComputedStyle(document.querySelector('.cui-sticky')).backgroundColor }; });
      const r = ratio(hex(c.fg), hex(c.bg));
      check(`F themed, ${scheme}: the skip link is accent_fg on accent at ${r.toFixed(2)}:1 and the header is the theme's bg`, hex(c.bg) === tokens[scheme].accent && hex(c.fg) === tokens[scheme].accent_fg && r >= 4.5 && hex(c.hdr) === tokens[scheme].bg, JSON.stringify(c));
      await ctx.close();
    }
    for (const scheme of ['light', 'dark']) {
      const ctx = await browser.newContext({ ...PIXEL, colorScheme: scheme });
      const page = await ctx.newPage();
      await page.goto(url);
      await sleep(200);
      await page.keyboard.press('Tab');
      const c = await page.evaluate(() => { const s = getComputedStyle(document.activeElement); return { bg: s.backgroundColor, fg: s.color }; });
      const r = ratio(hex(c.fg), hex(c.bg));
      check(`F unthemed, ${scheme}: the skip link is ${r.toFixed(2)}:1`, r >= 4.5, JSON.stringify(c));
      await ctx.close();
    }
    s3.close();
    const f2 = path.join(work, 'plain.cui');
    fs.writeFileSync(f2, 'page { title: "P"; lang: "en"; skip_link: "Skip"; }\ncomponent P {\n view { Column { Header { Link { to: "#main" "Top" } } Main { id: "main" Text { "x" } } } }\n}\n');
    const plain = build('plain', f2, ROOT);
    const s4 = await serve(plain, []);
    const ctx = await browser.newContext(PIXEL);
    const page = await ctx.newPage();
    await page.goto(`http://127.0.0.1:${s4.address().port}/`);
    const p = await page.evaluate(() => ({ pad: getComputedStyle(document.documentElement).scrollPaddingTop, pos: getComputedStyle(document.querySelector('header')).position, v: document.documentElement.style.getPropertyValue('--cui-header-h') }));
    check('F a program with no sticky header reserves no room and measures nothing', p.pad === 'auto' && p.pos === 'static' && p.v === '', JSON.stringify(p));
    await ctx.close();
    s4.close();
  }

  check('no request went to another host', external.length === 0, external.join(', '));
  await browser.close();
  server.close();
  clearTimeout(watchdog);
  fs.rmSync(work, { recursive: true, force: true });
  if (failures.length) { console.log(`FAIL ${failures.length}: ${failures.join('; ')}`); process.exit(1); }
  console.log('PASS');
})();
