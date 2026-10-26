#!/usr/bin/env node
// The landing-page gallery (docs/index.html #gallery, docs/gallery.json): outside the gate, because it needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-gallery.js [path/to/cancho-ui]     the rules, on the repository's docs/
//   node scripts/check-gallery.js --self-test [cancho-ui]  the same rules against deliberately broken copies: each must fail
// The rules:
//   R1 no drift          the section in index.html is exactly what scripts/build-gallery.js writes from gallery.json
//   R2 built = source    every supported tile's example compiles, and docs/demo/<id>/ is byte-identical to a fresh build
//   R3 loads clean       every supported page loads in Chromium with no console error, page error or failed request
//   R4 soon is tracked   every coming-soon tile says what is missing and links exactly one issue of the repository
//   R5 soon is true      every coming-soon tile's probe still shows the feature absent (a refused program, a string
//                        absent from the sources, a sentence in a doc): a tile cannot outlive the gap it names
//   R6 no overflow       no horizontal scroll at 420 px, in both colour schemes
//   R7 accessible        headings in order, every iframe titled, text contrast >= 4.5:1 in both schemes, a visible focus ring
//   R8 counts are true   the landing page's example counts match what the repository holds
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');
const G = require('./build-gallery.js');

const ROOT = G.ROOT;
const args = process.argv.slice(2);
const SELF_TEST = args.includes('--self-test');
const CANCHO = args.find((a) => !a.startsWith('--')) || path.join(ROOT, 'target', 'debug', 'cancho-ui');

function walk(dir, base = dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { walk(p, base, out); } else { out.push(path.relative(base, p)); }
  }
  return out.sort();
}
function copyDir(from, to) { fs.cpSync(from, to, { recursive: true }); }

