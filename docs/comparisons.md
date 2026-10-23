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

A comparison with an operand whose type the pass cannot see is not refused: an event parameter, a `For` binding, a `match` arm
(`docs/conditionals.md` section 6). Measured: a `TextField` handler `on Param(x) { out = "" + (x <= 7) + " " + (x == 7); }` given
`"7"`: replay faults at `<=` (`replay-type-error`), the page answers `true`; `==` is `false` in both. Closing that is option B (a
strict comparison in the page), a runtime cost in every comparison, and is filed as its own issue (#103) rather than done here.
