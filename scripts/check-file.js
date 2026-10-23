#!/usr/bin/env node
// `FileField` (docs/file-upload.md, issue #79) in Chromium, in the mobile profile (Pixel 7), with files set through the browser's
// file chooser (Playwright's setInputFiles):
//  A. tests/file_corpus.json, the file `cargo test` replays (crates/cancho-ui/tests/file.rs): for each file the handler is told exactly
//     the JSON text the corpus gives (strict UTF-8, accept judged by the page by extension and by type, the size before the bytes, a
//     BOM dropped, quotes and non-ASCII in names), and nothing is told when the chooser is cleared;
//  B. the choice survives the redraws the page makes (the rebuilt input shows the file again, and no second event fires);
//  C. examples/upload against a stand-in endpoint: the body that arrives is the JSON the program built, byte for byte, with the file's
//     text; refused files cannot be sent;
//  D. the default cap: 1 MiB is read, 1 MiB + 1 byte is not;
//  E. the accessibility tree (a button named by its label, the label visible) and `disabled:`.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-file.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

function serve(dir, onPost) {
  const types = { '.html': 'text/html', '.js': 'text/javascript' };
  return new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const u = req.url.split('?')[0];
      if (req.method === 'POST' && u === '/api/upload') {
        const chunks = [];
        req.on('data', (c) => chunks.push(c));
        req.on('end', () => { onPost({ body: Buffer.concat(chunks), type: req.headers['content-type'] }); res.writeHead(200, { 'content-type': 'text/plain' }); res.end('ok'); });
        return;
      }
      const f = path.join(dir, u === '/' ? 'index.html' : path.normalize(u));
      if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': types[path.extname(f)] || 'text/plain', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
}

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-file-'));
  const build = (name, file) => {
    const dir = path.join(tmp, name);
    const r = spawnSync(CANCHO_UI, ['build', file, '--page', '-o', dir, '--name', name], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stdout}${r.stderr}`); }
    return dir;
  };
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const open = async (dir, onPost = () => {}) => {
    const s = await serve(dir, onPost);
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(`pageerror ${e}`));
    page.on('console', (m) => { if (m.type() === 'error') { problems.push(`console ${m.text()}`); } });
    await page.goto(`http://localhost:${s.address().port}/`);
    await page.waitForSelector('#root input[type=file]');
    return { s, ctx, page, problems, close: async () => { await ctx.close(); s.close(); } };
  };
  const pick = async (page, name, type, bytes) => {
    await page.locator('#root input[type=file]').setInputFiles({ name, mimeType: type, buffer: Buffer.from(bytes) });
    await page.waitForTimeout(120);
  };
  const lastOf = (page) => page.evaluate(() => {
    const s = Array.from(document.querySelectorAll('#root span')).find((e) => e.textContent.startsWith('last='));
    return s ? s.textContent.slice(5) : null;
  });
  const countOf = (page) => page.evaluate(() => Number((document.body.innerText.match(/picked=(\d+)/) || [])[1]));

  // ---- A and B. the corpus ----
  {
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'file_corpus.json'), 'utf8'));
    const dir = path.join(tmp, 'corpus');
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), corpus.program.join('\n') + '\n');
    const t = await open(build('corpus', path.join(dir, 'p.cui')));
    const { page } = t;
    let n = 0;
    for (const s of corpus.steps) {
      await pick(page, s.name, s.type, Buffer.from(s.hex, 'hex'));
      n += 1;
      const got = await lastOf(page);
      check(`corpus: ${s.why}`, got === s.value && (await countOf(page)) === n, `${got} vs ${s.value}`);
    }
    // B. the choice survives the redraws
    const last = corpus.steps[corpus.steps.length - 1];
    await page.getByRole('button', { name: 'Bump', exact: true }).tap();
    await page.waitForTimeout(80);
    const after = await page.evaluate(() => { const i = document.querySelector('#root input[type=file]'); return { n: i.files.length, name: i.files[0] && i.files[0].name }; });
    check('the chosen file is still in the input after a redraw', after.n === 1 && after.name === last.name, JSON.stringify(after));
    check('and the redraw told the handler nothing', (await countOf(page)) === n);
    // cancelling the chooser (an empty selection) tells nothing and keeps the earlier choice
    await page.locator('#root input[type=file]').setInputFiles([]);
    await page.waitForTimeout(80);
    check('clearing the chooser tells nothing', (await countOf(page)) === n && (await lastOf(page)) === last.value);
    await page.getByRole('button', { name: 'Bump', exact: true }).tap();
    await page.waitForTimeout(80);
    check('and the earlier choice is back at the next redraw', (await page.evaluate(() => document.querySelector('#root input[type=file]').files.length)) === 1);
    check('corpus: no page or console error', t.problems.length === 0, t.problems.join('; '));
    await t.close();
  }

  // ---- C. the example against a stand-in endpoint ----
  {
    const posts = [];
    const t = await open(build('upload', path.join(ROOT, 'examples/upload/upload.cui')), (p) => posts.push(p));
    const { page } = t;
    const status = () => page.evaluate(() => (Array.from(document.querySelectorAll('[role=status]')).map((e) => e.textContent).join(' | ')));
    const sendEnabled = () => page.getByRole('button', { name: 'Send', exact: true }).isEnabled();
    check('upload: Send is disabled until a file is chosen', !(await sendEnabled()));
    const text = 'première ligne\nsecond "line" \\ with é and 日本\n';
    await pick(page, 'notes.txt', 'text/plain', text);
    check('upload: the page says what it got', /notes\.txt \(\d+ bytes\) is ready to send\./.test(await status()), await status());
    check('upload: the size is the bytes, not the characters', (await status()).includes(`(${Buffer.byteLength(text)} bytes)`), await status());
    check('upload: the first line is shown', (await page.evaluate(() => document.body.innerText)).includes('first line: première ligne'));
    check('upload: Send is enabled', await sendEnabled());
    await page.getByRole('button', { name: 'Send', exact: true }).tap();
    await page.waitForFunction(() => /notes\.txt was sent\./.test(document.body.innerText), null, { timeout: 5000 });
    check('upload: the body is the JSON the program built, with the file\'s text, byte for byte', posts.length === 1 && posts[0].body.equals(Buffer.from(JSON.stringify(['notes.txt', text]), 'utf8')) && posts[0].type === 'application/json', posts[0] && posts[0].body.toString());
    await pick(page, 'logo.png', 'image/png', Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 0]));
    check('upload: a PNG is refused by the page and cannot be sent', /logo\.png cannot be sent: not an accepted type\./.test(await status()) && !(await sendEnabled()), await status());
    await pick(page, 'big.txt', 'text/plain', Buffer.alloc(100001, 0x78));
    check('upload: 100,001 bytes is too large (max_bytes: 100000) and cannot be sent', /big\.txt cannot be sent: too large\./.test(await status()) && !(await sendEnabled()), await status());
    await pick(page, 'bin.txt', 'text/plain', Buffer.from([0xff, 0xfe, 0xfd]));
    check('upload: bytes that are not UTF-8 are "not text" and cannot be sent', /bin\.txt cannot be sent: not text\./.test(await status()) && !(await sendEnabled()), await status());
    check('upload: only the one body was sent', posts.length === 1);
    check('upload: no page or console error', t.problems.length === 0, t.problems.join('; '));
    await t.close();
  }

  // ---- D and E. the default cap, the accessibility tree, disabled ----
  {
    const dir = path.join(tmp, 'plain');
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'p.cui'), `component P {
    state last: String = "";
    state off: Bool = false;
    on Picked(f) { last = f; }
    on Off { off = true; }
    view {
        Column {
            FileField { label: "Any file" on: Picked disabled: off }
            Button { label: "Off" on: Off }
            Text { "last={last}" }
        }
    }
}
`);
    const t = await open(build('plain', path.join(dir, 'p.cui')));
    const { page } = t;
    await pick(page, 'edge.txt', 'text/plain', Buffer.alloc(1048576, 0x61));
    const a = JSON.parse(await lastOf(page));
    check('default cap: exactly 1 MiB is read', a.size === 1048576 && a.text && a.text.length === 1048576 && a.error === undefined, `${a.size} ${a.error}`);
    await pick(page, 'edge2.txt', 'text/plain', Buffer.alloc(1048577, 0x61));
    const b = JSON.parse(await lastOf(page));
    check('default cap: 1 MiB + 1 byte is "too large" and not read', b.error === 'too large' && b.text === undefined, JSON.stringify({ ...b, text: undefined }));
    await pick(page, 'anything.bin', 'application/octet-stream', Buffer.from('plain text in a .bin'));
    const c = JSON.parse(await lastOf(page));
    check('no accept: any name is read if it is UTF-8', c.text === 'plain text in a .bin', JSON.stringify(c));
    const cdp = await t.ctx.newCDPSession(page);
    await cdp.send('Accessibility.enable');
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    const n = nodes.find((x) => x.role && x.role.value === 'button' && x.name && /Any file/.test(x.name.value));
    check('a11y: the file input is a button named by its label', !!n, JSON.stringify(n && { role: n.role.value, name: n.name.value }));
    const shown = await page.evaluate(() => { const l = document.querySelector('#root label.cui-field'); return l ? { text: l.firstChild.textContent, visible: l.getBoundingClientRect().height > 0, input: l.querySelector('input[type=file]') !== null } : null; });
    check('a11y: the label is drawn above the input, in a label', !!shown && shown.text === 'Any file' && shown.visible && shown.input, JSON.stringify(shown));
    const box = await page.locator('#root input[type=file]').boundingBox();
    check('a11y: the control is at least 44 px high (touch target)', box && box.height >= 44, JSON.stringify(box));
    await page.keyboard.press('Tab');
    check('a11y: the keyboard reaches it', await page.evaluate(() => document.activeElement.type === 'file'));
    await page.getByRole('button', { name: 'Off', exact: true }).tap();
    await page.waitForTimeout(80);
    check('disabled: the input is disabled', await page.evaluate(() => document.querySelector('#root input[type=file]').disabled));
    check('no page or console error', t.problems.length === 0, t.problems.join('; '));
    await t.close();
  }

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
