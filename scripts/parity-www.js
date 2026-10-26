#!/usr/bin/env node
// Parity of the port (docs/www-port/site/home.cui) against the real cancho-www page, in Chromium: DESKTOP CHROMIUM with the Pixel 7 profile and a
// desktop window, not a device. Not part of the gate (the reference clone is not in this repository): it needs the clone,
//   node scripts/parity-www.js <path to a cancho-www clone> [--write]       (--write rewrites docs/www-port/parity.md)
// It measures: the visible text of every section, every link, the outline of headings and landmarks, the anchors, the accessibility tree, and the
// page with JavaScript off. Every difference is looked up in docs/www-port/parity-rules.json (intentional, deferred to a later PR, or a defect
// that was found and is listed as open); a difference no rule explains is UNEXPLAINED and the script exits 1: nothing is "done" while one is.
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const REF = path.resolve(process.argv[2] || '');
const WRITE = process.argv.includes('--write');
if (!process.argv[2] || !fs.existsSync(path.join(REF, 'index.html'))) { console.error('usage: node scripts/parity-www.js <cancho-www clone> [--write]'); process.exit(2); }
const RULES = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'www-port', 'parity-rules.json'), 'utf8')).rules;
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'parity-'));
const out = path.join(work, 'port');
const b = spawnSync(path.join(ROOT, 'target', 'debug', 'cancho-ui'), ['build', 'docs/www-port/site/home.cui', '--page', '-o', out], { cwd: ROOT, encoding: 'utf8' });
if (b.status !== 0) { console.error(b.stderr + b.stdout); process.exit(1); }
const refSha = spawnSync('git', ['-C', REF, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).stdout.trim();

function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.md': 'text/markdown', '.json': 'application/json', '.png': 'image/png', '.webp': 'image/webp', '.jpg': 'image/jpeg', '.woff2': 'font/woff2' }[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, '127.0.0.1', () => ok(s));
  });
}
const SECTIONS = ['header', 'top', 'try', 'trust', 'verifies', 'authority', 'ecosystem', 'proof', 'demo', 'not-here', 'agents', 'next', 'status', 'name', 'footer'];
const norm = (t) => t.replace(/[ \s]+/g, ' ').trim().toLowerCase();

// what a page says, with the browser's own idea of visible text, split by block
const grab = () => {
  const sel = (id) => (id === 'header' ? document.querySelector('header') : id === 'footer' ? document.querySelector('footer') : document.getElementById(id));
  const lines = (el) => (el ? el.innerText.split('\n') : []);
  const ids = ['header', 'top', 'try', 'trust', 'verifies', 'authority', 'ecosystem', 'proof', 'demo', 'not-here', 'agents', 'next', 'status', 'name', 'footer'];
  const text = {};
  for (const id of ids) { text[id] = lines(sel(id)); }
  const links = [...document.querySelectorAll('a[href]')].map((a) => ({ href: a.getAttribute('href'), text: (a.innerText ?? a.textContent ?? '').replace(/\s+/g, ' ').trim() }));
  const headings = [...document.querySelectorAll('h1,h2,h3,h4')].map((h) => ({ level: Number(h.tagName[1]), text: (h.innerText ?? h.textContent ?? '').replace(/\s+/g, ' ').trim() }));
  const landmarks = [...document.querySelectorAll('header,nav,main,footer,aside,section[aria-label],section[aria-labelledby],[role=banner],[role=navigation],[role=main],[role=contentinfo],[role=complementary]')].map((e) => ({ tag: e.tagName.toLowerCase(), label: e.getAttribute('aria-label') || '' }));
  const idsAll = [...document.querySelectorAll('[id]')].map((e) => e.id);
  const anchors = links.filter((l) => l.href.startsWith('#')).map((l) => l.href.slice(1));
  return { text, links, headings, landmarks, ids: idsAll, anchors };
};

