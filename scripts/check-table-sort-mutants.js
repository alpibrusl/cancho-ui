#!/usr/bin/env node
// Mutants of the compiler for a sortable table header (docs/table-sort.md, section 3): each must make `cargo test --test table_sort` or `scripts/check-table-sort.js`
// fail. A survivor is explained in the document or the checks are extended. Needs node, playwright and cargo.
//   node scripts/check-table-sort-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const SH = 'crates/cancho-ui-ir/src/sortheader.rs';
const VIEW = 'crates/cancho-ui-codegen-web/src/view.rs';
const MUTANTS = [
  ['descending is skipped (ascending goes to unsorted)', SH, '== key {{ \\"-\\" + key }}', '== key {{ \\"\\" }}'],
  ['unsorted is skipped (descending goes to ascending)', SH, '{{ \\"\\" }} else {{ key }}; }}', '{{ key }} else {{ key }}; }}'],
  ['another column starts at descending', SH, 'else {{ key }}; }}', 'else {{ \\"-\\" + key }}; }}'],
  ['ascending and descending are swapped', SH, '{{ \\"ascending\\" }} else if', '{{ \\"descending\\" }} else if'],
  ['the button label carries an arrow', SH, 'label: Some(label),', 'label: Some(format!("{label} ↑")),'],
  ['a header outside a table is accepted', SH, 'if !is_header || !self.in_table {', 'if false && (!is_header || !self.in_table) {'],
  ['a field that is not a String is accepted', SH, '.any(|s| &s.name == f && s.ty == Type::String) =>', '.any(|s| &s.name == f) =>'],
  ['a key may start with a minus', SH, "&& !k.starts_with('-')", ''],
  ['a duplicate key is accepted', SH, 'if self.keys.contains(&k) {', 'if false && self.keys.contains(&k) {'],
  ['a label with an interpolation is accepted', SH, '[TextPart::Literal(s)] if !s.is_empty() => Some(s.clone()),', '[TextPart::Literal(s)] if !s.is_empty() => Some(s.clone()),\n            [_, ..] => Some(String::from("x")),'],
  ['aria-sort is written on every header', VIEW, "if (__s !== '') {{ {1}.setAttribute('aria-sort', __s); }}", "{1}.setAttribute('aria-sort', __s);"],
  ['a second key of one table is not tracked (keys reset per header)', SH, 'self.keys.push(k.clone());', 'let _ = k.clone();'],
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
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'table_sort'], { cwd: ROOT, encoding: 'utf8' });
    const b = spawnSync('node', [path.join(__dirname, 'check-table-sort.js')], { cwd: ROOT, encoding: 'utf8' });
    const by = [t.status !== 0 ? 'rust tests' : '', b.status !== 0 ? 'browser' : ''].filter(Boolean).join(' + ');
    console.log(`${by ? 'killed  ' : 'SURVIVED'} ${name}${by ? `  [${by}]` : ''}`);
    if (!by) { survivors += 1; }
  } finally {
    fs.writeFileSync(full, original);
  }
}
spawnSync('cargo', ['build', '--release'], { cwd: ROOT });
console.log(`${ran - survivors}/${ran} killed`);
process.exit(survivors ? 1 : 0);
