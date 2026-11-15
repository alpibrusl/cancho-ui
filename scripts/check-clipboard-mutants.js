#!/usr/bin/env node
// Mutants of the compiler for `Button { copy: field }` (docs/clipboard.md, section 5): each must make `cargo test --test clipboard` or
// `scripts/check-clipboard.js` fail. A survivor is explained in the document or the checks are extended. Needs node, playwright and cargo.
//   node scripts/check-clipboard-mutants.js [--only SUBSTR]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const MUTANTS = [
  ['a copy button is not a use of Clipboard', 'crates/cancho-ui-ir/src/lower.rs', '|| crate::structure::view_has_button_copy(&c.view.body)))', '|| false))'],
  ['the declaration is not checked', 'crates/cancho-ui-ir/src/controls.rs', 'if !c.requires.iter().any(|r| r.name == "Clipboard") {\n        return Err(LowerError::ControlRefused {\n            code: "clipboard-not-declared",\n            message: "`Button { copy', 'if false {\n        return Err(LowerError::ControlRefused {\n            code: "clipboard-not-declared",\n            message: "`Button { copy'],
  ['the field is not checked for its type', 'crates/cancho-ui-ir/src/controls.rs', '.any(|f| &f.name == name && f.ty == IrType::String);\n            if !is_string {', '.any(|f| &f.name == name);\n            if !is_string {'],
  ['the adapter is emitted for every program', 'crates/cancho-ui-codegen-web/src/emit.rs', '.any(|c| cancho_ui_ir::structure::view_has_button_copy(&c.view.body))', '.any(|_c| true)'],
  ['the write is at render, not at the tap', 'crates/cancho-ui-codegen-web/src/view.rs', "{0}{1}.addEventListener('click', function () {{ __cui_copy_text({1}, {2}{3}); }});", "{0}__cui_copy_text({1}, {2}{3});"],
  ['the limit is one unit too high', 'crates/cancho-ui-codegen-web/src/adapters.rs', 'if (value.length > 1000000)', 'if (value.length > 1000001)'],
  ['the limit is one unit too low', 'crates/cancho-ui-codegen-web/src/adapters.rs', 'if (value.length > 1000000)', 'if (value.length >= 1000000)'],
  ['the adapter reads the clipboard', 'crates/cancho-ui-codegen-web/src/adapters.rs', "const value = String(text);", "const value = String(text); if (navigator.clipboard && navigator.clipboard.readText) { navigator.clipboard.readText(); }"],
  ['a rejected write says Copied', 'crates/cancho-ui-codegen-web/src/adapters.rs', "function () { say('Not copied'); });\n    } else {", "function () { say('Copied'); });\n    } else {"],
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
    const t = spawnSync('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'clipboard'], { cwd: ROOT, encoding: 'utf8' });
    const b = spawnSync('node', [path.join(__dirname, 'check-clipboard.js')], { cwd: ROOT, encoding: 'utf8' });
    const by = [t.status !== 0 ? 'rust tests' : '', b.status !== 0 ? 'browser' : ''].filter(Boolean).join(' + ');
    console.log(`${by ? 'killed  ' : 'SURVIVED'} ${name}${by ? `  [${by}]` : ''}`);
    if (!by) { survivors += 1; }
  } finally {
    fs.writeFileSync(full, original);
  }
}
spawnSync('cargo', ['build', '--release'], { cwd: ROOT });
console.log(`${ran - (survivors)}/${ran} killed`);
process.exit(survivors ? 1 : 0);
