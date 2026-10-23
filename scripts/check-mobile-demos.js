#!/usr/bin/env node
// The gallery demos as a phone gets them (docs/mobile-backend.md section 5.2, alpibrusl/cancho-ui#65): a mobile profile (a Pixel 7:
// 412 px wide, touch, an Android Chrome user agent), a static host serving what a commit would contain, not what is on the disk.
//   - the landing page with every lazy iframe scrolled into view: each demo renders, no request fails, no console or page error, and
//     viewing the page registers no service worker (a demo is a page, not an app);
//   - each demo opened on its own ("Open on its own"): it renders, links no manifest and the browser reports none
//     (Page.getAppManifest has an empty url), and registers no service worker. Whether Chrome would then show an install prompt
//     cannot be measured here: headless Chromium answers an empty Page.getInstallabilityErrors with a valid manifest and with none
//     (measured), so the check is on the cause, the manifest, which is what Chrome's install criteria start from.
// The host: `git ls-files --cached --others --exclude-standard docs`, copied to a temporary directory and served under /cancho-ui/ as
// GitHub Pages serves a project page, so a file that is git-ignored (the bug of #65: `*.js` ignored `docs/demo/<id>/app.js`) is
// absent here exactly as it is on the site. Outside the gate: needs node, git, the `playwright` package with a Chromium.
//   node scripts/check-mobile-demos.js              the check
//   node scripts/check-mobile-demos.js --self-test  the same checks against deliberately broken copies: each must fail
//   node scripts/check-mobile-demos.js --tree <dir> the check on a docs/ directory you built yourself
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const args = process.argv.slice(2);
const SELF_TEST = args.includes('--self-test');
const TREE = args.includes('--tree') ? path.resolve(args[args.indexOf('--tree') + 1]) : null;
const PREFIX = '/cancho-ui';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json', '.json': 'application/json' };

