# Deterministic replay — `cancho-ui test`

> **Status: designed before built (Phase 3, "replayable tests").** Written
> before the code, per [CONTRIBUTING.md](../CONTRIBUTING.md) §1. Where building
> disagreed with it, the correction is made in place and listed in §11.
> Guarantee tier of everything below: **tested**. A replay is an *execution of
> a model* of the semantics in [`semantics.md`](semantics.md) (Q2, Q3, Q4, Q5,
> Q6), not a proof, and not an execution of the emitted JavaScript. §9 says
> what that does and does not buy.

```sh
cancho-ui test <file.cui> <trace.json> [--output json]
```

The question it answers: *given this exact sequence of events, responses and
clock ticks, what is the component's state after each one, and is it what the
trace says it should be?* It is the answer to the Phase 3 deliverable
"deterministic event inputs for supported event sequences" and to
[`semantics.md`](semantics.md) Q3's "same input, same output, every run", made a
checked property instead of an aspiration. Its headline use is the one the
proposal names: **the adverse-ordering test**. A slow earlier request resolving
after a fast newer one must be dropped, and the trace says so in 25 lines
(`examples/search/stale.trace.json`), and the replay fails if the runtime rule is
removed (§13).

---

## 1. What is replayed, and on what

The `.cui` file goes through the **whole** front end (lex, parse, type-check,
lower, `import` resolution) exactly as `check` does. A file that does not
compile is a refusal, never a replay. The typed IR (`cancho-ui-ir`) is then
**interpreted** by the new crate `cancho-ui-replay`: a pure Rust evaluator of
handler bodies. No browser, no JavaScript runtime, no DOM, no thread, no timer,
no socket, no I/O. `replay` is a function from `(IR, trace text)` to a report; it
cannot fail and does not read the clock, the network or the disk.

The interpreter follows the **web backend's behaviour** where the semantics
document and the emitter could differ, because the emitter is the only
implementation there is (§8 lists the places this mattered).

One component is replayed at a time, in isolation: the trace's `component`, by
default the file's last component (the one the page mounts). It has its own
state and its own handlers. Child component instances in its view are **not**
created (§9).

---

## 2. What a trace is

A trace is one JSON file: a header and an ordered list of steps.

```json
{
  "schema": "cancho-ui-trace",
  "version": "0.1.0",
  "component": "Search",
  "clock": 0,
  "steps": [
    { "event": "Change", "params": ["a"], "state": { "query": "a" } },
    { "event": "Submit",
      "state":  { "status": "loading" },
      "expect": { "outcome": "suspended",
                  "issued": [ { "request": 1, "effect": "net.search", "args": ["a"] } ],
                  "pending": [1] } },
    { "resolve": 1, "with": { "Ok": ["Ada Lovelace"] },
      "state": { "status": "done", "results": ["Ada Lovelace"] } }
  ]
}
```

| Header field | Required | Meaning |
|---|---|---|
| `schema` | yes | the string `cancho-ui-trace` |
| `version` | yes | `0.1.0`; a different major version is `trace-schema-unsupported` |
| `component` | no | the component to replay; default the file's last component |
| `clock` | no | the fake clock at the start, in milliseconds; default `0` |
| `steps` | yes | the ordered list; at least one |

### 2.1 Steps

A step has **exactly one action key**, plus optional `state`, `expect` and
`note`:

| Action | Meaning |
|---|---|
| `{"event": "Name", "params": [...]}` | the platform delivers event `Name` with these parameters. The handler runs, synchronously, to its end or to its first unresolved `await` |
| `{"resolve": N, "with": {"Ok": v}}` or `{"with": {"Err": v}}` | the scripted network answers request `N` (§4) |
| `{"advance": ms}` | the fake clock moves forward `ms` milliseconds; every `sleep` that falls due fires, in order (§5) |
| `{"dispose": true}` | the host unmounts the component: its pending requests are marked disposed (§6) |
| `{"hide": true}` / `{"show": true}` | the page becomes hidden / visible again: no `every` tick runs while it is hidden, and `show` runs one catch-up tick for each that fell due (`schedules.md` section 11). Repeating the state the page is in is `trace-bad-step` |

`params` defaults to `[]` and must match the handler's parameter count.
`note` is free text for the human; it is never read.

Steps run strictly in list order, one finishing before the next begins. That
*is* the sequential-within-a-component rule of `semantics.md` Q3; the replay has
no other ordering to offer and does not claim one (§9).

