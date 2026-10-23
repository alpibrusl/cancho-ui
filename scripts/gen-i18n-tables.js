#!/usr/bin/env node
// Writes crates/cancho-ui-ir/src/i18n_data.rs (docs/i18n.md section 3.3): what Chromium's `Intl` says for the languages the compiler knows, read
// off with formatToParts and probes, as data. The compiler, the page and replay use the data and never call `Intl`; the check
// (scripts/check-i18n.js) compares what they make with `Intl` again, for the input they were not built from.
//   node scripts/gen-i18n-tables.js            writes the file
//   node scripts/gen-i18n-tables.js --check    fails if the file is not what the script writes now (it is, for the Chromium that wrote it)
'use strict';
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');
setTimeout(() => { console.error('FAIL the generator took more than 2 minutes'); process.exit(1); }, 120000).unref();

const LANGS = ['en', 'es', 'de', 'fr', 'pt', 'ru', 'ar', 'he', 'ja', 'hi'];
const RTL = ['ar', 'he'];
const OUT = path.join(__dirname, '..', 'crates', 'cancho-ui-ir', 'src', 'i18n_data.rs');

// runs in the page. Chromium's format() writes U+0020 where formatToParts() has U+202F in the literals of a date or time (a compatibility rule of V8: text written for
// the old CLDR broke on the new space). The table follows format(), which is what a page that calls Intl gets and what the corpus holds.
function probe(LANGS) {
  const data = {};
  const U = (y, m, d, h = 0, mi = 0, s = 0) => Date.UTC(y, m, d, h, mi, s);
  for (const l of LANGS) {
    const nf = (o, v) => new Intl.NumberFormat(l, o).format(v);
    const d = {};
    // numbers: separators from a number that has both
    const parts = new Intl.NumberFormat(l, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).formatToParts(-12345.5);
    d.decimal = parts.find((p) => p.type === 'decimal').value;
    d.group = parts.find((p) => p.type === 'group').value;
    // everything written before the digits of a negative number: the sign, and the direction mark some languages put before it
    d.minus = parts.slice(0, parts.findIndex((p) => p.type === 'integer')).map((p) => p.value).join('');
    // grouping: 1000 grouped? 10000? 1234567 shape
    const g = (v) => new Intl.NumberFormat(l, { useGrouping: 'auto' }).format(v);
    d.minGroup = g(1000).includes(d.group) ? 1 : 2;
    d.indian = g(12345678).split(d.group).map((s) => s.length).join(',') === '1,2,2,3';
    // percent: the text around the number
    const pc = nf({ style: 'percent' }, 0.26);
    const at = pc.indexOf('26');
    d.pctPrefix = pc.slice(0, at);
    d.pctSuffix = pc.slice(at + 2);
    // dates
    d.date = {};
    for (const style of ['short', 'medium', 'long', 'full']) {
      const f = new Intl.DateTimeFormat(l, { dateStyle: style, timeZone: 'UTC' });
      const sample = f.formatToParts(U(2026, 9, 24));
      const single = f.formatToParts(U(2026, 0, 5));
      const pat = sample.map((p, i) => {
        const q = single[i];
        if (p.type === 'literal') { return ['l', p.value.replace(/\u202f/g, ' ')]; }
        if (p.type === 'day') { return ['d', q.value.length === 2 ? 1 : 0]; }
        if (p.type === 'year') { return ['y', p.value.length === 2 ? 1 : 0]; }
        if (p.type === 'month') {
          if (/^\d+$/.test(p.value)) { return ['m', q.value.length === 2 ? 1 : 0]; }
          const names = [];
          for (let m = 0; m < 12; m++) { names.push(f.formatToParts(U(2026, m, 15)).find((x) => x.type === 'month').value); }
          return ['M', names];
        }
        if (p.type === 'weekday') {
          const names = [];
          for (let w = 0; w < 7; w++) { names.push(f.formatToParts(U(2026, 0, 4 + w)).find((x) => x.type === 'weekday').value); }
          return ['w', names];
        }
        throw new Error(`${l} ${style}: a part of type ${p.type}`);
      });
      if (sample.length !== single.length) { throw new Error(`${l} ${style}: the parts change with the date`); }
      d.date[style] = pat;
    }
    // time (short)
    const tf = new Intl.DateTimeFormat(l, { timeStyle: 'short', timeZone: 'UTC' });
    const t15 = tf.formatToParts(U(2026, 9, 24, 15, 5));
    const t09 = tf.formatToParts(U(2026, 9, 24, 9, 5));
    d.time = t15.map((p, i) => {
      if (p.type === 'literal') { return ['l', p.value.replace(/\u202f/g, ' ')]; }
      if (p.type === 'hour') { return ['h', t09[i].value.length === 2 ? 1 : 0, t15.some((x) => x.type === 'dayPeriod') ? 1 : 0]; }
      if (p.type === 'minute') { return ['n']; }
      if (p.type === 'dayPeriod') { return ['p', t09.find((x) => x.type === 'dayPeriod').value, p.value]; }
      throw new Error(`${l} time: a part of type ${p.type}`);
    });
    data[l] = d;
  }
  return data;
}

