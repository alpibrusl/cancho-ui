# Comparing values: `==`, `!=`, `<`, `<=`, `>`, `>=`

> **Status: BUILT** (alpibrusl/cancho-ui#95). Found reading the answer of `net.post` in `examples/tickets` (`docs/tickets.md`
> section 3). The measurement came first; section 4 says what was built.

## 1. What was measured before the change

`json.get` answers a String, so the natural check of a status, `json.get(reply, "status", "") == 200`, passed `cancho-ui check`
and is **always false** in both evaluators. Measured over every pair of types and every operator, one expression per program, in
`cancho-ui test` (replay) and in the emitted page in Chromium (state: `a: Int = 7`, `s: String = "7"`, `t: Bool = true`,
`l: List[String] = ["x"]`, `r: Result = Ok("7")`):

| operands | `==` `!=` before | `<` `<=` `>` `>=` before |
|---|---|---|
| number, string (either order) | accepted; replay and page agree: `==` false, `!=` true | accepted; **replay faults** (`replay-type-error`), the **page coerces** (`"7" <= 7` is true, `"7" < 7` false) |
| number, bool | accepted; agree: false / true | replay faults; page coerces (`7 >= true` true) |
| string, bool | accepted; agree: false / true | replay faults; page coerces |
| bool, bool | accepted; agree | **replay faults**; page orders them (`true >= true` true) |
| list, list (`l == l`) | accepted; **replay faults** (it does not model reference equality), the **page** answers by identity (`true`) | replay faults; page answers |
| list, number; `Result`, string | accepted; replay faults; page answers `false`/`true` | the same |
| number, number; string, string | agree | agree (strings by UTF-16 code unit in both) |

So there are two different defects. Across types, `==` is a silent constant. Everywhere else the two evaluators **disagree**: a
program that replay refuses runs in the page and answers something. Replay's reading is the language's: it defines `==` and `!=`
on two numbers, two strings or two bools, and orders two numbers or two strings, and nothing else.

## 2. Options

| | Option | Against |
|---|---|---|
| A | Refuse, at `check`, a comparison whose operand types are both **known** and are not one of those pairs (`comparison-type-mismatch`) | a type the pass cannot see (an event parameter, a `For` binding) is not refused; the residue is section 5 |
| B | Make the page strict too: a runtime helper for every comparison that throws `replay-type-error` as replay does | bytes in every page for every comparison; and a program with a mistake still builds |
| C | Make `==` across types a type error only (as the issue first asked) and leave `<` alone | leaves the measured disagreement in place for `<`, `>`, lists and bools |

**Chosen: A, for all six operators.** `<` and `>` need it as much as `==`, and more: they are where the evaluators disagree. B
is the only way to close the residue and is a separate decision (section 5).

## 3. A numeric sibling of `json.get`: not built

The issue asked whether `json.get` should have one (`json.get_num`, say) so that a status is a number without passing through
text. Not built, for three reasons.

