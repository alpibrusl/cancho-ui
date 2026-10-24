#!/usr/bin/env node
// The device probe (docs/probe/, docs/mobile-backend.md section 8.1) checked three ways. NONE OF THIS IS A DEVICE: part C is DESKTOP CHROMIUM (the Playwright
// Chromium on this machine, with the Pixel 7 profile: touch, 412 px, a phone's user agent). It shows that the probe runs, asks what it should, and does what it
// says it does; it shows nothing about what an iPhone or an Android phone answers.
//  A. the page sends nothing: no code path to another origin, no analytics, no beacon (a scan of the page's own files);
//  B. the decisions, as pure functions, against made-up facts (an iPhone Safari tab, an iPhone Home Screen app, an Android installed app, a browser with nothing):
//     the user agent parser, the zoom verdict, the touch-target verdict, the offline verdict, the permission lines;
//  C. the real page in DESKTOP CHROMIUM: it loads, registers its worker, starts offline after one visit (server stopped and the network off), asks for NO permission by
//     itself and records what a tap on each button did, reads the zoom of a tap in two fields, measures the ops console's targets, keeps a heartbeat, and makes no
//     request to any host but its own.
// Outside the gate: needs node, the `playwright` package with a Chromium. `--out <file>` writes the Chromium run's JSON (what section 8.2 of the note quotes).
//   node scripts/check-probe.js [--out report.json]
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium, devices } = require('playwright');

// a check that waits for something that never comes must end, not hang
setTimeout(() => { console.error('FAIL the run took more than 4 minutes: something never arrived'); process.exit(1); }, 240000).unref();

const ROOT = path.join(__dirname, '..');
const DOCS = path.join(ROOT, 'docs');
const PROBE = path.join(DOCS, 'probe');
const outIdx = process.argv.indexOf('--out');
const OUT = outIdx > 0 ? process.argv[outIdx + 1] : null;
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}
const J = JSON.stringify;
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const read = (f) => fs.readFileSync(path.join(PROBE, f), 'utf8');

