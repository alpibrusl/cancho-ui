#!/usr/bin/env node
// The contrast oracle of tests/theme_corpus.json (docs/theme.md section 4), written apart from the compiler's Rust. It computes the WCAG 2.x
// contrast ratio of every pair of the vendored palette, in both schemes, and for each case of the corpus the pairs that fail with their
// ratio cut to two decimals (4.499 is "4.49", never "4.50"). `cargo test` reads what it wrote; Rust never writes the expectations.
//   node scripts/theme-oracle.js           fail if the corpus's expected numbers are not what this computes
//   node scripts/theme-oracle.js --write   write them
'use strict';
const fs = require('fs');
const path = require('path');
const FILE = path.join(__dirname, '..', 'tests', 'theme_corpus.json');
const corpus = JSON.parse(fs.readFileSync(FILE, 'utf8'));

// WCAG 2.x, as the Understanding documents write it: the 0.03928 knee and the 0.2126/0.7152/0.0722 weights.
function channel(v) { const s = v / 255; return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4); }
function lum(hex) { const n = parseInt(hex.slice(1), 16); return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255); }
function ratio(a, b) { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); }
const cut2 = (r) => (Math.floor(r * 100 + 1e-9) / 100).toFixed(2);
module.exports = { lum, ratio, cut2 };

// A case's palette: the base palette with the case's edits applied. Edits that make no colour (`add`, `dup`, `drop`, `rename`) leave
// the colours alone, so only the `set` edits matter to the numbers.
function palette(c) {
  const p = JSON.parse(JSON.stringify(corpus.palette));
  for (const [k, v] of Object.entries(c.set || {})) { const [s, t] = k.split('.'); p[s][t] = v; }
  return p;
}
function fails(p) {
  const out = [];
  for (const s of ['light', 'dark']) {
    for (const [fg, bg] of corpus.pairs) {
      const a = p[s][fg], b = p[s][bg];
      if (!/^#[0-9a-fA-F]{6}$/.test(a) || !/^#[0-9a-fA-F]{6}$/.test(b)) { continue; }
      const r = ratio(a, b);
      if (r < corpus.min_ratio) { out.push(`${s}: ${fg} on ${bg} is ${cut2(r)}:1`); }
    }
  }
  return out;
}
if (require.main === module) {
  const want = { ratios: {}, fails: [] };
  for (const s of ['light', 'dark']) {
    want.ratios[s] = {};
    for (const [fg, bg] of corpus.pairs) { want.ratios[s][`${fg}/${bg}`] = Number(ratio(corpus.palette[s][fg], corpus.palette[s][bg]).toFixed(4)); }
  }
  corpus.cases.forEach((c) => { want.fails.push(c.code === 'theme-contrast' ? fails(palette(c)) : []); });
  // a case that claims a contrast failure fails; a case that claims none, has none
  const problems = [];
  corpus.cases.forEach((c, i) => {
    const f = want.fails[i];
    if (c.code === 'theme-contrast' && f.length === 0) { problems.push(`${c.name}: expected a contrast failure, none computed`); }
    if (c.code === null && fails(palette(c)).length > 0) { problems.push(`${c.name}: expected a pass, ${fails(palette(c)).join('; ')}`); }
  });
  for (const s of ['light', 'dark']) {
    for (const [pair, r] of Object.entries(want.ratios[s])) { if (r < corpus.min_ratio) { problems.push(`the base palette fails: ${s} ${pair} ${r}`); } }
  }
  if (problems.length) { console.log(problems.join('\n')); process.exit(1); }
  if (process.argv.includes('--write')) {
    corpus.ratios = want.ratios;
    corpus.cases.forEach((c, i) => { if (c.code === 'theme-contrast') { c.expect_fails = want.fails[i]; } });
    fs.writeFileSync(FILE, JSON.stringify(corpus, null, 1) + '\n');
    console.log(`wrote ${Object.keys(want.ratios.light).length * 2} ratios and ${corpus.cases.filter((c) => c.code === 'theme-contrast').length} contrast cases`);
  } else {
    const same = JSON.stringify(corpus.ratios) === JSON.stringify(want.ratios)
      && corpus.cases.every((c, i) => c.code !== 'theme-contrast' || JSON.stringify(c.expect_fails) === JSON.stringify(want.fails[i]));
    console.log(same ? 'ok   the corpus has the numbers this computes' : 'FAIL the corpus is not what this computes: run with --write');
    process.exit(same ? 0 : 1);
  }
}
