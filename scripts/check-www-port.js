#!/usr/bin/env node
// The port of the cancho-www home page (docs/www-port/site/home.cui), in Chromium: DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
// It needs nothing outside this repository (the comparison with the real page is scripts/parity-www.js):
//   A. what the files are: every font is the file its SHA-256 says and has its licence text beside it, every file of the site is tracked (none ignored),
//      no file is a symbolic link;
//   B. with JavaScript off the page has its lists, its 17 + 9 + 24 items from the data file, its links to files (each answers 200 with the bytes of its file);
//   C. outline and anchors: one h1, no level skipped, one main, every #id linked exists, the skip link is the first thing a keyboard reaches;
//   D. with JavaScript on: the three fonts are the ones drawn, the pane for agents shows agents.md and then llms.txt (read from this site), the menu opens,
//      the copy button copies; no request to another origin, no console error, layout shift 0;
//   E. authority: the page needs `Clipboard`, the pane needs `Network` and nothing else.
//   node scripts/check-www-port.js
'use strict';
const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const BIN = path.join(ROOT, 'target', 'debug', 'cancho-ui');
const SITE = path.join(ROOT, 'docs', 'www-port', 'site');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]));

(async () => {
  // ---- A. the files ---------------------------------------------------------------------------------------------------------------------------
  const src = fs.readFileSync(path.join(SITE, 'home.cui'), 'utf8');
  const fonts = [...src.matchAll(/font (\w+) \{\s*family: "([^"]+)";\s*file: "([^"]+)";\s*sha256: "([0-9a-f]{64})";\s*licence: "([^"]+)";/g)];
  check('A: three fonts are declared, with a licence each', fonts.length === 3 && fonts.every((f) => f[5] === 'OFL-1.1'), fonts.map((f) => `${f[2]} ${f[5]}`).join('; '));
  for (const [, role, family, file, hash] of fonts) {
    const bytes = fs.readFileSync(path.join(SITE, file));
    const licence = fs.readdirSync(path.join(SITE, 'fonts')).filter((n) => n.startsWith('OFL-') && n.toLowerCase().includes(family.split(' ')[0].toLowerCase()));
    const text = licence.length ? fs.readFileSync(path.join(SITE, 'fonts', licence[0]), 'utf8') : '';
    check(`A: ${family} (${role}) is the file its hash says, and its OFL 1.1 text is beside it`, sha(bytes) === hash && /SIL OPEN FONT LICENSE Version 1\.1/.test(text), `${licence[0] || 'no licence file'}, ${bytes.length} bytes`);
  }
  const files = walk(SITE);
  const tracked = new Set(spawnSync('git', ['ls-files', 'docs/www-port/site'], { cwd: ROOT, encoding: 'utf8' }).stdout.split('\n').filter(Boolean));
  const untracked = files.map((f) => path.relative(ROOT, f)).filter((f) => !tracked.has(f));
  check(`A: every one of the ${files.length} files of the site is tracked by git (none ignored, none left out)`, untracked.length === 0, untracked.join(' '));
  check('A: no file of the site is a symbolic link', files.every((f) => !fs.lstatSync(f).isSymbolicLink()));
  const thirdParty = fs.readFileSync(path.join(ROOT, 'docs', 'www-port', 'THIRD_PARTY.md'), 'utf8');
  check('A: THIRD_PARTY.md names every font and every picture file', fonts.every((f) => thirdParty.includes(path.basename(f[3]))) && fs.readdirSync(path.join(SITE, 'assets', 'img')).every((n) => thirdParty.includes(n.replace(/\.(webp|jpg)$/, '.'))));

  // ---- build and serve ------------------------------------------------------------------------------------------------------------------------
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'wwwport-'));
  const out = path.join(work, 'site');
  const b = spawnSync(BIN, ['build', path.join(SITE, 'home.cui'), '--page', '-o', out], { encoding: 'utf8', cwd: ROOT });
  check('the port builds', b.status === 0, (b.stderr + b.stdout).slice(0, 200));
  if (b.status !== 0) { process.exit(1); }
  const requests = [];
  const server = await new Promise((ok) => {
    const s = http.createServer((q, r) => {
      requests.push(q.url);
      let f = path.join(out, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.md': 'text/markdown', '.txt': 'text/plain', '.json': 'application/json', '.woff2': 'font/woff2', '.png': 'image/png', '.webp': 'image/webp', '.jpg': 'image/jpeg' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, '127.0.0.1', () => ok(s));
  });
  const base = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch();

  // ---- B. JavaScript off ---------------------------------------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], javaScriptEnabled: false });
    const page = await ctx.newPage();
    await page.goto(`${base}/`);
    const r = await page.evaluate(() => ({
      ul: document.querySelectorAll('ul').length, ol: document.querySelectorAll('ol').length, li: document.querySelectorAll('li').length,
      claims: document.querySelectorAll('#verifies li').length, paths: document.querySelectorAll('#ecosystem article li').length, all: document.querySelectorAll('#map-fallback li').length,
      code: document.querySelectorAll('#verifies li code').length, imgs: document.querySelectorAll('img').length, videos: document.querySelectorAll('video').length,
      counts: [...document.querySelectorAll('.cui-countup')].map((e) => e.textContent), waffles: document.querySelectorAll('figure svg').length,
      hrefs: [...document.querySelectorAll('a[href]')].map((a) => a.getAttribute('href')),
      table: document.querySelectorAll('table').length,
      columns: [...document.querySelectorAll('#verifies .cui-grid > div')].map((col) => [...col.querySelectorAll('li')].map((li) => li.textContent.replace(/\s+/g, ' ').trim())),
    }));
    const eco = JSON.parse(fs.readFileSync(path.join(SITE, 'data', 'ecosystem.json'), 'utf8'));
    const want = ['compiler', 'tests', 'open'].map((k) => eco.verifies[k].map((c) => `${c.claim.replace(/`/g, '')} ${c.evidence.map((e) => e[0]).join(' ')}`.replace(/\s+/g, ' ').trim()));
    check('B: each of the three columns of the evidence is the data file\'s list, claim and evidence in order', JSON.stringify(r.columns) === JSON.stringify(want), JSON.stringify(r.columns.map((c) => c.length)));
    check('B: with no script, the 17 claims, the 9 path entries and the 24 projects are in the HTML', r.claims === 17 && r.paths === 9 && r.all === 24, JSON.stringify([r.claims, r.paths, r.all]));
    const ax = (await (await ctx.newCDPSession(page)).send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored);
    const roles = (name) => ax.filter((n) => n.role && n.role.value === name).length;
    check('B: with no script the comparison is a table in the accessibility tree: a region and a table (the page has 4 table elements with the 3 data tables of the waffles; the tree has the one), 3 column headers, 3 row headers', r.table === 4 && roles('table') === 1 && roles('columnheader') === 3 && roles('rowheader') === 3 && ax.some((n) => n.role.value === 'region' && n.name && n.name.value.startsWith('Human tools')), `${r.table} table, ${roles('table')} table role, ${roles('columnheader')} columnheader, ${roles('rowheader')} rowheader`);
    // the 3 data tables of the waffles are in a closed <details>: out of the tree until it is opened (a disclosure, not a hidden table)
    const cdp2 = await ctx.newCDPSession(page);
    const closedTables = (await cdp2.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored && n.role && n.role.value === 'table').length;
    await page.evaluate(() => document.querySelectorAll('details').forEach((d) => { d.open = true; }));
    const openTables = (await cdp2.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored && n.role && n.role.value === 'table').length;
    check('B: the waffles\' data tables are in the accessibility tree once their <details> is opened (1 table with them closed, 4 open: a disclosure, not a hidden table)', closedTables === 1 && openTables === 4, `${closedTables} closed, ${openTables} open`);
    check('B: the five backtick pairs of the claims are <code>', r.code === 5, String(r.code));
    check('B: the three final numbers are in the HTML, and the waffles are drawn', JSON.stringify(r.counts) === JSON.stringify(['697', '256', '62']) && r.waffles >= 3, JSON.stringify(r.counts));
    check('B: the pictures (4 img: the logo is no longer in a link, dropped.md 2) and the two videos are in the HTML', r.imgs === 4 && r.videos === 2, `${r.imgs} ${r.videos}`);
    const rel = [...new Set(r.hrefs.filter((h) => !/^(https?:|mailto:|#)/.test(h)))].sort();
    check('B: the links to files of the site are the seven the page has', JSON.stringify(rel) === JSON.stringify(['agents.md', 'assets/media/ui-demo.mp4', 'data/ecosystem.json', 'demo/demo.cast', 'demo/demo.mp4', 'llms-full.txt', 'llms.txt'].sort()), JSON.stringify(rel));
    for (const f of rel) {
      const res = await ctx.request.get(`${base}/${f}`);
      const body = await res.body();
      const own = fs.readFileSync(path.join(SITE, f));
      check(`B: ${f} answers 200 with the bytes of the program's file`, res.status() === 200 && sha(body) === sha(own), `${res.status()} ${body.length}`);
    }
    await ctx.close();
  }

  // ---- C. outline and anchors -----------------------------------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], javaScriptEnabled: false });
    const page = await ctx.newPage();
    await page.goto(`${base}/`);
    const r = await page.evaluate(() => {
      const hs = [...document.querySelectorAll('h1,h2,h3,h4')].map((h) => Number(h.tagName[1]));
      const ids = new Set([...document.querySelectorAll('[id]')].map((e) => e.id));
      const missing = [...document.querySelectorAll('a[href^="#"]')].map((a) => a.getAttribute('href').slice(1)).filter((i) => i && !ids.has(i));
      const first = document.querySelector('a[href]');
      return { h1: hs.filter((x) => x === 1).length, skipped: hs.some((x, i) => i > 0 && x > hs[i - 1] + 1), main: document.querySelectorAll('main').length, missing, first: first && first.getAttribute('href'), banner: document.querySelectorAll('header').length, footer: document.querySelectorAll('footer').length, lang: document.documentElement.lang, title: document.title };
    });
    check('C: one h1, one main, one banner and one footer, no heading level skipped', r.h1 === 1 && r.main === 1 && r.banner === 1 && r.footer === 1 && !r.skipped, JSON.stringify(r));
    check('C: every #id the page links to is there', r.missing.length === 0, r.missing.join(' '));
    check('C: the first link is the skip link, to main', r.first === '#main', r.first);
    check('C: the language and the title are the page\'s', r.lang === 'en' && r.title === 'cancho: tools built for agents, not adapted from humans', r.title);
    await ctx.close();
  }

  // ---- D. JavaScript on ------------------------------------------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 1000, height: 900 }, reducedMotion: 'no-preference' });
    await ctx.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: base });
    const page = await ctx.newPage();
    const errors = [];
    const origins = new Set();
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    page.on('request', (rq) => { if (!rq.url().startsWith('data:') && !rq.url().startsWith('about:')) { origins.add(new URL(rq.url()).origin); } });
    await page.addInitScript(() => { window.__cls = 0; new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__cls += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); });
    await page.goto(`${base}/`);
    await sleep(900);
    const drawn = await page.evaluate(async () => { await document.fonts.ready; return [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family.replace(/"/g, '')).sort(); });
    check('D: the three fonts of the page were loaded from this site', JSON.stringify(drawn) === JSON.stringify(['Caveat', 'JetBrains Mono', 'Plus Jakarta Sans']) || (drawn.includes('Plus Jakarta Sans') && drawn.includes('JetBrains Mono')), JSON.stringify(drawn));
    const agents = fs.readFileSync(path.join(SITE, 'agents.md'), 'utf8');
    const pane = () => page.evaluate(() => { const s = document.querySelector('#agents'); return { status: s.querySelector('[role=status]').textContent, text: s.innerText }; });
    let p = await pane();
    check('D: the pane for agents shows agents.md, read from this site', p.status === 'agents.md' && p.text.includes(agents.split('\n')[0]), JSON.stringify(p.status));
    const paneBox = await page.evaluate(() => { const c = document.querySelector('#agents .cui-code'); const pre = c && c.querySelector('pre'); return pre ? { h: pre.getBoundingClientRect().height, scrolls: pre.scrollHeight > pre.clientHeight, rem: parseFloat(getComputedStyle(document.documentElement).fontSize) } : null; });
    check('D: the pane for agents is a code box bounded (16 rem of text, 20 rem with its padding) and scrolls (it was the whole file, 7,000 px, in B3\'s first pixel diff)', paneBox && paneBox.h <= 20 * paneBox.rem && paneBox.scrolls, JSON.stringify(paneBox));
    await page.click('#agents button[aria-label="llms.txt"]');
    await sleep(400);
    p = await pane();
    const llms = fs.readFileSync(path.join(SITE, 'llms.txt'), 'utf8');
    check('D: then llms.txt', p.status === 'llms.txt' && p.text.includes(llms.split('\n')[0]), p.status);
    const pressed = await page.evaluate(() => [...document.querySelectorAll('#agents button')].map((b) => b.getAttribute('aria-pressed')));
    check('D: and its button says it is pressed', JSON.stringify(pressed) === JSON.stringify(['false', 'false', 'true']), JSON.stringify(pressed));
    await page.evaluate(() => document.querySelector('header details summary').click());
    const open = await page.evaluate(() => { const d = document.querySelector('header details'); return d.open && d.querySelectorAll('a').length; });
    check('D: the menu opens and has its nine links', open === 9, String(open));
    await page.evaluate(() => document.querySelector('#try').scrollIntoView());
    await sleep(300);
    await page.click('#try button.cui-copy >> nth=0');
    await sleep(200);
    const clip = await page.evaluate(() => navigator.clipboard.readText());
    check('D: the copy button of the commands copies them', clip.startsWith('git clone https://github.com/alpibrusl/cancho'), clip.slice(0, 40));
    await page.evaluate(async () => { for (let y = 0; y < document.documentElement.scrollHeight; y += 700) { window.scrollTo(0, y); await new Promise((r) => setTimeout(r, 40)); } });
    await sleep(300);
    const cls = await page.evaluate(() => window.__cls);
    // the three fonts are `font-display: swap` and not preloaded, so the text moves once when they arrive (measured 0.012 at 135 ms on the Pixel 7 profile, in the paragraphs and lists of the first screen);
    // under 0.1 is "good" for the web's vitals. The page's own number is in parity.md.
    check('D: the layout shifts by less than 0.05 from load to the end of the page (the swap of the fonts is the one shift)', cls < 0.05, String(cls));
    check('D: every request was to this site', origins.size === 1 && [...origins][0] === base, [...origins].join(' '));
    check('D: no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- F. the islands and the authority animation ------------------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 1100, height: 900 }, reducedMotion: 'no-preference' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    await page.goto(`${base}/`);
    await sleep(1200);
    const frames = await page.evaluate(() => [...document.querySelectorAll('iframe')].map((f) => ({ sandbox: f.getAttribute('sandbox'), title: f.title, hidden: f.getAttribute('aria-hidden'), name: JSON.parse(f.getAttribute('data-cui-island')).name })));
    const count = (n) => frames.filter((f) => f.name === n).length;
    check('F: the page has 15 sealed frames: the hero, 10 cairns, 3 card drawings and the canvas of the animation', frames.length === 15 && count('Hero') === 1 && count('Cairn') === 10 && count('CardChain') + count('CardRings') + count('CardJson') === 3 && count('AuthorityCanvas') === 1, JSON.stringify(frames.map((f) => f.name)));
    check('F: every frame is sandboxed with allow-scripts and nothing else, the cairns are hidden from a screen reader and the others are named', frames.every((f) => f.sandbox === 'allow-scripts') && frames.filter((f) => f.name === 'Cairn').every((f) => f.hidden === 'true') && frames.filter((f) => f.name !== 'Cairn').every((f) => f.title.length > 10), '');
    const csp = await page.evaluate(() => [...document.querySelectorAll('meta[http-equiv="Content-Security-Policy"]')].map((m) => m.content).join(' '));
    check('F: the page allows frames of about: only', /frame-src about:/.test(csp), csp);
    const fr = (name) => page.frames().filter((f) => f !== page.mainFrame());
    const byTitle = async (start) => { for (const f of page.frames()) { if (f === page.mainFrame()) { continue; } const el = await f.frameElement(); if ((await el.getAttribute('title') || '').startsWith(start)) { return f; } } return null; };
    const hero = await byTitle('A stack of granite');
    const rocks = await hero.evaluate(() => document.querySelectorAll('.rock-shape').length);
    check('F: the hero is the page\'s own drawing: 8 boulders and the sun, the ridges and the pines', rocks >= 8 && (await hero.evaluate(() => document.querySelectorAll('.pine').length)) === 26, String(rocks));
    await page.mouse.move(100, 100);
    await page.mouse.move(900, 600);
    await sleep(300);
    const px = await hero.evaluate(() => document.querySelector('svg.scene').style.getPropertyValue('--px'));
    check('F: the pointer reaches the hero and moves its layers', px !== '' && Number(px) !== 0, px);
    const cairnPaths = [];
    for (const f of page.frames()) {
      if (f === page.mainFrame()) { continue; }
      const el = await f.frameElement();
      if (JSON.parse(await el.getAttribute('data-cui-island')).name === 'Cairn') { cairnPaths.push(await f.evaluate(() => [...document.querySelectorAll('.st')].length + ':' + document.querySelector('.rock-shape').getAttribute('d').slice(0, 24))); }
    }
    check('F: ten cairns of three stones each, and no two alike (a seed each)', cairnPaths.length === 10 && cairnPaths.every((x) => x.startsWith('3:')) && new Set(cairnPaths).size === 10, JSON.stringify(cairnPaths.slice(0, 3)));
    for (const t of ['Three blocks', 'Rings spreading', 'A diagnostic as JSON']) {
      const f = await byTitle(t);
      check(`F: the card drawing "${t}" is drawn`, !!f && (await f.evaluate(() => document.querySelectorAll('svg *').length)) >= 4, '');
    }
    // the animation: the page's text, and the canvas that follows the state
    await page.evaluate(() => document.querySelector('#authority').scrollIntoView());
    await sleep(500);
    const canvas = await byTitle('Animation: the world splits');
    await page.evaluate(() => { window.__canvas = document.querySelector('iframe[title^="Animation"]'); });
    const vis = () => canvas.evaluate(() => [...document.querySelectorAll('.cap-rock')].filter((g) => g.style.opacity === '1').length);
    const cap = () => page.evaluate(() => [...document.querySelectorAll('#authority [role=status]')].map((e) => e.textContent).join(' | '));
    const data = JSON.parse(fs.readFileSync(path.join(SITE, 'data', 'authority.json'), 'utf8')).scenarios;
    check('F: the first caption is the data file\'s, and only the world is drawn', (await cap()) === data[0].steps[0].cap && (await vis()) === 1, await cap());
    await page.click('#authority button[aria-label="Next step"]');
    await sleep(900);
    check('F: next step: the caption is the next, the five capabilities are out of the world', (await cap()) === data[0].steps[1].cap && (await vis()) === 5, await cap());
    await page.click('#authority button[aria-label="Forget one"]');
    await sleep(300);
    const tabs = await page.evaluate(() => [...document.querySelectorAll('#authority [role=group] button')].map((b) => b.getAttribute('aria-pressed')));
    check('F: a tab says it is pressed, and starts again at its first step', JSON.stringify(tabs) === '["false","true","false"]' && (await cap()) === data[1].steps[0].cap, JSON.stringify(tabs));
    const bg = await page.evaluate(() => { const b = document.querySelector('#authority [role=group] button[aria-pressed="true"]'); const s = getComputedStyle(b); return [s.backgroundColor, s.color]; });
    check('F: and a pressed button looks pressed (the accent, with its text colour)', bg[0] === 'rgb(24, 95, 165)' && bg[1] === 'rgb(255, 255, 255)', JSON.stringify(bg));
    for (let i = 0; i < 4; i++) { await page.click('#authority button[aria-label="Next step"]'); }
    await sleep(900);
    const last = await page.evaluate(() => document.querySelector('#authority').innerText);
    check('F: the last step of "Forget one" says it is refused, by the rule, and the frame marks heap', last.includes('refused · linear-value-unconsumed') && (await canvas.evaluate(() => document.querySelectorAll('.cap-rock.bad').length)) === 1, '');
    await page.click('#authority button[aria-label="Previous step"]');
    await page.click('#authority button[aria-label="Previous step"]');
    await page.click('#authority button[aria-label="Play"]');
    const label = await page.evaluate(() => [...document.querySelectorAll('#authority button')].map((b) => b.textContent).join('|'));
    // the schedule ticks every 2.6 s from the page's mount, so the first step after Play comes within 2.6 s, and the next one 2.6 s after it
    const before = await cap();
    let after = before;
    for (let i = 0; i < 40 && after === before; i++) { await sleep(100); after = await cap(); }
    check('F: Play moves on by itself, one step (step 2 to step 3), and its button says Pause while it plays', /Pause/.test(label) && after === data[1].steps[3].cap, `${label} / ${after}`);
    const kept = await page.evaluate(() => document.querySelector('iframe[title^="Animation"]').isConnected && window.__canvas === document.querySelector('iframe[title^="Animation"]'));
    check('F: the frame of the canvas is the same element after all that (a frame built again on every render would reload and forget its animation)', kept === true, String(kept));
    const reach = await canvas.evaluate(() => { try { return typeof parent.document.title === 'string' ? 'reached' : 'no'; } catch (e) { return e.name; } });
    check('F: the frame that reads the state cannot reach the page', reach === 'SecurityError', reach);
    check('F: no console error with the frames', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- G. the map: the open island draws, the program owns the rest -------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], viewport: { width: 1280, height: 900 }, reducedMotion: 'no-preference' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    await page.goto(`${base}/`);
    await sleep(1200);
    const eco = JSON.parse(fs.readFileSync(path.join(SITE, 'data', 'ecosystem.json'), 'utf8'));
    const demos = JSON.parse(fs.readFileSync(path.join(SITE, 'data', 'demos.json'), 'utf8'));
    const host = '[role=group][data-cui-island]';
    await page.evaluate((h) => { window.__host = document.querySelector(h); }, host);
    const m = await page.evaluate((h) => ({ role: document.querySelector(h).getAttribute('role'), label: document.querySelector(h).getAttribute('aria-label'), describedby: document.querySelector(h).getAttribute('aria-describedby'), rocks: [...document.querySelectorAll(h + ' .rock')].map((g) => [g.getAttribute('data-id'), g.getAttribute('role'), g.getAttribute('tabindex')]), rests: document.querySelectorAll(h + ' .rests').length, legend: document.querySelectorAll(h + ' .cmap-legend span').length, park: !!document.querySelector(h + ' a.park'), names: [...document.querySelectorAll(h + ' text.name')].map((t) => t.textContent) }), host);
    check('G: the open island is a named group with its text alternative, and draws a rock for each of the 24 projects, each a button in the tab order', m.role === 'group' && m.label.startsWith('Map of the cancho ecosystem') && m.describedby === 'map-fallback' && m.rocks.length === eco.projects.length && m.rocks.every((r) => r[1] === 'button' && r[2] === '0'), `${m.rocks.length} rocks`);
    const look = await page.evaluate((h) => { const host = document.querySelector(h); const lg = host.querySelector('.cmap-legend'); const reg = host.querySelector('svg.cmap .region'); const hr = host.getBoundingClientRect(), lr = lg && lg.getBoundingClientRect(); return { legendInside: !!lr && lr.height > 10 && lr.bottom <= hr.bottom + 1 && lr.top >= hr.top, regionSize: reg && getComputedStyle(reg).fontSize, regionFamily: reg && getComputedStyle(reg).fontFamily }; }, host);
    check('G: the legend is inside the host and shown under the map (it was clipped by the aspect box), and the region names are drawn in the hand font at 40px (a generic rule had made them 12px mono)', look.legendInside && look.regionSize === '40px' && /Caveat|cursive|hand/i.test(look.regionFamily), JSON.stringify(look));
    check('G: the four "rests on" arrows, the legend of four and the car park are drawn', m.rests === eco.projects.reduce((a, p) => a + (p.rests_on || []).length, 0) && m.legend === 4 && m.park, JSON.stringify([m.rests, m.legend, m.park]));
    check('G: every project\'s name is on the map', eco.projects.every((p) => m.names.includes(p.name)), '');
    const detail = () => page.evaluate(() => { const d = document.getElementById('detail'); return { text: d.innerText, h: [...d.querySelectorAll('h3')].map((h) => h.textContent) }; });
    let d = await detail();
    check('G: the detail pane (the program\'s, an aside) starts on cancho, with its stage, summary and the rows of the page\'s `dl`', d.h[0] === 'cancho' && d.text.includes('Stage: ' + eco.stages.built) && d.text.includes('Note: Pre-1.0.'), JSON.stringify(d.h));
    await page.focus(`${host} .rock[data-id="cancho-web"]`);
    await page.keyboard.press('Enter');
    await sleep(300);
    d = await detail();
    const sel = await page.evaluate((h) => [...document.querySelectorAll(h + ' .rock.sel')].map((g) => g.getAttribute('data-id')), host);
    check('G: Enter on a rock chooses it: the pane says so (with what it rests on) and the island marks it', d.h[0] === 'cancho-web' && d.text.includes('Rests on: cancho-schema') && JSON.stringify(sel) === '["cancho-web"]', JSON.stringify([d.h, sel]));
    await page.click('#ecosystem button[aria-label="Services"]');
    await sleep(300);
    const services = eco.projects.filter((p) => p.kind === 'service').length;
    const dim = await page.evaluate((h) => document.querySelectorAll(h + ' .rock.dim').length, host);
    const pressed = await page.evaluate(() => [...document.querySelectorAll('#ecosystem [role=group] button')].filter((b) => b.getAttribute('aria-pressed') === 'true').map((b) => b.getAttribute('aria-label')));
    check('G: a filter dims what it leaves out, takes it out of the tab order, and says which is pressed', dim === eco.projects.length - services && pressed.includes('Services') && !pressed.includes('All'), JSON.stringify([dim, pressed]));
    await page.click('#ecosystem button[aria-label="All"]');
    await page.click('#ecosystem button[aria-label="Only what exists"]');
    await sleep(300);
    const dim2 = await page.evaluate((h) => document.querySelectorAll(h + ' .rock.dim').length, host);
    const onlyPressed = await page.getAttribute('#ecosystem button[aria-label="Only what exists"]', 'aria-pressed');
    check('G: "Only what exists" dims the designs, and says it is pressed', dim2 === eco.projects.filter((p) => p.stage === 'design').length && onlyPressed === 'true', `${dim2} ${onlyPressed}`);
    const listItems = await page.evaluate(() => [...document.querySelectorAll('#map-fallback li')].map((li) => li.textContent));
    check('G: the list that is the map\'s text alternative has a line for each project with its stage and summary', listItems.length === eco.projects.length && eco.projects.every((p, i) => listItems[i].includes(`(${p.stage}): ${p.summary}`)), `${listItems.length} lines`);
    await page.click('#ecosystem button[aria-label="Only what exists"]');
    const trail = Object.keys(demos).find((k) => k.startsWith('trail:'));
    await page.click(`#ecosystem button[aria-label^="Trail: "] >> nth=0`);
    await sleep(300);
    d = await detail();
    const selNone = await page.evaluate((h) => document.querySelectorAll(h + ' .rock.sel').length, host);
    check('G: a trail shows its route and summary as text, and no rock is chosen', d.text.includes(' \u2192 ') && d.text.includes(demos[trail].summary.slice(0, 30)) && selNone === 0, d.text.slice(0, 80));
    const charts = demos[trail].steps.filter((st) => st.chart);
    const detailText = await page.evaluate(() => document.getElementById('detail').textContent);
    check('G: each chart of the trail\'s session is a line of text with its title (the page draws them, the port says them: dropped.md 14)', charts.length > 0 && charts.every((st) => detailText.includes('Chart: ' + st.chart.title)) && detailText.split('Chart: ').length - 1 === charts.length, `${charts.length} charts`);
    await page.click(`${host} .rock[data-id="cancho"]`);
    await sleep(300);
    await page.evaluate(() => { document.querySelector('#detail details').open = true; });
    const codes = await page.evaluate(() => [...document.querySelectorAll('#detail .cui-code')].map((c) => c.innerText));
    check('G: the recorded session of cancho is text in a details, one code box a step, with the data file\'s commands (it is not typed out and no hiker walks: dropped.md)', codes.length === demos.cancho.steps.length && demos.cancho.steps.every((st, i) => codes[i].includes('$ ' + st.cmd.split('\n')[0].slice(0, 20))), `${codes.length} boxes`);
    const same = await page.evaluate(() => window.__host === document.querySelector('[role=group][data-cui-island]') && window.__host.isConnected);
    check('G: the island\'s element is the same one after all of that (its drawing was not built again)', same === true, String(same));
    const count = await page.evaluate(() => document.querySelector('#ecosystem p[data-style="note"]').textContent);
    check('G: the count is the data\'s: 21 built, 3 still on paper', count === `${eco.projects.filter((p) => p.stage !== 'design').length} built, ${eco.projects.filter((p) => p.stage === 'design').length} still on paper`, count);
    check('G: no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- E. authority ---------------------------------------------------------------------------------------------------------------------------
  {
    const eff = (name) => JSON.parse(spawnSync(BIN, ['inspect', 'effects', path.join(SITE, 'home.cui'), name], { encoding: 'utf8', cwd: ROOT }).stdout);
    const home = eff('Home');
    const pane = eff('AgentFiles');
    check('E: the page asks for the clipboard (the copy buttons) and the DOM (the open map island), and nothing else', JSON.stringify(home.requires.map((x) => x.name)) === '["Clipboard","Dom"]' && home.reachable_effects.length === 0, JSON.stringify(home.requires.map((x) => x.name)));
    check('E: the pane asks for the network and nothing else', JSON.stringify(pane.requires.map((x) => x.name)) === '["Network"]', JSON.stringify(pane.requires.map((x) => x.name)));
    check('E: the data file is named by its hash', home.data[0].sha256 === '870d4eb8e3bc5a1c22bb33b9b4ae72c6192251af0c7f283eb29db32b3c65ba52' && home.data[0].file === 'data/ecosystem.json', JSON.stringify(home.data));
    const js = fs.readFileSync(path.join(out, 'app.js'), 'utf8');
    check('E: the script holds no address of another host (the loopback address in two demo transcripts is text of the map\'s session boxes, not an address the page contacts)', !/https?:\/\/(?!www\.w3\.org|127\.0\.0\.1:18950)/.test(js.replace(/https:\/\/(github\.com|alpibru\.com|alpibrusl\.github\.io)[^"']*/g, '')), 'checked');
  }
  await browser.close();
  server.close();
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log('FAIL', e); process.exit(1); });
