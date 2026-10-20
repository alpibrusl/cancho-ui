#!/usr/bin/env node
// The emitted `__cui_json_get` against the corpus tests/json_get_corpus.json (docs/language-design.md decision 13),
// the same file the Rust model is checked against by `cargo test`. Outside the gate: needs node and a built `cancho-ui`.
//   node scripts/check-json-get.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-json-get-'));
const src = path.join(dir, 'p.cui');
fs.writeFileSync(src, 'component P {\n    state v: String = "";\n    on Go { v = json.get("x", "a", "?"); }\n    view { Text { v } }\n}\n');
const out = path.join(dir, 'p.js');
const r = spawnSync(CANCHO_UI, ['build', src, '-o', out], { encoding: 'utf8' });
if (r.status !== 0) { console.error(r.stderr); process.exit(1); }
const js = fs.readFileSync(out, 'utf8');
const from = js.indexOf('function __cui_json_depth');
const to = js.indexOf('function mount_', from);
if (from < 0 || to < 0) { console.error('the helper is not in the built program'); process.exit(1); }
const lib = new Function(js.slice(from, to) + '\nreturn { get: __cui_json_get, items: __cui_json_items };')();
const jsonGet = lib.get;

const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'json_get_corpus.json'), 'utf8'));
let failed = 0;
function same(a, b) { return JSON.stringify(a) === JSON.stringify(b); }
for (const e of corpus) {
  let ok;
  let detail = '';
  if (e.items_n) {
    const got = lib.items('[' + Array(e.items_n).fill('1').join(',') + ']', '');
    ok = got.length === e.expect_len;
    detail = `got ${got.length} items, expected ${e.expect_len}`;
  } else if (e.items) {
    const got = lib.items(e.text, e.path).map((item) => jsonGet(item, e.then, '?'));
    ok = same(got, e.expect);
    detail = `got ${JSON.stringify(got)}, expected ${JSON.stringify(e.expect)}`;
  } else {
    let text, p, d;
    if (e.nest) {
      text = '['.repeat(e.nest) + '7' + ']'.repeat(e.nest);
      p = Array(e.nest).fill('0').join('.');
      d = '?';
    } else {
      ({ text, path: p, default: d } = e);
    }
    const got = jsonGet(text, p, d);
    ok = got === e.expect;
    detail = `got ${JSON.stringify(got)}, expected ${JSON.stringify(e.expect)}`;
  }
  if (!ok) { failed++; }
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${e.why}${ok ? '' : `: ${detail}`}`);
}
// a deep text and a hostile one never throw
for (const hostile of ['['.repeat(200000), '{"a":'.repeat(100000), '"' + '\\'.repeat(50001), '\u0000', String.fromCharCode(0xd800)]) {
  let threw = false;
  try { jsonGet(hostile, 'a', '?'); lib.items(hostile, 'a'); } catch (x) { threw = true; }
  if (threw) { failed++; }
  console.log(`${threw ? 'FAIL' : 'ok  '} a hostile text of ${hostile.length} units does not throw`);
}
console.log(failed ? `\n${failed} check(s) failed` : `\nall ${corpus.length} cases and 5 hostile texts passed`);
process.exit(failed ? 1 : 0);
