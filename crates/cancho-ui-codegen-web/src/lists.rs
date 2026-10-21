//! The list and string builtins that are pure functions (docs/lists.md): `sort(list, x => key)`, `reverse(list)`,
//! `contains(text, needle)` and `lower(text)`. No capability tells the emitter they are used, so it looks at what it
//! emitted. Each helper has a model in `cancho_ui_replay::eval`, and `tests/lists_corpus.json` is checked against both.

const SORT_JS: &str = "\
// Builtin: sort(list, x => key) (docs/lists.md). A new list, stable, ascending by key: numbers (NaN last) before strings,
// numbers numerically, strings by UTF-16 code units; any other key (a bool, a list) ranks after them and compares equal.
function __cui_sort(list, key) {
    var rank = function (k) { return typeof k === \"number\" ? 0 : typeof k === \"string\" ? 1 : 2; };
    var rows = list.map(function (v, i) { return { v: v, k: key(v), i: i }; });
    rows.sort(function (a, b) {
        var ra = rank(a.k), rb = rank(b.k), c = 0;
        if (ra !== rb) { c = ra - rb; }
        else if (ra === 0) {
            var na = a.k !== a.k, nb = b.k !== b.k;
            c = na || nb ? (na && nb ? 0 : na ? 1 : -1) : a.k < b.k ? -1 : a.k > b.k ? 1 : 0;
        } else if (ra === 1) { c = a.k < b.k ? -1 : a.k > b.k ? 1 : 0; }
        return c !== 0 ? c : a.i - b.i;
    });
    return rows.map(function (r) { return r.v; });
}
";

const REVERSE_JS: &str = "\
// Builtin: reverse(list) (docs/lists.md). A new list; the argument is not changed.
function __cui_reverse(list) { return list.slice().reverse(); }
";

const CONTAINS_JS: &str = "\
// Builtin: contains(text, needle) (docs/lists.md). Case-sensitive; an empty needle is contained in every text.
function __cui_contains(text, needle) { return String(text).indexOf(String(needle)) !== -1; }
";

const LOWER_JS: &str = "\
// Builtin: lower(text) (docs/lists.md). ASCII letters only: other characters are left as they are, so that the page and
// replay cannot disagree about a locale's rules.
function __cui_lower(text) {
    return String(text).replace(/[A-Z]/g, function (c) { return String.fromCharCode(c.charCodeAt(0) + 32); });
}
";

const NUM_JS: &str = "\
// Builtin: num(text, default) (docs/lists.md section 5). A JSON number, exactly: -?(0|[1-9]d*)(.d+)?([eE][+-]?d+)?, no
// space, no plus sign, no hex, no NaN or Infinity; anything else, and a number that overflows to infinity, is `default`.
// Never NaN, never throws.
function __cui_num(text, fallback) {
    var s = typeof text === \"string\" ? text : String(text);
    if (!/^-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?$/.test(s)) { return fallback; }
    var n = Number(s);
    return isFinite(n) ? n : fallback;
}
";

/// The helpers that `emitted` (the components, after the adapters) calls, as one block of JavaScript.
pub fn helpers_used(emitted: &str) -> String {
    let mut out = String::new();
    for (call, js) in [
        ("__cui_sort(", SORT_JS),
        ("__cui_reverse(", REVERSE_JS),
        ("__cui_contains(", CONTAINS_JS),
        ("__cui_lower(", LOWER_JS),
        ("__cui_num(", NUM_JS),
    ] {
        if emitted.contains(call) {
            out.push_str(js);
            out.push('\n');
        }
    }
    out
}
