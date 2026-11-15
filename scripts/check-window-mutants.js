#!/usr/bin/env node
// Mutants of the compiler for a table that draws only the rows in view (docs/window.md, section 4): each must make `cargo test --test window` or `scripts/check-window.js` fail. A survivor is explained
// in the document or the checks are extended. Needs node, playwright and cargo.
//   node scripts/check-window-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const V = 'crates/cancho-ui-codegen-web/src/view.rs';
const MUTANTS = [
  ['the top spacer is a row too short', V, 'insertBefore(spacer(wf),', 'insertBefore(spacer(wf - 1),'],
  ['the bottom spacer is missing', V, 'appendChild(spacer(Math.max(0, wt - wf - (rows.length - h))));', 'appendChild(spacer(0));'],
  ['the index is rounded up', V, 'Math.floor({scroll}.scrollTop / px)', 'Math.ceil({scroll}.scrollTop / px)'],
  ['the event is fired for every scroll', V, 'if (at !== wf) {{', 'if (true) {{'],
  ['first is not clamped', V, 'const wf = Math.min(Math.max(0, Math.floor(Number({first}) || 0)), Math.max(0, wt - 1));', 'const wf = Math.max(0, Math.floor(Number({first}) || 0));'],
  ['the row number is off by one', V, '(k - h) + 1)); }});', '(k - h) + 2)); }});'],
  ['the header is not counted in aria-rowcount', V, "String(wt + h)", "String(wt)"],
  ['the spacers are not hidden from assistive technology', V, "tr.setAttribute('aria-hidden', 'true');", "tr.setAttribute('data-spacer', 'true');"],
  ['the scroll is not put back after a redraw', 'crates/cancho-ui-codegen-web/src/emit.rs', 'if (__cui_win[i] !== undefined)', 'if (false)'],
  ['the row height is not applied', 'crates/cancho-ui/src/pwa.rs', 'td { height: var(--cui-row); box-sizing', 'td { box-sizing'],
  ['the row height is not checked', 'crates/cancho-ui-ir/src/window.rs', 'Some(n) if (ROW_HEIGHT.0..=ROW_HEIGHT.1).contains(&n) => {}', 'Some(_) => {}'],
  ['the five bindings are not required together', 'crates/cancho-ui-ir/src/window.rs', 'if !has(need) {', 'if false && !has(need) {'],
  ['a count that is not a number is accepted', 'crates/cancho-ui-ir/src/conditions.rs', 'code: "window-count-not-number",', 'code: "window-count-fine",'],
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
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'window'], { cwd: ROOT, encoding: 'utf8' });
    // a mutant that hangs the page is killed too: the check gets four minutes
    const b = spawnSync('node', [path.join(__dirname, 'check-window.js')], { cwd: ROOT, encoding: 'utf8', timeout: 240000 });
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
