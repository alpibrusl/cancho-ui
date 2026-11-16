#!/usr/bin/env node
// `AudioField` and `AudioClip` (docs/structure.md 12.0t) in Chromium, in the mobile profile (a Pixel 7: 412 x 915, touch), with every sound made at run time (no sound is kept in the repository):
//  A. tests/audio_corpus.json, the file `cargo test` replays (crates/cancho-ui/tests/audio.rs): the page's check of a recording text (`__cui_audio_kind`, cut out of the page's script and run
//     here) agrees with the compiler's for every case;
//  B. the recorder, with Chrome's fake microphone and its prompt answered: a tap records (the key says Stop and is pressed, the time runs, the status says so), a second tap ends it and the
//     program is given {data, seconds, mime} with a text the player accepts and that fits the cap; the cap in seconds ends a recording nobody ended; the cap in characters ends one early and
//     is never passed (a cap nobody can meet is "too big"); a render in the middle (a poll) does not end the recording and the new key is attached to it;
//  C. the states that are not a recording: the microphone refused (denied), none found, another failure, and no MediaRecorder at all (unsupported), each given to the program as `error` and
//     said in words, and the key usable again afterwards;
//  D. the player, with a WAV made here (8-bit, 8 kHz, 2.4 s): the key is named, the time is known before it plays, a tap plays (the label says Pause, the bar moves), a second tap pauses where it
//     is, a third goes on, a render in the middle keeps it playing, another clip stops this one, the end puts it back at the start; a text that is not a recording is a key that is disabled;
//  E. no <audio>, no <img>, no `blob:` or `data:` request, no policy violation, and the page's policy has no `media-src` and no `blob:`;
//  F. the gallery demo (examples/audio) builds, records and plays what it recorded.
// Outside the gate: needs node, a Chromium (Chrome found through $CHROME or the usual places, over the DevTools protocol; the checks that tap, press keys or need the fake microphone are reported
// as skipped under playwright), and a built `cancho-ui`.
//   node scripts/check-audio.js [path/to/cancho-ui]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const vm = require('vm');
const { spawnSync } = require('child_process');
const { serve, launch } = require('./lib-browser.js');

