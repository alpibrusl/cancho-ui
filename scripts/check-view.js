#!/usr/bin/env node
// Browser-side check that the outline of the view (docs/view-test.md, section 3) is what the page draws. For each example, the outline in the DOM is read with a function that applies the table of
// the document to the page, and compared, line by line, with the outline `cancho-ui test --view` gives for the trace of the same events, after each real interaction (a click, a typing, a key, a scroll).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. Chromium only.
//   node scripts/check-view.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'release', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };

const failures = [];
let compared = 0;
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail !== undefined && !ok ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

// The table of docs/view-test.md applied to the document: a tag or an attribute is a line. Runs in the page.
function domOutline() {
  const out = [];
  const norm = (s) => s.split(/\s+/).filter(Boolean).join(' ');
  const q = (s) => JSON.stringify(s);
  const line = (d, role, name, props) => out.push('  '.repeat(d) + role + (name === null ? '' : ' ' + q(name)) + (props.length ? ' ' + props.join(' ') : ''));
  const kids = (el, d) => { for (const c of el.children) { walk(c, d); } };
  const walk = (el, d) => {
    if (el.getAttribute('aria-hidden') === 'true' || /^(SCRIPT|STYLE|TEMPLATE)$/.test(el.tagName)) { return; }
    const tag = el.tagName;
    const role = el.getAttribute('role');
    if (el.classList.contains('cui-table-scroll')) { kids(el, d); return; }
    if (tag === 'TABLE') {
      const scroll = el.closest('.cui-table-scroll');
      const name = el.getAttribute('aria-label') || (scroll && scroll.getAttribute('aria-label')) || null;
      const props = el.hasAttribute('aria-rowcount') ? [`rows=${el.getAttribute('aria-rowcount')}`] : [];
      line(d, 'table', name, props);
      kids(el, d + 1);
      return;
    }
    if (tag === 'TBODY' || tag === 'THEAD') { kids(el, d); return; }
    if (tag === 'TR') { line(d, 'row', null, []); kids(el, d + 1); return; }
    if (tag === 'TH' || tag === 'TD') {
      const props = [];
      if (tag === 'TH' && el.getAttribute('aria-sort')) { props.push(`sort=${el.getAttribute('aria-sort')}`); }
      if (el.classList.contains('cui-num')) { props.push('numeric'); }
      line(d, tag === 'TH' ? 'columnheader' : 'cell', norm(el.textContent), props);
      for (const c of el.children) { if (/^(BUTTON|INPUT|TEXTAREA|SELECT|A)$/.test(c.tagName)) { walk(c, d + 1); } }
      return;
    }
    if (tag === 'BUTTON') {
      const props = [];
      if (el.getAttribute('aria-pressed') === 'true') { props.push('pressed'); }
      if (el.disabled) { props.push('disabled'); }
      line(d, 'button', norm(el.getAttribute('aria-label') || el.textContent), props);
      return;
    }
    if (tag === 'INPUT' && el.type === 'checkbox') {
      const props = [];
      if (el.checked) { props.push('checked'); }
      if (el.disabled) { props.push('disabled'); }
      line(d, 'checkbox', el.getAttribute('aria-label'), props);
      return;
    }
    if (tag === 'INPUT' || tag === 'TEXTAREA') {
      const props = [`value=${q(el.value)}`];
      if (el.disabled) { props.push('disabled'); }
      if (el.getAttribute('aria-invalid') === 'true') { props.push('invalid'); }
      line(d, 'textbox', el.getAttribute('aria-label'), props);
      return;
    }
    if (/^H[1-6]$/.test(tag)) { line(d, 'heading', norm(el.textContent), [`level=${tag[1]}`]); return; }
    if (tag === 'UL') { line(d, 'list', el.getAttribute('aria-label'), []); kids(el, d + 1); return; }
    if (tag === 'LI') { line(d, 'listitem', norm(el.textContent), []); return; }
    if (tag === 'SPAN' || tag === 'P') {
      if (role === 'heading') { line(d, 'heading', norm(el.textContent), [`level=${el.getAttribute('aria-level') || 2}`]); return; }
      line(d, role === 'status' || role === 'alert' ? role : 'text', norm(el.textContent), []);
      return;
    }
    // a layout box has no line of its own; one that was given a role is that role, with its name
    if (role && role !== 'presentation' && role !== 'none') {
      line(d, role, el.getAttribute('aria-label'), []);
      kids(el, d + 1);
      return;
    }
    kids(el, d);
  };
  const root = document.getElementById('root');
  for (const c of root.children) { walk(c, 0); }
  return out;
}