(async () => {
  const sRef = await serve(REF);
  const sPort = await serve(out);
  const browser = await chromium.launch();
  const open = async (server, opts) => {
    const ctx = await browser.newContext(opts);
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/`, { waitUntil: 'load' });
    await page.waitForTimeout(1800);
    return { ctx, page, errors };
  };
  const results = {};
  const profiles = { 'Pixel 7': { ...devices['Pixel 7'], reducedMotion: 'reduce' }, desktop: { viewport: { width: 1280, height: 900 }, reducedMotion: 'reduce' } };
  for (const [name, opts] of Object.entries(profiles)) {
    const a = await open(sRef, opts);
    const p = await open(sPort, opts);
    const ref = await a.page.evaluate(grab);
    const port = await p.page.evaluate(grab);
    const cdp = await Promise.all([a.ctx.newCDPSession(a.page), p.ctx.newCDPSession(p.page)]);
    const ax = await Promise.all(cdp.map(async (c) => (await c.send('Accessibility.getFullAXTree')).nodes.filter((n) => !n.ignored)));
    const axSummary = (nodes) => {
      const count = {};
      for (const n of nodes) { const r = n.role && n.role.value; count[r] = (count[r] || 0) + 1; }
      return count;
    };
    results[name] = { ref, port, ax: [axSummary(ax[0]), axSummary(ax[1])], errors: [a.errors, p.errors] };
    await a.ctx.close();
    await p.ctx.close();
  }
  // JavaScript off
  const nojs = {};
  for (const [label, server] of [['ref', sRef], ['port', sPort]]) {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 }, javaScriptEnabled: false });
    const page = await ctx.newPage();
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    nojs[label] = await page.evaluate(() => ({ text: document.body.innerText.split('\n').map((l) => l.replace(/\s+/g, ' ').trim()).filter(Boolean), buttons: document.querySelectorAll('button').length, lis: document.querySelectorAll('li').length, svgs: document.querySelectorAll('svg').length, imgs: document.querySelectorAll('img').length, videos: document.querySelectorAll('video').length }));
    await ctx.close();
  }
  await browser.close();
  sRef.close();
  sPort.close();

  const classify = (kind, section, side, text) => {
    for (const r of RULES) {
      if (r.kind !== kind) { continue; }
      if (r.section && r.section !== section && r.section !== '*') { continue; }
      if (r.side && r.side !== side) { continue; }
      if (new RegExp(r.match, 'i').test(text)) { return r; }
    }
    return null;
  };
  const report = [];
  let unexplained = 0;
  const row = (kind, where, side, text) => {
    const r = classify(kind, where, side, text);
    if (!r) { unexplained++; }
    report.push({ kind, where, side, text, class: r ? r.class : 'UNEXPLAINED', why: r ? r.why : '', ref: r ? r.ref : '' });
  };
  // the runs of words that are in one text and not in the other (longest common subsequence of the words): what a reader would find different
const runs = (A, B) => {
  const a = A.join(' ').split(' ').filter(Boolean), b = B.join(' ').split(' ').filter(Boolean);
  const n = a.length, m = b.length;
  const L = Array.from({ length: n + 1 }, () => new Uint16Array(m + 1));
  for (let i = n - 1; i >= 0; i--) { for (let j = m - 1; j >= 0; j--) { L[i][j] = a[i] === b[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]); } }
  const out = [];
  let i = 0, j = 0, cur = null;
  const flush = () => { if (cur) { out.push(cur); cur = null; } };
  const add = (side, w) => { if (cur && cur.side === side) { cur.words.push(w); } else { flush(); cur = { side, words: [w] }; } };
  while (i < n && j < m) {
    if (a[i] === b[j]) { flush(); i++; j++; } else if (L[i + 1][j] >= L[i][j + 1]) { add('page', a[i++]); } else { add('port', b[j++]); }
  }
  while (i < n) { add('page', a[i++]); }
  while (j < m) { add('port', b[j++]); }
  flush();
  return out.map((r) => ({ side: r.side, text: r.words.join(' ') }));
};
const only = (a, b) => { const m = new Map(); for (const x of b) { m.set(x, (m.get(x) || 0) + 1); } const o = []; for (const x of a) { if (m.get(x)) { m.set(x, m.get(x) - 1); } else { o.push(x); } } return o; };
  const main = results.desktop;
  for (const id of SECTIONS) {
    const A = main.ref.text[id].map(norm).filter(Boolean);
    const B = main.port.text[id].map(norm).filter(Boolean);
    for (const r of runs(A, B)) { row('text', id, r.side, r.text); }
  }
  const linkKey = (l) => l.href;
  const setOf = (l) => new Set(l.map(linkKey));
  const [sr, sp] = [setOf(main.ref.links), setOf(main.port.links)];
  for (const t of sr) { if (!sp.has(t)) { row('link', '*', 'page', t); } }
  for (const t of sp) { if (!sr.has(t)) { row('link', '*', 'port', t); } }
  row('linkcount', '*', 'both', `links: page ${main.ref.links.length} (${sr.size} distinct), port ${main.port.links.length} (${sp.size} distinct)`);
  const hk = (h) => `h${h.level} ${norm(h.text)}`;
  for (const t of only(main.ref.headings.map(hk), main.port.headings.map(hk))) { row('heading', '*', 'page', t); }
  for (const t of only(main.port.headings.map(hk), main.ref.headings.map(hk))) { row('heading', '*', 'port', t); }
  const lm = (l) => `${l.tag}${l.label ? ` "${l.label}"` : ''}`;
  for (const t of only(main.ref.landmarks.map(lm), main.port.landmarks.map(lm))) { row('landmark', '*', 'page', t); }
  for (const t of only(main.port.landmarks.map(lm), main.ref.landmarks.map(lm))) { row('landmark', '*', 'port', t); }
  for (const t of only(main.ref.ids, main.port.ids)) { row('id', '*', 'page', t); }
  for (const t of only(main.port.ids.filter((i) => !i.startsWith('cui-')), main.ref.ids)) { row('id', '*', 'port', t); }
  // anchors: every #id a page links to exists in that page
  for (const [side, d] of [['page', main.ref], ['port', main.port]]) {
    for (const a of new Set(d.anchors)) { if (a && !d.ids.includes(a)) { row('anchor', '*', side, `#${a} is linked and there is no such id`); } }
  }
  for (const a of new Set(main.ref.anchors)) { if (a && !main.port.anchors.includes(a) && !main.port.ids.includes(a)) { row('anchor', '*', 'port', `#${a} is linked by the page and not by the port`); } }
  // accessibility tree: roles that differ by count
  // the roles a person using a screen reader meets; the browser's own inline boxes and generic wrappers are not a difference
  const MEANT = new Set(['banner', 'navigation', 'main', 'contentinfo', 'complementary', 'region', 'heading', 'link', 'button', 'list', 'listitem', 'image', 'figure', 'table', 'article', 'group', 'status', 'blockquote', 'term', 'definition', 'DescriptionList', 'Figcaption', 'paragraph', 'log', 'cell', 'columnheader', 'rowheader', 'row', 'code']);
  const roles = new Set([...Object.keys(main.ax[0]), ...Object.keys(main.ax[1])].filter((r) => MEANT.has(r)));
  const axRows = [];
  for (const r of [...roles].sort()) { const x = main.ax[0][r] || 0, y = main.ax[1][r] || 0; if (x !== y) { axRows.push([r, x, y]); row('ax', '*', 'both', `${r}: page ${x}, port ${y}`); } }
  // JavaScript off
  const lineSet = (l) => l.map(norm).filter(Boolean);
  for (const r of runs(lineSet(nojs.ref.text), lineSet(nojs.port.text))) { row('nojs', '*', r.side, r.text); }
  // the phone profile: the same text as the desktop one, on each side (nothing is dropped by width)
  const phone = results['Pixel 7'];
  for (const [side, d, dd] of [['page', phone.ref, main.ref], ['port', phone.port, main.port]]) {
    for (const id of SECTIONS) {
      const A = d.text[id].map(norm).filter(Boolean), B = dd.text[id].map(norm).filter(Boolean);
      for (const r of runs(A, B)) { row('width', id, side, `${r.side === 'page' ? 'phone only' : 'desktop only'}: ${r.text}`); }
    }
  }
  const errs = results.desktop.errors;

  const by = {};
  for (const r of report) { by[r.class] = (by[r.class] || 0) + 1; }
  const totals = ['text', 'link', 'linkcount', 'heading', 'landmark', 'id', 'anchor', 'ax', 'nojs', 'width'].map((k) => `${k} ${report.filter((r) => r.kind === k).length}`).join(', ');
  const md = [];
  md.push('# Parity of the port against the page, measured', '');
  md.push(`> Generated by \`node scripts/parity-www.js <clone> --write\` against cancho-www \`${refSha}\`. Chromium (Playwright), desktop 1280 x 900 and the Pixel 7 profile, reduced motion (so that nothing waits to be seen). Every difference is a row; its class comes from [\`parity-rules.json\`](parity-rules.json); **a row no rule explains is \`UNEXPLAINED\`** and fails the script.`, '');
  md.push(`**${report.length} differences** (${totals}). By class: ${Object.entries(by).map(([k, v]) => `${k} ${v}`).join(', ')}.`, '');
  md.push(`Console errors: page ${errs[0].length}, port ${errs[1].length}${errs[0].concat(errs[1]).length ? `: ${errs[0].concat(errs[1]).join(' | ')}` : ''}.`, '');
  md.push('| what | where | side | the difference | class | why |', '|---|---|---|---|---|---|');
  for (const r of report) { md.push(`| ${r.kind} | ${r.where} | ${r.side} | ${r.text.replace(/\|/g, '\\|').slice(0, 150)} | **${r.class}** | ${r.why}${r.ref ? ` (${r.ref})` : ''} |`); }
  md.push('', `JavaScript off: the page has ${nojs.ref.lis} \`li\`, ${nojs.ref.svgs} \`svg\`, ${nojs.ref.imgs} \`img\`, ${nojs.ref.buttons} \`button\`; the port has ${nojs.port.lis}, ${nojs.port.svgs}, ${nojs.port.imgs}, ${nojs.port.buttons}.`, '');
  fs.writeFileSync(path.join(work, 'parity.md'), md.join('\n') + '\n');
  if (WRITE) { fs.writeFileSync(path.join(ROOT, 'docs', 'www-port', 'parity.md'), md.join('\n') + '\n'); }
  console.log(md.slice(0, 6).join('\n'));
  for (const r of report.filter((x) => x.class === 'UNEXPLAINED')) { console.log(`UNEXPLAINED ${r.kind} [${r.where}] ${r.side}: ${r.text.slice(0, 160)}`); }
  console.log(unexplained ? `\n${unexplained} unexplained` : '\nevery difference is classified');
  process.exit(unexplained ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
