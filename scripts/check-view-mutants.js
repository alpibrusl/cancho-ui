#!/usr/bin/env node
// Mutants of the compiler for the outline of the view (docs/view-test.md, section 4): each must make `cargo test --test view_test` or `scripts/check-view.js` fail. A survivor is explained in the document or
// the checks are extended. Needs node, playwright and cargo.
//   node scripts/check-view-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const O = 'crates/cancho-ui-replay/src/outline.rs';
const L = 'crates/cancho-ui-replay/src/lib.rs';
const MUTANTS = [
  ['an If ignores its condition', O, 'if !matches!(self.eval(cond)?, Value::Bool(false)) {\n                    self.nodes(children, depth)?;', 'if true {\n                    self.nodes(children, depth)?;'],
  ['a For draws one element too few', O, 'Value::List(items) => Ok(items),', 'Value::List(items) => Ok(items.into_iter().skip(1).collect()),'],
  ['a For draws every element twice', O, 'Value::List(items) => Ok(items),', 'Value::List(items) => Ok(items.iter().cloned().chain(items.iter().cloned()).collect()),'],
  ['a button has no name', O, 'self.line(depth, "button", Some(name), props);', 'self.line(depth, "button", Some(String::new()), props);'],
  ['sort= is missing', O, 'props.push(format!("sort={s}"));', 'let _ = s;'],
  ['numeric is missing', O, 'props.push("numeric".to_string());', 'let _ = 0;'],
  ['pressed says disabled', O, 'props.push("pressed".to_string());', 'props.push("disabled".to_string());'],
  ['a text field has no value', O, 'vec![format!("value={}", quote(&value))]', 'vec![format!("value={}", quote(""))]'],
  ['the rows of a window leave the header out', O, 'props.push(format!("rows={}", total + header_rows));', 'props.push(format!("rows={}", total));'],
  ['a role given to a layout box is dropped', O, 'self.line(depth, r, name, vec![]);', 'let _ = (r, name);'],
  ['view_has ignores the order', L, 'trimmed[at..].iter()', 'trimmed[..].iter()'],
  ['view ignores the last line', L, 'if want.as_slice() != lines {', 'if want.len() != lines.len() || (want.len() > 1 && want[..want.len() - 1] != lines[..lines.len() - 1]) {'],
  ['a node the outline cannot say is skipped, not refused', L, 'if asserts && let Some(l)', 'if false && asserts && let Some(l)'],
  ['an odd indentation is a good line', 'crates/cancho-ui-replay/src/trace.rs', 'if !indent.is_multiple_of(2) {', 'if false {'],
  ['any word is a property', 'crates/cancho-ui-replay/src/trace.rs', 'if !known {', 'if false && !known {'],
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
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'view_test'], { cwd: ROOT, encoding: 'utf8' });
    // the browser check uses the binary the tests just built; a mutant that hangs it is killed too
    const b = spawnSync('node', [path.join(__dirname, 'check-view.js')], { cwd: ROOT, encoding: 'utf8', timeout: 300000 });
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
