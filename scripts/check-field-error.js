#!/usr/bin/env node
// `error:` on a TextField (docs/controls.md section 7) in Chromium, from tests/field_error_corpus.json (`cargo test` replays the same
// file): while the message holds text the field has aria-invalid="true" and aria-describedby naming the status element that
// holds the message, and the accessibility tree has the textbox invalid with that message as its description; while it is
// empty the field is valid, has no describedby, and the message element has no tone and no mark. Typing keeps the focus in the
// field. Two fields in the rows of a For get two different ids. Outside the gate: needs node, the `playwright` package with a
// Chromium, and a built `cancho-ui`.
//   node scripts/check-field-error.js [path/to/cancho-ui]
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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-field-error-'));
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
  async function open(source) {
    const d = path.join(dir, `case${n++}`);
    fs.mkdirSync(d);
    fs.writeFileSync(path.join(d, 'p.cui'), source);
    const r = spawnSync(CANCHO_UI, ['build', path.join(d, 'p.cui'), '--pwa', '-o', path.join(d, 'out')], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(r.stderr); }
    served = path.join(d, 'out');
    const ctx = await browser.newContext({ serviceWorkers: 'block' });
    const page = await ctx.newPage();
    await page.goto(`http://localhost:${server.address().port}/`);
    await page.waitForSelector('#root input');
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Accessibility.enable');
    return { page, cdp, ctx };
  }
  const textboxes = async (cdp) => {
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    return nodes.filter((x) => x.role && x.role.value === 'textbox').map((x) => ({
      name: x.name && x.name.value,
      description: (x.description && x.description.value) || '',
      invalid: Object.fromEntries((x.properties || []).map((q) => [q.name, q.value.value])).invalid,
    }));
  };
  const dom = (page) => page.evaluate(() => {
    const i = document.querySelector('#root input');
    const id = i.getAttribute('aria-describedby');
    const target = id ? document.getElementById(id) : null;
    const sibling = i.closest('label').nextElementSibling;
    return {
      invalid: i.getAttribute('aria-invalid'), describedby: id, target: target && target.textContent, targetRole: target && target.getAttribute('role'),
      sibling: sibling && { role: sibling.getAttribute('role'), tone: sibling.getAttribute('data-tone'), text: sibling.textContent, mark: getComputedStyle(sibling, '::before').content, height: Math.round(sibling.getBoundingClientRect().height) },
      focus: document.activeElement && document.activeElement.tagName,
    };
  });

  const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'field_error_corpus.json'), 'utf8'));
  const { page, cdp, ctx } = await open(corpus.program.join('\n') + '\n');
  const expect = async (label, want) => {
    const d = await dom(page);
    const ax = (await textboxes(cdp))[0];
    if (want.invalid) {
      check(`${label}: aria-invalid, aria-describedby and the message element`, d.invalid === 'true' && !!d.describedby && d.target === want.message && d.targetRole === 'status' && d.sibling.tone === 'error', JSON.stringify(d));
      check(`${label}: the accessibility tree has the textbox invalid, described by the message`, ax.invalid === 'true' && ax.description === `\u2717 ${want.message}`, JSON.stringify(ax) + ' (the description is the message with the error mark, which is generated content, in front)');
    } else {
      check(`${label}: no aria-invalid, no aria-describedby, an empty message with no tone and no mark`, d.invalid === null && d.describedby === null && d.sibling.role === 'status' && d.sibling.text === '' && d.sibling.tone === null && d.sibling.mark === 'none' && d.sibling.height === 0, JSON.stringify(d));
      check(`${label}: the accessibility tree has the textbox valid, with no description`, ax.invalid !== 'true' && ax.description === '', JSON.stringify(ax));
    }
  };
  await expect('initial', corpus.initial);
  for (const s of corpus.steps) {
    await page.fill('#root input', s.typed);
    await page.waitForTimeout(30);
    await expect(s.why, s);
    check(`${s.why}: the focus stays in the field`, (await dom(page)).focus === 'INPUT');
  }
  await ctx.close();

  // Two fields in the rows of a For: two ids, each field pointing at its own message.
  {
    const { page: p2, ctx: c2 } = await open(`component R {
    state rows: List[String] = ["a", "b"];
    state msg: String = "bad";
    on V(x) { msg = x; }
    view { Column { For { over: rows as: r TextField { label: "Row " + r value: msg on: V error: msg } } } }
}
`);
    const ids = await p2.$$eval('#root input', (is) => is.map((i) => i.getAttribute('aria-describedby')));
    const targets = await p2.$$eval('#root input', (is) => is.map((i) => { const t = document.getElementById(i.getAttribute('aria-describedby')); return t && t.parentNode === i.closest('label').parentNode && t.previousElementSibling === i.closest('label'); }));
    check('two fields in a For have two different ids', ids.length === 2 && ids[0] && ids[1] && ids[0] !== ids[1], JSON.stringify(ids));
    check('each field points at the message right after it', targets.every((t) => t === true), JSON.stringify(targets));
    check('the ids are unique in the page', await p2.evaluate(() => { const all = Array.from(document.querySelectorAll('[id]')).map((e) => e.id); return all.length === new Set(all).size; }));
    await c2.close();
  }

  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