// ---- A. it sends nothing ------------------------------------------------------------------------------------------------------------------------
{
  const js = read('probe.js');
  const html = read('index.html');
  const sw = read('sw.js');
  const banned = ['XMLHttpRequest', 'sendBeacon', 'WebSocket', 'EventSource', 'new Image', 'importScripts', 'import(', 'postMessage', 'navigator.share'];
  const found = banned.filter((b) => js.includes(b) || html.replace(/<[^>]*>/g, '').includes(b) || sw.includes(b));
  check('A: none of the ways a page talks to another site is in the probe\'s files', found.length === 0, found.join(', '));
  const fetches = (js.match(/\bfetch\(/g) || []).length + (html.match(/\bfetch\(/g) || []).length;
  check('A: the page has exactly one fetch, and it is for its own manifest (link.href)', fetches === 1 && js.includes('win.fetch(link.href, { cache: \'no-store\' })'), String(fetches));
  const urls = (html.match(/https?:\/\/[^"'\s<)]+/g) || []).filter((u) => !u.startsWith('https://github.com/alpibrusl/cancho-ui/'));
  check('A: the only addresses in the page are links to the cancho-ui repository (a link is followed only by a tap)', urls.length === 0, urls.join(' '));
  check('A: the worker answers only same-origin GETs from its own cache', sw.includes("new URL(request.url).origin !== self.location.origin") && !/\bfetch\(.*https?:/.test(sw));
  check('A: nothing in the page asks for a permission by itself: requestPermission and persist are only in click handlers', (() => {
    const re = /requestPermission\(|storage\.persist\(|\.prompt\(/g;
    const spans = ['notify', 'persist', 'install'].map((id) => {
      const start = html.indexOf(`$('${id}').addEventListener('click'`);
      if (start < 0) { return null; }
      const rest = html.slice(start + 10);
      const next = rest.search(/\n    (\$\(|\/\/|\[|window\.addEventListener)/);
      return [start, next < 0 ? html.length : start + 10 + next];
    });
    if (spans.some((x) => x === null)) { return false; }
    const sites = [...html.matchAll(re)].map((m) => m.index);
    return sites.length > 0 && sites.every((i) => spans.some(([x, y]) => i >= x && i < y)) && !/requestPermission\(|\.persist\(|\.prompt\(/.test(js) && !/requestPermission\(|\.persist\(/.test(sw);
  })());
}

// ---- B. the decisions against made-up facts --------------------------------------------------------------------------------------------------------
const P = require(path.join(PROBE, 'probe.js'));
{
  const UAS = [
    ['iPhone Safari', 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_4_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4.1 Mobile/15E148 Safari/604.1', 5, { os: 'iOS', osVersion: '17.4.1', browser: 'Safari', browserVersion: '17.4.1', engine: 'WebKit', mobile: true }],
    ['iPhone Chrome', 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/123.0.6312.52 Mobile/15E148 Safari/604.1', 5, { os: 'iOS', osVersion: '16.7', browser: 'Chrome (iOS)', browserVersion: '123.0.6312.52', engine: 'WebKit', mobile: true }],
    ['iPad that looks like a Mac', 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15', 5, { os: 'iPadOS', browser: 'Safari', browserVersion: '17.4', engine: 'WebKit', mobile: true }],
    ['a Mac', 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15', 0, { os: 'macOS', osVersion: '10.15.7', browser: 'Safari', mobile: false }],
    ['Android Chrome', 'Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.6312.99 Mobile Safari/537.36', 5, { os: 'Android', osVersion: '14', browser: 'Chrome', browserVersion: '123.0.6312.99', engine: 'Blink', mobile: true }],
    ['Samsung Internet', 'Mozilla/5.0 (Linux; Android 13; SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/24.0 Chrome/117.0.0.0 Mobile Safari/537.36', 5, { os: 'Android', osVersion: '13', browser: 'Samsung Internet', browserVersion: '24.0', engine: 'Blink' }],
    ['Firefox Android', 'Mozilla/5.0 (Android 14; Mobile; rv:124.0) Gecko/124.0 Firefox/124.0', 5, { os: 'Android', osVersion: '14', browser: 'Firefox', browserVersion: '124.0', engine: 'Gecko' }],
    ['Edge on Windows', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36 Edg/123.0.2420.65', 0, { os: 'Windows', osVersion: '10.0', browser: 'Edge', browserVersion: '123.0.2420.65', engine: 'Blink', mobile: false }],
    ['headless Chrome on Linux', 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/141.0.7390.37 Safari/537.36', 0, { os: 'Linux', browser: 'Chrome (headless)', browserVersion: '141.0.7390.37', engine: 'Blink', mobile: false }],
    ['nothing', '', 0, { os: 'unknown', browser: 'unknown', engine: 'unknown' }],
  ];
  const bad = [];
  for (const [name, ua, touch, want] of UAS) {
    const got = P.parseUA(ua, touch);
    for (const k of Object.keys(want)) { if (got[k] !== want[k]) { bad.push(`${name}: ${k} ${J(got[k])} want ${J(want[k])}`); } }
  }
  check(`B: the user agent parser reads ${UAS.length} user agents (iPhone, iPad as a Mac, Android, Samsung, Firefox, Edge, headless, nothing)`, bad.length === 0, bad.slice(0, 3).join(' | '));

  check('B: a zoom is a scale that grew by more than 0.01', P.zoomVerdict(1, 1.2).zoomed === true && P.zoomVerdict(1, 1).zoomed === false && P.zoomVerdict(1, 1.005).zoomed === false && P.zoomVerdict(1.5, 1.5).zoomed === false && P.zoomVerdict(1, 1.011).zoomed === true && P.zoomVerdict(null, 1).zoomed === null && P.zoomVerdict(2, 1).zoomed === false);
  const t = P.targetVerdict([{ width: 44, height: 44 }, { width: 43.9, height: 60 }, { width: 80, height: 30 }, { width: 100, height: 44 }], 44);
  check('B: a touch target is small when either side is below 44 CSS px', t.count === 4 && t.below === 2 && t.smallest.width === 43.9 && t.smallest.height === 30, J(t));
  check('B: none small, and none measured', P.targetVerdict([{ width: 44, height: 44 }], 44).below === 0 && P.targetVerdict([], 44).count === 0 && P.targetVerdict([{ width: 42, height: 60 }]).below === 1);

  const load = (visit, onLine, controller, standalone) => ({ visit, onLine, controller, standalone: !!standalone });
  check('B: offline start is proven only by a later visit that was offline and controlled by the worker', P.offlineVerdict([load(1, true, false), load(2, true, true)]).status === 'needs a tap'
    && P.offlineVerdict([load(1, false, true)]).status === 'needs a tap'
    && P.offlineVerdict([load(1, true, false), load(2, false, false)]).status === 'needs a tap'
    && P.offlineVerdict([load(1, true, false), load(2, false, true, true)]).status === 'yes'
    && /installed app/.test(P.offlineVerdict([load(1, true, false), load(2, false, true, true)]).detail)
    && P.offlineVerdict([]).status === 'needs a tap');

  const baseFacts = (over) => Object.assign({ generated: 'x', page: { url: 'https://example.test/probe/' }, userAgent: UAS[0][1], display: { standalone: false, navigatorStandalone: false, maxTouchPoints: 5, safeAreaInsets: { top: 47, right: 0, bottom: 34, left: 0 } }, history: [load(1, true, false)], manifest: { ok: true, name: 'p', display: 'standalone', icons: 4, start_url: './' }, serviceWorker: { supported: true, cachesApi: true }, notifications: { hasNotification: false, hasPushManager: false }, background: {}, storage: {}, focusZoom: {}, touchTargets: null }, over || {});
  const iphoneTab = P.report(baseFacts());
  check('B: an iPhone Safari tab: not installed, no Notification API, and the line says what to compare, as a measurement still to make', iphoneTab.install.runningInstalled.status === 'no' && iphoneTab.notifications.notificationApi.status === 'unsupported' && /compare this line/.test(iphoneTab.notifications.notificationApi.detail), J(iphoneTab.notifications.notificationApi));
  const iphoneApp = P.report(baseFacts({ display: { standalone: false, navigatorStandalone: true, maxTouchPoints: 5, safeAreaInsets: { top: 47, right: 0, bottom: 34, left: 0 } }, notifications: { hasNotification: true, permission: 'default', hasPushManager: true, pushPermissionState: 'prompt', hasSubscription: false } }));
  check('B: an iPhone Home Screen app (only navigator.standalone is true) is installed, with notification and push present and the permission read, not asked', iphoneApp.install.runningInstalled.status === 'yes' && iphoneApp.notifications.notificationApi.status === 'yes' && iphoneApp.notifications.permission.detail.includes('"default"') && iphoneApp.notifications.pushPermissionState.detail.includes('"prompt"') && iphoneApp.notifications.requestPermissionTap.status === 'needs a tap', J(iphoneApp.notifications.requestPermissionTap));
  const tapped = P.report(baseFacts({ notifications: { hasNotification: true, permission: 'granted', hasPushManager: true, tap: { before: 'default', result: 'granted', error: null, after: 'granted' }, shown: { ok: true } } }));
  check('B: after a tap the report says what the tap did (before, result, after), and that a test notification was handed over', tapped.notifications.requestPermissionTap.status === 'yes' && /before "default", after "granted"/.test(tapped.notifications.requestPermissionTap.detail) && tapped.notifications.testNotificationTap.status === 'yes');
  const android = P.report(baseFacts({ userAgent: UAS[4][1], display: { standalone: true, displayMode: 'standalone', navigatorStandalone: null, maxTouchPoints: 5, safeAreaInsets: { top: 0, right: 0, bottom: 0, left: 0 } }, beforeInstallPrompt: true, background: { periodicSync: true, backgroundSync: true, backgroundFetch: false, wakeLock: true, periodicSyncPermission: 'denied' }, storage: { estimate: { usage: 3145728, quota: 104857600 }, persisted: false }, history: [load(1, true, false), load(2, false, true, true)] }));
  check('B: an Android installed app: standalone, an install prompt on offer (not taken), offline start proven, background APIs by presence, storage as a number', android.install.runningInstalled.status === 'yes' && android.install.installPrompt.status === 'needs a tap' && android.offlineStart.status === 'yes' && android.background.periodicSync.status === 'yes' && android.background.backgroundFetch.status === 'unsupported' && android.storage.persisted.status === 'no' && android.storage.estimate.usage === 3145728 && /3 MB of a quota of 100 MB/.test(android.storage.estimate.detail), J(android.storage.estimate));
  const none = P.report({ generated: 'x', page: {}, userAgent: '', display: {}, history: [] });
  check('B: a browser that offers nothing is described, not crashed on, and every request for a permission stays "needs a tap"', none.serviceWorker.api.status === 'unsupported' && none.notifications.pushManager.status === 'unsupported' && none.notifications.requestPermissionTap.status === 'needs a tap' && none.storage.estimate.status === 'unsupported' && none.touchTargets.status === 'not measured' && none.focusZoom.field16.status === 'needs a tap');
  const zoomed = P.report(baseFacts({ focusZoom: { field16: { before: 1, after: 1, fontSize: '16px' }, field14: { before: 1, after: 1.25, fontSize: '14px' } } }));
  check('B: a tap that grew the scale is reported as a zoom, one that did not as none', zoomed.focusZoom.field14.zoomed === true && zoomed.focusZoom.field16.zoomed === false && zoomed.focusZoom.field14.status === 'yes' && zoomed.focusZoom.field16.status === 'no');
  const targets = P.report(baseFacts({ touchTargets: { rects: [{ tag: 'button', label: 'a', width: 44, height: 44 }, { tag: 'input', label: 'q', width: 300, height: 30 }], viewportWidth: 390, overflowX: true } }));
  const t42 = P.report(baseFacts({ touchTargets: { rects: [{ tag: 'button', label: 'a', width: 60, height: 42 }], viewportWidth: 390, overflowX: false } }));
  check('B: ops-console targets: a control 42 px high is below 44 (the line judges against 44, not 40) and no overflow is claimed', t42.touchTargets.status === 'no' && t42.touchTargets.below === 1 && /horizontal overflow: no/i.test(t42.touchTargets.detail), t42.touchTargets.detail);
  check('B: ops-console targets: a 30 px field is below 44, and a horizontal overflow is reported', targets.touchTargets.status === 'no' && targets.touchTargets.below === 1 && /horizontal overflow: YES/.test(targets.touchTargets.detail), targets.touchTargets.detail);
  check('B: the manual steps are listed (speech, Lighthouse, a real push, the store, the cancho-ui PWA) and never filled in', iphoneTab.manualSteps.length === 5 && /VoiceOver/.test(iphoneTab.manualSteps[0]) && /Lighthouse/.test(iphoneTab.manualSteps[1]));
  check('B: the report says it sends nothing', /no request to any origin but its own/.test(iphoneTab.sendsNothing) && iphoneTab.schema === 'cancho-ui-device-probe' && iphoneTab.version === 1);
}

// ---- C. the real page in DESKTOP CHROMIUM -------------------------------------------------------------------------------------------------------------
(async () => {
  console.log('--- C. DESKTOP CHROMIUM (Playwright Chromium with the Pixel 7 profile): not a device ---');
  const requests = [];
  const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
  let server;
  const start = (port) => new Promise((ok) => {
    server = http.createServer((req, res) => {
      requests.push(req.url);
      const rel = decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html';
      const f = path.join(DOCS, path.normalize(rel));
      let file = f;
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { file = path.join(f, 'index.html'); }
      if (!file.startsWith(DOCS) || !fs.existsSync(file)) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
      fs.createReadStream(file).pipe(res);
    });
    server.listen(port, '127.0.0.1', () => ok(server));
  });
  await start(0);
  const port = server.address().port;
  const base = `http://localhost:${port}`;
  const browser = await chromium.launch({ args: ['--no-proxy-server'] });
  const ctx = await browser.newContext({ ...devices['Pixel 7'], permissions: ['clipboard-read', 'clipboard-write'] });
  const stray = [];
  await ctx.route('**/*', (route) => {
    const u = new URL(route.request().url());
    if (u.host === `localhost:${port}`) { route.continue(); } else { stray.push(route.request().url()); route.abort(); }
  });
  // count what the page asks for by itself, before any tap
  await ctx.addInitScript(() => {
    window.__asked = { requestPermission: 0, persist: 0, prompt: 0 };
    if (window.Notification) { const r = Notification.requestPermission.bind(Notification); Notification.requestPermission = function () { window.__asked.requestPermission++; return r.apply(null, arguments); }; }
    if (navigator.storage && navigator.storage.persist) { const p = navigator.storage.persist.bind(navigator.storage); navigator.storage.persist = function () { window.__asked.persist++; return p(); }; }
  });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`${base}/probe/`);
  await page.waitForFunction(() => window.__probeReport && window.__probeReport.serviceWorker, null, { timeout: 15000 });
  const report = () => page.evaluate(() => window.__probeReport);
  const press = async (label) => { await page.click(`button:has-text("${label}")`); await sleep(500); };

  let rep = await report();
  check('C: the first visit is visit 1, not controlled by the worker yet, offline start not yet proven', rep.loadHistory.length === 1 && rep.loadHistory[0].visit === 1 && rep.loadHistory[0].controller === false && rep.offlineStart.status === 'needs a tap', J(rep.loadHistory));
  check('C: it registered its worker', rep.serviceWorker.registered.status === 'yes', J(rep.serviceWorker.registered));
  check('C: it asked for no permission and no persistence by itself', J(await page.evaluate(() => window.__asked)) === J({ requestPermission: 0, persist: 0, prompt: 0 }), J(await page.evaluate(() => window.__asked)));
  check('C: the Notification permission was read, not asked', rep.notifications.permission.status === 'yes' && rep.notifications.requestPermissionTap.status === 'needs a tap', J(rep.notifications.permission));

  // the second visit: the worker controls the page
  await page.reload();
  await page.waitForFunction(() => window.__probeReport && window.__probeReport.serviceWorker, null, { timeout: 15000 });
  await sleep(500);
  rep = await report();
  check('C: the second visit is controlled by the worker, and its cache holds the page\'s files', rep.loadHistory.length === 2 && rep.loadHistory[1].controller === true && rep.serviceWorker.controlsThisPage.status === 'yes' && rep.serviceWorker.cache.status === 'yes', J(rep.serviceWorker.cache));
  check('C: the manifest was read: name, display standalone, 4 icons', rep.install.manifest.status === 'yes' && /"cancho-ui device probe"/.test(rep.install.manifest.detail) && /4 icons/.test(rep.install.manifest.detail), rep.install.manifest.detail);

  // the taps: what each did
  await press('Ask for notification permission');
  rep = await report();
  const nt = rep.notifications.requestPermissionTap;
  check('C: a tap on "Ask for notification permission" is recorded with what it did', nt.status === 'yes' && /before "(default|denied)"/.test(nt.detail), nt.detail);
  check('C: and the page asked exactly once, because of that tap', (await page.evaluate(() => window.__asked.requestPermission)) === 1);
  await press('Show a test notification');
  rep = await report();
  check('C: "Show a test notification" is recorded (it is allowed to fail: this Chromium has the permission it has)', ['yes', 'no'].includes(rep.notifications.testNotificationTap.status), rep.notifications.testNotificationTap.detail);
  await press('Ask for persistent storage');
  rep = await report();
  check('C: a tap on "Ask for persistent storage" is recorded', rep.storage.persistTap.status === 'yes' && (await page.evaluate(() => window.__asked.persist)) === 1, rep.storage.persistTap.detail);

  // the zoom of a tap in two fields (a touch device profile; a desktop browser has no pinch zoom, so the answer is "no" for both and says nothing about iOS)
  await page.tap('#f16');
  await sleep(1000);
  await page.tap('#f14');
  await sleep(1000);
  rep = await report();
  check('C: both taps are recorded with the scale before and after and the font size of the field', rep.focusZoom.field16.status !== 'needs a tap' && rep.focusZoom.field14.status !== 'needs a tap' && rep.focusZoom.field16.detail.includes('16px') && rep.focusZoom.field14.detail.includes('14px'), J([rep.focusZoom.field16.detail, rep.focusZoom.field14.detail]));

  // the ops console's targets
  await page.waitForFunction(() => window.__probeReport && window.__probeReport.touchTargets && window.__probeReport.touchTargets.count > 0, null, { timeout: 15000 }).catch(() => {});
  await page.click('button:has-text("Re-measure")');
  await sleep(1200);
  rep = await report();
  check('C: the ops console\'s interactive elements were measured in a frame as wide as the page', rep.touchTargets.count > 0, rep.touchTargets.detail);

  // the heartbeat
  await sleep(2500);
  rep = await report();
  check('C: the 1 s heartbeat ran', /ran \d+ times/.test(rep.background.timers.detail) && !/ran 0 times/.test(rep.background.timers.detail), rep.background.timers.detail);

  const online = rep;
  // copy
  await page.click('button:has-text("Copy JSON")');
  await sleep(300);
  const clip = await page.evaluate(() => navigator.clipboard.readText());
  let parsed = null;
  try { parsed = JSON.parse(clip); } catch (e) { parsed = null; }
  check('C: Copy JSON puts one JSON document on the clipboard', parsed && parsed.schema === 'cancho-ui-device-probe' && parsed.version === 1 && parsed.userAgent && parsed.display && parsed.manualSteps.length === 5, clip.slice(0, 60));
  check('C: the page shows the same text as the JSON block and lists the manual steps', (await page.locator('#manual li').count()) === 5 && (await page.inputValue('#json')).includes('"cancho-ui-device-probe"'));

  // offline start after one visit: the server is gone and the network is off
  await new Promise((ok) => server.close(ok));
  server.closeAllConnections();
  await ctx.setOffline(true);
  await page.reload();
  await page.waitForFunction(() => window.__probeReport && window.__probeReport.loadHistory.length >= 3, null, { timeout: 15000 });
  await sleep(500);
  rep = await report();
  const last = rep.loadHistory[rep.loadHistory.length - 1];
  check('C: with the server stopped and the network off, the page opened from its worker\'s cache', last.onLine === false && last.controller === true, J(last));
  check('C: and the probe says offline start is proven', rep.offlineStart.status === 'yes', rep.offlineStart.detail);

  check('C: no request left this machine\'s own server (every other host was refused by the check and none was asked for)', stray.length === 0, J(stray));
  check('C: the server saw only the page\'s own files and the ops console frame', requests.every((u) => /^\/(probe\/|demo\/ops-console\/|favicon\.ico)/.test(u)), J([...new Set(requests.map((u) => u.split('?')[0]))]));
  check('C: no script error', errors.length === 0, errors.join(' | '));

  if (OUT) { fs.writeFileSync(OUT, JSON.stringify(online, null, 2)); console.log(`wrote ${OUT}`); }
  await browser.close();
  console.log(failures.length ? `\n${failures.length} check(s) failed` : '\nall checks passed');
  process.exit(failures.length ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