### 2.2 Values

Trace values are JSON, mapped one-to-one onto the interpreter's values. A
variant is a single-key object, which is how a `Result`, `Option` or the
`Cancelled` marker appears in a trace:

| JSON | Value |
|---|---|
| number | a number (an IEEE double: **both `Int` and `Float`** — see §8) |
| string, `true`/`false` | `String`, `Bool` |
| `[...]` | `List` |
| `{"Ok": v}`, `{"Err": v}`, `{"Some": v}` | the constructors |
| `{"None": null}`, `{"Cancelled": null}` | the payload-free variants |
| `null` | the unit value (what `sleep` resolves with: `Ok(unit)`) |

Anything else (an object with other keys, a two-key object) is
`trace-bad-value`, located at its path (`steps[3].params[0]`).

### 2.3 Expectations

- **`state`** is a partial map `field -> value`. After the step, each named
  state field must equal the value exactly (numbers compare as numbers, `3` ==
  `3.0`; lists and variants compare structurally, element by element). Naming a
  field the component does not declare is `trace-unknown-state-field`.
- **`expect`** is a map over what *happened* in the step. Its keys:

  | Key | Value after the step |
  |---|---|
  | `outcome` | `"completed"` (the handler ran to its end), `"suspended"` (it is waiting on a request), `"dropped"` (a superseded request resolved and the rest of its handler was abandoned), or `"ok"` (`advance`, `dispose`) |
  | `issued` | the requests this step started: `[{"request": N, "effect": "net.search", "args": [...]}]` (a `sleep` also has `"due": ms`) |
  | `pending` | ids of requests started and not yet resolved, ascending (superseded and disposed ones included: they have not *resolved*) |
  | `fired` | the awaits resumed by this step: `[{"request": N, "delivered": "Ok"\|"Err"\|"Cancelled"\|"dropped", "outcome": ...}]` |

  An expected value must be a **subset** of the actual one: every key of an
  expected object must be present and equal; an expected array must have the
  same length and match element by element, each as a subset. So
  `"fired": [{"request": 1, "delivered": "dropped"}]` does not have to repeat
  `outcome`.

A trace in which **no step carries a `state` or an `expect`** can never fail,
so it proves nothing and is refused as `trace-no-assertions`.

---

## 3. What replay computes

For each step: the component's **state after the step**, per the semantics of
[`semantics.md`](semantics.md):

1. **State is initialised** from the declared initial values, in declaration
   order, before step 0 (Q1). An initial value must be a literal expression
   (literals, `Ok`/`Err`/`Some`/`None`, list literals, arithmetic and `len`
   over those; a `filter`/`map` callback reads its item as an identifier, so it
   is refused too). An initial value that reads an identifier or
   calls a function is `undefined` in the emitted JavaScript; replay refuses it
   (`replay-unsupported`) rather than invent a value.
2. **An event runs its handler body** statement by statement, in order, to
   completion or to the first `await` that has no answer yet (Q3). An `Assign`
   evaluates its right-hand side **then** writes the field; reads see the writes
   of earlier statements.
3. **A write is visible at once** to the next step. Between a handler's
   suspension and its resumption, other events run and may change state; when
   the handler resumes it reads *current* state for anything it has not yet
   read, and keeps the value it already read for anything it has (§3.1).
4. **An event with no handler** is silently dropped by the language (Q2). A
   *trace* that names one is almost certainly a typo, and a test that cannot
   tell the two apart is a worse test, so replay refuses it
   (`trace-unknown-event`). This is a stricter reading than the runtime's, on
   purpose.

### 3.1 How an `await` suspends without a JavaScript runtime

The evaluator is a plain recursive function over the IR. An `await` can sit
anywhere in an expression, so the handler cannot simply be paused mid-recursion.
The interpreter uses **record and replay of the statement**: every evaluation
step that depends on something outside the expression (a read of a state field, a
`clock.now()`, the result of an `await`) is appended to a per-statement
*journal* the first time it happens. When a statement reaches an `await` with no
recorded answer, it stops with `Suspend(request)`. When the answer arrives it is
appended to the journal and the statement is evaluated again from its start,
**consuming** the journal instead of re-reading the world: the state read before
the `await` returns the value it had then, and `clock.now()` the instant it had
then, not now. The statement is otherwise pure, so this is exactly a
continuation, and it matches JavaScript's left-to-right evaluation (`a + await(f())`
reads `a` first). A test pins it (`tests/replay.rs`,
`state_read_before_an_await_keeps_the_value_it_had`).

