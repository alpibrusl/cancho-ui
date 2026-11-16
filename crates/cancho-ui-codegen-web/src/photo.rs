//! `PhotoField` and `Photo` on the page (docs/structure.md 12.0r). The element is a few DOM calls; what it does is in two scripts, each written only for a program that
//! has the element: `PHOTO_JS` (the check of a picture text and the drawing of a `Photo`, which reuses the decoder and the cache of `face(..)`) and `PHOTO_FIELD_JS` (the
//! chooser: decode, cut, shrink until the text fits). Every string from the program goes in with `textContent` or `setAttribute`; the picture is drawn on a canvas, so the
//! page has no `<img>`, no `data:` address and no request for it, and its policy needs nothing for it.

use cancho_ui_ir::ir::*;

use crate::emit::js_string;
use crate::expr::emit_expr_runtime;

/// A `Photo`: a `div` (a box of the ratio, role `img` and its label, or hidden from a screen reader) that `__cui_photo_draw` fills with a canvas once the text decodes.
pub(crate) fn emit_photo(
    indent: &str,
    var: &str,
    parent: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let (w, h) = cancho_ui_ir::photo::ratio_of(bindings);
    let expr_of = |key: &str| {
        bindings
            .iter()
            .find(|b| b.key == key)
            .and_then(|b| match &b.value {
                IrBindingValue::Const { expr, .. } => Some(emit_expr_runtime(expr, params)),
                _ => None,
            })
    };
    let data = expr_of("data").unwrap_or_else(|| "\"\"".to_string());
    let tone = expr_of("tone").unwrap_or_else(|| "\"\"".to_string());
    let decorative = bindings.iter().any(|b| {
        b.key == "decorative"
            && matches!(&b.value, IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { value: true, .. }))
    });
    let mut out = format!(
        "{indent}const {var} = document.createElement('div');\n{indent}{var}.className = 'cui-photo';\n{indent}{var}.style.aspectRatio = '{w} / {h}';\n"
    );
    if decorative {
        out.push_str(&format!(
            "{indent}{var}.setAttribute('aria-hidden', 'true');\n"
        ));
    } else {
        let label = match (&accessibility.label, &accessibility.label_expr) {
            (Some(l), _) => js_string(l),
            (None, Some(e)) => format!("String({})", emit_expr_runtime(e, params)),
            (None, None) => "\"\"".to_string(),
        };
        out.push_str(&format!(
            "{indent}{var}.setAttribute('role', 'img');\n{indent}{var}.setAttribute('aria-label', {label});\n"
        ));
    }
    out.push_str(&format!(
        "{indent}__cui_photo_draw({var}, {data}, {tone}, {w}, {h});\n{indent}{parent}.appendChild({var});\n"
    ));
    out
}

/// A `PhotoField`: a `div` of a key (a label that holds the file input, which covers it and is the target, the icon and the label under it) and the status line.
pub(crate) fn emit_field(
    indent: &str,
    var: &str,
    parent: &str,
    key: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let (side, chars, square) = cancho_ui_ir::photo::limits_of(bindings);
    let icon = cancho_ui_ir::photo::icon_of(bindings);
    let event = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("on", IrBindingValue::EventRef(e)) => Some(e.clone()),
        _ => None,
    });
    let label = match (&accessibility.label, &accessibility.label_expr) {
        (Some(l), _) => js_string(l),
        (None, Some(e)) => format!("String({})", emit_expr_runtime(e, params)),
        (None, None) => "\"\"".to_string(),
    };
    let deliver = match &event {
        Some(e) => format!("function (v) {{ __on_{e}(v); }}"),
        None => "function () {}".to_string(),
    };
    format!(
        "{indent}const {var} = document.createElement('div');\n\
{indent}{var}.className = 'cui-photo-field';\n\
{indent}const {var}_key = document.createElement('label');\n\
{indent}{var}_key.className = 'cui-photo-key';\n\
{indent}const {var}_in = document.createElement('input');\n\
{indent}{var}_in.setAttribute('type', 'file');\n\
{indent}{var}_in.setAttribute('accept', 'image/jpeg,image/png,image/webp');\n\
{indent}{var}_in.setAttribute('aria-label', {label});\n\
{indent}{var}_in.className = 'cui-photo-in';\n\
{indent}{var}_key.appendChild({var}_in);\n\
{indent}__cui_icon({var}_key, {icon}, \"cui-icon cui-icon-m\", \"20\", null);\n\
{indent}const {var}_cap = document.createElement('span');\n\
{indent}{var}_cap.className = 'cui-photo-cap';\n\
{indent}{var}_cap.textContent = {label};\n\
{indent}{var}_key.appendChild({var}_cap);\n\
{indent}{var}.appendChild({var}_key);\n\
{indent}const {var}_st = document.createElement('span');\n\
{indent}{var}_st.className = 'cui-photo-status';\n\
{indent}{var}_st.setAttribute('role', 'status');\n\
{indent}{var}.appendChild({var}_st);\n\
{indent}__cui_photo_wire(root, \"{key}\", {var}_in, {var}_st, {side}, {square}, {chars}, {deliver});\n\
{indent}{parent}.appendChild({var});\n",
        icon = js_string(icon),
    )
}

