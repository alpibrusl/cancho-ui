# Lists and text: `sort`, `reverse`, `contains`, `lower`, `num`

> **Status: built (written before the code; measured in section 4).** The third item of the ui order that helps
> `cancho-industrial`: a console shows rows, and a person filters and orders them. `filter`, `map` and `len`
> (Tier 11) already exist; what was missing is an order and a way to ask a text a question.

## 1. The decision

```
sort(list, x => key)        -> a new list, stable, ascending by key
reverse(list)               -> a new list, the elements in the opposite order
contains(text, needle)      -> Bool; case-sensitive; an empty needle is in every text
lower(text)                 -> the text with ASCII A to Z made a to z
num(text, default)          -> the number the text spells, or default
take(list, n), skip(list, n) -> the first n elements, the rest (section 6)
```

All four are **pure**: no capability, no effect, they run inside the render pass or a handler. `sort`'s second
argument is a lambda like `filter`'s and `map`'s, it never escapes, and it must be pure (`pure-lambda-required`);
`sort(list, "k")` is `builtin-shape-mismatch`, as is any wrong argument count, and the message names the form.

* **The order of keys is total and the same in both runtimes.** Numbers first, numerically, NaN after every number;
  then strings, by UTF-16 code units (JavaScript's `<` on strings; replay compares `encode_utf16`); then any other key
  (a bool, a list), which compares equal to its own kind. A mixed list therefore sorts without an error, and a program
  that wants one kind makes its key one kind (`json.get` always answers a String, so sorting rows by a JSON field is
  by text; to sort by number the program has to produce a number, and today it has no text-to-number builtin: 5).
* **Stable.** Equal keys keep their order; `reverse(sort(..))` therefore reverses ties too.
* **`lower` is ASCII only,** on purpose: `toLowerCase` and Rust's `to_lowercase` disagree on `İ` and on a final
  sigma, and a page and its replay must not disagree. `contains(lower(a), lower(b))` is the case-insensitive search.
* **`num` reads exactly a JSON number** (`-?(0|[1-9][0-9]*)(.[0-9]+)?([eE][+-]?[0-9]+)?`, ASCII digits only) and
  nothing else: no space, no `+`, no leading zero, no `.5` or `5.`, no hex, no `NaN`/`Infinity`, no decimal comma. A
  text that is not one, and a number that overflows to infinity (`1e999`), is `default`; the result is never NaN and
  `num` never throws. `-0` stays `-0`; decimals are correctly rounded in both runtimes (Rust's `parse::<f64>` and
  JavaScript's `Number`). The default is a number (a String default is `replay-type-error` in replay). `json.get`
  answers a String, so `num(json.get(row, "n", ""), 0)` is a number to sort by: `sort(rows, r => num(json.get(r,
  "n", ""), 0))`. The type is a number, not an `Int`: the checker does not track it, as it does not track `json.get`.
* **No sort descending.** `reverse(sort(..))` is it; one way to say it.

## 2. What it touches

* **IR:** a node `SortBy { list, item, key }` (the shape of `Filter`/`Map`), serialized, rendered by `cancho-ui diff`
  as `sort(list, x => key)`. The other six are `Call`s of a one-segment path that need no capability, so a
  program's authority is unchanged by using them.
* **Emit:** the helpers live in `crates/cancho-ui-codegen-web/src/lists.rs` (`emit.rs` was near the 2,000-line
  ceiling and has since been split into `emit.rs`, `expr.rs`, `view.rs` and `adapters.rs`, with byte-identical output) and are emitted only when the program calls them, found by looking at what was emitted, as `json.get` is.
* **Replay:** `IrExpr::SortBy` and the three calls, with `sort_key_order` as the model of the emitted comparator.

## 3. Found while building it (not changed here)

1. **A `match` arm `true` or `false` is a binding, not a literal.** `match flag { true => {..} false => {..} }`
   compiled, and the first arm always matched (`Pattern::Ident` is a catch-all binding; the emitted code was
   `true ? .. : true ? ..`). The browser check first used it and showed the "desc" order at load. *(Corrected
   2026-10-21, issue #26: it is refused now, `pattern-literal-unsupported`, by `check`, `build` and `test`; tests in
   `crates/cancho-ui/tests/pattern_literals.rs`.)*
2. **A state initializer that calls a function is `undefined` in the emitted JavaScript** (and replay refuses it:
   `unsupported_init`), as already documented; `sort(..)` in an initializer is the same. Fill a list in a handler.

## 4. Checked

* `cargo test` (`crates/cancho-ui/tests/lists.rs`, 6 tests): the model against `tests/lists_corpus.json` (15 cases:
  numeric not textual, code-unit order, numbers before strings, stability, empty and one element, NaN last, `-0`,
  a bool/list/null key, an astral character before a BMP high one, prefixes, digits as text, case); the helpers are
  emitted only when used; seven misuses refused with their form; an impure key; a trace that filters, sorts and
  reverses as the query changes; a sort by a constant key and a non-text `contains` argument do not panic.
* `scripts/check-lists.js` (Chromium, outside the gate): **the same corpus against the emitted `__cui_sort`**, the
  `lower`/`contains`/`reverse`/`num` helpers, and a page where typing filters a sorted list and a button reverses it.

* `num` (added after): `tests/num_corpus.json` (45 cases: the grammar and every way to leave it, overflow, `-0`, rounding, Arabic-Indic and full-width digits, a trailing newline) against `num_of_text` in `cargo test` and against the emitted `__cui_num` in `scripts/check-lists.js`, and a page that sorts text rows by number.

**The checks can fail.** Six deliberate breaks of the emitted helpers, each run against the browser script: numbers
and strings swapped in the rank (killed by 4 checks), NaN not last (1), `lower` over more than ASCII (3), `contains`
case-insensitive (1), `reverse` in place (1). **One survives, and cannot be observed:** removing the index tie-break
from `__cui_sort`. `Array.prototype.sort` is stable in every engine since ES2019, so the tie-break changes nothing
there; it stays so that the result does not depend on the engine. Six more for `num`, on the emitted helper: the finite check removed (killed by 2 corpus cases), leading zeros allowed (2), a plus sign allowed (1), `5.` allowed (2), the end anchor removed (7); and in the Rust model the leading-zero rule removed (killed by the corpus test).

## 5. Not done

*(A text-to-number builtin: built, `num`, section 1. Paging: built, `take`/`skip`, section 6.)* `starts_with`/`ends_with`/
`split`/`join`/`trim`, `upper`, a locale-aware collation, and `sort` over anything but the keys above. Chromium only.

---

## 6. Paging: `take` and `skip`

### 6.1 The decision

```
take(list, n)   -> a new list: the first n elements
skip(list, n)   -> a new list: everything after the first n
```

Pure, no capability, plain calls like `reverse`. A page is `take(skip(rows, page * size), size)`; the number of pages is
`len(rows)` over `size`, computed by the program. Neither changes its argument, and elements of any kind are kept as they
are.

* **`n` is cut toward zero** (`1.9` counts 1), **a negative `n` counts 0, `NaN` counts 0, `-0` counts 0**, `+Infinity`
  counts every element and `-Infinity` counts 0; an `n` larger than the list is the whole list (`take`) or nothing
  (`skip`). So a page number computed from user input (`p * 2` with `p` = `-1`) is the first page, never an error.
* **Replay and the browser agree on every number** (`tests/paging_corpus.json`, 18 cases). They differ on one thing
  only, as `num` does: replay refuses an `n` that is not a number (`replay-type-error`, the trace is wrong), while the
  emitted `__cui_page_count` answers 0 for anything that is not a number (including a numeric string: `"2"` is 0, it is
  not coerced), because a page must not throw.
* The helpers are one block (`__cui_page_count`, `__cui_take`, `__cui_skip`) emitted once whether a program uses one or
  both: a module may not declare a function twice.

### 6.2 Checked

* `cargo test` (`tests/lists.rs`, 4 tests): the model against the corpus; a trace that pages a list (pages 0 to 3 and a
  negative page); the helper is emitted once when both calls are used and not at all when neither is; wrong arity and a
  non-number count.
* `scripts/check-lists.js` (Chromium): **the same corpus against the emitted helpers** (take, skip, and that the input
  list is unchanged), a count that is not a number, and a page that skips one and takes two.

**The checks can fail.** Six deliberate breaks of the emitted helper, each run against the browser script: `Math.round`
for `Math.trunc` (killed by 4 checks), a negative count not clamped (2), `take` mutating its argument (12), `skip` off by
one (14), no number check (1: only the numeric-string case sees it), a negative count taken as 1 (8); and in the Rust
model `page_count` answering 0 for a count beyond the list (killed by the corpus test). The Rust model's `trunc` and its
NaN test are **equivalent mutants**: `f64 as usize` already cuts toward zero and saturates NaN to 0, so removing either
changes nothing; they stay for the reader.

### 6.3 Not done

A page count (`ceil`), a `slice(list, from, to)`, a `last(list, n)`, and keeping the current page when the list shrinks
(the program's decision). Chromium only.
