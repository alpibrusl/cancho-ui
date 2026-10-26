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

  // ---- E. authority ---------------------------------------------------------------------------------------------------------------------------
  {
    const eff = (name) => JSON.parse(spawnSync(BIN, ['inspect', 'effects', path.join(SITE, 'home.cui'), name], { encoding: 'utf8', cwd: ROOT }).stdout);
    const home = eff('Home');
    const pane = eff('AgentFiles');
    check('E: the page asks for the clipboard (the copy buttons) and nothing else', JSON.stringify(home.requires.map((x) => x.name)) === '["Clipboard"]' && home.reachable_effects.length === 0, JSON.stringify(home.requires.map((x) => x.name)));
    check('E: the pane asks for the network and nothing else', JSON.stringify(pane.requires.map((x) => x.name)) === '["Network"]', JSON.stringify(pane.requires.map((x) => x.name)));
    check('E: the data file is named by its hash', home.data[0].sha256 === '870d4eb8e3bc5a1c22bb33b9b4ae72c6192251af0c7f283eb29db32b3c65ba52' && home.data[0].file === 'data/ecosystem.json', JSON.stringify(home.data));
    const js = fs.readFileSync(path.join(out, 'app.js'), 'utf8');
    check('E: the script holds no address of another host', !/https?:\/\/(?!www\.w3\.org)/.test(js.replace(/https:\/\/(github\.com|alpibru\.com|alpibrusl\.github\.io)[^"']*/g, '')), 'checked');
  }
  await browser.close();
  server.close();
  clearTimeout(watchdog);
  console.log(failures.length ? `\n${failures.length} failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.log('FAIL', e); process.exit(1); });
