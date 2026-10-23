#!/usr/bin/env node
// Writes tests/i18n_pages.json (docs/i18n.md section 4): one program with three languages and what its `Show` event must put in `shown`
// for several counts, languages and browser language lists. The texts around the numbers are the program's catalogs; every number, date and
// plural is what Chromium's `Intl` answers (the same oracle as tests/i18n_corpus.json), put together here from the catalogs.
//   node scripts/gen-i18n-pages.js
'use strict';
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');
setTimeout(() => { console.error('FAIL the generator took more than 2 minutes'); process.exit(1); }, 120000).unref();

const PROGRAM = [
  'component Demo {',
  '    requires Locale;',
  '    state typed: String = "";',
  '    state count: Int = 0;',
  '    state shown: String = "";',
  '    state ok: Bool = false;',
  '    messages {',
  '        en { title: "Orders"; hello: "Hello, {name}"; orders(n): one "{n} order", other "{n} orders"; }',
  '        ar { title: "الطلبات"; hello: "مرحبا {name}"; orders(n): zero "لا طلبات", one "طلب واحد", two "طلبان", few "{n} طلبات", many "{n} طلبًا", other "{n} طلب"; }',
  '        ru { title: "Заказы"; hello: "Привет, {name}"; orders(n): one "{n} заказ", few "{n} заказа", many "{n} заказов", other "{n} заказа"; }',
  '    }',
  '    on SetCount(text) { typed = text; count = num(text, 0); }',
  '    on English { ok = locale.set("en"); }',
  '    on Arabic { ok = locale.set("ar"); }',
  '    on Russian { ok = locale.set("ru"); }',
  '    on Japanese { ok = locale.set("ja"); }',
  '    on Show {',
  '        shown = t("title") + " | " + t("orders", count) + " | " + t("hello", "Ada") + " | " + fmt.number(1234567.891, 2) + " | " + fmt.percent(0.256) + " | " + fmt.date(1792854309000, "full") + " | " + fmt.time(1792854309000) + " | " + locale.current();',
  '    }',
  '    view {',
  '        Column {',
  '            Text { role: heading t("title") }',
  '            Row {',
  '                Button { label: "English" on: English }',
  '                Button { label: "Arabic" on: Arabic }',
  '                Button { label: "Russian" on: Russian }',
  '                Button { label: "Japanese" on: Japanese }',
  '            }',
  '            TextField { label: "Count" value: typed on: SetCount }',
  '            Button { label: "Show" on: Show }',
  '            Text { "{shown}" }',
  '            Table {',
  '                label: "Orders"',
  '                Row { Text { role: columnheader "Name" } Text { role: columnheader "Qty" } }',
  '                Row { Text { "pump" } Text { fmt.number(1234.5, 1) } }',
  '            }',
  '        }',
  '    }',
  '}',
];
// the Show of a language for a count, built from Intl (this runs in the page)
function shownFor(lang, count) {
  const CAT = {
    en: { title: 'Orders', hello: (n) => `Hello, \u2068${n}\u2069`, orders: { one: (n) => `${n} order`, other: (n) => `${n} orders` } },
    ar: { title: 'الطلبات', hello: (n) => `مرحبا \u2068${n}\u2069`, orders: { zero: () => 'لا طلبات', one: () => 'طلب واحد', two: () => 'طلبان', few: (n) => `${n} طلبات`, many: (n) => `${n} طلبًا`, other: (n) => `${n} طلب` } },
    ru: { title: 'Заказы', hello: (n) => `Привет, \u2068${n}\u2069`, orders: { one: (n) => `${n} заказ`, few: (n) => `${n} заказа`, many: (n) => `${n} заказов`, other: (n) => `${n} заказа` } },
  };
  const c = CAT[lang];
  const cat = new Intl.PluralRules(lang).select(count);
  const when = 1792854309000;
  const nf = new Intl.NumberFormat(lang, { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(1234567.891);
  const pc = new Intl.NumberFormat(lang, { style: 'percent', maximumFractionDigits: 0 }).format(0.256);
  const date = new Intl.DateTimeFormat(lang, { dateStyle: 'full', timeZone: 'UTC' }).format(when);
  const time = new Intl.DateTimeFormat(lang, { timeStyle: 'short', timeZone: 'UTC' }).format(when);
  return [c.title, c.orders[cat](count), c.hello('Ada'), nf, pc, date, time, lang].join(' | ');
}
const SRC = `(${shownFor.toString()})`;

(async () => {
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const page = await browser.newPage();
  await page.goto('about:blank');
  const shown = (lang, count) => page.evaluate(`${SRC}(${JSON.stringify(lang)}, ${count})`);
  const cases = [];
  const docOf = (l) => ({ lang: l, dir: l === 'ar' ? 'rtl' : 'ltr' });
  for (const [lang, counts] of [['en', [0, 1, 2, 21]], ['ar', [0, 1, 2, 3, 10, 11, 99, 100, 101, 102]], ['ru', [0, 1, 2, 5, 11, 21, 22, 25, 101, 111]]]) {
    cases.push({
      why: `the browser prefers ${lang}: the page starts in it, and says the count in its plural forms`,
      locales: [lang === 'en' ? 'en-GB' : lang === 'ar' ? 'ar-EG' : 'ru-RU', 'en'],
      start: docOf(lang),
      steps: [{ do: 'Show', count: 0, shown: await shown(lang, 0), document: docOf(lang) }].concat(
        await Promise.all(counts.slice(1).map(async (n) => ({ do: 'Show', count: n, shown: await shown(lang, n), document: docOf(lang) })))),
    });
  }
  cases.push({
    why: 'a language the page has not (fr, then ja) is passed over for the next one it has; with none, the first catalog is in use',
    locales: ['fr-FR', 'ja', 'ru'],
    start: docOf('ru'),
    steps: [{ do: 'Show', count: 5, shown: await shown('ru', 5), document: docOf('ru') }],
  });
  cases.push({
    why: 'none of the browser\'s languages is one the page has: the first catalog (en)',
    locales: ['fr-FR', 'ja'],
    start: docOf('en'),
    steps: [{ do: 'Show', count: 1, shown: await shown('en', 1), document: docOf('en') }],
  });
  cases.push({
    why: 'locale.set switches the language at once, answers false for one the page has not, and the document follows (lang, then dir)',
    locales: ['en'],
    start: docOf('en'),
    steps: [
      { do: 'Show', count: 3, shown: await shown('en', 3), document: docOf('en') },
      { do: 'Arabic', ok: true, document: docOf('ar') },
      { do: 'Show', count: 3, shown: await shown('ar', 3), document: docOf('ar') },
      { do: 'Japanese', ok: false, document: docOf('ar') },
      { do: 'Russian', ok: true, document: docOf('ru') },
      { do: 'Show', count: 3, shown: await shown('ru', 3), document: docOf('ru') },
      { do: 'English', ok: true, document: docOf('en') },
    ],
  });
  const version = browser.version();
  await browser.close();
  fs.writeFileSync(path.join(__dirname, '..', 'tests', 'i18n_pages.json'), JSON.stringify({ note: `Generated by scripts/gen-i18n-pages.js from Chromium ${version}'s Intl.`, program: PROGRAM, cases }, null, 1));
  console.log(`wrote tests/i18n_pages.json: ${cases.length} cases, ${cases.reduce((n, c) => n + c.steps.length, 0)} steps`);
})().catch((e) => { console.error(e); process.exit(1); });
