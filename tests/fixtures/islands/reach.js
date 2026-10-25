// What a sealed island can reach (docs/islands.md section 2). This file is an island: it tries each thing below from inside the frame, then says
// what happened, as one JSON text, in its `Result` event. The addresses are props: `other` is a server of another origin, `page` the origin of the
// page, `stun` the UDP port of a STUN server on this machine. The check counts what each server and the socket received; the answer of an attempt
// is the second half of the evidence, the first is that nothing arrived.
(function () {
    var P = cancho.props;
    var results = {};
    var wait = function (ms) { return new Promise(function (ok) { setTimeout(ok, ms); }); };
    var ran = function (name, f) {
        return Promise.resolve().then(f).then(function (r) { results[name] = String(r); }, function (e) { results[name] = (e && e.name) || String(e); });
    };
    var A = P.page, B = P.other;
    window.__inline_ran = 0;
    function first() {
        return Promise.all([]).then(function () { return ran('read parent.document', function () { parent.document.title; return 'reached'; }); })
            .then(function () { return ran('read top.location', function () { return 'reached ' + top.location.href; }); })
            .then(function () { return ran('localStorage', function () { localStorage.setItem('a', 'b'); return 'reached'; }); })
            .then(function () { return ran('document.cookie', function () { document.cookie = 'a=b'; return document.cookie ? 'reached' : 'none kept'; }); })
            .then(function () { return ran('indexedDB', function () { var r = indexedDB.open('x'); return new Promise(function (ok) { r.onsuccess = function () { ok('reached'); }; r.onerror = function () { ok('error'); }; setTimeout(function () { ok('no answer'); }, 300); }); }); })
            .then(function () { return ran('fetch to another origin', function () { return fetch(B + '/leak?fetch', { mode: 'no-cors' }).then(function () { return 'sent'; }); }); })
            .then(function () { return ran('fetch to the page origin', function () { return fetch(A + '/leak?fetchA', { mode: 'no-cors' }).then(function () { return 'sent'; }); }); })
            .then(function () { return ran('XMLHttpRequest', function () { var x = new XMLHttpRequest(); x.open('GET', B + '/leak?xhr'); x.send(); return wait(300).then(function () { return x.status ? 'answered' : 'blocked'; }); }); })
            .then(function () { return ran('new Image().src', function () { var i = new Image(); i.src = B + '/leak?img'; return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('sendBeacon', function () { return navigator.sendBeacon(B + '/leak?beacon', 'x') ? 'queued' : 'refused'; }); })
            .then(function () { return ran('WebSocket', function () { var w = new WebSocket('ws://' + B.slice(7) + '/leak?ws'); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('EventSource', function () { var s = new EventSource(B + '/leak?es'); return wait(300).then(function () { s.close(); return 'tried'; }); }); })
            .then(function () { return ran('<link rel=stylesheet>', function () { var l = document.createElement('link'); l.rel = 'stylesheet'; l.href = B + '/leak?css-link'; document.head.appendChild(l); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('CSS url() in a style', function () { var d = document.createElement('div'); d.style.cssText = 'width:9px;height:9px;background:url(' + B + '/leak?css-url)'; document.body.appendChild(d); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('<script src>', function () { var s = document.createElement('script'); s.src = B + '/leak?script-src'; document.head.appendChild(s); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('import()', function () { return import(B + '/mod.js').then(function () { return 'reached'; }); }); })
            .then(function () { return ran('nested <iframe src>', function () { var f = document.createElement('iframe'); f.src = B + '/leak?iframe'; document.body.appendChild(f); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('<form> submit', function () { var f = document.createElement('form'); f.method = 'post'; f.action = B + '/leak?form'; document.body.appendChild(f); f.submit(); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('window.open', function () { var w = window.open(B + '/leak?open'); return wait(300).then(function () { return w ? 'opened' : 'null'; }); }); })
            .then(function () { return ran('eval', function () { return eval('1+1'); }); })
            .then(function () { return ran('new Function', function () { return new Function('return 1')(); }); })
            .then(function () { return ran('Worker from a blob', function () { var w = new Worker(URL.createObjectURL(new Blob(['postMessage(1)']))); return new Promise(function (ok) { w.onmessage = function () { ok('ran'); }; w.onerror = function () { ok('error'); }; setTimeout(function () { ok('no answer'); }, 300); }); }); })
            .then(function () { return ran('serviceWorker.register', function () { return navigator.serviceWorker.register('/sw.js').then(function () { return 'registered'; }); }); })
            .then(function () { return ran('clipboard.writeText', function () { return navigator.clipboard.writeText('x').then(function () { return 'written'; }); }); })
            .then(function () { return ran('<link rel=prefetch>', function () { var l = document.createElement('link'); l.rel = 'prefetch'; l.href = B + '/leak?prefetch'; document.head.appendChild(l); return wait(300).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('<object data>', function () { var o = document.createElement('object'); o.data = B + '/leak?object'; document.body.appendChild(o); return wait(300).then(function () { return 'tried'; }); }); })
            // the one thing that is allowed: the document is the compiler's, and no script that is not one of its hashes runs
            .then(function () { return ran('an inline <script> the island adds', function () { var s = document.createElement('script'); s.textContent = 'window.__inline_ran = 1'; document.head.appendChild(s); return window.__inline_ran ? 'ran' : 'not run'; }); })
            .then(function () { return ran('document.write of a script', function () { document.open(); document.write('<script>window.__inline_ran = 2<\/script>'); document.close(); return window.__inline_ran === 2 ? 'ran' : 'not run'; }); })
            .then(function () { return ran('an inline event handler attribute', function () { var d = document.createElement('div'); d.setAttribute('onclick', 'window.__inline_ran = 3'); document.body.appendChild(d); d.click(); return window.__inline_ran === 3 ? 'ran' : 'not run'; }); })
            .then(function () { return ran('a javascript: address', function () { var a = document.createElement('a'); a.href = 'javascript:window.__inline_ran = 4'; document.body.appendChild(a); a.click(); return wait(100).then(function () { return window.__inline_ran === 4 ? 'ran' : 'not run'; }); }); })
            .then(function () { return ran('RTCPeerConnection to a STUN server', function () { var p = new RTCPeerConnection({ iceServers: [{ urls: 'stun:127.0.0.1:' + P.stun }] }); p.createDataChannel('x'); return p.createOffer().then(function (o) { return p.setLocalDescription(o); }).then(function () { return wait(1200); }).then(function () { return 'tried'; }); }); })
            .then(function () { return ran('webkitRTCPeerConnection', function () { var p = new webkitRTCPeerConnection({ iceServers: [{ urls: 'stun:127.0.0.1:' + P.stun }] }); return 'constructed'; }); })
            .then(function () { return ran('postMessage to the parent', function () { parent.postMessage({ hello: 1 }, '*'); return 'sent'; }); });
    }
    // the ways out that leave the frame are last: the answer is sent first, because the frame may not be there after them
    function last() {
        var m = document.createElement('meta'); m.httpEquiv = 'refresh'; m.content = '0;url=' + B + '/leak?meta'; document.head.appendChild(m);
        var a = document.createElement('a'); a.href = B + '/leak?top'; a.target = '_top'; document.body.appendChild(a); a.click();
        setTimeout(function () { location.href = B + '/leak?selfnav'; }, 10);
    }
    first().then(function () {
        cancho.emit('Result', JSON.stringify(results));
        return wait(100);
    }).then(last);
})();
