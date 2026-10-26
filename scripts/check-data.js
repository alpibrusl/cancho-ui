#!/usr/bin/env node
// `data` in Chromium (docs/data.md): DESKTOP CHROMIUM with the Pixel 7 profile, not a device. Watchdog: 5 minutes.
//   A. examples/ecosystem, built from the cancho-www page's own data file, read with JAVASCRIPT OFF and compared with what this check reads from the same
//      file itself (JSON.parse, no code of the compiler): every project, every link, every chip, every claim and its evidence, the count-ups, the waffles;
//   B. the page asks for nothing: the page and its script, no request for the data file or another host, no console error, layout shift 0;
//   C. a hostile file (markup, a script tag, quotes, a `javascript:` address) is text on the page or a refusal at build, and nothing runs;
//   D. the nodes the page was written with are the same nodes after app.js ran.
//   node scripts/check-data.js
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
const EXAMPLE = path.join(ROOT, 'examples', 'ecosystem');
const failures = [];
const watchdog = setTimeout(() => { console.log('FAIL watchdog: the check did not finish in 5 minutes'); process.exit(1); }, 5 * 60 * 1000);
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'data-'));
function build(name, file) {
  const out = path.join(work, name);
  const r = spawnSync(BIN, ['build', file, '--page', '-o', out], { encoding: 'utf8', cwd: ROOT });
  return { out, code: r.status, err: r.stderr + r.stdout };
}
function serve(dir, seen) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      seen.push(q.url);
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => { if (e) { r.statusCode = 404; r.end(); return; } r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json' }[path.extname(f)] || 'application/octet-stream'); r.end(d); });
    }).listen(0, () => ok(s));
  });
}
const PIXEL = devices['Pixel 7'];
const norm = (s) => s.replace(/\s+/g, ' ').trim();