### 3.2 What an expression means

The operators follow the **emitted JavaScript**, restricted to the cases where
that is well-defined; everything else is a refusal, not a guess:

- `+`: number + number, or string + (string, number or bool) in either order
  (concatenation, numbers printed as JavaScript prints them). Other operand
  types: `replay-type-error`. `- * /` and unary `-`: numbers only.
- `/` is **real division**, as in JavaScript (`7 / 2` is `3.5`), whatever the
  static type says. Division by zero and any result that is not finite are
  `replay-non-finite`, because JSON cannot carry them and JavaScript's
  `Infinity`/`NaN` are not part of the language.
- `==`/`!=` (strict `===`/`!==`) on numbers, strings and bools. On lists and
  variants JavaScript compares *references*; replay does not model references and
  refuses (`replay-type-error`) rather than say `false`.
- `< <= > >=`: number/number or string/string.
- `len(x)`: elements of a list; UTF-16 code units of a string (JavaScript's
  `.length`).
- `filter`/`map`: over a list; the predicate's value uses JavaScript truthiness
  for bools, numbers and strings.
- `match`: arms in order, the first that matches wins. A `Variant` pattern binds
  the payload of `Ok`, `Err` or `Some`. An identifier pattern matches anything
  and **binds nothing** (the emitter binds nothing). No arm matching is
  `replay-no-match-arm` — the emitted JavaScript throws there — and the checker
  already refuses a non-exhaustive match on the cases it can see.

---

## 4. The scripted network: `net.search`

`net.search(q)` is a request, not a call that returns. In the browser its answer
arrives after a delay, and *when* it arrives relative to other requests is the
whole stale-response hazard (proposal §2; `semantics.md` Q4). Replay therefore
does **not** model the delay. The trace owns the order of arrival, exactly:

- Starting a request assigns it the next id (1, 2, 3, ... in the order started,
  across the whole replay) and records `{effect: "net.search", args: [q]}`. The
  step's `issued` says which id it got.
- A `{"resolve": N, "with": {...}}` step is the network answering request `N`
  *now*, with `Ok(v)` or `Err(v)`. Nothing else makes a `net.search` request
  resolve, and the corpus-filtering stub in the emitted adapter is **not**
  executed: the response is whatever the trace says. That is what makes a replay
  deterministic and lets a trace choose the adverse order.
- Resolving an id that was never issued is `trace-unknown-request`; one that has
  already resolved, `trace-request-not-pending`; a `sleep` request,
  `trace-request-not-scriptable` (timers belong to the clock, §5); a `with` that
  is not `{"Ok": v}` or `{"Err": v}` is `trace-bad-response`. `Cancelled` cannot
  be scripted: only disposal produces it (§6), as in the runtime.

### 4.1 Supersede and stale responses (`semantics.md` Q4, Q5)

Every `await` is a cancellation-tracked call in the *group* of its effect
(`net.search` -> `net_search`, `sleep` -> `sleep`), one table per component
instance. Replay implements the runtime's rule verbatim:

1. Starting a request **supersedes** the group's still-pending request.
2. When a request resolves, in this order: if it was superseded, the rest of its
   handler is **dropped**: no further statement runs and nothing is written
   (`delivered: "dropped"`); else if it was disposed, the `await` yields
   `Cancelled` and the handler continues; else the `await` yields the scripted
   value and the group's slot is cleared.
3. A superseded request is no longer in the group table, so a later `dispose` does
   not touch it: superseded-then-disposed still drops. The reverse (disposed, then
   superseded) cannot be built from a trace: after `dispose` no event can start a
   request, and only one request per group was current to be disposed. So the
   order of rule 2's first two tests is unobservable; replay uses the emitter's.

A request that is superseded is **still pending** until the trace resolves it:
the browser's request is still in flight, and resolving it is the adverse event
the test exists to exercise.

---

## 5. The fake clock

The clock is a number of milliseconds the trace controls. Nothing reads the host
clock.

- `clock.now()` returns the fake clock. The header `clock` sets its start
  (default `0`).
