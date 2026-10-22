# Semantics

> **Status: Phase 0.** Every numbered question in the proposal
> §5.3 is answered below, with the **guarantee tier** labelled
> (`static | tested | runtime | trust | unknown`). Where the
> answer inherits a Cancho Core rule, the source is cited. Where
> the answer is net-new for cancho-ui, the rationale is given.
>
> A "Phase 1 enforces" line says the Phase 1 compiler must
> enforce the rule. A "Phase 2+ refines" line says the rule is
> refined by a later phase and the Phase 1 version is provisional.

---

## Guarantee tiers

The five tiers — defined fully in
[`docs/security-model.md`](security-model.md) — re-stated here
because every question below labels itself with one:

| Tier | What it means | Enforced by |
|---|---|---|
| **static** | Compiled-time guarantee under documented assumptions | Compiler / verifier |
| **tested** | Exercised by a named test or test suite | Test harness |
| **runtime** | Checked at execution time | Runtime / platform adapter |
| **trust** | External or native; not verified by cancho-ui | An external party |
| **unknown** | Not yet established | — |

Every "I will not say" line in this document is a refusal to
claim a guarantee the implementation cannot back. The same
discipline Cancho Core applies to its authority report — the
report names what is bounded, and refuses what crosses a foreign
boundary.

---

## Question 1 — Who owns each piece of state?

**Answer.** A component owns its state, the same way a Cancho
Core function owns its `res` values. State is `res` by structure: it
must be consumed (updated or disposed) on every path through the
component's lifecycle.

**Tier.** **static** (declaration) + **runtime** (enforcement).

**Source.** Cancho Core's
[`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md).
A `res` value is consumed exactly once on every path; the
compiler refuses programs that forget. The same rule applies to
UI state.

**Refusals.**

- A `state` declared but never read or updated: `state-unused`
  (the same reasoning as `capability-unused` in §3 of
  [`docs/language-design.md`](language-design.md)).
- A state read after the component has been disposed: `state-after-dispose`
  (Phase 1 forbids statically, traps at runtime).
- A state passed out of the component as an owned value: refused
  statically with `state-escapes`.

**Phase 1 enforces.** Declaration form, `state-unused` detection,
basic `state-after-dispose` static analysis. **Phase 2+ refines.**
Cross-component state sharing (via parent-owned children) and
the full lifecycle story.

---

## Question 2 — Which transitions are legal, and what happens when an event is invalid?

**Answer.** Each component declares the events it accepts (`on
Event { ... }` blocks). An event that fires for which the
component has no `on` block is **silently ignored** in Phase 1
(the event is dropped on the floor — Phase 2 may change this to
a diagnostic, but the Phase 1 contract is "drop, not crash").

A transition is legal if:

- The event is declared.
- The body can be type-checked against the component's state
  schema.
- The body does not perform an effect the component has not
  declared.

**Tier.** **static** (declarations and reachability) + **runtime**
(invalid event dispatch).

**Source.** Cancho Core's "exact effect row" rule
([`linearity-and-effects.md §3.1`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)):
a function's effect row is exact in both directions. The same
rule applies to a component's transition set.

**Refusals.**

- A transition body that performs a non-declared effect:
  `effect-not-declared`.
- A transition body that updates a state field the component
  does not own: `state-not-owned`.

**Phase 1 enforces.** Static effect reachability, basic
state-ownership. **Phase 2+ refines.** The full event ordering
semantics in §3.

---

## Question 3 — Are event handlers sequential, transactional, or otherwise ordered?

**Answer.** Within a single component, transitions run
**sequentially** — one finishes before the next begins, in the
order events fire. There is no transaction model in Phase 1.

**Tier.** **runtime** (semantics) + **tested** (determinism).

**Source.** Cancho Core's
[`defined-behaviour.md`](https://github.com/alpibrusl/cancho/blob/main/docs/defined-behaviour.md):
"left-to-right evaluation, every C hole named and closed."
The same discipline applies to event ordering.

**The determinism requirement.** A test fixture that fires
events `A`, `B`, `C` on a fixed state must produce a
**deterministic** sequence of state updates — same input,
same output, every run. The test harness in Phase 2 makes this
a checked property, not an aspirational one.

**What is not defined in Phase 1.**

- Cross-component event ordering: if two components' handlers
  fire in the same tick, the ordering is **runtime** —
  dependent on the platform event loop. Documented as
  `cross-component-ordering-undefined` in the IR.
- Priority/preemption: not in Phase 1. The Phase 1 contract is
  FIFO within a component.

**Refusals.**

- A test that asserts an ordering the language does not
  guarantee: refused as a test rule (`test-asserts-unspecified`)
  with the message naming the unspecified behavior.

**Phase 1 enforces.** Sequential semantics, deterministic
replay (built: [`docs/replay.md`](replay.md), `cancho-ui test`; it replays one
component at a time, so cross-component ordering is still not defined).
**Phase 2+ refines.** Cross-component ordering if an
asker appears.

---

## Question 4 — How are asynchronous results correlated with the request that created them?

**Answer.** Every asynchronous effect returns a **request token**
(a `Request<T>` value). The result is correlated with the
request by the language: a result delivered after the request's
component has been disposed is **dropped, not delivered**.

```text
component NearbySearch {
    requires Network

    state status: SearchStatus = Idle

    on SearchRequested(query) {
        status = Searching
        let token = net.search(query)
        on token.resolved(result) {
            status = Done(result)
        }
    }
}
```

**Tier.** **static** (request token type) + **runtime** (correlation).

**The hazard this prevents.** A search request issued for query
`"a"` resolves after a second request for `"b"` has been issued.
The `b` request's result overwrites the `a` request's — a
"stale-response" bug. The language makes the correlation
explicit; the runtime drops results for disposed requests; the
test suite exercises the cases.

**Refusals.**

- A `token.resolved` block that updates state in a component
  whose lifecycle has ended: `state-after-dispose` (Question 1).
- A request whose token is dropped before resolving:
  `token-dropped-before-resolve` — refused at compile time
  when the lifetime can be proved, trapped at runtime otherwise.

**Source.** The hazard is named in the proposal §2 and §5.3 #4.
The mechanism follows Cancho Core's linear-resource model
([`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)):
a `res` value is consumed exactly once.