function serve(docs) {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json', '.json': 'application/json', '.mp4': 'video/mp4', '.webm': 'video/webm', '.jpg': 'image/jpeg' };
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    if (url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    let f = path.join(docs, path.normalize(url));
    if (url.endsWith('/')) { f = path.join(f, 'index.html'); }
    if (!f.startsWith(docs) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': types[path.extname(f)] || 'text/plain', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

// --- the rules; each returns a list of problems (empty = holds) -----------------------------------------------------

function r1(ctx) {
  const html = fs.readFileSync(path.join(ctx.docs, 'index.html'), 'utf8');
  const a = html.indexOf(G.START);
  const b = html.indexOf(G.END);
  if (a < 0 || b < 0) { return ['index.html has no gallery markers']; }
  const have = html.slice(a, b + G.END.length);
  const want = G.renderSection(ctx.manifest);
  const problems = have === want ? [] : ['the gallery section differs from what build-gallery.js writes from gallery.json'];
  if (!/<a class="opt" href="#gallery">/.test(html)) { problems.push('the nav has no link to #gallery'); }
  return problems;
}

function r2(ctx) {
  const problems = [];
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'gallery-fresh-'));
  try {
    G.buildDemos(ctx.manifest, CANCHO, tmp);
  } catch (e) {
    return [String(e.message).split('\n')[0]];
  }
  for (const t of ctx.manifest.supported) {
    const fresh = path.join(tmp, t.id);
    const have = path.join(ctx.docs, 'demo', t.id);
    if (!fs.existsSync(have)) { problems.push(`${t.id}: docs/demo/${t.id}/ is missing`); continue; }
    const a = walk(fresh);
    const b = walk(have);
    if (JSON.stringify(a) !== JSON.stringify(b)) { problems.push(`${t.id}: files differ (${a.join(',')} vs ${b.join(',')})`); continue; }
    for (const f of a) {
      if (!fs.readFileSync(path.join(fresh, f)).equals(fs.readFileSync(path.join(have, f)))) { problems.push(`${t.id}: ${f} is not what a fresh build writes`); }
    }
  }
  // the hero demo, docs/demo/counter.js, is a plain build of the same example
  const plain = path.join(tmp, 'counter.js');
  const r = spawnSync(CANCHO, ['build', 'examples/counter/counter.cui', '-o', plain], { encoding: 'utf8', cwd: ROOT });
  if (r.status !== 0 || !fs.readFileSync(plain).equals(fs.readFileSync(path.join(ctx.docs, 'demo', 'counter.js')))) {
    problems.push('docs/demo/counter.js is not what a fresh build of examples/counter writes');
  }
  return problems;
}

async function r3(ctx) {
  const problems = [];
  for (const t of ctx.manifest.supported) {
    const page = await ctx.browser.newPage();
    const errs = [];
    page.on('console', (m) => { if (m.type() === 'error') { errs.push(m.text()); } });
    page.on('pageerror', (e) => errs.push(String(e)));
    page.on('requestfailed', (r) => errs.push(`request failed: ${r.url()}`));
    page.on('response', (r) => { if (r.status() >= 400) { errs.push(`HTTP ${r.status()}: ${r.url()}`); } });
    await page.goto(`${ctx.url}/demo/${t.id}/`);
    await page.waitForFunction(() => document.getElementById('root') && document.getElementById('root').innerText.trim().length > 0, null, { timeout: 8000 }).catch(() => errs.push('the page rendered nothing'));
    if (t.id === 'ops-console') {
      await page.waitForFunction(() => /of 23 machines/.test(document.body.innerText), null, { timeout: 8000 }).catch(() => errs.push('the console never listed its 23 machines'));
    }
    // the first button that can be pressed (the upload demo's Send is disabled until a file is chosen)
    const button = page.locator('button:enabled').first();
    if (await button.count()) { await button.click().catch((e) => errs.push(`click: ${e.message}`)); }
    await page.waitForTimeout(250);
    if (errs.length) { problems.push(`${t.id}: ${errs.slice(0, 3).join(' | ')}`); }
    await page.close();
  }
  // the landing page itself, with every lazy iframe scrolled into view
  const page = await ctx.browser.newPage();
  const errs = [];
  page.on('console', (m) => { if (m.type() === 'error') { errs.push(m.text()); } });
  page.on('pageerror', (e) => errs.push(String(e)));
  await page.goto(`${ctx.url}/index.html`);
  for (const f of await page.locator('#gallery iframe').all()) { await f.scrollIntoViewIfNeeded(); }
  await page.waitForTimeout(800);
  if ((await page.locator('#gallery iframe').count()) !== ctx.manifest.supported.length) { errs.push('the landing page does not hold one iframe per supported tile'); }
  if (errs.length) { problems.push(`index.html: ${errs.slice(0, 3).join(' | ')}`); }
  await page.close();
  return problems;
}

function r4(ctx) {
  const problems = [];
  const html = fs.readFileSync(path.join(ctx.docs, 'index.html'), 'utf8');
  const sec = html.slice(html.indexOf(G.START), html.indexOf(G.END));
  const tiles = sec.split('<li class="gtile soon">').slice(1).map((s) => s.split('</li>')[0]);
  if (tiles.length !== ctx.manifest.soon.length || tiles.length === 0) { problems.push(`${tiles.length} coming-soon tiles on the page, ${ctx.manifest.soon.length} in the manifest`); }
  for (const [i, tile] of tiles.entries()) {
    const name = (ctx.manifest.soon[i] || {}).title || `tile ${i}`;
    const links = [...tile.matchAll(/href="([^"]+)"/g)].map((m) => m[1]);
    const issue = links.filter((l) => new RegExp(`^${ctx.manifest.repo.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/issues/\\d+$`).test(l));
    if (links.length !== 1 || issue.length !== 1) { problems.push(`${name}: expected exactly one link, to an issue of the repository; got ${JSON.stringify(links)}`); }
    if (!/Missing: .{20,}/.test(tile.replace(/<[^>]+>/g, ''))) { problems.push(`${name}: it does not say what is missing`); }
    if (/\b(20\d\d|Q[1-4]|soon\b.*\b(week|month)|will (ship|land|be))/i.test(tile.replace(/<span class="badge">Coming soon<\/span>/, '').replace(/<[^>]+>/g, ''))) { problems.push(`${name}: it states a date or a promise`); }
  }
  return problems;
}

function r5(ctx) {
  const problems = [];
  const sources = [];
  const crates = path.join(ctx.root, 'crates');
  for (const f of walk(crates)) { if (f.endsWith('.rs') && !f.includes('/tests/')) { sources.push(fs.readFileSync(path.join(crates, f), 'utf8')); } }
  for (const t of ctx.manifest.soon) {
    const p = t.probe;
    if (!p) { problems.push(`${t.title}: no probe`); continue; }
    if (p.view) {
      const src = `component P {\n    state s: String = "";\n    state n: Int = 0;\n    on Go { n = n + 1; }\n    view {\n        Column {\n            ${p.view}\n        }\n    }\n}\n`;
      const f = path.join(os.tmpdir(), `gallery-probe-${process.pid}-${t.id}.cui`);
      fs.writeFileSync(f, src);
      const r = spawnSync(CANCHO, ['check', f, '--output', 'json'], { encoding: 'utf8' });
      let codes = [];
      try { codes = JSON.parse(r.stdout).map((d) => d.code); } catch (e) { /* not JSON: the program passed */ }
      if (!codes.includes(p.rule)) { problems.push(`${t.title}: \`${p.view}\` is no longer refused with ${p.rule} (got ${JSON.stringify(codes)}); the tile claims it is missing`); }
    } else if (p.absent) {
      for (const needle of p.absent) {
        if (sources.some((s) => s.includes(needle))) { problems.push(`${t.title}: "${needle}" now appears in crates/; the tile claims it is missing`); }
      }
    } else if (p.doc) {
      if (!fs.readFileSync(path.join(ctx.root, p.doc), 'utf8').includes(p.contains)) { problems.push(`${t.title}: ${p.doc} no longer says "${p.contains}"`); }
    } else { problems.push(`${t.title}: a probe of an unknown kind`); }
  }
  return problems;
}

async function pageAt(ctx, width, scheme) {
  const context = await ctx.browser.newContext({ viewport: { width, height: 900 }, colorScheme: scheme });
  const page = await context.newPage();
  await page.goto(`${ctx.url}/index.html`);
  return { page, context };
}

async function r6(ctx) {
  const problems = [];
  for (const scheme of ['light', 'dark']) {
    const { page, context } = await pageAt(ctx, 420, scheme);
    const wide = await page.evaluate(() => ({ doc: document.documentElement.scrollWidth, view: document.documentElement.clientWidth, over: Array.from(document.querySelectorAll('#gallery *')).filter((e) => e.getBoundingClientRect().right > document.documentElement.clientWidth + 0.5).map((e) => e.tagName + '.' + e.className).slice(0, 3) }));
    if (wide.doc > wide.view) { problems.push(`${scheme}: the page scrolls sideways at 420 px (${wide.doc} > ${wide.view})`); }
    if (wide.over.length) { problems.push(`${scheme}: ${wide.over.join(', ')} reach past the viewport`); }
    await context.close();
  }
  return problems;
}

async function r7(ctx) {
  const problems = [];
  const { page, context } = await pageAt(ctx, 1000, 'light');
  const heads = await page.evaluate(() => Array.from(document.querySelectorAll('#gallery h2, #gallery h3, #gallery h4')).map((h) => Number(h.tagName[1])));
  if (heads[0] !== 2 || heads.some((h, i) => i > 0 && h - heads[i - 1] > 1)) { problems.push(`headings are out of order: ${heads.join(',')}`); }
  const untitled = await page.evaluate(() => Array.from(document.querySelectorAll('#gallery iframe')).filter((f) => !(f.getAttribute('title') || '').trim()).length);
  if (untitled) { problems.push(`${untitled} iframe(s) without a title`); }
  const emptyLinks = await page.evaluate(() => Array.from(document.querySelectorAll('#gallery a')).filter((a) => !a.textContent.trim()).length);
  if (emptyLinks) { problems.push(`${emptyLinks} link(s) without text`); }
  await context.close();
  for (const scheme of ['light', 'dark']) {
    const c = await pageAt(ctx, 1000, scheme);
    const bad = await c.page.evaluate(() => {
      const parse = (s) => { const m = s.match(/rgba?\(([^)]+)\)/); const v = m[1].split(',').map((x) => parseFloat(x)); return { r: v[0], g: v[1], b: v[2], a: v.length > 3 ? v[3] : 1 }; };
      const lum = ({ r, g, b }) => { const f = (x) => { x /= 255; return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4); }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
      const backdrop = (el) => {
        const layers = [];
        for (let e = el; e; e = e.parentElement) { const c = parse(getComputedStyle(e).backgroundColor); if (c.a > 0) { layers.push(c); if (c.a >= 1) { break; } } }
        let base = { r: 255, g: 255, b: 255 };
        if (!layers.length || layers[layers.length - 1].a < 1) { base = parse(getComputedStyle(document.body).backgroundColor); }
        for (const c of layers.reverse()) { base = { r: c.r * c.a + base.r * (1 - c.a), g: c.g * c.a + base.g * (1 - c.a), b: c.b * c.a + base.b * (1 - c.a) }; }
        return base;
      };
      const out = [];
      for (const el of document.querySelectorAll('#gallery h2, #gallery h3, #gallery h4, #gallery p, #gallery a, #gallery .badge, #gallery code')) {
        if (!el.textContent.trim()) { continue; }
        const fg = parse(getComputedStyle(el).color);
        const bg = backdrop(el);
        const l1 = lum(fg); const l2 = lum(bg);
        const ratio = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
        if (ratio < 4.5) { out.push(`${el.tagName.toLowerCase()}${el.className ? '.' + el.className : ''} ${ratio.toFixed(2)}`); }
      }
      return out.slice(0, 4);
    });
    if (bad.length) { problems.push(`${scheme}: contrast below 4.5:1: ${bad.join('; ')}`); }
    await c.context.close();
  }
  const k = await pageAt(ctx, 1000, 'light');
  await k.page.locator('#gallery a').first().focus();
  const ring = await k.page.evaluate(() => { const s = getComputedStyle(document.activeElement); return { style: s.outlineStyle, width: parseFloat(s.outlineWidth) }; });
  await k.page.keyboard.press('Tab');
  const ring2 = await k.page.evaluate(() => { const s = getComputedStyle(document.activeElement); return { tag: document.activeElement.tagName, style: s.outlineStyle, width: parseFloat(s.outlineWidth) }; });
  if (ring.style === 'none' || ring.width < 2) { problems.push(`a focused gallery link has no visible focus ring (${ring.style} ${ring.width}px)`); }
  if (ring2.tag === 'A' && (ring2.style === 'none' || ring2.width < 2)) { problems.push('the next link in tab order has no visible focus ring'); }
  await k.context.close();
  return problems;
}