const CASES = [
  {
    name: 'counter',
    file: 'examples/counter/counter.cui',
    steps: [
      { event: 'Increment', act: async (p) => p.getByRole('button', { name: 'Increment' }).click() },
      { event: 'Increment', act: async (p) => p.getByRole('button', { name: 'Increment' }).click() },
      { event: 'Reset', act: async (p) => p.getByRole('button', { name: 'Reset' }).click() },
    ],
  },
  {
    name: 'rows',
    file: 'examples/rows/rows.cui',
    steps: [
      { event: 'Pick', params: ['M-102', 2], act: async (p) => p.getByRole('button', { name: 'Select M-102' }).click() },
      { event: 'Reverse', act: async (p) => p.getByRole('button', { name: 'Reverse' }).click() },
      { event: 'Pick', params: ['M-103', 2], act: async (p) => p.getByRole('button', { name: 'Select M-103' }).click() },
    ],
  },
  {
    name: 'table-sort',
    file: 'examples/table-sort/table-sort.cui',
    steps: [
      { event: '__sort_order', params: ['name'], act: async (p) => p.locator('th:nth-child(1) button').click() },
      { event: '__sort_order', params: ['name'], act: async (p) => p.locator('th:nth-child(1) button').click() },
      { event: '__sort_order', params: ['length'], act: async (p) => p.locator('th:nth-child(2) button').click() },
      { event: '__sort_order', params: ['length'], act: async (p) => p.locator('th:nth-child(2) button').click() },
      { event: '__sort_order', params: ['length'], act: async (p) => p.locator('th:nth-child(2) button').click() },
    ],
  },
  {
    name: 'keys',
    file: 'examples/keys/keys.cui',
    steps: [
      { event: 'SetQuery', params: ['ab'], act: async (p) => { await p.locator('input').click(); await p.locator('input').pressSequentially('ab'); }, trace: [{ event: 'SetQuery', params: ['a'] }, { event: 'SetQuery', params: ['ab'] }] },
      { event: 'Move', params: [1], act: async (p) => { await p.locator('[aria-label="Rows"]').focus(); await p.keyboard.press('ArrowDown'); } },
      { event: 'Extend', params: [1], act: async (p) => p.keyboard.press('Shift+ArrowDown') },
      { event: 'Disable', act: async (p) => p.getByRole('button', { name: 'Disable x' }).click() },
    ],
  },
  {
    name: 'window',
    file: 'examples/window/window.cui',
    steps: [
      { event: 'Scrolled', params: [10], act: async (p) => p.evaluate(() => { document.querySelector('.cui-table-scroll').scrollTop = 10 * 32; }) },
      { event: 'Scrolled', params: [44], act: async (p) => p.evaluate(() => { document.querySelector('.cui-table-scroll').scrollTop = 44 * 32; }) },
    ],
  },
  { name: 'table-static', file: 'examples/table-static/table-static.cui', steps: [] },
];