const ROOT = path.join(__dirname, '..');
const CANCHO_UI = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail !== undefined && detail !== '' ? `: ${detail}` : ''}`);
  if (!ok) { failures.push(label); }
}

const PROGRAM = [
  'component A {',
  '    state clip: String = "";',
  '    state other: String = "";',
  '    state bad: String = "this is not a recording";',
  '    state got: String = "";',
  '    state tiny: String = "";',
  '    state n: Int = 0;',
  '    state nt: Int = 0;',
  '    state one: String = "";',
  '    state no: Int = 0;',
  '    state mid: String = "";',
  '    state nm: Int = 0;',
  '    state bumps: Int = 0;',
  '    on Recorded(r) { got = r; n = n + 1; clip = json.get(r, "data", ""); }',
  '    on RecordedTiny(r) { tiny = r; nt = nt + 1; }',
  '    on RecordedOne(r) { one = r; no = no + 1; }',
  '    on RecordedMid(r) { mid = r; nm = nm + 1; }',
  '    on SetClip(v) { clip = v; }',
  '    on SetOther(v) { other = v; }',
  '    on Bump { bumps = bumps + 1; }',
  '    view {',
  '        Column {',
  '            Text { "n={n} nt={nt} nm={nm} no={no} bumps={bumps}" }',
  '            Text { "got={got}" }',
  '            Text { "tiny={tiny}" }',
  '            Text { "one={one}" }',
  '            Text { "mid={mid}" }',
  '            Button { label: "Bump" on: Bump }',
  '            AudioField { label: "Voice" max_seconds: 3 max_chars: 20000 on: Recorded }',
  '            AudioField { label: "Tiny" max_seconds: 10 max_chars: 100 on: RecordedTiny }',
  '            AudioField { label: "One" max_seconds: 1 max_chars: 48000 on: RecordedOne }',
  '            AudioField { label: "Mid" max_seconds: 10 max_chars: 2500 on: RecordedMid }',
  '            TextField { label: "clip" value: clip on: SetClip }',
  '            TextField { label: "other" value: other on: SetOther }',
  '            AudioClip { label: "One" data: clip }',
  '            AudioClip { label: "Two" data: other }',
  '            AudioClip { label: "Bad" data: bad }',
  '            AudioClip { label: "Empty" data: tiny }',
  '        }',
  '    }',
  '}',
  '',
].join('\n');

const helpers = `
  { const st = document.createElement('style'); st.textContent = 'span, p, div { overflow-wrap: anywhere; }'; document.head.appendChild(st); }
  window.sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  window.field = (label) => Array.from(document.querySelectorAll('.cui-audio-field')).find((f) => f.querySelector('.cui-audio-key').getAttribute('aria-label') === label);
  window.clip = (label) => Array.from(document.querySelectorAll('.cui-audio-clip')).find((f) => f.getAttribute('aria-label') === label);
  window.counts = () => { const t = Array.from(document.querySelectorAll('span')).map((s) => s.textContent).find((x) => /^n=/.test(x)) || ''; const o = {}; t.split(' ').forEach((kv) => { const [k, v] = kv.split('='); o[k] = Number(v); }); return o; };
  window.shown = (name) => { const t = Array.from(document.querySelectorAll('span')).map((s) => s.textContent).find((x) => x.indexOf(name + '=') === 0); return t ? t.slice(name.length + 1) : ''; };
  window.json = (name) => { try { return JSON.parse(shown(name)); } catch (e) { return null; } };
  window.mid = (el) => { const b = el.getBoundingClientRect(); return { x: Math.round((b.left + b.right) / 2), y: Math.round((b.top + b.bottom) / 2) }; };
  window.setText = (label, v) => { const i = Array.from(document.querySelectorAll('input')).find((x) => x.getAttribute('aria-label') === label); i.value = v; i.dispatchEvent(new Event('input', { bubbles: true })); };
  // a WAV made here: 8-bit mono, 8 kHz, a sine, 2.4 s
  window.makeWav = (secs) => {
    const rate = 8000, n = Math.round(rate * secs), b = new Uint8Array(44 + n), v = new DataView(b.buffer);
    const w = (o, s) => { for (let i = 0; i < s.length; i++) { b[o + i] = s.charCodeAt(i); } };
    w(0, 'RIFF'); v.setUint32(4, 36 + n, true); w(8, 'WAVE'); w(12, 'fmt '); v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
    v.setUint32(24, rate, true); v.setUint32(28, rate, true); v.setUint16(32, 1, true); v.setUint16(34, 8, true); w(36, 'data'); v.setUint32(40, n, true);
    for (let i = 0; i < n; i++) { b[44 + i] = 128 + Math.round(90 * Math.sin(2 * Math.PI * 440 * i / rate)); }
    let s = ''; for (let i = 0; i < b.length; i += 0x8000) { s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000)); }
    return btoa(s);
  };
  true
