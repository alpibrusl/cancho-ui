#!/usr/bin/env node
// What docs/nesting.md section 2 says, measured: for each construct, the smallest nesting at which `cancho-ui check`, `build` and `test` abort
// with a stack overflow (a signal, exit 134) instead of answering with a diagnostic, on the 8 MiB main stack and on a 2 MiB one (the default of a
// thread a library user spawns), in the debug and the release binary. Not a check of the compiler. Outside the gate.
//   node scripts/measure-nesting.js [--fast]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

setTimeout(() => { console.error('FAIL the measurement took more than 25 minutes'); process.exit(1); }, 25 * 60 * 1000).unref();
const ROOT = path.join(__dirname, '..');
// BIN_DEBUG and BIN_RELEASE name the binaries to measure (the compiler before the fix is built from main in another directory)
const BIN = { debug: process.env.BIN_DEBUG || path.join(ROOT, 'target', 'debug', 'cancho-ui'), release: process.env.BIN_RELEASE || path.join(ROOT, 'target', 'release', 'cancho-ui') };
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'nesting-'));

const rep = (s, n) => s.repeat(n);
const handler = (stmt) => `component T {\n    state a: Int = 0;\n    state r: Result[String, String] = Ok("");\n    state l: List[String] = [];\n    on Go { ${stmt} }\n    view { Column { Text { "{a}" } } }\n}\n`;
const viewOf = (v) => `component T {\n    state a: Int = 0;\n    view { ${v} }\n}\n`;
const CONSTRUCTS = {
  paren: (n) => handler(`a = ${rep('(', n)}1${rep(')', n)};`),
  unary: (n) => handler(`a = ${rep('- ', n)}1;`),
  'add-chain': (n) => handler(`a = 1${rep(' + 1', n)};`),
  'and-chain': (n) => handler(`a = 1${rep(' + 1', 0)}; if true${rep(' && true', n)} { a = 2; }`),
  call: (n) => handler(`a = ${rep('max(', n)}1${rep(', 1)', n)};`),
  'list-literal': (n) => handler(`l = ${rep('[', n)}${rep(']', n)};`),
  'match-arm': (n) => handler(`a = ${rep('match r { Ok(_) => { ', n)}1${rep(' } Err(_) => { 2 } Cancelled => { 3 } }', n)};`),
  'if-else-chain': (n) => handler(`a = ${rep('if true { 1 } else ', n)}{ 2 };`),
  lambda: (n) => handler(`l = ${rep('filter(l, x => ', n)}true${rep(')', n)};`),
  type: (n) => `component T {\n    state t: ${rep('List[', n)}String${rep(']', n)} = [];\n    view { Column { Text { "x" } } }\n}\n`,
  interpolation: (n) => viewOf(`Text { "{${rep('(', n)}a${rep(')', n)}}" }`),
  'view-column': (n) => viewOf(`${rep('Column { ', n)}Text { "x" }${rep(' }', n)}`),
  'view-if': (n) => viewOf(`${rep('If { when: true ', n)}Text { "x" }${rep(' }', n)}`),
  'view-for': (n) => viewOf(`${rep('For { over: [1] as: i ', n)}Text { "x" }${rep(' }', n)}`),
};
const TRACE = JSON.stringify({ schema: 'cancho-ui-trace', version: '0.1.0', steps: [{ event: 'Go', state: { a: 0 } }] });
fs.writeFileSync(path.join(dir, 't.trace.json'), TRACE);

function run(mode, bin, stackKb, src) {
  const f = path.join(dir, 't.cui');
  fs.writeFileSync(f, src);
  const args = mode === 'check' ? ['check', f] : mode === 'build' ? ['build', f, '--page', '-o', path.join(dir, 'out')] : ['test', f, path.join(dir, 't.trace.json')];
  const r = spawnSync('sh', ['-c', `ulimit -s ${stackKb}; exec "$0" "$@"`, bin, ...args], { encoding: 'utf8', timeout: 30000, killSignal: 'SIGKILL', maxBuffer: 1 << 26 });
  // a run that did not end in 30 s was killed by this script: it is slow, it is not a stack overflow
  const slow = !!(r.error && r.error.code === 'ETIMEDOUT');
  const aborted = !slow && (r.signal !== null || r.status === 134 || r.status === 139 || /overflowed its stack/.test(r.stderr || ''));
  return { aborted, slow, status: r.status, signal: r.signal };
}
// the smallest n in [1, MAX] that aborts (null when none does, 'slow' when the run at MAX did not end in 30 s); the abort is monotone in n
function firstAbort(mode, bin, kb, make, MAX) {
  const top = run(mode, bin, kb, make(MAX));
  if (top.slow) { return 'slow'; }
  if (!top.aborted) { return null; }
  let lo = 1; let hi = MAX; // hi aborts
  while (lo < hi) { const mid = (lo + hi) >> 1; if (run(mode, bin, kb, make(mid)).aborted) { hi = mid; } else { lo = mid + 1; } }
  return lo;
}
const fast = process.argv.includes('--fast');
const only = (process.argv.find((a) => a.startsWith('--only=')) || '').slice(7).split(',').filter(Boolean);
const MAX = 200000;
const rows = [];
for (const [name, make] of Object.entries(CONSTRUCTS)) {
  if (only.length && !only.includes(name)) { continue; }
  const row = { construct: name };
  for (const [kind, bin] of Object.entries(BIN)) {
    if (fast && kind === 'release') { continue; }
    for (const kb of [8192, 2048]) {
      for (const mode of ['check', 'build', 'test']) {
        row[`${kind}/${kb / 1024}MiB/${mode}`] = firstAbort(mode, bin, kb, make, MAX);
      }
    }
  }
  console.log(JSON.stringify(row));
  rows.push(row);
}
process.exit(0);
