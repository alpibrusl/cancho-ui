# Conditionals: `if`, `&&`, `||`, `!`

> **Status: BUILT, step 1 (the expression form, with `else if`) and, since 2026-10-22, the statement form (`docs/handler-sequences.md`, #93). Derived state and literal patterns are not.**
> Asked for by `alpibrusl/cancho-ui#40` (found building `examples/ops-console`, `docs/ops-console.md` 2.2). The document was a
> design first; the questions of section 5 were answered by following its own recommendations (section 5, "Decided"), and
> section 6 says what was built, what it measured and what differs from the estimate. Sections 1 to 4 are kept as the design
> was written; where the build corrected a claim, the correction is in place and says so.

> **See also:** `docs/comparisons.md` (what `==` and `<` accept, #95) and `docs/view-conditional.md` — a conditional in the *view* (`If { when: cond .. }`), which this document's `if`
> expression could not be (alpibrusl/cancho-ui#90).

## 1. What exists today

* Expressions: literals, state names, calls of the pure builtins, `+ - * /`, `== != < <= > >=`, unary `-`, `Ok(..)`/`Err(..)`,
  and `match` (`BinOp` has no boolean operator and `IrUnOp` has only `Neg`; the lexer refuses `&&`, `||` and `!` with
  `lex-unexpected-char`).
* `match` works on a `Result`/`Option`/effect answer. A pattern is `_`, a binding name, `Name(binding)` or `Name`. **There are no
  literal patterns.** A `true` or `false` arm used to bind a name that matched everything (#26); it is now refused as
  `pattern-literal-unsupported`, so today there is no way to branch on a `Bool` at all.
* Handlers hold assignments and expression statements only. A handler cannot call another and there is no local `let` or derived
  state.
* A `Bool` does exist: `Bool` state fields, `==`/`<` and friends produce one, and `disabled:` takes one.

What the missing branch costs, counted in `examples/ops-console/ops-console.cui`:

| Where | What it is written as | What it means |
|---|---|---|
| `on Next` | `page + len(take(skip(shown, (page + 1) * 5), 1))` | "one more page if there is one" |
| `on Prev` | `page - len(take(["x"], page))` | "one less page if page > 0" |
| `on Refresh` | `page * len(take(skip(shown, page * 5), 1))` | "back to 0 if the page is now empty" |
| `on Apply` | `num(setpoint, 1) == num(setpoint, 2)` | "setpoint is a number" |
| three handlers | `shown = sort(filter(rows, ..), ..)` (3 copies, the same line) and `paged = take(skip(shown, ..), 5)` (5 copies) | the derived values, written out because nothing can be shared |

The last row is **not** a conditional problem: no `if` removes it. It is derived state (`derive shown = ..`), and it is the
larger share of the example's repeated text. Section 5 returns to it.

## 2. The options

| | Option | What it adds | Fixes #40 rows 1 to 4 | Fixes the Bool hazard of #26 |
|---|---|---|---|---|
| A | **`if` expression and statement, plus `&&`, `\|\|`, `!`** | one construct that yields a value (`if c { a } else { b }`), and the three operators on `Bool` | yes | yes: a `Bool` is branched on with `if`, never with `match` |
| B | **Literal patterns** `true`/`false` (and `_`) in `match`, with exhaustiveness | `match c { true => {a} false => {b} }`; the same machinery would cover `Int` and `String` literals | yes, in a heavier syntax; `&&` and `\|\|` still missing | yes, by making the arm mean what it says |
| C | **A pure builtin `choose(c, a, b)`** | no syntax; both `a` and `b` are evaluated | partly: rows 1 to 3, awkwardly | not touched (`match` on a `Bool` stays refused) |
| D | **Do nothing**; keep the list arithmetic | nothing | no | the refusal stays |

### 2.1 Why not C

`choose(c, a, b)` is cheap and honest about being eager, but: it has no statement form (a handler that sets two fields on a
condition needs two `choose`s and repeats the condition); it cannot guard a partial expression (`b != 0 && a / b > 1` needs
short-circuit; `choose(b != 0, a / b, 0)` divides by zero in the other branch); and it does not give `&&`/`||`, which is what
most readers reach for first. It is the right stopgap only if A is refused.

### 2.2 Why not B alone

Literal patterns are the principled fix to #26's confusion (a name that looks like a literal). But they cost more than A for
less: the parser must tell a literal from a binding, exhaustiveness must grow beyond constructors (`match-non-exhaustive` for a
`Bool` needs `true` and `false`, or `_`; for `Int`/`String` it needs `_` always), and replay and the emitter need a literal
test per pattern. And the result is still not an operator: `a && b` becomes a nested `match`. B is worth doing *later*, as the
generalisation, for `Int` and `String` state (a `Mode` string, a small enumeration), not as the answer to #40.

## 3. Recommendation: A, in two steps

**Step 1 (the decision requested):** `&&`, `||`, `!` on `Bool`, and `if cond { a } else { b }` as an **expression**, in any
position an expression can stand (an assignment's right side, a binding, an argument, a `Text` label).

**Step 2 (after step 1 is measured; built 2026-10-22 as #93, `docs/handler-sequences.md`, and wider than proposed here: its branches may hold effects):** the **statement** form `if cond { assignments } else { assignments }` inside a handler.
It is only syntax over step 1 for assignments (`x = if c { a } else { x }`), but it is what removes the duplicated condition.
It can wait because step 1 already deletes the four unreadable rows of section 1.

### 3.1 Semantics

* **Pure.** The condition and both branches are *pure expressions*: no effect call, no `await`, no `net.*`, `storage.*`, `clock.*`
  or `location.*`, in the condition or in a branch. An effect inside is refused with a tagged rule, because an `if` that
  issues a request on one side would make the *set of issued effects* depend on a value the replay trace has to supply, which
  the trace format (`steps` with `issued`) does not describe today. (A handler that must issue an effect only on a condition
  can still be written as two events; a statement-level `if` around an effect is a separate decision, not proposed.)
  **Corrected 2026-10-22 (alpibrusl/cancho-ui#93, `docs/handler-sequences.md`).** The reason given above is wrong for a condition
  over state. What a trace supplies is the *answer* to a request that was issued; whether one is issued is decided by the handler
  from the state, which replay computes exactly as the page does, and the step's `issued` is derived, not supplied. The expression
  form stays pure (a value has no meaning when one side suspends and the other does not), but the *statement* form may hold
  effects in its branches; "two events and let the view pick the button" does not cover a choice made after an answer.
* **Typed.** The condition must be `Bool`. Both branches must have the same type. `else` is **required** in the expression form
  (no unit value to fall back on); the statement form may omit it.
* **Strict operators, short-circuit evaluation.** `a && b` and `a || b` evaluate `b` only if `a` does not decide the answer,
  exactly as JavaScript does. Because both operands are pure, short-circuit is *observable* through division: **measured in the code,
  the two evaluators disagreed on `a / 0`**. Replay refused it (`finite()` in `eval.rs` faults with `replay-non-finite`);
  the emitted JS (`a / b` through `BinOp::js_op`) yielded `Infinity` or `NaN` with no guard. Short-circuit therefore decides
  whether `b != 0 && a / b > 1` is a value or a replay fault, so the division disagreement was a prerequisite of this design.
  **Settled** (section 5, question 5; `docs/semantics.md`, "Arithmetic is finite"): the emitted `+ - * /` now refuse a result that is
  not finite as replay does.
  The design keeps short-circuit because that guard is the use everybody will write.
* **Precedence** (low to high): `||`, `&&`, comparisons, `+ -`, `* /`, unary `- !`. `!` binds tighter than a comparison, so
  `!a == b` is `(!a) == b`, which is a type error (`Bool == Int` is already refused where it is wrong).

### 3.2 Exhaustiveness

An `if` expression is exhaustive by construction (`else` required). No change to `match-non-exhaustive`. A `match` on a `Bool`
is **still refused** (`pattern-literal-unsupported`); its remediation text should change from "there are no literal patterns" to
"branch on a Bool with `if cond { a } else { b }`". That keeps #26's guarantee (no silent wrong result) and gives the refusal a
way out, which today it does not.

### 3.3 Replay parity with the emitted JavaScript

The project has two evaluators of one language: `cancho-ui-replay/src/eval.rs` (what `cancho-ui test` runs) and
`cancho-ui-codegen-web/src/expr.rs` (what the page runs). A construct is not done until both give the same answer, and the two
differ in kind: the emitted JS gets `&&`, `||`, `!` and `?:` from the host language; the Rust evaluator must reproduce them.
The places they can disagree, and the check each needs:

| Risk | Emitted JS | Replay | Check that would catch it |
|---|---|---|---|
| a non-`Bool` reaching `&&` (JS `&&` returns an operand, not a `Bool`) | the checker refuses it, so JS only ever sees `Bool` | the same refusal | a type-error test per operand position, and a trace that reads the result into a `Bool` field |
| short-circuit | JS `&&`/`\|\|`/`?:` | `eval.rs` must not evaluate the right side first | a trace and a Chromium page on `b != 0 && a / b > 1` and `a == 0 \|\| 100 / a > 5`, same output in both |
| an `if` in a `Text` interpolation | `${c ? a : b}` | `eval.rs` | a view test: the label text under both values |
| precedence | emitted with explicit parentheses around every operator | the parser's own tree | a parse-print-reparse round trip (`cancho-ui fmt` if it exists, else a test over the AST) |
| the typed value of `!` on a state field changed by an earlier statement in the same handler | statements run in order | statements run in order | a two-statement handler test |

`scripts/check-conditionals.js` (not written) would build one page that uses all four in a handler and a view, and the same
trace through `cancho-ui test`, and compare. The ops-console would be rewritten with step 1 as the measured example: the three
unreadable rows become `page = if page + 1 < pages { page + 1 } else { page }`-style lines, and their traces must pass
unchanged, which is the replay-parity evidence for a real program.

### 3.4 Proposed rule tags (to be catalogued with the build)

| Tag | When |
|---|---|
| `condition-not-bool` | the condition of an `if`, or an operand of `&&`, `\|\|`, `!`, is not a `Bool` |
| `if-branch-type-mismatch` | the two branches of an `if` have different types |
| `if-effect-in-expression` | a condition or a branch calls an effect or awaits |
| `if-else-required` | an `if` expression without `else` |

`pattern-literal-unsupported` keeps its tag and changes its remediation only.

### 3.5 What it touches (an estimate to be corrected by the build)

Lexer (`&&`, `||`, `!`, keywords `if` and `else`; `!=` must still lex as one token); parser (precedence, the `if` expression);
AST/IR (`BinOp::And`/`Or`, `UnOp::Not`, `IrExpr::If`); the checker (the four rules); both evaluators; the emitter; `cancho-ui
diff` (an `if` inside a handler changes the semantic text of that handler, which already diffs as text of the handler);
`AGENTS.md` (the "do not match on a Bool" workaround is removed); `spec/diagnostics/README.md`. No effect, capability or trace
format change. Each source file stays under the 2,000-line ceiling: `eval.rs` is 1,064 lines and `expr.rs` 412.

## 4. The hazards, said first

* **`if` invites logic into the view.** A page full of nested `if` is what the language was shaped to avoid. Mitigation: `if`
  stays an expression of the same pure kind as `filter` and `sort`; no statement-level `if` around effects.
  *(The second half was revised 2026-10-22, #93: a statement `if` may hold effects. The view still has only the expression form;
  `If { when: .. }` of `docs/view-conditional.md` is a node, not logic.)*
* **The `match await(..)` hazard is related, not fixed.** An `await` in a branch is already refused (`if-effect-in-expression`);
  the existing `match await(..)` that emits a bad IIFE is a separate bug and is not touched here.
* **Short-circuit makes evaluation order part of the language.** It must be specified in `semantics.md`, together with the
  division disagreement of 3.1 (replay faults, JS answers `Infinity`).
* **A Bool with three values.** There is none: `Bool` is `true`/`false`; there is no `null` to fall through (the emitted JS never
  sees `undefined` for a declared `Bool`, except the known "state initializer that calls a function is `undefined`" hazard, which
  must be fixed or refused before `!x` can be trusted on such a field).

## 5. What this does not do, and the questions (decided: see the end of this section)

It does not give a local `let`, derived state, or a handler calling another. The repeated `shown = ..` / `paged = ..` lines
(section 1, last row) are cured by **derived state** (`derive shown = ..`, recomputed when its inputs change), not by
conditionals; it is the larger share of ops-console's repetition and the bigger change (a dependency order between fields, a
replay rule for when a derivation runs). It needs its own design and should be decided in that order: conditionals first
(small, local), derive second (large, global).

Open questions:

1. **A, with step 1 then step 2?** (Recommended.) Or C as a stopgap if A is deferred?
2. **Is a statement-level `if` around an effect ever wanted?** (Proposed: no. It changes what a trace's `issued` lists depend on.)
   **Revised 2026-10-22: yes** (#93; the evidence is `examples/tickets`, where the orders were requested after a failed sign-in, `docs/handler-sequences.md` section 1). The claim in the parenthesis does not hold: `issued` depends on the state, which replay computes.
3. **`else if`:** sugar for a nested `if` in the `else` branch, or required to be written as `else { if .. }`? (Proposed: allow
   `else if`, it is only parsing.)
4. **Literal patterns (B) for `Int`/`String`:** a separate later decision once there is a program that needs them? (Proposed: yes.)
5. **Division by zero:** replay faults and the emitted JS yields `Infinity`/`NaN` (3.1). Which is the language's answer? Either
   way it is a bug to fix before step 1, and it is true today with no `if` in sight.
   **Decided, built (alpibrusl/cancho-ui, the PR that precedes the conditionals):** the page refuses, as replay does. A result
   that is not finite is a fault in both: `replay-non-finite` in `cancho-ui test`, a thrown `Error` with that message in the page, which
   stops the handler at that statement. Making replay answer `Infinity` was rejected (JSON cannot carry it; an `Int` field would hold
   a value no `Int` has; a later `==` or `sort` would depend on it). Measured by `tests/arith_corpus.json` (22 cases) against the
   Rust model (`cargo test`) and the emitted JavaScript (`scripts/check-arith.js`, Chromium); six deliberate breaks were each caught by
   at least one of them (the check that throws removed, `+` unchecked, `/` unguarded in the emitter, `*` unguarded, the model allowing a
   non-finite result, the sentinel declaration removed). One latent bug fell out: a program with no `await` did not declare
   `__cui_cancelled`, which every handler's `catch` reads, so any error a handler threw surfaced as a `ReferenceError` instead of itself.
6. **Derive:** open its design after step 1 is built, or before?

**Decided (2026-10-22), by following the recommendation written against each question** (the maintainer asked for exactly that):

1. **A, in two steps.** Step 1 is built (section 6). The statement form (step 2) is not built.
2. **No statement-level `if` around an effect.** An `if` is pure; `if-effect-in-expression` refuses an effect or an `await` in it.
   **Revised 2026-10-22 (#93):** the expression `if` is pure, as written; the statement form (`if cond { .. }` at the start of a
   statement, `docs/handler-sequences.md`) may hold effects and `await` in its branches. Its condition is still pure.
3. **`else if` is allowed**, as sugar for an `if` in the `else` branch (a parsing rule only).
4. **Literal patterns for `Int`/`String`:** a separate, later decision, once a program needs them. Not built.
5. **Division by zero:** the page refuses, as replay does (`docs/semantics.md`, "Arithmetic is finite"); built and merged before this.
6. **Derive:** designed after step 1. Not started.

## 6. What was built (step 1), measured

* **Syntax.** `&&`, `||`, `!`, `if cond { a } else { b }` and `else if`, as an expression anywhere an expression stands: a
  handler's right-hand side, an argument, a `Text` label, an interpolation, a `Button` label, an event argument. Precedence from
  low to high: `||`, `&&`, comparisons, `+ -`, `* /`, unary `- !` (a unary operator binds tighter than a comparison:
  `!a == b` is `(!a) == b`). `if` and `else` are keywords. `!=` is still one token. A lone `&` or `|` is still
  `lex-unexpected-char`.
* **Evaluation.** `&&` and `||` evaluate the right side only when the left does not decide the answer; an `if` evaluates only
  the branch it takes. Both evaluators do it (replay in `eval.rs`; the page with the host's `&&`, `||` and `?:`). Operands and
  conditions must be `Bool`: replay refuses anything else (`replay-type-error`, no JavaScript truthiness); the page's
  `&&` would answer an operand, which is why the types are checked statically as far as they can be (below).
* **Rules**, all `Static`, reported by `check`, `build` and `test` (`spec/diagnostics/README.md`):
  `if-else-required`, `if-effect-in-expression`, `condition-not-bool`, `if-branch-type-mismatch`.
  `pattern-literal-unsupported` keeps its tag; its remediation now names `if cond { a } else { b }`.
* **Typing is partial, said plainly.** The language has no general expression typing (replay refuses an ill-typed operator
  when it runs). `crates/cancho-ui-ir/src/conditions.rs` types an expression only as far as the source shows it (a literal, a
  state field's declared type, a comparison, a builtin whose answer is fixed) and calls the rest `Unknown` (an event parameter,
  a `For` or lambda binding, a `match` arm). It refuses only when a type is known and wrong, so it never refuses a right
  program; what it cannot see, replay refuses when the program runs, and the page would answer an operand.
* **Measured.** `tests/cond_corpus.json`, 54 cases (truth tables, precedence, `!` against `==`, short-circuit in both
  operators and both orders, a division by zero in the condition, in a taken branch, in a branch not taken, in an `else if`
  that is and is not reached, nesting, `if` as an operand, in a concatenation, compared, of Bools), is replayed in-process by
  `cargo test` (`crates/cancho-ui/tests/conditionals.rs`) and run in the page in Chromium by `scripts/check-conditionals.js`;
  both agree on all 54 (a division by zero is `replay-non-finite` in replay and a thrown `replay-non-finite` error in the
  page). Ten rule tests (a misuse per tag, the effect cases, the view position, the names the pass cannot see, `&`/`|`, `!=`,
  the emitted parentheses, the remediation of `pattern-literal-unsupported`, the semantic diff). Eleven deliberate breaks,
  each caught: `&&`/`||` not short-circuiting in replay, an `if` evaluating both branches, `&&` and `||` at one precedence,
  the emitter never writing the `else`, `condition-not-bool` off, `if-branch-type-mismatch` off, the effect check off, the lexer
  splitting `!=`, `else if` not parsed, `!` using JavaScript truthiness in replay (survived the first draft of the tests; a
  test per operator was added), and in the page the branches swapped (21 of 54 Chromium checks fail).
* **The ops console, rewritten with it**, its traces unchanged and passing (38 steps, 63 checks; the offline trace; `scripts/check-ops-console.js`):
  the status tone was four `join(filter(["ok"], t => starts_with(code, "2")), "") + ..` terms and is now
  `tone = if starts_with(code, "2") { "ok" } else if starts_with(code, "4") { "warn" } else if starts_with(code, "5") || starts_with(code, "x") { "error" } else { "" }`;
  the clamp after the list shrinks was `page = page * len(take(skip(shown, page * 5), 1))` and is now
  `page = if page * 5 < len(shown) { page } else { 0 }`. (`Next` and `Prev` were already `min`/`max` since #43.)
* **Corrections to the estimate of 3.5.** The checker was not where the rules went: lowering has the scopes and the IR, so a pass
  after lowering (`conditions.rs`, as `controls.rs` is) holds them. `IrExpr::If`, `IrBinOp::{And, Or}` and `IrUnOp::Not` are
  additive, the IR version did not change. A view can hold an `if` as a `Text` body (`Text { if c { "a" } else { "b" } }`).
  `eval.rs` is 1,204 lines (estimated 1,064 before), `expr.rs` 446, `lower.rs` 1,712, `parser.rs` 1,835 (the nearest to the 2,000-line ceiling), the new `conditions.rs` 331.
* **Also fixed on the way: `name-unknown` (alpibrusl/cancho-ui#55).** The pass keeps the scope (state fields, event
  parameters, `For` bindings, lambda parameters, `match` bindings), so a name that is none of them is now refused at `check`
  instead of being written as a `ReferenceError` that blanks the page. Thirteen positions are tested (an assignment, an operand, a
  builtin argument, a text interpolation, a label, an event argument, the `over:` of a `For`, a lambda body, a `match` arm, an
  `if` condition, a negation), the names a binding introduces are tested as in scope, and every example still checks.
  Seven deliberate breaks (the assignments, the text parts, the label, the event arguments, the `over:` not walked; a lambda parameter leaking out of its lambda; a `match` binding not in scope), each caught. The replay rule `replay-unknown-identifier` stays in the catalog as the
  second line of defence; no program that lowers can reach it now.
* **Not built, still true** (the statement form of `if` was built later: `docs/handler-sequences.md`): `derive`; literal patterns; `if` in a state initialiser is emitted but
  an initialiser that reads another field is the known `undefined` hazard (unchanged); an `if` whose branches have a type the
  pass calls `Unknown` is not compared.
