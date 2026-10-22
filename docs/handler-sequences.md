# Handlers that branch and share: statement `if`, `block` and `run`

> **Status: BUILT** (alpibrusl/cancho-ui#93). Found building `examples/tickets` (`docs/tickets.md` section 3). The design came
> first; section 8 says what was built and what it measured. This document revises one decision of `docs/conditionals.md`
> (section 2), in place there, with the evidence below.

## 1. What was measured before the change

* **A handler cannot start another.** `on Go { n = 1; Next; }` with `on Next { n = 2; }` is
  `error[name-unknown]: Next is neither a state field nor a binding in scope here`.
* **A request cannot sit inside an `if`.** `on Go { if ok { r = await(net.get("a")); } }` is
  `error[parse-unexpected-token]: expected `}`, found "="`: `if` is an expression of values (`docs/conditionals.md`), and an
  effect in one is `if-effect-in-expression`. The remedy that tag gave, "write two events and let the view pick the button",
  does not apply here: the choice is made *inside* the handler, after an answer that only exists once the first request has
  returned.
* **`examples/tickets` shows both costs** (`SignIn`, `Save`):
  * `SignIn` posts the credentials, and then always asks for the orders. Replay says so
    (`cancho-ui test examples/tickets/tickets.cui examples/tickets/tickets.trace.json`, steps 5 to 7): after the `401` of a wrong
    password the next thing issued is `#2 net.get("api/orders")`, which the service refuses with another `401` and the program
    drops.
  * The statements that read the orders and keep the non-empty lines (`r = await(net.get("api/orders"));` then
    `orders = match r { .. }`, and in `SignIn` `note = match r { .. }`) are written twice, in `SignIn` and in `Save`. A change to
    the shape of a record is two edits, and the two copies already differ (`Err(_) => { [] }` against `Err(_) => { orders }`).
  * `start Refresh;` and `every 5000 Refresh;` run an event, but only at mount or on a period, not after something happened.

## 2. The options

| | Option | Fixes | Cost to the replay model | Against |
|---|---|---|---|---|
| a | **`then Event;`**: an event runs another after the current one | the repetition | a queue of events, run when a handler ends; the trace has to say in which step the chained handler's requests are issued; a handler that is dropped (superseded) must not run its `then`; a loop (`A` then `B` then `A`) needs an `event-cycle` rule, but a guarded loop is legal, so the rule must be conservative | does not make a request conditional; needs (c) as well; a new way for events to run, with its own ordering |
| b | **`block Name { .. }` and `run Name;`**: a named sequence of statements, written once and run from several handlers | the repetition | none: `run Name;` is replaced by the block's statements when the program is lowered, so the IR, replay and the page see the handler as if it had been written out | a block has no parameters; it reads and writes state |
| c | **A statement `if cond { .. } else { .. }` whose branches may hold effects and `await`** | the conditional request | the position of a suspended handler is a path through nested statements, not one index; the commit points of `docs/semantics.md` ("A handler that faults") are still the statements that awaited | revises `docs/conditionals.md` decision 2 |
| d | `await` inside an expression `if` | the conditional request | an expression that suspends in one branch has no value to give in the other; the "hard part" named in the issue | |
| e | Do nothing | | | the example keeps its request after a failed sign-in |

**Chosen: b and c.** a was rejected: it adds a second scheduler for events and still needs c. d was rejected for the reason in the
table: it is what the issue called the hard part, and c has no such problem because a statement has no value.

## 3. The revision of `docs/conditionals.md`

`docs/conditionals.md` section 3.1 refused an effect inside `if` because "the set of issued effects would depend on a value the
replay trace has to supply". The claim is corrected there, in place. What a trace supplies is the *answer* to a request that was
issued; whether the request is issued is decided by the handler from the state, which replay computes exactly as the page does.
The trace's `issued` list is something replay *derives* and the step *asserts*. The condition of a statement `if` is still pure (no
effect, no `await`: `if-effect-in-expression`), so nothing ever suspends before a branch is chosen. Section 5, question 2, and the
decisions list are corrected to say: no `if` *expression* around an effect, as before (`if-effect-in-expression`); a statement `if`
may hold effects in its branches.

## 4. The forms and their rules

```
on SignIn {
    ...
    if signed {
        run LoadOrders;
    }
}

block LoadOrders {
    r = await(net.get("api/orders"));
    orders = match r { Ok(text) => { filter(lines(text), x => len(json.get(x, "id", "")) > 0) } Err(_) => { orders } Cancelled => { orders } };
}
```

* **Statement `if`.** `if cond { stmts }`, `if cond { stmts } else { stmts }` and `else if`. `else` may be omitted (a statement
  has no value to give on the other side). The condition is a pure `Bool` expression: `condition-not-bool` (the tag of the
  expression form, with the message naming "the condition of an `if` statement"), `name-unknown` for a name not in scope,
  `if-effect-in-expression` for an effect or an `await` in it. A statement at the start of a statement position that begins with
  `if` is this form; `x = if c { 1 } else { 2 };` is still the expression.
* **Branches** hold what a handler holds: assignments, expression statements, `run`, and further `if`s. An effect in a branch is
  an effect of the component as any other (capabilities are checked, `capability-unused` counts it).
* **`block Name { stmts }`** is a component item, like `on`. **`run Name;`** is a statement in a handler, a block or a branch.
  `block` and `run` are words only at those two places: a state field may still be called `block` or `run`.
* **No parameters.** A block names no parameters and is checked on its own: a name in it is a state field or a binding of the block
  itself, never a parameter of the handler that happens to run it (`name-unknown`).
