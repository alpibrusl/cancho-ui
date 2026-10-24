#!/usr/bin/env node
// Status live regions keep their node (docs/presentation.md section 3a, issue #106) in Chromium, in the mobile profile (Pixel 7). A `Text { role: status }`
// and the `error:` message of a field are drawn again at every render, as the whole page is; the node must be the same one afterwards:
//  A. a render in which only something else changed keeps the DOM node, its accessibility node (the DevTools protocol's node id) and its properties
//     (role status, live polite, atomic), and a MutationObserver on it sees nothing (so nothing is announced again);
//  B. a message filled later is delivered as a change to that node: the observer sees the text added, and replaced when it changes;
//  C. typing in a text field whose handler writes the status at every key keeps the focus, the caret and every character;
//  D. the `error:` message of a field: the same node, the same id, `aria-describedby` and `aria-invalid` following the text;
//  E. rows of a `For`: each keeps its node by order; a row that goes is gone; a status under an `If` that hides it is a new node when it comes back;
//  F. `role: alert` is kept the same way, and `tone:` changes are changes of the same node.
// Whether a screen reader announces any of it is NOT measured here (docs/presentation.md section 3a).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`. Given an older compiler and `--report` it prints
// what that one does (the measurement of the issue) instead of checking.
//   node scripts/check-status.js [path/to/cancho-ui] [--report]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();

const ROOT = path.join(__dirname, '..');
const args = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const REPORT = process.argv.includes('--report');
const CANCHO_UI = args[0] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const J = JSON.stringify;
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

