// The device probe's service worker (docs/mobile-backend.md section 8.1): the same strategy as the one `cancho-ui build --pwa` writes (docs/mobile-backend.md
// section 5), so that "does it start offline after one visit" is asked of the same mechanism: precache the page's own files, answer a same-origin GET from the
// cache first, fall back to the cached page for a navigation that is not cached. It talks to nothing but this page's own origin. Change CACHE when any file changes.
const CACHE = 'cancho-probe-1';
const FILES = ['./', './index.html', './probe.js', './manifest.webmanifest', './icon-192.png', './icon-512.png'];
self.addEventListener('install', function (event) {
    event.waitUntil(caches.open(CACHE).then(function (cache) { return cache.addAll(FILES); }));
});
self.addEventListener('activate', function (event) {
    event.waitUntil(
        caches.keys().then(function (keys) {
            return Promise.all(keys.filter(function (k) { return k.indexOf('cancho-probe-') === 0 && k !== CACHE; }).map(function (k) { return caches.delete(k); }));
        })
    );
});
self.addEventListener('fetch', function (event) {
    const request = event.request;
    if (request.method !== 'GET' || new URL(request.url).origin !== self.location.origin) { return; }
    event.respondWith(
        caches.match(request, { ignoreSearch: true }).then(function (hit) {
            if (hit) { return hit; }
            return fetch(request).catch(function () {
                if (request.mode === 'navigate') { return caches.match('./index.html'); }
                return Response.error();
            });
        })
    );
});
// a tap on a test notification brings the probe to the front (shown by registration.showNotification from the page, nothing is pushed from anywhere)
self.addEventListener('notificationclick', function (event) {
    event.notification.close();
    event.waitUntil(self.clients.matchAll({ type: 'window' }).then(function (cs) { return cs.length ? cs[0].focus() : self.clients.openWindow('./'); }));
});
