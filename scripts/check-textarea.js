#!/usr/bin/env node
// `TextArea` (docs/controls.md section 8) in Chromium, from tests/textarea_corpus.json (`cargo test` replays the same file with the
// `received` value): what is typed, set as a whole or pasted into a real <textarea> reaches the handler as `received` (line
// breaks are LF, a line break counts one toward `maxlength`). Also checked: it is a multiline textbox in the accessibility tree
// with its label as its name, `rows:` is the attribute, Enter inserts a line break, `error:` ties its message to it, the caret
// and the scroll position survive the redraw at each key, and the focus stays. Outside the gate: needs node, the `playwright`
// package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-textarea.js [path/to/cancho-ui]
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
const program = (bindings) => `component N {
    state note: String = "";
    state msg: String = "";
    on Note(v) { note = v; msg = if len(v) < 3 { "A note has at least 3 characters." } else { "" }; }
    view { Column { TextArea { label: "Note" value: note on: Note ${bindings} } Text { "len={len(note)}" } } }
}
`;

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-textarea-'));
  let served = dir;
  const server = http.createServer((req, res) => {
    const u = req.url.split('?')[0];
    const f = path.join(served, u === '/' ? 'index.html' : path.normalize(u));
    if (!f.startsWith(served) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : 'text/html', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  let n = 0;
  async function open(bindings) {
    const d = path.join(dir, `case${n++}`);
    fs.mkdirSync(d);
    fs.writeFileSync(path.join(d, 'p.cui'), program(bindings));
    const r = spawnSync(CANCHO_UI, ['build', path.join(d, 'p.cui'), '--pwa', '-o', path.join(d, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(r.stderr); }
    served = path.join(d, 'out');
    const ctx = await browser.newContext({ serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root textarea');
    return { page, ctx };
  }
  const value = (page) => page.inputValue('#root textarea');

  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'textarea_corpus.json'), 'utf8'));
  for (const c of corpus.typing) {
    const { page, ctx } = await open(c.maxlength ? `maxlength: ${c.maxlength}` : '');
    await page.focus('#root textarea');
    if (c.how === 'keys') { await page.keyboard.type(c.typed); } else if (c.how === 'paste') { await page.keyboard.insertText(c.typed); } else { await page.fill('#root textarea', c.typed); }
    await page.waitForTimeout(30);
    check(`${c.why} [${c.how} ${JSON.stringify(c.typed)}${c.maxlength ? `, maxlength ${c.maxlength}` : ''}]`, (await value(page)) === c.received, `${JSON.stringify(await value(page))} want ${JSON.stringify(c.received)}`);
    await ctx.close();
  }

  {
    const { page, ctx } = await open('rows: 5 error: msg');
    check('rows: is the attribute', (await page.getAttribute('#root textarea', 'rows')) === '5');
    check('the label is shown with the field, in a label element', await page.evaluate(() => { const l = document.querySelector('#root textarea').closest('label'); return !!l && l.classList.contains('cui-field') && l.firstChild.textContent === 'Note'; }));
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Accessibility.enable');
    await page.fill('#root textarea', 'ab');
    await page.waitForTimeout(30);
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    const box = nodes.find((x) => x.role && x.role.value === 'textbox');
    const props = Object.fromEntries((box.properties || []).map((q) => [q.name, q.value.value]));
    check('it is a multiline textbox named by its label, invalid, described by its message', props.multiline === true && box.name.value === 'Note' && props.invalid === 'true' && (box.description.value || '').endsWith('A note has at least 3 characters.'), JSON.stringify({ name: box.name.value, multiline: props.multiline, invalid: props.invalid, description: box.description && box.description.value }));
    await page.fill('#root textarea', 'abc');
    await page.waitForTimeout(30);
    check('a valid note has no aria-invalid and no aria-describedby', (await page.getAttribute('#root textarea', 'aria-invalid')) === null && (await page.getAttribute('#root textarea', 'aria-describedby')) === null);
    await page.fill('#root textarea', '');
    await page.focus('#root textarea');
    await page.keyboard.type('x');
    await page.keyboard.press('Enter');
    await page.keyboard.type('y');
    check('Enter inserts a line break and does nothing else', (await value(page)) === 'x\ny');

    // The page draws the field again at every key; the caret, the scroll position and the focus have to come back.
    await page.fill('#root textarea', Array.from({ length: 40 }, (_, i) => `line ${i}`).join('\n'));
    const before = await page.evaluate(() => {
      const t = document.querySelector('#root textarea');
      t.focus();
      t.scrollTop = 300;
      const lines = t.value.split('\n');
      const line = Math.round(300 / (parseFloat(getComputedStyle(t).lineHeight) || 18)) + 2;
      let off = 0;
      for (let i = 0; i < line; i++) { off += lines[i].length + 1; }
      t.setSelectionRange(off, off);
      return { off, top: t.scrollTop };
    });
    await page.keyboard.type('Z');
    const after = await page.evaluate(() => { const t = document.querySelector('#root textarea'); return { start: t.selectionStart, top: t.scrollTop, focus: document.activeElement.tagName, has: t.value.includes('Zline') || t.value.includes('Z') }; });
    check('typing in the middle keeps the caret after the typed letter, the lines scrolled to, and the focus', after.start === before.off + 1 && after.top === before.top && after.focus === 'TEXTAREA', JSON.stringify({ before, after }));
    await ctx.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