// docs/ as a commit would hold it.
function exportDocs() {
  const out = fs.mkdtempSync(path.join(os.tmpdir(), 'pages-'));
  const list = spawnSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z', 'docs'], { cwd: ROOT, encoding: 'utf8' });
  if (list.status !== 0) { throw new Error(list.stderr); }
  for (const rel of list.stdout.split('\0').filter(Boolean)) {
    const from = path.join(ROOT, rel);
    if (!fs.existsSync(from) || fs.statSync(from).isDirectory()) { continue; } // staged for deletion, or a submodule
    const to = path.join(out, rel.replace(/^docs\//, ''));
    fs.mkdirSync(path.dirname(to), { recursive: true });
    fs.copyFileSync(from, to);
  }
  return out;
}

function serve(site) {
  const server = http.createServer((req, res) => {
    let u = decodeURIComponent(req.url.split('?')[0]);
    if (!u.startsWith(PREFIX)) { res.writeHead(404); res.end(); return; }
    u = u.slice(PREFIX.length) || '/';
    if (u.endsWith('/')) { u += 'index.html'; }
    const f = path.join(site, path.normalize(u));
    if (!f.startsWith(site) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404, { 'content-type': 'text/plain' }); res.end('Not Found'); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

// Every problem of one run, as strings.
async function run(site, manifest) {
  const problems = [];
  const server = await serve(site);
  const base = `http://localhost:${server.address().port}${PREFIX}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  try {
    const ctx = await browser.newContext({ ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    const seen = [];
    page.on('console', (m) => { if (m.type() === 'error') { seen.push(`console: ${m.text().slice(0, 100)}`); } });
    page.on('pageerror', (e) => seen.push(`page error: ${String(e).slice(0, 100)}`));
    page.on('response', (r) => { if (r.status() >= 400) { seen.push(`HTTP ${r.status()} ${r.url().replace(base, '')}`); } });
    page.on('requestfailed', (r) => seen.push(`request failed ${r.url().replace(base, '')}`));
    await page.goto(`${base}/`);
    const vp = page.viewportSize();
    if (vp.width !== 412) { problems.push(`the profile is ${vp.width} px wide, not 412`); }
    const frames = await page.$$('#gallery iframe');
    if (frames.length !== manifest.supported.length) { problems.push(`${frames.length} iframes, ${manifest.supported.length} supported tiles`); }
    for (const [i, t] of manifest.supported.entries()) {
      const el = frames[i];
      if (!el) { continue; }
      if ((await el.getAttribute('loading')) !== 'lazy') { problems.push(`${t.id}: the iframe is not lazy (the landing page would load every demo at once)`); }
      await el.scrollIntoViewIfNeeded();
      const fr = await el.contentFrame();
      const ok = fr && await fr.waitForFunction(() => document.getElementById('root') && document.getElementById('root').innerText.trim().length > 0, null, { timeout: 5000 }).then(() => true).catch(() => false);
      if (!ok) { problems.push(`${t.id}: the embedded demo rendered nothing`); }
    }
    await page.waitForTimeout(300);
    if (seen.length) { problems.push(`landing page: ${[...new Set(seen)].slice(0, 4).join(' | ')}`); }
    const regs = await page.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).map((r) => r.scope));
    if (regs.length) { problems.push(`viewing the landing page registered service workers: ${regs.join(', ')}`); }
    for (const link of await page.$$eval('#gallery .glinks a[href^="demo/"]', (as) => as.map((a) => a.getAttribute('href')))) {
      if (!manifest.supported.some((t) => link === `demo/${t.id}/`)) { problems.push(`"Open on its own" links ${link}, which is no demo`); }
    }
    // each demo on its own
    for (const t of manifest.supported) {
      const p = await ctx.newPage();
      const own = [];
      p.on('console', (m) => { if (m.type() === 'error') { own.push(`console: ${m.text().slice(0, 100)}`); } });
      p.on('pageerror', (e) => own.push(`page error: ${String(e).slice(0, 100)}`));
      p.on('response', (r) => { if (r.status() >= 400) { own.push(`HTTP ${r.status()} ${r.url().replace(base, '')}`); } });
      await p.goto(`${base}/demo/${t.id}/`);
      const rendered = await p.waitForFunction(() => document.getElementById('root') && document.getElementById('root').innerText.trim().length > 0, null, { timeout: 5000 }).then(() => true).catch(() => false);
      if (!rendered) { problems.push(`${t.id}: opened on its own, it rendered nothing`); }
      if (own.length) { problems.push(`${t.id} on its own: ${[...new Set(own)].slice(0, 3).join(' | ')}`); }
      const hasManifest = await p.evaluate(() => !!document.querySelector('link[rel=manifest]'));
      if (hasManifest) { problems.push(`${t.id}: the page links a manifest, so a browser offers to install it`); }
      const cdp = await ctx.newCDPSession(p);
      await cdp.send('Page.enable');
      const manifestSeen = await cdp.send('Page.getAppManifest');
      if (manifestSeen.url) { problems.push(`${t.id}: the browser found the manifest ${manifestSeen.url.replace(base, '')}`); }
      const regs2 = await p.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).length);
      if (regs2) { problems.push(`${t.id}: opened on its own, it registered a service worker`); }
      await p.close();
    }
  } finally {
    await browser.close();
    server.close();
  }
  return problems;
}

function copyTree(from) {
  const to = fs.mkdtempSync(path.join(os.tmpdir(), 'pages-broken-'));
  fs.cpSync(from, to, { recursive: true });
  return to;
}

(async () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'gallery.json'), 'utf8'));
  const site = TREE || exportDocs();
  if (!SELF_TEST) {
    const problems = await run(site, manifest);
    for (const p of problems) { console.log(`FAIL ${p}`); }
    console.log(problems.length ? `\n${problems.length} problem(s)` : `ok   ${manifest.supported.length} demos, embedded and on their own, in a 412 px mobile profile, installing nothing`);
    process.exit(problems.length ? 1 : 0);
  }
  const clean = await run(site, manifest);
  if (clean.length) { console.log('FAIL the unbroken tree is not clean:', clean.join(' | ')); process.exit(1); }
  console.log('ok   the unbroken tree is clean');
  const breaks = [
    ['a demo\'s script is not published (the bug of #65: *.js is git-ignored)', (d) => fs.rmSync(path.join(d, 'demo/clock/app.js'))],
    ['a demo links a manifest again', (d) => { const f = path.join(d, 'demo/clock/index.html'); fs.writeFileSync(f, fs.readFileSync(f, 'utf8').replace('</head>', '<link rel="manifest" href="./manifest.webmanifest">\n</head>')); fs.writeFileSync(path.join(d, 'demo/clock/manifest.webmanifest'), JSON.stringify({ name: 'x', start_url: './', display: 'standalone', icons: [{ src: 'i.png', sizes: '192x192', type: 'image/png' }] })); }],
    ['a demo registers a service worker', (d) => { const f = path.join(d, 'demo/search/index.html'); fs.writeFileSync(f, fs.readFileSync(f, 'utf8').replace('</body>', '<script>navigator.serviceWorker.register("./sw.js")</script></body>')); fs.writeFileSync(path.join(d, 'demo/search/sw.js'), '// a worker\n'); }],
    ['an iframe is not lazy', (d) => { const f = path.join(d, 'index.html'); fs.writeFileSync(f, fs.readFileSync(f, 'utf8').replace(/ loading="lazy"/g, '')); }],
    ['a demo throws when it loads', (d) => { const f = path.join(d, 'demo/match/app.js'); fs.writeFileSync(f, fs.readFileSync(f, 'utf8') + '\nthrow new Error("broken demo");\n'); }],
    ['a demo is blank', (d) => { const f = path.join(d, 'demo/notes/app.js'); fs.writeFileSync(f, '// nothing\n'); }],
  ];
  let missed = 0;
  for (const [what, apply] of breaks) {
    const copy = copyTree(site);
    apply(copy);
    const problems = await run(copy, manifest);
    console.log(`${problems.length ? 'ok  ' : 'MISS'} break (${what}) ${problems.length ? 'is caught: ' + problems[0] : 'is not caught'}`);
    if (!problems.length) { missed++; }
  }
  console.log(missed ? `\n${missed} break(s) not caught` : '\nevery break was caught');
  process.exit(missed ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
