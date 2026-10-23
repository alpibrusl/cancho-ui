#!/usr/bin/env node
// `Checkbox` (docs/controls.md section 9) in Chromium, from tests/checkbox_corpus.json (`cargo test` replays the same file; the
// `outcome` of a step is what replay answers, `disabled` for a click on a disabled box): a click on the box, a click on the text
// of its label and Space on the focused box each report the new state to the handler (`params`), the box is drawn from its
// `Bool` at each redraw, the focus survives the redraw (Space twice in a row), a disabled box reports nothing, `error:` ties its
// message to it, and the accessibility tree has a checkbox named by its label with the checked state, and a touch target of at
// least 44 by 44 px. Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-checkbox.js [path/to/cancho-ui]
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
  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'checkbox_corpus.json'), 'utf8'));
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-checkbox-'));
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
  const ctx = await browser.newContext({ serviceWorkers: 'block', viewport: { width: 420, height: 800 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root input[type=checkbox]');
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Accessibility.enable');

  const read = () => page.evaluate(() => {
    const i = document.querySelector('#root input[type=checkbox]');
    const id = i.getAttribute('aria-describedby');
    const t = id ? document.getElementById(id) : null;
    const l = i.closest('label');
    const box = l.getBoundingClientRect();
    return {
      checked: i.checked, invalid: i.getAttribute('aria-invalid') === 'true', message: t ? t.textContent : '', disabled: i.disabled,
      log: (document.body.innerText.match(/log=(\S*)/) || [])[1] || '', focus: document.activeElement.tagName, label: l.firstChild === i && l.lastChild.textContent, w: Math.round(box.width), h: Math.round(box.height),
    };
  });
  const axbox = async () => {
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    const n = nodes.find((x) => x.role && x.role.value === 'checkbox');
    return { name: n.name && n.name.value, checked: (n.properties || []).find((q) => q.name === 'checked'), description: n.description && n.description.value };
  };
  const expect = async (label, want) => {
    const d = await read();
    check(`${label}: checked, invalid, message, log`, d.checked === want.checked && d.invalid === want.invalid && d.message === want.message && d.log === want.log, JSON.stringify(d));
    return d;
  };

  await expect('initial', corpus.initial);
  let first = await read();
  check('the label is the text after the box, and the whole label is at least 44 by 44 px', first.label === 'I agree' && first.w >= 44 && first.h >= 44, JSON.stringify({ label: first.label, w: first.w, h: first.h }));
  let ax = await axbox();
  check('the accessibility tree has a checkbox named by its label, unchecked, described by its message', ax.name === 'I agree' && ax.checked && ax.checked.value.value === 'false' && (ax.description || '').endsWith('You must agree.'), JSON.stringify(ax));

  let i = 0;
  for (const s of corpus.steps) {
    const label = `${s.why} [step ${i++}: ${s.do}]`;
    const before = errors.length;
    if (s.do === 'click box') {
      await page.locator('#root input[type=checkbox]').click({ force: s.outcome === 'disabled' });
    } else if (s.do === 'click text') {
      await page.locator('#root label.cui-check').click({ position: { x: 60, y: 10 } });
    } else if (s.do === 'space') {
      if (i === 3) { await page.focus('#root input[type=checkbox]'); }
      check(`${label}: the focus is on the box before Space`, (await read()).focus === 'INPUT');
      await page.keyboard.press('Space');
    } else if (s.do === 'lock') {
      await page.click('button[aria-label="Lock"]');
    }
    await page.waitForTimeout(30);
    const d = await expect(label, s);
    if (s.outcome === 'disabled') { check(`${label}: the box is disabled`, d.disabled === true); }
    check(`${label}: no error`, errors.length === before, JSON.stringify(errors.slice(before)));
  }
  ax = await axbox();
  check('after the steps the tree has the box checked, with no description', ax.checked.value.value === 'true' && !ax.description, JSON.stringify(ax));

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
