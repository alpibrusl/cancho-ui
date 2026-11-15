#!/usr/bin/env node
// Mutants of `exact` (docs/lists.md section 11.4): each must make `cargo test --test exact` or `scripts/check-exact.js` fail, and the table says which.
// Needs node and cargo.   node scripts/check-exact-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const RS = 'crates/cancho-ui-replay/src/eval.rs';
const JS = 'crates/cancho-ui-codegen-web/src/lists.rs';
const MUTANTS = [
  ['rust: the length of a positive is not padded to six digits', RS, 'key.push_str(&format!("{int_len:06}"));', 'key.push_str(&format!("{int_len}"));'],
  ['rust: the length of a negative is not complemented', RS, '999_999 - int_len', 'int_len'],
  ['rust: the digits of a negative are not complemented', RS, "key.push(char::from(b'9' - (d - b'0')));", 'key.push(char::from(d));'],
  ['rust: the end of a negative has no tilde', RS, "        key.push('~');\n", ''],
  ['rust: the zeros of the fraction are kept', RS, 'while kept > 0 && frac[kept - 1] == b\'0\' {', 'while false && kept > 0 && frac[kept - 1] == b\'0\' {'],
  ['rust: zero is keyed as a positive', RS, 'if int.iter().all(|&c| c == b\'0\') && frac.is_empty() {', 'if false {'],
  ['rust: positives and negatives share a class', RS, "        key.push('2');", "        key.push('0');"],
  ['rust: a leading zero is accepted', RS, ' || (rest[0] == b\'0\' && int_len > 1)', ''],
  ['rust: a million digits are accepted', RS, 'int_len > 999_999', 'int_len > 9_999_999'],
  ['rust: the default is not used', RS, '        .or_else(|| decimal_key(default))\n', ''],
  ['js: the length is padded to five digits', JS, 'slice(-6)', 'slice(-5)'],
  ['js: the digits of a negative are not complemented', JS, 'String(9 - Number(c))', 'String(Number(c))'],
  ['js: the end of a negative has no tilde', JS, ' + comp(frac) + \\"~\\"', ' + comp(frac)'],
  ['js: the zeros of the fraction are kept', JS, 'm[3].replace(/0+$/, \\"\\")', 'm[3]'],
  ['js: an exponent is accepted', JS, '(?:\\\\.([0-9]+))?$/.exec(s)', '(?:\\\\.([0-9]+))?(?:[eE][+-]?[0-9]+)?$/.exec(s)'],
  ['js: the default is ignored', JS, 'return b !== null ? b : \\"3\\";', 'return \\"3\\";'],
  ['js: a leading zero is accepted', JS, '/^(-?)(0|[1-9][0-9]*)(?:', '/^(-?)([0-9]+)(?:'],
];

const only = process.argv.includes('--only') ? process.argv[process.argv.indexOf('--only') + 1] : '';
let survivors = 0, ran = 0;
for (const [name, file, from, to] of MUTANTS) {
  if (!name.includes(only)) { continue; }
  const full = path.join(ROOT, file);
  const original = fs.readFileSync(full, 'utf8');
  const count = original.split(from).length - 1;
  if (count !== 1) { console.log(`SKIP ${name}: the text occurs ${count} times, not once`); survivors += 1; continue; }
  ran += 1;
  try {
    fs.writeFileSync(full, original.replace(from, () => to));
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'exact'], { cwd: ROOT, encoding: 'utf8' });
    const b = spawnSync('node', [path.join(__dirname, 'check-exact.js')], { cwd: ROOT, encoding: 'utf8' });
    const by = [t.status !== 0 ? 'rust tests' : '', b.status !== 0 ? 'node' : ''].filter(Boolean).join(' + ');
    console.log(`${by ? 'killed  ' : 'SURVIVED'} ${name}${by ? `  [${by}]` : ''}`);
    if (!by) { survivors += 1; }
  } finally {
    fs.writeFileSync(full, original);
  }
}
spawnSync('cargo', ['build', '--release'], { cwd: ROOT });
console.log(`${ran - survivors}/${ran} killed`);
process.exit(survivors ? 1 : 0);
