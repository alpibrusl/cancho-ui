# Lists and text: `sort`, `reverse`, `contains`, `lower`, `num`, `take`, `skip`, the text builtins (section 7) and the number builtins (section 8)

> **Status: built (written before the code; measured in section 4; `concat` is section 9).** The third item of the ui order that helps
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
* **`lower` is ASCII only,** on purpose (for a caseless search of any language use `fold`, section 10): `toLowerCase` and Rust's `to_lowercase` disagree on `İ` and on a final
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

*(A text-to-number builtin: built, `num`, section 1. Paging: built, `take`/`skip`, section 6.)* *(`starts_with`/`ends_with`/`split`/`join`/`trim`/`upper`/`lines`: built, section 7.)* A locale-aware collation, and `sort` over anything but the keys above. Chromium only.

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

---

## 7. Text to list and back: `lines`, `split`, `join`, `trim`, `upper`, `starts_with`, `ends_with`

> **Status: built (written before the code; measured in 7.3).** Asked for by `alpibrusl/cancho-ui#18`, found by the
> plant-floor spike in `alpibrusl/cancho-industrial#9` (`docs/ui-spike.md` there): an endpoint that answers one record per
> line could not be rendered with `For`, because `net.get` answers a String and the language had no way to a list. The
> JSON route exists (`json.items`, `adapters.md` 14); this is the plain-text route, which needs no change to the server.

### 7.1 The decision

```
lines(text)                 -> List[String]
split(text, sep)            -> List[String]
join(list, sep)             -> String
trim(text)                  -> String
upper(text)                 -> String
starts_with(text, prefix)   -> Bool
ends_with(text, suffix)     -> Bool
```

All pure (no capability), all total, all with the same result in the emitted JavaScript and in replay.

* **`lines`** splits on `\n` and tolerates `\r\n`: one trailing `\r` is removed from each line. A text that ends with a newline has no
  final empty line, so `"a\nb\n"` is `["a", "b"]`; the empty text is `[]`; `"\n"` is one empty line `[""]`; empty lines in
  the middle are kept. **At most 10,000 lines**: a longer answer is cut to its first 10,000 (the bound of the issue's
  fixture; `json.items` has its own, 1,000). The cut is silent, as `json.items`' is, and documented here.
* **`split(text, sep)`** cuts at each non-overlapping occurrence of `sep`, left to right, and keeps empty pieces
  (`"a,,b"` is `["a", "", "b"]`; the empty text is `[""]`). An **empty `sep` does not split**: the result is `[text]`
  (a total answer where JavaScript and Rust would otherwise differ on what "between every character" means for a surrogate
  pair). The same 10,000 bound.
