#!/usr/bin/env node
// What docs/drag-and-drop.md section 2 says, measured in Chromium (Pixel 7 profile: touch, 412 px) and by driving a page the way the
// compiler's own pages are driven (the whole view is rebuilt after every state change). Not a check of the compiler. Outside the gate.
//   node scripts/measure-dnd.js
'use strict';
const { chromium, devices } = require('playwright');
setTimeout(() => { console.error('FAIL the measurement took more than 3 minutes'); process.exit(1); }, 180000).unref();
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const say = (k, v) => console.log(`${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`);

const LIST = `<style>li{height:48px;margin:2px;border:1px solid #888;list-style:none;display:flex;align-items:center}</style>
<ul id="u"><li id="a" draggable="true">A</li><li id="b" draggable="true">B</li><li id="c" draggable="true">C</li></ul>
<script>
window.log = [];
for (const li of document.querySelectorAll('li')) {
  for (const t of ['dragstart','dragenter','dragover','drop','dragend','pointerdown','pointermove','pointerup','pointercancel','touchstart','touchmove','touchend','touchcancel','contextmenu','click']) {
    li.addEventListener(t, (e) => { if (t === 'dragover' || t === 'drop') { e.preventDefault(); } window.log.push(t + ':' + li.id); });
  }
}
</script>`;

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();
  const cdp = await ctx.newCDPSession(page);
  const summarize = (log) => { const out = []; for (const e of log) { if (out[out.length - 1] !== e && !(out.length && out[out.length - 1].split(':')[0] === e.split(':')[0] && e.startsWith('pointermove') ) ) { out.push(e); } } return out.slice(0, 24); };
  const touch = async (type, x, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x, y, id: 1 }] });

  // (a) HTML5 drag and drop with a mouse
  await page.setContent(LIST);
  const b = async (id) => { const r = await page.locator('#' + id).boundingBox(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; };
  await page.mouse.move((await b('a')).x, (await b('a')).y);
  await page.mouse.down();
  await page.mouse.move((await b('c')).x, (await b('c')).y, { steps: 5 });
  await page.mouse.up();
  await sleep(100);
  say('mouse on draggable items: the events, in order', summarize(await page.evaluate(() => window.log)));

  // (b) a touch drag on a draggable item: pressed and moved at once, and pressed for 700 ms first
  for (const hold of [0, 700]) {
    await page.setContent(LIST);
    const a = await b('a');
    const c = await b('c');
    await touch('touchStart', a.x, a.y);
    await sleep(hold);
    for (let i = 1; i <= 8; i++) { await touch('touchMove', a.x, a.y + ((c.y - a.y) * i) / 8); await sleep(20); }
    await touch('touchEnd', 0, 0);
    await sleep(200);
    say(`touch on draggable items, held ${hold} ms before moving: the events`, summarize(await page.evaluate(() => window.log)));
  }

  // (c) the same touch drag with pointer events, as a page that draws its own drag would hear it
  await page.setContent(LIST.replace('draggable="true"', '').replace(/draggable="true"/g, '').replace('<ul', '<ul style="touch-action:none"'));
  {
    const a = await b('a');
    const c = await b('c');
    await touch('touchStart', a.x, a.y);
    for (let i = 1; i <= 8; i++) { await touch('touchMove', a.x, a.y + ((c.y - a.y) * i) / 8); await sleep(20); }
    await touch('touchEnd', 0, 0);
    await sleep(150);
    say('touch with touch-action:none, no draggable: pointer events', summarize(await page.evaluate(() => window.log)));
  }
  await page.setContent(LIST.replace(/draggable="true"/g, ''));
  {
    const a = await b('a');
    const c = await b('c');
    await touch('touchStart', a.x, a.y);
    for (let i = 1; i <= 8; i++) { await touch('touchMove', a.x, a.y + ((c.y - a.y) * i) / 8); await sleep(20); }
    await touch('touchEnd', 0, 0);
    await sleep(150);
    say('touch with the default touch-action, no draggable: pointer events', summarize(await page.evaluate(() => window.log)));
  }

  // (d) the source is removed from the page while it is being dragged (what a rebuild after a state change does)
  await page.setContent(LIST);
  await page.evaluate(() => { document.getElementById('a').addEventListener('dragstart', () => { setTimeout(() => { document.getElementById('u').innerHTML = '<li id="a2" draggable="true">A</li><li id="b2">B</li><li id="c2">C</li>'; window.log.push('REBUILT'); }, 50); }); });
  await page.mouse.move((await b('a')).x, (await b('a')).y);
  await page.mouse.down();
  await page.mouse.move((await b('c')).x, (await b('c')).y, { steps: 6 });
  await sleep(200);
  await page.mouse.up();
  await sleep(100);
  say('the list is rebuilt 50 ms into a mouse drag: the events', summarize(await page.evaluate(() => window.log)));
  say('  ... and a dragend reached the new source', (await page.evaluate(() => window.log)).includes('dragend:a2'));

  // (e) keyboard: can an item with draggable=true be dragged from the keyboard?
  await page.setContent(LIST.replace(/<li /g, '<li tabindex="0" '));
  await page.focus('#a');
  await page.keyboard.press('Space');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await sleep(100);
  say('keyboard on a focused draggable item (Space, ArrowDown, Enter): drag events', (await page.evaluate(() => window.log)).filter((e) => /^drag|^drop/.test(e)));

  // (f) buttons as the alternative: a page rebuilt the way the compiler's pages are; where does the focus go after "Up" on the third row?
  await page.setContent('<div id="root"></div>');
  await page.evaluate(() => {
    let items = ['alpha', 'beta', 'gamma'];
    const root = document.getElementById('root');
    function render() {
      root.textContent = '';
      const ul = document.createElement('ul');
      items.forEach((it, i) => {
        const li = document.createElement('li');
        li.appendChild(document.createTextNode(it + ' '));
        const up = document.createElement('button');
        up.textContent = 'Up ' + it;
        up.onclick = () => { if (i > 0) { const t = items[i - 1]; items[i - 1] = items[i]; items[i] = t; render(); } };
        li.appendChild(up);
        ul.appendChild(li);
      });
      root.appendChild(ul);
    }
    render();
  });
  await page.getByRole('button', { name: 'Up gamma' }).focus();
  await page.keyboard.press('Enter');
  await sleep(50);
  say('"Up gamma" pressed with the keyboard on a list rebuilt from scratch: the focus is on', await page.evaluate(() => document.activeElement === document.body ? 'body (lost)' : document.activeElement.textContent));

  // (g) a drag handle: the touch target and the long press of the browser
  say('Pixel 7 profile', { hasTouch: devices['Pixel 7'].hasTouch, viewport: devices['Pixel 7'].viewport });
  await browser.close();
})().catch((e) => { console.error(e); process.exit(1); });
