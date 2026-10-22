#!/usr/bin/env node
// examples/tickets in Chromium against the stand-in service (docs/tickets.md section 4). Outside the gate: needs node, the
// `playwright` package with a Chromium, and a built `cancho-ui`. Every check is a measurement of the built page, including the
// ones that record a gap (they print what was measured; the gap is a filed issue, docs/tickets.md section 3).
//   node scripts/check-tickets.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium } = require('playwright');
const { createServer } = require('./tickets-server.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
function note(label, detail) { console.log(`note ${label}: ${detail}`); }

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'check-tickets-'));
  const built = spawnSync(CANCHO_UI, ['build', 'examples/tickets/tickets.cui', '--pwa', '-o', path.join(dir, 'out'), '--name', 'Work orders'], { encoding: 'utf8', cwd: ROOT });
  if (built.status !== 0) { throw new Error(built.stderr); }
  const behaviour = {};
  const server = await createServer(path.join(dir, 'out'), behaviour);
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const context = await browser.newContext({ viewport: { width: 420, height: 900 }, serviceWorkers: 'block' });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error' && !/401|Failed to load resource/.test(m.text())) { errors.push(m.text()); } });
  const body = () => page.locator('#root').innerText();
  await page.goto(url);
  await page.waitForSelector('#root *');

  // 1. the sign-in screen
  check('the sign-in form shows first, with a message for each empty field', /Enter your user name\./.test(await body()) && /Enter your password\./.test(await body()));
  const signIn = page.getByRole('button', { name: 'Sign in' });
  check('the Sign in button is disabled while a field is invalid', await signIn.isDisabled());
  check('the password field is a password field (docs/controls.md section 5)', (await page.getByLabel('Password').getAttribute('type')) === 'password' && (await page.getByLabel('User name').getAttribute('type')) === 'text');
  check('the fields are reachable by their labels', (await page.getByLabel('User name').count()) === 1 && (await page.getByLabel('Password').count()) === 1);

  // 2. validation messages as the user types
  await page.getByLabel('User name').fill('ad');
  check('a short user name says so', /A user name has at least 3 characters\./.test(await body()));
  await page.getByLabel('User name').fill('ada');
  check('the message goes away when the name is long enough', !/user name has at least|Enter your user name/.test(await body()));
  check('and leaves no stray tone mark behind (an empty message with a tone would still draw its prefix: docs/tickets.md 3.5)', await page.evaluate(() => Array.from(document.querySelectorAll('[role=status]')).filter((e) => !e.textContent.trim()).every((e) => getComputedStyle(e, '::before').content === 'none')));
  await page.getByLabel('Password').fill('short');
  check('a short password says so and Sign in stays disabled', /A password has at least 8 characters\./.test(await body()) && await signIn.isDisabled());
  await page.getByLabel('Password').press('Enter');
  await page.waitForTimeout(150);
  check('Enter in a field does not send a form that is not valid', server.log.logins.length === 0, `${server.log.logins.length} logins`);

  // 3. a wrong password, then the right one
  await page.getByLabel('Password').fill('wrongpass1');
  check('a valid-looking form enables Sign in', await signIn.isEnabled());
  await signIn.click();
  await page.waitForFunction(() => /Wrong user name or password\./.test(document.body.innerText), null, { timeout: 5000 });
  check('a wrong password is said, the service saw one login, the form is still shown', server.log.logins.length === 1 && (await page.getByLabel('User name').count()) === 1 && (await page.getByLabel('Password').inputValue()) === '');
  check('a refused sign-in asks for no orders (docs/handler-sequences.md)', server.log.orderRequests === 0, `${server.log.orderRequests} requests for the orders`);
  await page.getByLabel('Password').fill('hunter22!');
  await page.getByLabel('Password').press('Enter');
  await page.waitForFunction(() => /orders: loaded/.test(document.body.innerText), null, { timeout: 5000 });
  check('the right password, sent with Enter, shows the work orders', (await page.locator('tbody tr:has(td)').count()) === 4, `${await page.locator('tbody tr:has(td)').count()} rows`);
  check('the session is a cookie the page cannot read (HttpOnly)', (await page.evaluate(() => document.cookie)) === '' && (await context.cookies()).some((c) => c.name === 'sid' && c.httpOnly));
  check('the sign-in form is gone and the password is not left in the page', (await page.getByLabel('Password').count()) === 0 && !(await page.content()).includes('hunter22!'));
  note('focus after the screen changed', `document.activeElement is ${await page.evaluate(() => document.activeElement.tagName)} (docs/tickets.md 3.4)`);

  // 4. the detail and the edit form
  await page.getByRole('button', { name: 'Open T-2' }).click();
  check('Open T-2 shows the detail with its values', /Order T-2: Calibrate the furnace probe/.test(await body()) && (await page.getByLabel('Status (open, doing or done)').inputValue()) === 'doing' && (await page.getByLabel('Note').inputValue()) === 'probe ordered');
  const save = page.getByRole('button', { name: 'Save' });
  await page.getByLabel('Status (open, doing or done)').fill('finished');
  check('an invalid status says so and Save is disabled', /Use open, doing or done\./.test(await body()) && await save.isDisabled());
  await page.getByLabel('Status (open, doing or done)').fill('done');
  await page.getByLabel('Note').fill('x'.repeat(201));
  check('a note cannot be longer than 200 characters: the field stops there (docs/controls.md section 6)', (await page.getByLabel('Note').inputValue()).length === 200 && await save.isEnabled());
  await page.getByLabel('Note').fill('probe replaced');
  check('a valid form enables Save', await save.isEnabled());
  await save.click();
  await page.waitForFunction(() => /✓\s+saved|saved/.test(document.body.innerText) && /done/.test(document.body.innerText), null, { timeout: 5000 });
  check('the service received the edit', JSON.stringify(server.log.saves[0]) === JSON.stringify(['T-2', 'done', 'probe replaced']), JSON.stringify(server.log.saves));
  check('the list shows the new status after the reload of the orders', /T-2\s+Calibrate the furnace probe\s+done/.test(await body()), (await body()).replace(/\n/g, ' | ').slice(0, 200));
  check('a double click sends one save (the button is disabled while it is in flight)', await (async () => { await page.getByLabel('Note').fill('again'); await save.dblclick(); await page.waitForTimeout(300); return server.log.saves.length === 2; })(), `${server.log.saves.length} saves`);

  // 5. the service goes away while saving
  behaviour.down = true;
  await page.getByLabel('Note').fill('while down');
  await save.click();
  await page.waitForFunction(() => /not saved/.test(document.body.innerText), null, { timeout: 5000 });
  check('a save while the service is down says it was not saved', /not saved: /.test(await body()), (await body()).split('\n').find((l) => /not saved/.test(l)));
  behaviour.down = false;

  // 6. the layout and sign-out
  const wide = await page.evaluate(() => ({ d: document.documentElement.scrollWidth, v: document.documentElement.clientWidth }));
  check('no horizontal scroll at 420 px with the table and the detail open', wide.d <= wide.v, JSON.stringify(wide));
  await page.getByRole('button', { name: 'Sign out' }).click();
  await page.waitForFunction(() => /You are signed out\./.test(document.body.innerText), null, { timeout: 5000 });
  check('signing out shows the form again and clears the cookie', (await page.getByLabel('User name').count()) === 1 && !(await context.cookies()).some((c) => c.name === 'sid' && c.value));
  check('the orders are not reachable after sign-out', (await page.evaluate(async () => (await fetch('api/orders')).status)) === 401);
  check('no script errors', errors.length === 0, errors.join(' | '));
  await browser.close();
  server.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