* **`join(list, sep)`** is the inverse for a list of Strings. A list with a non-String element is `replay-type-error` in
  replay; the emitted helper converts the element with `String()` (the same stance as `take`'s count, 6.1).
* **`trim`** removes ASCII white space from both ends: space, tab, newline, carriage return, form feed, vertical tab.
  Nothing else: JavaScript's `trim` also removes U+00A0 and U+FEFF and Rust's removes every Unicode `White_Space`, and the two
  disagree on some, so neither is used.
* **`upper`** is `lower`'s mirror: ASCII `a` to `z` only.
* **`starts_with` / `ends_with`** compare code units from the start or the end, case-sensitively; an empty prefix or suffix
  is always true.

A wrong argument count is `builtin-shape-mismatch`, naming the form. A non-String argument is `replay-type-error` in replay
(the emitted helper converts with `String()`).

### 7.2 What it touches

`PURE_BUILTINS` in lowering (arity only), the one-segment call list in `expr.rs`, the helpers in
`crates/cancho-ui-codegen-web/src/lists.rs` (emitted only when called; `lines` and `split` share a bound constant), the
model in `cancho_ui_replay::eval` with `text_builtin_json` exported for the corpus check. No IR change; no authority change.

### 7.3 Checked

* `cargo test` (`tests/lists.rs`, 3 new tests): the model against `tests/text_corpus.json` (58 cases: the issue's fixtures
  (a trailing newline, an empty answer, a 10,000-line answer and its bound) plus CRLF, a lone CR, empty lines, a separator
  that overlaps itself, the empty separator, non-ASCII text, and for `trim` a no-break space, a byte order mark and an ideographic
  space, none of which is ASCII white space); eight misuses refused with their form and a non-text argument that is
  `replay-type-error`; and the scenario of the issue as a trace: an answer `"temp 21.5 \r\n  hum 40\n\npress 1013\n"` becomes four rows
  through `map(lines(t), l => trim(upper(l)))`.
* `scripts/check-lists.js` (Chromium): **the same corpus against the emitted helpers**, and a page that fetches `readings.txt`,
  renders a row per line, and filters and joins them with `starts_with`/`ends_with`/`join`.

**The checks can fail.** Eleven deliberate breaks of the emitted helpers, each run against the browser script, all killed:
`lines` keeping the trailing empty line (10 checks), unbounded (1), keeping the CR (4); `split` splitting on the empty separator
(2), unbounded (1); `trim` with JavaScript's `\s` (3), trimming only the start (4); `upper` beyond ASCII (1); `starts_with` as
`contains` (1). In the Rust model, eight breaks (the same ideas, plus `trim` without form feed and vertical tab), all killed by the corpus
test. **Two survive and cannot be observed:** `ends_with` without its length guard (a suffix longer than the text can never
equal the shorter tail that `slice` then returns) and `join` without `map(String)` (for a list of Strings, which is all a
checked program has, the two are the same).

### 7.4 Found while building it

* The language has no `||`, `&&` or `!` (a filter that wants "starts with T or ends with 3" is two filters). Not changed here.
* A text that is not a `String` reaches the emitted helpers converted with `String()` and `replay-type-error` in replay, the
  same stance as `take`'s count (6.1): a checked program cannot do it, and the two runtimes differ only on programs the
  checker does not yet type.

### 7.5 Not done

`replace`, `substring`/`slice` of a text, `index_of`, `lower`'s partner `capitalize`, a locale, `lines` on a bound the program
chooses, and reporting that a bound cut the answer. Chromium only.

## 8. Numbers: `min`, `max`, `abs`, `floor`, `ceil`, `round`

> **Status: built (issue #43, found building `examples/ops-console`).** A page count needs `ceil`; the console showed
> "page 4" and not "page 5 of 5".

```
min(a, b), max(a, b)  -> the smaller, the larger
abs(n)                -> the magnitude
floor(n), ceil(n)     -> toward -Infinity, toward +Infinity
round(n)              -> the nearest integer; a half goes toward +Infinity
```

Pure, no capability, an authority of nothing. The emitted helpers are `Math.min`, `Math.max`, `Math.abs`, `Math.floor`,
`Math.ceil` and `Math.round` of their arguments, and replay's `math_builtin` is their model, so the two agree on what
`f64::min`, `f64::max` and `f64::round` do not promise:

* **NaN:** `min` and `max` answer NaN when either argument is NaN (Rust's `f64::min` would answer the other one).
* **The sign of zero:** `max(0, -0)` is `0`, `min(0, -0)` is `-0`, `abs(-0)` is `0`, `ceil(-0.5)` and `round(-0.4)` are
  `-0`.
* **`round` takes halves toward +Infinity:** `round(2.5)` is `3`, `round(-2.5)` is `-2`, `round(-3.5)` is `-3` (Rust's
  `f64::round` says `-3` and `-4`), and the largest double below one half rounds to `0` (the textbook `floor(x + 0.5)`
  says `1`).
* **Not a number is not coerced:** the emitted helper turns anything that is not a number into NaN (`min("1", 2)` is NaN,
  not `1`); replay refuses it as `replay-type-error`, the way it refuses `take(list, "1")`.
* **Wrong argument count:** `builtin-shape-mismatch`, with the form (`min(a, b)`, `abs(n)`).
* **Not a way round `replay-non-finite`:** `ceil(m / 0)` faults in replay, because `m / 0` does first, and since the
  arithmetic became finite in the page too (`docs/semantics.md`, "Arithmetic is finite") it throws there as well, instead
  of `ceil(Infinity)`.

Checked: `tests/math_corpus.json` (64 cases: both argument orders, NaN on either side, both zeros, both infinities, halves
either side of zero, the double below a half, the edge of the exact range at 2^52, 1e300) against the model by
`cargo test` (comparing bits) and against the emitted JavaScript in Chromium by `scripts/check-lists.js` (comparing with
`Object.is`); a trace that counts pages; the page of `scripts/check-lists.js`; the ops console. Eleven deliberate breaks
of the model, the emitter and the lowering (round as `floor(x + 0.5)`, round as `f64::round`, `min` through `f64::min`,
`max` keeping `-0`, `round` losing the sign of zero, `round` not emitted, `min` of arity one; in the emitted JavaScript
`round` as `floor(n + 0.5)`, no coercion guard, `ceil` as `floor`, `max` as `min`) each fail at least one check.

## 9. Joining lists: `concat`, and `+` is for numbers and text (#39)

> **Status: built (written before the code; measured below).**

**Measured first** (the merge of #70): `state l: List[String] = ["a"]; on Go { l = l + l; }` passed `check` (exit 0), the
emitted JavaScript was `__state_l = (__state_l + __state_l);`, which is the **string** `"aa"` (arrays stringify), so the
state stopped being a list. Replay refused the same program when it ran (`replay-type-error`: `+` with a list), so the
two evaluators disagreed about a program the checker accepted, and the language had no way to join two lists at all.

**Decision.**

```
concat(a, b)   -> a new list: the elements of a, then those of b
```

* Pure, no capability, two lists. Neither input changes; a list that is an element stays one element
  (`concat([1], [[2, 3]])` is `[1, [2, 3]]`, not `[1, 2, 3]`). The same list twice doubles it. Order is kept, not sorted.
* Anything but two lists is `replay-type-error` in replay, and the emitted helper throws an `Error` that starts with the
  same rule name (a string passed to JavaScript's `concat` would join as text, the very defect of `+`). Since #59 a handler
  that throws is undone, so the page and replay stop in the same state.
* A wrong argument count is `builtin-shape-mismatch` naming `concat(a, b)`.
* **`+` is refused at `check` when an operand is known to be a list, a `Result`, an `Option` or a `Request`, or when it adds a
  `Bool` to a number or a `Bool`: `operand-type-mismatch`** (`spec/diagnostics/README.md`). A string with anything stays
  text (`"ready: " + ready` is allowed, as replay allows it). As in `docs/conditionals.md` section 3.1, the pass refuses
  only a type it knows; an event parameter or a `For` binding is `Unknown` and replay refuses it when it runs. When both
  operands are lists the message says to write `concat(a, b)`.

**Checked.** `tests/concat_corpus.json` (10 results, 7 refusals): through the real replay, one trace step per case
(`crates/cancho-ui/tests/lists.rs`, the refusals as `fault` steps that leave the state as it was), and against the emitted
`__cui_concat` in Chromium (`scripts/check-lists.js`, which also checks that the inputs are unchanged and the result is a new
list, and a page that joins two slices of a list). Eight refused `+` programs and five accepted ones; the helper is emitted
only when used; three wrong argument counts.

**Deliberate breaks, each caught:** the emitted helper without its list check (7 checks fail), arguments swapped (7), joined in
place (18); the replay arm with the two lists swapped (the corpus test fails); the checker letting a list through (the `+`
test fails); the checker letting `Bool + number` through (the `+` test fails).

**Not done.** `-`, `*` and `/` on a known non-number are not refused statically (replay faults when they run, as before); a
`+` whose operand has a type the pass cannot see is still left to replay.

## 10. Case-insensitive search for any language: `fold` (#37)

> **Status: built (written before the code; measured below).**

**Measured first** (`docs/ops-console.md` 2.6): with the records `Pumpe Ünit` and `pumpe ünit`, `contains(lower(name), lower(q))`
found only the lower-case record for `ünit` and only the upper-case one for `ÜNIT`, because `lower` is ASCII only on purpose
(section 1: `toLowerCase` and Rust's `to_lowercase` disagree on `İ` and on a final sigma). For German, Spanish, French or
Greek names that makes a case-insensitive search wrong.

**Decision.**

```
fold(text)   -> the text with every character replaced by its simple case folding (one code point for one code point)
```

* **Simple case folding** is Unicode's `CaseFolding.txt` statuses C and S. It is the folding meant for caseless matching and it
  never changes the number of characters (`ß` stays `ß`, and `ẞ` folds to `ß`; the full folding `ß` to `ss` is status F and is
  not used). `fold(a) == fold(b)` is the caseless comparison; `contains(fold(name), fold(q))` is the search.
* **One table, in both runtimes.** `crates/cancho-ui-ir/src/casefold_table.rs` is generated by `scripts/gen_casefold.py`
  (1,454 code points in 198 runs, Unicode 15.1.0) and checked in. Replay reads it through `cancho_ui_ir::casefold::fold_text`;
  the emitted `__cui_fold` is handed the very same runs. Neither runtime consults its own library, so they cannot disagree
  the way `lower` could. The generator is a development tool run by hand (Python's `unicodedata`; it derives the simple
  folding from the full one, and asserts the cases the standard names); the product, the compiler and the emitted code do not
  use Python.
* **Language-neutral, so not Turkish.** `I` folds to `i` everywhere; `İ` (U+0130) and `ı` (U+0131) are unchanged, so a Turkish
  user typing `ISTANBUL` finds `istanbul` but not `İstanbul`. A locale-aware fold (the Turkic mappings) is not built.
* **Code points, not UTF-16 units.** An astral letter (Deseret `𐐀`) folds as one. A lone surrogate, which a JavaScript string can
  hold and Rust text cannot, is passed through unchanged.
* **Types.** `fold` takes one String; anything else is `replay-type-error` in replay and the emitted helper throws an `Error` that
  starts with the same rule name (undone since #59). A wrong argument count is `builtin-shape-mismatch` naming `fold(text)`.
  The checker types its answer as a String. No capability; the helper is emitted only when used.
* **`lower` and `upper` are unchanged** (ASCII only), and `sort` still orders by UTF-16 code units: a caseless or accent-aware
  *order* needs a collation, which is not built.

**Cost.** A page that calls `fold` carries the table: measured 4,928 bytes against 1,292 for a page that calls `lower` (+3,636).

**Checked.** `tests/fold_corpus.json`, generated with the table: 20 cases (accents, Greek incl. final sigma, Turkish `İ`/`ı`,
`ß` and `ẞ`, Cyrillic, Armenian, Cherokee, an astral letter, a title-case digraph, combining marks, an emoji, idempotence), 4
pairs that must fold equal and 4 that must not, and **a digest over the fold of every code point** (1,112,064 of them, FNV-1a)
computed by the generator from Python's own folding. The digest is checked in `cargo test` against the Rust table and in Chromium
against the emitted helper (`scripts/check-lists.js`); the cases go through the real replay as trace steps and through the
emitted `__cui_fold`; a search program shows `ünit` and `ÜNIT` finding both records where `lower` finds one; a page where typing
`ÜNIT İ` shows `ünit İ`.

**Deliberate breaks, each caught:** the emitted helper ignoring the stride of a run (2 checks fail), using the engine's
`toLowerCase` (5), stepping by UTF-16 units (2), passing ASCII through (10); in Rust the stride ignored (2 tests), one run's delta
changed in the table (3), replay using `to_lowercase` (1).

**Not done.** A Turkic or locale-aware fold; a collation for `sort`; normalization (`é` as one code point and as `e` plus a combining
accent fold differently, as Unicode specifies; NFC/NFD is not built); full folding (`ß` to `ss`).
