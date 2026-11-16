//! `AudioField` and `AudioClip` on the page (docs/structure.md 12.0t). The elements are a few DOM calls; what they do is in three scripts, each written only for a program that has the
//! element: `AUDIO_OK_JS` (the check of a recording text), `AUDIO_FIELD_JS` (the recorder) and `AUDIO_CLIP_JS` (the player). Every string from the program goes in with `textContent` or
//! `setAttribute`. The sound is decoded by the audio engine and played from it, so the page has no audio element, no `blob:` or `data:` address and no request, and its policy needs nothing for it.
//! The page is drawn again from nothing at every render, so a recording in progress and a clip that is playing live in a registry the script keeps, not in the elements: each render's new
//! elements are attached to it.

use cancho_ui_ir::ir::*;

use crate::emit::js_string;
use crate::expr::emit_expr_runtime;

fn label_js(accessibility: &IrAccessibility, params: &[&str]) -> String {
    match (&accessibility.label, &accessibility.label_expr) {
        (Some(l), _) => js_string(l),
        (None, Some(e)) => format!("String({})", emit_expr_runtime(e, params)),
        (None, None) => "\"\"".to_string(),
    }
}

/// An `AudioField`: a `div` of a key (a button that holds an icon and its caption), the time while it records (hidden from a screen reader) and the status line (a live region).
/// The two icons the key shows are drawn into detached holders here, so that the icon table of the page has them, and the script copies the one it needs.
pub(crate) fn emit_field(
    indent: &str,
    var: &str,
    parent: &str,
    key: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let (secs, chars) = cancho_ui_ir::audio::limits_of(bindings);
    let icon = cancho_ui_ir::audio::icon_of(bindings);
    let event = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("on", IrBindingValue::EventRef(e)) => Some(e.clone()),
        _ => None,
    });
    let label = label_js(accessibility, params);
    let deliver = match &event {
        Some(e) => format!("function (v) {{ __on_{e}(v); }}"),
        None => "function () {}".to_string(),
    };
    format!(
        "{indent}const {var} = document.createElement('div');\n\
{indent}{var}.className = 'cui-audio-field';\n\
{indent}const {var}_key = document.createElement('button');\n\
{indent}{var}_key.setAttribute('type', 'button');\n\
{indent}{var}_key.className = 'cui-audio-key';\n\
{indent}{var}_key.setAttribute('aria-label', {label});\n\
{indent}const {var}_i0 = document.createElement('span');\n\
{indent}__cui_icon({var}_i0, {icon}, \"cui-icon cui-icon-m\", \"20\", null);\n\
{indent}const {var}_i1 = document.createElement('span');\n\
{indent}__cui_icon({var}_i1, \"square\", \"cui-icon cui-icon-m\", \"20\", null);\n\
{indent}{var}.appendChild({var}_key);\n\
{indent}const {var}_time = document.createElement('span');\n\
{indent}{var}_time.className = 'cui-audio-time';\n\
{indent}{var}_time.setAttribute('aria-hidden', 'true');\n\
{indent}{var}.appendChild({var}_time);\n\
{indent}const {var}_st = document.createElement('span');\n\
{indent}{var}_st.className = 'cui-audio-status';\n\
{indent}{var}_st.setAttribute('role', 'status');\n\
{indent}{var}.appendChild({var}_st);\n\
{indent}__cui_audio_field_wire(root, \"{key}\", {{ key: {var}_key, time: {var}_time, status: {var}_st, icons: [{var}_i0, {var}_i1], label: {label} }}, {secs}, {chars}, {deliver});\n\
{indent}{parent}.appendChild({var});\n",
        icon = js_string(icon),
    )
}

/// An `AudioClip`: a `div` (a named group) of a play key, a progress bar and the time; `__cui_audio_clip_draw` fills it from the clip's text.
pub(crate) fn emit_clip(
    indent: &str,
    var: &str,
    parent: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let data = bindings
        .iter()
        .find(|b| b.key == "data")
        .and_then(|b| match &b.value {
            IrBindingValue::Const { expr, .. } => Some(emit_expr_runtime(expr, params)),
            _ => None,
        })
        .unwrap_or_else(|| "\"\"".to_string());
    let label = label_js(accessibility, params);
    format!(
        "{indent}const {var} = document.createElement('div');\n\
{indent}{var}.className = 'cui-audio-clip';\n\
{indent}{var}.setAttribute('role', 'group');\n\
{indent}{var}.setAttribute('aria-label', {label});\n\
{indent}const {var}_i0 = document.createElement('span');\n\
{indent}__cui_icon({var}_i0, \"play\", \"cui-icon cui-icon-m\", \"20\", null);\n\
{indent}const {var}_i1 = document.createElement('span');\n\
{indent}__cui_icon({var}_i1, \"pause\", \"cui-icon cui-icon-m\", \"20\", null);\n\
{indent}__cui_audio_clip_draw({var}, {data}, {label}, [{var}_i0, {var}_i1]);\n\
{indent}{parent}.appendChild({var});\n"
    )
}

/// The check of a recording text, shared by the player and the recorder. It is `cancho_ui_ir::audio::audio_kind` in JavaScript: `tests/audio_corpus.json` is run against both.
pub(crate) const AUDIO_OK_JS: &str = r##"// Builtin: the check of a recording text (docs/structure.md 12.0t): base64 of WebM, Ogg, MP4 or WAV, told by its first bytes, 100 to 48,000 characters.
var __cui_audio_mimes = { webm: "audio/webm", ogg: "audio/ogg", mp4: "audio/mp4", wav: "audio/wav" };
function __cui_audio_kind(t) {
    var h, b0, b1, b2, b3;
    if (typeof t !== "string" || t.length < 100 || t.length > 48000 || !/^[A-Za-z0-9+\/]+={0,2}$/.test(t)) { return null; }
    try { h = atob(t.slice(0, 16)); } catch (e) { return null; }
    b0 = h.charCodeAt(0); b1 = h.charCodeAt(1); b2 = h.charCodeAt(2); b3 = h.charCodeAt(3);
    if (b0 === 26 && b1 === 69 && b2 === 223 && b3 === 163) { return "webm"; }
    if (h.slice(0, 4) === "OggS") { return "ogg"; }
    if (h.slice(4, 8) === "ftyp") { return "mp4"; }
    if (h.slice(0, 4) === "RIFF" && h.slice(8, 12) === "WAVE") { return "wav"; }
    return null;
}
function __cui_audio_clock(s) { s = Math.max(0, Math.floor(s)); return Math.floor(s / 60) + ":" + ("0" + (s % 60)).slice(-2); }
"##;

