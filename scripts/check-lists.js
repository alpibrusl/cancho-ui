#!/usr/bin/env node
// The emitted `__cui_sort`, `__cui_reverse`, `__cui_contains` and `__cui_lower` against the corpus tests/lists_corpus.json
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
    state names: List[String] = ["pear", "Apple", "fig", "apple", "Fig"];
    state q: String = "";
    state shown: List[String] = [];

    on Query(v) {
        q = v;
        shown = sort(filter(names, s => contains(lower(s), lower(v))), s => lower(s));
    }
    on Flip { shown = reverse(shown); }
    on Order { shown = reverse(sort(["9", "10", "x", "2.5", "-1"], s => num(s, 1000))); }

    view {
        Column {
            TextField { label: "Search" value: q on: Query }
            Button { label: "Flip" on: Flip }
            Button { label: "Order" on: Order }
            Text { "{len(shown)} matches" }
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
  const lib = new Function(js.slice(from, to) + '\nreturn { sort: __cui_sort, lower: __cui_lower, contains: __cui_contains, reverse: __cui_reverse, num: __cui_num };')();
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

  // the page
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
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
