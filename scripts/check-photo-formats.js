#!/usr/bin/env node
// `Photo` and the formats and proportions of the pictures it draws (docs/structure.md 12.0r), in Chromium. Every picture is drawn at run time on a canvas and encoded by the browser
// itself (JPEG, PNG and WebP; portrait, landscape and square): none is kept in the repository. The program is a text box whose value is the text a `Photo` draws, so a text can be put in
// from outside, as a message of a chat that another client sent is.
//  A. every format is drawn (a canvas in the box), whatever its proportions; what is not one of the three (a GIF, an SVG, a RIFF that is not WebP, a PNG signature cut short, a
//     `data:` address, a text over 48,000 characters) is not drawn, and the decoder is not even asked;
//  B. `fit: true`: the box and the canvas are the picture's own proportions (within 1:2 and 2:1), the picture is not stretched (a square on it is a square), a picture beyond the
//     bounds is cut to cover the bound (never stretched), and the box does not change its size when the picture is decoded (it has its size when the text is set);
//  C. without `fit:` the box is the `ratio:` and the picture is cut to cover it, as it was;
//  D. the policy of the page is what it was (img-src 'self' data:, no blob:, no unsafe-eval), nothing was refused, and the page has no <img>.
//   node scripts/check-photo-formats.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { serve, launch } = require('./lib-browser.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const PROGRAM = [
  'component F {',
  '    state data: String = "";',
  '    on Set(v) { data = v; }',
  '    view {',
  '        Column {',
  '            TextField { label: "Text" value: data on: Set }',
  '            Photo { label: "fixed" data: data ratio: "4:3" }',
  '            Photo { label: "fit" data: data ratio: "4:3" fit: true }',
  '        }',
  '    }',
  '}',
  '',
].join('\n');

