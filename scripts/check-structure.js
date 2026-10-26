#!/usr/bin/env node
// The structure elements in Chromium (docs/structure.md): DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   A. every `valid` case of tests/structure_corpus.json is built with `build --page` and the DOM it becomes, serialised as the page serialises
//      it (tags lower case, attributes sorted, double quotes, no whitespace between tags), is the `html` the corpus says; this is the
//      comparison the prerenderer will have to pass too;
//   B. examples/structure, read through the accessibility tree: the landmarks (banner, navigation named, main, contentinfo, complementary
//      named), the headings in order with no skipped level, every link named and not generic, the details and the figure, the in-page
//      links reaching their targets, the targets of links in a header, a nav and a footer at least 44 px, the appearances at least 44 px,
//      the focus ring on a focused link;
//   E. the copy button of a Code: nothing is written to the clipboard until it is tapped, a tap writes the text of the box once and the page never reads it, and with the
//      clipboard refused or absent the text is selected;
//   C. a screen that appears (an `If`) hands the focus to its `Heading`, as it does to a `Text { role: heading }`;
//   D. with a theme, a link of each appearance is drawn at a contrast of at least 4.5:1 in both colour schemes (the oracle's formula);
//   F. Image, Media and Grid (docs/structure.md section 10) in Chromium: the number of columns of a Grid against a formula written here, the
//      width of the columns of a weighted Grid (the ratio, the width at which it stacks), the space an Image holds before its file arrives, the
//      crop of a ratio, the accessible name of a picture and of a decorative one, a video's attributes (no autoplay, controls, described by
//      its caption), no horizontal scroll at 320 px, and no request to any origin but the page's.
//   node scripts/check-structure.js
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
const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'structure_corpus.json'), 'utf8'));
const failures = [];
let holdApp = 0;
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'structure-'));
fs.mkdirSync(path.join(work, 'site'), { recursive: true });
// the files the corpus names: the first bytes of each format (the compiler reads no more), written beside every program
function magic(file) {
  const ext = file.split('.').pop();
  const head = { png: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], jpg: [0xff, 0xd8, 0xff, 0xe0], gif: [0x47, 0x49, 0x46, 0x38, 0x39, 0x61],
    webp: [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50], avif: [0, 0, 0, 0x1c, 0x66, 0x74, 0x79, 0x70, 0x61, 0x76, 0x69, 0x66],
    mp4: [0, 0, 0, 0x1c, 0x66, 0x74, 0x79, 0x70, 0x69, 0x73, 0x6f, 0x6d], webm: [0x1a, 0x45, 0xdf, 0xa3] }[ext];
  // a file a link names (docs/www-port/gaps.md section 6) is text
  if (!head) { return Buffer.from('# a file of the site\n'); }
  return Buffer.from([...head, ...new Array(16).fill(0)]);
}
for (const f of corpus.assets.ok) {
  fs.mkdirSync(path.dirname(path.join(work, f)), { recursive: true });
  fs.writeFileSync(path.join(work, f), magic(f));
}
// a PNG that Chromium can decode, written here (zlib is in node; the CRC is the one of the PNG specification)
function png(w, h, rgb) {
  const zlib = require('zlib');
  const crcTable = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) { c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; } return c; });
  const crc = (b) => { let c = -1; for (const x of b) { c = crcTable[(c ^ x) & 255] ^ (c >>> 8); } return (c ^ -1) >>> 0; };
  const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([len, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(new Array(w).fill(rgb).flat())]);
  const raw = Buffer.concat(new Array(h).fill(row));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
fs.writeFileSync(path.join(work, 'assets', 'real.png'), png(64, 48, [200, 80, 40]));
fs.writeFileSync(path.join(work, 'assets', 'real2.png'), png(80, 60, [40, 80, 200]));
// a video Chromium can play (VP8, 0.5 s, 64 x 48): tests/fixtures/tiny.webm, made by
//   ffmpeg -f lavfi -i color=c=0x2060c0:s=64x48:d=0.5:r=10 -c:v libvpx -b:v 30k -an tiny.webm
fs.copyFileSync(path.join(ROOT, 'tests', 'fixtures', 'tiny.webm'), path.join(work, 'assets', 'tiny.webm'));
let nextBuild = 0;
function build(sourceFile, extra = []) {
  const id = `b${nextBuild++}`;
  const r = spawnSync(BIN, ['build', sourceFile, '--page', '-o', path.join(work, 'site', id), ...extra], { encoding: 'utf8', cwd: ROOT });
  return { id, code: r.status, err: r.stderr + r.stdout };
}
function buildText(text, extra) {
  const f = path.join(work, `p${nextBuild}.cui`);
  fs.writeFileSync(f, text);
  return build(f, extra);
}

// the page's serialisation, run inside the browser
const SERIALISE = `(function () {
  function esc(s) { return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
  function ser(n) {
    if (n.nodeType === 3) { return esc(n.data); }
    if (n.nodeType !== 1) { return ''; }
    const attrs = [...n.attributes].map((a) => [a.name, a.value]).sort((a, b) => (a[0] < b[0] ? -1 : 1))
      .map(([k, v]) => ' ' + k + '="' + v.replace(/&/g, '&amp;').replace(/"/g, '&quot;') + '"').join('');
    return '<' + n.tagName.toLowerCase() + attrs + '>' + [...n.childNodes].map(ser).join('') + '</' + n.tagName.toLowerCase() + '>';
  }
  return [...document.getElementById('root').childNodes].map(ser).join('');
})()`;

