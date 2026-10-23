//! The `subscribe` adapter (docs/streams.md): Server-Sent Events over a `fetch` stream, with a reconnect the page owns. Its machine
//! is `cancho_ui_replay`'s `stream.rs`; `tests/stream_corpus.json` is checked against both. The numbers are the IR crate's, so the
//! page and replay cannot disagree about them.

use cancho_ui_ir::stream::{BACKOFF_MS, MESSAGE_MAX};

const TEMPLATE: &str = r#"// Platform adapter: streams, `subscribe Event "url"` (docs/streams.md). Server-Sent Events over a `fetch` stream, because an
// EventSource cannot send a header and reconnects on a timer the page does not own. A message is at most /*MESSAGE_MAX*/ characters (the
// `data` lines joined with a newline); a line is at most /*LINE_MAX*/. The parser reads each chunk once, so its cost is linear in what
// the server sends and a hostile stream cannot make the page build an 8 MiB string.
function __cui_sse(on) {
    var partial = '', partialLen = 0, skipLF = false, data = '', dataLen = 0, idBuf = null, stopped = false;
    function oversize() { stopped = true; on.oversize(); }
    function dispatch() {
        if (idBuf !== null) { on.id(idBuf); }
        if (dataLen === 0) { return; }
        var text = data.slice(0, -1);
        data = ''; dataLen = 0;
        on.message(text);
    }
    function line(l) {
        if (l.length === 0) { dispatch(); return; }
        if (l.charCodeAt(0) === 58) { return; }
        var c = l.indexOf(':'), f = l, v = '';
        if (c !== -1) { f = l.slice(0, c); v = l.slice(c + 1); if (v.charCodeAt(0) === 32) { v = v.slice(1); } }
        if (f === 'data') {
            if (dataLen + v.length > /*MESSAGE_MAX*/) { oversize(); return; }
            data += v + '\n'; dataLen += v.length + 1;
        } else if (f === 'id') {
            if (v.indexOf('\u0000') === -1) { idBuf = v; }
        }
    }
    return {
        feed: function (text) {
            var n = text.length, i = 0, start = 0;
            if (skipLF && n > 0) { skipLF = false; if (text.charCodeAt(0) === 10) { i = 1; start = 1; } }
            while (i < n && !stopped) {
                var ch = text.charCodeAt(i);
                if (ch !== 10 && ch !== 13) { i++; continue; }
                var l = partialLen === 0 ? text.slice(start, i) : partial + text.slice(start, i);
                partial = ''; partialLen = 0;
                i++;
                if (ch === 13) { if (i < n) { if (text.charCodeAt(i) === 10) { i++; } } else { skipLF = true; } }
                start = i;
                if (l.length > /*LINE_MAX*/) { oversize(); return; }
                line(l);
            }
            if (stopped || start >= n) { return; }
            partial += text.slice(start); partialLen += n - start;
            if (partialLen > /*LINE_MAX*/) { oversize(); }
        }
    };
}
// cfg: {url, hosts, cred, onMessage(text), onStatus(word) | null}. The statuses are connecting, open, retrying, paused and failed. A
// drop (the network failed, the server closed the stream) waits /*BACKOFF*/ ms, the last for every later try, and an
// `open` starts the list again. A status other than 200, a content type other than text/event-stream, a redirect, an
// oversize message, a host the component's Network does not allow, or a credential it was not given ends the stream
// (`failed`): no retry. `pause` (the page is hidden) closes the connection; `resume` connects at once. Every callback of a
// connection that was closed is ignored, so nothing arrives after `dispose`.
function __cui_stream_new(cfg) {
    var BACKOFF = [/*BACKOFF*/];
    var status = null, step = 0, lastId = null, ctl = null, timer = null, gen = 0, ended = false;
    function emit(s) { if (ended) { return; } status = s; if (cfg.onStatus) { cfg.onStatus(s); } }
    function halt() {
        gen++;
        if (timer !== null) { clearTimeout(timer); timer = null; }
        if (ctl !== null) { try { ctl.abort(); } catch (e) { } ctl = null; }
    }
    function fail() { halt(); emit('failed'); }
    function drop(g) {
        if (g !== gen) { return; }
        halt();
        var wait = BACKOFF[step];
        if (step < BACKOFF.length - 1) { step++; }
        timer = setTimeout(connect, wait);
        emit('retrying');
    }
    function read(reader, g) {
        var dec = new TextDecoder('utf-8');
        var parser = __cui_sse({
            id: function (v) { lastId = v === '' ? null : v; },
            message: function (t) { if (g === gen) { cfg.onMessage(t); } },
            oversize: function () { if (g === gen) { fail(); } }
        });
        function pump() {
            reader.read().then(function (r) {
                if (g !== gen) { return; }
                if (r.done) { drop(g); return; }
                parser.feed(dec.decode(r.value, { stream: true }));
                if (g === gen) { pump(); }
            }).catch(function () { drop(g); });
        }
        pump();
    }
    function connect() {
        timer = null;
        if (ended) { return; }
        halt();
        var g = gen;
        emit('connecting');
        if (g !== gen) { return; }
        var target;
        try { target = new URL(String(cfg.url), window.location.href); } catch (e) { fail(); return; }
        if (target.protocol !== 'http:' && target.protocol !== 'https:') { fail(); return; }
        if (target.origin !== window.location.origin && cfg.hosts.indexOf(target.host.toLowerCase()) === -1) { fail(); return; }
        var auth = null;
        if (cfg.cred) {
            auth = __cui_credential_check(cfg.cred, function () { });
            if (auth === false) { fail(); return; }
        }
        var headers = { Accept: 'text/event-stream' };
        if (lastId !== null) { headers['Last-Event-ID'] = lastId; }
        if (auth !== null && __cui_credential_applies(cfg.cred, target)) { headers.Authorization = auth; }
        var init = { method: 'GET', headers: headers, credentials: 'same-origin', redirect: 'manual', cache: 'no-store' };
        if (typeof AbortController === 'function') { ctl = new AbortController(); init.signal = ctl.signal; }
        fetch(target.href, init).then(function (response) {
            if (g !== gen) { return; }
            if (response.type === 'opaqueredirect' || response.status !== 200) { fail(); return; }
            var type = (response.headers.get('content-type') || '').split(';')[0].trim().toLowerCase();
            if (type !== 'text/event-stream') { fail(); return; }
            if (!response.body) { drop(g); return; }
            step = 0;
            emit('open');
            if (g !== gen) { return; }
            read(response.body.getReader(), g);
        }).catch(function () { drop(g); });
    }
    return {
        start: function () {
            if (typeof document !== 'undefined' && document.visibilityState === 'hidden') { emit('paused'); } else { connect(); }
        },
        pause: function () {
            if (!ended && (status === 'connecting' || status === 'open' || status === 'retrying')) { halt(); emit('paused'); }
        },
        resume: function () { if (!ended && status === 'paused') { connect(); } },
        dispose: function () { ended = true; halt(); }
    };
}
"#;

/// The adapter, with the shared numbers written in.
pub(crate) fn adapter_js() -> String {
    let backoff = BACKOFF_MS
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    TEMPLATE
        .replace("/*MESSAGE_MAX*/", &MESSAGE_MAX.to_string())
        .replace("/*LINE_MAX*/", &(MESSAGE_MAX + 6).to_string())
        .replace("/*BACKOFF*/", &backoff)
}