// every other example whose outline has no node it cannot say, and that does nothing before the first event (no `start`, `every`, `subscribe`, `route`): compared at rest only
function atRest() {
  const have = new Set(CASES.map((c) => c.file));
  const found = [];
  const skipped = [];
  for (const d of fs.readdirSync(path.join(ROOT, 'examples')).sort()) {
    if (!fs.statSync(path.join(ROOT, 'examples', d)).isDirectory()) { continue; }
    for (const f of fs.readdirSync(path.join(ROOT, 'examples', d)).sort()) {
      if (!f.endsWith('.cui')) { continue; }
      const rel = `examples/${d}/${f}`;
      if (have.has(rel)) { continue; }
      const src = fs.readFileSync(path.join(ROOT, rel), 'utf8');
      const r = spawnSync(CANCHO_UI, ['inspect', 'view', path.join(ROOT, rel)], { encoding: 'utf8' });
      if (r.status !== 0) { skipped.push(`${rel}: no outline (${r.stderr.trim().split('\n')[0].slice(0, 80)})`); continue; }
      if (JSON.parse(r.stdout).some((l) => l.trim().startsWith('? '))) { skipped.push(`${rel}: has a node the outline cannot say`); continue; }
      if (/^\s*(start|every|subscribe|route)\b/m.test(src) || /requires\s+(Router|Locale|Storage|Network|Credential)/.test(src)) { skipped.push(`${rel}: works before the first event, or reads the browser`); continue; }
      found.push({ name: `${d}/${f.replace('.cui', '')}`, file: rel, steps: [] });
    }
  }
  return { found, skipped };
}

(async () => {
  const extra = atRest();
  for (const c of extra.found) { CASES.push(c); }
  for (const s of extra.skipped) { console.log(`skip ${s}`); }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-view-'));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  for (const c of CASES) {
    const src = path.join(ROOT, c.file);
    const out = path.join(dir, c.name);
    const b = spawnSync(CANCHO_UI, ['build', src, '--pwa', '-o', out], { encoding: 'utf8' });
    if (b.status !== 0) { throw new Error(b.stderr); }
    // the outline for the same events, from the replay
    const flat = c.steps.flatMap((s) => (s.trace || [{ event: s.event, params: s.params }]));
    // the trace needs one assertion; every step says `note` and the first step asserts a view that is always true of any page: the outline itself is read with --view
    const trace = { schema: 'cancho-ui-trace', version: '0.1.0', steps: flat.map((s) => ({ event: s.event, params: s.params || [], view_has: [] })) };
    if (flat.length === 0) { trace.steps = []; }
    const initial = spawnSync(CANCHO_UI, ['inspect', 'view', src], { encoding: 'utf8' });
    check(`${c.name}: the outline of the initial state is readable`, initial.status === 0, initial.stderr + initial.stdout);
    let replay = [];
    if (flat.length) {
      const tf = path.join(dir, `${c.name}.json`);
      fs.writeFileSync(tf, JSON.stringify(trace));
      const r = spawnSync(CANCHO_UI, ['test', src, tf, '--output', 'json', '--view'], { encoding: 'utf8' });
      const report = JSON.parse(r.stdout);
      check(`${c.name}: the trace replays`, report.failures.length === 0, JSON.stringify(report.failures).slice(0, 300));
      replay = report.steps.map((s) => s.view);
    }

    const server = await serve(out);
    const page = await browser.newPage({ viewport: { width: 700, height: 600 } });
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root > *');

    const compare = async (label, want) => {
      let got = await page.evaluate(domOutline);
      for (let k = 0; k < 20 && JSON.stringify(got) !== JSON.stringify(want); k++) { await page.waitForTimeout(25); got = await page.evaluate(domOutline); }
      compared += 1;
      const same = JSON.stringify(got) === JSON.stringify(want);
      let detail;
      if (!same) {
        const i = got.findIndex((l, n) => l !== want[n]);
        detail = `line ${i}: page ${JSON.stringify(got[i])} against replay ${JSON.stringify(want[i])} (${got.length} lines against ${want.length})`;
      }
      check(`${c.name}: ${label}: ${want.length} lines equal`, same, detail);
    };

    await compare('at rest', JSON.parse(initial.stdout));
    let at = 0;
    for (const s of c.steps) {
      await s.act(page);
      at += (s.trace || [1]).length;
      await compare(`after ${s.event}${s.params ? '(' + s.params.join(', ') + ')' : ''}`, replay[at - 1]);
    }
    check(`${c.name}: no script errors`, errors.length === 0, errors.join(' | '));
    await page.close();
    server.close();
  }
  await browser.close();
  console.log(`${compared} outlines compared`);
  console.log(failures.length === 0 ? 'all view checks passed' : `${failures.length} failed`);
  process.exit(failures.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });
