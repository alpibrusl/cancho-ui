#!/usr/bin/env node
// Mutants of the compiler for static table features (docs/table-static.md, section 5): each must make `cargo test --test table_static` or
// `scripts/check-table-static.js` fail. A survivor is explained in the document or the checks are extended. Needs node, playwright and cargo.
//   node scripts/check-table-static-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const MUTANTS = [
  ['the header class is not emitted', 'crates/cancho-ui-ir/src/recipe.rs', 'class.push_str(" cui-sticky-head");', 'class.push_str("");'],
  ['the column class is not emitted', 'crates/cancho-ui-ir/src/recipe.rs', 'class.push_str(" cui-sticky-col");', 'class.push_str("");'],
  ['the height class is not emitted', 'crates/cancho-ui-ir/src/recipe.rs', 'class.push_str(" cui-has-height");', 'class.push_str("");'],
  ['the height is ignored', 'crates/cancho-ui-ir/src/recipe.rs', 'format!("max-height: {n}rem;")', 'format!("max-height: {}rem;", n * 3)'],
  ['the header sticks one pixel down', 'crates/cancho-ui/src/pwa.rs', 'position: sticky; top: 0; background: Canvas; z-index: 1; }', 'position: sticky; top: 1px; background: Canvas; z-index: 1; }'],
  ['the header is transparent', 'crates/cancho-ui/src/pwa.rs', 'position: sticky; top: 0; background: Canvas; z-index: 1; }', 'position: sticky; top: 0; background: transparent; z-index: 1; }'],
  ['the column sticks one pixel in', 'crates/cancho-ui/src/pwa.rs', 'position: sticky; left: 0; background: Canvas;', 'position: sticky; left: 1px; background: Canvas;'],
  ['the corner is not raised', 'crates/cancho-ui/src/pwa.rs', '.cui-sticky-head.cui-sticky-col th:first-child { z-index: 2; }', '.cui-sticky-head.cui-sticky-col th:first-child { z-index: 1; }'],
  ['numeric is not aligned', 'crates/cancho-ui/src/pwa.rs', 'text-align: end; font-variant-numeric: tabular-nums;', 'text-align: start; font-variant-numeric: tabular-nums;'],
  ['numeric is not tabular', 'crates/cancho-ui/src/pwa.rs', 'text-align: end; font-variant-numeric: tabular-nums;', 'text-align: end;'],
  ['numeric emits no class', 'crates/cancho-ui-ir/src/structure.rs', 'return Some("cui-num".to_string());', 'return None;'],
  ['a sticky header without a height is accepted', 'crates/cancho-ui-ir/src/structure.rs', 'if head && max.is_none() {', 'if false && head && max.is_none() {'],
  ['numeric outside a table is accepted', 'crates/cancho-ui-ir/src/structure.rs', 'if matches!(kind, IrElementKind::Text) && bindings.iter().any(|b| b.key == "numeric") {', 'if false && matches!(kind, IrElementKind::Text) && bindings.iter().any(|b| b.key == "numeric") {'],
  ['the height range is not checked', 'crates/cancho-ui-ir/src/structure.rs', 'Some(n) if (CODE_HEIGHT.0..=CODE_HEIGHT.1).contains(&n) => Ok(()),\n        _ => Err(refuse(\n            "table-max-height-invalid"', 'Some(_) => Ok(()),\n        _ => Err(refuse(\n            "table-max-height-invalid"'],
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
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'table_static'], { cwd: ROOT, encoding: 'utf8' });
    const b = spawnSync('node', [path.join(__dirname, 'check-table-static.js')], { cwd: ROOT, encoding: 'utf8' });
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
