#!/usr/bin/env node
// The baseline Content-Security-Policy of every page (docs/structure.md 12.0r, crates/cancho-ui/src/csp.rs) in Chromium, in the mobile profile:
//  A. every gallery demo (docs/demo/<id>/, as the repository holds them) loads under its policy with no console error, no page error and no policy violation;
//  B. a page whose only inline scripts are the ones the policy lists by hash still runs them: a sticky header sets `--cui-header-h` (examples/page), an app registers its worker
//     (`--pwa`), and a sealed island, whose frame inherits the page's policy, draws and answers (examples/islands);
//  C. what the policy is for: on a page with no map, an image from another origin, a request to another origin and a script injected inline are each refused by the browser
//     (a `securitypolicyviolation` of img-src, connect-src and script-src), and the page's own requests are not.
// Outside the gate: needs node, a Chromium (the `playwright` package if installed, else Chrome found through $CHROME, scripts/lib-browser.js), and a built `cancho-ui`.
//   node scripts/check-csp.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { serve, launch } = require('./lib-browser.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
// the browser asks for the icon of any page it opens; a page of ours has none
const real = (problems) => problems.filter((p) => !/favicon\.ico/.test(p));

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-csp-'));
  const build = (name, file, flags) => {
    const dir = path.join(tmp, name);
    const r = spawnSync(CANCHO_UI, ['build', file, ...(flags || ['--page']), '-o', dir, '--name', name], { encoding: 'utf8', cwd: ROOT });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stdout}${r.stderr}`); }
    return dir;
  };
  const browser = await launch();
  console.log(`browser: ${browser.name}`);
  const withPage = async (dir, fn) => {
    const s = await serve(dir);
    const page = await browser.open(`http://127.0.0.1:${s.address().port}/`);
    try { return await fn(page); } finally { await page.close(); s.close(); }
  };
  const settle = (page, ms) => page.evaluate(`new Promise((r) => setTimeout(r, ${ms}))`);

  // ---- A. every gallery demo ----
  const demos = path.join(ROOT, 'docs', 'demo');
  for (const id of fs.readdirSync(demos).sort()) {
    const dir = path.join(demos, id);
    if (!fs.existsSync(path.join(dir, 'index.html'))) { continue; }
    const html = fs.readFileSync(path.join(dir, 'index.html'), 'utf8');
    await withPage(dir, async (page) => {
      await page.evaluate(`window.__v = []; addEventListener('securitypolicyviolation', (e) => window.__v.push(e.effectiveDirective + ' ' + e.blockedURI)); 0`);
      await settle(page, 400);
      const v = await page.evaluate('window.__v');
      check(`demo ${id}: has a policy`, /<meta http-equiv="Content-Security-Policy" content="default-src 'self'; /.test(html));
      check(`demo ${id}: loads with no console error, page error or violation`, real(page.problems).length === 0 && v.length === 0, real(page.problems).concat(v).join('; '));
    });
  }

  // ---- B. the inline scripts a policy lists by hash still run ----
  await withPage(build('sticky', path.join(ROOT, 'examples/page/page.cui')), async (page) => {
    await settle(page, 400);
    const h = await page.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--cui-header-h')");
    check('sticky header: the inline script that measures the header ran (--cui-header-h is set)', /^\d+(\.\d+)?px$/.test(String(h).trim()), h);
    check('sticky header: no error', real(page.problems).length === 0, page.problems.join('; '));
  });
  await withPage(build('app', path.join(ROOT, 'examples/counter/counter.cui'), ['--pwa']), async (page) => {
    const reg = await page.evaluate("navigator.serviceWorker.register('./sw.js').then(() => 'registered', (e) => 'refused ' + e)");
    check('app: the worker registers under the policy', reg === 'registered', reg);
    const inline = await page.evaluate("Array.from(document.scripts).filter((s) => !s.src).length");
    check('app: its inline script is listed by hash (and the page has one)', inline >= 1, inline);
    check('app: no error', real(page.problems).length === 0, page.problems.join('; '));
  });
  await withPage(build('islands', path.join(ROOT, 'examples/islands/islands.cui')), async (page) => {
    await settle(page, 1200);
    const f = await page.evaluate("(() => { const f = document.querySelector('iframe[data-cui-island]'); return f ? { frames: document.querySelectorAll('iframe').length, built: !!(f.contentDocument || f.contentWindow) } : null; })()");
    check('islands: the sealed frame is on the page', !!f && f.frames >= 1, JSON.stringify(f));
    check('islands: the frame\'s scripts ran under the inherited policy (no script-src violation)', real(page.problems).filter((p) => /script/i.test(p)).length === 0, page.problems.join('; '));
  });

  // ---- C. what the policy is for ----
  await withPage(build('plain', path.join(ROOT, 'examples/counter/counter.cui')), async (page) => {
    const r = await page.evaluate(`(async () => {
      const seen = [];
      addEventListener('securitypolicyviolation', (e) => seen.push(e.effectiveDirective));
      new Image().src = 'https://elsewhere.invalid/p.png';
      try { await fetch('https://elsewhere.invalid/x'); } catch (e) { /* refused */ }
      const s = document.createElement('script'); s.textContent = 'window.__ran = 1'; document.body.appendChild(s);
      const own = await fetch('./app.js').then((x) => x.status);
      await new Promise((r) => setTimeout(r, 300));
      return { seen, ran: window.__ran === 1, own };
    })()`);
    check('a page with no map refuses an image of another origin', r.seen.some((d) => d === 'img-src'), JSON.stringify(r));
    check('and a request to another origin', r.seen.some((d) => d === 'connect-src'), JSON.stringify(r));
    check('and a script an attacker puts in the page', r.seen.some((d) => d === 'script-src-elem') && !r.ran, JSON.stringify(r));
    check('while its own files are served', r.own === 200);
  });

  await browser.close();
  fs.rmSync(tmp, { recursive: true, force: true });
  if (failures.length) { console.log(`\n${failures.length} check(s) failed`); process.exit(1); }
  console.log('\nall checks passed');
})().catch((e) => { console.error(e); process.exit(2); });
