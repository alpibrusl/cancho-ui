#!/usr/bin/env node
// Internationalization (docs/i18n.md, issue #82) in Chromium, in the mobile profile (Pixel 7):
//  A. the page's formatting and plural functions (the adapter the compiler emits, run in the page) against tests/i18n_corpus.json, the answers
//     `Intl` gave when it was written, and against `Intl` itself on input generated here: random numbers, timestamps and counts, ten languages;
//  B. tests/i18n_pages.json, the file `cargo test` replays (crates/cancho-ui/tests/i18n.rs): the same browser language lists, events and
//     language changes, the same text, and the document's lang and dir;
//  C. a right-to-left page: the row of buttons reverses, the table header is aligned to the start, `dir` follows the language both ways;
//  E. a value put into a sentence of the other direction is isolated, so it reads as written;
//  D. examples/i18n as a page and as an installed app with the server stopped, and a page that calls no `Intl`.
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-i18n.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 6 minutes: something never arrived'); process.exit(1); }, 360000).unref();

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const CORPUS = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'i18n_corpus.json'), 'utf8'));
const PAGES = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'i18n_pages.json'), 'utf8'));
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const LANGS = ['en', 'es', 'de', 'fr', 'pt', 'ru', 'ar', 'he', 'ja', 'hi'];

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
async function until(fn, ms = 3000) {
  for (let t = 0; t < ms; t += 25) {
    const v = await fn();
    if (v) { return v; }
    await sleep(25);
  }
  return null;
}
function build(src, out, flags) {
  const r = spawnSync(CANCHO_UI, ['build', src, ...flags, '-o', out], { encoding: 'utf8' });
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
const show = (s) => String(s).replace(/ /g, '<U+202F>').replace(/ /g, '<U+00A0>').replace(/‎/g, '<LRM>').replace(/‏/g, '<RLM>');

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-i18n-'));
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });

  // ---- A. the adapter ---------------------------------------------------------------------------------------------------------------------------
  {
    // a program that speaks all ten languages, so that the module carries all the data
    const lines = ['component All {', '    requires Locale;', '    state s: String = "";', '    messages {'];
    for (const l of LANGS) { lines.push(`        ${l} { k: "x"; }`); }
    lines.push('    }', '    on Go { s = t("k"); }', '    view { Text { "{s}" } }', '}');
    fs.writeFileSync(path.join(tmp, 'all.cui'), lines.join('\n') + '\n');
    build(path.join(tmp, 'all.cui'), path.join(tmp, 'all'), ['--page']);
    const js = fs.readFileSync(path.join(tmp, 'all', 'app.js'), 'utf8');
    const a = js.indexOf('const __CUI_I18N');
    const b = js.indexOf('// Component: All');
    check('A: the module carries the adapter', a > 0 && b > a);
    check('A: and no call to Intl or toLocale*String', !/\bIntl\./.test(js) && !/toLocale/.test(js));
    const adapter = js.slice(a, b);
    const ctx = await browser.newContext({ ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    await page.goto('about:blank');
    await page.evaluate(`window.__i18n = (function () { ${adapter}; return { number: __cui_fmt_number, percent: __cui_fmt_percent, date: __cui_fmt_date, time: __cui_fmt_time, plural: __cui_plural }; })();`);
    const bad = await page.evaluate((corpus) => {
      const out = [];
      const loc = (l) => ({ lang: l });
      let n = 0;
      const chk = (what, got, want, input) => { n++; if (got !== want && out.length < 12) { out.push(`${what} ${input}: got ${JSON.stringify(got)} wanted ${JSON.stringify(want)}`); } };
      for (const e of corpus.numbers) { chk('number', window.__i18n.number(loc(e.lang), e.x, e.digits), e.want, `${e.lang} ${e.x} ${e.digits}`); }
      for (const e of corpus.percents) { chk('percent', window.__i18n.percent(loc(e.lang), e.x), e.want, `${e.lang} ${e.x}`); }
      for (const e of corpus.dates) { chk('date', window.__i18n.date(loc(e.lang), e.ms, e.style), e.want, `${e.lang} ${e.ms} ${e.style}`); }
      for (const e of corpus.times) { chk('time', window.__i18n.time(loc(e.lang), e.ms), e.want, `${e.lang} ${e.ms}`); }
      for (const l of Object.keys(corpus.plurals)) { corpus.counts.forEach((c, i) => chk('plural', window.__i18n.plural(l, c), corpus.plurals[l][i], `${l} ${c}`)); }
      return { n, out };
    }, CORPUS);
    check(`A: ${bad.n} answers of the corpus (numbers, percents, dates, times, plural categories) are what Intl said`, bad.out.length === 0, bad.out.map(show).join(' | '));

    // fresh input against Intl in this browser: the corpus cannot have been fitted to it
    const fresh = await page.evaluate((LANGS) => {
      let seed = 20261024;
      const rnd = () => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed / 0x7fffffff; };
      const out = [];
      let n = 0;
      const note = (s) => { if (out.length < 12) { out.push(s); } };
      for (const l of LANGS) {
        const loc = { lang: l };
        for (let i = 0; i < 400; i++) {
          const mag = 10 ** Math.floor(rnd() * 14 - 5);
          let x = Number(((rnd() - 0.3) * mag).toPrecision(1 + Math.floor(rnd() * 15)));
          if (x !== 0 && Math.abs(x) < 1e-6) { continue; }
          const d = Math.floor(rnd() * 7);
          const want = new Intl.NumberFormat(l, { minimumFractionDigits: d, maximumFractionDigits: d }).format(x);
          const got = window.__i18n.number(loc, x, d);
          n++;
          if (got !== want) { note(`number ${l} ${x} ${d}: ${JSON.stringify(got)} vs ${JSON.stringify(want)}`); }
          const wantP = new Intl.NumberFormat(l, { style: 'percent', maximumFractionDigits: 0 }).format(x);
          const gotP = window.__i18n.percent(loc, x);
          n++;
          if (gotP !== wantP) { note(`percent ${l} ${x}: ${JSON.stringify(gotP)} vs ${JSON.stringify(wantP)}`); }
        }
        for (let i = 0; i < 300; i++) {
          const ms = Math.floor(-30610224000000 + rnd() * (253402300799999 + 30610224000000)) + (rnd() < 0.2 ? 0.5 : 0);
          for (const style of ['short', 'medium', 'long', 'full']) {
            const want = new Intl.DateTimeFormat(l, { dateStyle: style, timeZone: 'UTC' }).format(Math.trunc(ms));
            const got = window.__i18n.date(loc, ms, style);
            n++;
            if (got !== want) { note(`date ${l} ${ms} ${style}: ${JSON.stringify(got)} vs ${JSON.stringify(want)}`); }
          }
          const wantT = new Intl.DateTimeFormat(l, { timeStyle: 'short', timeZone: 'UTC' }).format(Math.trunc(ms));
          n++;
          if (window.__i18n.time(loc, ms) !== wantT) { note(`time ${l} ${ms}: ${JSON.stringify(window.__i18n.time(loc, ms))} vs ${JSON.stringify(wantT)}`); }
        }
        const pr = new Intl.PluralRules(l);
        const counts = Array.from({ length: 3001 }, (_, i) => i).concat([1e4, 1e5 + 1, 1e6, 2e6, 3e6, 1e6 + 1, 1e7, 123456789, 1e9, 1e12, 2 ** 40, 2 ** 52]);
        for (const c of counts) { n++; if (window.__i18n.plural(l, c) !== pr.select(c)) { note(`plural ${l} ${c}: ${window.__i18n.plural(l, c)} vs ${pr.select(c)}`); } }
      }
      return { n, out };
    }, LANGS);
    check(`A: ${fresh.n} answers on fresh input (random numbers at 0 to 6 places, percents, 7,500 timestamps in the years 1000 to 9999, counts to 3000 and large ones) are what Intl answers here`, fresh.out.length === 0, fresh.out.map(show).join(' | '));
    check('A: the edges are empty text and not a throw', await page.evaluate(() => {
      const L = { lang: 'en' };
      const f = window.__i18n;
      return [f.number(L, 1e21, 2), f.number(L, NaN, 2), f.number(L, 1, 7), f.number(L, 1, 0.5), f.number(L, 'x', 2), f.date(L, 1e15, 'short'), f.date(L, 0, 'tiny'), f.date(L, NaN, 'short'), f.time(L, -1e15), f.percent(L, Infinity)].every((s) => s === '')
        && f.number(L, 1e-7, 2) === '0.00' && f.number(L, -0.001, 2) === '-0.00';
    }));
    await ctx.close();
  }

  // ---- B. the pages corpus, in the page --------------------------------------------------------------------------------------------------
  const pageDir = path.join(tmp, 'pages');
  fs.mkdirSync(pageDir);
  fs.writeFileSync(path.join(pageDir, 'demo.cui'), PAGES.program.join('\n') + '\n');
  build(path.join(pageDir, 'demo.cui'), path.join(pageDir, 'out'), ['--page']);
  const server = await serve(path.join(pageDir, 'out'));
  const base = `http://localhost:${server.address().port}/`;
  async function open(locales, extra = {}) {
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block', ...extra });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(String(e)));
    await page.addInitScript((ls) => {
      Object.defineProperty(navigator, 'languages', { get: () => ls, configurable: true });
      Object.defineProperty(navigator, 'language', { get: () => ls[0], configurable: true });
    }, locales);
    await page.goto(base);
    await page.waitForSelector('#root button');
    return { ctx, page, problems };
  }
  const doc = (page) => page.evaluate(() => ({ lang: document.documentElement.lang, dir: document.documentElement.dir }));
  const shownText = (page) => page.evaluate(() => {
    const e = [...document.querySelectorAll('#root *')].find((x) => x.children.length === 0 && / \| /.test(x.textContent));
    return e ? e.textContent : '';
  });
  {
    let ci = 0;
    for (const c of PAGES.cases) {
      ci++;
      const { ctx, page, problems } = await open(c.locales);
      let failed = null;
      const wrong = (i, what) => { if (!failed) { failed = `step ${i}: ${what}`; } };
      const d0 = await doc(page);
      if (JSON.stringify(d0) !== JSON.stringify(c.start)) { wrong(-1, `the document starts as ${JSON.stringify(d0)}, wanted ${JSON.stringify(c.start)}`); }
      let typed = null;
      for (const [i, s] of c.steps.entries()) {
        if (failed) { break; }
        if (s.count !== undefined && s.count !== typed) {
          await page.getByRole('textbox', { name: 'Count' }).fill(String(s.count));
          typed = s.count;
        }
        await page.getByRole('button', { name: s.do, exact: true }).click();
        await sleep(30);
        if (s.shown !== undefined) {
          const ok = await until(async () => (await shownText(page)) === s.shown);
          if (!ok) { wrong(i, `the page shows ${show(await shownText(page))}, wanted ${show(s.shown)}`); }
        }
        const d = await doc(page);
        if (JSON.stringify(d) !== JSON.stringify(s.document)) { wrong(i, `the document is ${JSON.stringify(d)}, wanted ${JSON.stringify(s.document)}`); }
      }
      if (!failed && problems.length) { failed = problems.join('; '); }
      check(`B${ci}: ${c.why.slice(0, 100)}`, !failed, failed || '');
      await ctx.close();
    }
  }

  // ---- C. right to left -------------------------------------------------------------------------------------------------------------------
  {
    const lay = async (locales) => {
      const { ctx, page } = await open(locales);
      const r = await page.evaluate(() => {
        const left = (name) => Math.round([...document.querySelectorAll('#root button')].find((b) => b.textContent === name).getBoundingClientRect().left);
        const th = document.querySelector('#root [role=columnheader], #root th');
        const cell = th ? th.getBoundingClientRect() : null;
        const range = document.createRange();
        if (th) { range.selectNodeContents(th); }
        const text = th ? range.getBoundingClientRect() : null;
        return {
          order: ['English', 'Arabic', 'Russian', 'Japanese'].map(left),
          align: th ? getComputedStyle(th).textAlign : null,
          thEdgeGap: th ? { start: Math.round(text.left - cell.left), end: Math.round(cell.right - text.right) } : null,
          dir: document.documentElement.dir,
          bodyDir: getComputedStyle(document.body).direction,
          vw: document.documentElement.clientWidth, sw: document.documentElement.scrollWidth,
        };
      });
      return { ctx, page, r };
    };
    const ltr = await lay(['en']);
    const rtl = await lay(['ar-EG']);
    check('C: LTR: the buttons run left to right', ltr.r.order[0] < ltr.r.order[1] && ltr.r.order[1] < ltr.r.order[2], JSON.stringify(ltr.r));
    check('C: RTL: the same Row of buttons runs right to left, with no change to the program or the stylesheet', rtl.r.order[0] > rtl.r.order[1] && rtl.r.order[1] > rtl.r.order[2], JSON.stringify(rtl.r));
    check('C: dir is rtl in Arabic and ltr in English, on the document and as the body computes it', rtl.r.dir === 'rtl' && rtl.r.bodyDir === 'rtl' && ltr.r.dir === 'ltr' && ltr.r.bodyDir === 'ltr');
    await ltr.ctx.close();
    // the table header: aligned to the start of its cell, whichever way that is (the text of a role=columnheader Text; no <th> in this page)
    const probe = await rtl.page.evaluate(() => ({ cols: document.querySelectorAll('#root [role=columnheader]').length, tables: document.querySelectorAll('#root table').length }));
    await rtl.page.evaluate(() => { const t = document.createElement('table'); t.className = 'cui-table'; t.innerHTML = '<tr><th>Name</th><th>Qty</th></tr><tr><td>pump</td><td>3</td></tr>'; document.body.appendChild(t); });
    const th = await rtl.page.evaluate(() => {
      const el = document.querySelector('table.cui-table th');
      const cell = el.getBoundingClientRect();
      const range = document.createRange();
      range.selectNodeContents(el);
      const text = range.getBoundingClientRect();
      return { align: getComputedStyle(el).textAlign, startGap: Math.round(cell.right - text.right), endGap: Math.round(text.left - cell.left) };
    });
    check('C: in RTL a .cui-table header text sits at the start (right) of its cell', th.align === 'start' && th.startGap <= th.endGap, JSON.stringify({ probe, th }));
    // switching: English then Arabic, the document follows both ways and the row reverses and returns
    await rtl.page.getByRole('button', { name: 'English', exact: true }).click();
    await sleep(60);
    const back = await rtl.page.evaluate(() => ({ dir: document.documentElement.dir, order: ['English', 'Arabic', 'Russian'].map((n) => Math.round([...document.querySelectorAll('#root button')].find((b) => b.textContent === n).getBoundingClientRect().left)) }));
    check('C: switching Arabic to English turns the page left to right again', back.dir === 'ltr' && back.order[0] < back.order[1] && back.order[1] < back.order[2], JSON.stringify(back));
    check('C: no horizontal scroll at 412 px in either direction', ltr.r.sw <= ltr.r.vw + 1 && rtl.r.sw <= rtl.r.vw + 1, JSON.stringify([ltr.r.sw, ltr.r.vw, rtl.r.sw, rtl.r.vw]));
    await rtl.ctx.close();
  }
  server.close();

  // ---- D. examples/i18n --------------------------------------------------------------------------------------------------------------------
  {
    const dir = path.join(tmp, 'ex');
    build(path.join(ROOT, 'examples', 'i18n', 'i18n.cui'), dir, ['--pwa']);
    const s = await serve(dir);
    const url = `http://localhost:${s.address().port}/`;
    const ctx = await browser.newContext({ ...devices['Pixel 7'], locale: 'ru-RU' });
    const page = await ctx.newPage();
    const problems = [];
    page.on('pageerror', (e) => problems.push(String(e)));
    await page.goto(url);
    await page.waitForSelector('#root button');
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    const text = () => page.evaluate(() => document.querySelector('#root').innerText.replace(/[\u2068\u2069]/g, '').replace(/\s+/g, ' '));
    check('D: the browser prefers Russian: the page starts in it, with the plural form of 3 and the formats of Russian', /Заказы/.test(await text()) && /3 заказа/.test(await text()) && /Итого: 1 234,50/.test(await text()) && /26 %/.test(await text()), await text());
    check('D: the document is ru ltr', JSON.stringify(await page.evaluate(() => [document.documentElement.lang, document.documentElement.dir])) === '["ru","ltr"]');
    await page.getByRole('button', { name: 'العربية' }).click();
    await sleep(60);
    const ar = await text();
    check('D: Arabic: three orders is "3 طلبات", the document turns right to left', /3 طلبات/.test(ar) && (await page.evaluate(() => document.documentElement.dir)) === 'rtl', ar);
    for (let i = 0; i < 8; i++) { await page.getByRole('button', { name: 'طلب آخر' }).click(); }
    await sleep(60);
    check('D: eleven orders in Arabic take the "many" form', /11 طلبًا/.test(await text()), await text());
    await page.getByRole('button', { name: 'English' }).click();
    await sleep(60);
    check('D: English: 11 orders, the American date and 12-hour time', /11 orders/.test(await text()) && /Total: 1,234.50/.test(await text()) && /Due October 24, 2026 at 3:05 PM/.test(await text()), await text());
    // the live region carries the count only
    const live = await page.evaluate(() => [...document.querySelectorAll('#root [role=status]')].map((e) => e.textContent));
    check('D: the order count is the one live region', live.length === 1 && /11 orders/.test(live[0]), JSON.stringify(live));
    // offline: the data is in the page, not fetched
    await new Promise((ok) => { s.closeAllConnections(); s.close(ok); });
    await page.reload();
    await page.waitForSelector('#root button', { timeout: 5000 });
    check('D: with the server stopped the installed page starts and formats (nothing is fetched)', /Заказы/.test(await text()) && /26 %/.test(await text()), await text());
    check('D: no page error', problems.length === 0, problems.join('; '));
    await ctx.close();
  }

  // ---- E. a hole is isolated: a Latin value with punctuation inside an Arabic sentence (and the reverse) reads as written -----------------------
  {
    fs.writeFileSync(path.join(tmp, 'bidi.cui'), 'component B {\n    requires Locale;\n    messages {\n        ar { h: "الطلب {id} جاهز."; }\n        en { h: "Order {id} is ready."; }\n    }\n    view { Column { Text { t("h", "(B)") } Text { t("h", "C#7!") } Text { t("h", "-5") } } }\n}\n');
    build(path.join(tmp, 'bidi.cui'), path.join(tmp, 'bidi'), ['--page']);
    const s = await serve(path.join(tmp, 'bidi'));
    for (const lang of ['ar', 'en']) {
      const ctx = await browser.newContext({ ...devices['Pixel 7'], locale: lang });
      const page = await ctx.newPage();
      await page.goto(`http://localhost:${s.address().port}/`);
      await page.waitForSelector('#root p, #root span, #root div');
      const rows = await page.evaluate(() => [...document.querySelectorAll('#root *')].filter((e) => e.children.length === 0 && /⁨/.test(e.textContent)).map((e) => {
        const node = e.firstChild;
        const t = node.textContent;
        const a = t.indexOf('⁨') + 1;
        const b = t.indexOf('⁩');
        const xs = [];
        for (let i = a; i < b; i++) { const r = document.createRange(); r.setStart(node, i); r.setEnd(node, i + 1); xs.push(Math.round(r.getBoundingClientRect().left)); }
        return { value: t.slice(a, b), xs, ltr: xs.every((x, i) => i === 0 || x > xs[i - 1]) };
      }));
      check(`E: in ${lang === 'ar' ? 'an Arabic' : 'an English'} sentence the values (B), C#7! and -5 read left to right, as written`, rows.length === 3 && rows.every((r) => r.ltr), JSON.stringify(rows));
      await ctx.close();
    }
    s.close();
  }

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