/// The recorder of an `AudioField`. A recording in progress is kept on the component's root by the place of the field in the source: the page is drawn again at every render (a poll, a
/// typed letter), which makes a new key, and the new key is attached to the recording that goes on.
pub(crate) const AUDIO_FIELD_JS: &str = r##"// Platform adapter: AudioField (docs/structure.md 12.0t). The microphone is asked for by a tap, in the browser's own prompt; MediaRecorder records until the person taps again, `secs`
// have gone, or the base64 text would pass `chars`. Nothing is sent anywhere: the text is given to the program's event.
var __cui_audio_words = { ask: "Waiting for the microphone", rec: "Recording", work: "Preparing the recording", ok: "Recording added", denied: "The microphone is blocked", unsupported: "Recording is not available here", mic: "No microphone found", failed: "The recording failed", big: "The recording is too long" };
var __cui_audio_codes = { denied: "denied", unsupported: "unsupported", "no microphone": "mic", failed: "failed", "too big": "big" };
function __cui_audio_supported() { return !!(typeof navigator !== "undefined" && navigator.mediaDevices && navigator.mediaDevices.getUserMedia && typeof MediaRecorder !== "undefined"); }
function __cui_audio_type() {
    var cand = ["audio/webm;codecs=opus", "audio/ogg;codecs=opus", "audio/mp4", "audio/webm"], i;
    if (typeof MediaRecorder.isTypeSupported !== "function") { return ""; }
    for (i = 0; i < cand.length; i++) { if (MediaRecorder.isTypeSupported(cand[i])) { return cand[i]; } }
    return "";
}
function __cui_audio_paint(st) {
    var u = st.ui, rec = st.phase === "recording", icon, i;
    if (!u) { return; }
    u.key.setAttribute("aria-pressed", rec ? "true" : "false");
    u.key.setAttribute("aria-disabled", st.phase === "asking" || st.phase === "working" ? "true" : "false");
    if (st.shown !== (rec ? 1 : 0) || u.key.childNodes.length === 0) {
        while (u.key.firstChild) { u.key.removeChild(u.key.firstChild); }
        icon = u.icons[rec ? 1 : 0].firstChild;
        if (icon) { u.key.appendChild(icon.cloneNode(true)); }
        i = document.createElement("span"); i.className = "cui-audio-cap"; i.textContent = rec ? "Stop" : u.label; u.key.appendChild(i);
        st.shown = rec ? 1 : 0;
    }
    u.status.textContent = st.words || "";
    u.time.textContent = rec ? __cui_audio_clock((Date.now() - st.t0) / 1000) + " / " + __cui_audio_clock(st.secs) : "";
}
function __cui_audio_release(st) {
    if (st.timer) { clearInterval(st.timer); st.timer = 0; }
    if (st.stream) { st.stream.getTracks().forEach(function (t) { t.stop(); }); st.stream = null; }
}
function __cui_audio_fail(st, code) {
    __cui_audio_release(st);
    st.phase = "idle"; st.rec = null; st.words = __cui_audio_words[__cui_audio_codes[code]];
    __cui_audio_paint(st);
    st.deliver(JSON.stringify({ error: code }));
}
function __cui_audio_stop(st) {
    var rec = st.rec;
    if (st.phase !== "recording" || !rec) { return; }
    st.phase = "working"; st.words = __cui_audio_words.work; st.elapsed = Date.now() - st.t0;
    if (st.timer) { clearInterval(st.timer); st.timer = 0; }
    __cui_audio_paint(st);
    try { if (rec.state !== "inactive") { rec.stop(); } } catch (e) { __cui_audio_fail(st, "failed"); }
}
function __cui_audio_done(st, n, type) {
    var blob, r;
    if (st.seq !== n) { return; }
    __cui_audio_release(st);
    blob = new Blob(st.chunks, { type: type });
    st.chunks = []; st.rec = null;
    if (blob.size === 0) { __cui_audio_fail(st, "failed"); return; }
    r = new FileReader();
    r.onload = function () {
        var b64 = String(r.result).split(",")[1] || "", kind;
        if (st.seq !== n) { return; }
        if (b64.length > st.chars) { __cui_audio_fail(st, "too big"); return; }
        kind = __cui_audio_kind(b64);
        if (kind === null) { __cui_audio_fail(st, "failed"); return; }
        st.phase = "idle"; st.words = __cui_audio_words.ok;
        __cui_audio_paint(st);
        st.deliver(JSON.stringify({ data: b64, seconds: Math.min(st.secs, Math.max(1, Math.round(st.elapsed / 1000))), mime: __cui_audio_mimes[kind] }));
    };
    r.onerror = function () { if (st.seq === n) { __cui_audio_fail(st, "failed"); } };
    r.readAsDataURL(blob);
}
function __cui_audio_start(st) {
    var n, type, bps;
    if (!__cui_audio_supported()) { __cui_audio_fail(st, "unsupported"); return; }
    n = st.seq = (st.seq || 0) + 1;
    st.phase = "asking"; st.words = __cui_audio_words.ask; __cui_audio_paint(st);
    navigator.mediaDevices.getUserMedia({ audio: true }).then(function (stream) {
        var opts = {}, rec;
        if (st.seq !== n) { stream.getTracks().forEach(function (t) { t.stop(); }); return; }
        st.stream = stream;
        type = __cui_audio_type();
        // the bit rate that fits the cap: the most bytes the text may hold (three in four of its characters), over the seconds, with a fifth to spare for the container
        bps = Math.max(6000, Math.min(128000, Math.floor(st.chars * 0.75 * 8 * 0.8 / st.secs)));
        if (type) { opts.mimeType = type; }
        opts.audioBitsPerSecond = bps;
        try { rec = new MediaRecorder(stream, opts); } catch (e) { try { rec = new MediaRecorder(stream); } catch (e2) { __cui_audio_fail(st, "unsupported"); return; } }
        st.rec = rec; st.chunks = []; st.bytes = 0;
        rec.ondataavailable = function (e) {
            if (st.seq !== n || !e.data || !e.data.size) { return; }
            st.chunks.push(e.data); st.bytes += e.data.size;
            // a text of the next slice would pass the cap: stop now, with what fits
            if (4 * Math.ceil(st.bytes / 3) >= st.chars * 0.9) { __cui_audio_stop(st); }
        };
        rec.onstop = function () { __cui_audio_done(st, n, rec.mimeType || type || "audio/webm"); };
        rec.onerror = function () { if (st.seq === n) { __cui_audio_fail(st, "failed"); } };
        st.t0 = Date.now(); st.phase = "recording"; st.words = __cui_audio_words.rec + ", up to " + st.secs + " seconds";
        rec.start(250);
        st.timer = setInterval(function () {
            if (Date.now() - st.t0 >= st.secs * 1000) { __cui_audio_stop(st); } else { __cui_audio_paint(st); }
        }, 200);
        __cui_audio_paint(st);
    }, function (err) {
        var name = err && err.name;
        if (st.seq !== n) { return; }
        __cui_audio_fail(st, name === "NotAllowedError" || name === "SecurityError" || name === "PermissionDeniedError" ? "denied" : (name === "NotFoundError" || name === "OverconstrainedError" || name === "DevicesNotFoundError" ? "no microphone" : "failed"));
    });
}
function __cui_audio_field_wire(root, key, ui, secs, chars, deliver) {
    var all = (root.__cui_audio_st = root.__cui_audio_st || {}), st = (all[key] = all[key] || { phase: "idle", words: "", seq: 0, shown: -1 });
    st.ui = ui; st.secs = secs; st.chars = chars; st.deliver = deliver; st.shown = -1;
    if (st.phase === "idle" && !st.words && !__cui_audio_supported()) { st.words = __cui_audio_words.unsupported; }
    __cui_audio_paint(st);
    ui.key.addEventListener("click", function () {
        if (st.phase === "recording") { __cui_audio_stop(st); return; }
        if (st.phase === "idle") { __cui_audio_start(st); }
    });
}
"##;

