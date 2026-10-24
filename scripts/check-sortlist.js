#!/usr/bin/env node
// SortList (docs/drag-and-drop.md, issue #80) in Chromium, in the mobile profile (Pixel 7: touch, 412 px):
//  A. `move` in the page against tests/move_corpus.json, the file `cargo test` runs on replay's;
//  B. the buttons with the keyboard: Tab, Enter and Space; the ends are disabled; the focus follows the row, three moves in a row; the live region;
//  C. a drag with the mouse, with a finger (DevTools touch events on the handle), Escape cancelling, a drop the program refuses;
//  D. a finger on the rest of the row still scrolls the page, and on the handle does not;
//  E. a render in the middle of a drag (a timer) does not lose the drop and runs after it;
//  F. 200 rows; a right-to-left page (the handle at the start); the targets are 44 px;
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-sortlist.js [path/to/cancho-ui]
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
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript' };
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
async function until(fn, ms = 3000) {
  for (let t = 0; t < ms; t += 25) { const v = await fn(); if (v) { return v; } await sleep(25); }
  return null;
}
function build(src, out) {
  const r = spawnSync(CANCHO_UI, ['build', src, '--page', '-o', out], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(`build ${src}: ${r.stdout}${r.stderr}`); }
}
function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const f = path.join(dir, path.normalize(rel));
    if (!f.startsWith(dir) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}
