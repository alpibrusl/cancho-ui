#!/usr/bin/env node
// What docs/presentation.md section 3a says, measured in Chromium: the options of issue #106 for a status live region on a page that is drawn again
// from nothing at every render (as the compiler's pages are). Four small pages, one render function each:
//   rebuild    the page today: root.textContent = '' and every node is new, the status span included;
//   persist    option 1: the status span is created once, and put into each new tree (its text is written only when it changed);
//   inplace    the ideal of option 1: nothing is removed, only the span's text is written (what a diffing renderer would do);
//   announcer  option 2: the visible span is drawn again as today and one hidden role=status element outside `root` is written when the text changes.
// For each, over the same renders (empty; filled; unrelated render with the same text; changed), it reads: whether the span (or the announcer) is the same
// DOM node; what a MutationObserver on it sees; the accessibility tree through the DevTools protocol (the live region's properties and whether its
// accessibility node id is the same before and after); whether the focus stays in a text field being typed in; and the time of a render with 200 status texts.
// A screen reader is not measured. Not a check of the compiler. Outside the gate.
//   node scripts/measure-status.js
'use strict';
const { chromium, devices } = require('playwright');

setTimeout(() => { console.error('FAIL the measurement took more than 3 minutes'); process.exit(1); }, 180000).unref();
const say = (k, v) => console.log(`${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`);