* **Rules** (`spec/diagnostics/README.md`): `block-unknown` (a `run` of a block the component does not declare),
  `block-duplicate` (two blocks of one name), `block-name-conflict` (a block named like a state field or an event), `block-cycle` (a
  block that runs itself, directly or through others; a block's expansion would never end).
* **Absent `else` and `run` of an empty block** are fine.

## 5. How replay and the page agree

* **A block is expanded when the program is lowered**, before any pass or evaluator sees it; replay and the page cannot disagree
  about it. The expansion keeps the spans of the block's own statements, so a fault or a write is reported where the block is
  written.
* **Page.** `if (cond) { .. } else { .. }` inside the async handler. The segment atomicity of `docs/semantics.md` ("A handler that
  faults") is kept: the snapshot of the fields the handler writes (now including those written in branches) is taken at the start
  and again after each statement that awaited, at any depth, and put back if a statement throws.
  The paint before the first await (`__render()`) is emitted before the first top-level statement that holds an await, so an `if`
  that awaits is painted before its condition is read.
* **Replay.** A suspended handler records where it is as a path through the nested statements (`Task.pc`). Entering a branch does
  not re-evaluate the condition when the handler resumes: the branch was chosen when the `if` was entered. A statement that awaited
  ends a segment (a commit point) at any depth, as in the page.
* **A handler that is superseded** (`dropped`) abandons the rest of the handler, however deep; the page's sentinel does the same.
* **The paint, corrected on the way.** The corpus measured a gap that was not about branches: the page painted what the handler had
  written only before the *first* awaiting statement, so in `SignIn` (post the login, then ask for the orders) the sign-in message
  reached the screen only when the second request came back. `docs/semantics.md` already said the state is painted when the handler
  suspends. The page now paints before every awaiting statement at any depth (except one that starts the handler); the correction
  is in place in `docs/semantics.md`.
* **Hidden hazards, said first.** The page evaluates the condition once, when the statement is reached; so does replay. A condition
  that reads a field written by another handler while this one is suspended is read at the time it is reached in both.

## 6. Cost

Measured in section 8 and said plainly there: the sharing is in the source, not in the page, because a `run` is replaced by the
block's statements; the page gets slightly larger, not smaller.

## 7. What this does not do

No `then Event;`; no parameters for blocks; no `return` or early exit (the example uses an `if`); a block is not a value; `match`
arms still hold expressions, not statements.

## 8. What was built, measured

* **Syntax.** `if cond { stmts } [else if .. | else { stmts }]` as a statement (`Stmt::If`, `IrStmt::If`), `block Name { stmts }`
  (`BlockDecl`), `run Name;` (`Stmt::Run`, gone after `blocks::expand`). `block` and `run` are words only in their two places.
  `parser.rs` was at 1,962 lines of the 2,000 ceiling, so the view half of the parser moved to `parser/view.rs` unchanged
  (1,515 and 591 lines now).
* **Rules** (`spec/diagnostics/README.md`): `block-unknown`, `block-duplicate`, `block-name-conflict`, `block-cycle`,
  `block-expansion-too-large` (10,000 statements per handler after expansion; a chain of blocks that each run the next twice is
  refused, a diamond of forty is not), `parse-nesting-too-deep` (32). `condition-not-bool`, `if-effect-in-expression` and
  `name-unknown` apply as before; a block cannot see a handler's parameters. The cycle search visits each block once, so it is not
  exponential on a diamond; `run` of an empty block counts against the limit, so nested empty blocks cannot be slow either.
* **Found on the way: alpibrusl/cancho-ui#99.** A deeply nested expression (1,000 parentheses) or view (2,000 `Column`s) aborts the
  compiler with a stack overflow; only statements got a limit here.
* **Replay.** `Task.pc` is a path (`[i]`, `[i, branch, j]`); the branch is chosen once; a statement that awaited commits at any depth.
* **Measured.** `tests/stmt_if_corpus.json`, 7 cases (a conditional request; a second request that depends on the first answer; a
  block run by two handlers and by a block; nested ifs with an await in the inner one; a superseded request inside an `if`; a fault
  in a branch; a fault after an await in a branch), 34 steps. `cargo test` replays the file (`crates/cancho-ui/tests/handler_seq.rs`,
  with 12 tests of the rules and the emission) and `scripts/check-handler-seq.js` runs it in Chromium, comparing the URLs the page requests
  with the `issued` list replay derives, step by step. Both agree on every step.
* **Deliberate breaks**, `node scripts/deliberate-breaks.js scripts/breaks/handler-seq.json`: 14, each caught. The first run
  missed two (the commit after an await inside a branch, in replay and in the page), because no case faulted after such an await;
  the seventh case was added for them and both are caught now.
* **`examples/tickets`** reads the orders from one block (`LoadOrders`), run by `SignIn` only `if signed` and by `Save`. Its trace
  loses the request after a refused sign-in (28 steps, 72 checks) and `scripts/check-tickets.js` asserts that the service saw none
  (`server.log.orderRequests === 0`). **The size, honestly:** `tickets.cui` 8,686 to 8,696 bytes and 207 to 209 lines, the built
  page 43,748 to 44,354 bytes (+606, +1.4 %): the block's statements are emitted at both run sites and the page paints before each
  await. What was bought is one definition (the two copies already differed), and no request after a refused sign-in.
* **Not built:** `then Event;`; parameters for a block; early exit; a statement in a `match` arm.
