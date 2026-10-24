# Nesting: one bound on expressions, view nodes and types (issue #99)

> **Status: BUILT.** Written 2026-10-24 ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1); numbers measured with `scripts/measure-nesting.js` on main at
> c18158e (the "before" column, from a build of that commit) and on the fix (the "after" line). Anything not measured says **assumption**.

## 1. What was wrong

`if` statements had a nesting limit of their own (`MAX_STMT_DEPTH`, 32, `parse-nesting-too-deep`, #93). Expressions, view nodes and types had none, and so
a program of a few thousand characters aborted the compiler: `thread 'main' has overflowed its stack`, `fatal runtime error: stack overflow, aborting`, exit 134.
That is a signal, not a panic: `catch_unwind` cannot see it and the mutation-based `crates/cancho-ui/tests/no_panic.rs` cannot reach it.

## 2. Measured

The smallest nesting at which a pass aborts, for each construct (the smallest of `check`, `build` and `test` (replay), found by bisection between 1 and 200,000;
a program of one handler `a = (((..1..)))` or one view `Column { Column { .. } }`; the stack is `ulimit -s`: 8 MiB is the main thread of the command
line, **2 MiB is the default stack of a thread a library user spawns**):

| construct | debug, 8 MiB | debug, 2 MiB | release, 8 MiB | release, 2 MiB |
|---|---|---|---|---|
| paren | 738 | 181 | 7576 | 1877 |
| unary | 323 | 79 | 5449 | 1351 |
| add-chain | 323 | 79 | 5445 | 1350 |
| and-chain | 323 | 79 | 5445 | 1350 |
| call | 196 | 49 | 3170 | 787 |
| list-literal | 324 | 80 | 5446 | 1350 |
| match-arm | 323 | 79 | 5445 | 1350 |
| if-else-chain | 323 | 79 | 5447 | 1350 |
| lambda | 312 | 76 | 4751 | 1180 |
| type | 1460 | 359 | 18671 | 4625 |
| interpolation | 737 | 180 | 7575 | 1876 |
| view-column | 308 | 76 | 3035 | 1337 |
| view-if | 308 | 76 | 3187 | 790 |
| view-for | 308 | 76 | 2937 | 728 |

* **Every construct aborted**, each at a different depth. The same recursion is in the parser, the lowering, the checks, the emitter and replay, so the first pass
  to overflow sets the number: replay for a nest of calls (49 on a 2 MiB debug thread), the emitter for nested views (76), the parser or lowering for the rest.
* **A chain is as deep as a parenthesis.** `1 + 1 + .. + 1` does not recurse in the parser (a loop folds the left side), but the tree it builds is as deep as it
  is long, and every later pass recurses along it. 79 `+` aborted a 2 MiB debug thread.
* **Time, not only stack.** The emitter's time for nested `If` views on the unfixed release build: 250 deep **0.05 s**, 500 **0.41 s**, 1,000 **5.8 s**, 2,000
  **56 s** (about ten times for each doubling). A deep view was a way to stall a build before it was a way to crash it.
* **A deep trace is not the same problem.** A trace is JSON, read by `serde_json`, whose own limit (128 levels) answers `trace-malformed` (`not valid JSON:
  recursion limit exceeded`): measured with 100,000 nested arrays in a `state`, no abort.

## 3. The decision

* **One bound, `MAX_NEST_DEPTH` = 32, in the parser**, counted over expressions, view nodes and types together, the way `MAX_STMT_DEPTH` counts statements.
  A deeper input is `parse-nesting-too-deep` with the position of the first token past the bound, and the message says what nested ("expressions, view nodes
  and types" or "statements"). Nothing is rebuilt with a bigger stack: the stack is not the contract.
* **What counts.** Each operand a parser function opens: a parenthesis, a unary minus, a call argument, a list element, a `match` scrutinee or arm body, an
  `if` expression and each `else if`, a lambda body, an interpolation (counted through the nested parser, which used to start from zero), a view node
  inside a view node (`Column`, `If`, `For`, any element), a type argument (`List[List[..]]`). **A chain of binary operators counts one for each operator**:
  `a + b + c` is two. A handler's expression starts at 1, so 31 parentheses are allowed and 32 are not.
* **Why 32.** It is the number statements already use. On a 2 MiB debug stack, the worst construct measured is a nest of calls in replay, aborting at 49: a
  bound of 32 leaves a factor of 1.5 there, and 2 or more for every other construct; in the release build, with 8 MiB, the margin is 25 times or more. A
  program at the limit of each construct was run through `check`, `build` and `test` on a 2 MiB stack (`crates/cancho-ui/tests/nesting.rs`), and so were
  two mixtures of the worst kind (30 nested `if` statements around a call 30 deep; the limit of a view around an interpolation), all without an abort. On a
  **1 MiB** stack two of those still abort (a nest of calls in replay, and `else if` chains in the check): the margin to a stack half the default is not
  there, and is not claimed.
* **Real programs are far from it.** Every `examples/*/*.cui` still checks (a test), and none was changed; the deepest view in them is a few levels. A string
  built from more than 32 pieces with `+` is refused: write it as one interpolated string or in two statements. That is the cost.
* **Not done:** a bound on the **size** of a program (a flat list of a million elements, 100,000 handlers) is not a nesting limit and is not measured here.
  Blocks (`run`) and imports recurse after parsing and have their own bounds: section 5.

## 4. Checked

* `crates/cancho-ui/tests/nesting.rs` (6 tests, in the gate): for each of 15 constructs, and for chains of blocks and of imports, the program at the limit is not refused and does not abort (`check`, and where the
  program is valid `build` and `test`), run in a child process on a **2 MiB** stack; one nesting past the limit, and at 100, 1,000 and 100,000, is exactly
  `["parse-nesting-too-deep"]` with exit 1; the two reproductions of the report (parentheses and `Column`s at 1,000, 2,000, 5,000 and 200,000) are refused; no
  example is refused.
* `scripts/measure-nesting.js` after the change: **no construct aborts at 200,000, in any pass, on 8 MiB or 2 MiB, debug or release** (the "before" table is
  the same script on the unfixed build).
* `scripts/breaks/nesting.json`: 11 breaks, **all caught by `cargo test` itself** (no kill, no `HUNG`): expressions, chains, view elements, types, `else if` not counted; the limit raised to 1000; an interpolation counting from zero again; a refusal inside an interpolation turning into another tag; the expansion depth, the cycle walk and the import chain not bounded. **One break was not caught the first time**: removing only the depth check of the expansion left the chain bound of the cycle walk to refuse every chain I had tried (the same tag), so the test missed it. A case that only the expansion check refuses (20 blocks with an `if` around each `run`, 40 deep once expanded) was added and the break re-run alone: caught.

## 5. Found while building it

* **`run` is replaced after parsing, so the parser's bound does not see it.** 2,000 blocks that each run the next overflowed a 2 MiB thread (and 2,000 that
  each run the next inside an `if`). `block-expansion-too-deep` (new tag, `spec/diagnostics/README.md`): blocks that run blocks and the `if`s inside them
  may nest **32** deep once expanded (the cycle walk is bounded by the same number, since it recurses along the chain too). 31 chained blocks still check.
* **An import chain recurses too.** 2,000 files that each import the next overflowed a 2 MiB thread. `import-chain-too-deep` (new tag): a chain of imports is
  at most **32** files long; 30 files check, build and run on a 2 MiB stack (`crates/cancho-ui/tests/nesting.rs`).
* **Components that use components are not a recursion of the compiler.** 2,000 components in one file, each using the next, check, build and replay on a 2 MiB
  stack with no abort (measured). The page's own JavaScript stack for that many nested instances is **not measured** (assumption: the browser's recursion limit,
  which is the emitted page's, not the compiler's).
* **The message now says what nested** (`TooDeep { what, .. }`): "statements" for `if` statements as before, "expressions, view nodes and types" for the new bound.
* **The parser used to hide a refusal inside an interpolation.** The sub-parser's error was turned into `parse-invalid-number` with the error's debug text; a
  nesting refusal now passes through as it is.
