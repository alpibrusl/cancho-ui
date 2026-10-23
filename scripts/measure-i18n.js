#!/usr/bin/env node
// What docs/i18n.md section 2 says about Intl, plural rules and right-to-left, measured: the same grid of calls in Chromium (the engine
// of the page) and in node (another ICU: another CLDR), and the layout of a page with dir="rtl". Outside the gate.
//   node scripts/measure-i18n.js            prints the numbers of the note
//   node scripts/measure-i18n.js --json     the grids, as JSON
'use strict';
const path = require('path');
const { chromium, devices } = require('playwright');
setTimeout(() => { console.error('FAIL the measurement took more than 2 minutes'); process.exit(1); }, 120000).unref();

const LOCALES = ['en', 'en-GB', 'de', 'fr', 'es', 'pt-BR', 'ru', 'pl', 'tr', 'ja', 'hi', 'ar', 'ar-EG', 'he', 'fa'];
// the grid; a function so that it can be shipped to the page as text
function grid() {
  const out = { numbers: {}, dates: {}, plurals: {}, ordinals: {}, lists: {}, info: {} };
  const values = [0, 1, -0.5, 1234.5, 1234567.891, 0.256];
  const when = Date.UTC(2026, 9, 24, 15, 5, 9);
  for (const l of LOCALES) {
    out.numbers[l] = values.map((v) => new Intl.NumberFormat(l).format(v))
      .concat([new Intl.NumberFormat(l, { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(1234.5)])
      .concat([new Intl.NumberFormat(l, { style: 'percent' }).format(0.256)])
      .concat([new Intl.NumberFormat(l, { style: 'currency', currency: 'EUR' }).format(1234.5)]);
    out.dates[l] = ['short', 'medium', 'long', 'full'].map((s) => new Intl.DateTimeFormat(l, { dateStyle: s, timeZone: 'UTC' }).format(when))
      .concat([new Intl.DateTimeFormat(l, { timeStyle: 'short', timeZone: 'UTC' }).format(when)])
      .concat([new Intl.DateTimeFormat(l, { dateStyle: 'medium', timeStyle: 'medium', timeZone: 'UTC' }).format(when)]);
    const pr = new Intl.PluralRules(l);
    out.plurals[l] = Array.from({ length: 112 }, (_, n) => pr.select(n)).join(',');
    const po = new Intl.PluralRules(l, { type: 'ordinal' });
    out.ordinals[l] = Array.from({ length: 25 }, (_, n) => po.select(n)).join(',');
    out.lists[l] = new Intl.ListFormat(l, { type: 'conjunction' }).format(['a', 'b', 'c']);
    out.info[l] = { categories: pr.resolvedOptions().pluralCategories.join(','), numberingSystem: new Intl.NumberFormat(l).resolvedOptions().numberingSystem, calendar: new Intl.DateTimeFormat(l).resolvedOptions().calendar };
  }
  out.zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  out.noTimeZoneGiven = new Intl.DateTimeFormat('en', { dateStyle: 'short', timeStyle: 'short' }).format(Date.UTC(2026, 9, 24, 23, 30));
  return out;
}
const GRID_SRC = `(() => { const LOCALES = ${JSON.stringify(LOCALES)}; return (${grid.toString()})(); })()`;

function diff(a, b) {
  const rows = [];
  for (const sect of ['numbers', 'dates', 'plurals', 'ordinals', 'lists', 'info']) {
    for (const l of LOCALES) {
      const x = JSON.stringify(a[sect][l]);
      const y = JSON.stringify(b[sect][l]);
      if (x !== y) { rows.push({ sect, l, chromium: a[sect][l], node: b[sect][l] }); }
    }
  }
  return rows;
}
const show = (s) => JSON.stringify(s).replace(/ /g, '<U+202F>').replace(/ /g, '<U+00A0>').replace(/‏/g, '<RLM>').replace(/؜/g, '<ALM>');

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();
  await page.goto('about:blank');
  const inChromium = await page.evaluate(GRID_SRC);
  const inNode = eval(GRID_SRC);
  const ua = await page.evaluate(() => navigator.userAgent);
  console.log(`chromium ${browser.version()} / node ${process.version} (ICU ${process.versions.icu}, CLDR ${process.versions.cldr})`);
  const d = diff(inChromium, inNode);
  const cells = (g) => Object.values(g.numbers).flat().length + Object.values(g.dates).flat().length;
  console.log(`grid: ${LOCALES.length} locales, ${cells(inChromium)} formatted strings, 112 plural categories and 25 ordinals per locale`);
  console.log(`locale rows that differ between the two engines: ${d.length} of ${LOCALES.length * 6}`);
  for (const r of d) { console.log(`  DIFF ${r.sect} ${r.l}: chromium ${show(r.chromium)}\n                 node     ${show(r.node)}`); }
  console.log('numbers, de:', show(inChromium.numbers.de));
  console.log('numbers, fr:', show(inChromium.numbers.fr));
  console.log('numbers, ar:', show(inChromium.numbers.ar), '| ar-EG:', show(inChromium.numbers['ar-EG']));
  console.log('numbers, hi:', show(inChromium.numbers.hi));
  console.log('dates, en:', show(inChromium.dates.en));
  console.log('dates, ar:', show(inChromium.dates.ar));
  console.log('dates, ja:', show(inChromium.dates.ja));
  console.log('plural categories:', Object.fromEntries(LOCALES.map((l) => [l, inChromium.info[l].categories])));
  console.log('numbering systems:', Object.fromEntries(LOCALES.map((l) => [l, inChromium.info[l].numberingSystem])));
  console.log('time zone of this machine:', inChromium.zone, '| a date with no timeZone given:', show(inChromium.noTimeZoneGiven));
  const tzPage = await (await browser.newContext({ timezoneId: 'Pacific/Auckland', locale: 'de-DE' })).newPage();
  await tzPage.goto('about:blank');
  console.log('same call, another time zone and browser locale:', show(await tzPage.evaluate(() => new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'short' }).format(Date.UTC(2026, 9, 24, 23, 30)))));
  console.log('Intl.NumberFormat(undefined) in that page:', show(await tzPage.evaluate(() => new Intl.NumberFormat().format(1234.5))));

  // right to left: the same markup, dir ltr and rtl
  const markup = '<div class="cui-row" id="row"><button>One</button><button>Two</button><button>Three</button></div><table class="cui-table" id="t" style="border-collapse:collapse;width:100%"><tr><th style="text-align:left">Name</th><th style="text-align:left">Qty</th></tr><tr><td style="text-align:left">pump</td><td style="text-align:left">3</td></tr></table><input id="f" value="12:30 ab"><div id="flex" style="display:flex;flex-direction:column;gap:.5rem"><span id="s" style="margin-left:8px">margin-left</span><span id="e" style="margin-inline-start:8px">margin-inline-start</span></div>';
  const css = '.cui-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; } body{margin:0;font:16px system-ui}';
  const lay = async (dir) => {
    const p = await ctx.newPage();
    await p.setContent(`<!doctype html><html lang="${dir === 'rtl' ? 'ar' : 'en'}" dir="${dir}"><style>${css}</style><body>${markup}</body></html>`);
    const r = await p.evaluate(() => {
      const x = (e) => Math.round(document.querySelector(e).getBoundingClientRect().left);
      const b = [...document.querySelectorAll('#row button')].map((e) => Math.round(e.getBoundingClientRect().left));
      const w = document.documentElement.clientWidth;
      return { viewport: w, buttonsLeftEdges: b, thLeft: x('#t th'), thRight: Math.round(document.querySelector('#t th').getBoundingClientRect().right), tableRight: Math.round(document.querySelector('#t').getBoundingClientRect().right), fieldAlign: getComputedStyle(document.querySelector('#f')).textAlign, mlEdge: x('#s'), misEdge: x('#e'), scrollW: document.documentElement.scrollWidth };
    });
    await p.close();
    return r;
  };
  const ltr = await lay('ltr');
  const rtl = await lay('rtl');
  console.log('layout ltr:', JSON.stringify(ltr));
  console.log('layout rtl:', JSON.stringify(rtl));
  const sw = await ctx.newPage();
  await sw.setContent('<!doctype html><html lang="en"><body>x</body></html>');
  console.log('lang/dir settable at run time:', await sw.evaluate(() => { document.documentElement.lang = 'ar'; document.documentElement.dir = 'rtl'; return [document.documentElement.lang, document.documentElement.dir, getComputedStyle(document.body).direction]; }));
  await browser.close();
  if (process.argv.includes('--json')) { console.log(JSON.stringify({ chromium: inChromium, node: inNode }, null, 1)); }
})().catch((e) => { console.error(e); process.exit(1); });