function r8(ctx) {
  const problems = [];
  const html = fs.readFileSync(path.join(ctx.docs, 'index.html'), 'utf8');
  const ex = path.join(ctx.root, 'examples');
  const programs = walk(ex).filter((f) => f.endsWith('.cui'));
  let ran = 0;
  const replayed = new Set();
  for (const f of programs) {
    const r = spawnSync(CANCHO, ['check', path.join(ex, f)], { encoding: 'utf8' });
    if (r.status !== 0) { problems.push(`${f} does not compile`); }
    const dir = path.dirname(path.join(ex, f));
    for (const t of fs.readdirSync(dir).filter((x) => x.endsWith('trace.json'))) {
      const rr = spawnSync(CANCHO, ['test', path.join(ex, f), path.join(dir, t)], { encoding: 'utf8' });
      if (rr.status === 0) { replayed.add(f); }
    }
    ran++;
  }
  const m1 = html.match(/<b>(\d+)<\/b><span>example programs compile, and (\d+) of them replay a recorded trace/);
  const m2 = html.match(/\((\d+) programs\)/);
  if (!m1 || Number(m1[1]) !== programs.length || Number(m1[2]) !== replayed.size) { problems.push(`the page says ${m1 ? m1[1] + ' compile, ' + m1[2] + ' replay' : 'nothing'}; the repository has ${programs.length} programs, ${replayed.size} replaying a trace`); }
  if (!m2 || Number(m2[1]) !== programs.length) { problems.push(`the page says "(${m2 ? m2[1] : '?'} programs)"; the repository has ${programs.length}`); }
  if (/Nine more examples/.test(html)) { problems.push('the stale sentence "Nine more examples" is still on the page'); }
  const words = ['none', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten', 'eleven', 'twelve', 'thirteen', 'fourteen', 'fifteen', 'sixteen'];
  const live = html.match(/(\w+) of them run live/);
  if (live && words.indexOf(live[1].toLowerCase()) !== ctx.manifest.supported.length) { problems.push(`the page says ${live[1].toLowerCase()} run live; the manifest has ${ctx.manifest.supported.length}`); }
  return problems;
}

const RULES = [
  ['R1 no drift', r1], ['R2 built = source', r2], ['R3 loads clean', r3], ['R4 soon is tracked', r4],
  ['R5 soon is true', r5], ['R6 no overflow at 420 px', r6], ['R7 accessible', r7], ['R8 counts are true', r8],
];

// --- the self-test: one deliberate break per rule -------------------------------------------------------------------

function editFile(f, fn) { fs.writeFileSync(f, fn(fs.readFileSync(f, 'utf8'))); }

const BREAKS = [
  ['R1 no drift', 'a word changed in a tile', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('Missing: Push', 'Missing: PUSH')), 'r1'],
  ['R2 built = source', 'a demo is not what the source builds', (c) => editFile(path.join(c.docs, 'demo/clock/app.js'), (s) => s + '\n// edited\n'), 'r2'],
  ['R3 loads clean', 'a demo throws when it loads', (c) => editFile(path.join(c.docs, 'demo/match/app.js'), (s) => s + '\nthrow new Error("broken demo");\n'), 'r3'],
  ['R4 soon is tracked', 'a coming-soon tile loses its issue link', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace(/<p class="glinks"><a href="[^"]*\/issues\/68">Tracked in #68<\/a><\/p>/, '<p class="glinks"></p>')), 'r4'],
  ['R4 soon is tracked', 'a coming-soon tile promises a date', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('Push notifications and background work', 'Push notifications and background work, arriving in Q4 2027')), 'r4'],
  ['R5 soon is true', 'a probe that now passes (the feature exists)', (c) => { c.manifest.soon[0].probe = { view: 'Text { "x" }', rule: 'component-kind-unknown' }; }, 'r5'],
  ['R6 no overflow at 420 px', 'a tile wider than the screen', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('</style>', '.gtile { min-width: 34rem !important; }\n</style>')), 'r6'],
  ['R7 accessible', 'an iframe without a title', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('title="Counter, built by cancho-ui" ', '')), 'r7'],
  ['R7 accessible', 'low contrast text', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('</style>', '.gtile p { color: #c8c8c8 !important; }\n</style>')), 'r7'],
  ['R7 accessible', 'no focus ring', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('</style>', '.gallery a:focus, .gallery a:focus-visible { outline: none !important; }\n</style>')), 'r7'],
  ['R7 accessible', 'headings skip a level', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('<h3 class="gh">Built with cancho-ui</h3>', '<h5 class="gh">Built with cancho-ui</h5>')), 'r7'],
  ['R8 counts are true', 'the page says 17 programs', (c) => editFile(path.join(c.docs, 'index.html'), (s) => s.replace('(34 programs)', '(17 programs)')), 'r8'],
];

