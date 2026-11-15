#!/usr/bin/env node
// Browser-side checks of a table that draws only the rows in view (docs/window.md, section 4). Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
// Chromium only.
//   node scripts/check-window.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'release', 'cancho-ui');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const N = 3000;

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail !== undefined && !ok ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

function serve(dir) {
  const server = http.createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
    const file = path.join(dir, path.normalize(rel));
    if (!file.startsWith(dir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

const ROWS = Array.from({ length: N }, (_, i) => `"row-${String(i).padStart(4, '0')}"`).join(', ');
// the window, and the same 3,000 rows drawn all at once, for the measurement
const WINDOW = `component Rows {
    state rows: List[String] = [${ROWS}];
    state first: Int = 0;
    state events: Int = 0;
    state bumps: Int = 0;
    on Scrolled(at) { first = at; events = events + 1; }
    on Bump { bumps = bumps + 1; }
    on Jump { first = 5000; }
    view {
        Column {
            Button { label: "Bump" on: Bump }
            Button { label: "Past the end" on: Jump }
            Text { "events {events} bumps {bumps} first {first}" }
            Table {
                label: "Rows"
                sticky: "header"
                max_height: 12
                row_height: 2
                total: len(rows)
                first: first
                on: Scrolled
                Row { Text { role: columnheader "name" } Text { role: columnheader numeric: true "length" } }
                For { over: take(skip(rows, first), 16) as: r
                    Row { Text { "{r}" } Text { numeric: true "{len(r)}" } }
                }
            }
        }
    }
}
`;
const ALL = `component Rows {
    state rows: List[String] = [${ROWS}];
    view {
        Table {
            label: "Rows"
            sticky: "header"
            max_height: 12
            Row { Text { role: columnheader "name" } Text { role: columnheader numeric: true "length" } }
            For { over: rows as: r
                Row { Text { "{r}" } Text { numeric: true "{len(r)}" } }
            }
        }
    }
}
`;

function build(dir, name, source) {
  const file = path.join(dir, `${name}.cui`);
  fs.writeFileSync(file, source);
  const out = path.join(dir, name);
  const r = spawnSync(CANCHO_UI, ['build', file, '--pwa', '-o', out], { encoding: 'utf8' });
  if (r.status !== 0) { throw new Error(r.stderr); }
  return out;
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-window-'));
  const server = await serve(build(dir, 'win', WINDOW));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage({ viewport: { width: 700, height: 600 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`http://localhost:${server.address().port}/`);
  await page.waitForSelector('#root table');

  const box = '.cui-table-scroll';
  const info = () => page.evaluate(() => {
    const b = document.querySelector('.cui-table-scroll');
    const body = document.querySelector('tbody');
    const rows = [...body.children];
    const data = rows.filter((r) => !r.hasAttribute('aria-hidden') && r.querySelector('td'));
    const head = body.querySelector('th').closest('tr').getBoundingClientRect();
    const bb = b.getBoundingClientRect();
    return {
      scrollHeight: b.scrollHeight, clientHeight: b.clientHeight, scrollTop: b.scrollTop, headH: head.height,
      trs: rows.length, drawn: data.length, hidden: rows.filter((r) => r.hasAttribute('aria-hidden')).length,
      firstText: data[0] && data[0].children[0].textContent, lastText: data[data.length - 1] && data[data.length - 1].children[0].textContent,
      firstTop: data[0] ? data[0].getBoundingClientRect().top - bb.top - b.clientTop : null,
      headTop: body.querySelector('th').getBoundingClientRect().top - bb.top - b.clientTop,
      rowH: data[0] ? data[0].getBoundingClientRect().height : null,
      rowcount: document.querySelector('table').getAttribute('aria-rowcount'),
      firstIndex: data[0] && data[0].getAttribute('aria-rowindex'), headIndex: body.querySelector('th').closest('tr').getAttribute('aria-rowindex'),
    };
  });
  const status = () => page.evaluate(() => [...document.querySelectorAll('#root span, #root p, #root div')].map((e) => e.textContent).find((t) => /^events /.test(t)) || '');
  const waitFirst = (n) => page.waitForFunction((k) => [...document.querySelectorAll('#root span, #root p, #root div')].some((e) => new RegExp(`^events \\d+ bumps \\d+ first ${k}$`).test(e.textContent)), n, { timeout: 8000 });
  const font = await page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).fontSize));
  const ROW = 2 * font;

  const a = await info();
  check('the DOM holds the header, sixteen rows and two spacers, not 3,000', a.trs === 1 + 16 + 2 && a.drawn === 16 && a.hidden === 2, JSON.stringify(a));
  check('every body row is exactly 2 rem high', Math.abs(a.rowH - ROW) <= 0.5, String(a.rowH));
  check('the box scrolls the height of 3,000 rows (and the header)', Math.abs(a.scrollHeight - (N * ROW + a.headH)) <= 1.5, `${a.scrollHeight} vs ${N * ROW + a.headH}`);
  check('aria-rowcount is 3,001 and the header is row 1 and the first row 2', a.rowcount === '3001' && a.headIndex === '1' && a.firstIndex === '2', JSON.stringify([a.rowcount, a.headIndex, a.firstIndex]));

  // scroll to row 1,500
  await page.evaluate((y) => { document.querySelector('.cui-table-scroll').scrollTop = y; }, 1500 * ROW);
  await waitFirst(1500);
  const b = await info();
  check('at row 1,500 the first row drawn is the 1,500th and is the first under the header', b.firstText === 'row-1500' && Math.abs(b.firstTop - b.headH) <= 1 && b.drawn === 16, JSON.stringify(b));
  check('its row number for assistive technology is 1,502', b.firstIndex === '1502', b.firstIndex);
  check('the sticky header is still at the top of the box', Math.abs(b.headTop) <= 0.5, String(b.headTop));
  check('the scroll position survived the redraw', Math.abs(b.scrollTop - 1500 * ROW) <= 1, String(b.scrollTop));

  // a redraw that is not a scroll does not move the box
  const before = (await info()).scrollTop;
  await page.getByRole('button', { name: 'Bump' }).click();
  await page.waitForFunction(() => /bumps 1/.test(document.body.textContent));
  const after = await info();
  check('a state change that is not a scroll does not move the box', Math.abs(after.scrollTop - before) <= 1 && after.firstText === 'row-1500', JSON.stringify([before, after.scrollTop, after.firstText]));

  // less than a row fires nothing
  const e0 = await status();
  await page.evaluate((y) => { document.querySelector('.cui-table-scroll').scrollTop = y; }, 1500 * ROW + ROW / 2);
  await page.waitForTimeout(150);
  const e1 = await status();
  check('a scroll of half a row fires no event', e0 === e1, `${e0} | ${e1}`);

  // 100 steps of 30 rows from the top to 3,000: one event for each
  await page.evaluate(() => { document.querySelector('.cui-table-scroll').scrollTop = 0; });
  await waitFirst(0);
  const ev0 = Number((await status()).match(/events (\d+)/)[1]);
  for (let s = 1; s <= 99; s++) {
    await page.evaluate((y) => { document.querySelector('.cui-table-scroll').scrollTop = y; }, s * 30 * ROW);
    await waitFirst(s * 30);
  }
  const ev1 = Number((await status()).match(/events (\d+)/)[1]);
  check('99 steps of 30 rows fire 99 events: not one for each pixel', ev1 - ev0 === 99, String(ev1 - ev0));

  // a `first` past the end is clamped: the box is still as tall as the list
  await page.getByRole('button', { name: 'Past the end' }).click();
  await waitFirst(5000);
  const past = await info();
  check('a first past the end is clamped: the box is still as tall as 3,000 rows', Math.abs(past.scrollHeight - (N * ROW + past.headH)) <= 1.5, JSON.stringify(past));
  // the end
  await page.evaluate(() => { const b = document.querySelector('.cui-table-scroll'); b.scrollTop = b.scrollHeight; });
  await page.waitForFunction(() => document.body.textContent.includes('row-2999') || [...document.querySelectorAll('tbody td')].some((t) => t.textContent === 'row-2999'));
  const z = await info();
  const bottom = await page.evaluate(() => { const b = document.querySelector('.cui-table-scroll'); const rows = [...document.querySelectorAll('tbody tr')].filter((r) => !r.hasAttribute('aria-hidden')); const last = rows[rows.length - 1].getBoundingClientRect(); return { lastText: rows[rows.length - 1].children[0].textContent, gap: b.getBoundingClientRect().bottom - b.clientTop - last.bottom, atEnd: b.scrollHeight - b.clientHeight - b.scrollTop }; });
  check('at the end the last row of the list is drawn and is at the bottom of the box', bottom.lastText === 'row-2999' && bottom.gap <= 20 && bottom.gap >= -20, JSON.stringify(bottom));
  check('and the DOM is still sixteen rows or fewer', z.drawn <= 16, JSON.stringify(z));
  check('no script errors', errors.length === 0, errors.join(' | '));

  // what the window buys: build and lay out 3,000 rows at once against sixteen
  const all = await serve(build(dir, 'all', ALL));
  const timeOf = async (port) => {
    const times = [];
    for (let k = 0; k < 5; k++) {
      const p = await browser.newPage({ viewport: { width: 700, height: 600 } });
      await p.goto(`http://localhost:${port}/`);
      await p.waitForSelector('#root table');
      times.push(await p.evaluate(() => { document.body.offsetHeight; return performance.now(); }));
      await p.close();
    }
    return Math.min(...times);
  };
  const tw = await timeOf(server.address().port);
  const ta = await timeOf(all.address().port);
  console.log(`measured: ready and laid out, minimum of 5: the window ${tw.toFixed(0)} ms, all 3,000 rows ${ta.toFixed(0)} ms`);
  check('the window is ready sooner than the 3,000 rows drawn at once', tw < ta, `${tw} vs ${ta}`);
  all.close();

  await browser.close();
  server.close();
  console.log(failures.length === 0 ? 'all window checks passed' : `${failures.length} failed`);
  process.exit(failures.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });
