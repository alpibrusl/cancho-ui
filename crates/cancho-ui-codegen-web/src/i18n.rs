//! The page side of docs/i18n.md: the data of the languages a module uses, the formatting and plural functions that run it, `t(..)`, and
//! the language in use. The steps are those of `cancho_ui_ir::i18n` (the data and the algorithms replay runs) written in JavaScript;
//! `tests/i18n_corpus.json` and `scripts/check-i18n.js` hold both to what Chromium's `Intl` answered.

use cancho_ui_ir::i18n::{LANGS, Part, categories, lang as lang_data};
use cancho_ui_ir::ir::{IrCatalog, IrComponent, MsgPart};
use serde_json::{Value, json};

fn part_json(p: &Part) -> Value {
    match p {
        Part::Lit(s) => json!(["l", s]),
        Part::Day(pad) => json!(["d", u8::from(*pad)]),
        Part::MonthNum(pad) => json!(["m", u8::from(*pad)]),
        Part::Year(short) => json!(["y", u8::from(*short)]),
        Part::MonthName(names) => json!(["M", names]),
        Part::Weekday(names) => json!(["w", names]),
        Part::Hour(pad, twelve) => json!(["h", u8::from(*pad), u8::from(*twelve)]),
        Part::Minute => json!(["n"]),
        Part::DayPeriod(am, pm) => json!(["p", am, pm]),
    }
}

fn lang_json(code: &str) -> Value {
    let Some(l) = lang_data(code) else {
        return Value::Null;
    };
    let pat = |ps: &[Part]| Value::Array(ps.iter().map(part_json).collect());
    json!({
        "r": u8::from(l.rtl), "dec": l.decimal, "grp": l.group, "neg": l.minus, "mg": l.min_group, "in": u8::from(l.indian),
        "pp": l.pct_prefix, "ps": l.pct_suffix,
        "short": pat(l.date_short), "medium": pat(l.date_medium), "long": pat(l.date_long), "full": pat(l.date_full), "time": pat(l.time),
    })
}

fn parts_json(parts: &[MsgPart]) -> Value {
    Value::Array(
        parts
            .iter()
            .map(|p| match p {
                MsgPart::Lit(s) => json!(s),
                MsgPart::Hole(h) => json!([h]),
            })
            .collect(),
    )
}

