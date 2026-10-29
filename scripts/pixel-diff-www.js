#!/usr/bin/env node
// Pixel diff of the port against the real cancho-www page (B3). Chromium (Playwright), the two sites served locally, reduced motion, light and dark, at the widths
// 360, 412, 768, 1024, 1280 and 1536. For each width and scheme the page is scrolled through (lazy pictures, reveals), a full-page screenshot is taken of each
// side and cut into the page's fifteen sections (the ids the parity script uses). The comparison is done inside Chromium (a canvas), so nothing but Node and
// Playwright is needed: a section is compared at its top-left corner, the part of the taller one that the other does not cover counts as different, and a pixel
// differs when a channel moves by more than 24 of 255 (so that anti-aliasing is not a difference). Because the layouts are not the same (dropped.md 10: the port
// has the compiler's baseline look with the page's palette and fonts), most of every section differs; the number is a measure of *how far apart*, and the
// classification (docs/www-port/pixel-rules.json) is what says why. Written: docs/www-port/pixel.md and one contact sheet per width and scheme in
// docs/www-port/pixel/ (page | port | difference, one row per section, scaled; WebP).
//   node scripts/pixel-diff-www.js <path to a cancho-www clone> [--write] [--widths=360,412] [--schemes=light]
// The compiler is target/debug/cancho-ui of this checkout unless CANCHO_UI_BIN names another binary.
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const REF = path.resolve(process.argv[2] || '');
const WRITE = process.argv.includes('--write');
const arg = (name, dflt) => { const a = process.argv.find((x) => x.startsWith(`--${name}=`)); return a ? a.slice(name.length + 3).split(',') : dflt; };
const WIDTHS = arg('widths', ['360', '412', '768', '1024', '1280', '1536']).map(Number);
const SCHEMES = arg('schemes', ['light', 'dark']);
const CROPS = (arg('crops', [null])[0]) || null; // a directory: also write the raw crops there (for looking at, not committed)
if (!process.argv[2] || !fs.existsSync(path.join(REF, 'index.html'))) { console.error('usage: node scripts/pixel-diff-www.js <cancho-www clone> [--write]'); process.exit(2); }
const SECTIONS = ['header', 'top', 'try', 'trust', 'verifies', 'authority', 'ecosystem', 'proof', 'demo', 'not-here', 'agents', 'next', 'status', 'name', 'footer'];
const RULES = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'www-port', 'pixel-rules.json'), 'utf8'));
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.md': 'text/markdown', '.json': 'application/json', '.png': 'image/png', '.webp': 'image/webp', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.woff2': 'font/woff2', '.mp4': 'video/mp4', '.webm': 'video/webm', '.txt': 'text/plain', '.cast': 'text/plain', '.webmanifest': 'application/manifest+json' };

function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => {
        if (e) { r.statusCode = 404; r.end(); return; }
        r.setHeader('content-type', TYPES[path.extname(f)] || 'application/octet-stream');
        r.end(d);
      });
    }).listen(0, '127.0.0.1', () => ok(s));
  });
}

