#!/usr/bin/env node
// `Select` (docs/controls.md section 10) in Chromium, from tests/select_corpus.json (`cargo test` replays the same file and checks
// that replay refuses a value that is not an option): the page draws one <option> per string of the list, read from the state at each
// redraw; a value that is not in the list selects nothing (no first option shown in its place); choosing with the mouse API or the
// arrow keys reports the option's string to the handler; the focus survives the redraw (ArrowUp twice in a row); `error:` ties its
// message to the select; the accessibility tree has a combobox named by its label, with the chosen value. Outside the gate: needs
// node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-select.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

(async () => {
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'select_corpus.json'), 'utf8'));
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-select-'));
  fs.writeFileSync(path.join(dir, 'p.cui'), corpus.program.join('\n') + '\n');
  const built = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (built.status !== 0) { throw new Error(built.stderr); }
  const server = http.createServer((req, res) => {
    const u = req.url.split('?')[0];
    const f = path.join(dir, 'out', u === '/' ? 'index.html' : path.normalize(u));
    if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : f.endsWith('.png') ? 'image/png' : 'text/html', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ serviceWorkers: 'block' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root select');
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Accessibility.enable');

  const read = () => page.evaluate(() => {
    const s = document.querySelector('#root select');
    const id = s.getAttribute('aria-describedby');
    const t = id ? document.getElementById(id) : null;
    return {
      options: Array.from(s.options).map((o) => o.value), value: s.value, selectedIndex: s.selectedIndex, invalid: s.getAttribute('aria-invalid') === 'true',
      message: t ? t.textContent : '', log: (document.body.innerText.match(/log=(\S*)/) || [])[1] || '', focus: document.activeElement.tagName,
      labelled: s.closest('label') && s.closest('label').classList.contains('cui-field') && s.closest('label').firstChild.textContent,
    };
  });
  const axSelect = async () => {
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    const n = nodes.find((x) => x.role && (x.role.value === 'combobox' || x.role.value === 'listbox' || x.role.value === 'menulist'));
    return n && { role: n.role.value, name: n.name && n.name.value, value: n.value && n.value.value };
  };

  const first = await read();
  const want0 = corpus.initial;
  check('initial: the options of the list, nothing selected, invalid', JSON.stringify(first.options) === JSON.stringify(want0.options) && first.selectedIndex === want0.selectedIndex && first.value === want0.value && first.invalid === want0.invalid, JSON.stringify(first));
  check('the label is shown with the select', first.labelled === 'Status', JSON.stringify(first.labelled));
  let ax = await axSelect();
  check('the accessibility tree has a select named by its label', !!ax && ax.name === 'Status', JSON.stringify(ax));

  let i = 0;
  for (const s of corpus.steps) {
    const label = `${s.why} [step ${i++}: ${s.do}]`;
    const before = errors.length;
    if (s.do === 'select') {
      await page.selectOption('#root select', s.pick);
    } else if (s.do === 'key') {
      if (i === 3) { await page.focus('#root select'); }
      check(`${label}: the focus is on the select before the key`, (await read()).focus === 'SELECT');
      await page.keyboard.press(s.key);
    } else if (s.do === 'more') {
      await page.click('button[aria-label="More"]');
    }
    await page.waitForTimeout(30);
    const d = await read();
    check(`${label}: value, validity, log${s.options ? ', options' : ''}`, d.value === s.value && d.invalid === s.invalid && d.log === s.log && (!s.options || JSON.stringify(d.options) === JSON.stringify(s.options)), JSON.stringify(d));
    check(`${label}: no error`, errors.length === before, JSON.stringify(errors.slice(before)));
  }
  ax = await axSelect();
  check('after the steps the tree has the chosen value', !!ax && ax.value === 'blocked', JSON.stringify(ax));

  // Strings that are not tidy: an option with spaces around it reports exactly itself; two options with one string report that
  // string, and the redraw selects the first of them (the state holds a string, not a position).
  {
    const d2 = path.join(dir, 'odd');
    fs.mkdirSync(d2);
    fs.writeFileSync(path.join(d2, 'p.cui'), `component O {
    state v: String = "";
    state log: String = "";
    on Pick(x) { v = x; log = log + "[" + x + "]"; }
    view { Column { Select { label: "O" options: ["  spaced  ", "x", "x", "y"] value: v on: Pick } Text { "log={log}" } } }
}
`);
    const r = spawnSync(CANCHO_UI, ['build', path.join(d2, 'p.cui'), '--pwa', '-o', path.join(d2, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(r.stderr); }
    const s2 = http.createServer((req, res) => {
      const u = req.url.split('?')[0];
      const f = path.join(d2, 'out', u === '/' ? 'index.html' : path.normalize(u));
      if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : 'text/html' });
      fs.createReadStream(f).pipe(res);
    });
    await new Promise((ok) => s2.listen(0, '127.0.0.1', ok));
    const p2 = await browser.newPage();
    await p2.goto(`http://localhost:${s2.address().port}/`);
    await p2.waitForSelector('#root select');
    await p2.selectOption('#root select', { index: 0 });
    await p2.waitForTimeout(30);
    check('an option with spaces around it reports exactly itself', (await p2.inputValue('#root select')) === '  spaced  ', JSON.stringify(await p2.inputValue('#root select')));
    await p2.selectOption('#root select', { index: 2 });
    await p2.waitForTimeout(30);
    const dup = await p2.evaluate(() => ({ value: document.querySelector('#root select').value, index: document.querySelector('#root select').selectedIndex }));
    check('two options with one string: the handler gets the string, the redraw selects the first of them', dup.value === 'x' && dup.index === 1, JSON.stringify(dup));
    await p2.close();
    s2.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