/// JSON with every character outside ASCII written as `\uXXXX` (one per UTF-16 unit): a built module is ASCII whatever its texts are, so a
/// server that sends it with the wrong charset cannot garble a catalog (`tests/build_output.rs`).
fn ascii(json: String) -> String {
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// The languages a component speaks: its catalogs', in order, or English.
pub fn languages(c: &IrComponent) -> Vec<String> {
    if c.messages.is_empty() {
        vec!["en".to_string()]
    } else {
        c.messages.iter().map(|m| m.lang.clone()).collect()
    }
}

fn catalog_json(cat: &IrCatalog) -> Value {
    let mut m = serde_json::Map::new();
    for e in &cat.entries {
        let forms: Vec<Value> = e
            .forms
            .iter()
            .map(|(c, parts)| json!([c, parts_json(parts)]))
            .collect();
        m.insert(e.key.clone(), json!({"p": e.plural, "f": forms}));
    }
    Value::Object(m)
}

/// `{langs, m: {lang: {key: {p, f}}}, o: {key: [holes]}}`: what `t(..)` needs of one component.
pub fn component_json(c: &IrComponent) -> String {
    let mut m = serde_json::Map::new();
    for cat in &c.messages {
        m.insert(cat.lang.clone(), catalog_json(cat));
    }
    let mut o = serde_json::Map::new();
    if let Some(first) = c.messages.first() {
        for e in &first.entries {
            o.insert(e.key.clone(), json!(cancho_ui_ir::messages::hole_order(e)));
        }
    }
    ascii(json!({"langs": languages(c), "m": m, "o": o}).to_string())
}

/// The module-level adapter, with the data of the given languages.
pub fn adapter_js(langs: &[String]) -> String {
    let mut data = serde_json::Map::new();
    for l in langs {
        data.insert(l.clone(), lang_json(l));
    }
    let cats: serde_json::Map<String, Value> = LANGS
        .iter()
        .filter(|l| langs.iter().any(|x| x == l.code))
        .filter_map(|l| categories(l.code).map(|c| (l.code.to_string(), json!(c))))
        .collect();
    TEMPLATE
        .replace("/*DATA*/", &ascii(Value::Object(data).to_string()))
        .replace("/*CATS*/", &Value::Object(cats).to_string())
}

const TEMPLATE: &str = r#"// Platform adapter: Locale (docs/i18n.md). The formats of the languages this page uses are data (below), not Intl: two engines disagree with
// each other about a date, and replay has to say the same text. The steps are those of cancho_ui_ir::i18n, in JavaScript.
const __CUI_I18N = /*DATA*/;
const __CUI_CATS = /*CATS*/;
function __cui_i18n_digits(a) {
    if (a < 1e-6) { return ['0', '']; }
    const s = String(a);
    const i = s.indexOf('.');
    return i < 0 ? [s, ''] : [s.slice(0, i), s.slice(i + 1)];
}
function __cui_i18n_round(int, frac, d) {
    const all = (int + frac).split('').map(Number);
    const keep = int.length + d;
    const up = keep < all.length && all[keep] >= 5;
    if (all.length > keep) { all.length = keep; }
    while (all.length < keep) { all.push(0); }
    if (up) {
        let i = all.length;
        for (;;) {
            if (i === 0) { all.unshift(1); break; }
            i--;
            if (all[i] === 9) { all[i] = 0; } else { all[i]++; break; }
        }
    }
    const il = all.length - d;
    const t = all.join('');
    return [t.slice(0, il), t.slice(il)];
}
function __cui_i18n_group(L, int) {
    const n = int.length;
    if (n < 3 + L.mg) { return int; }
    let first = n - 3;
    let out = int.slice(first);
    while (first > 0) {
        const from = Math.max(0, first - (L.in ? 2 : 3));
        out = int.slice(from, first) + L.grp + out;
        first = from;
    }
    return out;
}
function __cui_i18n_write(L, negative, int, frac, d) {
    const r = __cui_i18n_round(int, frac, d);
    return (negative ? L.neg : '') + __cui_i18n_group(L, r[0]) + (d > 0 ? L.dec + r[1] : '');
}
function __cui_fmt_number(loc, x, d) {
    if (typeof x !== 'number' || typeof d !== 'number' || !isFinite(x) || Math.abs(x) >= 1e21 || d % 1 !== 0 || d < 0 || d > 6) { return ''; }
    const g = __cui_i18n_digits(Math.abs(x));
    return __cui_i18n_write(__CUI_I18N[loc.lang], x < 0 || Object.is(x, -0), g[0], g[1], d);
}
function __cui_fmt_percent(loc, x) {
    if (typeof x !== 'number' || !isFinite(x) || Math.abs(x) >= 1e21) { return ''; }
    const L = __CUI_I18N[loc.lang];
    const g = __cui_i18n_digits(Math.abs(x));
    let all = g[0] + g[1];
    const point = g[0].length + 2;
    while (all.length < point) { all += '0'; }
    const body = __cui_i18n_write(L, false, all.slice(0, point).replace(/^0+/, '') || '0', all.slice(point), 0);
    return (x < 0 || Object.is(x, -0) ? L.neg : '') + L.pp + body + L.ps;
}
function __cui_i18n_civil(ms) {
    if (typeof ms !== 'number' || !isFinite(ms) || ms < -30610224000000 || ms > 253402300799999) { return null; }
    const d = new Date(Math.trunc(ms));
    return { y: d.getUTCFullYear(), mo: d.getUTCMonth(), d: d.getUTCDate(), w: d.getUTCDay(), h: d.getUTCHours(), mi: d.getUTCMinutes() };
}
function __cui_i18n_two(n, pad) { return pad && n < 10 ? '0' + n : String(n); }
function __cui_i18n_apply(parts, c) {
    let out = '';
    for (const p of parts) {
        switch (p[0]) {
            case 'l': out += p[1]; break;
            case 'd': out += __cui_i18n_two(c.d, p[1]); break;
            case 'm': out += __cui_i18n_two(c.mo + 1, p[1]); break;
            case 'y': out += p[1] ? __cui_i18n_two(c.y % 100, 1) : String(c.y); break;
            case 'M': out += p[1][c.mo]; break;
            case 'w': out += p[1][c.w]; break;
            case 'h': out += __cui_i18n_two(p[2] ? (c.h % 12 === 0 ? 12 : c.h % 12) : c.h, p[1]); break;
            case 'n': out += __cui_i18n_two(c.mi, 1); break;
            case 'p': out += c.h < 12 ? p[1] : p[2]; break;
        }
    }
    return out;
}
function __cui_fmt_date(loc, ms, style) {
    const c = __cui_i18n_civil(ms);
    const ok = style === 'short' || style === 'medium' || style === 'long' || style === 'full';
    return c === null || !ok ? '' : __cui_i18n_apply(__CUI_I18N[loc.lang][style], c);
}
function __cui_fmt_time(loc, ms) {
    const c = __cui_i18n_civil(ms);
    return c === null ? '' : __cui_i18n_apply(__CUI_I18N[loc.lang].time, c);
}
function __cui_plural(lang, count) {
    let n = Math.abs(count);
    if (typeof count !== 'number' || isNaN(n) || n % 1 !== 0 || n >= 9007199254740992) { return 'other'; }
    const m10 = n % 10;
    const m100 = n % 100;
    switch (lang) {
        case 'en': case 'de': return n === 1 ? 'one' : 'other';
        case 'hi': return n <= 1 ? 'one' : 'other';
        case 'fr': case 'es': case 'pt':
            if (n === 1 || (n === 0 && lang !== 'es')) { return 'one'; }
            return n !== 0 && n % 1000000 === 0 ? 'many' : 'other';
        case 'ru':
            if (m10 === 1 && m100 !== 11) { return 'one'; }
            return m10 >= 2 && m10 <= 4 && !(m100 >= 12 && m100 <= 14) ? 'few' : 'many';
        case 'ar':
            if (n === 0) { return 'zero'; }
            if (n === 1) { return 'one'; }
            if (n === 2) { return 'two'; }
            if (m100 >= 3 && m100 <= 10) { return 'few'; }
            return m100 >= 11 && m100 <= 99 ? 'many' : 'other';
        case 'he': return n === 1 ? 'one' : n === 2 ? 'two' : 'other';
        default: return 'other';
    }
}
// the language in use: the first of the browser's languages that the component has, else its first
function __cui_locale_init(langs) {
    const prefs = (typeof navigator !== 'undefined' && (navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language])) || [];
    for (const p of prefs) {
        const primary = String(p).toLowerCase().split(/[-_]/)[0];
        if (langs.indexOf(primary) >= 0) { return primary; }
    }
    return langs[0];
}
function __cui_doc(lang) {
    if (typeof document === 'undefined') { return; }
    document.documentElement.lang = lang;
    document.documentElement.dir = __CUI_I18N[lang].r ? 'rtl' : 'ltr';
}
function __cui_locale_current(loc) { return loc.lang; }
function __cui_locale_set(loc, lang) {
    if (typeof lang !== 'string' || loc.langs.indexOf(lang) < 0) { return false; }
    loc.lang = lang;
    __cui_doc(lang);
    return true;
}
function __cui_t(loc, msgs, key) {
    const e = msgs.m[loc.lang] && msgs.m[loc.lang][key];
    if (!e) { return ''; }
    let form;
    let holes;
    if (e.p !== null) {
        const cat = __cui_plural(loc.lang, arguments[3]);
        form = e.f.find(function (f) { return f[0] === cat; });
        holes = [e.p];
    } else {
        form = e.f[0];
        holes = msgs.o[key];
    }
    if (!form) { return ''; }
    let out = '';
    for (const part of form[1]) {
        if (typeof part === 'string') { out += part; }
        else if (e.p !== null) { out += String(arguments[3]); }
        else { out += '\u2068' + String(arguments[3 + holes.indexOf(part[0])]) + '\u2069'; }
    }
    return out;
}
"#;