(async () => {
  const server = http.createServer((q, r) => {
    let f = path.join(work, 'site', decodeURIComponent(q.url.split('?')[0]));
    if (f.endsWith(path.sep) || (fs.existsSync(f) && fs.statSync(f).isDirectory())) { f = path.join(f, 'index.html'); }
    fs.readFile(f, (e, d) => {
      if (e) { r.statusCode = 404; r.end(); return; }
      r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.woff2': 'font/woff2' }[path.extname(f)] || 'application/octet-stream');
      // part G holds the script back, to look at the page before it has run
      if (holdApp && f.endsWith('app.js')) { setTimeout(() => r.end(d), holdApp); } else { r.end(d); }
    });
  });
  await new Promise((ok) => server.listen(0, ok));
  const base = `http://localhost:${server.address().port}`;
  const browser = await chromium.launch();

  // A. the corpus
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const errors = [];
    let now = '';
    page.on('pageerror', (e) => errors.push(`${now}: ${String(e)}`));
    for (const c of corpus.valid) {
      now = c.name;
      const b = buildText(`component C {\n    ${c.requires ? `requires ${c.requires};\n    ` : ''}${corpus.state}\n    view { Column { ${c.view} } }\n}\n`);
      if (b.code !== 0) { check(`A: ${c.name}: builds`, false, b.err.slice(0, 160)); continue; }
      await page.goto(`${base}/${b.id}/`);
      await page.waitForFunction(() => document.getElementById('root') && document.getElementById('root').childNodes.length > 0);
      const got = await page.evaluate(SERIALISE);
      check(`A: ${c.name}: the page is the DOM the corpus says`, got === c.html, got === c.html ? '' : `\n  got  ${got}\n  want ${c.html}`);
    }
    check('A: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // G. prerender (docs/prerender.md): the page that was written at build time, read with JavaScript OFF, is the corpus's DOM: all of it for a view
  // with no hole, and the DOM with the holes left out (`html_static`) for a view that has some. Then the script runs and the static parts are
  // what they were (A has the whole DOM with script).
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], javaScriptEnabled: false, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    for (const c of corpus.valid) {
      const b = buildText(`component C {\n    ${c.requires ? `requires ${c.requires};\n    ` : ''}${corpus.state}\n    view { Column { ${c.view} } }\n}\n`);
      if (b.code !== 0) { check(`G: ${c.name}: builds`, false, b.err.slice(0, 160)); continue; }
      await page.goto(`${base}/${b.id}/`);
      const got = await page.evaluate(SERIALISE);
      const want = c.html_static !== undefined ? c.html_static : c.html;
      check(`G: ${c.name}: without script the page is ${c.html_static !== undefined ? 'its static part' : 'the whole DOM'}`, got === want, got === want ? '' : `\n  got  ${got}\n  want ${want}`);
    }
    await ctx.close();
  }

  // G (continued). the page of examples/structure before and after its script: written at build time, the script held back for 1.5 s
  {
    const exs = build(path.join(ROOT, 'examples/structure/structure.cui'));
    check('G: the example builds with a prerendered page', exs.code === 0, exs.err.slice(0, 160));
    const html = fs.readFileSync(path.join(work, 'site', exs.id, 'index.html'), 'utf8');
    check('G: the page carries the one script that goes to a fragment, after the root and before the module', (html.match(/scrollIntoView/g) || []).length === 1 && html.indexOf('<div id="root">') < html.indexOf('scrollIntoView') && html.indexOf('scrollIntoView') < html.indexOf('type="module"'));
    check('G: and says some parts need script, as the example has buttons', html.includes('<noscript>Some parts of this page need JavaScript.</noscript>') && !html.includes('This app needs JavaScript.'));
    // without script: no control at all, and nothing is a hole's leftover
    const off = await browser.newContext({ ...devices['Pixel 7'], javaScriptEnabled: false, serviceWorkers: 'block' });
    const op = await off.newPage();
    await op.goto(`${base}/${exs.id}/`);
    const bare = await op.evaluate(() => ({ controls: document.querySelectorAll('button, input, textarea, select').length, headings: [...document.querySelectorAll('h1,h2,h3')].map((h) => h.textContent), text: document.body.innerText.length, ids: [...document.querySelectorAll('[id]')].length }));
    check('G: without script a visitor is shown headings and text and no control that does nothing', bare.controls === 0 && bare.headings.length >= 6 && bare.text > 300, JSON.stringify(bare));
    await off.close();
    holdApp = 1500;
    // a short window, so that the page is taller than the screen and #demo can come to the top
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 412, height: 300 }, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.addInitScript(() => { window.__shifts = 0; new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__shifts += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); });
    const where = () => page.evaluate(() => ({ y: Math.round(scrollY), top: Math.round(document.getElementById('demo').getBoundingClientRect().top), max: document.documentElement.scrollHeight - innerHeight }));
    const atTarget = (w) => Math.abs(w.top) <= 2 || w.y >= w.max - 1;
    // (a) opened at a fragment: the page is there before the script, and after it
    await page.goto(`${base}/${exs.id}/#demo`, { waitUntil: 'commit' });
    await page.waitForSelector('h1');
    await sleep(500);
    const before = { ...(await where()), buttons: await page.evaluate(() => document.querySelectorAll('button').length) };
    check('G: before the script the page is already at #demo (or as far down as it goes), with no button yet', atTarget(before) && before.buttons === 0, JSON.stringify(before));
    await page.waitForFunction(() => document.querySelectorAll('button').length > 0, null, { timeout: 8000 });
    await sleep(300);
    const after = await page.evaluate(() => { const ids = [...document.querySelectorAll('[id]')].map((e) => e.id); return { dup: ids.length - new Set(ids).size, roots: document.querySelectorAll('#root').length, shifts: window.__shifts, h1: document.querySelectorAll('h1').length }; });
    const afterAt = await where();
    check('G: after the script nothing is duplicated, the page is still at #demo, and nothing moved', after.dup === 0 && after.roots === 1 && after.h1 === 1 && atTarget(afterAt) && after.shifts < 0.01, JSON.stringify({ before, after, afterAt }));
    // (b) a link after the parts the script draws, focused before the script: the same link has the focus after it. Not with a fragment in
    // the address: the browser runs its fragment navigation again when the page has loaded, which clears the focus (measured: it does so on a page with
    // no prerender as well)
    await page.goto(`${base}/${exs.id}/`, { waitUntil: 'commit' });
    await page.waitForSelector('footer a');
    await page.locator('footer a').first().focus();
    const focusBefore = await page.evaluate(() => document.activeElement.textContent);
    await page.waitForFunction(() => document.querySelectorAll('button').length > 0, null, { timeout: 8000 });
    await sleep(300);
    const focusAfter = await page.evaluate(() => [document.activeElement.tagName, document.activeElement.textContent, document.activeElement.closest('footer') !== null]);
    check('G: a link focused before the script, below the parts it draws, has the focus after it', focusAfter[0] === 'A' && focusAfter[1] === focusBefore && focusAfter[2], JSON.stringify({ focusBefore, focusAfter }));
    // (c) the same, where the part the script draws is a sibling BEFORE the link, so the link has another place in the tree once the script has run
    const sib = buildText('component C {\n    state n: Int = 0;\n    on Up { n = n + 1; }\n    view { Column { Heading { level: 1 "Title" } Button { label: "Up" on: Up } Link { to: "https://a.example/" "A link" } Link { to: "https://b.example/" "Another link" } } }\n}\n');
    check('G: the sibling program builds', sib.code === 0, sib.err.slice(0, 160));
    await page.goto(`${base}/${sib.id}/`, { waitUntil: 'commit' });
    await page.waitForSelector('h1');
    check('G: before the script the links are the only things after the heading', (await page.evaluate(() => document.querySelectorAll('#root button').length)) === 0);
    await page.locator('a', { hasText: 'Another link' }).focus();
    await page.waitForFunction(() => document.querySelectorAll('#root button').length > 0, null, { timeout: 8000 });
    await sleep(300);
    const moved = await page.evaluate(() => [document.activeElement.tagName, document.activeElement.textContent, [...document.querySelectorAll('#root a')].indexOf(document.activeElement)]);
    check('G: a link that has another place in the tree after the script (a button came before it) still has the focus', moved[0] === 'A' && moved[1] === 'Another link', JSON.stringify(moved));
    holdApp = 0;
    await ctx.close();
  }

  // H. hydration (docs/prerender.md section 13): the script builds only the holes, so what a visitor did to the page before it ran is still there
  // after it, and after a render. The same scenario is measured against the earlier compiler by scripts/measure-hydration.js.
  {
    const lines = Array.from({ length: 40 }, (_, i) => `line ${i + 1} of the code box`).join('\\n');
    const src = `component Page {\n    state n: Int = 0;\n    on Up { n = n + 1; }\n    view {\n        Column {\n            Heading { level: 1 "Hydration" }\n            Rich { "Select " Strong { "this sentence" } " before the script runs." }\n            Details { summary: "More" Text { "Hidden text." } }\n            Code { max_height: 4 "${lines}" }\n            Link { to: "https://a.example/" "A link" }\n            Button { label: "Up" on: Up }\n            Text { "Count: {n}" }\n        }\n    }\n}\n`;
    const b = buildText(src);
    check('H: the hydration program builds', b.code === 0, b.err.slice(0, 160));
    const html = fs.readFileSync(path.join(work, 'site', b.id, 'index.html'), 'utf8');
    check('H: the page carries its skeleton: a hash on the root and a marker pair for each of the two holes', /<div id="root" data-cui-skeleton="[0-9a-f]{16}">/.test(html) && (html.match(/<!--cui:\d+--><!--\/cui:\d+-->/g) || []).length === 2, html.slice(html.indexOf('<div id="root"'), html.indexOf('<div id="root"') + 120));
    holdApp = 1500;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 412, height: 700 }, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.addInitScript(() => { window.__shifts = 0; new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__shifts += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); });
    await page.goto(`${base}/${b.id}/`, { waitUntil: 'commit' });
    await page.waitForSelector('h1');
    const STATIC = 'h1, p, strong, details, summary, pre, code, a';
    const before = await page.evaluate((sel) => {
      const root = document.getElementById('root');
      const range = document.createRange();
      range.selectNodeContents(root.querySelector('p'));
      const s = getSelection();
      s.removeAllRanges();
      s.addRange(range);
      root.querySelector('details').open = true;
      root.querySelector('pre').scrollTop = 40;
      root.querySelector('a').focus();
      window.__static = [...root.querySelectorAll(sel)];
      return { buttons: root.querySelectorAll('button').length, selected: String(s), scroll: root.querySelector('pre').scrollTop, nodes: window.__static.length };
    }, STATIC);
    check('H: before the script the visitor has selected a sentence, opened the details, scrolled the code and focused the link, and there is no button', before.buttons === 0 && before.selected.startsWith('Select this sentence') && before.scroll > 0 && before.nodes === 8, JSON.stringify(before));
    await page.waitForFunction(() => document.querySelectorAll('#root button').length > 0, null, { timeout: 8000 });
    await sleep(300);
    const probe = () => page.evaluate((sel) => {
      const root = document.getElementById('root');
      const now = [...root.querySelectorAll(sel)];
      return { selected: String(getSelection()), open: root.querySelector('details').open, scroll: root.querySelector('pre').scrollTop, focus: document.activeElement === root.querySelector('a'), same: now.length === window.__static.length && now.every((n, i) => n === window.__static[i]), count: ([...root.querySelectorAll('span')].find((x) => x.textContent.startsWith('Count:')) || { textContent: '' }).textContent, shifts: window.__shifts };
    }, STATIC);
    const a = await probe();
    check('H: after the script the selection, the open details, the scroll of the code and the focus of the link are still there', a.selected === before.selected && a.open && a.scroll === before.scroll && a.focus, JSON.stringify(a));
    check('H: and every static node is the very node the page had, none replaced', a.same && a.shifts < 0.01, JSON.stringify({ same: a.same, shifts: a.shifts }));
    await page.locator('#root button').click();
    await sleep(200);
    const k = await probe();
    check('H: a click that changes the state redraws the hole and nothing else: the count is 1, the static nodes are still the very nodes, the details is open and the code is where it was', k.count === 'Count: 1' && k.same && k.open && k.scroll === before.scroll, JSON.stringify(k));
    check('H: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
    holdApp = 0;
    // a script built for another page: the hash on the root does not match, so the script leaves the page as it is
    const other = buildText(src.replace('"Hydration"', '"Another title"'));
    const mixed = path.join(work, 'site', 'mixed');
    fs.mkdirSync(mixed, { recursive: true });
    fs.copyFileSync(path.join(work, 'site', b.id, 'index.html'), path.join(mixed, 'index.html'));
    fs.copyFileSync(path.join(work, 'site', other.id, 'app.js'), path.join(mixed, 'app.js'));
    const c2 = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const p2 = await c2.newPage();
    const logs = [];
    const errs2 = [];
    p2.on('console', (m) => { if (m.type() === 'error') { logs.push(m.text()); } });
    p2.on('pageerror', (e) => errs2.push(String(e)));
    await p2.goto(`${base}/mixed/`);
    await sleep(500);
    const left = await p2.evaluate(() => ({ h1: document.querySelector('h1') && document.querySelector('h1').textContent, buttons: document.querySelectorAll('button').length }));
    check('H: a script built for another page leaves the page as it is, says so, and throws nothing', left.h1 === 'Hydration' && left.buttons === 0 && logs.some((l) => l.includes('does not carry the markup')) && errs2.length === 0, JSON.stringify({ left, logs, errs2 }));
    await c2.close();
  }

  // B. the example, through the accessibility tree
  const ex = build(path.join(ROOT, 'examples/structure/structure.cui'));
  check('B: the example builds', ex.code === 0, ex.err.slice(0, 160));
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 412, height: 900 }, serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.goto(`${base}/${ex.id}/`);
    await page.getByRole('heading', { level: 1 }).waitFor();
    const cdp = await ctx.newCDPSession(page);
    const ax = (await cdp.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored);
    const prop = (n, name) => { const p = (n.properties || []).find((x) => x.name === name); return p ? p.value.value : undefined; };
    const role = (n) => n.role && n.role.value;
    const nm = (n) => (n.name && n.name.value) || '';
    const roles = {};
    ax.forEach((n) => { roles[role(n)] = (roles[role(n)] || 0) + 1; });
    check('B: landmarks: one banner, one main, one contentinfo, one navigation and one complementary', roles.banner === 1 && roles.main === 1 && roles.contentinfo === 1 && roles.navigation === 1 && roles.complementary === 1, JSON.stringify({ banner: roles.banner, main: roles.main, contentinfo: roles.contentinfo, navigation: roles.navigation, complementary: roles.complementary }));
    check('B: the navigation and the complementary have the names the program gave', ax.some((n) => role(n) === 'navigation' && nm(n) === 'Sections') && ax.some((n) => role(n) === 'complementary' && nm(n) === 'Notes'));
    check('B: no section is made a landmark by being a section (no `region`)', !roles.region, String(roles.region));
    const levels = ax.filter((n) => role(n) === 'heading').map((n) => prop(n, 'level'));
    let skipped = false;
    levels.forEach((l, i) => { if (i > 0 && l > levels[i - 1] + 1) { skipped = true; } });
    check('B: the headings, in order, skip no level', levels.length >= 6 && !skipped, levels.join(' '));
    const links = ax.filter((n) => role(n) === 'link');
    check('B: every link has a name, and none is a generic one', links.length >= 8 && links.every((n) => nm(n).trim() !== '' && !/^(here|click here|link|read more|more)$/i.test(nm(n).trim())), links.map(nm).join(' | '));
    check('B: the details has its summary as a disclosure and its text is hidden until it is opened', await page.evaluate(() => { const d = document.querySelector('details'); return d && d.querySelector('summary') && !d.open && !d.querySelector('code').checkVisibility(); }));
    check('B: the figure is named by its caption', ax.some((n) => role(n) === 'figure') && await page.evaluate(() => document.querySelector('figure figcaption').textContent === 'One minute.'));
    // in-page links reach their target
    const before = await page.evaluate(() => Math.round(document.getElementById('demo').getBoundingClientRect().top));
    await page.getByRole('link', { name: 'Demo', exact: true }).click();
    await sleep(600);
    const at = await page.evaluate(() => ({ hash: location.hash, top: Math.round(document.getElementById('demo').getBoundingClientRect().top) }));
    check('B: a link to #demo goes to the section', at.hash === '#demo' && at.top < before && at.top >= -1 && at.top < 900, JSON.stringify({ before, ...at }));
    // targets
    const sizes = await page.evaluate(() => [...document.querySelectorAll('header a, nav a, footer a, a[class^="cui-link-"]')].map((a) => { const r = a.getBoundingClientRect(); return [a.textContent, Math.round(r.height), Math.round(r.width)]; }));
    check('B: the links of a header, a nav and a footer, and the appearances, are at least 44 px high', sizes.length >= 8 && sizes.every((s) => s[1] >= 44), JSON.stringify(sizes.filter((s) => s[1] < 44)));
    check('B: an appearance is at least 44 px wide too', sizes.filter((s) => true).length > 0 && (await page.evaluate(() => [...document.querySelectorAll('a[class^="cui-link-"]')].every((a) => a.getBoundingClientRect().width >= 44))));
    // focus: a fresh page, so that the first Tab is the first control
    await page.goto(`${base}/${ex.id}/`);
    await page.getByRole('heading', { level: 1 }).waitFor();
    await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => { const s = getComputedStyle(document.activeElement); return [document.activeElement.tagName, s.outlineStyle, parseFloat(s.outlineWidth)]; });
    check('B: Tab reaches a link and it has a focus ring', ring[0] === 'A' && ring[1] !== 'none' && ring[2] >= 1, JSON.stringify(ring));
    await ctx.close();
  }

  // C. a new screen takes the focus on its Heading (a routed program: examples/routes with Heading in place of a heading text, the detail screen's inside a Section, so that the heading is found by the selector and not only as a direct child)
  {
    const src = fs.readFileSync(path.join(ROOT, 'examples/routes/routes.cui'), 'utf8')
      .replace('Text { role: heading "Machines" }', 'Heading { level: 1 "Machines" }')
      .replace('Text { role: heading "Machine {item}" }', 'Section { Heading { level: 2 "Machine {item}" } }')
      .replace('Text { role: heading "Not found" }', 'Heading { level: 2 "Not found" }');
    const b = buildText(src);
    check('C: the routes example with Heading builds', b.code === 0, b.err.slice(0, 160));
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.goto(`${base}/${b.id}/`);
    await page.getByRole('link', { name: 'valve' }).click();
    await page.getByRole('heading', { name: 'Machine valve' }).waitFor();
    await sleep(400);
    const focused = await page.evaluate(() => [document.activeElement.tagName, document.activeElement.textContent]);
    check('C: the screen that appeared has the focus on its Heading, as on a heading text', focused[0] === 'H2' && focused[1] === 'Machine valve', JSON.stringify(focused));
    await ctx.close();
  }

  // D. a themed page draws each appearance at 4.5:1 or more in both schemes
  {
    const theme = fs.readFileSync(path.join(ROOT, 'examples/theme/theme.cui'), 'utf8').split('component Palette')[0];
    fs.mkdirSync(path.join(work, 'src', 'fonts'), { recursive: true });
    fs.copyFileSync(path.join(ROOT, 'examples/theme/fonts/jetbrains-mono-latin-wght-normal.woff2'), path.join(work, 'src/fonts/jetbrains-mono-latin-wght-normal.woff2'));
    const body = fs.readFileSync(path.join(ROOT, 'examples/structure/structure.cui'), 'utf8');
    const f = path.join(work, 'src', 'themed.cui');
    fs.copyFileSync(path.join(ROOT, 'examples/structure/agents.md'), path.join(work, 'src', 'agents.md'));
    fs.writeFileSync(f, theme + body);
    const b = build(f);
    check('D: the themed structure example builds', b.code === 0, b.err.slice(0, 160));
    const hex = (css) => { const m = /rgba?\((\d+), (\d+), (\d+)/.exec(css); return '#' + [1, 2, 3].map((i) => Number(m[i]).toString(16).padStart(2, '0')).join(''); };
    for (const scheme of ['light', 'dark']) {
      const ctx = await browser.newContext({ ...devices['Pixel 7'], colorScheme: scheme, serviceWorkers: 'block' });
      const page = await ctx.newPage();
      await page.goto(`${base}/${b.id}/`);
      await page.getByRole('heading', { level: 1 }).waitFor();
      const drawn = await page.evaluate(() => {
        const css = (el) => { const s = getComputedStyle(el); let bg = s.backgroundColor, e = el; while ((bg === 'rgba(0, 0, 0, 0)' || bg === 'transparent') && e.parentElement) { e = e.parentElement; bg = getComputedStyle(e).backgroundColor; } return [s.color, bg]; };
        const q = (sel) => document.querySelector(sel);
        return { primary: css(q('a.cui-link-primary')), secondary: css(q('a.cui-link-secondary')), plain: css(q('header a')), eyebrow: css(q('[data-style="eyebrow"]')), lead: css(q('[data-style="lead"]')), note: css(q('[data-style="note"]')), code: css(q('.cui-code')) };
      });
      for (const [name, [fg, bg]] of Object.entries(drawn)) {
        const r = ratio(hex(fg), hex(bg));
        check(`D: ${scheme}: ${name} is drawn at ${r.toFixed(2)}:1, at least 4.5`, r >= 4.5);
      }
      await ctx.close();
    }
  }

  // E. the copy button: the clipboard is written once, by a tap, with the text of the box, and never read
  {
    const WATCH = `(function () {
      window.__clip = { writes: [], reads: 0 };
      const c = navigator.clipboard;
      if (!c) { return; }
      const w = c.writeText.bind(c), r = c.readText.bind(c);
      c.writeText = (t) => { window.__clip.writes.push(String(t)); return w(t); };
      c.readText = () => { window.__clip.reads += 1; return r(); };
    })();`;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block', permissions: ['clipboard-read', 'clipboard-write'] });
    await ctx.addInitScript(WATCH);
    const page = await ctx.newPage();
    await page.goto(`${base}/${ex.id}/`);
    await page.getByRole('heading', { level: 1 }).waitFor();
    await page.getByRole('button', { name: 'Click', exact: true }).click(); // a render
    await sleep(300);
    check('E: the page has not touched the clipboard at load or at a render', (await page.evaluate(() => window.__clip.writes.length + window.__clip.reads)) === 0);
    await page.evaluate(() => document.querySelector('details').setAttribute('open', ''));
    const wanted = await page.evaluate(() => document.querySelector('.cui-code pre').textContent);
    const btn = page.getByRole('button', { name: 'Copy the code' });
    check('E: the copy button is at least 44 x 44 and named', await btn.evaluate((b) => { const r = b.getBoundingClientRect(); return r.width >= 44 && r.height >= 44; }));
    await btn.click();
    await sleep(300);
    const after = await page.evaluate(async () => ({ writes: window.__clip.writes, reads: window.__clip.reads, text: await navigator.clipboard.readText(), label: document.querySelector('.cui-copy').textContent }));
    check('E: a tap writes the text of the box, once, and says so', after.writes.length === 1 && after.writes[0] === wanted && after.text === wanted && after.label === 'Copied', JSON.stringify({ n: after.writes.length, label: after.label }));
    check('E: the page itself never read the clipboard', after.reads === 0, String(after.reads));
    await ctx.close();
    // the clipboard refused, and absent: the text is selected for Ctrl+C and the button says what to do
    for (const [name, init, want] of [['refused', 'navigator.clipboard.writeText = () => Promise.reject(new Error("no"));', 'Select and copy'], ['absent', 'Object.defineProperty(navigator, "clipboard", { value: undefined });', 'Press Ctrl+C']]) {
      const c2 = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
      await c2.addInitScript(init);
      const p2 = await c2.newPage();
      await p2.goto(`${base}/${ex.id}/`);
      await p2.getByRole('heading', { level: 1 }).waitFor();
      await p2.evaluate(() => document.querySelector('details').setAttribute('open', ''));
      await p2.getByRole('button', { name: 'Copy the code' }).click();
      await sleep(300);
      const got = await p2.evaluate(() => ({ label: document.querySelector('.cui-copy').textContent, selected: String(getSelection()) }));
      check(`E: with the clipboard ${name}, the button says "${want}" and the text is selected`, got.label === want && got.selected.includes('cargo build'), JSON.stringify(got));
      await c2.close();
    }
  }

  // F. Image, Media and Grid
  {
    const src = `component C {
    view {
        Column {
            Grid { min: 17 gap: 3 Text { "a" } Text { "b" } Text { "c" } Text { "d" } Text { "e" } }
            Grid { min: 18 gap: 3 weights: [1, 1.75] Column { Text { "code" } } Column { Text { "text" } } }
            Grid { min: 10 gap: 3 weights: [1, 2, 4] Column { Text { "a" } } Column { Text { "b" } } Column { Text { "c" } } }
            Grid { min: 22 gap: 3 Column { Text { "a" } } Column { Text { "b" } } }
            Image { src: "assets/real.png" alt: "A crop" width: 1024 height: 768 ratio: "16:9" }
            Image { src: "assets/real2.png" alt: "Blue squares" width: 400 height: 300 }
            Image { src: "assets/real.png" decorative: true width: 100 height: 100 }
            Image { src: "assets/real.png" alt: "Hero" width: 100 height: 100 priority: true }
            Figure {
                Media { label: "The demo" kind: "video" src: ["assets/v.mp4", "assets/v.webm"] poster: "assets/real.png" ratio: "16:9" caption: "cap" transcript: "tr" }
                Caption { id: "cap" "One minute of the demo." }
            }
            Figure {
                Media { label: "A playable clip" kind: "video" src: "assets/tiny.webm" caption: "cap2" }
                Caption { id: "cap2" "Half a second of blue." }
            }
            Details { id: "tr" summary: "As text" Rich { "what is said" } }
        }
    }
}
`;
    const b = buildText(src);
    check('F: the page of pictures, video and grids builds', b.code === 0, b.err.slice(0, 200));
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const origins = new Set();
    page.on('request', (r) => origins.add(new URL(r.url()).origin));
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    // the second picture arrives late: the space it will fill must already be held
    await page.route('**/real2.png', async (route) => { await sleep(1200); await route.continue(); });
    await page.goto(`${base}/${b.id}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => document.querySelectorAll('picture').length === 4);
    const before = await page.evaluate(() => { const p = document.querySelectorAll('picture')[1]; const r = p.getBoundingClientRect(); return { h: r.height, w: r.width, done: p.querySelector('img').complete }; });
    await page.waitForFunction(() => document.querySelectorAll('picture')[1].querySelector('img').complete, null, { timeout: 10000 });
    const after = await page.evaluate(() => { const p = document.querySelectorAll('picture')[1]; const r = p.getBoundingClientRect(); return { h: r.height, w: r.width, nat: p.querySelector('img').naturalWidth }; });
    check('F: an image holds its space before its file arrives (width and height attributes): no layout shift', !before.done && after.nat === 80 && Math.abs(before.h - after.h) <= 1 && before.h > 50, JSON.stringify({ before, after }));
    check('F: an image is never drawn wider than its file size says', await page.evaluate(() => [...document.querySelectorAll('picture')].every((p) => p.getBoundingClientRect().width <= parseFloat(p.style.maxWidth) + 0.5)));
    const crop = await page.evaluate(() => { const p = document.querySelectorAll('picture')[0]; const r = p.getBoundingClientRect(); const i = p.querySelector('img').getBoundingClientRect(); return { ratio: r.width / r.height, imgRatio: i.width / i.height, fit: getComputedStyle(p.querySelector('img')).objectFit }; });
    check('F: ratio: "16:9" crops the picture to 16:9 and the file is covered, not stretched', Math.abs(crop.ratio - 16 / 9) < 0.01 && Math.abs(crop.imgRatio - 16 / 9) < 0.01 && crop.fit === 'cover', JSON.stringify(crop));
    check('F: priority loads at once; the others are lazy and decode asynchronously', await page.evaluate(() => { const imgs = [...document.querySelectorAll('picture img')]; return imgs[3].loading === 'eager' && imgs[3].getAttribute('fetchpriority') === 'high' && imgs.slice(0, 3).every((i) => i.loading === 'lazy' && i.decoding === 'async'); }));
    const cdp = await ctx.newCDPSession(page);
    const ax = (await cdp.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored);
    const role = (n) => n.role && n.role.value;
    const nm = (n) => (n.name && n.name.value) || '';
    const imgs = ax.filter((n) => role(n) === 'image' || role(n) === 'img');
    check('F: a picture with alt is an image named by its alt, a decorative one is not in the accessibility tree', imgs.some((n) => nm(n) === 'A crop') && imgs.some((n) => nm(n) === 'Blue squares') && imgs.some((n) => nm(n) === 'Hero') && imgs.length === 3, imgs.map(nm).join(' | '));
    // the video
    const vid = await page.evaluate(() => { const v = document.querySelector('video'); return { autoplay: v.autoplay, hasAutoplay: v.hasAttribute('autoplay'), paused: v.paused, controls: v.controls, playsinline: v.hasAttribute('playsinline'), preload: v.preload, describedby: v.getAttribute('aria-describedby'), details: v.getAttribute('aria-details'), sources: [...v.querySelectorAll('source')].map((s) => s.getAttribute('type')), fallback: v.querySelector('a').textContent, poster: v.getAttribute('poster'), ratio: v.getBoundingClientRect().width / v.getBoundingClientRect().height }; });
    check('F: a video has controls, plays inline, loads only its metadata, and does not play by itself', vid.controls && vid.playsinline && vid.preload === 'metadata' && !vid.autoplay && !vid.hasAutoplay && vid.paused, JSON.stringify(vid));
    check('F: the video is described by its caption and has the transcript as its details; its sources are typed; its fallback names it', vid.describedby === 'cap' && vid.details === 'tr' && vid.sources.join() === 'video/mp4,video/webm' && vid.fallback === 'The demo' && vid.poster === 'assets/real.png', JSON.stringify(vid));
    check('F: a video of ratio 16:9 is drawn 16:9', Math.abs(vid.ratio - 16 / 9) < 0.02, String(vid.ratio));
    // a video that cannot load is named by Chromium "Unable to play media." (measured), so the name is read on the clip that plays
    await page.waitForFunction(() => { const v = document.querySelectorAll('video')[1]; return v.readyState >= 1; }, null, { timeout: 10000 });
    await sleep(1000);
    const ax2 = (await cdp.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored);
    const vnodes = ax2.filter((n) => /^video$/i.test(String(role(n))));
    const playable = vnodes.find((n) => nm(n) === 'A playable clip');
    check('F: the accessibility tree names a video by its label and gives it the caption as its description', !!playable && playable.description && playable.description.value === 'Half a second of blue.', JSON.stringify(vnodes.map((n) => [nm(n), n.description && n.description.value])));
    const played = await page.evaluate(() => { const v = document.querySelectorAll('video')[1]; return { paused: v.paused, t: v.currentTime, ready: v.readyState, w: v.videoWidth }; });
    check('F: a video with its metadata loaded has not started by itself', played.paused && played.t === 0 && played.w === 64, JSON.stringify(played));

    // Grid: columns against a formula written here (the CSS grid algorithm for repeat(auto-fit, minmax(min(100%, M), 1fr)) with five items)
    function columnsFormula(W, gapPx, minPx, items) {
      const m = Math.min(W, minPx);
      const n = Math.max(1, Math.floor((W + gapPx) / (m + gapPx)));
      return Math.min(n, items);
    }
    const widths = [320, 360, 375, 412, 480, 600, 700, 768, 900, 1024, 1280, 1440];
    let gridOk = true; const gridDetail = [];
    let weightOk = true; const weightDetail = [];
    for (const w of widths) {
      await page.setViewportSize({ width: w, height: 900 });
      const m = await page.evaluate(() => {
        const grids = [...document.querySelectorAll('.cui-grid')];
        const info = (g) => ({ W: g.clientWidth, gap: parseFloat(getComputedStyle(g).columnGap), kids: [...g.children].map((c) => { const r = c.getBoundingClientRect(); return { left: Math.round(r.left), top: Math.round(r.top), w: r.width }; }) });
        return { a: info(grids[0]), b: info(grids[1]), c: info(grids[2]), e: info(grids[3]), overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth };
      });
      const cols = new Set(m.a.kids.map((k) => k.left)).size;
      const want = columnsFormula(m.a.W, m.a.gap, 17 * 16, 5);
      if (cols !== want) { gridOk = false; gridDetail.push(`${w}: ${cols} columns, formula ${want}`); }
      // weights [1, 1.75] at min 18: side by side when the line holds 2.75 x 18 rem and the gap
      const side = m.b.W >= 2.75 * 18 * 16 + m.b.gap;
      const [k1, k2] = m.b.kids;
      if (side) {
        const ok = Math.abs(k2.w - 1.75 * k1.w) <= 1.75 && Math.abs(k1.w + k2.w + m.b.gap - m.b.W) <= 1 && k1.top === k2.top;
        if (!ok) { weightOk = false; weightDetail.push(`${w}: ${k1.w.toFixed(1)} + ${k2.w.toFixed(1)} in ${m.b.W}`); }
      } else {
        const ok = Math.abs(k1.w - m.b.W) <= 1 && Math.abs(k2.w - m.b.W) <= 1 && k2.top > k1.top;
        if (!ok) { weightOk = false; weightDetail.push(`${w}: stacked expected, ${k1.w.toFixed(1)} and ${k2.w.toFixed(1)} in ${m.b.W}, tops ${k1.top} ${k2.top}`); }
      }
      // two items in a Grid of min 22 (the page's .videos): as many columns as fit, never more than there are items, and the row is filled; a column
      // is never wider than the box, at 320 px too (the minimum is `min(100%, 22rem)`)
      {
        const per = columnsFormula(m.e.W, m.e.gap, 22 * 16, 2);
        const lefts = new Set(m.e.kids.map((k) => k.left)).size;
        const rowWidth = m.e.kids.slice(0, per).reduce((t, k) => t + k.w, 0) + (per - 1) * m.e.gap;
        if (lefts !== per || Math.abs(rowWidth - m.e.W) > 1 || m.e.kids.some((k) => k.w > m.e.W + 1)) { gridOk = false; gridDetail.push(`${w}: two items in min 22: ${lefts} columns (formula ${per}), row ${rowWidth.toFixed(1)} of ${m.e.W}`); }
      }
      if (m.overflow > 0) { gridOk = false; gridDetail.push(`${w}: scrolls sideways by ${m.overflow}px`); }
      // three weights [1, 2, 4], min 10: all on a line when it holds 7 x 10 rem plus two gaps: then the widths are 1 : 2 : 4
      const [c1, c2, c3] = m.c.kids;
      if (m.c.W >= 7 * 10 * 16 + 2 * m.c.gap) {
        const ok = Math.abs(c2.w - 2 * c1.w) <= 2 && Math.abs(c3.w - 4 * c1.w) <= 4 && c1.top === c3.top;
        if (!ok) { weightOk = false; weightDetail.push(`${w}: 1:2:4 expected, ${c1.w.toFixed(1)} ${c2.w.toFixed(1)} ${c3.w.toFixed(1)}`); }
      }
    }
    check(`F: a Grid has the columns the formula says at ${widths.length} widths from 320 to 1440, and the page never scrolls sideways`, gridOk, gridDetail.join('; '));
    check('F: a Grid with weights is 1 : 1.75 (within 2 px) side by side, and stacked, each the full width, below the width the formula says', weightOk, weightDetail.join('; '));
    // the breakpoint of the weighted grid, found by looking: the width at which the two columns come to one line
    let lo = 320, hi = 1440;
    const sideBySide = async (w) => { await page.setViewportSize({ width: w, height: 900 }); return page.evaluate(() => { const g = document.querySelectorAll('.cui-grid')[1]; const [a, b] = g.children; return a.getBoundingClientRect().top === b.getBoundingClientRect().top; }); };
    while (hi - lo > 1) { const mid = (lo + hi) >> 1; if (await sideBySide(mid)) { hi = mid; } else { lo = mid; } }
    const gridW = await page.evaluate(() => document.querySelectorAll('.cui-grid')[1].clientWidth);
    const wantAt = 2.75 * 18 * 16 + 16 + (hi - gridW); // the viewport at which the grid is as wide as the formula says
    check('F: the weighted grid changes from stacked to side by side at the width the formula says (within 1 px)', Math.abs(2.75 * 18 * 16 + 16 - gridW) <= 1, JSON.stringify({ viewport: hi, gridWidth: gridW, formula: 2.75 * 18 * 16 + 16, wantAt }));
    // 320 px: an Image of 1024 px, a 16:9 video, and a Grid hold the page to the width of the screen
    await page.setViewportSize({ width: 320, height: 900 });
    check('F: at 320 px nothing is wider than the screen', await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth));
    // the weights are not pixels of the program: a Grid in a narrow column keeps its rule
    const foreign = [...origins].filter((o) => o !== base);
    check('F: the page asked nothing of any origin but its own', foreign.length === 0, foreign.join(' '));
    check('F: no script error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  await browser.close();
  server.close();
  fs.rmSync(work, { recursive: true, force: true });
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