async function runRule(name, ctx) {
  const fn = RULES.find((r) => r[0] === name)[1];
  return fn(ctx);
}

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const failures = [];
  const report = (label, problems, expectFail) => {
    const ok = expectFail ? problems.length > 0 : problems.length === 0;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${problems.length ? ': ' + problems[0] : ''}`);
    if (!ok) { failures.push(label); }
  };

  if (!SELF_TEST) {
    const manifest = G.loadManifest();
    const server = await serve(path.join(ROOT, 'docs'));
    const ctx = { docs: path.join(ROOT, 'docs'), root: ROOT, manifest, browser, url: `http://127.0.0.1:${server.address().port}` };
    for (const [name] of RULES) { report(name, await runRule(name, ctx), false); }
    server.close();
  } else {
    for (const [rule, what, mutate, _key] of BREAKS) {
      const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'gallery-break-'));
      copyDir(path.join(ROOT, 'docs'), path.join(tmp, 'docs'));
      const manifest = JSON.parse(JSON.stringify(G.loadManifest()));
      const server = await serve(path.join(tmp, 'docs'));
      const ctx = { docs: path.join(tmp, 'docs'), root: ROOT, manifest, browser, url: `http://127.0.0.1:${server.address().port}` };
      mutate(ctx);
      report(`break (${what}) is caught by ${rule}`, await runRule(rule, ctx), true);
      server.close();
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  }
  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
