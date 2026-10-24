//! The page side of a number field (docs/number-input.md): a `TextField { type: "number" }` is a text field with a decimal keyboard whose event
//! is given the number typed, in the language of the component, as the text `num` reads, or the typed text when it is not a number. This
//! is `cancho_ui_replay::parse_number` in JavaScript; `tests/number_corpus.json` and `scripts/check-number.js` hold both to the same cases.
//! The module is ASCII: the characters it names are escapes.

use cancho_ui_ir::ir::{IrBindingValue, IrElementKind, IrViewNode};

/// Does a view hold a number field?
pub fn view_has_number(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            (matches!(kind, IrElementKind::TextField)
                && bindings.iter().any(|b| {
                    b.key == "type"
                        && matches!(&b.value, IrBindingValue::String(s) if s == "number")
                }))
                || children.iter().any(view_has_number)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(view_has_number)
        }
        IrViewNode::Text { .. } => false,
    }
}

pub const ADAPTER_JS: &str = r#"// Platform adapter: number fields (docs/number-input.md).
function __cui_number_lang(loc) {
    return typeof __CUI_I18N !== 'undefined' && loc && __CUI_I18N[loc.lang] ? __CUI_I18N[loc.lang] : null;
}
// The number `text` is in the language L (null: English) as the text `num` reads, or null: an optional sign (the language's minus, '-',
// U+2212 or '+'), digits, at most one decimal separator (the language's, and '.' when '.' is not its grouping mark), an exponent, no grouping
// mark; spaces around it are ignored; digits of the Arabic-Indic, extended Arabic-Indic and full-width scripts count; zero, or a magnitude
// from 1e-6 up to 1e21, is a number and nothing else is.
function __cui_number_parse(L, text) {
    const lang = L || { dec: '.', grp: ',', neg: '-' };
    const marks = /[\u200e\u200f\u061c]/g;
    let t = String(text).replace(marks, '').replace(/^[ \t\n\r\u00a0\u202f]+|[ \t\n\r\u00a0\u202f]+$/g, '');
    t = t.replace(/[\u0660-\u0669\u06f0-\u06f9\uff10-\uff19]/g, function (c) {
        const n = c.charCodeAt(0);
        return String(n >= 0xff10 ? n - 0xff10 : n >= 0x06f0 ? n - 0x06f0 : n - 0x0660);
    });
    const neg = String(lang.neg).replace(marks, '');
    let sign = '';
    if (neg !== '' && t.indexOf(neg) === 0) { sign = '-'; t = t.slice(neg.length); }
    else if (t[0] === '-' || t[0] === '\u2212') { sign = '-'; t = t.slice(1); }
    else if (t[0] === '+') { t = t.slice(1); }
    const isDigit = function (c) { return c !== undefined && c >= '0' && c <= '9'; };
    const dec = String(lang.dec);
    let i = 0;
    let int = '';
    while (isDigit(t[i])) { int += t[i]; i++; }
    let point = false;
    if (dec !== '' && t.startsWith(dec, i)) { point = true; i += dec.length; }
    else if (dec !== '.' && lang.grp !== '.' && t[i] === '.') { point = true; i += 1; }
    let frac = '';
    if (point) { while (isDigit(t[i])) { frac += t[i]; i++; } }
    if (int === '' && frac === '') { return null; }
    let exp = '';
    if (t[i] === 'e' || t[i] === 'E') {
        i++;
        if (t[i] === '+' || t[i] === '-') { exp += t[i]; i++; }
        const from = i;
        while (isDigit(t[i])) { exp += t[i]; i++; }
        if (i === from) { return null; }
    }
    if (i !== t.length) { return null; }
    const n = Number(sign + (int === '' ? '0' : int) + (frac === '' ? '' : '.' + frac) + (exp === '' ? '' : 'e' + exp));
    if (!isFinite(n) || (n !== 0 && !(Math.abs(n) >= 1e-6 && Math.abs(n) < 1e21))) { return null; }
    return String(n);
}
// What the field's event is given for `typed`: the number, or the text itself.
function __cui_number_text(L, typed) {
    const n = __cui_number_parse(L, typed);
    return n === null ? typed : n;
}
// What the field shows for the state `v`: the number in the language's separators, or `v` itself when it is not the text of a number.
function __cui_number_show(L, v) {
    if (typeof v !== 'string' || !/^-?[0-9]+(\.[0-9]+)?(e[+-]?[0-9]+)?$/.test(v)) { return v; }
    const lang = L || { dec: '.', neg: '-' };
    return v.replace(/^-/, function () { return lang.neg; }).replace('.', function () { return lang.dec; });
}
"#;