// runs in the browser: compares two PNG data URLs, returns {w, h, differ, total, overlap} and draws the contact-sheet row
const COMPARE = async ({ a, b }) => {
  const load = (u) => new Promise((ok, no) => { const i = new Image(); i.onload = () => ok(i); i.onerror = no; i.src = u; });
  const [A, B] = await Promise.all([load(a), load(b)]);
  const W = Math.max(A.width, B.width), H = Math.max(A.height, B.height);
  const px = (img) => { const c = document.createElement('canvas'); c.width = W; c.height = H; const x = c.getContext('2d', { willReadFrequently: true }); x.drawImage(img, 0, 0); return x.getImageData(0, 0, W, H).data; };
  const da = px(A), db = px(B);
  const covered = (img, x, y) => x < img.width && y < img.height;
  let differ = 0, overlapDiffer = 0, overlap = 0;
  const out = document.createElement('canvas'); out.width = W; out.height = H;
  const ox = out.getContext('2d'); const od = ox.createImageData(W, H);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 4;
      const inA = covered(A, x, y), inB = covered(B, x, y);
      let d;
      if (inA && inB) { overlap++; d = Math.abs(da[i] - db[i]) > 24 || Math.abs(da[i + 1] - db[i + 1]) > 24 || Math.abs(da[i + 2] - db[i + 2]) > 24; if (d) { overlapDiffer++; } } else { d = true; }
      if (d) { differ++; od.data[i] = 220; od.data[i + 1] = 40; od.data[i + 2] = 40; od.data[i + 3] = inA && inB ? 255 : 120; } else { od.data[i] = 245; od.data[i + 1] = 245; od.data[i + 2] = 245; od.data[i + 3] = 255; }
    }
  }
  ox.putImageData(od, 0, 0);
  return { W, H, aw: A.width, ah: A.height, bw: B.width, bh: B.height, differ, overlapDiffer, overlap, diffUrl: out.toDataURL('image/png') };
};

// runs in the browser: the contact sheet (page | port | difference per section), scaled
const SHEET = async ({ rows, scale, label }) => {
  const load = (u) => new Promise((ok, no) => { const i = new Image(); i.onload = () => ok(i); i.onerror = no; i.src = u; });
  const imgs = [];
  for (const r of rows) { imgs.push([await load(r.a), await load(r.b), await load(r.d)]); }
  const colW = Math.max(...rows.map((r) => r.W)) * scale, gap = 8, head = 18;
  const total = rows.reduce((s, r) => s + r.H * scale + head + gap, 0) + 24;
  const c = document.createElement('canvas'); c.width = Math.ceil(colW * 3 + gap * 2); c.height = Math.ceil(total);
  const x = c.getContext('2d'); x.fillStyle = '#fff'; x.fillRect(0, 0, c.width, c.height); x.fillStyle = '#000'; x.font = '12px sans-serif';
  x.fillText(label + '   (page | port | difference; red = differs, translucent red = outside the other side)', 4, 14);
  let y = 24;
  rows.forEach((r, i) => {
    x.fillStyle = '#000'; x.fillText(`${r.id}: page ${r.aw}x${r.ah}, port ${r.bw}x${r.bh}, ${(100 * r.differ / (r.W * r.H)).toFixed(0)}% differs`, 4, y + 12);
    y += head;
    [0, 1, 2].forEach((k) => { x.drawImage(imgs[i][k], k * (colW + gap), y, imgs[i][k].width * scale, imgs[i][k].height * scale); x.strokeStyle = '#999'; x.strokeRect(k * (colW + gap), y, imgs[i][k].width * scale, imgs[i][k].height * scale); });
    y += r.H * scale + gap;
  });
  return c.toDataURL('image/webp', 0.25);
};

