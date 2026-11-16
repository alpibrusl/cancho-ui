// The tests of scripts/check-photo.js that run inside the page (docs/structure.md 12.0r). `pageTests(win, cfg)` is serialised with `toString()` and run in the page
// under test, so it is self-contained: nothing outside its own body. Every image it needs is drawn here, at run time, on a canvas (no picture is kept in the repository).
// `win` is the window of the page; `cfg.mode` is `main` (every behaviour of the two elements) or `cap` (one field, one image, the payload it gives).
// It returns the list of {label, ok, detail} that check-photo.js prints; a check that throws is a failed check, not a crash.
async function pageTests(win, cfg) {
    const doc = win.document;
    const results = [];
    const check = (label, ok, detail) => results.push({ label, ok: !!ok, detail: detail === undefined ? '' : String(detail) });
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const until = async (fn, ms) => {
        const end = Date.now() + (ms || 8000);
        while (Date.now() < end) { if (await fn()) { return true; } await sleep(25); }
        return false;
    };
    const violations = [];
    win.addEventListener('securitypolicyviolation', (e) => violations.push(e.effectiveDirective + ' ' + e.blockedURI));

    // ---- images, drawn now ----
    const canvas = (w, h) => { const c = doc.createElement('canvas'); c.width = w; c.height = h; return c; };
    const blobOf = (c, type, q) => new Promise((ok) => c.toBlob(ok, type, q));
    const bytesOf = async (blob) => new Uint8Array(await blob.arrayBuffer());
    const fill = (c, draw) => { const g = c.getContext('2d'); draw(g, c.width, c.height); return c; };
    // a picture of three vertical bands: red, green, blue
    const bands = (w, h) => fill(canvas(w, h), (g) => { ['#f00', '#0f0', '#00f'].forEach((col, i) => { g.fillStyle = col; g.fillRect(Math.floor(i * w / 3), 0, Math.ceil(w / 3), h); }); });
    const solid = (w, h, col) => fill(canvas(w, h), (g) => { g.fillStyle = col; g.fillRect(0, 0, w, h); });
    // noise from a fixed seed (mulberry32), so that the same picture is drawn in every run and every build: the cap test compares one picture with itself
    const noise = (w, h) => { let a = 0x9e3779b9; const rnd = () => { a = (a + 0x6d2b79f5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; const c = canvas(w, h), g = c.getContext('2d'), d = g.createImageData(w, h); for (let i = 0; i < d.data.length; i += 4) { d.data[i] = rnd() * 256; d.data[i + 1] = rnd() * 256; d.data[i + 2] = rnd() * 256; d.data[i + 3] = 255; } g.putImageData(d, 0, 0); return c; };
    const file = (bytes, name, type) => new win.File([bytes], name, { type });
    // an EXIF block with an orientation, a description and a place (GPS), written by hand: little-endian TIFF in an APP1 segment
    const withExif = (jpeg, orientation) => {
        const tiff = new Uint8Array(80), v = new DataView(tiff.buffer);
        tiff.set([0x49, 0x49, 0x2a, 0, 8, 0, 0, 0]);
        v.setUint16(8, 3, true);
        const entry = (at, tag, type, count, value) => { v.setUint16(at, tag, true); v.setUint16(at + 2, type, true); v.setUint32(at + 4, count, true); if (type === 3) { v.setUint16(at + 8, value, true); } else { v.setUint32(at + 8, value, true); } };
        entry(10, 0x010e, 2, 12, 68);
        entry(22, 0x0112, 3, 1, orientation);
        entry(34, 0x8825, 4, 1, 50);
        v.setUint32(46, 0, true);
        v.setUint16(50, 1, true);
        tiff.set([0x01, 0, 2, 0, 2, 0, 0, 0, 0x4e, 0, 0, 0], 52);
        v.setUint32(64, 0, true);
        tiff.set(Array.from('SECRETPLACE').map((ch) => ch.charCodeAt(0)), 68);
        const seg = new Uint8Array(4 + 6 + tiff.length);
        seg.set([0xff, 0xe1, 0, 2 + 6 + tiff.length, 0x45, 0x78, 0x69, 0x66, 0, 0]);
        seg.set(tiff, 10);
        const out = new Uint8Array(jpeg.length + seg.length);
        out.set(jpeg.subarray(0, 2), 0); out.set(seg, 2); out.set(jpeg.subarray(2), 2 + seg.length);
        return out;
    };
    // the segments before the picture data of a JPEG, as marker codes
    const markers = (b) => { const out = []; let i = 2; while (i + 3 < b.length && b[i] === 0xff && b[i + 1] !== 0xda) { out.push(b[i + 1]); i += 2 + ((b[i + 2] << 8) | b[i + 3]); } return out; };
    const text = (b) => Array.from(b).map((x) => String.fromCharCode(x)).join('');
    const unb64 = (s) => { const t = win.atob(s); const b = new Uint8Array(t.length); for (let i = 0; i < t.length; i++) { b[i] = t.charCodeAt(i); } return b; };
    // what a base64 JPEG is: its size and the colour at a place
    const look = async (b64) => {
        const bmp = await win.createImageBitmap(new win.Blob([unb64(b64)], { type: 'image/jpeg' }));
        const c = canvas(bmp.width, bmp.height), g = c.getContext('2d');
        g.drawImage(bmp, 0, 0);
        return { w: bmp.width, h: bmp.height, at: (x, y) => Array.from(g.getImageData(x, y, 1, 1).data) };
    };
    const near = (px, r, g, b, tol) => Math.abs(px[0] - r) <= tol && Math.abs(px[1] - g) <= tol && Math.abs(px[2] - b) <= tol;

    // ---- the page ----
    const inputs = () => Array.from(doc.querySelectorAll('input[type=file]'));
    const field = (i) => inputs()[i];
    const statusOf = (i) => { const f = field(i); return f ? f.closest('.cui-photo-field').querySelector('[role=status]').textContent : null; };
    const readCounts = () => { const m = /n=(\d+),(\d+),(\d+)/.exec(doc.body.innerText); return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null; };
    const payloadOf = (name) => { const el = Array.from(doc.querySelectorAll('span')).find((s) => s.textContent.startsWith(name + '=')); return el ? el.textContent.slice(name.length + 1) : null; };
    const choose = (i, f) => { const dt = new win.DataTransfer(); dt.items.add(f); const input = field(i); input.files = dt.files; input.dispatchEvent(new win.Event('change', { bubbles: true })); };
    const pickAndWait = async (i, f, name) => {
        const before = readCounts()[i];
        choose(i, f);
        const got = await until(() => (readCounts() || [])[i] > before);
        return got ? JSON.parse(payloadOf(name)) : null;
    };

    if (cfg.mode === 'cap') {
        // one field (the third), one noise picture: what the payload is under a cap given at build time
        const png = await bytesOf(await blobOf(noise(cfg.noise, cfg.noise), 'image/png'));
        const r = await pickAndWait(2, file(png, 'n.png', 'image/png'), 'cap');
        return { payload: r, violations };
    }

    // 1. the element: a native chooser, named by its label, a 44 px target, an icon over the caption, a status line
    const fields = inputs();
    check('three choosers', fields.length === 3, fields.length);
    check('each is a native file input that offers JPEG, PNG and WebP', fields.every((f) => f.getAttribute('accept') === 'image/jpeg,image/png,image/webp'));
    check('the label is the accessible name', fields.map((f) => f.getAttribute('aria-label')).join('|') === 'Square|Wide|Capped', fields.map((f) => f.getAttribute('aria-label')).join('|'));
    const keys = Array.from(doc.querySelectorAll('.cui-photo-key')).map((k) => k.getBoundingClientRect());
    check('every key is at least 44 px in both ways', keys.length === 3 && keys.every((r) => r.width >= 44 && r.height >= 44), keys.map((r) => Math.round(r.width) + 'x' + Math.round(r.height)).join(' '));
    const inputBox = fields[0].getBoundingClientRect(), keyBox = keys[0];
    check('the input covers the whole key (the target is the key)', Math.abs(inputBox.width - keyBox.width) < 1.5 && Math.abs(inputBox.height - keyBox.height) < 1.5);
    check('the icon is an svg that a screen reader skips, and the caption shows the label', doc.querySelectorAll('.cui-photo-key svg[aria-hidden=true]').length === 3 && Array.from(doc.querySelectorAll('.cui-photo-cap')).map((c) => c.textContent).join('|') === 'Square|Wide|Capped');
    check('a status line is a live region, empty at first', Array.from(doc.querySelectorAll('.cui-photo-status')).every((s) => s.getAttribute('role') === 'status' && s.textContent === ''));
    fields[0].focus();
    check('the input takes the keyboard focus', doc.activeElement === fields[0]);
    check('no <img> anywhere', doc.querySelectorAll('img').length === 0);

    // 2. a picture larger than max_side is brought down; square cuts the middle, the other keeps the proportions
    const big = await bytesOf(await blobOf(bands(3000, 2000), 'image/jpeg', 0.9));
    let r = await pickAndWait(1, file(big, 'big.jpg', 'image/jpeg'), 'wide');
    check('3000x2000 into max_side 300 (not square) is a JPEG text', r && typeof r.data === 'string' && r.data.indexOf('/9j/') === 0 && Object.keys(r).join() === 'data', JSON.stringify(r).slice(0, 80));
    let im = r && r.data ? await look(r.data) : null;
    check('3000x2000 into max_side 300 is 300x200: the proportions are kept', im && im.w === 300 && im.h === 200, im && im.w + 'x' + im.h);
    check('the whole picture is there (red at the start, blue at the end)', im && near(im.at(8, 100), 255, 0, 0, 60) && near(im.at(292, 100), 0, 0, 255, 60));
    r = await pickAndWait(0, file(big, 'big.jpg', 'image/jpeg'), 'sq');
    im = r && r.data ? await look(r.data) : null;
    check('3000x2000 into a square of max_side 200 is 200x200', im && im.w === 200 && im.h === 200, im && im.w + 'x' + im.h);
    // the middle 2000 of 3000 is a quarter red, half green, a quarter blue (the bands are thirds of the whole)
    check('the square is the middle of the picture: a quarter red, then green to three quarters, then blue', im && near(im.at(40, 100), 255, 0, 0, 70) && near(im.at(60, 100), 0, 255, 0, 70) && near(im.at(140, 100), 0, 255, 0, 70) && near(im.at(160, 100), 0, 0, 255, 70), im && JSON.stringify([40, 60, 140, 160].map((x) => im.at(x, 100))));
    // a small picture is not made larger
    const small = await bytesOf(await blobOf(bands(90, 60), 'image/png'));
    r = await pickAndWait(1, file(small, 'small.png', 'image/png'), 'wide');
    im = r && r.data ? await look(r.data) : null;
    check('a PNG of 90x60 stays 90x60 (never upscaled)', im && im.w === 90 && im.h === 60, im && im.w + 'x' + im.h);

    // 3. the EXIF orientation is applied, and the EXIF block (a description, a place) is gone from what the program gets
    const left = fill(canvas(200, 100), (g) => { g.fillStyle = '#f00'; g.fillRect(0, 0, 100, 100); g.fillStyle = '#00f'; g.fillRect(100, 0, 100, 100); });
    const plain = await bytesOf(await blobOf(left, 'image/jpeg', 0.95));
    const tagged = withExif(plain, 6);
    check('the test picture does carry the EXIF block and the place', text(tagged).indexOf('SECRETPLACE') > 0 && markers(tagged).indexOf(0xe1) >= 0);
    r = await pickAndWait(1, file(tagged, 'phone.jpg', 'image/jpeg'), 'wide');
    im = r && r.data ? await look(r.data) : null;
    check('orientation 6 (rotate a quarter turn) makes 200x100 into 100x200', im && im.w === 100 && im.h === 200, im && im.w + 'x' + im.h);
    check('and turns it the right way (the red half at the top, the blue at the foot)', im && near(im.at(50, 20), 255, 0, 0, 70) && near(im.at(50, 180), 0, 0, 255, 70));
    const out = r && r.data ? unb64(r.data) : new Uint8Array(0);
    check('what the program gets has no EXIF segment, no comment, no place', out.length > 0 && markers(out).indexOf(0xe1) < 0 && markers(out).indexOf(0xfe) < 0 && text(out).indexOf('Exif') < 0 && text(out).indexOf('SECRETPLACE') < 0, markers(out).join(','));

    // 4. a PNG with a transparent part is white there (a JPEG has no alpha), and a PNG with no type is still a picture
    const holes = fill(canvas(80, 80), (g) => { g.fillStyle = '#0a0'; g.fillRect(0, 0, 40, 80); });
    const pngBytes = await bytesOf(await blobOf(holes, 'image/png'));
    r = await pickAndWait(1, file(pngBytes, 'cut.png', 'image/png'), 'wide');
    im = r && r.data ? await look(r.data) : null;
    check('the transparent half of a PNG is white', im && near(im.at(70, 40), 255, 255, 255, 12), im && JSON.stringify(im.at(70, 40)));
    r = await pickAndWait(1, file(pngBytes, 'notype', ''), 'wide');
    check('a file whose type the browser does not know is judged by its bytes: a PNG is accepted', r && typeof r.data === 'string');

    // 5. what is not a picture is told so, and nothing else is: by its bytes, and by its type
    const refused = async (label, f, error) => {
        const got = await pickAndWait(1, f, 'wide');
        check(label, got && Object.keys(got).join() === 'error' && got.error === error, JSON.stringify(got));
    };
    await refused('text posing as a JPEG is "not a picture"', file(new TextEncoder().encode('this is not a picture at all'), 'x.jpg', 'image/jpeg'), 'not a picture');
    check('and the status line says so', statusOf(1) === 'Not a picture', statusOf(1));
    await refused('the bytes of a PNG under the type text/plain are "not a picture"', file(pngBytes, 'x.txt', 'text/plain'), 'not a picture');
    await refused('a GIF is "not a picture" (the page takes JPEG, PNG and WebP)', file(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 1, 0, 1, 0, 0, 0, 0, 0x3b]), 'a.gif', 'image/gif'), 'not a picture');
    await refused('an SVG is "not a picture"', file(new TextEncoder().encode('<svg xmlns="http://www.w3.org/2000/svg" width="9" height="9"/>'), 'a.svg', 'image/svg+xml'), 'not a picture');
    await refused('a file over 40 MiB is "too big", and is not read', file(new Uint8Array(41943041), 'huge.jpg', 'image/jpeg'), 'too big');
    check('the status line says that too', statusOf(1) === 'Picture too big', statusOf(1));

    // 6. what is told to the program is the page's own: a Photo draws it
    const green = await bytesOf(await blobOf(solid(300, 200, '#0a0'), 'image/jpeg', 0.9));
    r = await pickAndWait(1, file(green, 'g.jpg', 'image/jpeg'), 'wide');
    check('the status line says the picture was added', statusOf(1) === 'Picture added', statusOf(1));
    const shown = await until(() => doc.querySelector('[aria-label="Wide picture"] canvas'));
    const box = doc.querySelector('[aria-label="Wide picture"]');
    check('a Photo is a box with role img and its label, and a canvas once the text decodes', shown && box.getAttribute('role') === 'img' && box.querySelector('canvas').getAttribute('aria-hidden') === 'true');
    const cv = box && box.querySelector('canvas');
    check('the canvas has the ratio 2:1 (720x360)', cv && cv.width === 720 && cv.height === 360, cv && cv.width + 'x' + cv.height);
    const centre = cv ? Array.from(cv.getContext('2d').getImageData(360, 180, 1, 1).data) : [0, 0, 0];
    check('and the picture is drawn on it (green at the centre)', near(centre, 0, 170, 0, 40), JSON.stringify(centre));
    check('the box keeps its ratio in the layout', box && Math.abs(box.getBoundingClientRect().width / box.getBoundingClientRect().height - 2) < 0.02, box && box.getBoundingClientRect().width + 'x' + box.getBoundingClientRect().height);
    const deco = doc.querySelector('.cui-photo[aria-hidden=true]');
    check('a decorative Photo is hidden from a screen reader, has no role and no name', deco && !deco.hasAttribute('role') && !deco.hasAttribute('aria-label') && deco.querySelector('canvas'));
    const bad = doc.querySelector('[aria-label="Not a picture"]');
    check('text that is not a picture leaves the tone gradient (no canvas)', bad && !bad.querySelector('canvas') && bad.getAttribute('data-tone') === '3' && /gradient/.test(win.getComputedStyle(bad).backgroundImage), bad && win.getComputedStyle(bad).backgroundImage);
    check('a tone the program did not give is no tone', !box.hasAttribute('data-tone'));
    check('a Photo has no <img>, no data: address, no request but the page\'s own', doc.querySelectorAll('img').length === 0 && win.performance.getEntriesByType('resource').every((e) => e.name.indexOf('data:') !== 0 && e.name.indexOf('blob:') !== 0));

    // 7. the status line survives a redraw, and the redraw tells the program nothing
    const counts = readCounts();
    Array.from(doc.querySelectorAll('button')).find((b) => b.textContent === 'Bump').click();
    await until(() => /bump=1/.test(doc.body.innerText));
    check('the status line is still "Picture added" after the page was drawn again', statusOf(1) === 'Picture added', statusOf(1));
    check('and the redraw told no handler anything', JSON.stringify(readCounts()) === JSON.stringify(counts), JSON.stringify([readCounts(), counts]));
    check('the redrawn Photo is drawn again from the same text', await until(() => doc.querySelector('[aria-label="Wide picture"] canvas')));

    // 8. a choice that a newer one has replaced is told to nobody
    const heavy = await bytesOf(await blobOf(noise(2400, 2400), 'image/png'));
    const light = await bytesOf(await blobOf(solid(40, 30, '#00f'), 'image/png'));
    const n0 = readCounts()[1];
    choose(1, file(heavy, 'heavy.png', 'image/png'));
    choose(1, file(light, 'light.png', 'image/png'));
    await until(() => readCounts()[1] > n0);
    await sleep(1500);
    im = await look(JSON.parse(payloadOf('wide')).data);
    check('only the newer choice is told (once), and it is the one that is last', readCounts()[1] === n0 + 1 && im.w === 40 && im.h === 30, JSON.stringify(readCounts()) + ' ' + im.w + 'x' + im.h);

    // 9. the policy of the page: nothing the tests did was refused, and a request to another origin is
    check('no policy violation while the elements worked', violations.length === 0, violations.join('; '));
    const probe = new win.Image(); probe.src = 'https://elsewhere.invalid/p.png';
    let fetched = 'no error';
    try { await win.fetch('https://elsewhere.invalid/x'); } catch (e) { fetched = 'error'; }
    await sleep(300);
    check('the page\'s policy refuses an image of another origin', violations.some((v) => v.indexOf('img-src') === 0), violations.join('; '));
    check('and a fetch to another origin', violations.some((v) => v.indexOf('connect-src') === 0) && fetched === 'error', violations.join('; '));
    return { results };
}
module.exports = pageTests;