/// The player of an `AudioClip`. What plays is in `S` (one clip at a time, by its text), and every clip drawn is painted from it: a render that rebuilds the elements of the clip that is
/// playing finds it playing, with its progress.
pub(crate) const AUDIO_CLIP_JS: &str = r##"// Builtin: AudioClip (docs/structure.md 12.0t). The browser's audio engine decodes the text and plays it: no audio element, no address, no request.
var __cui_audio_S = { ctx: null, cur: null, uis: [], bufs: {}, order: [], timer: 0 };
function __cui_audio_decode(data) {
    var S = __cui_audio_S, bin, bytes, i, OC, c;
    if (S.bufs[data]) { return S.bufs[data]; }
    OC = window.OfflineAudioContext || window.webkitOfflineAudioContext;
    S.bufs[data] = new Promise(function (ok, no) {
        try {
            bin = atob(data); bytes = new Uint8Array(bin.length);
            for (i = 0; i < bin.length; i++) { bytes[i] = bin.charCodeAt(i); }
            c = new OC(1, 1, 44100);
            var p = c.decodeAudioData(bytes.buffer, ok, no);
            if (p && p.catch) { p.catch(no); }
        } catch (e) { no(e); }
    });
    S.order.push(data);
    if (S.order.length > 40) { delete S.bufs[S.order.shift()]; }
    return S.bufs[data];
}
function __cui_audio_pos(c) { var S = __cui_audio_S; return c.playing ? c.offset + (S.ctx.currentTime - c.t0) : c.offset; }
function __cui_audio_paint_clips() {
    var S = __cui_audio_S, live = [];
    S.uis.forEach(function (u) {
        var c = S.cur && S.cur.key === u.data ? S.cur : null, playing = !!(c && c.playing), pos = c ? Math.min(u.dur, __cui_audio_pos(c)) : 0, icon;
        // a clip that was in the page and is not any more (the page was drawn again) is let go; one that has not been put in yet is kept
        if (u.el.isConnected) { u.seen = true; } else if (u.seen) { return; }
        live.push(u);
        u.btn.setAttribute("aria-label", (playing ? "Pause " : "Play ") + u.label);
        u.btn.setAttribute("aria-disabled", u.dur > 0 ? "false" : "true");
        if (u.shown !== (playing ? 1 : 0)) {
            while (u.btn.firstChild) { u.btn.removeChild(u.btn.firstChild); }
            icon = u.icons[playing ? 1 : 0].firstChild;
            if (icon) { u.btn.appendChild(icon.cloneNode(true)); }
            u.shown = playing ? 1 : 0;
        }
        u.bar.value = u.dur > 0 ? Math.round(1000 * pos / u.dur) : 0;
        u.time.textContent = u.dur > 0 ? __cui_audio_clock(pos) + " / " + __cui_audio_clock(u.dur) : "";
    });
    S.uis = live.length > 300 ? live.slice(-300) : live;
    if (!(S.cur && S.cur.playing) && S.timer) { clearInterval(S.timer); S.timer = 0; }
}
function __cui_audio_halt(c) { if (c && c.src) { c.src.onended = null; try { c.src.stop(); } catch (e) { /* not started */ } c.src = null; } }
function __cui_audio_go(u) {
    var S = __cui_audio_S, AC = window.AudioContext || window.webkitAudioContext, c = S.cur, offset;
    if (!AC) { return; }
    // one clip plays at a time: another one stops this one and starts at its beginning
    if (c && c.key !== u.data) { __cui_audio_halt(c); S.cur = c = null; }
    offset = c ? __cui_audio_pos(c) : 0;
    if (c && c.playing) { __cui_audio_halt(c); c.offset = offset; c.playing = false; __cui_audio_paint_clips(); return; }
    if (offset >= u.dur - 0.05) { offset = 0; }
    S.ctx = S.ctx || new AC();
    c = S.cur = { key: u.data, offset: offset, t0: 0, playing: false, src: null };
    S.ctx.resume().then(function () {
        var src = S.ctx.createBufferSource();
        if (S.cur !== c) { return; }
        src.buffer = u.buf; src.connect(S.ctx.destination);
        src.onended = function () { if (S.cur === c && c.src === src) { c.src = null; S.cur = null; __cui_audio_paint_clips(); } };
        c.src = src; c.t0 = S.ctx.currentTime; c.playing = true;
        src.start(0, c.offset);
        if (!S.timer) { S.timer = setInterval(__cui_audio_paint_clips, 100); }
        __cui_audio_paint_clips();
    }, function () { if (S.cur === c) { S.cur = null; } __cui_audio_paint_clips(); });
}
function __cui_audio_clip_draw(el, data, label, icons) {
    var S = __cui_audio_S, btn = document.createElement("button"), bar = document.createElement("progress"), time = document.createElement("span"), st = document.createElement("span"), u;
    btn.setAttribute("type", "button"); btn.className = "cui-audio-play";
    bar.className = "cui-audio-bar"; bar.max = 1000; bar.value = 0; bar.setAttribute("aria-hidden", "true");
    time.className = "cui-audio-time"; time.setAttribute("aria-hidden", "true");
    st.className = "cui-audio-status"; st.setAttribute("role", "status");
    el.appendChild(btn); el.appendChild(bar); el.appendChild(time); el.appendChild(st);
    u = { el: el, btn: btn, bar: bar, time: time, label: String(label), icons: icons, data: data, dur: 0, buf: null, shown: -1 };
    if (__cui_audio_kind(data) === null) {
        // nothing to play: a key that says so, and (when there is a text that is not a recording) a word for it
        btn.setAttribute("aria-disabled", "true"); btn.setAttribute("aria-label", "Play " + u.label);
        if (icons[0].firstChild) { btn.appendChild(icons[0].firstChild.cloneNode(true)); }
        if (data) { st.textContent = "This is not a recording"; }
        return;
    }
    S.uis.push(u);
    btn.addEventListener("click", function () { if (u.dur > 0) { __cui_audio_go(u); } });
    __cui_audio_paint_clips();
    __cui_audio_decode(data).then(function (buf) { u.buf = buf; u.dur = buf.duration; __cui_audio_paint_clips(); }, function () { st.textContent = "This recording cannot be played"; });
}
"##;
