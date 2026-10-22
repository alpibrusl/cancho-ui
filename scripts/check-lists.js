#!/usr/bin/env node
// The emitted `__cui_sort`, `__cui_reverse`, `__cui_contains`, `__cui_lower` and the number helpers against the corpus tests/lists_corpus.json
// (docs/lists.md), the same file the Rust model is checked against by `cargo test`, and a page in Chromium that filters,
// sorts and reverses a list as the user types. Outside the gate: needs node, the `playwright` package with a Chromium,
// and a built `cancho-ui`.
//   node scripts/check-lists.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const SRC = `
component Rows {
    requires Network;
    state names: List[String] = ["pear", "Apple", "fig", "apple", "Fig"];
    state q: String = "";
    state shown: List[String] = [];
    state body: String = "";
    state recs: List[String] = [];
    state joined: String = "";
    state folded: String = "";
    state pages: Int = 0;
    state r: Result[String, String] = Ok("");

    on Query(v) {
        q = v;
        folded = fold(v);
        body = "\\{\\\"q\\\":" + json.quote(v) + "\\}";
        shown = sort(filter(names, s => contains(lower(s), lower(v))), s => lower(s));
    }
    on Flip { shown = reverse(shown); }
    on Next { shown = take(skip(shown, 1), 2); }
    on Join { shown = concat(take(names, 1), skip(names, 2)); }
    on Load {
        r = await(net.get("readings.txt"));
        recs = match r { Ok(t) => { map(lines(t), l => trim(upper(l))) } Err(e) => { [] } Cancelled => { [] } };
        joined = join(filter(recs, l => starts_with(l, "T")), "|") + "/" + join(filter(recs, l => ends_with(l, "3")), "|");
    }
    on Pages { pages = max(ceil(len(names) / 2), 1) + min(1, 2) + abs(0 - 1) + floor(1.5) + round(1.5); }
    on Order { shown = reverse(sort(["9", "10", "x", "2.5", "-1"], s => num(s, 1000))); }

    view {
        Column {
            TextField { label: "Search" value: q on: Query }
            Button { label: "Flip" on: Flip }
            Button { label: "Order" on: Order }
            Button { label: "Next" on: Next }
            Button { label: "Join" on: Join }
            Button { label: "Load" on: Load }
            Button { label: "Pages" on: Pages }
            Text { "pages {pages}" }
            Text { "joined: {joined}" }
            Text { "folded: {folded}" }
            For {
                over: recs
                as: rec
                Text { "rec {rec}" }
            }
            Text { "{len(shown)} matches" }
            Text { "body {body}" }
            For {
                over: shown
                as: row
                Text { "row {row}" }
            }
        }
    }
}
`;

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-lists-'));
  const src = path.join(dir, 'p.cui');
  fs.writeFileSync(src, SRC);
  const r = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }

  // the helpers against the corpus
  const js = fs.readFileSync(path.join(dir, 'out', fs.readdirSync(path.join(dir, 'out')).find((f) => f.endsWith('.js') && f !== 'sw.js')), 'utf8');
  const from = js.indexOf('function __cui_sort');
  const to = js.indexOf('function mount_', from);
  if (from < 0 || to < 0) { throw new Error('the helpers are not in the built program'); }
  const lib = new Function(js.slice(from, to) + '\nreturn { sort: __cui_sort, lower: __cui_lower, contains: __cui_contains, reverse: __cui_reverse, num: __cui_num, take: __cui_take, skip: __cui_skip, concat: __cui_concat, fold: __cui_fold, fold_cp: __cui_fold_cp, lines: __cui_lines, split: __cui_split, join: __cui_join, trim: __cui_trim, upper: __cui_upper, starts_with: __cui_starts_with, ends_with: __cui_ends_with, quote: __cui_json_quote, min: __cui_min, max: __cui_max, abs: __cui_abs, floor: __cui_floor, ceil: __cui_ceil, round: __cui_round };')();
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'lists_corpus.json'), 'utf8'));
  for (const e of corpus) {
    const keys = e.keys.map((k) => (k !== null && typeof k === 'object' && !Array.isArray(k) ? NaN : k));
    const got = lib.sort(keys.map((_, i) => i), (i) => keys[i]);
    check(e.why, JSON.stringify(got) === JSON.stringify(e.expect), `got ${JSON.stringify(got)}`);
  }
  check('lower: ASCII only', lib.lower('AbCÉİz') === 'abcÉİz');
  check('contains: case-sensitive, empty needle', lib.contains('Apple', 'pp') && !lib.contains('Apple', 'PP') && lib.contains('x', ''));
  const orig = [1, 2, 3];
  check('reverse: a new list', JSON.stringify(lib.reverse(orig)) === '[3,2,1]' && orig[0] === 1);

  const nums = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'num_corpus.json'), 'utf8'));
  for (const e of nums) {
    const got = lib.num(e.text, e.default);
    const want = e.negzero ? -0 : e.expect;
    check(`num ${JSON.stringify(e.text)} -> ${e.negzero ? '-0' : e.expect}`, Object.is(got, want), `got ${got}`);
  }

  const paging = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'paging_corpus.json'), 'utf8'));
  const special = { NaN: NaN, Infinity: Infinity, '-Infinity': -Infinity };
  for (const e of paging) {
    const n = typeof e.n === 'string' ? special[e.n] : e.n;
    const input = JSON.stringify(e.list);
    const t = lib.take(e.list, n);
    const k = lib.skip(e.list, n);
    check(`take: ${e.why}`, JSON.stringify(t) === JSON.stringify(e.take), `got ${JSON.stringify(t)}`);
    check(`skip: ${e.why}`, JSON.stringify(k) === JSON.stringify(e.skip), `got ${JSON.stringify(k)}`);
    check(`unchanged: ${e.why}`, JSON.stringify(e.list) === input);
  }
  check('take/skip: a count that is not a number is 0', lib.take([1, 2], 'x').length === 0 && lib.take([1, 2, 3], '2').length === 0 && lib.skip([1, 2], undefined).length === 2);

  const concat = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'concat_corpus.json'), 'utf8'));
  for (const e of concat.ok) {
    const a = JSON.stringify(e.a);
    const b = JSON.stringify(e.b);
    const got = lib.concat(e.a, e.b);
    check(`concat: ${e.why}`, JSON.stringify(got) === JSON.stringify(e.concat), `got ${JSON.stringify(got)}`);
    check(`concat unchanged inputs: ${e.why}`, JSON.stringify(e.a) === a && JSON.stringify(e.b) === b);
    check(`concat is a new list: ${e.why}`, got !== e.a && got !== e.b);
  }
  for (const e of concat.refused) {
    let message = null;
    try { lib.concat(e.a, e.b); } catch (err) { message = String(err && err.message); }
    check(`concat refuses: ${e.why}`, message !== null && message.startsWith(e.rule), `got ${message}`);
  }

  const fold = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'fold_corpus.json'), 'utf8'));
  for (const e of fold.cases) {
    check(`fold: ${e.why}`, lib.fold(e.text) === e.fold, `got ${JSON.stringify(lib.fold(e.text))}`);
  }
  for (const [a, b] of fold.equal) { check(`fold equal: ${a} ${b}`, lib.fold(a) === lib.fold(b)); }
  for (const [a, b] of fold.different) { check(`fold different: ${a} ${b}`, lib.fold(a) !== lib.fold(b)); }
  // Every code point through the emitted table: an FNV-1a digest, computed by the generator from Python's own folding.
  let h = 2166136261;
  for (let cp = 0; cp < 0x110000; cp++) {
    if (cp >= 0xD800 && cp <= 0xDFFF) { continue; }
    h = Math.imul(h ^ lib.fold_cp(cp), 16777619) >>> 0;
  }
  check(`fold: the digest over every code point (${fold.all_code_points.count}) matches the generator's`, h === fold.all_code_points.fnv1a32, `got ${h}, want ${fold.all_code_points.fnv1a32}`);
  let threw = null;
  try { lib.fold(5); } catch (err) { threw = String(err && err.message); }
  check('fold refuses a non-string', threw !== null && threw.startsWith('replay-type-error'), `got ${threw}`);

  const math = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'math_corpus.json'), 'utf8'));
  const special_num = { NaN: NaN, Infinity: Infinity, '-Infinity': -Infinity };
  const num_of = (v) => (typeof v === 'string' ? special_num[v] : v);
  for (const e of math) {
    const got = lib[e.op](...e.args.map(num_of));
    check(`${e.op}(${e.args.join(', ')}): ${e.why}`, Object.is(got, num_of(e.expect)), `got ${Object.is(got, -0) ? '-0' : got}`);
  }
  check('math: a value that is not a number is NaN, not coerced', Number.isNaN(lib.min('1', 2)) && Number.isNaN(lib.abs('-3')) && Number.isNaN(lib.ceil(undefined)) && Number.isNaN(lib.floor(null)));

  const texts = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'text_corpus.json'), 'utf8'));
  for (const e of texts) {
    const args = e.args.map((a) => (a !== null && typeof a === 'object' && !Array.isArray(a) ? a.repeat[0].repeat(a.repeat[1]) : a));
    const got = lib[e.op](...args);
    const ok = e.expect_len !== undefined ? got.length === e.expect_len : JSON.stringify(got) === JSON.stringify(e.expect);
    check(`${e.op}: ${e.why}`, ok, `got ${JSON.stringify(got).slice(0, 80)}`);
  }

  const quotes = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'json_quote_corpus.json'), 'utf8'));
  for (const e of quotes) {
    const got = lib.quote(e.text);
    check(`json.quote: ${e.why}`, got === e.expect, `got ${JSON.stringify(got)}`);
  }

  // the page
  fs.writeFileSync(path.join(dir, 'out', 'readings.txt'), 'temp 21.5 \r\n  hum 40\n\npress 1013\n');
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, 'out', path.normalize(rel));
    if (!file.startsWith(path.join(dir, 'out')) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  const rows = async () => (await page.locator('#root').innerText()).split('\n').filter((l) => l.startsWith('row ')).map((l) => l.slice(4));
  await page.fill('input', 'x');
  await page.fill('input', '');
  check('sorted by lower-cased name, stable', JSON.stringify(await rows()) === JSON.stringify(['Apple', 'apple', 'fig', 'Fig', 'pear']), JSON.stringify(await rows()));
  await page.fill('input', 'F');
  check('filtered as the user types', JSON.stringify(await rows()) === JSON.stringify(['fig', 'Fig']), JSON.stringify(await rows()));
  check('the count follows', (await page.locator('#root').innerText()).includes('2 matches'));
  await page.fill('input', '');
  await page.click('text=Flip');
  check('reversed', JSON.stringify(await rows()) === JSON.stringify(['pear', 'Fig', 'fig', 'apple', 'Apple']), JSON.stringify(await rows()));
  await page.click('text=Order');
  check('rows sorted by a number held in text', JSON.stringify(await rows()) === JSON.stringify(['x', '10', '9', '2.5', '-1']), JSON.stringify(await rows()));
  await page.click('text=Next');
  check('paged: skip one, take two', JSON.stringify(await rows()) === JSON.stringify(['10', '9']), JSON.stringify(await rows()));
  await page.click('text=Join');
  check('concat on the page: the first name, then all but the first two', JSON.stringify(await rows()) === JSON.stringify(['pear', 'fig', 'apple', 'Fig']), JSON.stringify(await rows()));
  await page.click('text=Pages');
  check('min, max, abs, floor, ceil and round on the page', (await page.locator('#root').innerText()).includes('pages 8'), (await page.locator('#root').innerText()).split('\n').find((l) => l.startsWith('pages')));
  await page.click('text=Load');
  const recs = async () => (await page.locator('#root').innerText()).split('\n').filter((l) => l === 'rec' || l.startsWith('rec ')).map((l) => l.slice(4));
  for (let i = 0; i < 100 && (await recs()).length === 0; i++) { await page.waitForTimeout(30); }
  check('a text answer is a row per line', JSON.stringify(await recs()) === JSON.stringify(['TEMP 21.5', 'HUM 40', '', 'PRESS 1013']), JSON.stringify(await recs()));
  check('and is filtered and joined', (await page.locator('#root').innerText()).includes('joined: TEMP 21.5/PRESS 1013'));
  await page.fill('input', 'a"b\\c');
  check('braces and json.quote on the page: an object body whose text cannot change it', (await page.locator('#root').innerText()).includes('body {"q":"a\\"b\\\\c"}'), (await page.locator('#root').innerText()).split('\n').find((l) => l.startsWith('body')));
  await page.fill('input', '\u00dcNIT \u0130');
  check('fold on the page: capital U-umlaut folds, the dotted capital I stays', (await page.locator('#root').innerText()).includes('folded: \u00fcnit \u0130'), (await page.locator('#root').innerText()).split('\n').find((l) => l.startsWith('folded')));
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