const q = (s) => JSON.stringify(s).replace(/[\u0080-￿]/g, (c) => `\\u{${c.charCodeAt(0).toString(16)}}`).replace(/\\u([0-9a-f]{4})/g, '\\u{$1}');
const rs = (s) => `"${[...s].map((c) => {
  const cp = c.codePointAt(0);
  if (c === '"' || c === '\\') { return `\\${c}`; }
  return cp < 0x7f && cp >= 0x20 ? c : `\\u{${cp.toString(16)}}`;
}).join('')}"`;
function partRs(p) {
  const [k, a, b] = p;
  if (k === 'l') { return `Part::Lit(${rs(a)})`; }
  if (k === 'd') { return `Part::Day(${a === 1})`; }
  if (k === 'm') { return `Part::MonthNum(${a === 1})`; }
  if (k === 'y') { return `Part::Year(${a === 1})`; }
  if (k === 'M') { return `Part::MonthName([${a.map(rs).join(', ')}])`; }
  if (k === 'w') { return `Part::Weekday([${a.map(rs).join(', ')}])`; }
  if (k === 'h') { return `Part::Hour(${a === 1}, ${b === 1})`; }
  if (k === 'n') { return 'Part::Minute'; }
  if (k === 'p') { return `Part::DayPeriod(${rs(a)}, ${rs(b)})`; }
  throw new Error(k);
}

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  await page.goto('about:blank');
  const data = await page.evaluate(`(${probe.toString()})(${JSON.stringify(LANGS)})`);
  const version = browser.version();
  await browser.close();
  let out = `//! GENERATED by scripts/gen-i18n-tables.js from Chromium ${version}'s Intl (docs/i18n.md section 3.3). Do not edit by hand.\n`;
  out += '//! What the page and replay know of ten languages: separators, grouping, the text around a percent, the patterns of dates and times and\n//! the names in them. Neither calls Intl; scripts/check-i18n.js compares what they make with Intl for input they were not built from.\n\n';
  out += 'use crate::i18n::{Lang, Part};\n\n';
  out += `pub const CHROMIUM: &str = ${rs(version)};\n\n`;
  out += 'pub static LANGS: &[Lang] = &[\n';
  for (const l of LANGS) {
    const d = data[l];
    out += '    Lang {\n';
    out += `        code: ${rs(l)},\n        rtl: ${RTL.includes(l)},\n        decimal: ${rs(d.decimal)},\n        group: ${rs(d.group)},\n        minus: ${rs(d.minus)},\n        min_group: ${d.minGroup},\n        indian: ${d.indian},\n        pct_prefix: ${rs(d.pctPrefix)},\n        pct_suffix: ${rs(d.pctSuffix)},\n`;
    for (const s of ['short', 'medium', 'long', 'full']) {
      out += `        date_${s}: &[${d.date[s].map(partRs).join(', ')}],\n`;
    }
    out += `        time: &[${d.time.map(partRs).join(', ')}],\n`;
    out += '    },\n';
  }
  out += '];\n';
  if (process.argv.includes('--check')) {
    const have = fs.readFileSync(OUT, 'utf8');
    if (have !== out) { console.error('FAIL crates/cancho-ui-ir/src/i18n_data.rs is not what this script writes now'); process.exit(1); }
    console.log('ok   the table is what the script writes');
    return;
  }
  fs.writeFileSync(OUT, out);
  console.log(`wrote ${OUT} (${out.length} bytes, Chromium ${version})`);
})().catch((e) => { console.error(e); process.exit(1); });
