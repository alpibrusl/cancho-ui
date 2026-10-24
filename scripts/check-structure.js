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
//   D. with a theme, a link of each appearance is drawn at a contrast of at least 4.5:1 in both colour schemes (the oracle's formula).
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
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'structure-'));
fs.mkdirSync(path.join(work, 'site'), { recursive: true });
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
      r.end(d);
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
    page.on('pageerror', (e) => errors.push(String(e)));
    for (const c of corpus.valid) {
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

  await browser.close();
  server.close();
  fs.rmSync(work, { recursive: true, force: true });
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log(`FAIL ${e.stack || e}`); process.exit(1); });