// ---- bidirectional isolation of a hole (docs/i18n.md section 8.3): does a value inside a sentence of the other direction read as written? ------
if (process.argv.includes('--bidi')) {
  (async () => {
    const browser = await chromium.launch({ args: ['--no-proxy-server'] });
    const page = await browser.newPage();
    const rows = await page.evaluate(() => {
      const out = [];
      const cases = [['rtl', ['A-12', 'C#7!', '(B)', '12', '-5', 'x.y', 'Ada Lovelace']], ['ltr', ['علي', '(ب)', 'ب!', '-٥', 'ب 12']]];
      for (const [dir, ids] of cases) {
        for (const id of ids) {
          for (const iso of [false, true]) {
            const el = document.createElement('p');
            el.dir = dir;
            el.style.cssText = 'font: 20px sans-serif; width: 600px';
            const FSI = iso ? '⁨' : '';
            const PDI = iso ? '⁩' : '';
            const text = dir === 'rtl' ? `الطلب ${FSI}${id}${PDI} جاهز.` : `Order ${FSI}${id}${PDI} is ready.`;
            el.textContent = text;
            document.body.appendChild(el);
            const node = el.firstChild;
            const start = text.indexOf(id);
            const xs = [...id].map((_, i) => { const r = document.createRange(); r.setStart(node, start + i); r.setEnd(node, start + i + 1); return Math.round(r.getBoundingClientRect().left); });
            // as written: left to right for a value that starts with a Latin letter, digit or sign, right to left for an Arabic one
            const rtlValue = /^[؀-ۿ(]/.test(id) && /[؀-ۿ]/.test(id);
            out.push({ dir, id, isolated: iso, readsAsWritten: xs.every((x, i) => i === 0 || (rtlValue ? x < xs[i - 1] : x > xs[i - 1])), xs });
            el.remove();
          }
        }
      }
      return out;
    });
    for (const r of rows) { console.log(`bidi ${r.dir} ${JSON.stringify(r.id)} isolated=${r.isolated}: reads as written ${r.readsAsWritten} (x ${r.xs.join(',')})`); }
    await browser.close();
  })().catch((e) => { console.error(e); process.exit(1); });
}
