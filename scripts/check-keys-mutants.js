#!/usr/bin/env node
// Mutants of the compiler for the keys a program binds (docs/keys.md, section 4): each must make `cargo test --test keys` or `scripts/check-keys.js` fail. A survivor is explained in the document or the
// checks are extended. Needs node, playwright and cargo.
//   node scripts/check-keys-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const CG = 'crates/cancho-ui-codegen-web/src/keys.rs';
const IR = 'crates/cancho-ui-ir/src/keys.rs';
const MUTANTS = [
  ['the editable-target rule is removed', CG, 'if (t && (t.isContentEditable', 'if (false && (t.isContentEditable'],
  ['preventDefault is removed', CG, '{indent}    e.preventDefault();\\n{indent}    e.stopPropagation();', '{indent}    e.stopPropagation();'],
  ['stopPropagation is removed (the outer container fires too)', CG, '\\n{indent}    e.stopPropagation();', ''],
  ['repeat is allowed for every key', CG, 'if !repeats {', 'if false && !repeats {'],
  ['repeat is refused for arrows', CG, 'let repeats = REPEATING.contains(&combo.key.as_str());', 'let repeats = false;'],
  ['Shift is ignored', CG, 'combo.control,\n        combo.shift\n', 'combo.control,\n        "e.shiftKey"\n'],
  ['Control is ignored', CG, '        combo.control,\n', '        "e.ctrlKey",\n'],
  ['Alt and Meta are ignored', CG, 'if (e.isComposing || e.altKey || e.metaKey) {{ return; }}', 'if (e.isComposing) {{ return; }}'],
  ['an IME composition is ignored', CG, 'if (e.isComposing || e.altKey', 'if (e.altKey'],
  ['a disabled key still fires', CG, 'if let Some(d) = disabled {', 'if let Some(d) = disabled.filter(|_| false) {'],
  ['the container is not made focusable', CG, "{{ {1}.setAttribute('tabindex', '0'); }}", '{{ }}'],
  ['the listener is on the document', CG, "{1}.addEventListener('keydown'", "{1}.ownerDocument.addEventListener('keydown'"],
  ['a letter takes Control', IR, 'if control && !is_arrow_or_edge(rest) {', 'if false && control && !is_arrow_or_edge(rest) {'],
  ['a letter takes Shift', IR, 'if shift && single {', 'if false && shift && single {'],
  ['the container is not required to have a name', IR, 'if has_key && accessibility.label.is_none()', 'if false && has_key && accessibility.label.is_none()'],
  ['a duplicate combination is accepted', IR, 'if seen.contains(&combo) {', 'if false && seen.contains(&combo) {'],
  ['a container that takes keys stays in the prerendered page', 'crates/cancho-ui-ir/src/recipe.rs', 'if children.iter().any(|c| {', 'if false && children.iter().any(|c| {'],
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
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'keys'], { cwd: ROOT, encoding: 'utf8' });
    // a mutant that hangs the page is killed too: the check gets two minutes
    const b = spawnSync('node', [path.join(__dirname, 'check-keys.js')], { cwd: ROOT, encoding: 'utf8', timeout: 120000 });
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
