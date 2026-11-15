#!/usr/bin/env node
// The emitted `__cui_exact` (docs/lists.md section 11), in node: it reproduces every key of tests/exact_corpus.json (the keys the Rust replay makes), and
// the order of its keys is the order of the numbers, by BigInt arithmetic (not a double, and no code shared with the key), for integer parts of up
// to 40 digits and fractions of up to 20. Needs node and a built `cancho-ui`.
//   node scripts/check-exact.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const vm = require('vm');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'release', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${!ok && detail !== undefined ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const out = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'check-exact-')), 'exact.js');
const built = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/exact/exact.cui'), '-o', out], { encoding: 'utf8' });
if (built.status !== 0) { throw new Error(built.stderr); }
const src = fs.readFileSync(out, 'utf8');
const start = src.indexOf('function __cui_exact(');
if (start < 0) { throw new Error('the helper is not in the emitted code'); }
let depth = 0, end = start;
for (let i = src.indexOf('{', start); i < src.length; i++) {
  if (src[i] === '{') { depth += 1; } else if (src[i] === '}') { depth -= 1; if (depth === 0) { end = i + 1; break; } }
}
const exact = vm.runInNewContext(`${src.slice(start, end)}; __cui_exact`);

// 1. the corpus: the keys the replay makes
const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests/exact_corpus.json'), 'utf8'));
const bad = corpus.filter((c) => exact(c.text, c.default) !== c.key);
check(`the ${corpus.length} keys of the corpus are reproduced`, bad.length === 0, JSON.stringify(bad.slice(0, 3)));

// 2. the oracle: BigInt, aligned to a common scale
const SCALE = 20;
function scaled(text) {
  const neg = text.startsWith('-');
  const [i, f = ''] = (neg ? text.slice(1) : text).split('.');
  const v = BigInt(i) * 10n ** BigInt(SCALE) + BigInt(f.padEnd(SCALE, '0'));
  return neg ? -v : v;
}
let seed = 0x2545F491n;
const rnd = (n) => { seed = (seed * 6364136223846793005n + 1442695040888963407n) & 0xFFFFFFFFFFFFFFFFn; return Number((seed >> 33n) % BigInt(n)); };
const digits = (n) => Array.from({ length: n }, () => rnd(10)).join('');
function decimal() {
  const ilen = [0, 1, 1, 2, 5, 12, 25, 40][rnd(8)];
  const flen = [0, 0, 1, 3, 8, 20][rnd(6)];
  let int = digits(ilen).replace(/^0+(?=.)/, '') || '0';
  const frac = digits(flen);
  const a = (rnd(2) ? '-' : '') + int + (frac ? '.' + frac : '');
  return [a, a + (a.includes('.') ? '0' : '.0')];
}
let wrong = [];
const N = 30000;
for (let k = 0; k < N && wrong.length < 3; k++) {
  const [a, a2] = decimal();
  const [b] = decimal();
  const ka = exact(a, '0'), ka2 = exact(a2, '0'), kb = exact(b, '0');
  const want = scaled(a) < scaled(b) ? -1 : scaled(a) > scaled(b) ? 1 : 0;
  const got = ka < kb ? -1 : ka > kb ? 1 : 0;
  if (ka !== ka2 || got !== want) { wrong.push({ a, b, ka, kb, want, got, same: ka === ka2 }); }
}
check(`${N} random pairs (integer parts to 40 digits, fractions to 20) are ordered like the numbers, equal values with equal keys`, wrong.length === 0, JSON.stringify(wrong));

// 3. the invalid texts
const invalid = ['', ' ', ' 1', '1 ', '+1', '1e3', '1E3', '.5', '5.', '007', '-', '--1', '1.2.3', '0x10', 'NaN', 'Infinity', '1,5', '-.5', '1\n', '1.5\n'];
const stray = invalid.filter((t) => exact(t, '7') !== exact('7', '0') || exact(t, 'oops') !== '3');
check('invalid texts are the default\'s key, and 3 when the default is not a decimal', stray.length === 0, JSON.stringify(stray));
check('a million digits is not a decimal', exact('1'.repeat(1000000), '0') === exact('0', '0'));
console.log(failures.length === 0 ? 'all exact checks passed' : `${failures.length} failed`);
process.exit(failures.length === 0 ? 0 : 1);
