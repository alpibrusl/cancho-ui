#!/usr/bin/env node
// A status Text with a tone draws its mark only while it has something to say (docs/presentation.md section 3), measured in
// Chromium: empty or whitespace-only text has no `data-tone`, no `::before` mark and no height; a message brings the tone and the
// mark back; and in every state the element is still there with `role=status`, in the accessibility tree as a polite, atomic
// live region, so a message put there by a later render is in a live region. Outside the gate: needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-tone.js [path/to/cancho-ui]
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
const SOURCE = `component T {
    state msg: String = "";
    state tone: String = "error";
    on Fill { msg = "Enter your password."; }
    on Space { msg = " "; }
    on Clear { msg = ""; }
    on Warn { tone = "warn"; msg = "Careful."; }
    view { Column {
        Button { label: "Fill" on: Fill } Button { label: "Space" on: Space } Button { label: "Clear" on: Clear } Button { label: "Warn" on: Warn }
        Text { role: status tone: tone "{msg}" }
        Text { role: status tone: "ok" "constant tone, empty: {msg}" }
        Text { role: status tone: "error" "{trim(\\"\\")}" }
    } }
}
`;

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-tone-'));
  fs.writeFileSync(path.join(dir, 'p.cui'), SOURCE);
  const built = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (built.status !== 0) { throw new Error(built.stderr); }
  const server = http.createServer((req, res) => {
    const u = req.url.split('?')[0];
    const f = path.join(dir, 'out', u === '/' ? 'index.html' : path.normalize(u));
    if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : 'text/html', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ serviceWorkers: 'block' });
  const page = await ctx.newPage();
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Accessibility.enable');

  const status = async () => page.evaluate(() => Array.from(document.querySelectorAll('[role=status]')).map((e) => ({
    tone: e.getAttribute('data-tone'), before: getComputedStyle(e, '::before').content, height: Math.round(e.getBoundingClientRect().height), text: e.textContent,
  })));
  const live = async () => {
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    return nodes.filter((n) => n.role && n.role.value === 'status').map((n) => Object.fromEntries((n.properties || []).map((q) => [q.name, q.value.value])));
  };
  const state = async (label, wantFirst) => {
    const s = await status();
    const l = await live();
    check(`${label}: three status elements, each a polite atomic live region in the accessibility tree`, s.length === 3 && l.length === 3 && l.every((x) => x.live === 'polite' && x.atomic === true), JSON.stringify(l));
    check(`${label}: the message`, s[0].tone === wantFirst.tone && (s[0].before === 'none') === wantFirst.noMark && (s[0].height === 0) === wantFirst.noMark, JSON.stringify(s[0]));
    return s;
  };

  let s = await state('start (empty message)', { tone: null, noMark: true });
  check('a status text that has text keeps its constant tone and mark', s[1].tone === 'ok' && s[1].before !== 'none', JSON.stringify(s[1]));
  check('a constant tone whose text is empty has no tone and no mark', s[2].tone === null && s[2].before === 'none' && s[2].height === 0, JSON.stringify(s[2]));
  await page.click('button[aria-label="Fill"]'); await page.waitForTimeout(30);
  s = await state('filled', { tone: 'error', noMark: false });
  check('the mark is the cross of the error tone', s[0].before === '"✗ "', s[0].before);
  await page.click('button[aria-label="Space"]'); await page.waitForTimeout(30);
  await state('whitespace only', { tone: null, noMark: true });
  await page.click('button[aria-label="Clear"]'); await page.waitForTimeout(30);
  await state('cleared', { tone: null, noMark: true });
  await page.click('button[aria-label="Warn"]'); await page.waitForTimeout(30);
  s = await state('filled again with another tone', { tone: 'warn', noMark: false });
  check('the warn tone draws its own mark', s[0].before === '"⚠ "', s[0].before);

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
