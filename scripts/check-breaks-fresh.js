#!/usr/bin/env node
// Every deliberate break (scripts/breaks/*.json) names an exact `from` that must occur exactly once in its `file`, or the harness reports it BAD and it
// catches nothing. The breaks are written against a source that moves; this check fails the day one of them stops applying, instead of the day
// someone runs the whole harness (32 had gone stale unseen, docs/www-port/gaps.md section 8). It runs no break (that takes minutes); it also checks that
// `to` differs from `from`, that a break name is unique in its file, and that an `equivalent` entry (a mutation measured to change nothing) says why.
//   node scripts/check-breaks-fresh.js
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..');
const dir = path.join(ROOT, 'scripts', 'breaks');
const bad = [];
let n = 0;
let files = 0;
for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.json')).sort()) {
  files++;
  const spec = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
  const names = new Set();
  if (!Array.isArray(spec.checks) || spec.checks.length === 0) { bad.push(`${f}: no checks`); }
  for (const b of spec.breaks) {
    n++;
    if (names.has(b.name)) { bad.push(`${f}: "${b.name}" twice`); }
    names.add(b.name);
    let text;
    try { text = fs.readFileSync(path.join(ROOT, b.file), 'utf8'); } catch (e) { bad.push(`${f}: "${b.name}": ${b.file} is not there`); continue; }
    const count = text.split(b.from).length - 1;
    if (count !== 1) { bad.push(`${f}: "${b.name}": \`from\` occurs ${count} times in ${b.file}`); }
    if (b.from === b.to) { bad.push(`${f}: "${b.name}": from and to are the same`); }
  }
  for (const e of spec.equivalent || []) {
    if (!e.why || e.why.length < 40) { bad.push(`${f}: equivalent "${e.name}" does not say why`); }
  }
}
for (const m of bad) { console.log(`FAIL ${m}`); }
console.log(bad.length ? `\n${bad.length} stale or malformed break(s)` : `ok   ${n} breaks in ${files} files: every \`from\` occurs once in its file`);
process.exit(bad.length ? 1 : 0);