* It is already one call: `num(json.get(reply, "status", ""), 0) == 200` (`num(text, default)` reads a number from text, or
  answers the default). Measured: both evaluators answer `true` (`tests/compare_corpus.json`, "the status of a reply, as a
  number").
* The failure was never a missing capability. It was a comparison that said nothing, and it is refused now, with a remediation that
  names `num(text, default)` and the other spelling, a string literal (`== "200"`, which `examples/tickets` uses).
* A sibling is a new builtin: a name in `AGENTS.md`, `call-unknown`'s list, both evaluators, a corpus, and its own rules for a
  member that is not a number. It would add that for a composition that exists.

## 4. What was built

* **The rule.** `comparison-type-mismatch` (`spec/diagnostics/README.md`), in `crates/cancho-ui-ir/src/conditions.rs`: when both
  operand types are known, `==` and `!=` need two numbers, two strings or two bools, and `<`, `<=`, `>`, `>=` need two numbers or
  two strings. The message names the types and, for a text against a number, `num(text, default)` and the string literal; for a
  list, `len(a) == len(b)` or `join`; for a `Result`, `match`.
* **Measured.** `tests/compare_corpus.json`: 36 accepted cases (every operator on numbers, strings and bools; integer against
  float; `0 == -0`; `0.1 + 0.2 == 0.3`; `"B" < "a"`; `"10" < "9"`; a surrogate pair against U+FF5E; `num(..)` and `json.get(..)`
  spellings of a status; `len(l) == 1`), 25 refused (each pair of the table in section 1 and each operator class, lists and
  `Result`s), 2 that stay accepted because the type is unknown. `cargo test` replays the accepted ones and checks the refusals
  (`crates/cancho-ui/tests/comparisons.rs`); `scripts/check-comparisons.js` runs the accepted ones in Chromium and has
  `cancho-ui build` refuse the others. Both evaluators agree on all 36.
* **Every example still checks**, and `docs/demo` is unchanged.
* **Deliberate breaks:** `scripts/breaks/comparisons.json`, seven (the rule off; `==` across scalar types, `<` on bools, `<` across number and string, `==` on lists and on a `Result` accepted; an unknown type refused), each caught.

## 5. What is left, and why

*(Corrected, #103, section 6: the residue below is closed by option B, applied only where an operand's type is not seen.)* A comparison with an operand whose type the pass cannot see is not refused: an event parameter, a `For` binding, a `match` arm
(`docs/conditionals.md` section 6). Measured: a `TextField` handler `on Param(x) { out = "" + (x <= 7) + " " + (x == 7); }` given
`"7"`: replay faults at `<=` (`replay-type-error`), the page answers `true`; `==` is `false` in both. Closing that is option B (a
strict comparison in the page), a runtime cost in every comparison, and is filed as its own issue (#103) rather than done here.

## 6. An operand the compiler cannot type: the page is strict there (issue #103)

> **Status: BUILT.** Option B of section 2, applied only where it is needed.

**Measured before** (`scripts/check-compare-unknown.js <older compiler> --report`): over `tests/compare_unknown_corpus.json`, 128 comparisons of an
event parameter, a lambda binding and a `match` binding against numbers, strings and bools, **62 answered differently** in the page and in replay: replay
faults (`replay-type-error`), the page coerced (`"7" <= 7` true, `true >= 7` true, `7 < true` false). The probe of the issue (a `TextField` given
`7`, handler `x <= 7`): replay faults, the page answered `true`.

**What was built.**
* `__cui_lt`, `__cui_le`, `__cui_gt`, `__cui_ge`: throw `replay-type-error` unless both operands are numbers or both are strings, then compare. `__cui_eq` (and `!=` as its negation): throws if either
  operand is a list, a variant or not a value, else strict equality, which across scalar types is false, as replay's. Each is a few lines, emitted only when a comparison needs it (the order
  helpers and the equality helper are separate chunks).
* **Where.** A comparison goes through a helper only when an operand is not *known* to be a number, a string or a bool: `scalar_kind` in `conditions.rs` is the same typing the
  `comparison-type-mismatch` rule uses (a literal, a state field's declared type, a builtin whose answer is fixed); an event parameter, a `For` or lambda binding and a
  `match` binding are not known. Where both operands are known the compiler has already refused what replay would, and the host's own operator is emitted, as before.
* **The cost, measured** (the module `cancho-ui build` writes, before and after):

| example | before | after | added |
|---|---|---|---|
| `examples/ops-console` | 48,524 bytes | 48,524 | **0** (no comparison of an unknown operand) |
| `examples/tickets` | 48,919 | 49,225 | **306** (0.6 %: `__cui_eq` and one call, `json.get(x, "id", "") == id`) |
| `examples/cancel` | 12,726 | 13,226 | 500 (the ordering helpers) |
| `examples/routes` | 14,988 | 15,299 | 311 |

  Every other example is byte-for-byte unchanged. `docs/demo/routes/app.js` is regenerated.
* **After:** all 128 cases answer what replay answers (value, or `replay-type-error`), in `cargo test` (`crates/cancho-ui/tests/compare_unknown.rs`) and in Chromium; the probe is a fault in both, and the
  state is unchanged. A page throws the error from the handler: the program stops at that statement, the state keeps what it had, and the error is the page's uncaught error (replay stops the trace).

**A `For` binding in the view.** Replay does not draw the view, so it never evaluates `Text { "{n <= 5}" }` inside a `For`: that case cannot be compared with replay. The page is strict there too (checked in Chromium,
numbers compare, strings against a number throw), and the corpus covers the same thing through a lambda binding, which both evaluators evaluate. **Not modelled in replay:** a comparison in a view expression.

**Not done.** Typing event parameters (`on Pick(id: String)`, option 2 of the issue; #56) closes the hole at the source and is the larger change; this does not preclude it.
