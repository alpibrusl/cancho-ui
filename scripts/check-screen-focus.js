#!/usr/bin/env node
// Focus, announcements and the page title when a screen changes (docs/screen-focus.md, issue #92) in Chromium, in the mobile profile
// (Pixel 7: touch, 412 px, Android UA): when the control that had the focus is replaced with its screen, the new screen's first heading
// takes it (tabindex -1, no ring after touch, the browser's ring after the keyboard); a redraw that does not lose focus, a page whose focus is
// elsewhere, and the load of a page move nothing; a routed page's title follows the heading and a page that does not route never has its
// title written; a screen with no heading leaves the focus where it was. The speech of a screen reader is not checked (section 7).
// Outside the gate: needs node, the `playwright` package with a Chromium, and a built `cancho-ui`.
//   node scripts/check-screen-focus.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const SIGNIN = `component Auth {
    state user: String = "";
    state signed: Bool = false;
    on Name(v) { user = v; }
    on SignIn { signed = true; }
    on SignOut { signed = false; }
    view {
        Column {
            Text { role: heading "Portal" }
            If { when: !signed
                Text { role: heading "Sign in" }
                TextField { label: "User" value: user on: Name submit: SignIn }
                Button { label: "Sign in" on: SignIn }
            }
            If { when: signed
                Text { role: heading "Work orders" }
                Text { "Hello {user}" }
                Button { label: "Sign out" on: SignOut }
            }
        }
    }
}
`;
// A screen with no heading: a button that replaces itself.
const NOHEAD = `component Plain {
    state shown: Bool = false;
    on Flip { shown = !shown; }
    view {
        Column {
            If { when: !shown
                Button { label: "Show" on: Flip }
            }
            If { when: shown
                Text { "shown" }
                Button { label: "Hide" on: Flip }
            }
        }
    }
}
`;
// A page that redraws by itself while the focus is outside it.
const TICKER = `component Ticker {
    state n: Int = 0;
    every 100 Tick;
    on Tick { n = n + 1; }
    view {
        Column {
            Text { role: heading "Ticker" }
            Button { label: "Press" on: Tick }
            Text { "n={n}" }
        }
    }
}
`;
// A routed page with a screen that has no heading.
const ROUTED_NOHEAD = `component Pages {
    requires Router;
    state path: String = "/";
    route Heard;
    on Heard(p) { path = p; }
    view {
        Column {
            If { when: path == "/"
                Text { role: heading "Home" }
                NavLink { label: "Go bare" to: "/bare" }
            }
            If { when: path == "/bare"
                Text { "no heading here" }
                NavLink { label: "Back home" to: "/" }
            }
        }
    }
}
`;

function serve(dir) {
  const types = { '.html': 'text/html', '.js': 'text/javascript' };
  return new Promise((ok) => {
    const s = http.createServer((req, res) => {
      const u = req.url.split('?')[0];
      const f = path.join(dir, u === '/' ? 'index.html' : path.normalize(u));
      if (!fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': types[path.extname(f)] || 'text/plain', 'cache-control': 'no-store' });
      fs.createReadStream(f).pipe(res);
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
}

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-screen-focus-'));
  const make = (name, src, flag = '--page', extra = []) => {
    const dir = path.join(tmp, name);
    fs.mkdirSync(dir);
    const file = src.startsWith('/') ? src : path.join(dir, 'p.cui');
    if (!src.startsWith('/')) { fs.writeFileSync(file, src); }
    const r = spawnSync(CANCHO_UI, ['build', file, ...(flag ? [flag, '-o', path.join(dir, 'out'), '--name', name] : ['-o', path.join(dir, 'out.js')]), ...extra], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stdout}${r.stderr}`); }
    return dir;
  };
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const open = async (dir, hash = '') => {
    const s = await serve(path.join(dir, 'out'));
    const ctx = await browser.newContext({ ...devices['Pixel 7'], serviceWorkers: 'block' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(`http://localhost:${s.address().port}/${hash}`);
    await page.waitForSelector('#root');
    await page.waitForTimeout(60);
    return { s, ctx, page, errors, close: async () => { await ctx.close(); s.close(); } };
  };
  const at = (page) => page.evaluate(() => {
    const a = document.activeElement;
    return { tag: a.tagName, text: a.tagName === 'BODY' ? '' : a.textContent.trim(), role: a.getAttribute('role'), tabindex: a.getAttribute('tabindex'), visible: a.matches(':focus-visible'), title: document.title };
  });
  const isHeading = (a, text) => a.role === 'heading' && a.text === text && a.tabindex === '-1';

  // ---- sign-in: the screen replaces the control that had the focus ----
  {
    const t = await open(make('signin', SIGNIN));
    const { page } = t;
    check('sign-in: nothing is focused at load', (await at(page)).tag === 'BODY');
    await page.getByRole('textbox', { name: 'User' }).fill('ann');
    await page.getByRole('textbox', { name: 'User' }).focus();
    await page.keyboard.type('x');
    check('sign-in: typing in a field keeps the focus in it (no move to a heading)', (await at(page)).tag === 'INPUT', JSON.stringify(await at(page)));
    await page.keyboard.press('Enter');
    await page.waitForTimeout(60);
    let a = await at(page);
    check('sign-in: after Enter the new screen\'s heading (not the application title above it) holds the focus, with tabindex -1', isHeading(a, 'Work orders'), JSON.stringify(a));
    check('sign-in: the browser draws its ring after the keyboard', a.visible === true, JSON.stringify(a));
    check('sign-in: a page that does not route never has its title written', a.title === 'signin', a.title);
    await page.getByRole('button', { name: 'Sign out' }).focus();
    await page.keyboard.press('Enter');
    await page.waitForTimeout(60);
    a = await at(page);
    check('sign-in: sign-out returns to the first screen\'s heading', isHeading(a, 'Sign in'), JSON.stringify(a));
    await page.getByRole('button', { name: 'Sign in' }).tap();
    await page.waitForTimeout(60);
    a = await at(page);
    check('sign-in: after a touch tap the heading holds the focus and no ring is drawn', isHeading(a, 'Work orders') && a.visible === false, JSON.stringify(a));
    check('sign-in: no page error', t.errors.length === 0, t.errors.join('; '));
    await t.close();
  }

  // ---- a screen with no heading: the focus is where it was (the body) ----
  {
    const t = await open(make('nohead', NOHEAD));
    await t.page.getByRole('button', { name: 'Show' }).tap();
    await t.page.waitForTimeout(60);
    const a = await at(t.page);
    check('no heading: the focus is left on the body, nothing throws', a.tag === 'BODY' && t.errors.length === 0, JSON.stringify(a) + t.errors.join('; '));
    await t.close();
  }

  // ---- a redraw that is not about the focus ----
  {
    const dir = make('ticker', TICKER, '');
    fs.mkdirSync(path.join(dir, 'out'));
    fs.writeFileSync(path.join(dir, 'out', 'index.html'), '<!doctype html><title>ext host</title><input id="ext" aria-label="outside"><div id="root"></div><script type="module" src="./out.js"></script>');
    fs.renameSync(path.join(dir, 'out.js'), path.join(dir, 'out', 'out.js'));
    const t = await open(dir);
    await t.page.evaluate(() => document.getElementById('ext').focus());
    await t.page.waitForTimeout(450);
    const a = await at(t.page);
    const n = await t.page.evaluate(() => (document.body.innerText.match(/n=(\d+)/) || [])[1]);
    check('a timer redraws the page while the focus is outside it: the focus stays outside', a.tag === 'INPUT' && Number(n) >= 3, JSON.stringify(a) + ` n=${n}`);
    await t.page.getByRole('button', { name: 'Press' }).focus();
    await t.page.waitForTimeout(350);
    const b = await at(t.page);
    check('and a control that is redrawn in place keeps the focus', b.tag === 'BUTTON', JSON.stringify(b));
    // the heading that was given the focus keeps it through the redraws that follow (its tabindex is written again)
    await t.page.evaluate(() => { const h = document.querySelector('[role=heading]'); h.setAttribute('tabindex', '-1'); h.focus(); });
    await t.page.waitForTimeout(350);
    const c = await at(t.page);
    check('a heading that holds the focus keeps it through the redraws that follow', c.role === 'heading' && c.tabindex === '-1', JSON.stringify(c));
    await t.close();
  }

  // ---- routes: a link, Back, a typed URL ----
  {
    const t = await open(make('routes', path.join(ROOT, 'examples/routes/routes.cui')), '#/items/valve');
    const { page } = t;
    let a = await at(page);
    check('routes: the deep link sets the title and moves nothing', a.tag === 'BODY' && a.title === 'routes'.length && false || (a.tag === 'BODY' && a.title === 'Machine valve – routes'), JSON.stringify(a));
    await page.getByRole('link', { name: 'All machines' }).tap();
    await page.waitForTimeout(80);
    a = await at(page);
    check('routes: a link moves focus to the new screen\'s heading and sets the title', isHeading(a, 'Machines') && a.title === 'Machines – routes', JSON.stringify(a));
    await page.getByRole('link', { name: 'pump' }).tap();
    await page.waitForTimeout(80);
    await page.evaluate(() => document.activeElement.blur());
    await page.goBack();
    await page.waitForTimeout(80);
    a = await at(page);
    check('routes: Back moves focus to the heading even though the focus was not in the page', isHeading(a, 'Machines') && a.title === 'Machines – routes', JSON.stringify(a));
    await page.goto(page.url().replace(/#.*$/, '') + '#/items/gauge');
    await page.waitForTimeout(80);
    a = await at(page);
    check('routes: a typed URL moves focus to the heading and sets the title', isHeading(a, 'Machine gauge') && a.title === 'Machine gauge – routes', JSON.stringify(a));
    check('routes: no page error', t.errors.length === 0, t.errors.join('; '));
    await t.close();
  }

  // ---- routes: a screen with no heading gets the loaded title back ----
  {
    const t = await open(make('pages', ROUTED_NOHEAD));
    let a = await at(t.page);
    check('routed, no heading: the title follows the heading at load', a.title === 'Home – pages', a.title);
    await t.page.getByRole('link', { name: 'Go bare' }).tap();
    await t.page.waitForTimeout(80);
    a = await at(t.page);
    check('routed, no heading: the loaded title is restored and the focus stays on the body', a.title === 'pages' && a.tag === 'BODY', JSON.stringify(a));
    await t.close();
  }

  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
