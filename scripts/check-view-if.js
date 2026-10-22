#!/usr/bin/env node
// Every case of tests/view_if_corpus.json (docs/view-conditional.md) run in the emitted page, in Chromium: an `If` draws its
// children while the condition holds and has no node in the page when it does not, a control that is not in the page cannot be
// used, and Enter in a field is blocked only by a disabled button that is in the page. The same file is replayed by
// `cargo test` (crates/cancho-ui/tests/view_if.rs): `outcome` is what replay answers (`hidden`, `disabled`, `completed`) and
// what the page makes of the control. Also checked here only: a component instance under an `If` is disposed when the condition
// turns false and mounted fresh when it turns true again. Outside the gate: needs node, the `playwright` package with a
// Chromium, and a built `cancho-ui`.
//   node scripts/check-view-if.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

// What a reader of the page can see: the lines of text, the labels of the buttons, the rows of a table.
async function snapshot(page) {
  const texts = (await page.locator('#root').innerText()).split(/[\n\t]+/).map((s) => s.trim()).filter(Boolean);
  const buttons = await page.$$eval('#root button', (bs) => bs.map((b) => b.getAttribute('aria-label')));
  const rows = await page.$$eval('#root tr', (rs) => rs.length);
  return { texts, buttons, rows };
}
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-view-if-'));
  let served = root;
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(served, path.normalize(rel));
    if (!file.startsWith(served) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  async function open(source, name) {
    const dir = fs.mkdtempSync(path.join(root, `${name}-`));
    fs.writeFileSync(path.join(dir, 'p.cui'), source);
    const r = spawnSync(CANCHO_UI, ['build', path.join(dir, 'p.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { return { error: r.stderr }; }
    served = path.join(dir, 'out');
    const page = await browser.newPage();
    const errors = [];
    await page.exposeFunction('__report', (m) => errors.push(String(m)));
    await page.addInitScript(() => {
      window.addEventListener('unhandledrejection', (ev) => { window.__report(ev.reason && ev.reason.message ? ev.reason.message : String(ev.reason)); });
      window.addEventListener('error', (ev) => { window.__report(ev.message); });
    });
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root *');
    return { page, errors };
  }

  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'view_if_corpus.json'), 'utf8'));
  let n = 0;
  for (const c of corpus) {
    const opened = await open(c.source.join('\n') + '\n', `case${n++}`);
    if (opened.error) { check(c.why, false, `build failed: ${opened.error}`); continue; }
    const { page, errors } = opened;
    const want = (s) => ({ texts: s.texts, buttons: s.buttons, rows: s.rows === undefined ? undefined : s.rows });
    const got = async (s) => { const g = await snapshot(page); if (s.rows === undefined) { g.rows = undefined; } return g; };
    check(`${c.why} [initial]`, same(await got(c.initial), want(c.initial)), JSON.stringify(await snapshot(page)));
    let i = 0;
    for (const s of c.steps) {
      const label = `${c.why} [step ${i++}: ${s.event}${s.args ? `(${s.args})` : ''} ${s.outcome}]`;
      const target = s.enter ? page.locator(`#root input[aria-label="${s.enter}"]`) : page.locator(`#root button[aria-label="${s.label}"]`);
      const present = await target.count();
      if (s.outcome === 'hidden') {
        check(`${label}: the control is not in the page`, present === 0, `${present} found`);
      } else if (s.outcome === 'disabled' && !s.enter) {
        check(`${label}: the control is disabled`, present === 1 && await target.isDisabled(), `${present} found`);
      } else {
        check(`${label}: the control is in the page`, present === 1, `${present} found`);
        if (present === 1) {
          if (s.enter) { await target.focus(); await target.press('Enter'); } else { await target.click(); }
          await page.waitForTimeout(20);
        }
      }
      const g = await got(s);
      check(`${label}: the page after`, same(g, want(s)) && errors.length === 0, `${JSON.stringify(g)} errors ${JSON.stringify(errors)}`);
    }
    await page.close();
  }

  // A component instance under an If: its state is dropped when the condition turns false (disposed, as under a For that
  // shrinks) and the instance mounted again is fresh.
  const withInstance = `component Counter {
    state n: Int = 0;
    on Up { n = n + 1; }
    view { Column { Button { label: "Up" on: Up } Text { "n {n}" } } }
}
component App {
    state shown: Bool = true;
    on Flip { shown = !shown; }
    view { Column { Button { label: "Flip" on: Flip } If { when: shown Counter { } } } }
}
`;
  const inst = await open(withInstance, 'instance');
  if (inst.error) {
    check('an instance under an If builds', false, inst.error);
  } else {
    const { page, errors } = inst;
    await page.click('button[aria-label="Up"]');
    await page.click('button[aria-label="Up"]');
    check('the instance counts', (await snapshot(page)).texts.includes('n 2'), JSON.stringify(await snapshot(page)));
    await page.click('button[aria-label="Flip"]');
    check('the instance is gone while the condition is false', same((await snapshot(page)).buttons, ['Flip']), JSON.stringify(await snapshot(page)));
    await page.click('button[aria-label="Flip"]');
    check('the instance mounted again is fresh', (await snapshot(page)).texts.includes('n 0'), JSON.stringify(await snapshot(page)));
    check('no error was raised', errors.length === 0, JSON.stringify(errors));
    await page.close();
  }

  // Where focus goes when the branch that holds it is removed (docs/view-conditional.md section 4): the whole view is drawn again at
  // each render and focus is put back only on an element of the same place, tag and label, so it is the document body. This is
  // measured, not designed; alpibrusl/cancho-ui#92 asks for better.
  const closing = `component A {
    state shown: Bool = true;
    on Close { shown = false; }
    view { Column { Text { "page" } If { when: shown Button { label: "Close" on: Close } } } }
}
`;
  const closed = await open(closing, 'closing');
  if (closed.error) {
    check('a closing branch builds', false, closed.error);
  } else {
    const { page } = closed;
    await page.focus('button[aria-label="Close"]');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(20);
    const active = await page.evaluate(() => document.activeElement && document.activeElement.tagName);
    check('focus inside a removed branch falls to the body', active === 'BODY' && (await snapshot(page)).buttons.length === 0, `activeElement ${active}`);
    await page.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