**Phase 1 enforces.** Request-token type, basic correlation
between active requests and their handlers. **Phase 2+ refines.**
The full cancellation model in Question 5.

---

## Question 5 — What happens to pending work when a component is disposed or a request becomes obsolete?

**Answer.** On dispose, **every pending request is cancelled**
and its result (if it arrives) is dropped. On
request-obsoletion (e.g. a new search issued while the old is
in flight), the older request is cancelled and its result is
dropped.

**Tier.** **runtime** (enforcement) + **tested** (semantic).

**Mechanism.** A pending request is a `res` value held by the
component. Disposing the component releases the request, the same
way `release` releases a capability in Cancho Core. The
cancellation is cooperative — the request itself is told to
stop — and the result-handling code receives a `Cancelled` value
rather than the actual result.

**Refusals.**

- A test that assumes a disposed component's result is delivered:
  the test itself is wrong, refused as
  `test-asserts-unspecified`.

**The semantic guarantee.** A disposed component **does not
update its own state**. The `state-after-dispose` rule from
Question 1 is what enforces this.

**Phase 1 enforces.** Pending-request cancellation on dispose.
**Phase 2+ refines.** Cross-component cancellation if an asker
appears.

---

## Question 6 — How are failures and cancellation represented?

**Answer.** An effect result is one of three values:

- `Ok(value)` — the effect completed normally.
- `Err(reason)` — the effect failed; `reason` is a typed value
  from the effect's declared error type.
- `Cancelled` — the request was cancelled before resolving.

There is **no exception, no implicit throw, no try/catch**. A
component that wants to handle an `Err` matches on the result
explicitly.

**Tier.** **static** (the type) + **runtime** (the delivery).