- `{"advance": ms}` moves it forward (`ms` a non-negative number, else
  `trace-bad-value`). `sleep(ms)` is a request due at `clock + ms`; it
  resolves **only** by the clock reaching its due time. During an `advance`,
  due sleeps fire in order of `(due, id)`; as each fires the clock reads its due
  time, so a handler that calls `clock.now()` after its `sleep` sees the moment
  the timer fired, not the end of the advance; a `sleep` started by a resumed
  handler that also falls inside the advance fires in the same step. The clock
  ends at `old + ms`. It never moves backwards.
- `sleep` resolves with `Ok(unit)`; it cannot fail, as in the adapter.
- Schedules ([`schedules.md`](schedules.md)): every `start` event runs once before the first step, in written order,
  and is reported with that step. An `advance` fires each `every` tick whose time falls inside it, in time order and,
  at one instant, in the order the `every` lines are written, before a `sleep` due at that instant. A tick is skipped
  (`"ran": false`) while a run of its event is in flight. The step report carries `ticks: [{event, at, ran}]` (absent
  when none); `expect.ticks` is compared as a subset, and absent means `[]`.

---

## 6. Disposal and `Cancelled`

`{"dispose": true}` is the host unmounting the component (`__cui_dispose`, Tier 17).
Every pending, not-yet-superseded request is marked disposed and the group table
is emptied. The pending `await` **does not resume at dispose time**: it resumes
when the underlying request resolves (a scripted `resolve` for `net.search`, the
clock for `sleep`), at which point it yields `Cancelled`, regardless of any value
the trace supplies, and the handler's mandatory `Cancelled` arm runs and the
handler continues. This corrects the shorthand in the Tier 17 text ("its pending
await resolves `Cancelled`"): in the emitted runtime the `Cancelled` arrives with
the underlying resolution (§11, 1).

After a dispose, delivering an event is `trace-event-after-dispose`: there is no
mounted view left to fire it. Resolves and advances remain legal: they are how
the disposed handlers finish.

---

## 7. Output and exit codes

Human form (stdout), from `examples/search/stale.trace.json`:

```
cancho-ui test examples/search/search.cui examples/search/stale.trace.json
component Search, 6 steps
  step 0  event Change("a")                  completed  query="a"
  step 1  event Submit()                     suspended  status="loading"  issued #1 net.search("a")
  step 2  event Change("ad")                 completed  query="ad"
  step 3  event Submit()                     suspended  status="loading"  issued #2 net.search("ad")
  step 4  resolve #2                         completed  r={"Ok":[...]}  status="done"  results=[...]  #2 -> Ok
  step 5  resolve #1                         dropped    #1 -> dropped
PASS  6 steps, 21 checks
```

(The `r=` and `results=` values are elided here; the command prints them.) Each
line shows the fields the step wrote, the requests it started, and what each
resumed `await` received.

`--output json` prints one object (the shape is part of the contract; keys are
sorted):

```json
{
  "schema": "cancho-ui-test", "version": "0.1.0",
  "file": "...", "trace": "...", "component": "Search",
  "result": "pass",
  "steps_total": 6, "steps_run": 6, "checks": 21,
  "failures": [],
  "steps": [ { "step": 5, "action": "resolve", "detail": 1, "outcome": "dropped",
               "state": { "...": "..." }, "issued": [], "pending": [],
               "fired": [ { "request": 1, "delivered": "dropped", "outcome": "dropped" } ],
               "writes": [], "clock": 0 } ],
  "requests": [ { "request": 1, "effect": "net.search", "args": ["a"],
                  "status": "resolved", "superseded": true, "disposed": false } ],
  "final_state": { "...": "..." },
  "not_claimed": [ "emitted_javascript_behaviour", "view_rendering", "..." ]
}
```

`detail` is `{"name", "params"}` for an event, the request id for a resolve, the
milliseconds for an advance. `writes` lists each state assignment the step made,
with its source span. `checks` counts the individual `state` and `expect`
comparisons made.

A failure is

```json
{ "rule": "test-state-mismatch", "tier": "tested", "step": 5,
  "path": "steps[5].state.results", "message": "...",
  "expected": ["Ada Lovelace"], "actual": ["Ada Lovelace", "Grace Hopper"],
  "span": { "file": "...", "start": {"line": 36, "column": 8}, "end": {"line": 36, "column": 40} } }
```

`path` locates the assertion in the trace (JSON syntax errors carry the parser's
`line`/`column` instead, since the JSON value tree has no source positions);
`span` locates the **source** that produced the actual value: for a `state`
mismatch the assignment that last wrote the field during this step, else the
field's declaration; for an `expect` mismatch the handler that ran or resumed;
for a runtime refusal the expression that was refused. It is `null` for trace
errors, which are located by `path` instead. A span names a position in the
`.cui` file given on the command line; a component pulled in by `import` carries
the position in its own file, which the span cannot name (the IR has no file on
its spans).

| Exit | Meaning |
|---|---|
| `0` | every step ran and every expectation held |
| `1` | the `.cui` file did not compile; the located diagnostic is printed as for `check` (nothing was replayed) |
| `2` | command line wrong (not exactly a file and a trace, unknown flag or `--output` value) |
| `3` | environment failed: a file is missing or unreadable |
| `4` | **a test failed**: an expectation did not hold, the trace is malformed or names something that does not exist, or the program did something replay refuses. The report says which, by `rule` |

`4` for a malformed trace is deliberate: a trace is the test, and a test that
cannot run has failed. `1` stays reserved for the *program* not compiling.
Expectation failures do not stop the replay (the interpreter's state is still the
truth, so later steps still mean something) and are all reported. A trace error or
a runtime refusal **stops** it: there is no defined state to continue from.

### 7.1 Rule tags

Every refusal has a stable tag. Tier **tested** throughout.

| Rule | Meaning | Stops? |
|---|---|---|
| `test-state-mismatch` | a `state` expectation did not hold | no |
| `test-expect-mismatch` | an `expect` expectation did not hold | no |
| `trace-no-assertions` | no step carries `state` or `expect` | n/a |
| `trace-malformed` | not JSON, or the wrong shape (a string where a list belongs, missing `steps`) | yes |
| `trace-schema-unsupported` | `schema` or `version` is not one this build reads | yes |
| `trace-unknown-key` | a key that is not part of the format, at the header, a step or `expect` | yes |
| `trace-bad-step` | zero or several action keys, or a bad action value | yes |
| `trace-bad-value` | a JSON value with no interpreter value, a negative `advance`, ... | yes |
| `trace-unknown-component` | `component` names no component in the file | yes |
| `trace-unknown-event` | the component has no handler by that name | yes |
| `trace-params-arity` | `params` length differs from the handler's | yes |
| `trace-unknown-state-field` | a `state` expectation names an undeclared field | yes |
| `trace-unknown-request` | `resolve` of an id never issued | yes |
| `trace-request-not-pending` | `resolve` of a request that already resolved | yes |
| `trace-request-not-scriptable` | `resolve` of a `sleep` | yes |
| `trace-bad-response` | `with` is not `{"Ok": v}` or `{"Err": v}` | yes |
| `trace-event-after-dispose` | an event after `dispose` | yes |
| `replay-unsupported` | a construct outside the replayed subset (§9) | yes |
| `replay-tick-limit` | one `advance` would fire more than 10,000 ticks | yes |
| `replay-type-error` | an operator or builtin applied to values it is not defined on | yes |
| `replay-unknown-identifier` | an identifier that is neither a local nor a state field | yes |
| `replay-no-match-arm` | a `match` fell through | yes |
| `replay-non-finite` | arithmetic produced a value that is not a finite number | yes |
| `replay-number-format` | a number JavaScript would print in exponent form, in a string concatenation | yes |

These are appended to the catalog in
[`spec/diagnostics/README.md`](../spec/diagnostics/README.md) §3.2.

---

## 8. Where the semantics document and the emitter differ, and what replay does

- **Numbers.** The static types are `Int` and `Float`; the emitted code is
  JavaScript, where both are doubles and `/` is real division. The comment in
  `examples/clock/clock.cui` says elapsed seconds are "integer-truncated"; the
  emitted JavaScript does not truncate (`1500 / 1000` is `1.5`). Replay follows the
  JavaScript: one number type (a double), real division. The comment is corrected
  in place (§11, 2).
- **`Cancelled` timing.** `semantics.md` Q5 says a disposed component's request is
  cancelled and the handler receives `Cancelled`; Tier 17 says the pending await
  "resolves" `Cancelled`. The emitted runtime delivers it when the underlying
  request completes. Replay follows the runtime (§6).
- **Identifier patterns** bind nothing (§3.2).
- **Unknown events** are dropped by the runtime and refused by replay (§3, 4).

---

## 9. What it deliberately does not claim

Every report carries these in `not_claimed`, so a `pass` is never read as a claim
about them.

- **Not the emitted JavaScript.** Replay is a second implementation of the
  semantics. A pass means *the interpreter and the trace agree*; it does not mean
  the JavaScript behaves the same. The two are tied together only by the examples
  and by review. A differential test that runs the emitted code under a JavaScript
  engine against the same trace would close that gap, and needs a JS runtime,
  which this slice refuses to depend on (§12).
- **No view.** Text interpolation, `For`, attributes, accessibility metadata, focus
  and the DOM are not rendered or checked. `Text { "Elapsed: {clock.now()...}" }` is
  never evaluated; only handlers are. (`not_claimed: view_rendering`.)
- **No timing.** There is no wall clock and no latency model. `net.search`'s
  `60 + 45 * len(q)` delay in the adapter is not modelled; ordering is whatever the
  trace scripts. "A slow request" in a trace is a request the trace resolves
  *later*. Whether the real adapter's delays produce that order is a fact about the
  adapter, not about replay. (`timing`.)
- **No real network.** `net.search` is scripted (§4); the adapter's corpus, its
  filtering and its failure behaviour are not executed. (`real_network`.)
- **Async is the modelled subset only:** `await` of `net.search(..)` or `sleep(..)`.
  An `await` of anything else is `replay-unsupported`. There is no
  `Promise`, no microtask ordering and no interleaving *inside* a synchronous
  segment: an event handler runs atomically up to its first unresolved `await`,
  as in single-threaded JavaScript. (`javascript_event_loop`.)
- **No child instances.** A component is replayed alone. Its children are not
  created, their properties are not passed, a parent's re-render does not reach
  them, and a slot's disposal is not triggered by a list shrinking. To test a
  child, name it as the `component`. The `For`-driven disposal cascade of Tier 17 is
  therefore replayed only through an explicit `dispose` step.
  (`child_component_instances`, `cross_component_ordering`.)
- **No properties.** State starts from the declared initial values; the initial
  value a parent would pass cannot be supplied. (`properties`.)
- **No foreign code.** Nothing here crosses a `foreign` boundary.
- **Not a proof, and not exhaustive.** A trace is one path. The report counts the
  steps and the checks it ran, nothing more.

---

## 10. Surfaces

`cancho-ui test` appears in `--help`, `introspect` (with its exit codes, including
`4`), the `skill` text, the README command list and
[CONTRIBUTING](../CONTRIBUTING.md) §5. The example traces live next to their
programs: every `examples/<name>/*.trace.json` is replayed against
`examples/<name>/<name>.cui` by `tests/replay.rs`, and a `.cui` and trace pair is
a regression test the CI gate runs.

---

## 11. Corrections made while building

1. **`Cancelled` is delivered when the request completes, not when the component is
   disposed.** The Tier 17 text in `AGENTS.md` §3 and the emitter's comments say the
   pending await "resolves" `Cancelled` on disposal. Reading `__cui_call`: dispose only
   sets `token.disposed`; the `.then` that returns `Cancelled` runs when the
   underlying promise settles (400 ms later for `sleep(400)`). Replay follows the
   code (§6). `AGENTS.md` is corrected in place.
2. **`examples/clock/clock.cui` said elapsed seconds are "integer-truncated".** The
   emitted JavaScript divides with `/` and does not truncate (§8). The comment is
   corrected in place; the program is unchanged.
3. **Concurrent timers in one component are impossible.** The first draft of §5
   described due sleeps racing; building the tests showed that every `sleep` shares
   one group, so starting a second supersedes the first, and the earlier one is
   dropped when it fires. The ordering rule is
   still needed (a chain of sleeps started inside one advance, and a superseded
   timer firing before its successor) and is tested, but a trace cannot make two
   live timers race. `sleeps_in_one_component_share_a_group...` pins this.
4. **A trace with no assertion is refused** (`trace-no-assertions`). Not in the first
   draft; added after asking what a `pass` on `{"steps": [{"event": "Increment"}]}`
   would prove (nothing).
5. **Two replayed rules are unobservable and so untested** (§4.1, rule 3: disposed
   vs. superseded order; the `inflight` slot comparison on resolve). They mirror the
   emitter and are listed as surviving mutants in §13, with the reason.
6. **The step report grew `writes`, `clock` and `detail`.** §7 was written first
   without them; they are needed to print the human form and to locate a failure.

---

## 12. Alternatives considered

- **Run the emitted JavaScript in an embedded engine** (QuickJS, boa). Rejected
  for this slice: the hard rule is pure Rust with no new C/FFI dependency, and a
  pure-Rust engine is a large dependency whose own semantics become part of the
  claim. The cost is §9's first bullet.
- **Resolve `net.search` by running the adapter's delay model on the fake clock.**
  Rejected: it turns "a slow request" into an arithmetic fact about a stub, and
  the test would break when the stub's constants change. The trace states the
  order it means.
- **Continuations by a CPS evaluator or a bytecode VM** instead of the journal
  (§3.1). Either works; the journal reuses one straightforward recursive
  evaluator and is a few dozen lines.

---

## 13. Measured

Everything below was run on this commit; nothing is estimated.

- **Example traces:** 9 traces over 7 programs (`counter`, `clock`, `await`,
  `cancel` x2, `list`, `match`, `search` x2): 42 steps, 87 checks, all pass;
  the whole set replays in 0.06 s wall clock, including nine process starts
  (release build).
- **Tests:** 150 pass under `cargo test --workspace` (109 on `origin/main`; the README
  said 108). New: 35 in `tests/replay.rs`, 2 replay arms in `tests/no_panic.rs`,
  4 in `tests/cli.rs` (the `skill`/`introspect` argument fix).
- **No panic:** `replay` ran on 17,161 mutated traces (every prefix, every
  one-character deletion, and every JSON node replaced by each of 13 awkward values,
  of the nine example traces; 32,555 steps reached the interpreter), and on 15,486
  compiling mutants of the example programs with their own traces. None panicked, and
  every report was self-consistent (`result`, `steps_run`, serialisable, renderable).
- **A test that cannot fail proves nothing:** the interpreter was mutated 28 times,
  one change at a time, and `tests/replay.rs` run against each. **26 mutants were
  killed, 2 survived, and both survivors are equivalent mutants** (unobservable by
  any trace, §4.1 rule 3 and §11, 5).

  | # | Mutation | Result |
  |---|---|---|
  | 01 | statements run in reverse order | killed (16 tests) |
  | 02 | starting a request does not supersede the pending one | killed (5) |
  | 03 | ignore the `Cancelled` arm: a disposed request delivers its answer | killed (3) |
  | 04 | disposed tested before superseded | **survived** (equivalent, §4.1) |
  | 05 | state reads not journaled | killed (1) |
  | 06 | `clock.now()` not journaled | killed (1) |
  | 07 | timers do not set the clock to their due time | killed (3) |
  | 08 | due timers fire in id order | killed (1) |
  | 09 | `sleep` due ignores the clock at issue | killed (1) |
  | 10 | `/` truncates | killed (1) |
  | 11 | a variant pattern ignores its tag | killed (5) |
  | 12 | `>` behaves as `>=` | killed (1) |
  | 13 | `sleep` resolves with `Err` | killed (3) |
  | 14 | `pending` lists resolved requests | killed (6) |
  | 15 | event parameters bind in reverse | killed (1) |
  | 16 | `expect` lists match by prefix | killed (1) |
  | 17 | `len` counts characters, not UTF-16 units | killed (1) |
  | 18 | `filter` keeps what the predicate rejects | killed (1) |
  | 19 | `advance` does not move the clock | killed (3) |
  | 20 | a dropped request still runs the rest of its handler | killed (5) |
  | 21 | numeric assignments write 0 | killed (19) |
  | 22 | `trace-no-assertions` removed | killed (1) |
  | 23 | `state` expectations compare nothing | killed (4) |
  | 24 | the group slot is cleared by any resolve | **survived** (equivalent, §11, 5) |
  | 25 | `+` swaps its string operands | killed (1) |
  | 26 | `-` swaps its operands | killed (3) |
  | 27 | an `Err` answer is reported as `Ok` | killed (1) |
  | 28 | the journal is cleared when an answer arrives | killed (10) |

  The headline: with mutation 02 applied, `cancho-ui test examples/search/search.cui
  examples/search/stale.trace.json` exits `4` with four located failures at step 5
  (the stale response overwrote `results`, `outcome` was `completed`, not `dropped`).
  Mutations 06 and 15 survived the first version of the suite; the tests
  `clock_read_before_an_await_keeps_the_instant_it_had` and
  `parameters_bind_in_declaration_order` were added for them.