const PAGE = (mode) => `<!doctype html><meta charset=utf-8><body>
<div id=root></div>
${mode === 'announcer' ? '<div id=live role=status aria-live=polite style="position:absolute;width:1px;height:1px;overflow:hidden"></div>' : ''}
<script>
const mode = '${mode}';
let text = '', tick = 0, N = 1;
let persistent = null;
const root = document.getElementById('root');
function statusNode(i) {
  if (mode === 'persist' || mode === 'inplace') {
    window.__sp = window.__sp || [];
    if (!window.__sp[i]) { const s = document.createElement('span'); s.setAttribute('role', 'status'); window.__sp[i] = s; }
    return window.__sp[i];
  }
  const s = document.createElement('span'); s.setAttribute('role', mode === 'announcer' ? 'none' : 'status'); return s;
}
function render() {
  if (mode === 'inplace' && root.firstChild && window.__sp.length >= N) { for (let i = 0; i < N; i++) { const s = window.__sp[i]; if (s.textContent !== text) { s.textContent = text; } } return; }
  const active = document.activeElement; const keep = active && active.tagName === 'INPUT' ? { v: active.value, a: active.selectionStart } : null;
  root.textContent = '';
  const col = document.createElement('div');
  const b = document.createElement('button'); b.textContent = 'tick ' + tick; col.appendChild(b);
  const inp = document.createElement('input'); inp.setAttribute('aria-label', 'name'); col.appendChild(inp);
  for (let i = 0; i < N; i++) {
    const s = statusNode(i);
    if (mode === 'persist') { if (s.textContent !== text) { s.textContent = text; } } else { s.textContent = text; }
    col.appendChild(s);
  }
  root.appendChild(col);
  if (mode === 'announcer') { const live = document.getElementById('live'); if (live.textContent !== text) { live.textContent = text; } }
  if (keep) { inp.focus(); inp.value = keep.v; inp.setSelectionRange(keep.a, keep.a); }
}
window.set = (t, n) => { text = t; tick++; if (n) { N = n; } render(); };
render();
</script>`;

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  for (const mode of ['rebuild', 'persist', 'inplace', 'announcer']) {
    const ctx = await browser.newContext({ ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Accessibility.enable');
    await page.setContent(PAGE(mode));
    const target = mode === 'announcer' ? '#live' : '#root [role=status]';
    const ax = async () => {
      const { nodes } = await cdp.send('Accessibility.getFullAXTree');
      const n = nodes.find((x) => !x.ignored && x.role && x.role.value === 'status');
      if (!n) { return null; }
      const prop = (name) => ((n.properties || []).find((p) => p.name === name) || {}).value;
      return { axId: n.nodeId, backend: n.backendDOMNodeId, live: prop('live') && prop('live').value, atomic: prop('atomic') && prop('atomic').value, relevant: prop('relevant') && prop('relevant').value, name: n.name && n.name.value, text: (n.childIds || []).length };
    };
    // the node we hold, the observer on it, and the AX node before
    await page.evaluate((sel) => { window.__held = document.querySelector(sel); window.__recs = []; new MutationObserver((r) => { window.__recs.push(...r.map((x) => x.type + (x.type === 'childList' ? `:+${x.addedNodes.length}-${x.removedNodes.length}` : ''))); }).observe(window.__held, { childList: true, characterData: true, subtree: true, attributes: true }); }, target);
    const out = { mode };
    out.axBefore = await ax();
    // 1. filled later: the first message
    await page.evaluate(() => window.set('Saved'));
    await page.waitForTimeout(50);
    const after1 = await page.evaluate((sel) => ({ same: document.querySelector(sel) === window.__held, connected: window.__held.isConnected, recs: window.__recs.slice() }), target);
    out.filled = { sameDomNode: after1.same, heldIsConnected: after1.connected, observerSaw: after1.recs };
    out.axAfterFill = await ax();
    // 2. a render in which the status text did not change
    await page.evaluate(() => { window.__recs.length = 0; });
    await page.evaluate(() => window.set('Saved'));
    await page.waitForTimeout(50);
    const after2 = await page.evaluate((sel) => ({ same: document.querySelector(sel) === window.__held, recs: window.__recs.slice() }), target);
    out.unrelatedRender = { sameDomNode: after2.same, observerSaw: after2.recs };
    out.axAfterUnrelated = await ax();
    // 3. changed
    await page.evaluate(() => { window.__recs.length = 0; });
    await page.evaluate(() => window.set('Failed'));
    await page.waitForTimeout(50);
    const after3 = await page.evaluate((sel) => ({ same: document.querySelector(sel) === window.__held, recs: window.__recs.slice(), text: document.querySelector(sel).textContent }), target);
    out.changed = { sameDomNode: after3.same, observerSaw: after3.recs, text: after3.text };
    out.axAfterChange = await ax();
    out.axSameNodeAcrossRenders = JSON.stringify([out.axBefore && out.axBefore.axId, out.axAfterFill && out.axAfterFill.axId, out.axAfterUnrelated && out.axAfterUnrelated.axId, out.axAfterChange && out.axAfterChange.axId]);
    // 4. the focus of a text field typed in while a render happens
    if (mode !== 'inplace') {
      await page.focus('#root input');
      await page.keyboard.type('ada');
      await page.evaluate(() => window.set('Typed'));
      await page.keyboard.type('x');
      const f = await page.evaluate(() => ({ focus: document.activeElement && document.activeElement.tagName, value: document.querySelector('#root input').value }));
      out.focusWhileTyping = f;
    } else {
      await page.focus('#root input');
      await page.keyboard.type('ada');
      await page.evaluate(() => window.set('Typed'));
      await page.keyboard.type('x');
      out.focusWhileTyping = await page.evaluate(() => ({ focus: document.activeElement && document.activeElement.tagName, value: document.querySelector('#root input').value }));
    }
    // 5. cost: a render with 200 status texts (median of 15)
    await page.evaluate(() => window.set('', 200));
    const times = [];
    for (let i = 0; i < 15; i++) { times.push(await page.evaluate((i) => { const t0 = performance.now(); window.set(i % 2 ? 'a' : 'b'); return performance.now() - t0; }, i)); }
    times.sort((a, b) => a - b);
    out.msPerRenderWith200Statuses = Number(times[7].toFixed(2));
    say(mode, out);
    await ctx.close();
  }
  await browser.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