/// The check of a picture text, shared by the drawing and the chooser. It is `cancho_ui_ir::photo::picture_ok` in JavaScript (and, for a `face(..)`, the check
/// `__cui_face_draw` makes inline): `tests/photo_corpus.json` is run against all of them.
pub(crate) const PHOTO_OK_JS: &str = r##"// Builtin: the check of a picture text (docs/structure.md 12.0r): base64 of a JPEG, 100 to 48,000 characters.
function __cui_photo_ok(t) { return typeof t === "string" && t.length >= 100 && t.length <= 48000 && t.indexOf("/9j/") === 0 && /^[A-Za-z0-9+\/]+={0,2}$/.test(t); }
"##;

/// The drawing of a `Photo`: the tone (a `data-tone` the stylesheet paints as a gradient, which is all there is until the picture is drawn, or when the text is not one), then the
/// picture on a canvas of the ratio, cut to cover it. The decoder and its cache are `face(..)`'s (`__cui_face_bmp`).
pub(crate) const PHOTO_JS: &str = r##"// Builtin: Photo (docs/structure.md 12.0r). A picture from state on a canvas: no image element, no address, no request.
function __cui_photo_draw(el, data, tone, w, h) {
    var p, long = 720, cw = w >= h ? long : Math.max(1, Math.round(long * w / h)), ch = w >= h ? Math.max(1, Math.round(long * h / w)) : long;
    tone = String(tone);
    if (/^(0|[1-9]|1[01])$/.test(tone)) { el.setAttribute("data-tone", tone); }
    if (!__cui_photo_ok(data)) { return; }
    p = __cui_face_bmp(data);
    if (!p || typeof p.then !== "function") { return; }
    p.then(function (bmp) {
        var cv = document.createElement("canvas"), g, s;
        cv.width = cw; cv.height = ch; cv.className = "cui-photo-pic"; cv.setAttribute("aria-hidden", "true");
        g = cv.getContext("2d");
        s = Math.max(cw / bmp.width, ch / bmp.height);
        g.drawImage(bmp, (cw - bmp.width * s) / 2, (ch - bmp.height * s) / 2, bmp.width * s, bmp.height * s);
        el.setAttribute("data-photo", "1"); el.appendChild(cv);
    }, function () {});
}
"##;

