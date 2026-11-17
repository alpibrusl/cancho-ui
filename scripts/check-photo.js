#!/usr/bin/env node
// `PhotoField` and `Photo` (docs/structure.md 12.0r) in Chromium, in the mobile profile (a Pixel 7: 412 x 915, touch), with every picture drawn at run time on a canvas (no picture is
// kept in the repository):
//  A. tests/photo_corpus.json, the file `cargo test` replays (crates/cancho-ui/tests/photo.rs): the page's check of a picture text (`__cui_photo_ok`, cut out of the page's script and run
//     here) agrees with the compiler's for every case, and the page's script and the compiler's say the same limits;
//  B. scripts/photo-page-tests.js in the page: the chooser (a native file input named by its label, a 44 px key, an icon over the caption), a picture larger than max_side brought down,
//     the square cut and the proportions kept, a small picture not made larger, the EXIF orientation applied and the EXIF block and the place gone, a PNG's transparent part white, what
//     is not a picture (by its bytes and by its type), a file over 40 MiB, the status line kept through a redraw, a stale choice told to nobody, a `Photo` drawn on a canvas of its ratio,
//     a decorative one hidden from a screen reader, no <img>, no policy violation, and the policy refusing another origin;
//  C. the cap to the character: a cap that the first encoding exactly meets is met by that encoding (not by a worse one), a cap one under it is met by a smaller one, and a cap no
//     picture can meet (100) is "too big";
//  D. the gallery demo (examples/photo) builds as a page, takes a picture and draws it three ways.
// Outside the gate: needs node, a Chromium: the `playwright` package if it is installed, else Chrome or Chromium found through $CHROME (or the usual places) and driven over the
// DevTools protocol; and a built `cancho-ui`.
//   node scripts/check-photo.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const vm = require('vm');
const { spawnSync } = require('child_process');
const pageTests = require('./photo-page-tests.js');
const { serve, launch } = require('./lib-browser.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

// ---- the programs: three choosers (a square one, one that keeps the proportions, one whose cap is given) and the pictures they are drawn by ----
function program(cap) {
  return [
    'component P {',
    '    state picture: String = "";',
    '    state bad: String = "this is not the text of a picture";',
    '    state tone: String = "";',
    '    state sq: String = "";',
    '    state wide: String = "";',
    '    state capd: String = "";',
    '    state nsq: Int = 0;',
    '    state nwide: Int = 0;',
    '    state ncap: Int = 0;',
    '    state bump: Int = 0;',
    '    on PickSq(r) { sq = r; nsq = nsq + 1; }',
    '    on PickWide(r) { wide = r; nwide = nwide + 1; picture = json.get(r, "data", ""); }',
    '    on PickCap(r) { capd = r; ncap = ncap + 1; }',
    '    on Bump { bump = bump + 1; }',
    '    view {',
    '        Column {',
    '            PhotoField { label: "Square" square: true max_side: 200 max_chars: 48000 on: PickSq }',
    '            PhotoField { label: "Wide" max_side: 300 max_chars: 48000 on: PickWide }',
    `            PhotoField { label: "Capped" max_side: 160 max_chars: ${cap} on: PickCap }`,
    '            Text { "n={nsq},{nwide},{ncap}" }',
    '            Text { "sq={sq}" }',
    '            Text { "wide={wide}" }',
    '            Text { "cap={capd}" }',
    '            Button { label: "Bump" on: Bump }',
    '            Text { "bump={bump}" }',
    '            Photo { label: "Wide picture" data: picture ratio: "2:1" tone: tone }',
    '            Photo { decorative: true data: picture ratio: "1:1" }',
    '            Photo { label: "Not a picture" data: bad ratio: "4:3" tone: "3" }',
    '        }',
    '    }',
    '}',
    '',
  ].join('\n');
}

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-photo-'));
  const build = (name, file, extra) => {
    const dir = path.join(tmp, name);
    const r = spawnSync(CANCHO_UI, ['build', file, '--page', '-o', dir, '--name', name, ...(extra || [])], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stdout}${r.stderr}`); }
    return dir;
  };
  const buildCap = (cap) => {
    const dir = path.join(tmp, `cap${cap}-src`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'p.cui'), program(cap));
    return build(`cap${cap}`, path.join(dir, 'p.cui'));
  };
  const browser = await launch();
  console.log(`browser: ${browser.name}`);
  const run = async (dir, cfg) => {
    const s = await serve(dir);
    const page = await browser.open(`http://127.0.0.1:${s.address().port}/`);
    try {
      const out = await page.evaluate(`(${pageTests.toString()})(window, ${JSON.stringify(cfg)})`);
      return { out, problems: page.problems };
    } finally { await page.close(); s.close(); }
  };

  // ---- A. the corpus: the compiler's check and the page's ----
  {
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'photo_corpus.json'), 'utf8'));
    const dir = buildCap(48000);
    const js = fs.readFileSync(path.join(dir, 'app.js'), 'utf8');
    const m = /function __cui_photo_kind\(t\) \{[\s\S]*?\n\}\nfunction __cui_photo_ok\(t\) \{[^\n]*\}\n/.exec(js);
    check('the page carries __cui_photo_kind and __cui_photo_ok', !!m);
    const ok = vm.runInNewContext(`${m[0]}; __cui_photo_ok`, { atob });
    for (const c of corpus.pictures) {
      const text = c.head + c.fill.repeat(Math.max(0, c.len - c.head.length - (c.tail || '').length)) + (c.tail || '');
      check(`corpus: ${c.why}`, ok(text) === c.ok);
    }
    check('corpus: a value that is not text is no picture', ok(5) === false && ok(null) === false && ok(undefined) === false && ok(['/9j/']) === false);
    const face = /photo\.length < (\d+) \|\| photo\.length > (\d+) \|\| photo\.indexOf\("([^"]+)"\) !== 0/.exec(js);
    check('the page\'s face(..) keeps its limits and its JPEG-only start (it is not changed)', !!face && (face[1] === '100' && face[2] === '48000' && face[3] === '/9j/'));
  }

  // ---- B. the elements ----
  {
    const t = await run(buildCap(48000), { mode: 'main' });
    for (const r of t.out.results) { check(r.label, r.ok, r.ok ? '' : r.detail); }
    check('main: no page or console error', t.problems.filter((p) => !/elsewhere\.invalid|Content Security Policy|Refused to|ERR_FAILED|net::|favicon\.ico/.test(p)).length === 0, t.problems.join('; '));
  }

  // ---- C. the cap, to the character ----
  {
    const probe = async (cap) => (await run(buildCap(cap), { mode: 'cap', noise: 700 })).out;
    const first = await probe(48000);
    const L = first.payload && first.payload.data ? first.payload.data.length : 0;
    check('cap: a noise picture of 700x700 fits 48000 characters at the first try', L > 0 && L % 4 === 0 && L <= 48000, L);
    const exact = await probe(L);
    check(`cap: a cap of exactly ${L} is met by the same encoding, not by a worse one`, exact.payload && exact.payload.data && exact.payload.data.length === L, exact.payload && JSON.stringify(exact.payload).slice(0, 60));
    const under = await probe(L - 1);
    check(`cap: a cap of ${L - 1} is met by a smaller picture, never by a longer text`, under.payload && under.payload.data && under.payload.data.length <= L - 1 && under.payload.data.length > 0, under.payload && under.payload.data ? under.payload.data.length : JSON.stringify(under.payload));
    const tiny = await probe(100);
    check('cap: a cap no picture can meet (100) is "too big"', tiny.payload && JSON.stringify(tiny.payload) === '{"error":"too big"}', JSON.stringify(tiny.payload));
    check('cap: no policy violation', [first, exact, under, tiny].every((r) => r.violations.length === 0));
  }

  // ---- D. the gallery demo ----
  {
    const dir = build('demo', path.join(ROOT, 'examples/photo/photo.cui'));
    const s = await serve(dir);
    const page = await browser.open(`http://127.0.0.1:${s.address().port}/`);
    try {
      const r = await page.evaluate(`(async () => {
        const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
        const c = document.createElement('canvas'); c.width = 640; c.height = 480;
        const g = c.getContext('2d'); g.fillStyle = '#d33'; g.fillRect(0, 0, 640, 480); g.fillStyle = '#fc0'; g.beginPath(); g.arc(320, 240, 120, 0, 7); g.fill();
        const blob = await new Promise((ok) => c.toBlob(ok, 'image/png'));
        const dt = new DataTransfer(); dt.items.add(new File([blob], 'sun.png', { type: 'image/png' }));
        const input = document.querySelector('input[type=file]'); input.files = dt.files; input.dispatchEvent(new Event('change', { bubbles: true }));
        for (let i = 0; i < 200 && document.querySelectorAll('canvas').length < 3; i++) { await sleep(50); }
        const cv = Array.from(document.querySelectorAll('.cui-photo canvas'));
        return { note: document.body.innerText.includes('Picture ready.'), canvases: cv.length, sizes: cv.map((x) => x.width + 'x' + x.height), cover: !!document.querySelector('.cui-overlay-cover > .cui-photo:first-child canvas'), banner: !!document.querySelector('.cui-overlay-banner > .cui-photo[aria-hidden=true]:first-child canvas'), imgs: document.querySelectorAll('img').length };
      })()`);
      check('demo: a picture drawn at run time and chosen is drawn three ways (wide, cover card, banner)', r.note && r.canvases === 3 && r.cover && r.banner && r.imgs === 0, JSON.stringify(r));
      check('demo: no page or console error', page.problems.filter((p) => !/favicon\.ico/.test(p)).length === 0, page.problems.join('; '));
    } finally { await page.close(); s.close(); }
  }

  await browser.close();
  fs.rmSync(tmp, { recursive: true, force: true });
  if (failures.length) { console.log(`\n${failures.length} check(s) failed`); process.exit(1); }
  console.log('\nall checks passed');
})().catch((e) => { console.error(e); process.exit(2); });