(async () => {
  const data = JSON.parse(fs.readFileSync(path.join(EXAMPLE, 'data', 'ecosystem.json'), 'utf8'));
  const site = build('site', path.join(EXAMPLE, 'ecosystem.cui'));
  if (site.code !== 0) { console.log(`FAIL build: ${site.err}`); process.exit(1); }
  const seen = [];
  const server = await serve(site.out, seen);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  const external = [];
  async function open(base, opts) {
    const ctx = await browser.newContext(opts);
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); } });
    await page.route('**/*', (r) => { if (r.request().url().startsWith('http://127.0.0.1')) { r.continue(); } else { external.push(r.request().url()); r.abort(); } });
    await page.addInitScript(() => { window.__cls = 0; try { new PerformanceObserver((l) => { for (const e of l.getEntries()) { if (!e.hadRecentInput) { window.__cls += e.value; } } }).observe({ type: 'layout-shift', buffered: true }); } catch (e) { /* none */ } });
    await page.goto(base);
    await sleep(500);
    return { ctx, page, errors };
  }

  // ---- A. what the data says, read with JavaScript off ------------------------------------------------------------------------------------
  {
    const { ctx, page, errors } = await open(url, { ...PIXEL, javaScriptEnabled: false });
    const got = await page.evaluate(() => ({
      list: [...document.querySelectorAll('#ecosystem details > p')].map((p) => ({ text: p.textContent.replace(/\s+/g, ' ').trim(), hrefs: [...p.querySelectorAll('a')].map((a) => a.getAttribute('href')) })),
      paths: [...document.querySelectorAll('#paths article')].map((a) => ({ title: a.querySelector('h3').textContent, chips: [...a.querySelectorAll('.cui-chip')].map((c) => [c.textContent, c.className.split('cui-chip-')[1]]), hrefs: [...a.querySelectorAll('a')].map((x) => x.getAttribute('href')) })),
      claims: [...document.querySelectorAll('#verifies p')].map((p) => p.textContent.replace(/\s+/g, ' ').trim()),
      codes: [...document.querySelectorAll('#verifies p code')].map((c) => c.textContent),
      evidence: [...document.querySelectorAll('#verifies a')].map((a) => [a.textContent, a.getAttribute('href')]),
      counts: [...document.querySelectorAll('.cui-countup')].map((e) => e.textContent),
      waffles: [...document.querySelectorAll('figure.cui-waffle')].map((f) => ({ aria: f.querySelector('svg').getAttribute('aria-label'), counted: ((f.querySelector('.cui-wf-on') || { getAttribute: () => '' }).getAttribute('d').match(/M/g) || []).length, rest: [...f.querySelectorAll('.cui-wf-known, .cui-wf-open')].reduce((n, p) => n + (p.getAttribute('d').match(/M/g) || []).length, 0), cls: [...f.querySelectorAll('.cui-wf-known, .cui-wf-open')].map((p) => p.getAttribute('class')) })),
    }));
    // the oracle: this file, read here
    const want = data.projects.map((p) => ({ text: `${p.name}${p.page ? ' · page' : ''} (${p.stage}): ${p.summary}`, hrefs: p.page ? [p.url, p.page] : [p.url] }));
    const okList = got.list.length === want.length && want.every((w, i) => norm(got.list[i].text) === norm(w.text) && JSON.stringify(got.list[i].hrefs) === JSON.stringify(w.hrefs));
    check(`A the list of ${want.length} projects is the file's, in order: names, stages, summaries, addresses`, okList, okList ? '' : JSON.stringify([got.list.length, got.list[0]]));
    const byId = Object.fromEntries(data.projects.map((p) => [p.id, p]));
    const wantPaths = data.paths.map((p) => ({ title: p.title, chips: p.entries.map((e) => [byId[e.id].stage, { built: 'accent', working: 'good', alpha: 'warn', early: 'warn', design: 'quiet' }[byId[e.id].stage]]), hrefs: p.entries.flatMap((e) => [byId[e.id].url, ...(byId[e.id].page ? [byId[e.id].page] : []), e.next_url]) }));
    check('A the three paths: titles, the stage and tone of each chip, every address in order (the join of an entry with its project)', JSON.stringify(got.paths) === JSON.stringify(wantPaths), JSON.stringify(got.paths.map((p) => p.hrefs.length)));
    const claims = [...data.verifies.compiler, ...data.verifies.tests, ...data.verifies.open];
    // a pair of backticks in a claim is a <code> (docs/www-port/gaps.md section 4): the text of the page is the claim without the backticks
    const plain = (t) => t.replace(/`/g, '');
    check(`A the ${claims.length} claims are the file's, in order`, got.claims.length === claims.length && claims.every((c, i) => norm(got.claims[i]) === norm(plain(c.claim))), String(got.claims.length));
    const wantCodes = claims.flatMap((c) => [...c.claim.matchAll(/`([^`]+)`/g)].map((m) => m[1]));
    check(`A the ${wantCodes.length} backtick pairs of the claims are <code> elements with their text, in order, and no backtick is left`, wantCodes.length === 5 && JSON.stringify(got.codes) === JSON.stringify(wantCodes) && got.claims.every((t) => !t.includes('`')), JSON.stringify(got.codes));
    const ev = claims.flatMap((c) => c.evidence);
    check(`A the ${ev.length} evidence links are the file's: label and address`, JSON.stringify(got.evidence) === JSON.stringify(ev), String(got.evidence.length));
    const f = Object.fromEntries(data.measured.figures.map((x) => [x.id, x]));
    check('A the three count-ups are the final numbers of the file, in the HTML', JSON.stringify(got.counts) === JSON.stringify([String(f.conformance.passed), String(f.refusals.passed), String(f.rules.value)]), JSON.stringify(got.counts));
    check('A the three waffles: counted and not counted squares are the file\'s (697 of 704 known, 256 of 333 open, 62 of 62)', got.waffles.length === 3 && got.waffles[0].counted === f.conformance.passed && got.waffles[0].rest === f.conformance.of - f.conformance.passed && got.waffles[0].cls[0] === 'cui-wf-known' && got.waffles[1].counted === f.refusals.passed && got.waffles[1].rest === f.refusals.of - f.refusals.passed && got.waffles[1].cls[0] === 'cui-wf-open' && got.waffles[2].counted === f.rules.value && got.waffles[2].rest === 0, JSON.stringify(got.waffles.map((w) => [w.counted, w.rest])));
    check('A no console error', errors.length === 0, errors.join(' | '));
    await ctx.close();
  }

  // ---- B. the page asks for nothing ---------------------------------------------------------------------------------------------------------
  {
    seen.length = 0;
    const { ctx, page, errors } = await open(url, { ...PIXEL });
    await page.evaluate(async () => { for (let y = 0; y < document.documentElement.scrollHeight; y += 300) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 15)); } });
    const cls = await page.evaluate(() => window.__cls);
    check('B the page asks for the page and its script and nothing else (no data file)', JSON.stringify(seen.filter((u) => !u.includes('favicon'))) === JSON.stringify(['/', '/app.js']), JSON.stringify(seen));
    check('B layout shift 0 from load to the end, no console error', cls === 0 && errors.length === 0, `${cls} ${errors.join('|')}`);
    check('B the file is not in the build', !fs.existsSync(path.join(site.out, 'data')) && !fs.readdirSync(site.out).some((n) => n.endsWith('.json')), fs.readdirSync(site.out).join(','));
    await ctx.close();
  }

  // ---- C. a hostile file ---------------------------------------------------------------------------------------------------------------------------
  {
    const dir = path.join(work, 'hostile');
    fs.mkdirSync(dir);
    const evil = [{ name: '<img src=x onerror="window.__pwn=1"><script>window.__pwn=2</script>', url: 'https://example.org/?a=1&b="2"', stage: 'built' }, { name: '</title></head><body onload="window.__pwn=3">', url: 'https://example.org/"onmouseover="window.__pwn=4', stage: 'x' }];
    const text = JSON.stringify({ items: evil });
    fs.writeFileSync(path.join(dir, 'h.json'), text);
    const h = crypto.createHash('sha256').update(text).digest('hex');
    fs.writeFileSync(path.join(dir, 'p.cui'), `data H { file: "h.json" sha256: "${h}"; }\ncomponent C {\n view { Column { For { over: data.items(H, "items") as: p Rich { Link { to: json.get(p, "url", "") "{json.get(p, \\"name\\", \\"\\")}" } } } } }\n}\n`);
    const b = build('hostile', path.join(dir, 'p.cui'));
    check('C a hostile file builds: markup in names, quotes in an https address', b.code === 0, b.err.slice(0, 200));
    if (b.code === 0) {
      const s = await serve(b.out, []);
      const { ctx, page } = await open(`http://127.0.0.1:${s.address().port}/`, { ...PIXEL });
      const r = await page.evaluate(() => ({ pwn: window.__pwn || 0, imgs: document.querySelectorAll('img').length, scripts: [...document.querySelectorAll('script')].filter((x) => /__pwn/.test(x.textContent)).length, texts: [...document.querySelectorAll('a')].map((a) => a.textContent), hrefs: [...document.querySelectorAll('a')].map((a) => a.getAttribute('href')), attrs: [...document.querySelectorAll('a')].map((a) => a.getAttributeNames().join()) }));
      check('C the names are text (no element, no script, nothing ran) and the addresses stay one attribute', r.pwn === 0 && r.imgs === 0 && r.scripts === 0 && r.texts[0] === evil[0].name && r.texts[1] === evil[1].name && r.hrefs[0] === evil[0].url && r.hrefs[1] === evil[1].url && r.attrs.every((a) => a === 'href'), JSON.stringify(r).slice(0, 300));
      await ctx.close();
      s.close();
    }
    const js = JSON.stringify({ items: [{ name: 'x', url: 'javascript:window.__pwn=5' }] });
    fs.writeFileSync(path.join(dir, 'j.json'), js);
    fs.writeFileSync(path.join(dir, 'j.cui'), `data H { file: "j.json" sha256: "${crypto.createHash('sha256').update(js).digest('hex')}"; }\ncomponent C {\n view { Column { For { over: data.items(H, "items") as: p Link { to: json.get(p, "url", "") "x" } } } }\n}\n`);
    const j = build('js', path.join(dir, 'j.cui'));
    check('C a javascript: address in the data is a refusal at build (link-scheme-refused), not a link', j.code === 1 && /link-scheme-refused/.test(j.err), j.err.slice(0, 160));
  }

  // ---- D. the nodes are not touched ------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page } = await open(url, { ...PIXEL });
    const same = await page.evaluate(() => 'ok');
    const ctx2 = await browser.newContext(PIXEL);
    const p2 = await ctx2.newPage();
    await p2.route('**/app.js', async (r) => { await new Promise((ok) => setTimeout(ok, 1200)); r.continue(); });
    await p2.goto(url, { waitUntil: 'domcontentloaded' });
    await p2.evaluate(() => { window.__nodes = [...document.querySelectorAll('#paths article, #ecosystem details p, figure.cui-waffle, .cui-chip')]; });
    await sleep(2200);
    const r = await p2.evaluate(() => ({ n: window.__nodes.length, same: window.__nodes.every((x) => x.isConnected) }));
    check('D the elements the data wrote are the same nodes after app.js ran', same === 'ok' && r.n === 39 && r.same, JSON.stringify(r));
    await ctx.close();
    await ctx2.close();
  }
  check('no request went to another host', external.length === 0, external.join(', '));
  await browser.close();
  server.close();
  clearTimeout(watchdog);
  fs.rmSync(work, { recursive: true, force: true });
  if (failures.length) { console.log(`FAIL ${failures.length}: ${failures.join('; ')}`); process.exit(1); }
  console.log('PASS');
})();