(async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'pixel-'));
  const out = path.join(work, 'port');
  const b = spawnSync(process.env.CANCHO_UI_BIN || path.join(ROOT, 'target', 'debug', 'cancho-ui'), ['build', 'docs/www-port/site/home.cui', '--page', '-o', out], { cwd: ROOT, encoding: 'utf8' });
  if (b.status !== 0) { console.error(b.stderr + b.stdout); process.exit(1); }
  const refSha = spawnSync('git', ['-C', REF, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
  const servers = { page: await serve(REF), port: await serve(out) };
  const browser = await chromium.launch();
  const tool = await (await browser.newContext()).newPage();
  await tool.goto('about:blank');
  const results = [];
  const sheets = [];
  for (const scheme of SCHEMES) {
    for (const width of WIDTHS) {
      const shots = {};
      for (const side of ['page', 'port']) {
        const ctx = await browser.newContext({ viewport: { width, height: 900 }, colorScheme: scheme, reducedMotion: 'reduce', serviceWorkers: 'block', deviceScaleFactor: 1 });
        const page = await ctx.newPage();
        await page.goto(`http://127.0.0.1:${servers[side].address().port}/`, { waitUntil: 'load' });
        await page.evaluate(async () => { for (let y = 0; y < document.body.scrollHeight; y += 500) { window.scrollTo(0, y); await new Promise((r) => setTimeout(r, 40)); } window.scrollTo(0, 0); });
        await page.waitForTimeout(1500);
        const boxes = await page.evaluate((ids) => Object.fromEntries(ids.map((id) => {
          const e = id === 'header' ? document.querySelector('header') : id === 'footer' ? document.querySelector('footer') : document.getElementById(id);
          if (!e) { return [id, null]; }
          const r = e.getBoundingClientRect();
          return [id, { x: 0, y: Math.round(r.top + scrollY), w: document.documentElement.clientWidth, h: Math.round(r.height) }];
        })), SECTIONS);
        const full = await page.evaluate(() => ({ h: document.documentElement.scrollHeight, w: document.documentElement.scrollWidth, cw: document.documentElement.clientWidth }));
        const png = await page.screenshot({ fullPage: true, type: 'png' });
        const crops = {};
        for (const id of SECTIONS) {
          const bx = boxes[id];
          if (!bx || bx.h < 1) { crops[id] = null; continue; }
          await page.evaluate((y) => window.scrollTo(0, y), Math.max(0, bx.y - 40));
          await page.waitForTimeout(350); // a frame of another origin paints only while it is near the screen
          const buf = await page.screenshot({ clip: { x: 0, y: bx.y, width: bx.w, height: Math.min(bx.h, 9000) }, fullPage: true, type: 'png' });
          crops[id] = 'data:image/png;base64,' + buf.toString('base64');
        }
        shots[side] = { crops, full, bytes: png.length, hscroll: full.w > full.cw };
        await ctx.close();
      }
      const rows = [];
      const sections = [];
      for (const id of SECTIONS) {
        const a = shots.page.crops[id], bb = shots.port.crops[id];
        if (!a || !bb) { sections.push({ id, missing: !a ? 'page' : 'port' }); continue; }
        if (CROPS) { fs.mkdirSync(CROPS, { recursive: true }); fs.writeFileSync(path.join(CROPS, `${scheme}-${width}-${id}-page.png`), Buffer.from(a.split(',')[1], 'base64')); fs.writeFileSync(path.join(CROPS, `${scheme}-${width}-${id}-port.png`), Buffer.from(bb.split(',')[1], 'base64')); }
        const r = await tool.evaluate(COMPARE, { a, b: bb });
        rows.push({ id, a, b: bb, d: r.diffUrl, ...r, diffUrl: undefined });
        sections.push({ id, aw: r.aw, ah: r.ah, bw: r.bw, bh: r.bh, pct: +(100 * r.differ / (r.W * r.H)).toFixed(1), overlapPct: +(100 * r.overlapDiffer / Math.max(1, r.overlap)).toFixed(1) });
      }
      results.push({ scheme, width, pageH: shots.page.full.h, portH: shots.port.full.h, pageHScroll: shots.page.hscroll, portHScroll: shots.port.hscroll, sections });
      const scale = Math.min(1, 230 / width, 9000 / rows.reduce((t, r) => t + r.H + 26 / 0.3, 0));
      const sheet = await tool.evaluate(SHEET, { rows, scale, label: `${scheme} ${width}px, cancho-www ${refSha} against the port` });
      sheets.push({ name: `${scheme}-${width}.webp`, data: Buffer.from(sheet.split(',')[1], 'base64') });
      console.log(`${scheme} ${width}: page ${shots.page.full.h}px, port ${shots.port.full.h}px tall; ${sections.map((s) => `${s.id} ${s.pct}%`).join(' ')}`);
    }
  }
  await browser.close();
  Object.values(servers).forEach((s) => s.close());

  // classification: the first rule of pixel-rules.json that matches the section (and width and scheme, when it says), else UNEXPLAINED
  const classify = (id, width, scheme) => RULES.rules.find((r) => (r.section === id || r.section === '*') && (!r.widths || r.widths.includes(width)) && (!r.scheme || r.scheme === scheme)) || null;
  let unexplained = 0;
  const md = [];
  const totalBytes = sheets.reduce((s, x) => s + x.data.length, 0);
  md.push('# Pixel diff of the port against the page, measured', '');
  md.push(`> Generated by \`node scripts/pixel-diff-www.js <clone> --write\` against cancho-www \`${refSha}\`. Chromium ${(await chromium.launch().then(async (x) => { const v = x.version(); await x.close(); return v; }))} (Playwright), reduced motion, device scale factor 1, the two sites served locally; widths ${WIDTHS.join(', ')}; ${SCHEMES.join(' and ')}. A section is cut from a full-page screenshot and compared **at its top-left corner**; "differs" counts pixels whose colour moves by more than 24 of 255 in a channel, and counts the part of the taller side that the other does not cover as different. **The layouts are not the same** (dropped.md 10), so a high number says how far apart the two are and not that something is wrong: the class and the reason of each section are in [\`pixel-rules.json\`](pixel-rules.json) and were written after looking at the contact sheets in [\`pixel/\`](pixel/) (${sheets.length} WebP files, ${(totalBytes / 1024).toFixed(0)} kB together; each is page | port | difference, one row per section, scaled to about 230 px a column). **A section no rule explains is \`UNEXPLAINED\` and fails the script.**`, '');
  for (const r of results) {
    md.push(`## ${r.scheme}, ${r.width} px`, '', `Full page: the page ${r.pageH} px tall, the port ${r.portH} px (${(r.portH / r.pageH).toFixed(2)} times). Horizontal scroll: page ${r.pageHScroll ? '**yes**' : 'no'}, port ${r.portHScroll ? '**yes**' : 'no'}. Contact sheet: [\`pixel/${r.scheme}-${r.width}.webp\`](pixel/${r.scheme}-${r.width}.webp).`, '');
    md.push('| section | page (w x h) | port (w x h) | differs | where both draw | class | why |', '|---|---|---|---|---|---|---|');
    for (const s of r.sections) {
      if (s.missing) { md.push(`| ${s.id} | | | | | **UNEXPLAINED** | missing on the ${s.missing} |`); unexplained++; continue; }
      const rule = classify(s.id, r.width, r.scheme);
      if (!rule) { unexplained++; }
      md.push(`| ${s.id} | ${s.aw} x ${s.ah} | ${s.bw} x ${s.bh} | ${s.pct}% | ${s.overlapPct}% | ${rule ? `**${rule.class}**` : '**UNEXPLAINED**'} | ${rule ? rule.why : ''} |`);
    }
    md.push('');
  }
  const stale = RULES.rules.filter((r) => !results.some((x) => x.sections.some((s) => (r.section === s.id || r.section === '*') && (!r.widths || r.widths.includes(x.width)) && (!r.scheme || r.scheme === x.scheme))));
  if (WRITE) {
    const dir = path.join(ROOT, 'docs', 'www-port', 'pixel');
    fs.mkdirSync(dir, { recursive: true });
    for (const s of sheets) { fs.writeFileSync(path.join(dir, s.name), s.data); }
    fs.writeFileSync(path.join(ROOT, 'docs', 'www-port', 'pixel.md'), md.join('\n') + '\n');
  }
  fs.writeFileSync(path.join(work, 'pixel.md'), md.join('\n'));
  console.log(`\n${sheets.length} contact sheets, ${(totalBytes / 1024).toFixed(0)} kB`);
  if (stale.length) { console.log(`${stale.length} rule(s) of pixel-rules.json match nothing`); }
  console.log(unexplained ? `${unexplained} unexplained` : 'every section is classified');
  process.exit(unexplained ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
