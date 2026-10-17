# Cancho UI

## Initial Proposal and Roadmap to an AI-Agent-Oriented Frontend and Mobile MVP

**Status:** Phase 0 — design contract, under review\
**Working name:** `cancho-ui`\
**Parent ecosystem:** [Cancho](https://github.com/alpibrusl/cancho)\
**Date:** 17 October 2026\
**License:** [EUPL-1.2](https://github.com/alpibrusl/cancho/blob/main/LICENSE),
matching the rest of the Cancho ecosystem\
**Primary hypothesis:** A UI language designed around explicit
behavior, capabilities, and machine-readable verification can help
AI agents build and maintain applications more reliably than using
conventional UI stacks alone.

> **Relationship to Cancho Core.** Cancho-UI extends Cancho Core's
> systems-language philosophy into the user-interface domain without
> putting DOM, browser lifecycle, accessibility, or mobile-UI
> concepts into the systems-language core. It depends on Cancho
> Core's type/effect semantics, capability model, and
> machine-readable diagnostics through a documented and versioned
> integration contract — not on unstable compiler internals. See
> [`README.md`](README.md) for the position diagram.

---

## 1. Executive summary

Cancho UI is a declarative language and toolchain for building
interactive applications for the web and mobile devices. It compiles
one source model to established platform technologies:

- **Web:** JavaScript and the real DOM, CSS, browser accessibility
  model, and standard web APIs. No JSX, no virtual-DOM library.
- **Mobile (later):** an established cross-platform runtime such
  as React Native, evaluated against explicit conformance criteria
  before commitment — not a bespoke native UI framework.

The goal is not to replace JavaScript, TypeScript, Swift, Kotlin,
or their ecosystems. The goal is to provide a **single, explicit
behavioral model** for UI state, asynchronous effects, permissions,
and lifecycle behavior, then compile that model to established
platform technologies.

The central differentiator is **AI-agent-oriented engineering**:

- The compiler exposes structured, machine-readable information
  about components, state transitions, effects, and platform
  requirements.
- Capabilities and authority are **declared and checked** rather
  than inferred from source-code conventions.
- State transitions and asynchronous behavior have **defined
  semantics** that can be tested and replayed.
- Agent changes can be evaluated through compiler checks, tests,
  semantic diffs, and evidence reports.
- Unverified native or foreign code is identified as a **trust
  boundary** rather than silently treated as proven safe.

This list is essentially the same shape as Cancho Core's
[`authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md)
and [`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md),
applied to UI: explicit effects, exact effect rows, symbol-level
foreign authority, refusal-driven diagnostics.

**The first implementation should be deliberately small.** Start
with a web vertical slice, prove the language and verification
model, then add a mobile backend using an established
cross-platform runtime. Dedicated Swift and Kotlin code generation
are considered only after the common semantics and agent-facing
workflow have demonstrated value.

**MVP success is not "an agent can write Cancho UI syntax."** It
is evidence that an agent can make correct changes with fewer
regressions, clearer authority boundaries, and less human
intervention than a comparable conventional implementation.

---

## 2. Problem statement

AI coding agents can produce frontend and mobile code today. The
problem is that **a successful compilation does not establish that
an application behaves correctly, uses only intended permissions,
handles asynchronous work safely, or remains consistent across
platforms.**

Common sources of difficulty:

1. **Implicit effects.** A component may acquire network, storage,
   analytics, location, or other behavior through a dependency.
   The same hazard Cancho Core
   [named for `Ffi("libc")`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md#5-the-design):
   authority can leak through a typed wrapper.
2. **Scattered state.** UI behavior can be spread across
   callbacks, hooks, mutable objects, effects, and lifecycle
   handlers. The same observation Cancho Core made about
   resource lifetimes
   ([`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)):
   ownership matters, and ownership is invisible when it is
   implicit.
3. **Asynchronous races.** Requests can finish after navigation,
   overwrite newer results, or update an inactive screen.
4. **Platform divergence.** Similar features may have subtly
   different behavior on web, Android, and iOS.
5. **Opaque diagnostics.** Agents often have to interpret logs
   and framework-specific errors before they can make a repair.
   Cancho Core's
   [`agent-errors.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md)
   rule-tagged refusals are the model this proposal asks cancho-ui
   to inherit.
6. **Incomplete verification.** Tests cover selected cases but
   may not explain what was checked, what remains unknown, or
   which assumptions cross native boundaries.
7. **Large change surface.** A small feature can require edits to
   many files and framework-specific conventions, making it
   harder for an agent to predict the consequences of a change.

Cancho-UI addresses these problems by making behavior and
authority explicit and by providing a compiler interface that
agents can query.

---

## 3. Design principles

### 3.1 Explicit behavior over hidden conventions

State, events, effects, permissions, and lifecycle rules are
represented in the language's semantic model. No implicit
behavior. This is the same rule Cancho Core enforces at the
systems-language level: a
[function's effect row is exact in both directions](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md#3-effects),
and "an effect is a borrowed capability" — which is the entire
mechanism this proposal asks cancho-ui to reuse.

### 3.2 Reuse platform ecosystems

Use existing browser APIs, JavaScript tooling, and mature mobile
rendering/runtime technologies wherever practical. Cancho-UI adds
a verification layer, not a replacement for every platform
library. The Cancho Core analogue: `Net` is built from socket
syscalls via typed builtins, [not by re-implementing `libc`](https://github.com/alpibrusl/cancho/blob/main/docs/native-sockets.md).

### 3.3 Guarantees must be precise

The toolchain distinguishes between five tiers, defined fully in
[`docs/security-model.md`](security-model.md):

- **Statically checked** — enforced by the compiler or verifier
  under documented assumptions.
- **Tested** — exercised by a named test or test suite.
- **Runtime enforced** — checked by the runtime or platform
  adapter at execution time.
- **Trusted boundary** — external or native behavior whose
  implementation is not verified by cancho-ui.
- **Unknown** — not yet established.

Ordinary compilation or test success is **never** described as
proof of complete correctness. This is the same discipline Cancho
Core applies to its [`authority --output json`](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md)
report: it leads with `"bounded": false` for any program reaching
foreign code, so a supervisor that reads nothing else refuses by
default.

### 3.4 Agent-operable interfaces

Important compiler and test operations have stable command-line
interfaces and **machine-readable output**. Agents do not scrape
human-oriented terminal text to understand a result. Cancho Core's
[`cancho introspect` / `cancho skill`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md)
are the shape this proposal asks cancho-ui to reuse; the
single-dash-flag lesson from
[`agent-cli.md §2`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md#2-the-false-familiarity-bug-this-found)
applies here too.

### 3.5 Small, composable semantics

The first version defines a limited component model, state model,
event model, effect system, and platform interface. Features are
added only when a concrete application or benchmark requires them.
"Cancho stands on its own" as of
[`docs/bootstrap.md`](https://github.com/alpibrusl/cancho/blob/main/docs/bootstrap.md):
the discipline that earned it that name is the discipline
cancho-ui inherits.

### 3.6 Human-readable source

The language is readable and reviewable by people. Agent-oriented
design improves human engineering rather than creating an opaque
agent-only format.

### 3.7 Explicit trust boundaries

Foreign JavaScript, third-party packages, native modules, and
platform APIs are reached through **declared interfaces**. A typed
wrapper does not prove the wrapped implementation safe. Cancho
Core's [`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md)
is the model: an `extern fn` that borrowed no `Ffi` was accepted,
ran a shell, and reported `bounded: true` — the hole was closed
by requiring a declared capability at the call site.

### 3.8 Reproducible results

Build inputs, compiler version, target configuration, and
verification results are recordable so that another developer or
agent can reproduce the result. The
[`hash-stability.md`](https://github.com/alpibrusl/cancho/blob/main/docs/hash-stability.md)
plateau and the per-declaration content hash are the precedent.

---

## 4. Relationship to Cancho Core

**Recommendation: implement cancho-ui in a separate repository,
initially `cancho-ui`, as an official Cancho ecosystem extension.**
Do not add DOM, browser lifecycle, accessibility, or mobile-UI
concepts directly to the systems-language core.

### Cancho Core owns

- Core type and effect semantics that are genuinely shared.
- Compiler infrastructure and stable integration points, where
  appropriate.
- Defined behavior and deterministic representations where
  already supported.
- Common authority concepts and machine-readable diagnostics
  where they can be reused.
- Versioning and compatibility contracts for shared compiler
  interfaces.

### Cancho UI owns

- Declarative UI source syntax.
- Component, layout, state, event, and lifecycle semantics.
- UI-specific effect and capability rules.
- Typed UI intermediate representation (IR).
- Web and mobile backends.
- UI testing, replay, accessibility metadata, and platform
  adapters.
- Agent-facing inspection and semantic-diff commands.

### Integration contract

The UI project depends on a documented and versioned interface
rather than unstable compiler internals. Where core capabilities
are not yet available, the UI prototype uses a small adapter or
explicitly marks the behavior as provisional; it does not force
premature changes to Cancho Core.

### Repository layout

```text
cancho-ui/
  docs/                    design
  spec/
    grammar/               EBNF (Phase 0 only — not implemented)
    ir-schema/             typed UI IR sketch
    diagnostics/           machine-readable diagnostic format
  compiler/                lands in Phase 1
  runtime/                 lands in Phase 1
  backends/
    web/                   lands in Phase 1
    mobile/                lands in Phase 4
  adapters/                lands in Phase 2
  tests/
    conformance/
    refusal/
    cross-platform/        lands in Phase 4
  examples/
  tools/
  eval/                    the agent-benchmark harness
```

This is a proposed structure, not a claim about existing files or
functionality. The Phase 0 deliverable is everything inside
`docs/`, `spec/` and `examples/`.

---

## 5. Proposed language model

Cancho UI expresses **what the interface is, how its state changes,
and which effects it is allowed to perform**, while leaving
platform rendering to a backend.

### 5.1 Core concepts

The initial language supports:

- **Components** — named, composable UI units.
- **Properties** — typed inputs to components.
- **State** — explicitly owned state with defined update rules.
- **Events** — user or platform events that trigger transitions.
- **Effects** — asynchronous work or interaction with the outside
  world.
- **Capabilities** — authority required for effects such as
  network access, storage, camera, or location.
- **Lifecycle** — defined behavior when a component becomes
  active, inactive, or is disposed.
- **Platform adapters** — explicit interfaces to browser and
  native APIs.
- **Stable identities** — deterministic IDs for components and
  events, useful for diagnostics, replay, and semantic diffs. The
  same shape as Cancho Core's
  [`cancho ids`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md).

### 5.2 Illustrative syntax

The following is a sketch to discuss the language design. It is
**not valid or implemented cancho-ui syntax**.

```text
component Counter {
    state count: Int = 0

    view {
        Column {
            Text("Count: {count}")

            Button("Increment") {
                count = count + 1
            }
        }
    }
}
```

An effectful component might eventually look like this:

```text
component NearbySearch {
    requires Network
    requires LocationWhenEnabled

    state query: String = ""
    state results: List<Result> = []
    state status: SearchStatus = Idle

    on SearchRequested {
        // Validate the input and initiate a declared effect.
        // The language must define cancellation and result ordering.
    }
}
```

The syntax is **not frozen** until the semantic rules and a small
prototype have validated the model — see
[`docs/language-design.md`](language-design.md) for the design
decisions behind this sketch and
[`docs/semantics.md`](semantics.md) for what each line actually
guarantees.

### 5.3 Semantics to specify before implementation

At minimum, the design document must answer (each labelled with the
guarantee tier in [`docs/semantics.md`](semantics.md)):

1. Who owns each piece of state?
2. Which transitions are legal, and what happens when an event is
   invalid?
3. Are event handlers sequential, transactional, or otherwise
   ordered?
4. How are asynchronous results correlated with the request that
   created them?
5. What happens to pending work when a component is disposed or a
   request becomes obsolete?
6. How are failures and cancellation represented?
7. How are capabilities declared, passed, and checked?
8. What must be checked at compile time versus runtime?
9. How are accessibility roles, labels, focus, and keyboard
   behavior represented?
10. How are platform-specific differences declared without
    hiding them?
11. What is the exact boundary between checked source code and
    foreign/native code?
12. Which properties are guaranteed by the language, and under
    what assumptions?

These are design questions, not features to assume already exist.
The answer to (1) is "the component, exactly as a `res` value in
Cancho Core" — see [`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)
for the inherited model.

---

## 6. Compiler and runtime architecture

Proposed pipeline:

```text
Cancho UI source
      |
      v
Parser and name/type checking
      |
      v
Effect, capability, state, and lifecycle checks
      |
      v
Typed UI IR
      |
      +--------------------+
      |                    |
      v                    v
Web backend          Mobile backend
      |                    |
      v                    v
JavaScript + DOM     Established cross-platform
and browser APIs     runtime + native adapters
      |                    |
      +---------+----------+
                |
                v
       Tests and evidence report
```

### 6.1 Typed UI IR

The IR is a central design asset. It represents at least:

- Component structure and stable IDs.
- Typed properties and state.
- Event sources and permitted transitions.
- Declared effects and required capabilities.
- Lifecycle constraints.
- Platform requirements and unsupported features.
- Foreign-interface declarations.
- Source mappings for diagnostics.

The IR has a versioned schema, canonical serialization where
feasible, and conformance tests. The schema sketch is in
[`spec/ir-schema/README.md`](../spec/ir-schema/README.md). Avoid
committing to a large universal IR before the first vertical
slice demonstrates which information the backends and tools
actually need.

### 6.2 Web backend and JavaScript

The browser remains a JavaScript environment. Cancho-UI initially
compiles to JavaScript and uses the real DOM, CSS, browser
accessibility model, and standard web APIs.

JavaScript remains useful for:

- Browser integration and DOM operations.
- Existing JavaScript libraries.
- Tooling and bundling.
- Platform APIs exposed by browsers.
- Interoperability with existing applications.

Cancho-UI adds a source-level behavioral model and checks. It
cannot guarantee the behavior of arbitrary JavaScript
dependencies merely by calling them through a typed wrapper.
Foreign calls need explicit contracts and appear in the
trust/evidence report. The same hole Cancho Core closed with
[`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md).

WebAssembly could later be considered for computationally
intensive shared logic, but it does not provide a complete UI or
DOM implementation on its own — exactly the conclusion Cancho
Core's [`docs/wasm.md`](https://github.com/alpibrusl/cancho/blob/main/docs/wasm.md)
reached ("Browser-specific bindings (JS glue, DOM). Possible
later; not the point.").

### 6.3 Mobile backend

Do not attempt to implement independent native UI frameworks for
iOS and Android in the first milestone.

Initial options:

- **React Native** — reuses a JavaScript runtime and provides
  access to native UI components and platform modules.
- **Other established cross-platform runtime** — evaluate only
  if it offers a better fit for the chosen semantics and
  verification needs.
- **Direct Swift/SwiftUI and Kotlin/Jetpack Compose code
  generation** — powerful, but expensive to maintain and
  validate.
- **WebView shell** — simple reuse of web UI, but the interface
  is web-rendered rather than a fully native UI.

**Initial recommendation:** prototype the web backend first, then
evaluate React Native as the first mobile target. Confirm that
the intended state, event, capability, and lifecycle semantics
can be preserved before committing. Swift and Kotlin remain
appropriate for explicit platform adapters and native modules.
The decision record is in
[`docs/platform-model.md`](platform-model.md).

The MVP does not claim that it compiles to native machine code
or that every UI element is native unless the chosen backend
actually delivers that property. The same honesty Cancho Core
applies to its
[`reach.md`](https://github.com/alpibrusl/cancho/blob/main/docs/reach.md):
the report says what the program can reach, not what the
developer hoped it could.

---

## 7. AI-agent-oriented tooling

Agent-facing tooling is part of the product, not a later
convenience.

### 7.1 Required commands

Exact names may change, but the toolchain exposes equivalent
operations:

```text
cancho-ui check
cancho-ui build --target web
cancho-ui test
cancho-ui inspect component <id>
cancho-ui inspect effects <id>
cancho-ui diff <base> <candidate>
cancho-ui report
cancho-ui skill [--output] [<out-file>]
cancho-ui introspect [--output json]
```

The last two mirror
[`cancho skill` / `cancho introspect`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md),
so an agent learns cancho-ui's command surface from the binary
itself rather than from a hand-written document that can drift.
Every command has documented exit codes
(`0` / `1` / `2` / `3`, `4` for `test` only — matching Cancho Core)
and stable machine-readable output, such as JSON. Human-readable
output is parallel.

### 7.2 Required inspection data

An agent can query:

- Component and state structure.
- Events that can change a given state.
- Reachable effects and required capabilities.
- Platform requirements and unsupported operations.
- Lifecycle and cancellation rules relevant to a component.
- Differences in behavior or authority between revisions.
- Which checks passed, failed, or were not performed.
- Which conclusions depend on foreign code or external
  assumptions.

This is the same shape as Cancho Core's `cancho authority`,
applied to UI: what the program reaches, in which rows, with
which trust boundaries.

### 7.3 Structured diagnostics

A diagnostic contains:

- Stable diagnostic code (kebab-case rule tag, e.g.
  `effect-not-declared`).
- Severity.
- Source span and component/state/effect identifier.
- Short explanation.
- Relevant expected and actual values.
- Suggested remediation when the compiler can justify one.
- Whether the issue is a hard error, warning, or unverified
  condition.
- **Guarantee tier** (`static | tested | runtime | trust |
  unknown`) — the same five this proposal uses throughout.

Diagnostics do not rely on LLM-generated explanations as the
authoritative result. Any suggested fix remains a suggestion and
is re-checked after application. The full format is in
[`spec/diagnostics/README.md`](../spec/diagnostics/README.md).
The Cancho Core precedent is the 58-rule catalog in
[`agent-errors.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md).

### 7.4 Semantic diff

A normal source diff shows text changes. A cancho-ui semantic
diff shows changes such as:

- New or removed capabilities.
- Changed state transitions.
- Changed event ordering or cancellation behavior.
- Added platform-specific behavior.
- New foreign-code dependencies.
- Changed accessibility metadata.

This is particularly useful when an agent proposes a small
feature that could have a larger behavioral effect.

---

## 8. Security, permissions, and verification model

The system makes a strict distinction between declaration,
checking, runtime enforcement, and evidence. Full detail is in
[`docs/security-model.md`](security-model.md); this section is the
position.

### 8.1 Capability examples

Potential capability categories:

- Network access.
- Persistent storage.
- Camera and microphone.
- Location.
- Notifications.
- Payments or other sensitive actions.
- Analytics or telemetry.
- File access.

The MVP implements only a small subset, such as network access
and a deliberately gated effect, rather than pretending every
platform capability is already modeled.

### 8.2 Static checks versus runtime reality

The compiler can check that a component declares a required
capability and that calls are not reachable from contexts
forbidden by the language's rules. It cannot by itself
guarantee that:

- The user grants a device permission.
- The OS or browser behaves correctly.
- A remote server returns trustworthy data.
- A third-party library is safe.
- A native adapter correctly implements its contract.

The runtime and platform adapter handle permissions, denials,
unavailable devices, errors, and cancellation. Reports make
these boundaries visible. The same shape as
[`under-a-grant.md`](https://github.com/alpibrusl/cancho/blob/main/docs/under-a-grant.md)
in Cancho Core: the report names what the program can reach,
and refuses anything that crosses a foreign boundary.

### 8.3 Refusal and negative tests

The test suite includes examples that **must be refused**, not
only valid examples that must compile.

Examples:

- A component attempts to access location without the required
  capability.
- A component calls an effect after its lifetime has ended.
- A stale asynchronous response overwrites a newer result when
  the semantics forbid it.
- A backend silently drops a required capability check.
- A foreign API is called without a declared interface.
- A platform target lacks a required feature but the build
  claims success.

These tests are fixed early and fail if the implementation
violates the contract. They live in
[`examples/refusal/`](../examples/refusal/) (Phase 0: written
down, not yet compiled).

---

## 9. MVP definition

### 9.1 MVP objective

Demonstrate that an AI agent can build and safely modify a small
interactive application from one cancho-ui source model, with
machine-readable evidence and a clear account of unverified
boundaries.

### 9.2 Recommended scope

The MVP includes:

1. A small, documented source language.
2. A parser and type checker.
3. A typed UI IR.
4. Defined state transitions and event semantics.
5. A minimal capability/effect checker.
6. A web backend producing JavaScript and DOM behavior.
7. A test harness with deterministic event replay for the
   supported subset.
8. Machine-readable diagnostics, inspection output, and evidence
   reports.
9. A representative demo application.
10. An agent benchmark comparing the prototype with a
    conventional baseline.

The first vertical slice is web-only. The **cross-platform MVP
milestone** adds one mobile backend — preferably React Native if
the feasibility test in Phase 4 succeeds — and demonstrates the
same shared behavioral contract on web and mobile. Mobile
research is **not** postponed until after the web language is
frozen; the spike runs in parallel with Phase 1, the way
[`net.md`](https://github.com/alpibrusl/cancho/blob/main/docs/net.md)
ran outbound and inbound feasibility in cancho Core before the
implementations landed.

### 9.3 Demo application

**Suggested demo: searchable item list.**

- Text input and form validation.
- List and detail view or simple navigation.
- Asynchronous search request.
- Loading, success, empty, and error states.
- Request cancellation or stale-result protection.
- One explicitly declared capability.
- A permission-gated feature or a simulated adapter in the
  initial web prototype.
- Tests for ordinary flows and adverse event ordering.

The demo is simple enough to understand, but complex enough to
reveal whether the semantics genuinely help.

### 9.4 Out of scope for the MVP

- Replacing JavaScript, Swift, or Kotlin.
- A complete general-purpose UI framework.
- A bespoke browser engine or rendering engine.
- A new mobile operating-system runtime.
- Direct native code generation for every iOS and Android UI
  API.
- Comprehensive formal verification of the entire application
  and all dependencies.
- Automatic proof that third-party libraries are safe.
- A large component marketplace or design system.
- Full production-grade IDE support.
- Supporting every browser, OS version, device API, and
  accessibility scenario at launch.
- Claiming production readiness before security, accessibility,
  lifecycle, and compatibility testing.

---

## 10. Roadmap and milestones

The estimates below are planning ranges, not commitments. The
operating form of this section, with cancho Core dependencies
named and stop/rethink conditions explicit, is
[`ROADMAP.md`](../ROADMAP.md).

| Phase | Goal | Indicative duration |
|---|---|---|
| 0 | Design contract and feasibility | 1–2 weeks |
| 1 | Minimal web vertical slice | 2–4 weeks |
| 2 | State, effects, agent-facing verification | 2–4 weeks |
| 3 | Replayable tests and agent benchmark | 2–3 weeks |
| 4 | Mobile backend feasibility and cross-platform contract | 2–4 weeks |
| 5 | Cross-platform MVP release candidate | 2–4 weeks |
| 6 | Conditional post-MVP work | — |

Each phase has an exit criterion and a stop/rethink condition.
The Phase 0 stop condition is the most consequential: the design
must be expressible without ambiguous ordering or lifecycle
rules, the safety story must not depend on trusting arbitrary
foreign code, and the language must add complexity only with an
identifiable agent-facing benefit.

---

## 11. MVP acceptance checklist

A release candidate is not called an MVP until the applicable
checks below are satisfied. Each check names the tier
(`static | tested | runtime | trust | unknown`).

### Language and compiler

- [ ] Grammar and semantics are documented
      ([`docs/language-design.md`](language-design.md),
      [`docs/semantics.md`](semantics.md)).
- [ ] Type errors and unsupported constructs fail with actionable
      diagnostics ([`spec/diagnostics/README.md`](../spec/diagnostics/README.md)).
- [ ] IR has a documented, versioned schema
      ([`spec/ir-schema/README.md`](../spec/ir-schema/README.md)).
- [ ] Build outputs are reproducible within the documented
      environment.

### State and lifecycle

- [ ] State ownership and update rules are defined.
- [ ] Event ordering is specified for the supported subset.
- [ ] Asynchronous cancellation/stale-result behavior is tested.
- [ ] Disposed components cannot perform forbidden state updates.

### Capabilities and trust

- [ ] At least one meaningful capability is checked end to end.
- [ ] Negative tests demonstrate rejection of unauthorized
      effects ([`examples/refusal/`](../examples/refusal/)).
- [ ] Foreign-code boundaries appear in reports.
- [ ] Static checks, runtime enforcement, and tests are clearly
      distinguished.

### Agent workflow

- [ ] Core commands have stable exit codes.
- [ ] Diagnostics and reports are machine-readable.
- [ ] Agent tasks are repeatable.
- [ ] Semantic diffs show capability or behavior changes.
- [ ] Benchmark methodology and failures are published.

### Platforms

- [ ] Web demo works on the declared supported environment.
- [ ] Mobile backend decision is evidence-based
      ([`docs/platform-model.md`](platform-model.md)).
- [ ] Cross-platform MVP demonstrates the same supported
      behavioral contract on web and mobile.
- [ ] Unsupported platform features fail clearly.

### Documentation

- [ ] Clean-checkout instructions work.
- [ ] Security assumptions and limitations are explicit
      ([`docs/security-model.md`](security-model.md)).
- [ ] No claim exceeds the evidence available.

---

## 12. Key risks and mitigations

| Risk | Why it matters | Mitigation |
|---|---|---|
| Building a new syntax without real guarantees | Little advantage over existing frameworks | Specify and test semantics before expanding syntax |
| Overambitious multi-platform scope | Multiple backends multiply complexity | Web vertical slice first; mobile feasibility spike early |
| Foreign code bypasses guarantees | Most real applications need external libraries and native APIs | Explicit contracts, capability boundaries, trust reporting (Cancho Core [`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md)) |
| Agents are unfamiliar with the language | Less training data and weaker prior knowledge | Simple syntax, excellent diagnostics, structured inspection tools, familiar generated targets |
| Framework mismatch | Existing runtimes may impose different lifecycle/state rules | Conformance tests; reject unsupported behavior instead of silently adapting |
| Verification reports become marketing rather than evidence | False confidence and unsafe releases | Every report item tied to an actual check, test, or explicit assumption |
| Excessive annotation burden | Developers and agents become slower | Infer only safe facts; require declarations at meaningful authority boundaries |
| Benchmark is biased | Apparent advantage may not generalize | Matched tasks, repeated runs, conventional baseline, report negative results |

---

## 13. Strategic positioning

Cancho UI is positioned as:

> A UI language and toolchain for applications that AI agents
> can change with explicit behavioral contracts and verifiable
> evidence.

It is **not** positioned as:

- A universal replacement for all frontend and mobile
  technologies.
- A language that automatically makes AI-generated software
  correct.
- A system that proves third-party or native code safe.
- A cross-platform solution that has equivalent behavior merely
  because it shares source code.

The strongest product story is not "AI writes this language
better." It is:

> The toolchain gives AI agents a smaller, more explicit space
> in which to make changes — and gives humans evidence about
> what those changes can do.

That claim is meaningful only if the benchmark supports it.

---

## 14. Recommended next actions

1. Create the separate `cancho-ui` repository as a
   proposal/design-stage project. ← **done**
2. Write the semantics and security contract before implementing
   a broad grammar. ← **in progress (Phase 0)**
3. Define 10–20 valid and invalid examples, including effect and
   lifecycle refusal cases. ← **in progress (Phase 0)**
4. Specify the minimal typed UI IR and machine-readable
   diagnostic schema. ← **in progress (Phase 0)**
5. Implement a small web-only vertical slice with one
   representative demo.
6. Add capability checks, replayable tests, semantic diffs, and
   an agent benchmark.
7. Evaluate React Native as a mobile backend against explicit
   conformance criteria.
8. Publish a results report before committing to native code
   generation or a larger framework.

## Final recommendation

Proceed, but treat cancho-ui as a **research-backed compiler and
agent-workflow project**, not initially as a broad UI framework.

The first proof is narrow and falsifiable: an agent modifies a
small application, the compiler checks state and authority
constraints, tests replay the relevant behavior, and the evidence
report clearly identifies what is and is not guaranteed. Then
test whether the same behavioral contract can be preserved on a
mobile backend.

If that experiment shows measurable improvements over a
conventional stack, cancho-ui has a credible reason to exist. If
it does not, the useful pieces — structured diagnostics, effect
reporting, semantic diffs, and agent-oriented verification — may
still be valuable as tools around existing languages.