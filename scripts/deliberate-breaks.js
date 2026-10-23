#!/usr/bin/env node
// Deliberate breaks: does each check of a change fail when the change is spoiled on purpose? A break file (scripts/breaks/*.json)
// lists source edits (`file`, an exact `from` that must occur once, a `to`) and the checks to run (`checks`: name and argv). For
// each break the edit is applied, the compiler is rebuilt, every check is run, and the file is restored byte for byte; a break
// that no check catches is reported and the exit status is 1. A baseline run (no edit) must pass first. Outside the gate: it
// rebuilds and runs the browser checks once per break (minutes).
//   node scripts/deliberate-breaks.js scripts/breaks/view-if.json [only-break-name ...]
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const spec = JSON.parse(fs.readFileSync(path.resolve(process.argv[2]), 'utf8'));
const only = process.argv.slice(3);

// A check that does not end by itself within the time is killed, and that is not a detection: the break is reported as HUNG and counts as
// not caught (a check that hangs on a spoiled program must carry its own watchdog and fail on its own).
const LIMIT_MS = 15 * 60 * 1000;
let hung = [];
function sh(argv) {
  const r = spawnSync(argv[0], argv.slice(1), { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28, timeout: LIMIT_MS, killSignal: 'SIGKILL' });
  if (r.error && r.error.code === 'ETIMEDOUT') { hung.push(argv.join(' ')); }
  return r.status === 0;
}
function runChecks() {
  hung = [];
  if (!sh(['cargo', 'build', '--quiet'])) { return null; }
  return spec.checks.filter((c) => !sh(c.argv)).map((c) => c.name);
}

const baseline = runChecks();
if (baseline === null || baseline.length) {
  console.error(`the baseline is not green: ${baseline === null ? 'build failed' : baseline.join(', ')}`);
  process.exit(2);
}
console.log(`baseline: ${spec.checks.length} checks pass`);
const uncaught = [];
for (const b of spec.breaks) {
  if (only.length && !only.includes(b.name)) { continue; }
  const file = path.join(ROOT, b.file);
  const original = fs.readFileSync(file, 'utf8');
  const at = original.indexOf(b.from);
  if (at < 0 || original.indexOf(b.from, at + 1) >= 0) {
    console.log(`BAD  ${b.name}: \`from\` must occur exactly once in ${b.file}`);
    uncaught.push(b.name);
    continue;
  }
  try {
    fs.writeFileSync(file, original.replace(b.from, () => b.to));
    const failed = runChecks();
    if (hung.length) {
      console.log(`HUNG ${b.name}: ${hung.join(', ')} did not end within ${LIMIT_MS / 60000} minutes and was killed; that is not a detection`);
      uncaught.push(b.name);
    } else if (failed === null) {
      console.log(`BUILD ${b.name}: the spoiled source does not compile, which no check detected; write a break that compiles`);
      uncaught.push(b.name);
    } else if (failed.length) {
      console.log(`ok   ${b.name}: caught by ${failed.join(', ')}`);
    } else {
      console.log(`MISS ${b.name}: every check still passes`);
      uncaught.push(b.name);
    }
  } finally {
    fs.writeFileSync(file, original);
  }
}
runChecks();
console.log(uncaught.length ? `\n${uncaught.length} break(s) not caught: ${uncaught.join(', ')}` : '\nevery break was caught');
process.exit(uncaught.length ? 1 : 0);