const SOURCE = `component S {
    state msg: String = "";
    state name: String = "";
    state tone: String = "error";
    state ticks: Int = 0;
    state rows: List[String] = ["a", "b", "c"];
    state shown: Bool = true;
    state alertText: String = "";
    state nameError: String = "";
    on Tick { ticks = ticks + 1; }
    on Fill { msg = "Saved"; }
    on Change { msg = "Failed"; }
    on Clear { msg = ""; }
    on Warn { tone = "warn"; }
    on Name(x) { name = x; msg = "typed " + x; nameError = if x == "bad" { "Not allowed" } else { "" }; }
    on Shrink { rows = take(rows, 2); }
    on Grow { rows = concat(rows, ["d"]); }
    on Hide { shown = false; }
    on Show { shown = true; }
    on Ring { alertText = "Fire"; }
    view { Column {
        Button { label: "Tick" on: Tick } Button { label: "Fill" on: Fill } Button { label: "Change" on: Change } Button { label: "Clear" on: Clear }
        Button { label: "Warn" on: Warn } Button { label: "Shrink" on: Shrink } Button { label: "Grow" on: Grow } Button { label: "Hide" on: Hide }
        Button { label: "Show" on: Show } Button { label: "Ring" on: Ring }
        Text { "ticks={ticks}" }
        Text { role: status tone: tone "{msg}" }
        TextField { label: "Name" value: name on: Name error: nameError }
        For { over: rows as: r Text { role: status "row {r} {msg}" } }
        If { when: shown Text { role: status "under an if: {msg}" } }
        Text { role: alert "{alertText}" }
    } }
}
`;

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'check-status-'));
  const dir = path.join(root, 'out');
  fs.writeFileSync(path.join(root, 'p.cui'), SOURCE);
  const r = spawnSync(CANCHO_UI, ['build', path.join(root, 'p.cui'), '--page', '-o', dir], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build: ${r.stdout}${r.stderr}`); }
  const server = await new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
      const f = path.join(dir, path.normalize(rel));
      if (!f.startsWith(dir) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': f.endsWith('.js') ? 'text/javascript' : 'text/html', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Accessibility.enable');
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root *');

  // the live regions in document order: 0 the toned status, 1 the message of the Name field, 2..4 the rows of the For, 5 the one under the If, 6 the alert
  const statusAx = async () => {
    const { nodes } = await cdp.send('Accessibility.getFullAXTree');
    return nodes.filter((n) => !n.ignored && n.role && (n.role.value === 'status' || n.role.value === 'alert')).map((n) => {
      const prop = (name) => ((n.properties || []).find((p) => p.name === name) || {}).value;
      return { id: n.nodeId, role: n.role.value, live: prop('live') && prop('live').value, atomic: prop('atomic') && prop('atomic').value, text: (n.childIds || []).length };
    });
  };
  // hold the nodes, watch them
  const hold = () => page.evaluate(() => {
    window.__held = [...document.querySelectorAll('#root [role=status], #root [role=alert]')];
    window.__recs = window.__held.map(() => []);
    window.__held.forEach((el, i) => new MutationObserver((rs) => { rs.forEach((x) => window.__recs[i].push(x.type === 'childList' ? `childList +${x.addedNodes.length} -${x.removedNodes.length}` : `${x.type}:${x.attributeName || ''}`)); }).observe(el, { childList: true, characterData: true, subtree: true, attributes: true }));
    return window.__held.length;
  });
  const state = () => page.evaluate(() => ({
    same: window.__held.map((el, i) => document.querySelectorAll('#root [role=status], #root [role=alert]')[i] === el),
    connected: window.__held.map((el) => el.isConnected),
    recs: window.__recs.map((r) => r.slice()),
    texts: window.__held.map((el) => el.textContent),
  }));
  const reset = () => page.evaluate(() => { window.__recs.forEach((r) => { r.length = 0; }); });
  const click = async (label) => { await page.click(`button[aria-label="${label}"]`); await sleep(40); };

  const count = await hold();
  const axBefore = await statusAx();
  check(`the page has ${count} live regions (the toned status, three rows, the one under the if, the field's message, the alert)`, count === 7, String(count));

  // ---- A. a render in which only something else changed ------------------------------------------------------------------------------------
  await click('Tick');
  let s = await state();
  const axA = await statusAx();
  if (REPORT) {
    console.log(`after an unrelated render with this compiler: same DOM node ${J(s.same)}; accessibility ids ${J(axBefore.map((a) => a.id))} -> ${J(axA.map((a) => a.id))}`);
    await browser.close(); server.close(); process.exit(0);
  }
  check('A: after a render that changed only something else, every live region is the same DOM node, still connected', s.same.every(Boolean) && s.connected.every(Boolean), J(s));
  check('A: and the same accessibility node (the protocol\'s node id), with role, live=polite/assertive and atomic as before', J(axA.map((a) => a.id)) === J(axBefore.map((a) => a.id)) && axA.every((a) => a.atomic === true && (a.role === 'alert' ? a.live === 'assertive' : a.live === 'polite')), J(axA));
  check('A: and nothing was written to any of them (an observer on each sees no change, so nothing is announced again)', s.recs.every((x) => x.length === 0), J(s.recs));

  // ---- B. a message filled later is a change to that node ----------------------------------------------------------------------------------------
  await reset();
  await click('Fill');
  s = await state();
  check('B: filling the status: the same node, and the observer on it sees the text added', s.same[0] && s.recs[0].some((x) => x.startsWith('childList +1')) && s.texts[0] === 'Saved', J({ same: s.same[0], rec: s.recs[0], text: s.texts[0] }));
  check('B: the rows and the one under the if, which show the message too, changed in place', [2, 3, 4, 5].every((i) => s.same[i] && s.texts[i].endsWith('Saved') && s.recs[i].length > 0), J(s.texts));
  await reset();
  await click('Tick');
  s = await state();
  check('B: a render with the same message writes nothing to it', s.recs[0].length === 0 && s.recs.every((x) => x.length === 0), J(s.recs));
  await reset();
  await click('Change');
  s = await state();
  check('B: a changed message replaces the text of the same node', s.same[0] && s.texts[0] === 'Failed' && s.recs[0].some((x) => x === 'childList +1 -1'), J({ rec: s.recs[0], text: s.texts[0] }));
  await reset();
  await click('Clear');
  s = await state();
  check('B: emptied, it is still the same node, in the tree as a live region, with no tone drawn', s.same[0] && s.texts[0] === '' && (await page.evaluate(() => window.__held[0].getAttribute('data-tone'))) === null && (await statusAx()).length === 7, J({ rec: s.recs[0], ax: (await statusAx()).length }));

  // ---- F. tone and alert -------------------------------------------------------------------------------------------------------------------------
  await click('Fill');
  await reset();
  await click('Warn');
  s = await state();
  check('F: a change of tone is a change of an attribute of the same node', s.same[0] && s.recs[0].includes('attributes:data-tone') && (await page.evaluate(() => window.__held[0].getAttribute('data-tone'))) === 'warn', J(s.recs[0]));
  await reset();
  await click('Ring');
  s = await state();
  check('F: role=alert is kept the same way: the same node, the text added to it', s.same[6] && s.texts[6] === 'Fire' && s.recs[6].some((x) => x.startsWith('childList +1')), J({ rec: s.recs[6], text: s.texts[6] }));

  // ---- C and D. typing, and the field's message --------------------------------------------------------------------------------------------------
  await reset();
  await page.focus('#root input');
  const typed = [];
  for (const ch of 'bad') {
    await page.keyboard.press(ch);
    typed.push(await page.evaluate(() => ({ focus: document.activeElement && document.activeElement.tagName, value: document.activeElement.value, caret: document.activeElement.selectionStart })));
  }
  await sleep(40);
  check('C: typing with a status written at every key keeps the focus, the caret and every character', typed.every((t, i) => t.focus === 'INPUT' && t.value === 'bad'.slice(0, i + 1) && t.caret === i + 1), J(typed));
  s = await state();
  check('C: the toned status is the same node and was written once per key', s.same[0] && s.texts[0] === 'typed bad' && s.recs[0].filter((x) => x.startsWith('childList')).length === 3, J(s.recs[0]));
  const err = await page.evaluate(() => { const i = document.querySelector('#root input'); const e = document.getElementById(i.getAttribute('aria-describedby')); return { invalid: i.getAttribute('aria-invalid'), described: i.getAttribute('aria-describedby'), text: e && e.textContent, role: e && e.getAttribute('role'), held: window.__held[1] === e }; });
  check('D: the field\'s message appeared in its own live region: aria-invalid, aria-describedby pointing at it', err.invalid === 'true' && err.text === 'Not allowed' && err.role === 'status', J(err));
  check('D: and it is the node that was there before, with its id', err.held && s.same[1], J(err));
  const id1 = err.described;
  await page.keyboard.press('x');
  await sleep(40);
  const err2 = await page.evaluate(() => { const i = document.querySelector('#root input'); return { invalid: i.getAttribute('aria-invalid'), described: i.getAttribute('aria-describedby'), text: document.querySelector('#root input').parentNode.querySelector('[role=status]') ? 'x' : '' }; });
  s = await state();
  check('D: when the message is gone the field is valid again, and the node stays (empty) with the same id', err2.invalid === null && err2.described === null && s.same[1] && s.texts[1] === '' && (await page.evaluate(() => window.__held[1].id)) === id1, J(err2));

  // ---- E. rows of a For, and an If ------------------------------------------------------------------------------------------------------------------
  await reset();
  await click('Shrink');
  s = await state();
  const rowsNow = await page.evaluate(() => [...document.querySelectorAll('#root [role=status]')].length);
  check('E: a row that goes is gone, the others keep their node by order', s.same[2] && s.same[3] && s.connected[4] === false && rowsNow === 5, J({ same: s.same, connected: s.connected, rowsNow }));
  await click('Grow');
  const grown = await page.evaluate(() => [...document.querySelectorAll('#root [role=status]')].map((e) => e.textContent));
  check('E: a row that comes is a new node with its text', grown.some((t) => t.startsWith('row d ')), J(grown));
  await click('Hide');
  const hidden = await page.evaluate(() => [...document.querySelectorAll('#root [role=status]')].some((e) => e.textContent.startsWith('under an if')));
  check('E: a status under an If that hides it is gone', hidden === false);
  await click('Show');
  const shownAgain = await page.evaluate(() => { const e = [...document.querySelectorAll('#root [role=status]')].find((x) => x.textContent.startsWith('under an if')); return { there: !!e, old: window.__held[5] === e }; });
  check('E: and when it comes back it is a new node (a region that is inserted with its content), with the text', shownAgain.there && shownAgain.old === false, J(shownAgain));

  check('no script error', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