const rowsOf = (n) => Array.from({ length: n }, (_, i) => `Row ${String(i + 1).padStart(3, '0')}`);
const program = (name, rows, extra = '', body = 'tasks = move(tasks, from, to); moves = moves + 1;') => `component ${name} {
    state tasks: List[String] = [${rows.map((r) => `"${r}"`).join(', ')}];
    state moves: Int = 0;
    state ticks: Int = 0;
${extra}
    on Moved(from, to) { ${body} }
    view {
        Column {
            Text { role: heading "List" }
            SortList { label: "Things" items: tasks on: Moved }
            Text { role: status "{moves} moves, {ticks} ticks" }
        }
    }
}
`;

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-sortlist-'));
  const make = (name, src) => { const d = path.join(tmp, name); fs.mkdirSync(d); fs.writeFileSync(path.join(d, 'p.cui'), src); build(path.join(d, 'p.cui'), path.join(d, 'out')); return path.join(d, 'out'); };
  const dirs = {
    four: make('four', program('Four', ['alpha', 'beta', 'gamma', 'delta'])),
    tall: make('tall', program('Tall', rowsOf(40))),
    ticker: make('ticker', program('Ticker', rowsOf(5), '    every 100 Tick;\n    on Tick { ticks = ticks + 1; }')),
    big: make('big', program('Big', rowsOf(200))),
    refuser: make('refuser', program('Refuser', ['alpha', 'beta', 'gamma'], '', 'moves = moves + 1;')),
  };
  const servers = {};
  for (const [k, d] of Object.entries(dirs)) { servers[k] = await serve(d); }
  const url = (k) => `http://localhost:${servers[k].address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  async function open(k, opts = {}) {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block', ...opts });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(String(e)));
    await page.goto(url(k));
    await page.waitForSelector('#root ul.cui-sort li');
    const cdp = await ctx.newCDPSession(page);
    return { ctx, page, problems, cdp };
  }
  const order = (page) => page.evaluate(() => [...document.querySelectorAll('#root ul.cui-sort li')].map((li) => li.getAttribute('data-item')));
  const status = (page) => page.evaluate(() => document.querySelector('#root [role=status]').textContent);
  const active = (page) => page.evaluate(() => { const a = document.activeElement; return a === document.body ? 'BODY' : (a.getAttribute('aria-label') || a.textContent); });
  const live = (page) => page.evaluate(() => (document.getElementById('cui-sort-live') || { textContent: '' }).textContent);
  // read in the page, in one task: the 'ticker' program draws the page again every 100 ms, and a locator that resolved a handle and then asked for its box
  // found null when the rebuild fell between the two (7 runs in 25, with the compiler of main as well, #106)
  const handleBox = (page, i) => page.evaluate((n) => {
    const h = document.querySelectorAll('#root ul.cui-sort li')[n].querySelector('.cui-handle');
    const r = h.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  }, i);
  const center = (b) => ({ x: b.x + b.width / 2, y: b.y + b.height / 2 });
  async function mouseDrag(page, from, to, hold = 0) {
    const a = center(await handleBox(page, from));
    const b = center(await handleBox(page, to));
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(a.x, (a.y + b.y) / 2, { steps: 4 });
    await page.mouse.move(b.x, b.y, { steps: 4 });
    if (hold) { await sleep(hold); }
    await page.mouse.up();
  }
  const touch = (cdp, type, x, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x, y, id: 1 }] });
  async function touchDrag(cdp, from, to) {
    await touch(cdp, 'touchStart', from.x, from.y);
    for (let i = 1; i <= 8; i++) { await touch(cdp, 'touchMove', from.x, from.y + ((to.y - from.y) * i) / 8); await sleep(15); }
    await touch(cdp, 'touchEnd', 0, 0);
  }

  // ---- A. move, in the page ------------------------------------------------------------------------------------------------------------
  {
    const js = fs.readFileSync(path.join(dirs.four, 'app.js'), 'utf8');
    const a = js.indexOf('function __cui_move(');
    check('A: the module carries move', a > 0);
    const b = js.indexOf('\n}\n', a) + 3;
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'move_corpus.json'), 'utf8'));
    const { ctx, page } = await open('four');
    const bad = await page.evaluate(({ src, cases }) => {
      const mv = new Function(`${src}; return __cui_move;`)();
      return cases.filter((c) => { const input = JSON.parse(JSON.stringify(c.list)); const got = mv(input, c.from, c.to); return JSON.stringify(got) !== JSON.stringify(c.want) || JSON.stringify(input) !== JSON.stringify(c.list); }).map((c) => c.why);
    }, { src: js.slice(a, b), cases: corpus.cases });
    check(`A: ${corpus.cases.length} cases of the corpus come out as written, and the list given is never changed`, bad.length === 0, bad.join('; '));
    await ctx.close();
  }

  // ---- B. the keyboard ------------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, problems } = await open('four');
    check('B: the list is named for a screen reader', (await page.evaluate(() => document.querySelector('ul.cui-sort').getAttribute('aria-label'))) === 'Things');
    check('B: the first row cannot go up and the last cannot go down', await page.evaluate(() => { const li = document.querySelectorAll('ul.cui-sort li'); return li[0].querySelectorAll('button')[0].disabled && !li[0].querySelectorAll('button')[1].disabled && li[3].querySelectorAll('button')[1].disabled && !li[3].querySelectorAll('button')[0].disabled; }));
    const names = await page.evaluate(() => [...document.querySelectorAll('ul.cui-sort button')].map((b) => b.getAttribute('aria-label')));
    check('B: each button says which row it moves and where', names[0] === 'Move alpha up' && names[1] === 'Move alpha down' && names[7] === 'Move delta down', JSON.stringify(names.slice(0, 2)));
    // Tab from the top of the page: the heading is no stop, the first enabled button is "Move alpha down"
    await page.keyboard.press('Tab');
    check('B: Tab lands on the first enabled button, "Move alpha down"', (await active(page)) === 'Move alpha down', await active(page));
    await page.keyboard.press('Enter');
    check('B: Enter moves alpha down one place', JSON.stringify(await order(page)) === JSON.stringify(['beta', 'alpha', 'gamma', 'delta']), JSON.stringify(await order(page)));
    check('B: and the focus is on "Move alpha down" of the row where it went', (await active(page)) === 'Move alpha down', await active(page));
    await page.keyboard.press('Space');
    await page.keyboard.press('Enter');
    check('B: Space and Enter again: alpha has gone down two more places, to the end', JSON.stringify(await order(page)) === JSON.stringify(['beta', 'gamma', 'delta', 'alpha']), JSON.stringify(await order(page)));
    check('B: at the end the focus is on "Move alpha up" (its down button is disabled now)', (await active(page)) === 'Move alpha up', await active(page));
    check('B: the live region says where it went', (await live(page)) === 'alpha, position 4 of 4', await live(page));
    check('B: the program counted three moves', (await status(page)).startsWith('3 moves'), await status(page));
    await page.keyboard.press('Enter');
    check('B: and the same button moves it up again', JSON.stringify(await order(page)) === JSON.stringify(['beta', 'gamma', 'alpha', 'delta']) && (await active(page)) === 'Move alpha up', JSON.stringify(await order(page)) + await active(page));
    check('B: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
  }

  // ---- C. a pointer ---------------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, cdp, problems } = await open('four');
    await mouseDrag(page, 3, 0);
    check('C: a mouse drag of the last row to the top', JSON.stringify(await order(page)) === JSON.stringify(['delta', 'alpha', 'beta', 'gamma']), JSON.stringify(await order(page)));
    check('C: the program heard one move', (await status(page)).startsWith('1 moves'), await status(page));
    check('C: the live region says where it went', (await live(page)) === 'delta, position 1 of 4', await live(page));
    check('C: nothing is left dragging (no class, no transform, renders allowed)', await page.evaluate(() => !document.querySelector('.cui-dragging, .cui-over') && !document.querySelector('li[style*=transform]')));
    // Escape cancels
    {
      const a = center(await handleBox(page, 1));
      const b = center(await handleBox(page, 3));
      await page.mouse.move(a.x, a.y);
      await page.mouse.down();
      await page.mouse.move(b.x, b.y, { steps: 6 });
      const during = await page.evaluate(() => ({ dragging: !!document.querySelector('.cui-dragging'), over: !!document.querySelector('.cui-over') }));
      await page.keyboard.press('Escape');
      await page.mouse.up();
      check('C: while dragging the row is marked and so is the place it would go', during.dragging && during.over, JSON.stringify(during));
      check('C: Escape cancels: nothing moved, nothing reported', JSON.stringify(await order(page)) === JSON.stringify(['delta', 'alpha', 'beta', 'gamma']) && (await status(page)).startsWith('1 moves'), JSON.stringify(await order(page)) + await status(page));
    }
    // a drag that ends where it started is no move
    await mouseDrag(page, 2, 2);
    check('C: dropped where it started: no move', (await status(page)).startsWith('1 moves'), await status(page));
    // a finger on the handle
    const from = center(await handleBox(page, 0));
    const to = center(await handleBox(page, 2));
    await touchDrag(cdp, from, to);
    await sleep(150);
    check('C: a finger drags the first row to the third place (DevTools touch events on the handle)', JSON.stringify(await order(page)) === JSON.stringify(['alpha', 'beta', 'delta', 'gamma']), JSON.stringify(await order(page)));
    check('C: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
  }
  {
    const { ctx, page } = await open('refuser');
    await mouseDrag(page, 2, 0);
    check('C: a program that does not change its list refuses the move: the page shows the list as it is', JSON.stringify(await order(page)) === JSON.stringify(['alpha', 'beta', 'gamma']) && (await status(page)).startsWith('1 moves'), JSON.stringify(await order(page)) + await status(page));
    await ctx.close();
  }

  // ---- D. scrolling ---------------------------------------------------------------------------------------------------------------------
  {
    const { ctx, page, cdp } = await open('tall');
    const scrollY = () => page.evaluate(() => window.scrollY);
    const row = await page.locator('#root ul.cui-sort li').nth(3).locator('.cui-sort-text').boundingBox();
    const y0 = row.y + row.height / 2;
    await touch(cdp, 'touchStart', row.x + 10, y0);
    for (let i = 1; i <= 10; i++) { await touch(cdp, 'touchMove', row.x + 10, y0 - i * 20); await sleep(15); }
    await touch(cdp, 'touchEnd', 0, 0);
    await sleep(300);
    check('D: a finger swiping on the text of a row scrolls the page', (await scrollY()) > 20, String(await scrollY()));
    await page.evaluate(() => window.scrollTo(0, 0));
    const before = await order(page);
    const h = center(await handleBox(page, 3));
    await touch(cdp, 'touchStart', h.x, h.y);
    for (let i = 1; i <= 6; i++) { await touch(cdp, 'touchMove', h.x, h.y + i * 20); await sleep(15); }
    await touch(cdp, 'touchEnd', 0, 0);
    await sleep(200);
    check('D: a finger on the handle moves the row and does not scroll the page', (await scrollY()) === 0 && JSON.stringify(await order(page)) !== JSON.stringify(before), `${await scrollY()} ${JSON.stringify((await order(page)).slice(0, 6))}`);
    await ctx.close();
  }

  // ---- E. a render in the middle of a drag ------------------------------------------------------------------------------------------------
  {
    const { ctx, page } = await open('ticker');
    const a = center(await handleBox(page, 0));
    const b = center(await handleBox(page, 3));
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    // from here the page is not redrawn: what it shows now is what it must still show at the end of the drag (the timer ticks every 100 ms, so
    // the row and the count are read after the press, not before it)
    const t0 = await page.evaluate(() => { window.__li = document.querySelector('.cui-dragging'); return Number(document.querySelector('[role=status]').textContent.match(/(\d+) ticks/)[1]); });
    await page.mouse.move(b.x, b.y, { steps: 6 });
    await sleep(700);
    const mid = await page.evaluate(() => ({ same: window.__li.isConnected, ticks: Number(document.querySelector('[role=status]').textContent.match(/(\d+) ticks/)[1]) }));
    check('E: 700 ms into a drag (seven timer ticks) the row being dragged is still in the page', mid.same, JSON.stringify(mid));
    check('E: and the page has not been redrawn meanwhile', mid.ticks === t0, `${mid.ticks} vs ${t0}`);
    await page.mouse.up();
    await sleep(150);
    const after = await status(page);
    check('E: the drop still moved the row', JSON.stringify((await order(page)).slice(0, 4)) === JSON.stringify(['Row 002', 'Row 003', 'Row 004', 'Row 001']), JSON.stringify(await order(page)));
    check('E: and the renders that waited ran after it (the ticks are counted)', Number(after.match(/(\d+) ticks/)[1]) > t0 + 3, after);
    await ctx.close();
  }

  // ---- F. 200 rows, right to left, sizes ------------------------------------------------------------------------------------------------------
  {
    const { ctx, page } = await open('big');
    check('F: 200 rows are drawn', (await order(page)).length === 200);
    const t = Date.now();
    await page.locator('#root ul.cui-sort li').nth(199).scrollIntoViewIfNeeded();
    await page.locator('#root ul.cui-sort li').nth(199).locator('button').nth(0).click();
    check('F: the last row moved up with its button in a list of 200', (await order(page))[198] === 'Row 200', `${Date.now() - t} ms`);
    await ctx.close();
  }
  {
    const { ctx, page } = await open('four');
    const sizes = await page.evaluate(() => { const li = document.querySelectorAll('ul.cui-sort li')[1]; const r = (e) => { const b = e.getBoundingClientRect(); return [Math.round(b.width), Math.round(b.height)]; }; return { handle: r(li.querySelector('.cui-handle')), up: r(li.querySelectorAll('button')[0]), down: r(li.querySelectorAll('button')[1]), row: r(li) }; });
    check('F: the handle and the buttons are at least 44 by 44 px', [sizes.handle, sizes.up, sizes.down].every(([w, h]) => w >= 44 && h >= 44), JSON.stringify(sizes));
    const ta = await page.evaluate(() => ({ handle: getComputedStyle(document.querySelector('.cui-handle')).touchAction, row: getComputedStyle(document.querySelector('ul.cui-sort li')).touchAction, text: getComputedStyle(document.querySelector('.cui-sort-text')).touchAction }));
    check('F: touch-action is none on the handle alone', ta.handle === 'none' && ta.row === 'auto' && ta.text === 'auto', JSON.stringify(ta));
    const ltr = await page.evaluate(() => { const li = document.querySelector('ul.cui-sort li'); return [li.querySelector('.cui-handle').getBoundingClientRect().left, li.querySelector('.cui-sort-text').getBoundingClientRect().left]; });
    await page.evaluate(() => { document.documentElement.dir = 'rtl'; });
    const rtl = await page.evaluate(() => { const li = document.querySelector('ul.cui-sort li'); return [li.querySelector('.cui-handle').getBoundingClientRect().left, li.querySelector('.cui-sort-text').getBoundingClientRect().left]; });
    check('F: the handle is at the start of the row in both directions', ltr[0] < ltr[1] && rtl[0] > rtl[1], JSON.stringify({ ltr, rtl }));
    await mouseDrag(page, 3, 0);
    check('F: and a drag works in a right-to-left page', JSON.stringify(await order(page)) === JSON.stringify(['delta', 'alpha', 'beta', 'gamma']), JSON.stringify(await order(page)));
    await ctx.close();
  }

  await browser.close();
  for (const s of Object.values(servers)) { s.close(); }
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
