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
// Builtin: num(text, default) (docs/lists.md section 1). A JSON number, exactly: -?(0|[1-9]d*)(.d+)?([eE][+-]?d+)?, no
// space, no plus sign, no hex, no NaN or Infinity; anything else, and a number that overflows to infinity, is `default`.
// Never NaN, never throws.
function __cui_num(text, fallback) {
    var s = typeof text === \"string\" ? text : String(text);
    if (!/^-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?$/.test(s)) { return fallback; }
    var n = Number(s);
    return isFinite(n) ? n : fallback;
}
";

const PAGING_JS: &str = "\
// Builtins: take(list, n) and skip(list, n) (docs/lists.md section 6). New lists. n is cut toward zero; a negative n,
// NaN, or anything not a number is 0; +Infinity is every element. take keeps the first n, skip drops them.
function __cui_page_count(n) {
    n = typeof n === \"number\" ? Math.trunc(n) : 0;
    return n > 0 ? n : 0;
}
function __cui_take(list, n) { return list.slice(0, __cui_page_count(n)); }
function __cui_skip(list, n) { return list.slice(__cui_page_count(n)); }
";

const MATH_JS: &str = "\
// Builtins: min, max, abs, floor, ceil, round (docs/lists.md section 8). Math.* of numbers; anything that is not a number
// counts as NaN, so a string is never coerced. min and max answer NaN when either is NaN; max(0, -0) is 0 and min(0, -0) is -0.
// round is Math.round: halves go toward +Infinity (round(-2.5) is -2) and a result of zero keeps the sign of its argument.
function __cui_num_arg(n) { return typeof n === \"number\" ? n : NaN; }
function __cui_min(a, b) { return Math.min(__cui_num_arg(a), __cui_num_arg(b)); }
function __cui_max(a, b) { return Math.max(__cui_num_arg(a), __cui_num_arg(b)); }
function __cui_abs(n) { return Math.abs(__cui_num_arg(n)); }
function __cui_floor(n) { return Math.floor(__cui_num_arg(n)); }
function __cui_ceil(n) { return Math.ceil(__cui_num_arg(n)); }
function __cui_round(n) { return Math.round(__cui_num_arg(n)); }
";

const TEXT_JS: &str = "\
// Builtins: lines, split, join, trim, upper, starts_with, ends_with (docs/lists.md section 7). Pure and total. lines and
// split return at most 10000 elements (the rest is dropped); an empty separator does not split; trim and upper are ASCII only.
function __cui_split(text, sep) {
    text = String(text); sep = String(sep);
    return sep === \"\" ? [text] : text.split(sep, 10000);
}
function __cui_lines(text) {
    var parts = String(text).split(\"\\n\");
    if (parts[parts.length - 1] === \"\") { parts.pop(); }
    return parts.slice(0, 10000).map(function (l) { return l.charAt(l.length - 1) === \"\\r\" ? l.slice(0, -1) : l; });
}
function __cui_join(list, sep) { return list.map(String).join(String(sep)); }
function __cui_trim(text) { return String(text).replace(/^[ \\t\\n\\r\\f\\v]+|[ \\t\\n\\r\\f\\v]+$/g, \"\"); }
function __cui_upper(text) {
    return String(text).replace(/[a-z]/g, function (c) { return String.fromCharCode(c.charCodeAt(0) - 32); });
}
function __cui_starts_with(text, prefix) { return String(text).indexOf(String(prefix)) === 0; }
function __cui_ends_with(text, suffix) {
    text = String(text); suffix = String(suffix);
    return suffix.length <= text.length && text.slice(text.length - suffix.length) === suffix;
}
";

/// The helpers that `emitted` (the components, after the adapters) calls, as one block of JavaScript.
const JSON_QUOTE_JS: &str = "\
// Builtin: json.quote(text) (docs/language-design.md decision 15). The JSON string literal of a text, quotes included: the
// quote and the backslash are escaped, control characters get their short escape or a lower-case four-digit hex escape, and
// nothing else changes. This is JSON.stringify of a string, which is what replay's json_quote writes.
function __cui_json_quote(text) { return JSON.stringify(String(text)); }
";

pub fn helpers_used(emitted: &str) -> String {
    let mut out = String::new();
    // A helper shared by two calls (take and skip) is listed once: a module may not declare a function twice.
    for (calls, js) in [
        (&["__cui_sort("][..], SORT_JS),
        (&["__cui_reverse("][..], REVERSE_JS),
        (&["__cui_contains("][..], CONTAINS_JS),
        (&["__cui_lower("][..], LOWER_JS),
        (&["__cui_num("][..], NUM_JS),
        (&["__cui_json_quote("][..], JSON_QUOTE_JS),
        (&["__cui_take(", "__cui_skip("][..], PAGING_JS),
        (
            &[
                "__cui_min(",
                "__cui_max(",
                "__cui_abs(",
                "__cui_floor(",
                "__cui_ceil(",
                "__cui_round(",
            ][..],
            MATH_JS,
        ),
        (
            &[
                "__cui_lines(",
                "__cui_split(",
                "__cui_join(",
                "__cui_trim(",
                "__cui_upper(",
                "__cui_starts_with(",
                "__cui_ends_with(",
            ][..],
            TEXT_JS,
        ),
    ] {
        if calls.iter().any(|c| emitted.contains(c)) {
            out.push_str(js);
            out.push('\n');
        }
    }
    out
}
