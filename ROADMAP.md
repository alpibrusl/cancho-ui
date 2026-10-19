# ROADMAP.md

> **Status: Phase 0 and Tier 1 of Phase 1 shipped in the initial
> commit (`42993cd`); Tiers 2–17 and the Phase 2 increments are
> shipped since — see the "What shipped" table.** This document
> records what cancho-ui intends to build, in what order, and how
> each phase ends. A
> phase that ends is a phase whose exit criteria are met; a phase
> that stops is one whose stop/rethink conditions fired. Estimates
> are planning ranges, not commitments — the same caveat
> [cancho's own ROADMAP.md](https://github.com/alpibrusl/cancho/blob/main/docs/ROADMAP.md)
> applies.
>
> The proposal's §10 is the source; this is its operating form, with
> the cancho Core dependencies named and the stop/rethink conditions
> made explicit at every phase.

## What shipped

| Slice | Status | Where it landed |
|---|---|---|
| **Phase 0** — design contract | ✅ done | initial commit `42993cd` |
| **Phase 1 / Tier 1** — static render end-to-end (Hello, world) | ✅ done | initial commit `42993cd` |
| **Phase 1 / Tier 2** — state + events + Counter works in the browser | ✅ done | per-tier commits |
| **Phase 1 / Tier 3** — `requires` enforcement, capability checker, searchable-list demo | ✅ done | `examples/search`, per-tier commits |
| **Phase 1 / Tier 4** — `match` on `Result`, `Option`/`List` + `For` | ✅ done | per-tier commits |
| **Phase 1 / Tier 5** — clock + async (`Clock`/`await`), `net.search` runtime | ✅ done | per-tier commits |
| **Phase 1 / Tier 9–11** — Network adapter, cancellation, `filter`/`map`/`len` | ✅ done | per-tier commits |
| **Phase 1 / Tier 12–14** — composition, properties, dynamic properties | ✅ done | `examples/cards`, per-tier commits |
| **Phase 1 / Tier 15** — instance-identity preservation (slots) | ✅ done | `5bf5043` |
| **Phase 1 / Tier 16** — cross-file `import` (Tier 17 = disposal, below) | ✅ done | `2bffe9b`, `examples/dashboard` |
| **Phase 1 / Tier 17** — component disposal → `Cancelled` to the arm | ✅ done | this commit, `examples/cancel` |
| **Phase 2** — capability/effect reachability, full refusal suite, evidence report | ✅ done incrementally per tier | `spec/diagnostics/README.md` frozen Phase 0 |

Per-tier rows 2–14 keep their detail in the git history and
`SESSION-LOG.md`; rows 15–17 and the Phase 2 increment are the
story of the recent sessions. The plan tiers below (Tier 2.1 …)
are the historical planning notes — reality diverged positively:
most landed as Tier 5–14 slices instead.

### Phase 1 / Tier 1 row

**Phase 1 Tier 1: hello world — static render end-to-end**
(`42993cd`). Shipped the lexer, parser, type-checker, typed UI IR,
canonical JSON serializer, JavaScript + DOM emitter, and CLI binary
in one commit. Hello-world source (`examples/hello/hello.cui`) compiles
to JavaScript that produces the right DOM tree in a real browser
(verified via jsdom). 19 tests, all green.

**Found while writing it:**

- **`EmptyView` check was added then removed.** It refused
  components with an empty `view` block, but empty view bodies are
  legitimate when a component composes an empty wrapper around a
  child. The check is gone; no rule in the Phase 1 catalog required
  it. Recorded here so a future reader doesn't add it back.
- **The IR `Span` orphan rule** required a wrapper type
  (`IrSpan`/`IrPosition`) in `cancho-ui-ir`. `Serialize` cannot be
  `impl`'d for a foreign type, so the IR owns its serializable
  positions and converts from `cancho_ui_syntax::span::Span`.
- **The proposal §5.2 sketch used call-style syntax
  (`Text("hello")`)**; the implementation and
  `spec/grammar/README.md` use block-style (`Text { "hello" }`).
  Corrected in [`docs/language-design.md`](docs/language-design.md)
  §10 decision 11 and in [`docs/proposal.md`](docs/proposal.md)
  §5.2 (which now notes the divergence and points at the grammar).
- **`.gitignore` initially forgot `/target`** — `git add .` pulled
  in build artifacts. Reset, fixed, restaged. Recorded so the next
  workspace doesn't repeat the mistake.
- **`/target` is in the ignore, but `examples/hello/hello.js` is
  generated** and ignored via `*.js`. Reproducing the hello world
  is `cargo run -p cancho-ui --bin cancho-ui -- build
  examples/hello/hello.cui -o examples/hello/hello.js`, not a
  committed artifact.

**Not in this slice (deliberately):** state, events, capabilities,
matches, async, foreign, target blocks, source maps,
`--output json`, agent-facing `inspect` / `diff` / `report` /
`skill` / `introspect` / `test`. These are Tier 2 / Tier 3 / Tier 4 /
Tier 5 below.

---

## Phase 0 — Design contract and feasibility

**Indicative duration:** 1–2 weeks.

**Status:** done. Phase 0 deliverables landed in the initial
commit (`42993cd`); see [`SESSION-LOG.md`](SESSION-LOG.md) for
the implementation record. The Tier 1 implementation followed
immediately and lives in the same commit.

- `docs/proposal.md` — the polished proposal, citing the cancho Core
  documents that settle each inherited idea.
- `docs/language-design.md` — the syntax sketch and the design
  decisions behind it.
- `docs/semantics.md` — answers to all twelve questions in §5.3 of
  the proposal, each labelled with the guarantee tier
  (`static | tested | runtime | trust | unknown`).
- `docs/security-model.md` — the five-tier guarantee model, the
  capability/effect rules, and the trust-boundary contract.
- `docs/platform-model.md` — the decision record for web-first and
  the mobile-backend candidates.
- `spec/ir-schema/README.md` — the typed UI IR sketch and its
  conformance fixtures.
- `spec/diagnostics/README.md` — the machine-readable diagnostic
  format.
- `spec/grammar/README.md` — the grammar sketch (Lark or hand-rolled
  EBNF; nothing implemented).
- `examples/README.md` — at least ten valid and ten invalid fixtures,
  written down, **not compiled** — the contract the Phase 1 compiler
  must meet.

**Exit criteria**

- Every inherited cancho Core guarantee is named against the cancho
  document that settles it (no hidden duplication, no
  re-stated-without-citation rules).
- Every net-new guarantee has a fixture under `examples/` and a tier
  labelled in `docs/semantics.md`.
- The mobile-backend decision is recorded with measured reasoning,
  not taste.
- The IR schema sketch has enough fidelity that the Phase 1 backend
  cannot back itself into a corner in two weeks.

**Stop / rethink if**

- The semantic rules cannot express asynchronous UI behavior without
  ambiguous ordering or lifecycle rules — i.e. there is no model the
  team can explain without framework-specific assumptions.
- The proposed safety story depends on trusting arbitrary foreign
  code as if it were verified cancho-ui.
- The language adds complexity without an identifiable
  agent-facing benefit (the central differentiator must remain
  visible).
- The cancho Core integration contract is not on a roadmap —
  cancho-ui cannot depend on a "to be added later" compiler IR.

---

## Phase 1 — Minimal web vertical slice

**Indicative duration:** 2–4 weeks.

**Deliverables**

- Parser and minimal type checker.
- Small typed UI IR (the schema from `spec/ir-schema/`, not a
  re-design).
- Component composition and typed properties.
- Local state and event handlers, with the semantic rules from
  `docs/semantics.md` §2 and §3.
- Web backend producing JavaScript + DOM behavior (no JSX, no
  virtual DOM library — the IR lowers directly to DOM ops).
- Source maps or usable source-level diagnostics.
- Build, check and test commands with stable exit codes
  (`cancho-ui build`, `cancho-ui check`, `cancho-ui test`).

**Exit criteria**

- A Counter-style example (component, state, event handler, view)
  compiles and runs in a browser.
- A searchable-list example compiles and runs, exercising
  asynchronous work, lifecycle, and one capability.
- All `examples/refusal/` fixtures are refused with the documented
  rule.
- The CLI's `--help` matches `cancho-ui skill`'s command list, the
  same way
  [`cancho agent_cli::every_dispatched_command_is_documented`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md)
  enforces.
- The build is reproducible from a clean checkout.

**Stop / rethink if**

- The prototype is mostly a thin syntax layer over framework code
  with no useful semantic checks.
- The IR cannot support diagnostics or backend conformance without
  excessive special cases — i.e. the schema from Phase 0 was
  underspecified.
- The capability checker cannot distinguish declared from undeclared
  authority on the web backend (a `Network` request that the
  compiler cannot link to a `requires Network` line means Phase 0's
  security model was incomplete).

---

## Phase 2 — State, effects, and agent-facing verification

**Indicative duration:** 2–4 weeks.

**Deliverables**

- Defined transition semantics (the answers to §5.3 #2, #3).
- Defined asynchronous result and cancellation model for the
  supported subset (§5.3 #4, #5, #6).
- Minimal capability declarations and effect reachability checks
  (§5.3 #7, #8).
- Structured JSON diagnostics — the locked format in `spec/diagnostics`.
- `cancho-ui inspect component`, `cancho-ui inspect effects` —
  machine-readable component and capability/effect introspection.
- `cancho-ui report` — the evidence report separating static
  checks, tests, runtime checks, and trust boundaries.
- The full refusal suite from `examples/refusal/`.

**Exit criteria**

- Unauthorized effects are rejected in defined cases
  (`capability-missing` rule).
- Lifecycle and stale-result tests behave according to the
  specification.
- Agents can identify why a build failed from machine-readable
  diagnostics.
- The report names every trust boundary and does not claim
  guarantees the implementation does not enforce.

**Stop / rethink if**

- The effect checker cannot distinguish declared from undeclared
  authority — i.e. `effect-not-declared` and `capability-missing`
  cannot be told apart, and the IR cannot encode the distinction.
- Asynchronous behavior remains nondeterministic in cases the
  language claims to define.
- The diagnostic format drifts between releases (a `crates/cancho-ui`
  snapshot test fails).

---

## Phase 3 — Replayable tests and agent benchmark

**Indicative duration:** 2–3 weeks.

**Deliverables**

- Deterministic event inputs for supported event sequences.
- Replayable traces for the demo application.
- `cancho-ui diff <base> <candidate>` — semantic diff for
  state/effect/capability changes. **Built**; see
  [`docs/semantic-diff.md`](docs/semantic-diff.md).
- A small set of repeatable agent tasks (the proposal §10 list).
- A conventional baseline implementation in a familiar web stack
  (React or Svelte, picked to match what an agent would write by
  default).
- Benchmark protocol and results report — the same shape
  cancho Core's [`eval/`](https://github.com/alpibrusl/cancho/tree/main/eval)
  uses, with pre-registration and a fixed oracle.

**Example agent tasks** (from the proposal §10)

- Add a validation rule.
- Fix a stale-response bug.
- Add a new state to the search flow.
- Introduce a capability-gated operation.
- Refactor a component without increasing its authority.

**Metrics**

- Task completion and functional correctness.
- Regression rate.
- Number of agent repair iterations.
- Human interventions required.
- Unintended capability expansion.
- Test coverage of specified behaviors.
- Time to diagnose and fix seeded bugs.

**Exit criteria**

- Tasks can be repeated using a documented protocol.
- The baseline and cancho-ui are compared on equivalent
  requirements.
- Results include failures and limitations, not only successful
  examples.
- The methodology and the harness are committed before the runs
  start (pre-registered), so the outcome cannot reshape the
  measurement.

**Stop / rethink if**

- No measurable improvement appears in correctness, diagnosability,
  authority control, or agent repair effort.
- The benchmark rewards language familiarity or task differences
  rather than the intended design advantages.

---

## Phase 4 — Mobile backend feasibility and cross-platform contract

**Indicative duration:** 2–4 weeks.

**Deliverables**

- A technical spike for React Native (or the candidate chosen in
  Phase 0's `docs/platform-model.md`).
- A written comparison of rendering, lifecycle, asynchronous
  behavior, permissions, accessibility, and foreign interfaces
  between web and mobile.
- A minimal mobile build of the same searchable-list demo, if
  feasible.
- Backend conformance tests against the shared IR and semantics.
- An explicit list of platform-specific exceptions (no silent
  degradation).

**Exit criteria**

- The same shared state/event specification produces equivalent
  observable behavior for the supported flows.
- Platform-specific effects go through declared adapters.
- Unsupported operations fail clearly rather than silently
  degrading.
- The team has enough evidence to choose a mobile backend for
  Phase 5.

**Stop / rethink if**

- The runtime cannot preserve essential language semantics without
  pervasive platform-specific escape hatches.
- The mobile implementation undermines the verification model
  (a `bounded: true` report for a program that actually uses
  untyped native code, for example).
- Cross-platform equivalence costs more than the value it delivers
  for the initial target users.

---

## Phase 5 — Cross-platform MVP release candidate

**Indicative duration:** 2–4 weeks.

**Deliverables**

- Web and one mobile backend for the demo.
- Reproducible setup/build instructions.
- Versioned source/IR format and documented compatibility
  expectations.
- Conformance, refusal, lifecycle, and regression suites.
- Agent-facing CLI and machine-readable evidence reports.
- A security and limitations document — the explicit list of what
  is and is not verified.
- A short demonstration showing an agent implementing a feature
  and repairing a deliberately introduced defect.

**Exit criteria**

- A new developer can build and test the project from a clean
  checkout.
- The demo runs on the supported targets.
- Core cross-platform behaviors pass the same
  specification-driven tests.
- Capability changes are visible in semantic diffs and reports.
- Known limitations and trust boundaries are documented.
- The benchmark supports a defensible statement about what is
  better, what is not, and what remains unproven.

**Stop / rethink if**

- Cross-platform behavior cannot be made consistent for the chosen
  feature subset.
- The safety claims are difficult to explain or reproduce.
- The implementation is too costly to maintain relative to its
  measured benefit.

---

## Phase 6 — Conditional post-MVP work

Only pursue after the MVP has evidence of value:

- Additional platform APIs and capability types.
- Better accessibility checks and test tooling.
- More robust component and layout model.
- More mature native adapters.
- Direct Swift/Kotlin generation experiments — gated on the
  mobile-backend decision in Phase 4.
- Optional WebAssembly for computation-heavy modules (the cancho
  Core [`docs/wasm.md`](https://github.com/alpibrusl/cancho/blob/main/docs/wasm.md)
  work is a starting point).
- IDE integrations and agent tool protocols
  ([`acli`](https://github.com/alpibrusl/acli) is the cancho
  precedent).
- Package ecosystem and reusable component libraries — gated on
  cancho Core's [`docs/package-system.md`](https://github.com/alpibrusl/cancho/blob/main/docs/package-system.md)
  having a cancho-UI equivalent, which itself depends on the
  Phase 1 IR.
- Broader verification of foreign interfaces and dependency
  contracts.

---

## Deliberately excluded (for now)

- Replacing JavaScript, Swift, or Kotlin.
- A complete general-purpose UI framework.
- A bespoke browser engine or rendering engine.
- A new mobile operating-system runtime.
- Direct native code generation for every iOS and Android UI API
  (this is a Phase 6 option, not a Phase 1 promise).
- Comprehensive formal verification of the entire application and
  all dependencies.
- Automatic proof that third-party libraries are safe.
- A large component marketplace or design system.
- Full production-grade IDE support.
- Supporting every browser, OS version, device API, and
  accessibility scenario at launch.
- Claiming production readiness before security, accessibility,
  lifecycle, and compatibility testing.

Each of these is "yes, later" rather than "no" — saying yes early
is what turns three months into three years.

---

## Beyond

If the benchmark shows measurable improvement, cancho-ui has a
credible reason to exist. If it does not, the useful parts
(structured diagnostics, effect reporting, semantic diffs, and
agent-oriented verification) may still be valuable as tools
around existing languages — and this project's contribution is
then the design and the evidence, not a competing runtime.

The Phase 5 results report decides that. Until then, this is a
research-backed compiler and agent-workflow project.
## Not yet built (against the proposal's Phases 3-5)

Stated here so the table above is not read as the whole of the proposal:

- The agent benchmark (`eval/` is still only the plan).
- A native mobile backend and its conformance tests. (An installable-web-app slice, `build --pwa`,
  exists; see "Built since" below and `docs/mobile-backend.md`. Nothing is tested on a phone or on iOS.)
- Self-hosting the compiler in cancho, which follows cancho's own
  `docs/self-hosting.md`: the Rust compiler stays the oracle, and nothing
  is ported until a program asks for it.

Built since: `cancho-ui test <file.cui> <trace.json>` (Phase 3, "replayable
tests"), designed in [`docs/replay.md`](docs/replay.md) first. It replays a JSON
trace of events over an interpreter of the typed IR (new crate
`cancho-ui-replay`), with no browser and no JavaScript runtime, and checks the
component's state after each step and the trace's `expect` assertions, with a
located failure. `net.search` is scripted by the trace (the trace orders the
answers) and the clock is fake, so a replay is deterministic; supersede (the
stale-response drop) and dispose-to-`Cancelled` follow the emitted runtime. The
proposal's headline adverse-ordering test is `examples/search/stale.trace.json`.
Exit `4` means a test failed. It does **not** replay the view, timing or latency,
the real network adapter, child component instances or properties, and it is a
second implementation of the semantics, not the emitted JavaScript (a pass means
the interpreter and the trace agree); every report carries these in
`not_claimed`. Measured: 9 example traces (42 steps, 87 checks) pass; 150 tests
in the workspace; replay ran on 17,161 mutated traces and 15,486 compiling program
mutants without a panic; 28 deliberate bugs in the interpreter were each tried
and 26 were caught (the 2 survivors are equivalent mutants, `docs/replay.md` §13).
Corrected in place while building it: `Cancelled` reaches the arm when the
request completes, not at the instant of disposal; and `clock.cui`'s
"integer-truncated" comment was wrong.

Built since: `cancho-ui build <file.cui> --pwa -o <dir>` (the first slice of Phase 4, mobile),
designed in [`docs/mobile-backend.md`](docs/mobile-backend.md) first: the mobile target is the web
backend installed from the phone's browser (page, manifest, offline service worker, placeholder icons, all
generated, no library). Measured in Chromium with Pixel 5 emulation: no installability errors, the
page starts with the server stopped, every control is at least 44 px; three deliberate breaks of the
generator are caught by `scripts/check-pwa.js`. **Not done:** real `Network`, `Storage` and `Location`
adapters (the web backend's adapters are two stubs, measured; `platform-model.md` said four and is
corrected), page-lifecycle mapping, a React Native comparison, any native shell, iOS and
a real phone. Found on the way: every flag that took a value swallowed the next argument in `check`,
`build` and `report` (`build -o out.js file.cui` failed); fixed, with a test.

Fixed: **focus was lost on every re-render** of the web backend (measured: typing "ada" into a `TextField`
left "a"; Enter pressed three times on a focused `Button` counted once, so a keyboard user could not
activate a button twice). The focused control and its text selection are now recorded before the rebuild and
restored after (`docs/semantics.md`, Question 9; `scripts/check-focus.js` fails without it). Found while
answering what cancho-ui cannot do that React can: it was the first item of that list. Not handled: input-method
composition, scroll position, a control that moves between renders.

Built since: `cancho-ui diff <base> <candidate>` (Phase 3), designed in
[`docs/semantic-diff.md`](docs/semantic-diff.md) first. It compares the
typed IRs by component name: capabilities (`authority: expanded | reduced |
unchanged`), state, events and their writes, handler bodies, reachable
effects with spans, and view elements. It does **not** yet cover
foreign/trust boundaries, component properties, lifecycle, event ordering or
cancellation, or view structure, because the IR has no such nodes (or, for
the view, because it compares a multiset of elements, not a tree); the report
names them in `not_compared`. Exit `5` means "differs" because `1` already
means "refused". Measured: 35 tests (`tests/diff.rs`, plus the diff arm of
`tests/no_panic.rs`, which diffed 14,698 compiling mutants of the examples
against their originals, both ways, without a panic); each of eight planted
bugs in the implementation was caught by at least one of them.

Corrected in place: `cancho-ui inspect effects` printed
`"reachable_effects": "[]"` for every component, a claim of no effects the
compiler had not checked. It now prints the IR's `reachable_effects`.
The gate (fmt, clippy `-D warnings`, test) failed on the initial tiers and
had no CI; both are fixed, and `tests/no_panic.rs` mutates every example
(each prefix, each one-character deletion) to look for a panic.