// runs in the page
async function pageTests(win) {
  const doc = win.document;
  const results = [];
  const check = (label, ok, detail) => results.push({ label, ok: !!ok, detail: detail === undefined ? '' : String(detail) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const until = async (fn, ms) => { const end = Date.now() + (ms || 4000); while (Date.now() < end) { if (await fn()) { return true; } await sleep(20); } return false; };
  const violations = [];
  win.addEventListener('securitypolicyviolation', (e) => violations.push(e.effectiveDirective + ' ' + e.blockedURI));

  // the pictures: a white field with a red square in the middle (a side of a quarter of the shorter side of the picture, so that a stretch shows as a rectangle)
  const picture = (w, h) => { const c = doc.createElement('canvas'); c.width = w; c.height = h; const g = c.getContext('2d'); g.fillStyle = '#fff'; g.fillRect(0, 0, w, h); const s = Math.min(w, h) / 2; g.fillStyle = '#e00'; g.fillRect((w - s) / 2, (h - s) / 2, s, s); return c; };
  const b64 = async (c, type, q) => {
    const blob = await new Promise((ok) => c.toBlob(ok, type, q === undefined ? 0.8 : q));
    if (blob.type !== type) { return null; }
    const bytes = new Uint8Array(await blob.arrayBuffer());
    let s = ''; for (let i = 0; i < bytes.length; i++) { s += String.fromCharCode(bytes[i]); }
    return win.btoa(s);
  };
  const input = doc.querySelector('input[type=text], input:not([type=file])');
  const put = (text) => { const set = Object.getOwnPropertyDescriptor(win.HTMLInputElement.prototype, 'value').set; set.call(input, text); input.dispatchEvent(new win.Event('input', { bubbles: true })); };
  const box = (name) => doc.querySelector(`[aria-label="${name}"]`);
  const rect = (name) => { const r = box(name).getBoundingClientRect(); return { w: r.width, h: r.height }; };
  const canvasOf = (name) => box(name) && box(name).querySelector('canvas');
  // the box of the red square on a canvas, and its share of the canvas
  const redBox = (cv) => {
    const g = cv.getContext('2d'), d = g.getImageData(0, 0, cv.width, cv.height).data;
    let x0 = 1e9, y0 = 1e9, x1 = -1, y1 = -1;
    for (let y = 0; y < cv.height; y += 2) { for (let x = 0; x < cv.width; x += 2) { const i = (y * cv.width + x) * 4; if (d[i] > 180 && d[i + 1] < 90 && d[i + 2] < 90) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); } } }
    return x1 < 0 ? null : { w: x1 - x0 + 2, h: y1 - y0 + 2, cx: (x0 + x1) / 2 / cv.width, cy: (y0 + y1) / 2 / cv.height };
  };
  const clear = async () => { put(''); await until(() => !canvasOf('fixed') && !canvasOf('fit')); };

  const shapes = { portrait: [300, 400], landscape: [400, 300], square: [300, 300] };
  const types = ['image/jpeg', 'image/png', 'image/webp'];
  // A. every format is drawn, in every shape
  for (const type of types) {
    for (const [shape, [w, h]] of Object.entries(shapes)) {
      const text = await b64(picture(w, h), type);
      if (text === null) { check(`${type} ${shape}: the browser can encode it`, false, 'toBlob gave another type'); continue; }
      await clear();
      put(text);
      const drew = await until(() => canvasOf('fixed') && canvasOf('fit'));
      check(`A. ${type} ${shape} (${w}x${h}): drawn in a Photo with a ratio and in one that fits`, drew, text.slice(0, 12));
      if (!drew) { continue; }
      // C. the fixed box keeps 4:3 whatever the picture, and the picture is cut to cover it
      const f = rect('fixed'), cf = canvasOf('fixed');
      check(`C. ${type} ${shape}: without fit: the box is 4:3 (${f.w.toFixed(1)}x${f.h.toFixed(1)}) and the canvas 720x540`, Math.abs(f.w / f.h - 4 / 3) < 0.02 && cf.width === 720 && cf.height === 540);
      // B. the fitted box is the picture's own proportions within 1:2 and 2:1
      const want = w / h, t = rect('fit'), cv = canvasOf('fit');
      check(`B. ${type} ${shape}: fit: the box is ${w}:${h} (${t.w.toFixed(1)}x${t.h.toFixed(1)})`, Math.abs(t.w / t.h - want) < 0.02, t.w / t.h);
      check(`B. ${type} ${shape}: fit: the canvas is ${w}:${h} (${cv.width}x${cv.height}), 720 on the long side`, Math.abs(cv.width / cv.height - want) < 0.01 && Math.max(cv.width, cv.height) === 720);
      const red = redBox(cv);
      check(`B. ${type} ${shape}: fit: the picture is not stretched (the red square is a square, in the middle)`, red && Math.abs(red.w / red.h - 1) < 0.06 && Math.abs(red.cx - 0.5) < 0.02 && Math.abs(red.cy - 0.5) < 0.02, JSON.stringify(red));
      // and nothing of the picture is cut: the square is half the shorter side, so the whole of the picture is in
      check(`B. ${type} ${shape}: fit: the square is half the shorter side (all of the picture is drawn, none cut)`, red && Math.abs(red.h / Math.min(cv.width, cv.height) - 0.5) < 0.04, JSON.stringify(red));
    }
  }

  // B. the bounds: 1:2 and 2:1, cut to cover the bound, never stretched
  for (const [name, w, h, bw, bh] of [['very tall', 200, 1000, 1, 2], ['very wide', 1000, 200, 2, 1], ['exactly 1:2', 300, 600, 1, 2], ['exactly 2:1', 600, 300, 2, 1]]) {
    const text = await b64(picture(w, h), 'image/png');
    await clear(); put(text);
    const drew = await until(() => canvasOf('fit'));
    const t = drew && rect('fit'), cv = drew && canvasOf('fit');
    check(`B. ${name} (${w}x${h}) is shown at ${bw}:${bh} at the most`, drew && Math.abs(t.w / t.h - bw / bh) < 0.02 && Math.abs(cv.width / cv.height - bw / bh) < 0.01, drew && `${t.w.toFixed(1)}x${t.h.toFixed(1)} canvas ${cv.width}x${cv.height}`);
    const red = drew && redBox(cv);
    check(`B. ${name}: not stretched at the bound`, red && Math.abs(red.w / red.h - 1) < 0.08, JSON.stringify(red));
    // cut to cover the bound: the square (half the picture's shorter side) fills half of the canvas's shorter side, as it would on a picture of the bound's own proportions
    check(`B. ${name}: cut to cover the bound (the picture fills the canvas, none of it left bare)`, red && Math.abs(red.w / Math.min(cv.width, cv.height) - 0.5) < 0.05 && Math.abs(red.h / Math.min(cv.width, cv.height) - 0.5) < 0.05, JSON.stringify(red));
  }

  // B. the box does not change when the picture is decoded: it has its size when the text is set
  for (const type of types) {
    for (const [shape, [w, h]] of Object.entries(shapes)) {
      const text = await b64(picture(w, h), type);
      await clear();
      const before = rect('fit');
      put(text);
      await sleep(0);
      const at0 = rect('fit');
      await until(() => canvasOf('fit'));
      await sleep(100);
      const after = rect('fit');
      check(`B. ${type} ${shape}: no jump: the box is ${after.w.toFixed(1)}x${after.h.toFixed(1)} the moment the text is set and when the picture is drawn (the empty one: ${before.h.toFixed(1)})`, Math.abs(at0.h - after.h) < 1 && Math.abs(at0.w - after.w) < 1, `${at0.h} then ${after.h}`);
    }
  }

  // B. the other kinds of header: a lossless WebP (VP8L, quality 1), a WebP with transparency (VP8X), a PNG with transparency, and a JPEG that says in its EXIF block that it is to be turned
  const alpha = (w, h) => { const c = picture(w, h); const g = c.getContext('2d'); g.clearRect(0, 0, 40, 40); return c; };
  const exif = (b64text, orientation) => {
    const bin = win.atob(b64text), tiff = [0x4d, 0x4d, 0, 0x2a, 0, 0, 0, 8, 0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, orientation, 0, 0, 0, 0, 0, 0];
    const seg = [0xff, 0xe1, 0, 2 + 6 + tiff.length, 0x45, 0x78, 0x69, 0x66, 0, 0, ...tiff];
    let o = bin.slice(0, 2); for (const x of seg) { o += String.fromCharCode(x); } return win.btoa(o + bin.slice(2));
  };
  const kinds = {
    'a lossless WebP (VP8L)': [await b64(picture(300, 400), 'image/webp', 1), 3 / 4],
    'a WebP with transparency': [await b64(alpha(300, 400), 'image/webp', 0.8), 3 / 4],
    'a PNG with transparency': [await b64(alpha(400, 300), 'image/png'), 4 / 3],
    'a landscape JPEG whose EXIF orientation is 6 (turned a quarter: shown portrait)': [exif(await b64(picture(400, 300), 'image/jpeg'), 6), 3 / 4],
    'a landscape JPEG whose EXIF orientation is 1 (as it is)': [exif(await b64(picture(400, 300), 'image/jpeg'), 1), 4 / 3],
  };
  for (const [name, [text, want]] of Object.entries(kinds)) {
    await clear();
    put(text);
    await sleep(0);
    const at0 = rect('fit');
    const drew = await until(() => canvasOf('fit'));
    await sleep(100);
    const t = rect('fit');
    check(`B. ${name}: drawn, the box is ${want.toFixed(2)} at once and after the decode`, drew && Math.abs(at0.w / at0.h - want) < 0.02 && Math.abs(t.w / t.h - want) < 0.02, `${(at0.w / at0.h).toFixed(3)} then ${drew && (t.w / t.h).toFixed(3)}`);
  }

  // A. what is not one of the three, or is too long, is not drawn
  const real = await b64(picture(300, 200), 'image/png');
  const png = win.atob(real);
  const asBytes = (s) => { let o = ''; for (const c of s) { o += String.fromCharCode(c.charCodeAt(0)); } return o; };
  const pad = (s, n) => { while (s.length < n) { s += '\0'; } return win.btoa(s); };
  const bad = {
    'a GIF': pad('GIF89a\x01\x00\x01\x00\x80\x00\x00', 300),
    'an SVG': win.btoa('<svg xmlns="http://www.w3.org/2000/svg" width="9" height="9"><rect width="9" height="9"/></svg>'.padEnd(300, ' ')),
    'a RIFF that is not WebP (a WAVE)': pad('RIFF\x24\x00\x00\x00WAVEfmt ', 300),
    'a PNG signature cut short (7 bytes)': pad('\x89PNG\r\n\x1a', 300).replace(/^.{8}/, (m) => m),
    'a BMP': pad('BM\x36\x00\x00\x00\x00\x00\x00\x00', 300),
    'a data: address of a PNG': 'data:image/png;base64,' + real,
    'a PNG over 48,000 characters': win.btoa(asBytes(png) + '\0'.repeat(36000)),
    'a PNG under 100 characters': real.slice(0, 80),
    'a PNG in lower case': real.toLowerCase(),
  };
  // the cut-short signature: the first seven bytes only, the rest zero (not a PNG, and not a JPEG)
  bad['a PNG signature cut short (7 bytes)'] = pad('\x89PNG\r\n\x1a', 300);
  let asked = 0;
  const realBitmap = win.createImageBitmap;
  win.createImageBitmap = function () { asked++; return realBitmap.apply(win, arguments); };
  for (const [name, text] of Object.entries(bad)) {
    await clear(); asked = 0;
    put(text);
    await sleep(250);
    check(`A. ${name} is not drawn, and the decoder is not asked`, !canvasOf('fixed') && !canvasOf('fit') && asked === 0, `asked ${asked}`);
  }
  win.createImageBitmap = realBitmap;
  // a text that has the signature and is not a picture draws nothing and breaks nothing (the decoder refuses it)
  await clear();
  put('iVBORw0KGgo' + 'A'.repeat(300));
  await sleep(300);
  check('A. a PNG signature and nothing else is not drawn (the decoder refuses it), the box stays a gradient', !canvasOf('fixed') && !!box('fixed'));

  // D. the policy
  check('D. no <img> and no data: or blob: request', doc.querySelectorAll('img').length === 0 && win.performance.getEntriesByType('resource').every((e) => e.name.indexOf('data:') !== 0 && e.name.indexOf('blob:') !== 0));
  const meta = doc.querySelector('meta[http-equiv="Content-Security-Policy"]');
  const csp = meta ? meta.getAttribute('content') : '';
  check("D. the page's policy is the baseline: img-src 'self' data:, no blob:, no unsafe-eval, no remote origin", /img-src 'self' data:(;|$)/.test(csp) && csp.indexOf('blob:') < 0 && csp.indexOf('unsafe-eval') < 0 && !/https?:/.test(csp), csp);
  check('D. nothing was refused by the policy', violations.length === 0, violations.join('; '));
  return results;
}

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-photo-formats-'));
  fs.writeFileSync(path.join(tmp, 'f.cui'), PROGRAM);
  const r = spawnSync(CANCHO_UI, ['build', path.join(tmp, 'f.cui'), '--page', '-o', path.join(tmp, 'out'), '--name', 'formats'], { encoding: 'utf8' });
  if (r.status !== 0) { check('the program builds', false, `${r.stdout}${r.stderr}`.slice(0, 300)); console.log(`\n${failures.length} check(s) failed`); process.exit(1); }
  const browser = await launch();
  console.log(`browser: ${browser.name}`);
  const s = await serve(path.join(tmp, 'out'));
  const page = await browser.open(`http://127.0.0.1:${s.address().port}/`);
  try {
    const results = await page.evaluate(`(${pageTests.toString()})(window)`);
    for (const x of results) { check(x.label, x.ok, x.ok ? '' : x.detail); }
    check('no page or console error', page.problems.filter((p) => !/favicon\.ico/.test(p)).length === 0, page.problems.join('; '));
  } finally { await page.close(); s.close(); }
  await browser.close();
  fs.rmSync(tmp, { recursive: true, force: true });
  if (failures.length) { console.log(`\n${failures.length} check(s) failed`); process.exit(1); }
  console.log('\nall checks passed');
})().catch((e) => { console.error(e); process.exit(2); });
