#!/usr/bin/env node
// Browser-side check of examples/ops-console (docs/ops-console.md): the integrated page, driven as an operator would, in
// Chromium against the stand-in service of scripts/ops-console-server.js. Screenshots go to the directory named by
// OPS_SHOTS (default: a temp directory, printed). Outside the gate: needs node, the `playwright` package with a Chromium,
// and a built `cancho-ui`.
//   node scripts/check-ops-console.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');
const { createServer, machines } = require('./ops-console-server.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-ops-console-'));
  const shots = process.env.OPS_SHOTS || path.join(dir, 'shots');
  fs.mkdirSync(shots, { recursive: true });
  const built = spawnSync(CANCHO_UI, ['build', path.join(ROOT, 'examples/ops-console/ops-console.cui'), '--pwa', '-o', path.join(dir, 'out')], { encoding: 'utf8' });
  if (built.status !== 0) { throw new Error(built.stderr); }
  const behaviour = {};
  const server = await createServer(path.join(dir, 'out'), behaviour);
  const log = server.log;
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage({ viewport: { width: 420, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.clock.install({ time: 0 });

  // the tone of the status text that shows the result of a write: its attribute, the prefix the stylesheet draws, its colour
  const toneOf = async (pg) => pg.evaluate(() => {
    const el = Array.from(document.querySelectorAll('#root span')).find((e) => (e.textContent || '').startsWith('result:'));
    return `${el.getAttribute('data-tone')}:${getComputedStyle(el, '::before').content.replace(/^"|"$/g, '')}:${getComputedStyle(el).color}`;
  });
  const texts = async () => page.evaluate(() => Array.from(document.querySelectorAll('#root span')).map((e) => e.textContent).concat(Array.from(document.querySelectorAll('#root tbody tr')).filter((tr) => tr.querySelector('td')).map((tr) => Array.from(tr.querySelectorAll('td')).filter((td) => !td.querySelector('button')).map((td) => td.textContent).join('  '))));
  const rows = async () => (await texts()).filter((t) => /^M-\d+ /.test(t));
  const names = async () => (await rows()).map((t) => t.split('  ')[1]);
  const temps = async () => (await rows()).map((t) => parseFloat(t.split('  ')[3]));
  const status = async (prefix) => (await texts()).find((t) => t.startsWith(prefix));
  const until = async (fn, ms = 4000) => { for (let t = 0; t < ms; t += 40) { if (await fn()) { return true; } await page.waitForTimeout(40); } return false; };
  const sorted = (a, d) => a.every((v, i) => i === 0 || (d > 0 ? a[i - 1] <= v : a[i - 1] >= v));
  const all = machines();
  const ascTemps = all.map((m) => m.temp).sort((a, b) => a - b);

  await page.goto(url);
  await page.waitForSelector('#root *');
  check('the list loads on start, 23 of 23, page 1 of 5', await until(async () => (await texts()).includes('23 of 23 machines, page 1 of 5')), JSON.stringify((await texts()).slice(0, 4)));
  check('the first page is the five lowest temperatures, in numeric order (not text order)', JSON.stringify(await temps()) === JSON.stringify(ascTemps.slice(0, 5)), JSON.stringify(await temps()));
  await page.screenshot({ path: path.join(shots, '1-loaded.png') });

  // filter
  await page.fill('input[aria-label="Search by name"]', 'PRESS');
  check('typing PRESS keeps the three presses, case-insensitively, and resets the page', await until(async () => (await texts()).includes('3 of 23 machines, page 1 of 1')) && (await names()).every((n) => /^Press /.test(n)), JSON.stringify(await names()));
  check('the filtered rows are still ordered by temperature', sorted(await temps(), 1), JSON.stringify(await temps()));
  await page.screenshot({ path: path.join(shots, '2-filtered.png') });
  await page.fill('input[aria-label="Search by name"]', 'zzz');
  check('a query that matches nothing shows no rows and says so', await until(async () => (await texts()).includes('0 of 23 machines, page 1 of 1')) && (await rows()).length === 0);
  await page.fill('input[aria-label="Search by name"]', '');
  check('clearing the query brings all 23 back', await until(async () => (await texts()).includes('23 of 23 machines, page 1 of 5')));

  // sort direction and paging
  await page.click('button[aria-label^="Sort by temperature"]');
  check('the other direction lists the five highest first', await until(async () => JSON.stringify(await temps()) === JSON.stringify(ascTemps.slice(-5).reverse())), JSON.stringify(await temps()));
  const first = await rows();
  await page.click('button[aria-label="Next page"]');
  check('Next goes to page 1 with different rows, still descending', await until(async () => (await texts()).includes('23 of 23 machines, page 2 of 5')) && JSON.stringify(await rows()) !== JSON.stringify(first) && sorted(await temps(), -1));
  for (let i = 0; i < 8; i++) { await page.click('button[aria-label="Next page"]'); }
  await page.waitForTimeout(150);
  const last = (await texts()).find((t) => /^23 of 23 machines, page/.test(t));
  check('Next stops at the last page (23 rows, 5 per page: page 5 of 5, three rows)', last === '23 of 23 machines, page 5 of 5' && (await rows()).length === 3, `${last} ${(await rows()).length}`);
  for (let i = 0; i < 8; i++) { await page.click('button[aria-label="Previous page"]'); }
  await page.waitForTimeout(150);
  check('Previous stops at page 1', (await texts()).includes('23 of 23 machines, page 1 of 5') && JSON.stringify(await rows()) === JSON.stringify(first));
  await page.click('button[aria-label^="Sort by temperature"]');
  await until(async () => JSON.stringify(await temps()) === JSON.stringify(ascTemps.slice(0, 5)));

  // a superseded request is aborted
  behaviour.slow = true;
  const getsBefore = log.gets;
  await page.click('button[aria-label="Refresh"]');
  check('a slow refresh starts and the page says it is loading', await until(async () => log.gets === getsBefore + 1) && (await texts()).includes('status: loading'));
  await page.click('button[aria-label="Refresh"]');
  check('a second refresh supersedes it: the service sees the first connection close', await until(async () => log.aborted === 1), String(log.aborted));
  check('and the second answer is shown', await until(async () => (await texts()).includes('status: updated')));
  server.release();

  // choosing a machine by clicking its row (issue #41): the row names the machine, the field is filled by the click
  const firstId = (await rows())[0].split('  ')[0];
  check('every row on the page has a button named by its machine', (await page.locator('button[aria-label^="Select M-"]').count()) === (await rows()).length, String(await page.locator('button[aria-label^="Select M-"]').count()));
  await page.click(`button[aria-label="Select ${firstId}"]`);
  check('clicking a row puts its machine id in the id field', await until(async () => (await page.inputValue('input[aria-label="Machine id"]')) === firstId), `${firstId} / ${await page.inputValue('input[aria-label="Machine id"]')}`);

  // posting a change
  await page.fill('input[aria-label="Machine id"]', 'M-101');
  await page.fill('input[aria-label="Setpoint"]', '80');
  await page.click('button[aria-label="Apply"]');
  check('a valid change is sent as JSON and answered 201', await until(async () => (await texts()).includes('result: 201 applied')) && /json/.test(log.posts[0].type) && log.posts[0].body === '["M-101", 80]', JSON.stringify(log.posts[0]));
  check('a 201 is shown with the tone ok, and the tone has a prefix as well as a colour', await toneOf(page) === 'ok:\u2713 :rgb(26, 127, 55)', await toneOf(page));
  await page.click('button[aria-label="Refresh"]');
  await page.fill('input[aria-label="Search by name"]', 'press 1');
  check('after a refresh the new value is in the list', await until(async () => (await rows()).some((t) => t.includes('M-101') && t.endsWith('  80'))), JSON.stringify(await rows()));
  await page.fill('input[aria-label="Search by name"]', '');
  await page.fill('input[aria-label="Machine id"]', 'M-107');
  await page.click('button[aria-label="Apply"]');
  check('a locked machine answers 409 and the page shows why', await until(async () => (await texts()).includes('result: 409 locked by maintenance')));
  check('a 409 is shown with the tone warn', await toneOf(page) === 'warn:\u26a0 :rgb(154, 103, 0)', await toneOf(page));
  await page.screenshot({ path: path.join(shots, '3-conflict.png') });
  behaviour.busy = true;
  await page.fill('input[aria-label="Machine id"]', 'M-102');
  await page.click('button[aria-label="Apply"]');
  check('a busy service answers 503 and the page shows it', await until(async () => (await texts()).includes('result: 503 busy, try again')));
  check('a 503 is shown with the tone error', await toneOf(page) === 'error:\u2717 :rgb(207, 34, 46)', await toneOf(page));
  behaviour.busy = false;
  await page.fill('input[aria-label="Machine id"]', 'M-999');
  await page.click('button[aria-label="Apply"]');
  check('an unknown machine answers 400', await until(async () => (await texts()).some((t) => t.startsWith('result: 400'))));
  await page.fill('input[aria-label="Machine id"]', 'M-102');
  await page.fill('input[aria-label="Setpoint"]', 'abc');
  await page.click('button[aria-label="Apply"]');
  check('a setpoint that is not a number is sent as written: the page does not validate (400)', await until(async () => (await texts()).some((t) => t.startsWith('result: 400'))), String(log.posts.length));

  // the refresh timer, and a hidden page
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1);
  const setVisibility = (state) => page.evaluate((st) => {
    Object.defineProperty(document, 'visibilityState', { value: st, configurable: true });
    document.dispatchEvent(new Event('visibilitychange'));
  }, state);
  let g = log.gets;
  await page.clock.runFor(5000);
  check('the page refreshes by itself every 5 seconds', await until(async () => log.gets === g + 1), `${log.gets - g}`);
  await setVisibility('hidden');
  g = log.gets;
  await page.clock.runFor(30000);
  await page.waitForTimeout(250);
  check('a hidden page does not poll', log.gets === g, `${log.gets - g}`);
  await setVisibility('visible');
  check('shown again: one catch-up refresh, not six', await until(async () => log.gets === g + 1) && (await page.waitForTimeout(300), log.gets === g + 1), `${log.gets - g}`);

  // the service goes away
  behaviour.down = true;
  await page.click('button[aria-label="Refresh"]');
  check('with the service down the status says so and the list is kept', await until(async () => (await texts()).some((x) => /^status: error: network error/.test(x))) && (await rows()).length === 5);
  await page.fill('input[aria-label="Machine id"]', 'M-102');
  await page.fill('input[aria-label="Setpoint"]', '50');
  behaviour.down = false;
  behaviour.busy = false;
  await page.click('button[aria-label="Apply"]');
  await until(async () => (await texts()).includes('result: 201 applied'));
  await page.screenshot({ path: path.join(shots, '4-end.png') });
  check('no script errors', errors.length === 0, errors.join(' | '));

  // ---------------------------------------------------------------- phase 2: awkward data, in a fresh page, real clock
  const p2 = await browser.newPage({ viewport: { width: 420, height: 900 } });
  const errors2 = [];
  p2.on('pageerror', (e) => errors2.push(String(e)));
  const texts2 = async () => p2.evaluate(() => Array.from(document.querySelectorAll('#root span')).map((e) => e.textContent).concat(Array.from(document.querySelectorAll('#root tbody tr')).filter((tr) => tr.querySelector('td')).map((tr) => Array.from(tr.querySelectorAll('td')).filter((td) => !td.querySelector('button')).map((td) => td.textContent).join('  '))));
  const rows2 = async () => (await texts2()).filter((t) => /^M-\d+ /.test(t));
  const until2 = async (fn, ms = 4000) => { for (let t = 0; t < ms; t += 40) { if (await fn()) { return true; } await p2.waitForTimeout(40); } return false; };
  const total2 = async () => (await texts2()).find((t) => /machines, page/.test(t));
  const click2 = (label) => p2.click(`button[aria-label="${label}"]`);
  const walk = async (dirClicks) => { const out = []; for (let i = 0; i < 8; i++) { out.push(...(await rows2())); await click2('Next page'); await p2.waitForTimeout(40); } for (let i = 0; i < 8; i++) { await click2('Previous page'); } return out; };
  await p2.goto(url);
  await until2(async () => (await total2()) === '23 of 23 machines, page 1 of 5');

  // the list shrinks under an open page
  for (let i = 0; i < 4; i++) { await click2('Next page'); }
  check('on the last page before the list shrinks', (await total2()) === '23 of 23 machines, page 5 of 5' && (await rows2()).length === 3);
  behaviour.limit = 20;
  await click2('Refresh');
  check('the list shrinks to 20: the page returns to 0 instead of showing an empty page 5', await until2(async () => (await total2()) === '20 of 20 machines, page 1 of 4' && (await rows2()).length === 5), String(await total2()));
  behaviour.limit = 0;

  // awkward records: markup, a malformed line, a line that is not JSON, a blank line, no temperature, a bad one, null
  behaviour.extra = [
    '{"id":"M-200","name":"<img src=x onerror=window.__pwned=1>","state":"idle","temp":1}',
    '{"id":"M-203","name":"Truncated","state":"idl', 'not json at all', '',
    '{"id":"M-204","name":"No temp","state":"idle"}', '{"id":"M-205","name":"Bad temp","state":"idle","temp":"ERR"}', '{"id":"M-206","name":"Null temp","state":"idle","temp":null}',
    '{"id":"M-207","name":"Pumpe \u00dcnit","state":"idle","temp":2}', '{"id":"M-208","name":"pumpe \u00fcnit","state":"idle","temp":3}',
  ];
  await click2('Refresh');
  check('a truncated line, a line that is not JSON and a blank line are dropped: 23 + 6 records, not 23 + 9', await until2(async () => (await total2()) === '29 of 29 machines, page 1 of 6'), String(await total2()));
  const seen = await walk();
  check('markup in a name is shown as text and runs nothing', seen.some((t) => t.includes('<img src=x onerror=window.__pwned=1>')) && (await p2.evaluate(() => window.__pwned)) === undefined);
  const lastAsc = seen.slice(-3);
  check('ascending, the three records without a usable temperature come last', lastAsc.every((t) => /No temp|Bad temp|Null temp/.test(t)), JSON.stringify(lastAsc));
  check('a missing temperature is shown as "no reading", a bad one as it was sent', lastAsc.some((t) => t.endsWith('no reading')) && lastAsc.some((t) => t.endsWith('ERR')), JSON.stringify(lastAsc));
  await click2('Sort by temperature, other direction');
  const seenDesc = await walk();
  check('descending, they are still last (the sentinel 1e9 is added after the direction is applied)', seenDesc.slice(-3).every((t) => /No temp|Bad temp|Null temp/.test(t)), JSON.stringify(seenDesc.slice(-3)));
  await click2('Sort by temperature, other direction');
  await p2.fill('input[aria-label="Search by name"]', '\u00fcnit');
  await p2.waitForTimeout(200);
  check('KNOWN GAP #1: lower() is ASCII only, so searching \u00fcnit does not find \u00dcnit', JSON.stringify((await rows2()).map((t) => t.split('  ')[1])) === JSON.stringify(['pumpe \u00fcnit']), JSON.stringify(await rows2()));
  await p2.fill('input[aria-label="Search by name"]', '');
  behaviour.extra = [];

  behaviour.crlf = true;
  await click2('Refresh');
  check('lines ending in CRLF are read as the same 23 records', await until2(async () => (await total2()) === '23 of 23 machines, page 1 of 5') && !(await rows2()).some((t) => t.includes('\r')));
  behaviour.crlf = false;

  // the form
  await p2.fill('input[aria-label="Machine id"]', 'M-102');
  await p2.fill('input[aria-label="Setpoint"]', '55');
  let posts = log.posts.length;
  await p2.press('input[aria-label="Setpoint"]', 'Enter');
  await p2.waitForTimeout(300);
  await p2.waitForTimeout(300);
  check('Enter in the setpoint field sends the write ONCE (`submit: Apply`, issue #36, fixed)', log.posts.length === posts + 1, String(log.posts.length - posts));
  posts = log.posts.length;
  await p2.dblclick('button[aria-label="Apply"]');
  await p2.waitForTimeout(400);
  check('a double click sends ONE write: Apply is `disabled: sending` while one is in flight (issue #35, fixed)', log.posts.length === posts + 1, String(log.posts.length - posts));
  await p2.fill('input[aria-label="Machine id"]', 'M-102", 99, "x');
  posts = log.posts.length;
  await click2('Apply');
  await until2(async () => log.posts.length === posts + 1);
  check('text typed into the id field cannot change the payload: the id is taken from the rows, an unknown one is sent as ""', log.posts[log.posts.length - 1].body === '["", 55]', log.posts[log.posts.length - 1].body);
  check('and the service refuses it', await until2(async () => (await texts2()).some((t) => t.startsWith('result: 400'))));
  await p2.fill('input[aria-label="Machine id"]', 'M-102');
  await p2.fill('input[aria-label="Setpoint"]', 'abc');
  await click2('Apply');
  check('a setpoint that is not a number blanks the id, so the service refuses it with 400 (the page cannot refuse to send)', await until2(async () => (await texts2()).includes('result: 400 unknown machine or setpoint')) && log.posts[log.posts.length - 1].body === '["", 0]', log.posts[log.posts.length - 1].body);

  // presentation (issue #42, docs/presentation.md): visible labels, a table, a gap, a width, tones with contrast
  const fieldLabels = await p2.evaluate(() => Array.from(document.querySelectorAll('label.cui-field')).map((l) => [l.firstChild.textContent, l.querySelector('input').getAttribute('aria-label'), l.getBoundingClientRect().height > 0]));
  check('every input has a visible <label> with its name', fieldLabels.length === 3 && fieldLabels.every(([t, a, v]) => t === a && t.length > 0 && v), JSON.stringify(fieldLabels));
  check('and the label names the input for assistive technology', (await p2.getByLabel('Machine id', { exact: true }).count()) === 1 && (await p2.getByRole('textbox', { name: 'Setpoint', exact: true }).count()) === 1);
  const table = p2.getByRole('table', { name: 'Machines', exact: true });
  check('the machines are a real table, named', (await table.count()) === 1);
  const heads = await table.getByRole('columnheader').allTextContents();
  check('with its five column headers', JSON.stringify(heads) === JSON.stringify(['id', 'name', 'state', 'temperature (C)', 'select']), JSON.stringify(heads));
  const dataRows = await table.getByRole('row').count();
  const cellsInFirst = await table.getByRole('row').nth(1).getByRole('cell').count();
  check('a row per machine on the page (and the header), five cells each', dataRows === 6 && cellsInFirst === 5, `${dataRows} rows, ${cellsInFirst} cells`);
  const aligned = await p2.evaluate(() => {
    const t = document.querySelector('table');
    const heads = Array.from(t.querySelectorAll('th')).map((h) => h.getBoundingClientRect().left);
    return Array.from(t.querySelectorAll('tbody tr')).filter((r) => r.querySelector('td')).every((r) => Array.from(r.querySelectorAll('td')).every((c, i) => Math.abs(c.getBoundingClientRect().left - heads[i]) < 1));
  });
  check('every cell starts where its column header starts: the columns align', aligned);
  const aria = await table.ariaSnapshot();
  check('the accessibility tree of the table reads columns and rows', /columnheader "id"/.test(aria) && /columnheader "temperature \(C\)"/.test(aria) && /row "M-101/.test(aria) === false && /cell/.test(aria), aria.split('\n').slice(0, 8).join(' / '));
  const gapPx = await p2.evaluate(() => {
    const a = document.querySelector('button[aria-label="Previous page"]').getBoundingClientRect();
    const b = document.querySelector('button[aria-label="Next page"]').getBoundingClientRect();
    return b.left - a.right;
  });
  check('the Previous and Next buttons no longer touch', gapPx >= 4, `${gapPx}px apart`);
  await p2.setViewportSize({ width: 1280, height: 800 });
  const wide = await p2.evaluate(() => {
    const col = document.querySelector('#root > .cui-col');
    const probe = document.createElement('div');
    probe.style.width = '90ch';
    col.appendChild(probe);
    const limit = probe.getBoundingClientRect().width;
    col.removeChild(probe);
    const box = col.getBoundingClientRect();
    const input = document.querySelector('input').getBoundingClientRect();
    return { limit, width: box.width, left: box.left, input: input.width, scroll: document.documentElement.scrollWidth, inner: window.innerWidth };
  });
  check('at 1280 px the page is no wider than 90 characters and is centred', wide.width <= wide.limit + 1 && wide.left > 20 && wide.input <= wide.limit + 1, JSON.stringify(wide));
  await p2.screenshot({ path: path.join(shots, '5-wide.png') });
  await p2.setViewportSize({ width: 420, height: 900 });
  const narrow = await p2.evaluate(() => ({ scroll: document.documentElement.scrollWidth, inner: window.innerWidth }));
  check('at 420 px nothing scrolls sideways', narrow.scroll <= narrow.inner, JSON.stringify(narrow));
  await p2.screenshot({ path: path.join(shots, '6-narrow.png') });
  // a tone must be readable in light and in dark: contrast against the page's own background (WCAG AA is 4.5)
  const ratio = (a, b) => {
    const lum = ([r, g, bl]) => { const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(bl); };
    const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
    return (hi + 0.05) / (lo + 0.05);
  };
  const rgb = (c) => c.match(/\d+/g).slice(0, 3).map(Number);
  for (const scheme of ['light', 'dark']) {
    await p2.emulateMedia({ colorScheme: scheme });
    const colours = await p2.evaluate(() => {
      const out = {};
      const canvas = document.createElement('div');
      canvas.style.background = 'Canvas';
      document.body.appendChild(canvas);
      out.bg = getComputedStyle(canvas).backgroundColor;
      canvas.remove();
      for (const tone of ['ok', 'warn', 'error']) {
        const t = document.createElement('span');
        t.setAttribute('data-tone', tone);
        document.body.appendChild(t);
        out[tone] = getComputedStyle(t).color;
        t.remove();
      }
      return out;
    });
    for (const tone of ['ok', 'warn', 'error']) {
      const r = ratio(rgb(colours[tone]), rgb(colours.bg));
      check(`tone ${tone} has contrast ${r.toFixed(2)}:1 on the ${scheme} background (>= 4.5)`, r >= 4.5);
    }
  }
  await p2.emulateMedia({ colorScheme: 'light' });
  check('no script errors in the second page', errors2.length === 0, errors2.join(' | '));
  await p2.close();

  // ---------------------------------------------------------------- phase 3: typing while the timer refreshes (fake clock)
  const p3 = await browser.newPage({ viewport: { width: 420, height: 900 } });
  await p3.clock.install({ time: 0 });
  await p3.goto(url);
  await p3.waitForSelector('#root span');
  const total3 = async () => p3.evaluate(() => (Array.from(document.querySelectorAll('#root span')).map((e) => e.textContent).find((t) => /machines, page/.test(t))));
  for (let t = 0; t < 4000 && (await total3()) !== '23 of 23 machines, page 1 of 5'; t += 40) { await p3.waitForTimeout(40); }
  await p3.clock.pauseAt((await p3.evaluate(() => Date.now())) + 1);
  await p3.focus('input[aria-label="Search by name"]');
  await p3.keyboard.type('Fur', { delay: 20 });
  const g3 = log.gets;
  await p3.clock.runFor(5000);
  for (let t = 0; t < 4000 && log.gets === g3; t += 40) { await p3.waitForTimeout(40); }
  await p3.waitForTimeout(300);
  await p3.keyboard.type('nace', { delay: 20 });
  await p3.waitForTimeout(200);
  const typed = await p3.evaluate(() => ({ value: document.querySelector('input[aria-label="Search by name"]').value, focused: document.activeElement && document.activeElement.getAttribute('aria-label') }));
  check('typing carries on across a refresh that lands in the middle of it: the text and the focus survive', typed.value === 'Furnace' && typed.focused === 'Search by name', JSON.stringify(typed));
  check('and the filtered list is the three furnaces', (await total3()) === '3 of 23 machines, page 1 of 1', String(await total3()));
  await p3.close();

  // ---------------------------------------------------------------- phase 4: the service is gone, with the service worker (real clock)
  // (Playwright's setOffline does not reach a service worker's own requests, so the outage is the server stopping.)
  const ctx4 = await browser.newContext({ viewport: { width: 420, height: 900 } });
  const p4 = await ctx4.newPage();
  await p4.goto(url);
  await p4.evaluate(() => navigator.serviceWorker.ready.then(() => true));
  await p4.waitForTimeout(500);
  await p4.reload();
  await p4.waitForSelector('#root span');
  server.closeAllConnections();
  await new Promise((done) => server.close(done));
  await p4.reload();
  await p4.waitForSelector('#root span', { timeout: 8000 });
  let offline = '';
  for (let t = 0; t < 4000; t += 80) { offline = await p4.evaluate(() => Array.from(document.querySelectorAll('#root span')).map((e) => e.textContent).join(' | ')); if (/status: error/.test(offline)) { break; } await p4.waitForTimeout(80); }
  check('with the service stopped, the installed page still opens from the service worker', /Machines/.test(offline), offline.slice(0, 120));
  check('and it shows the list saved by the earlier session, with a status that says the read failed (#44, fixed in the program: docs/adapters.md section 19)', /status: error: network error \(showing the last saved list/.test(offline) && /23 of 23 machines/.test(offline), offline.slice(0, 200));
  await p4.screenshot({ path: path.join(shots, '6-offline.png') });
  await ctx4.close();

  await browser.close();
  console.log(`screenshots: ${shots}`);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