/// The chooser of a `PhotoField`. The page is drawn again from nothing at every render, which makes a new input and a new status line: the words the status line says are kept on the
/// component's root by the place of the field in the source and put back, and what a stale choice finishes is told to nobody (a newer choice has the sequence number).
pub(crate) const PHOTO_FIELD_JS: &str = r##"// Platform adapter: PhotoField (docs/structure.md 12.0r). A native chooser; the picture is decoded (EXIF orientation applied), cut, and drawn again as a JPEG
// that is made smaller until its base64 text fits `chars`. Drawing again leaves the EXIF block (where and when the camera took it) behind.
var __cui_photo_words = { work: "Preparing the picture", ok: "Picture added", big: "Picture too big", bad: "Not a picture" };
var __cui_photo_types = ["image/jpeg", "image/png", "image/webp"];
function __cui_photo_decode(file) {
    return createImageBitmap(file, { imageOrientation: "from-image" }).catch(function (e) {
        if (e && e.name === "TypeError") { return createImageBitmap(file); }
        throw e;
    });
}
function __cui_photo_canvas(bmp, side, square) {
    var sx = 0, sy = 0, sw = bmp.width, sh = bmp.height, m, k, cv, g;
    if (square) { m = Math.min(sw, sh); sx = Math.floor((sw - m) / 2); sy = Math.floor((sh - m) / 2); sw = m; sh = m; }
    k = Math.min(1, side / Math.max(sw, sh));
    cv = document.createElement("canvas");
    cv.width = Math.max(1, Math.round(sw * k)); cv.height = Math.max(1, Math.round(sh * k));
    g = cv.getContext("2d");
    g.fillStyle = "#fff"; g.fillRect(0, 0, cv.width, cv.height);
    g.drawImage(bmp, sx, sy, sw, sh, 0, 0, cv.width, cv.height);
    return cv;
}
function __cui_photo_shrink(bmp, side, square, chars) {
    var q = 0.82, s = side, floor = Math.max(32, Math.round(side / 4));
    function once() {
        return new Promise(function (ok) { __cui_photo_canvas(bmp, s, square).toBlob(ok, "image/jpeg", q); }).then(function (blob) {
            if (blob && 4 * Math.ceil(blob.size / 3) <= chars) { return blob; }
            if (q > 0.45) { q -= 0.08; return once(); }
            if (s > floor) { s = Math.max(floor, Math.round(s * 0.8)); q = 0.72; return once(); }
            return null;
        });
    }
    return once().then(function (blob) {
        if (!blob) { return null; }
        return new Promise(function (ok, no) {
            var r = new FileReader();
            r.onload = function () { ok(String(r.result).split(",")[1] || ""); };
            r.onerror = function () { no(r.error); };
            r.readAsDataURL(blob);
        });
    });
}
function __cui_photo_wire(root, key, input, status, side, square, chars, deliver) {
    var st = (root.__cui_photo_st = root.__cui_photo_st || {}), el = (root.__cui_photo_el = root.__cui_photo_el || {}), seq = (root.__cui_photo_seq = root.__cui_photo_seq || {});
    el[key] = status; status.textContent = st[key] || "";
    function say(word) { st[key] = word; if (el[key]) { el[key].textContent = word; } }
    input.addEventListener("change", function () {
        var f = input.files && input.files[0], n, fail;
        if (!f) { return; }
        input.value = "";
        n = seq[key] = (seq[key] || 0) + 1;
        fail = function (error, word) { if (seq[key] !== n) { return; } say(word); deliver(JSON.stringify({ error: error })); };
        if (f.type !== "" && __cui_photo_types.indexOf(f.type.toLowerCase()) < 0) { fail("not a picture", __cui_photo_words.bad); return; }
        if (f.size > 41943040) { fail("too big", __cui_photo_words.big); return; }
        say(__cui_photo_words.work);
        __cui_photo_decode(f).then(function (bmp) {
            return __cui_photo_shrink(bmp, side, square, chars).then(function (b64) {
                if (bmp.close) { bmp.close(); }
                if (seq[key] !== n) { return; }
                if (b64 === null || !__cui_photo_ok(b64) || b64.length > chars) { fail("too big", __cui_photo_words.big); return; }
                say(__cui_photo_words.ok); deliver(JSON.stringify({ data: b64 }));
            });
        }, function () { fail("not a picture", __cui_photo_words.bad); }).catch(function () { fail("not a picture", __cui_photo_words.bad); });
    });
}
"##;