**Source.** Cancho Core's
[`stdlib result`](https://github.com/alpibrusl/cancho/tree/main/std/result.cho):
`Result[T, E]` is an enum, not an exception. The same shape
applies here. Errors are values, not control flow.

**Refusals.**

- An `Err` arm missing from a `match` on a request result:
  `match-non-exhaustive`. (Same rule as
  [`AGENTS.md §7.1`](https://github.com/alpibrusl/cancho/blob/main/AGENTS.md#71-option-result-loops-and-text-what-a-program-that-returns-an-error-needs)
  in Cancho Core.)
- An unhandled `Cancelled`: same refusal.

**Phase 1 enforces.** The result-type discipline, exhaustive
match. **Phase 2+ refines.** Error-type vocabulary (per
effect, per platform).

---

## Question 7 — How are capabilities declared, passed, and checked?

**Answer.** A capability is declared on the component that
uses it (`requires Capability`), borrowed by transition
handlers and view blocks that need it, and checked statically
(declared) plus at runtime (granted).

**Tier.** **static** (declaration and reachability) +
**runtime** (grant / denial).

**Source.** Cancho Core's effect-row model ([`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md))
and the authority report
([`authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md)).
A capability is a `res` value; owning one discharges its effect,
borrowing one declares it.

**Capabilities in Phase 1.** `Network`, `Storage`, `Clock`,
`Location` — the minimum a searchable-list demo needs. Each is
declared in the Phase 0 capability vocabulary (see
[`docs/security-model.md`](security-model.md)).

**Refusals.**

- An effect call without a `requires` declaration on the
  enclosing component: `capability-missing`.
- A `requires` declaration with no reachable use:
  `capability-unused` (the same "narrow the body, never widen
  the row" rule as Cancho Core).
- A `requires` declaration that is narrower than the effect's
  needs: `capability-insufficient` (e.g. `requires Network`
  when the effect needs `NetworkWhenAuthenticated`).

**Phase 1 enforces.** Declaration form, capability-missing,
capability-unused. **Phase 2+ refines.** Narrowing within
capabilities (e.g. `Network("api.example.com")`) — the same
prefix-narrowing model Cancho Core applies to `Fs(prefix)` and
`Net(bound)`.

---

## Question 8 — What must be checked at compile time versus runtime?

**Answer.**

| Checked at compile time | Checked at runtime |
|---|---|
| State ownership (Question 1) | Disposed-component state writes (trapped) |
| Capability declaration (Question 7) | Capability grant / denial |
| Effect reachability (Question 7) | Effect execution failure |
| Reserved-name use | Dynamic platform conditions |
| Foreign-interface declaration | Cancellation delivery |
| Accessibility metadata presence | Focus order, ARIA at runtime |

The rule is the same as Cancho Core's
[`check-cost.md`](https://github.com/alpibrusl/cancho/blob/main/docs/check-cost.md):
the price list of every check, read out of the binary.

**Tier.** **static** (compile-time column) + **runtime** (runtime column).

**The cost principle.** A check that can be done at compile time
**must be done at compile time** unless the source language
itself cannot express it. Runtime checks exist for things the
source cannot prove (e.g. whether the user granted a device
permission), not as a substitute for what the compiler can prove.

**Refusals.**

- A check the compiler can do but the implementation defers to
  runtime: caught by `conformance/deferred-checks.cho` (Phase 2).

**Phase 1 enforces.** The full compile-time column. **Phase 2+
refines.** The runtime column.

---

## Question 9 — How are accessibility roles, labels, focus, and keyboard behavior represented?

**Answer.** Every view primitive carries an **accessibility
block** at the language level: `role`, `label`, `focus_order`,
`keyboard_action`. The web backend lowers these to ARIA
attributes; the mobile backend lowers them to platform-native
roles.

```text
component SearchForm {
    view {
        Column {
            TextField(value: query, label: "Search query")
            Button(label: "Search", action: SearchRequested)
        }
    }
}
```

**Tier.** **static** (declaration) + **runtime** (delivery).

**Source.** The web's accessibility model is the WAI-ARIA spec;
the mobile backend's is the platform-native equivalent
(`UIAccessibility` on iOS, `AccessibilityNodeInfo` on Android).
The language exposes a uniform surface; the backend picks the
right lowering.

**Refusals.**

- A primitive with no `role`: `accessibility-role-undefined`.
- A primitive with a `label` that is empty when the role
  requires one: `accessibility-label-empty`.
- An interactive primitive (Button, TextField) with no
  `keyboard_action` for the Enter key: `accessibility-keyboard-missing`.

**A Button's `label` is also its visible text** (web backend), unless the
Button has children of its own, which are shown instead; the `label` stays the
accessible name either way. Until this was fixed, the web backend emitted the
label as an `aria-label` and an empty text node, so a labelled Button had
nothing written on it (found by the project page's demo, which hid it with a
CSS rule; tested in `crates/cancho-ui/tests/cli.rs`).

**Focus survives a re-render** (web backend). A view is rebuilt from scratch on
every state change, so the control the user was using used to be replaced:
typing "ada" into a `TextField` left "a" with focus on the page, and pressing
Enter three times on a focused `Button` counted once (both measured in Chromium).
Before a rebuild the web backend now records where the focused control sits
under the component's root (its path of child indices, its tag and its label) and
its text selection, and restores both afterwards; a control that is no longer
there, or is another kind or has another label, stays unfocused instead of
focusing the wrong one. Emitted only for a program with a `Button` or a
`TextField`. Checked in a browser by `scripts/check-focus.js` (and by mutation:
without the restore, all five checks fail). **Not handled:** an input-method
composition in progress; scroll position inside a rebuilt container; any
control that moves in the tree between two renders (it loses focus).

**Phase 1 enforces.** Role, label, basic keyboard. **Phase 2+
refines.** Focus order, screen-reader semantics, color-contrast
checks (external tool).

---

## Question 10 — How are platform-specific differences declared without hiding them?

**Answer.** Platform differences are declared explicitly with a
`target` annotation, either on a component or on a block. A
declaration that exists on one target but not another is
refused on the unsupported target with
`unsupported-on-target`.

```text
component GeolocationPrompt {
    requires Location

    view {
        target web {
            // web-specific DOM construction
        }
        target mobile {
            // mobile-native prompt UI
        }
    }
}
```

**Tier.** **static** (the unsupported-on-target refusal).

**Source.** Cancho Core's `wasm32-wasip1` work
([`docs/wasm.md`](https://github.com/alpibrusl/cancho/blob/main/docs/wasm.md))
already has a 20-fixture map of "refused because this target
lacks it" — the same shape this question asks for.

**Refusals.**

- A `target` block for a target the build does not include:
  `target-not-in-build`.
- A target block whose semantics differ from the shared spec
  in a way the build cannot prove equivalent:
  `target-divergence-unproved` (Phase 4).

**Phase 1 enforces.** `unsupported-on-target`. **Phase 4
refines.** The full mobile-backend target matrix.

---

## Question 11 — What is the exact boundary between checked source code and foreign/native code?

**Answer.** The boundary is the `foreign` block. Inside the
`foreign` block, names are declared with a `from "scope"` and a
return type. Outside, every effect call must resolve to a
declared `foreign` function or to a cancho-ui function whose body
the compiler can verify.

A foreign function whose implementation is provided by JavaScript
or native code is a **trust boundary**, full stop. The
implementation is opaque to cancho-ui; the evidence report names
it as such.

**Tier.** **static** (the boundary) + **trust** (the
implementation).

**Source.** Cancho Core's
[`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md):
an `extern fn` that borrowed no `Ffi` was accepted, ran a shell,
and reported `bounded: true`. The hole was closed by requiring a
declared capability at the call site. The same mechanism
applies to cancho-ui's `foreign` blocks.

**Refusals.**

- A call to a `foreign` function without a `requires`
  declaration on the enclosing component: `capability-missing`.
- A `foreign` function without a `from "scope"`:
  `foreign-scope-missing`.
- A `foreign` function whose body (in the language's terms)
  makes a promise the platform cannot back: caught at runtime as
  `foreign-promise-unmet` and reported in the evidence report.

**Phase 1 enforces.** The boundary declaration. **Phase 2+
refines.** Trust-boundary reporting in the evidence report.

---

## Question 12 — Which properties are guaranteed by the language, and under what assumptions?

**Answer.** Every guarantee above is conditional on the
following **explicit assumptions**:

1. The Phase 1 supported subset is what the language guarantees
   about. Anything outside it is `unknown`.
2. The platform backend behaves as documented. A backend that
   silently drops a capability check, for example, voids
   Question 7's runtime column.
3. The trust boundaries (Question 11) hold. A foreign function
   that misbehaves is not cancho-ui's fault, but is named in
   the evidence report so a reviewer can see what was trusted.
4. The replay harness (Phase 2) produces deterministic traces
   for the events the language defines as deterministic
   (Question 3).
5. The accessibility runtime delivers the metadata the
   language declares (Question 9). The Phase 1 contract is
   "declare it; the runtime is responsible for delivery."
6. No claim exceeds the evidence available. The `unknown`
   tier is used where applicable.

**Tier.** All five tiers, as applicable per question.

**Source.** Cancho Core's "five guarantee tiers" — same
discipline, same tier labels, same refusal to over-claim.

**Refusals.**

- A report that claims a guarantee the implementation does not
  enforce: caught by the `conformance/evidence-report.cho`
  suite (Phase 2).
- A claim in this document that turns out false: corrected in
  place in the same paragraph, with the date.

---

## Arithmetic is finite

**Answer.** `+ - * /` on numbers never answer `Infinity` or `NaN`; the language has no such value. A division by zero (`1 / 0`,
`0 / 0`), an overflow (`1e200 * 1e200`) and any other result that is not a finite number is a **fault**, the same in both
evaluators:

* in `cancho-ui test` it fails the step with `replay-non-finite` (`docs/replay.md` section 7.1);
* in the page, the emitted `+ - * /` go through `__cui_fin` / `__cui_add`, which throw an `Error` whose message begins
  `replay-non-finite`. The handler stops at that statement: the statements before it have run and the ones after it have not, as for any
  error a handler throws (a handler is an async function, so the page sees an unhandled rejection).

**Source.** Until this was written the two disagreed: replay refused, the page showed `Infinity` or `NaN`
(`docs/conditionals.md` section 3.1 found it by reading the code; the old emitter wrote `(100 / __state_m)`, which JavaScript
answers `Infinity` for a zero `m`, and the page then showed `Infinity`). Making replay answer
as JavaScript does was rejected: a trace cannot carry `Infinity` (JSON), a `Text` would print `Infinity`, an `Int` field would hold
a value no `Int` has, and a later `==` or `sort` would depend on it. Making the page refuse costs one function call per operation
and nothing else.

**Tier.** **tested**: `tests/arith_corpus.json` (22 cases: both signs of zero, `0.1 + 0.2`, division by zero of both signs,
`0 / 0`, overflow by `+`, `-`, `*` and `/`, underflow to zero) is checked against the Rust model (`cancho_ui_replay::arith`, which
`binary` calls) by `cargo test`, comparing bits, and against the emitted JavaScript in Chromium by `scripts/check-arith.js`,
comparing with `Object.is`; the same script clicks a button whose handler divides by zero.

**What is not covered.** `+` with a string is the host's concatenation and is not checked. The builtins `min`, `max`, `abs`,
`floor`, `ceil` and `round` model JavaScript's `Math.*` (`docs/lists.md` section 8), including NaN for a value that is not a
number; they are not routed through `__cui_fin`, so `min("1", 2)` is NaN in the page and `replay-type-error` in replay (unchanged).
A literal division by zero in source (`x / 0`) is not refused at `check`; it faults when it runs.

---

## Guarantees table

One row per question, with the tier and what is and is not
guaranteed.

| # | Question | Tier | What is guaranteed | What is not |
|---|---|---|---|---|
| 1 | State ownership | static + runtime | State is owned by its declaring component; cannot be read after dispose | Cross-component sharing beyond parent-child (Phase 2+) |
| 2 | Transition legality | static + runtime | Effect reachability, state ownership | Phase 1 silently drops unhandled events (Phase 2 may change) |
| 3 | Event ordering | runtime + tested | Sequential within a component | Cross-component ordering is undefined |
| 4 | Result correlation | static + runtime | Results are correlated with their request token; results for disposed requests are dropped | Ordering across multiple in-flight requests on the same token type |
| 5 | Cancellation | runtime + tested | Dispose cancels pending requests | Cooperative vs. preemptive — always cooperative |
| 6 | Failures | static + runtime | Results are `Ok`/`Err`/`Cancelled`; no exceptions | Per-effect error-type vocabulary (Phase 2+) |
| 7 | Capabilities | static + runtime | Declaration is checked; grant/denial at runtime | Narrowing within capabilities (Phase 2+) |
| 8 | Compile vs. runtime | static + runtime | Static checks at compile time, runtime checks at runtime | None — the split is by what the source can express |
| 9 | Accessibility | static + runtime | Role, label, keyboard declared at language level | Focus order, screen-reader semantics (runtime / external) |
| 10 | Platform differences | static | Declared with `target`; refused on unsupported targets | Equivalence across targets (Phase 4) |
| 11 | Foreign boundary | static + trust | The boundary is declared; implementations are trust | The implementation's behavior |
| 12 | Total claim | (all tiers) | The five tiers above, conditional on §12's six assumptions | Anything outside the supported subset |

---

## What Phase 1 must enforce

Recap of the Phase 1 contract, in one place:

- **Q1.** `state-unused`, `state-after-dispose`, `state-escapes`.
- **Q2.** `effect-not-declared`, `state-not-owned`.
- **Q3.** Sequential semantics, deterministic replay; refuse
  `test-asserts-unspecified`.
- **Q4.** Request-token type, basic correlation.
- **Q5.** Pending-request cancellation on dispose.
- **Q6.** Result-type discipline, exhaustive match.
- **Q7.** `capability-missing`, `capability-unused`,
  `capability-insufficient`.
- **Q8.** The full compile-time column.
- **Q9.** `accessibility-role-undefined`, `accessibility-label-empty`,
  `accessibility-keyboard-missing`.
- **Q10.** `unsupported-on-target`, `target-not-in-build`.
- **Q11.** `foreign-scope-missing`, `capability-missing` (already
  in Q7).
- **Q12.** All five tiers in reports; refuse
  `conformance/evidence-report.cho` over-claims.

The Phase 1 fixtures in
[`examples/refusal/`](../examples/refusal/) exercise every
refusal above. Phase 2+ fixtures exercise the refinements.