// The cancho-ui device probe (docs/mobile-backend.md section 8.1). It asks the browser it runs in what it says about the questions of issues #65 and #68, and
// shows the answers as text and as one JSON block to copy. It sends NOTHING anywhere: no request to any other origin, no analytics, no beacon; the only requests it
// makes are for its own files, from its own origin. It asks for no permission by itself: a permission is requested only by a tap on the button that says so, and
// the page records what that tap did. What only a person can judge (a screen reader's speech, Lighthouse) is listed as a manual step and is never filled in.
//
// The page is plain HTML and JavaScript, not a cancho-ui program: cancho-ui programs have capabilities (Network, Storage, Location, Clock, Locale, Credential)
// and none of them reads a browser's feature table, a permission, a service worker registration or a visual viewport.
//
// The decisions are pure functions of a `facts` object (parseUA, report and the verdicts below), so they are checked without a browser against made-up facts
// (scripts/check-probe.js); `collect` is the only part that touches the browser.
(function (root) {
    'use strict';

    var STATUS = { yes: 'yes', no: 'no', unsupported: 'unsupported', tap: 'needs a tap', manual: 'manual step', unknown: 'not measured' };

    // ---- who is asking -------------------------------------------------------------------------------------------------------------------------------
    function versionOf(m) { return m ? m[1].replace(/_/g, '.') : ''; }
    function parseUA(ua, maxTouchPoints) {
        ua = String(ua || '');
        var out = { os: 'unknown', osVersion: '', browser: 'unknown', browserVersion: '', engine: 'unknown', mobile: /Mobi|iPhone|iPad|Android/.test(ua) };
        var m;
        if ((m = /(iPhone|iPad|iPod); CPU (?:iPhone )?OS ([0-9_]+)/.exec(ua))) { out.os = m[1] === 'iPad' ? 'iPadOS' : 'iOS'; out.osVersion = versionOf({ 1: m[2] }); }
        else if ((m = /Android ([0-9.]+)/.exec(ua))) { out.os = 'Android'; out.osVersion = m[1]; }
        else if (/Macintosh/.test(ua) && maxTouchPoints > 1) { out.os = 'iPadOS'; out.osVersion = (m = /Version\/([0-9.]+)/.exec(ua)) ? m[1] + ' (Safari version; the user agent hides the OS version)' : ''; out.mobile = true; }
        else if ((m = /Mac OS X ([0-9_.]+)/.exec(ua))) { out.os = 'macOS'; out.osVersion = versionOf({ 1: m[1] }); }
        else if ((m = /Windows NT ([0-9.]+)/.exec(ua))) { out.os = 'Windows'; out.osVersion = m[1]; }
        else if (/CrOS/.test(ua)) { out.os = 'ChromeOS'; }
        else if (/Linux/.test(ua)) { out.os = 'Linux'; }
        if ((m = /EdgA?\/([0-9.]+)/.exec(ua))) { out.browser = 'Edge'; out.browserVersion = m[1]; }
        else if ((m = /SamsungBrowser\/([0-9.]+)/.exec(ua))) { out.browser = 'Samsung Internet'; out.browserVersion = m[1]; }
        else if ((m = /(?:Firefox|FxiOS)\/([0-9.]+)/.exec(ua))) { out.browser = /FxiOS/.test(ua) ? 'Firefox (iOS)' : 'Firefox'; out.browserVersion = m[1]; }
        else if ((m = /CriOS\/([0-9.]+)/.exec(ua))) { out.browser = 'Chrome (iOS)'; out.browserVersion = m[1]; }
        else if ((m = /(?:Chrome|HeadlessChrome)\/([0-9.]+)/.exec(ua))) { out.browser = /HeadlessChrome/.test(ua) ? 'Chrome (headless)' : 'Chrome'; out.browserVersion = m[1]; }
        else if ((m = /Version\/([0-9.]+).*Safari/.exec(ua))) { out.browser = 'Safari'; out.browserVersion = m[1]; }
        out.engine = /Firefox\//.test(ua) && !/FxiOS/.test(ua) ? 'Gecko' : (/AppleWebKit/.test(ua) && !/Chrome\/|Edg|SamsungBrowser/.test(ua) ? 'WebKit' : (/Chrome\/|Edg|SamsungBrowser/.test(ua) ? 'Blink' : 'unknown'));
        if (out.os === 'iOS' || out.os === 'iPadOS') { out.engine = 'WebKit'; }
        return out;
    }

    // ---- the verdicts: facts in, a status and a sentence out ---------------------------------------------------------------------------------
    function row(status, detail, extra) {
        var r = { status: status, detail: detail };
        if (extra) { for (var k in extra) { r[k] = extra[k]; } }
        return r;
    }

    function installVerdict(f) {
        var standalone = !!(f.display && (f.display.standalone || f.display.navigatorStandalone));
        return {
            runningInstalled: row(standalone ? STATUS.yes : STATUS.no, standalone
                ? 'The page is running as an installed app (display-mode standalone: ' + !!(f.display && f.display.standalone) + ', navigator.standalone: ' + (f.display && f.display.navigatorStandalone) + ').'
                : 'The page is running in a browser tab. Install it (iPhone: Share, then Add to Home Screen; Android Chrome: menu, Install app) and open it from the home screen, then read this line again.'),
            manifest: row(f.manifest && f.manifest.ok ? STATUS.yes : (f.manifest ? STATUS.no : STATUS.unknown),
                f.manifest ? (f.manifest.ok ? 'The manifest was read: name ' + JSON.stringify(f.manifest.name) + ', display ' + JSON.stringify(f.manifest.display) + ', ' + f.manifest.icons + ' icons, start_url ' + JSON.stringify(f.manifest.start_url) + '.' : 'The manifest could not be read or has no name, icons or start_url: ' + f.manifest.problem) : 'Not read.'),
            installPrompt: row(f.beforeInstallPrompt ? (f.installTap ? STATUS.yes : STATUS.tap) : STATUS.no,
                f.beforeInstallPrompt ? (f.installTap ? 'The browser offered an install prompt and a tap on the button answered: ' + JSON.stringify(f.installTap) + '.' : 'The browser offered an install prompt (beforeinstallprompt fired). Tap "Install" to see what it does.') : 'No beforeinstallprompt event was seen. Chrome on Android fires it when the page is installable and not installed; Safari does not implement it (documented by the vendors, not measured by this page), so installing is a manual step there. Seeing none is not by itself a failure.'),
            getInstalledRelatedApps: row(f.hasGetInstalledRelatedApps ? STATUS.yes : STATUS.unsupported, 'navigator.getInstalledRelatedApps is ' + (f.hasGetInstalledRelatedApps ? 'present.' : 'absent.')),
        };
    }

    function serviceWorkerVerdict(f) {
        var sw = f.serviceWorker || {};
        return {
            api: row(sw.supported ? STATUS.yes : STATUS.unsupported, 'navigator.serviceWorker is ' + (sw.supported ? 'present.' : 'absent (a service worker needs a secure context: https, or localhost).')),
            registered: row(sw.registered ? STATUS.yes : (sw.supported ? STATUS.no : STATUS.unsupported), sw.registered ? 'Registered, scope ' + JSON.stringify(sw.scope) + ', state ' + JSON.stringify(sw.state) + '.' : (sw.error ? 'Registration failed: ' + sw.error : 'Not registered.')),
            controlsThisPage: row(sw.controller ? STATUS.yes : STATUS.no, sw.controller ? 'This page was loaded through its service worker.' : 'This page is not controlled by a service worker yet (the first visit never is: reload once).'),
            cache: row(sw.cacheKeys && sw.cacheKeys.length ? STATUS.yes : (sw.cachesApi ? STATUS.no : STATUS.unsupported), sw.cacheKeys && sw.cacheKeys.length ? 'Cache storage holds ' + JSON.stringify(sw.cacheKeys) + '.' : 'Cache storage is ' + (sw.cachesApi ? 'empty.' : 'absent.')),
        };
    }

    // "starts offline after one online visit": one load in the history that has the network off, was controlled by the worker, and is not the first.
    function offlineVerdict(history) {
        history = history || [];
        var proven = history.filter(function (h) { return h.visit > 1 && h.onLine === false && h.controller === true; });
        var visits = history.length ? history[history.length - 1].visit : 0;
        if (proven.length) {
            return row(STATUS.yes, 'Proven: visit ' + proven[proven.length - 1].visit + ' of this page loaded with the network off (navigator.onLine false) through its service worker' + (proven[proven.length - 1].standalone ? ', as an installed app.' : ', in a browser tab.'));
        }
        return row(STATUS.tap, 'Not yet. Visit ' + visits + '. Open this page once online (done), then turn on airplane mode, close the app or tab completely, open it again, and read this line: it says "Proven" if the page opened.');
    }

    function notificationVerdict(f) {
        var n = f.notifications || {};
        var out = {
            notificationApi: row(n.hasNotification ? STATUS.yes : STATUS.unsupported, 'The Notification API is ' + (n.hasNotification ? 'present.' : 'absent' + (f.isIOS && !(f.display && (f.display.standalone || f.display.navigatorStandalone)) ? ' (on iOS, Apple documents it for a web app added to the Home Screen: install this page, open it from the home screen and compare this line; that comparison is the measurement).' : '.'))),
            permission: row(n.hasNotification ? STATUS.yes : STATUS.unsupported, n.hasNotification ? 'Notification.permission is ' + JSON.stringify(n.permission) + ' (read without asking).' : 'No permission state to read.'),
            pushManager: row(n.hasPushManager ? STATUS.yes : STATUS.unsupported, 'PushManager is ' + (n.hasPushManager ? 'present.' : 'absent.')),
            pushPermissionState: row(n.pushPermissionState ? STATUS.yes : (n.hasPushManager ? STATUS.unknown : STATUS.unsupported), n.pushPermissionState ? 'pushManager.permissionState({userVisibleOnly: true}) is ' + JSON.stringify(n.pushPermissionState) + '.' : (n.pushPermissionError ? 'permissionState failed: ' + n.pushPermissionError : 'Not read.')),
            existingSubscription: row(n.hasSubscription === true ? STATUS.yes : (n.hasSubscription === false ? STATUS.no : STATUS.unknown), n.hasSubscription === true ? 'This page already has a push subscription.' : (n.hasSubscription === false ? 'No push subscription (this page never subscribes: that needs a push server and contacts the browser vendor\'s push service).' : 'Not read.')),
            requestPermissionTap: n.tap ? row(STATUS.yes, 'A tap on "Ask for notification permission" gave ' + JSON.stringify(n.tap.result) + ' (' + (n.tap.error ? 'error: ' + n.tap.error : 'no error') + ', permission before ' + JSON.stringify(n.tap.before) + ', after ' + JSON.stringify(n.tap.after) + ').') : row(STATUS.tap, 'Not asked. The page never asks by itself; tap "Ask for notification permission" to find out what a tap does here.'),
            testNotificationTap: n.shown ? row(n.shown.ok ? STATUS.yes : STATUS.no, n.shown.ok ? 'A test notification was handed to the browser with registration.showNotification (look for it).' : 'showNotification failed: ' + n.shown.error) : row(STATUS.tap, 'Not tried. After permission is granted, tap "Show a test notification".'),
            serverPush: row(STATUS.manual, 'Receiving a push from a server is not testable from this page: it needs a push server and a subscription, which would contact the browser vendor\'s push service. This page only reports the presence and the permission state.'),
        };
        return out;
    }

    function backgroundVerdict(f) {
        var b = f.background || {};
        var presence = function (label, v) { return row(v ? STATUS.yes : STATUS.unsupported, label + ' is ' + (v ? 'present.' : 'absent.')); };
        return {
            periodicSync: presence('registration.periodicSync (Periodic Background Sync)', b.periodicSync),
            periodicSyncPermission: row(b.periodicSyncPermission ? STATUS.yes : STATUS.unknown, b.periodicSyncPermission ? 'permissions.query periodic-background-sync is ' + JSON.stringify(b.periodicSyncPermission) + '.' : 'Not readable here.'),
            backgroundSync: presence('registration.sync (Background Sync)', b.backgroundSync),
            backgroundFetch: presence('registration.backgroundFetch (Background Fetch)', b.backgroundFetch),
            wakeLock: presence('navigator.wakeLock (Screen Wake Lock)', b.wakeLock),
            timers: b.heartbeat ? row(STATUS.yes, 'A 1 s timer ran ' + b.heartbeat.ticks + ' times in ' + b.heartbeat.seconds + ' s; the longest gap between two ticks was ' + b.heartbeat.maxGapMs + ' ms, and ' + b.heartbeat.hiddenTicks + ' ticks fell while the page was hidden (' + b.heartbeat.hiddenMs + ' ms hidden). Switch to another app for 30 s and come back to see whether timers keep running.') : row(STATUS.unknown, 'No timer measured yet.'),
            lifecycle: row(STATUS.yes, 'Page lifecycle events seen: ' + JSON.stringify(b.lifecycle || {}) + '. (freeze/resume exist in Chromium only; pagehide/pageshow in all.)'),
        };
    }

    function storageVerdict(f) {
        var s = f.storage || {};
        var mb = function (n) { return typeof n === 'number' ? (Math.round(n / 1048576 * 10) / 10) + ' MB' : 'unknown'; };
        return {
            estimate: row(s.estimate ? STATUS.yes : STATUS.unsupported, s.estimate ? 'Using ' + mb(s.estimate.usage) + ' of a quota of ' + mb(s.estimate.quota) + '.' : 'navigator.storage.estimate is absent.', s.estimate ? { usage: s.estimate.usage, quota: s.estimate.quota } : null),
            persisted: row(s.persisted === true ? STATUS.yes : (s.persisted === false ? STATUS.no : STATUS.unsupported), s.persisted === true ? 'Storage is persistent (the browser will not evict it under pressure).' : (s.persisted === false ? 'Storage is best-effort (the browser may evict it).' : 'navigator.storage.persisted is absent.')),
            persistTap: s.persistTap ? row(STATUS.yes, 'A tap on "Ask for persistent storage" answered ' + JSON.stringify(s.persistTap.result) + (s.persistTap.error ? ' (error: ' + s.persistTap.error + ')' : '') + '.') : row(STATUS.tap, 'Not asked. Tap "Ask for persistent storage" to see what it does here.'),
            localStorage: row(s.localStorage ? STATUS.yes : STATUS.no, 'localStorage write and read ' + (s.localStorage ? 'worked.' : 'failed.')),
            indexedDB: row(s.indexedDB ? STATUS.yes : STATUS.no, 'indexedDB is ' + (s.indexedDB ? 'present.' : 'absent.')),
            cookies: row(s.cookies ? STATUS.yes : STATUS.no, 'Cookies are ' + (s.cookies ? 'enabled.' : 'disabled.')),
        };
    }

    // The zoom on focus: the scale of the visual viewport before and after a tap in a text field. A pinch zoom is a scale above 1.
    function zoomVerdict(before, after) {
        if (typeof before !== 'number' || typeof after !== 'number') { return { zoomed: null, delta: null }; }
        var d = Math.round((after - before) * 1000) / 1000;
        return { zoomed: d > 0.01, delta: d };
    }
    function focusZoomVerdict(f) {
        var z = f.focusZoom || {};
        var one = function (name, r) {
            if (!r) { return row(STATUS.tap, 'Not run. ' + name); }
            var v = zoomVerdict(r.before, r.after);
            if (v.zoomed === null) { return row(STATUS.unknown, 'visualViewport.scale is not available here.', r); }
            return row(v.zoomed ? STATUS.yes : STATUS.no, 'Tapping the ' + r.fontSize + ' text field changed the visual viewport scale from ' + r.before + ' to ' + r.after + ' (' + (v.zoomed ? 'the page ZOOMED' : 'the page did not zoom') + ').', { zoomed: v.zoomed, delta: v.delta });
        };
        return {
            field16: one('Tap the first field (16 px, the size cancho-ui\'s baseline stylesheet gives inputs).', z.field16),
            field14: one('Tap the second field (14 px, a control: iOS Safari is widely reported to zoom a field whose text is below 16 px; this field is here to show whether it does, and the first field whether cancho-ui\'s 16 px prevents it).', z.field14),
        };
    }

    // The touch targets: every interactive element's size, in CSS px, against 44.
    function targetVerdict(rects, min) {
        min = min || 44;
        rects = rects || [];
        var small = rects.filter(function (r) { return r.width < min || r.height < min; });
        var w = rects.length ? Math.min.apply(null, rects.map(function (r) { return r.width; })) : null;
        var h = rects.length ? Math.min.apply(null, rects.map(function (r) { return r.height; })) : null;
        return { count: rects.length, smallest: { width: w, height: h }, below: small.length, small: small.slice(0, 20), min: min };
    }
    function touchTargetVerdict(f) {
        var t = f.touchTargets;
        if (!t) { return row(STATUS.unknown, 'The ops console was not measured.'); }
        if (t.error) { return row(STATUS.unknown, 'The ops console could not be measured: ' + t.error); }
        var v = targetVerdict(t.rects, 44);
        v.viewportWidth = t.viewportWidth;
        v.overflowX = t.overflowX;
        return row(v.count === 0 ? STATUS.unknown : (v.below === 0 ? STATUS.yes : STATUS.no),
            v.count + ' interactive elements of the ops console at a viewport ' + t.viewportWidth + ' px wide; the smallest is ' + v.smallest.width + ' x ' + v.smallest.height + ' CSS px; ' + v.below + ' are below 44 px' + (v.below ? ' (the first: ' + JSON.stringify(v.small.slice(0, 3)) + ')' : '') + '; horizontal overflow: ' + (t.overflowX ? 'YES' : 'no') + '.', v);
    }

    function displayVerdict(f) {
        var d = f.display || {};
        var si = d.safeAreaInsets || {};
        return {
            viewport: row(STATUS.yes, 'innerWidth x innerHeight ' + d.innerWidth + ' x ' + d.innerHeight + ', screen ' + d.screenWidth + ' x ' + d.screenHeight + ', devicePixelRatio ' + d.devicePixelRatio + ', orientation ' + JSON.stringify(d.orientation) + (d.visualViewport ? ', visualViewport ' + d.visualViewport.width + ' x ' + d.visualViewport.height + ' at scale ' + d.visualViewport.scale : '') + '.'),
            displayMode: row(STATUS.yes, 'display-mode: ' + JSON.stringify(d.displayMode) + '; navigator.standalone: ' + JSON.stringify(d.navigatorStandalone) + '.'),
            safeAreaInsets: row(STATUS.yes, 'env(safe-area-inset-*) in CSS px: top ' + si.top + ', right ' + si.right + ', bottom ' + si.bottom + ', left ' + si.left + (si.top === 0 && si.bottom === 0 && si.left === 0 && si.right === 0 ? ' (all zero: no notch or home bar is reported, or viewport-fit=cover is not in effect).' : '.')),
            pointer: row(STATUS.yes, 'pointer ' + JSON.stringify(d.pointer) + ', hover ' + JSON.stringify(d.hover) + ', maxTouchPoints ' + d.maxTouchPoints + '.'),
            colorScheme: row(STATUS.yes, 'prefers-color-scheme ' + JSON.stringify(d.colorScheme) + ', prefers-reduced-motion ' + JSON.stringify(d.reducedMotion) + '.'),
        };
    }

    var MANUAL = [
        'VoiceOver (iPhone) and TalkBack (Android) on the ops console: does the table announce its name, its column headers and the row you are on, and does the labelled search field announce its label? A page cannot judge speech: listen, and write down what was said.',
        'Lighthouse, PWA and accessibility categories, on Android Chrome (DevTools over USB, or PageSpeed Insights on the hosted PWA): record the scores and every failed audit.',
        'A real push from a server to an installed page: this probe reports presence and permission only. It needs a push server and a subscription.',
        'App-store distribution: not testable from a web page; it is a product question (docs/mobile-backend.md section 6).',
        'The ops console itself installed with cancho-ui build --pwa: install it, turn on airplane mode, open it (issue #65 items 1 and 3). This probe asks the same questions of a page with the same service worker strategy, not of that build.',
    ];

    // ---- the report: everything above, in one object ------------------------------------------------------------------------------------------
    function report(f) {
        var ua = parseUA(f.userAgent, f.display && f.display.maxTouchPoints);
        var isIOS = ua.os === 'iOS' || ua.os === 'iPadOS';
        var facts = {};
        for (var k in f) { facts[k] = f[k]; }
        facts.isIOS = isIOS;
        return {
            schema: 'cancho-ui-device-probe',
            version: 1,
            generated: f.generated,
            page: f.page,
            sendsNothing: 'This page made no request to any origin but its own, used no analytics and no beacon.',
            userAgent: { string: f.userAgent, parsed: ua, userAgentData: f.userAgentData || null, platform: f.platform || null, language: f.language || null },
            display: displayVerdict(facts),
            install: installVerdict(facts),
            serviceWorker: serviceWorkerVerdict(facts),
            offlineStart: offlineVerdict(f.history),
            notifications: notificationVerdict(facts),
            background: backgroundVerdict(facts),
            storage: storageVerdict(facts),
            focusZoom: focusZoomVerdict(facts),
            touchTargets: touchTargetVerdict(facts),
            touchTargetsByOrientation: f.touchTargetsByOrientation || {},
            loadHistory: f.history || [],
            manualSteps: MANUAL,
        };
    }

    // ---- collecting the facts (the only part that touches the browser) ------------------------------------------------------------------------
    function mq(win, q) { try { return win.matchMedia(q).matches; } catch (e) { return null; } }
    function safeInsets(win) {
        var doc = win.document;
        var el = doc.createElement('div');
        el.style.cssText = 'position:absolute;visibility:hidden;padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)';
        doc.body.appendChild(el);
        var cs = win.getComputedStyle(el);
        var r = { top: parseFloat(cs.paddingTop) || 0, right: parseFloat(cs.paddingRight) || 0, bottom: parseFloat(cs.paddingBottom) || 0, left: parseFloat(cs.paddingLeft) || 0 };
        doc.body.removeChild(el);
        return r;
    }
    function displayFacts(win) {
        var nav = win.navigator;
        var vv = win.visualViewport;
        var mode = ['standalone', 'fullscreen', 'minimal-ui', 'browser'].filter(function (m) { return mq(win, '(display-mode: ' + m + ')'); })[0] || null;
        return {
            standalone: mq(win, '(display-mode: standalone)') === true,
            displayMode: mode,
            navigatorStandalone: typeof nav.standalone === 'boolean' ? nav.standalone : null,
            innerWidth: win.innerWidth, innerHeight: win.innerHeight,
            screenWidth: win.screen && win.screen.width, screenHeight: win.screen && win.screen.height,
            devicePixelRatio: win.devicePixelRatio,
            orientation: win.screen && win.screen.orientation ? win.screen.orientation.type : (win.innerWidth > win.innerHeight ? 'landscape' : 'portrait'),
            visualViewport: vv ? { width: Math.round(vv.width * 100) / 100, height: Math.round(vv.height * 100) / 100, scale: vv.scale } : null,
            safeAreaInsets: safeInsets(win),
            pointer: mq(win, '(pointer: coarse)') ? 'coarse' : (mq(win, '(pointer: fine)') ? 'fine' : 'none'),
            hover: mq(win, '(hover: hover)') ? 'hover' : 'none',
            maxTouchPoints: nav.maxTouchPoints || 0,
            colorScheme: mq(win, '(prefers-color-scheme: dark)') ? 'dark' : 'light',
            reducedMotion: mq(win, '(prefers-reduced-motion: reduce)') ? 'reduce' : 'no-preference',
        };
    }
    function loadHistory(win, controller) {
        var key = 'cancho-probe-history';
        var list = [];
        try { list = JSON.parse(win.localStorage.getItem(key) || '[]'); } catch (e) { list = []; }
        var visit = (list.length ? list[list.length - 1].visit : 0) + 1;
        list.push({ visit: visit, at: new Date().toISOString(), onLine: win.navigator.onLine, controller: !!controller, standalone: mq(win, '(display-mode: standalone)') === true || win.navigator.standalone === true });
        list = list.slice(-12);
        try { win.localStorage.setItem(key, JSON.stringify(list)); } catch (e) { /* storage unavailable: the history is this load only */ }
        return list;
    }
    function manifestFacts(win) {
        var link = win.document.querySelector('link[rel="manifest"]');
        if (!link) { return Promise.resolve({ ok: false, problem: 'no <link rel="manifest">' }); }
        return win.fetch(link.href, { cache: 'no-store' }).then(function (r) { return r.json(); }).then(function (m) {
            var ok = !!(m.name && m.start_url && m.icons && m.icons.length);
            return { ok: ok, name: m.name, display: m.display, icons: (m.icons || []).length, start_url: m.start_url, problem: ok ? '' : 'missing name, start_url or icons' };
        }).catch(function (e) { return { ok: false, problem: String(e && e.message || e) }; });
    }
    function collect(win, state) {
        var nav = win.navigator;
        var facts = {
            generated: new Date().toISOString(),
            page: { url: win.location.href, secureContext: win.isSecureContext === true },
            userAgent: nav.userAgent, platform: nav.platform, language: nav.language,
            userAgentData: nav.userAgentData ? { brands: nav.userAgentData.brands, mobile: nav.userAgentData.mobile, platform: nav.userAgentData.platform } : null,
            display: displayFacts(win),
            hasGetInstalledRelatedApps: typeof nav.getInstalledRelatedApps === 'function',
            beforeInstallPrompt: !!state.beforeInstallPrompt,
            installTap: state.installTap || null,
            serviceWorker: { supported: 'serviceWorker' in nav, cachesApi: 'caches' in win, controller: !!(nav.serviceWorker && nav.serviceWorker.controller), registered: false },
            notifications: { hasNotification: 'Notification' in win, permission: 'Notification' in win ? win.Notification.permission : null, hasPushManager: 'PushManager' in win, tap: state.notificationTap || null, shown: state.notificationShown || null },
            background: { periodicSync: false, backgroundSync: false, backgroundFetch: false, wakeLock: 'wakeLock' in nav, heartbeat: state.heartbeat ? state.heartbeat() : null, lifecycle: state.lifecycle || {} },
            storage: { estimate: null, persisted: null, persistTap: state.persistTap || null, localStorage: false, indexedDB: 'indexedDB' in win, cookies: nav.cookieEnabled === true },
            focusZoom: state.focusZoom || {},
            touchTargets: state.touchTargets || null,
            touchTargetsByOrientation: state.touchTargetsByOrientation || {},
        };
        try { win.localStorage.setItem('cancho-probe-test', '1'); facts.storage.localStorage = win.localStorage.getItem('cancho-probe-test') === '1'; win.localStorage.removeItem('cancho-probe-test'); } catch (e) { facts.storage.localStorage = false; }
        var jobs = [];
        jobs.push(manifestFacts(win).then(function (m) { facts.manifest = m; }));
        if (nav.storage && nav.storage.estimate) { jobs.push(nav.storage.estimate().then(function (e) { facts.storage.estimate = { usage: e.usage, quota: e.quota }; }).catch(function () { /* left null */ })); }
        if (nav.storage && nav.storage.persisted) { jobs.push(nav.storage.persisted().then(function (p) { facts.storage.persisted = p; }).catch(function () { /* left null */ })); }
        if (nav.permissions && nav.permissions.query) {
            jobs.push(nav.permissions.query({ name: 'periodic-background-sync' }).then(function (p) { facts.background.periodicSyncPermission = p.state; }).catch(function () { /* the name is not known to this browser */ }));
        }
        if (nav.serviceWorker && nav.serviceWorker.getRegistration) {
            jobs.push(nav.serviceWorker.getRegistration().then(function (reg) {
                var sw = facts.serviceWorker;
                if (state.registerError) { sw.error = state.registerError; }
                if (!reg) { return null; }
                sw.registered = true;
                sw.scope = reg.scope;
                var worker = reg.active || reg.waiting || reg.installing;
                sw.state = worker ? worker.state : null;
                facts.background.periodicSync = 'periodicSync' in reg;
                facts.background.backgroundSync = 'sync' in reg;
                facts.background.backgroundFetch = 'backgroundFetch' in reg;
                facts.notifications.hasRegistrationShowNotification = typeof reg.showNotification === 'function';
                var work = [];
                if (reg.pushManager) {
                    work.push(reg.pushManager.permissionState({ userVisibleOnly: true }).then(function (s) { facts.notifications.pushPermissionState = s; }).catch(function (e) { facts.notifications.pushPermissionError = String(e && e.message || e); }));
                    work.push(reg.pushManager.getSubscription().then(function (s) { facts.notifications.hasSubscription = !!s; }).catch(function () { /* left unknown */ }));
                }
                return Promise.all(work);
            }).catch(function () { /* no registration to read */ }));
        }
        if (win.caches && win.caches.keys) { jobs.push(win.caches.keys().then(function (k) { facts.serviceWorker.cacheKeys = k; }).catch(function () { /* left unset */ })); }
        return Promise.all(jobs).then(function () {
            facts.history = state.history;
            return facts;
        });
    }

    var api = { STATUS: STATUS, parseUA: parseUA, report: report, collect: collect, loadHistory: loadHistory, zoomVerdict: zoomVerdict, targetVerdict: targetVerdict, offlineVerdict: offlineVerdict, installVerdict: installVerdict, notificationVerdict: notificationVerdict, backgroundVerdict: backgroundVerdict, storageVerdict: storageVerdict, serviceWorkerVerdict: serviceWorkerVerdict, focusZoomVerdict: focusZoomVerdict, touchTargetVerdict: touchTargetVerdict };
    if (typeof module !== 'undefined' && module.exports) { module.exports = api; }
    root.CanchoProbe = api;
})(typeof window !== 'undefined' ? window : globalThis);