`;

(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'check-audio-'));
  const build = (name, file) => {
    const dir = path.join(tmp, name);
    const r = spawnSync(CANCHO_UI, ['build', file, '--page', '-o', dir, '--name', name], { encoding: 'utf8' });
    if (r.status !== 0) { throw new Error(`${name}: ${r.stdout}${r.stderr}`); }
    return dir;
  };
  fs.mkdirSync(path.join(tmp, 'src'));
  fs.writeFileSync(path.join(tmp, 'src', 'a.cui'), PROGRAM);
  const dir = build('audio', path.join(tmp, 'src', 'a.cui'));
  const browser = await launch(['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream']);
  console.log(`browser: ${browser.name}`);
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));

  // ---- A. the corpus ----
  let kindOf = null;
  {
    const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'tests', 'audio_corpus.json'), 'utf8'));
    const js = fs.readFileSync(path.join(dir, 'app.js'), 'utf8');
    const m = /var __cui_audio_mimes = [^\n]*\nfunction __cui_audio_kind\(t\) \{[\s\S]*?\n\}\n/.exec(js);
    check('A: the page carries __cui_audio_kind', !!m);
    const kind = kindOf = vm.runInNewContext(`${m[0]}; __cui_audio_kind`, { atob: (s) => Buffer.from(s, 'base64').toString('latin1') });
    for (const c of corpus.recordings) {
      let body = c.len - c.head.length - c.tail.length;
      let text = c.head;
      for (let i = 0; i < body; i++) { text += c.fill[i % c.fill.length]; }
      text += c.tail;
      check(`A: corpus: ${c.why}`, kind(text) === (c.ok ? c.kind : null), `${kind(text)} / ${c.ok ? c.kind : null}`);
    }
    check('A: a value that is not text is no recording', kind(5) === null && kind(null) === null && kind(undefined) === null);
  }

  const s = await serve(dir);
  const page = await browser.open(`http://127.0.0.1:${s.address().port}/`);
  const ev = (src) => page.evaluate(src);
  const haveCdp = typeof page.cdp === 'function';
  const touch = async (x, y) => {
    await page.cdp('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
    await page.cdp('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  };
  const tapField = async (label) => { await ev(`field('${label}').scrollIntoView({ block: 'center' })`); await wait(400); const p = await ev(`mid(field('${label}').querySelector('.cui-audio-key'))`); await touch(p.x, p.y); };
  const tapClip = async (label) => { await ev(`clip('${label}').scrollIntoView({ block: 'center' })`); await wait(400); const p = await ev(`mid(clip('${label}').querySelector('.cui-audio-play'))`); await touch(p.x, p.y); };
  try {
    await ev(helpers);
    await wait(500);
    if (!haveCdp) { console.log('skip B, C, D: taps need the protocol (not available under playwright)'); }

    // ---- structure ----
    const a = await ev(`(() => {
      const f = field('Voice'), k = f.querySelector('.cui-audio-key'), c = clip('One');
      return { tag: k.tagName, type: k.getAttribute('type'), pressed: k.getAttribute('aria-pressed'), label: k.getAttribute('aria-label'), icon: !!k.querySelector('svg'), cap: k.textContent, status: f.querySelector('.cui-audio-status').getAttribute('role'),
        timeHidden: f.querySelector('.cui-audio-time').getAttribute('aria-hidden'), h: Math.round(k.getBoundingClientRect().height), w: Math.round(k.getBoundingClientRect().width),
        clipRole: c.getAttribute('role'), clipLabel: c.getAttribute('aria-label'), playLabel: c.querySelector('.cui-audio-play').getAttribute('aria-label'), playDisabled: c.querySelector('.cui-audio-play').getAttribute('aria-disabled'),
        barHidden: c.querySelector('progress').getAttribute('aria-hidden'), playH: Math.round(c.querySelector('.cui-audio-play').getBoundingClientRect().height),
        audio: document.querySelectorAll('audio, img, video, source').length };
    })()`);
    check('structure: the field is a named button (a 44 px key with an icon and its caption), not pressed, with a live status line and a time hidden from a screen reader', a.tag === 'BUTTON' && a.type === 'button' && a.pressed === 'false' && a.label === 'Voice' && a.icon && a.cap === 'Voice' && a.status === 'status' && a.timeHidden === 'true' && a.h >= 44 && a.w >= 44, JSON.stringify(a));
    check('structure: the clip is a named group with a key named Play and its label, 44 px, a hidden bar; no <audio>, <img>, <video> or <source>', a.clipRole === 'group' && a.clipLabel === 'One' && a.playLabel === 'Play One' && a.barHidden === 'true' && a.playH >= 44 && a.audio === 0, JSON.stringify(a));
    check('structure: a clip with nothing to play has its key disabled (aria-disabled) and no words', a.playDisabled === 'true');

    // ---- D1. a text that is not a recording, an empty one ----
    const bad = await ev(`({ dis: clip('Bad').querySelector('.cui-audio-play').getAttribute('aria-disabled'), words: clip('Bad').querySelector('.cui-audio-status').textContent, empty: clip('Empty').querySelector('.cui-audio-status').textContent, emptyDis: clip('Empty').querySelector('.cui-audio-play').getAttribute('aria-disabled') })`);
    check('D: a text that is not a recording says so and cannot be played; an empty one is a disabled key with no words', bad.dis === 'true' && bad.words === 'This is not a recording' && bad.empty === '' && bad.emptyDis === 'true', JSON.stringify(bad));

    // ---- D. the player ----
    await ev(`setText('clip', makeWav(2.4)); setText('other', makeWav(2.4).slice(0, 0) + makeWav(2.2))`);
    await wait(800);
    const d0 = await ev(`({ time: clip('One').querySelector('.cui-audio-time').textContent, time2: clip('Two').querySelector('.cui-audio-time').textContent, dis: clip('One').querySelector('.cui-audio-play').getAttribute('aria-disabled'), len: document.querySelector('input').value.length })`);
    check('D: the length is known before it plays (decoded at once, nothing sounded)', d0.time === '0:00 / 0:02' && d0.dis === 'false', JSON.stringify(d0));
    if (haveCdp) {
      await tapClip('One'); await wait(900);
      const d1 = await ev(`({ label: clip('One').querySelector('.cui-audio-play').getAttribute('aria-label'), bar: clip('One').querySelector('progress').value, time: clip('One').querySelector('.cui-audio-time').textContent, icon: clip('One').querySelector('.cui-audio-play svg') !== null })`);
      check('D: a tap plays: the key says Pause and the label, the bar moves, the time runs', d1.label === 'Pause One' && d1.bar > 100 && d1.bar < 900 && /^0:0[01] \/ 0:02$/.test(d1.time) && d1.icon, JSON.stringify(d1));
      await ev(`Array.from(document.querySelectorAll('button')).find((b) => b.textContent === 'Bump').click()`);
      await wait(500);
      const d2 = await ev(`({ label: clip('One').querySelector('.cui-audio-play').getAttribute('aria-label'), bar: clip('One').querySelector('progress').value, st: counts() })`);
      check('D: a render in the middle (the page is drawn again) leaves it playing, and the new key goes on with the bar', d2.label === 'Pause One' && d2.bar > 300 && d2.st.bumps === 1, JSON.stringify(d2));
      await tapClip('One'); await wait(200);
      const p1 = await ev(`clip('One').querySelector('progress').value`);
      await wait(700);
      const d3 = await ev(`({ label: clip('One').querySelector('.cui-audio-play').getAttribute('aria-label'), bar: clip('One').querySelector('progress').value })`);
      check('D: a second tap pauses where it is (the bar stays), the key says Play', d3.label === 'Play One' && Math.abs(d3.bar - p1) <= 2 && d3.bar > 300, JSON.stringify([p1, d3]));
      await tapClip('One'); await wait(500);
      const d4 = await ev(`({ label: clip('One').querySelector('.cui-audio-play').getAttribute('aria-label'), bar: clip('One').querySelector('progress').value })`);
      check('D: a third tap goes on from there', d4.label === 'Pause One' && d4.bar > d3.bar, JSON.stringify([d3.bar, d4]));
      await tapClip('Two'); await wait(500);
      const d5 = await ev(`({ one: clip('One').querySelector('.cui-audio-play').getAttribute('aria-label'), oneBar: clip('One').querySelector('progress').value, two: clip('Two').querySelector('.cui-audio-play').getAttribute('aria-label'), twoBar: clip('Two').querySelector('progress').value, playing: Array.from(document.querySelectorAll('.cui-audio-play')).filter((b) => /^Pause/.test(b.getAttribute('aria-label'))).length })`);
      check('D: another clip stops this one and starts at its beginning: one clip plays at a time', d5.one === 'Play One' && d5.oneBar === 0 && d5.two === 'Pause Two' && d5.twoBar > 0 && d5.twoBar < 500 && d5.playing === 1, JSON.stringify(d5));
      await wait(2300);
      const d6 = await ev(`({ two: clip('Two').querySelector('.cui-audio-play').getAttribute('aria-label'), bar: clip('Two').querySelector('progress').value, time: clip('Two').querySelector('.cui-audio-time').textContent })`);
      check('D: at the end it stops and is back at its start', d6.two === 'Play Two' && d6.bar === 0 && d6.time === '0:00 / 0:02', JSON.stringify(d6));
    }

    // ---- C. what is not a recording ----
    if (haveCdp) {
      // 'real' is Chrome's fake microphone, wrapped to remember every stream it gives, so that the check can see that each one was let go
      const stub = (mode) => ev(`(() => { window.__realGUM = window.__realGUM || navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
        navigator.mediaDevices.getUserMedia = ${mode === 'real' ? '(c) => window.__realGUM(c).then((st) => { (window.__streams = window.__streams || []).push(st); return st; })' : `() => Promise.reject(new DOMException('x', '${mode}'))`}; return true; })()`);
      for (const [mode, code, words] of [['NotAllowedError', 'denied', 'The microphone is blocked'], ['NotFoundError', 'no microphone', 'No microphone found'], ['AbortError', 'failed', 'The recording failed']]) {
        await stub(mode);
        const before = (await ev(`counts()`)).n;
        await tapField('Voice'); await wait(400);
        const r = await ev(`({ n: counts().n, got: json('got'), status: field('Voice').querySelector('.cui-audio-status').textContent, pressed: field('Voice').querySelector('.cui-audio-key').getAttribute('aria-pressed'), cap: field('Voice').querySelector('.cui-audio-key').textContent })`);
        check(`C: ${mode} is given to the program as {"error":"${code}"}, said in words, and the key is a key again`, r.n === before + 1 && r.got && r.got.error === code && Object.keys(r.got).length === 1 && r.status === words && r.pressed === 'false' && r.cap === 'Voice', JSON.stringify(r));
      }
      // unsupported: no MediaRecorder
      await ev(`window.__MR = window.MediaRecorder; delete window.MediaRecorder; true`);
      await tapField('Voice'); await wait(300);
      const un = await ev(`({ got: json('got'), status: field('Voice').querySelector('.cui-audio-status').textContent })`);
      // a field that has not been tapped says so at its next drawing, in words, before the person tries
      await ev(`Array.from(document.querySelectorAll('button')).find((b) => b.textContent === 'Bump').click()`);
      await wait(300);
      const un2 = await ev(`field('Tiny').querySelector('.cui-audio-status').textContent`);
      check('C: a field that was not tapped says at its next drawing, in words, that recording is not available here', un2 === 'Recording is not available here', un2);
      check('C: no MediaRecorder: {"error":"unsupported"} and the words say it', un.got && un.got.error === 'unsupported' && un.status === 'Recording is not available here', JSON.stringify(un));
      await ev(`window.MediaRecorder = window.__MR; true`);
      await stub('real');
    }

    // ---- B. the recorder (Chrome's fake microphone) ----
    if (haveCdp) {
      const n0 = (await ev(`counts()`)).n;
      await tapField('Voice'); await wait(700);
      const r1 = await ev(`(() => { const f = field('Voice'), k = f.querySelector('.cui-audio-key'); return { pressed: k.getAttribute('aria-pressed'), cap: k.textContent, status: f.querySelector('.cui-audio-status').textContent, time: f.querySelector('.cui-audio-time').textContent, icon: !!k.querySelector('svg') }; })()`);
      check('B: a tap records: the key is pressed and says Stop, the status says what is going on, the time runs', r1.pressed === 'true' && r1.cap === 'Stop' && /^Recording, up to 3 seconds$/.test(r1.status) && /^0:0[01] \/ 0:03$/.test(r1.time) && r1.icon, JSON.stringify(r1));
      await ev(`Array.from(document.querySelectorAll('button')).find((b) => b.textContent === 'Bump').click()`);
      await wait(600);
      const r2 = await ev(`(() => { const f = field('Voice'), k = f.querySelector('.cui-audio-key'); return { pressed: k.getAttribute('aria-pressed'), cap: k.textContent, time: f.querySelector('.cui-audio-time').textContent, bumps: counts().bumps }; })()`);
      check('B: a render in the middle does not end the recording: the new key is the recording\'s, still pressed, the time still running', r2.pressed === 'true' && r2.cap === 'Stop' && /^0:0[12] \/ 0:03$/.test(r2.time) && r2.bumps >= 1, JSON.stringify(r2));
      await tapField('Voice'); await wait(900);
      const r3 = await ev(`(() => { const f = field('Voice'); return { n: counts().n, got: json('got'), status: f.querySelector('.cui-audio-status').textContent, pressed: f.querySelector('.cui-audio-key').getAttribute('aria-pressed'), cap: f.querySelector('.cui-audio-key').textContent, time: f.querySelector('.cui-audio-time').textContent }; })()`);
      const g = r3.got || {};
      const kind = g.data ? kindOf(g.data) : null;
      check('B: a second tap ends it and the program is given {data, seconds, mime}, once', r3.n === n0 + 1 && Object.keys(g).sort().join() === 'data,mime,seconds' && typeof g.data === 'string' && Number.isInteger(g.seconds) && g.seconds >= 1 && g.seconds <= 3 && r3.pressed === 'false' && r3.cap === 'Voice' && r3.time === '' && r3.status === 'Recording added', JSON.stringify({ n: r3.n, keys: Object.keys(g), seconds: g.seconds, status: r3.status, pressed: r3.pressed }));
      check('B: the text is a recording by the compiler\'s own rule (told by its first bytes), its mime is the one they say, and it fits the cap', kind !== null && g.mime === ({ webm: 'audio/webm', ogg: 'audio/ogg', mp4: 'audio/mp4', wav: 'audio/wav' })[kind] && g.data.length <= 20000, `${kind} ${g.mime} ${g.data && g.data.length}`);
      const played = await ev(`(async () => { await sleep(700); const c = clip('One'); return { time: c.querySelector('.cui-audio-time').textContent, dis: c.querySelector('.cui-audio-play').getAttribute('aria-disabled') }; })()`);
      check('B: and the player plays what the recorder made (it decodes, with a length)', /^0:00 \/ 0:0[0-3]$/.test(played.time) && played.dis === 'false', JSON.stringify(played));

      // the cap in seconds: nobody ends it
      const n1 = (await ev(`counts()`)).n;
      const t0 = Date.now();
      await tapField('Voice');
      let ended = false;
      for (let i = 0; i < 40 && !ended; i++) { await wait(200); ended = (await ev(`counts().n`)) === n1 + 1; }
      const took = Date.now() - t0;
      const cap = await ev(`json('got')`);
      check('B: a recording nobody ends stops by itself at max_seconds (3), and says how long it was', ended && took >= 2600 && took <= 5500 && cap && cap.seconds >= 2 && cap.seconds <= 3, `${took} ms ${cap && cap.seconds}`);

      // the cap in characters: 100 is a cap nobody can meet
      const nt = (await ev(`counts().nt`));
      const t1 = Date.now();
      await tapField('Tiny');
      let tdone = false;
      for (let i = 0; i < 40 && !tdone; i++) { await wait(200); tdone = (await ev(`counts().nt`)) === nt + 1; }
      const tiny = await ev(`({ got: json('tiny'), status: field('Tiny').querySelector('.cui-audio-status').textContent })`);
      check('B: a cap in characters is never passed: a cap nobody can meet (100) stops the recording at the first piece and says "too big", long before max_seconds', tdone && Date.now() - t1 < 5000 && tiny.got && tiny.got.error === 'too big' && tiny.status === 'The recording is too long', JSON.stringify(tiny));
    }
    if (haveCdp) {
      // a cap in characters that a recording can nearly meet: it ends the recording early, and what the program is given is never longer than the cap
      const nm = (await ev(`counts().nm`));
      const t2 = Date.now();
      // found by use: while the page was wider than the screen (a recording's text is one long word) a touch landed beside the key; the long text now wraps (the style in `helpers`), and a tap that began nothing is tried again, by the mouse
      for (let tries = 0; tries < 3; tries++) {
        if (tries === 0) { await tapField('Mid'); } else {
          const q = await ev(`mid(field('Mid').querySelector('.cui-audio-key'))`);
          await page.cdp('Input.dispatchMouseEvent', { type: 'mousePressed', x: q.x, y: q.y, button: 'left', clickCount: 1 });
          await page.cdp('Input.dispatchMouseEvent', { type: 'mouseReleased', x: q.x, y: q.y, button: 'left', clickCount: 1 });
        }
        await wait(300);
        if (await ev(`field('Mid').querySelector('.cui-audio-key').getAttribute('aria-pressed') === 'true' || /^(Waiting|Recording,)/.test(field('Mid').querySelector('.cui-audio-status').textContent)`)) { break; }
      }
      let mdone = false;
      for (let i = 0; i < 60 && !mdone; i++) { await wait(200); mdone = (await ev(`counts().nm`)) === nm + 1; }
      const mid = await ev(`json('mid')`);
      const ok = mid && ((mid.data && mid.data.length <= 2500 && kindOf(mid.data) !== null) || mid.error === 'too big');
      check('B: a cap of 2,500 characters ends a 10-second recording early and is never passed (a text of at most 2,500 characters, or "too big")', mdone && Date.now() - t2 < 8000 && ok, `${Date.now() - t2} ms ${mid && (mid.data ? mid.data.length : mid.error)}`);
    }
    if (haveCdp) {
      // the cap in seconds alone: one second, with room for far more text than a second makes, so that only the clock can end it
      const no0 = (await ev(`counts().no`));
      const t3 = Date.now();
      await tapField('One');
      let odone = false;
      for (let i = 0; i < 40 && !odone; i++) { await wait(100); odone = (await ev(`counts().no`)) === no0 + 1; }
      const one = await ev(`json('one')`);
      check('B: max_seconds alone ends a recording: one second, with room for far more text, is ended by the clock at about one second and says 1', odone && Date.now() - t3 >= 800 && Date.now() - t3 <= 2000 && one && one.seconds === 1 && one.data && one.data.length < 43000, `${Date.now() - t3} ms ${one && one.seconds} ${one && one.data ? one.data.length : one && one.error}`);

    }
    if (haveCdp) {
      const tr = await ev(`(window.__streams || []).map((st) => st.getTracks().map((t) => t.readyState).join(','))`);
      check('B: every microphone stream the page was given was let go (every track ended) when its recording ended', tr.length >= 4 && tr.every((x) => x !== '' && x.split(',').every((y) => y === 'ended')), JSON.stringify(tr));
    }
    check('E: no page error, no console error, no policy violation', page.problems.filter((p) => !/favicon\.ico/.test(p)).length === 0, page.problems.join('; '));
    const res = await ev(`(() => { const csp = (document.querySelector('meta[http-equiv="Content-Security-Policy"]') || {}).content || ''; return { csp, media: /media-src/.test(csp), blob: /blob:/.test(csp), reqs: performance.getEntriesByType('resource').map((e) => e.name).filter((n) => /^(blob|data):/.test(n)), tags: document.querySelectorAll('audio, img, video, source, track').length }; })()`);
    check('E: the policy is the baseline (no media-src, no blob:), no blob: or data: request was made, and the page has no media element', !res.media && !res.blob && res.reqs.length === 0 && res.tags === 0 && /^default-src 'self'/.test(res.csp), JSON.stringify(res));
  } finally { await page.close(); s.close(); }

  // ---- F. the gallery demo ----
  {
    const sv = await serve(build('demo', path.join(ROOT, 'examples/audio/audio.cui')));
    const pg = await browser.open(`http://127.0.0.1:${sv.address().port}/`);
    try {
      await pg.evaluate(helpers);
      await wait(300);
      if (typeof pg.cdp === 'function') {
        const tap = async () => {
          const p = await pg.evaluate(`mid(document.querySelector('.cui-audio-key'))`);
          await pg.cdp('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: p.x, y: p.y }] });
          await pg.cdp('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
        };
        await tap(); await wait(1300); await tap(); await wait(1200);
        const r = await pg.evaluate(`({ note: Array.from(document.querySelectorAll('[role=status]')).map((e) => e.textContent).join('|'), clips: document.querySelectorAll('.cui-audio-clip').length, times: Array.from(document.querySelectorAll('.cui-audio-clip .cui-audio-time')).map((t) => t.textContent), media: document.querySelectorAll('audio, img').length })`);
        check('F: the demo records and both players know its length', /Recording ready\./.test(r.note) && r.clips === 2 && r.times.every((t) => /^0:0[0-9] \/ 0:0[0-9]$/.test(t)) && r.media === 0, JSON.stringify(r));
      }
      check('F: the demo has no page or console error', pg.problems.filter((p) => !/favicon\.ico/.test(p)).length === 0, pg.problems.join('; '));
    } finally { await pg.close(); sv.close(); }
  }

  await browser.close();
  fs.rmSync(tmp, { recursive: true, force: true });
  if (failures.length) { console.log(`\n${failures.length} check(s) failed`); process.exit(1); }
  console.log('\nall checks passed');
  // the browser's own processes can keep a pipe open after it is killed, and node would wait for them: the work is done
  process.exit(0);
})().catch((e) => { console.error(e); process.exit(2); });
