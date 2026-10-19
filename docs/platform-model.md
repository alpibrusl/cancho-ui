# Platform model

> **Status: Phase 0 decision record.** This document records the
> web-first decision, the mobile-backend candidates, and the
> criteria by which the mobile choice is made in Phase 4. It is
> the operating form of the proposal §6.3.

---

## 1. The web decision

The web backend is **the first backend**, and is built in
Phase 1.

**Decision.** cancho-ui compiles to JavaScript + DOM, on top of
the real browser APIs. No JSX, no virtual-DOM library, no
React-or-Svelte-shaped abstraction.

**Reasons.**

1. **The browser is already a JavaScript environment.** DOM
   operations are JavaScript. Every existing UI toolchain on
   the web produces JavaScript. Compiling to anything else
   means a polyfill or a runtime that re-implements the DOM,
   which is the proposal's §3.2 anti-pattern.
2. **The reachable surface is what we already reason about.**
   `cancho-ui inspect effects` names the JavaScript function
   names, `cancho-ui report` lists the trust boundaries. The
   same shape Cancho Core uses for its
   [`authority --output json`](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md)
   report.
3. **The agent-facing surface is a known shape.** Browsers
   expose DevTools, screen readers, accessibility inspection.
   cancho-ui's accessibility metadata (Question 9 of
   [`docs/semantics.md`](semantics.md)) lowers directly to ARIA
   attributes, and a reviewer can verify them with the browser's
   own accessibility tree.

**What the web backend delivers.**

- Compilation of a cancho-ui `view` block to direct DOM
  construction.
- Compilation of a `view` block's data bindings to update
  operations on existing DOM nodes.
- An event loop adapter that converts DOM events into cancho-ui
  events.
- The four Phase 1 platform adapters: `Network`, `Storage`,
  `Clock`, `Location`. **Corrected 2026-10-19 (measured in the
  emitted code):** only `Clock` and `Network` exist, and both are
  stubs (`Network` is `net.search`, a simulated search over a fixed
  corpus, not `fetch`); `Storage` and `Location` are not written. **Updated 2026-10-19: `Storage` is
  now built** as the builtins `storage.get` / `storage.set` over `localStorage`
  ([`adapters.md`](adapters.md)); `Location` and a real `Network` are not.
  See [`mobile-backend.md`](mobile-backend.md) §2.
- Source maps for the cancho-ui → JavaScript lowering.
- The full evidence report (`cancho-ui report`) with the five
  tiers named.

**What the web backend does not deliver.**

- A JSX-like HTML shorthand.
- A CSS-in-language story (CSS is external in Phase 1; a Phase 6
  follow-up may add `style` blocks).
- A virtual DOM. cancho-ui compiles to direct DOM ops because a
  virtual DOM is exactly the kind of framework-specific
  abstraction the proposal §3.2 rejects.
- A polyfill for legacy browsers. The supported target is set
  in [`docs/platform-model.md`](#supported-targets) below.

---

## 2. The mobile decision (deferred to Phase 4)

The mobile backend is **not** built in Phase 1. The mobile
decision is the deliverable of Phase 4, recorded here so a
reader can see the criteria now and the choice later.

### 2.1 The candidates

The four candidates from the proposal §6.3, each with a one-line
characterization and the criterion it would be judged on.

| Candidate | What it is | The criterion |
|---|---|---|
| **React Native** | JavaScript runtime + native UI components | Preserves the cancho-ui state/event/capability/lifecycle semantics |
| **Other established cross-platform runtime** | A different VM + UI bridge | Better semantic preservation than RN, OR lower maintenance cost |
| **Direct Swift / Kotlin codegen** | Native UI from a cancho-ui IR | Worth the build and maintenance cost over a runtime |
| **WebView shell** | A native wrapper around the web backend | Honest about being web-rendered, not native |

### 2.2 The decision criteria (Phase 4)

The Phase 4 spike compares each candidate on:

1. **Semantic preservation.** Does the shared
   state/event/effect/capability/lifecycle spec produce
   equivalent observable behavior for the supported flows?
   Same fixture, both backends, byte-identical trace.
2. **Trust-boundary cost.** How many foreign calls does each
   candidate introduce? The evidence report on each side is
   compared; the one with fewer or narrower trust boundaries
   wins.
3. **Maintenance cost.** How many distinct platforms does the
   candidate add to the test matrix? How many bespoke native
   modules must be written?
4. **Performance.** Measured for the supported subset, with the
   comparison the same way Cancho Core measures
   [`against-c-and-rust.md`](https://github.com/alpibrusl/cancho/blob/main/docs/against-c-and-rust.md).

### 2.3 The default position if Phase 4 is inconclusive

If no candidate clearly wins, the **WebView shell** is the
default. It is honest about being web-rendered; it preserves
the cancho-ui semantics for the supported subset because the
web backend is already the supported subset; it does not
introduce a new trust boundary at the UI layer.

**Stop / rethink.** If the WebView shell's UX is unacceptable
for the target users (a real, measured user complaint, not a
taste-based one), the decision is escalated. Direct native
codegen is then the next move — with the warning that it
multiplies the test matrix and the maintenance cost.

### 2.4 What Phase 4 must not do

- Pick a mobile backend without measuring the trust-boundary
  cost.
- Promise native performance for a runtime that is not native.
- Promise semantic equivalence that is not proved by a shared
  fixture, both backends, byte-identical trace.

---

## 3. The decision record so far

| Date | Decision | Tier | Reversible? |
|---|---|---|---|
| 2026-10-17 | Web is the first backend | Phase 0 | Yes (pre-Phase 2) |
| 2026-10-17 | Mobile is Phase 4, not Phase 1 | Phase 0 | Yes (pre-Phase 4) |
| 2026-10-17 | No JSX, no virtual-DOM library | Phase 0 | No |
| 2026-10-17 | CSS is external in Phase 1 | Phase 0 | Yes (Phase 6) |
| 2026-10-17 | Mobile decision deferred to Phase 4 spike | Phase 0 | Yes (Phase 4) |
| TBD | Mobile backend chosen | Phase 4 | — |
| TBD | WebView shell as fallback | Phase 4 (default) | Yes (Phase 4) |

---

## 4. Supported targets (Phase 1)

The Phase 1 supported target matrix.

| Target | Phase | Notes |
|---|---|---|
| **`--target web`** | 1 | Chrome ≥ 120, Firefox ≥ 120, Safari ≥ 17 (TBD with measurement) |
| **`--target web-es2022`** | 1 | Same browsers, no top-level await (the default if measurement shows it's needed) |
| **`--target mobile`** | 4 | Chosen in Phase 4 |
| **`--target mobile-webview`** | 4 (fallback) | WebView shell |

### 4.1 What is not supported in Phase 1

- Internet Explorer (any version).
- Browsers older than the supported version.
- Server-side rendering. The Phase 1 contract is "compile to
  JavaScript that runs in a browser."
- React Native. Phase 4 only.

A program that depends on an unsupported capability is refused
with `unsupported-on-target`, the same refusal Cancho Core uses
for its `wasm32-wasip1` work
([`docs/wasm.md`](https://github.com/alpibrusl/cancho/blob/main/docs/wasm.md)).

---

## 5. The platform-adapter contract

A platform adapter is a small cancho-ui file that wraps a
platform API. The contract is in
[`docs/security-model.md`](security-model.md) §3.3; this section
is the operating form.

### 5.1 Phase 1 adapters

| Capability | Adapter | Foreign function |
|---|---|---|
| `Network` | `net.request` | `fetch` from `"browser"` |
| `Storage` | `storage.get`, `storage.set` | **built as builtins, not `foreign`** (`localStorage.getItem` / `setItem`, key prefix `cui:`): [`adapters.md`](adapters.md) |
| `Clock` | `clock.now` | `Date.now` from `"browser"` |
| `Location` | `location.current` | `navigator.geolocation.getCurrentPosition` from `"browser:location"` |

Each adapter is the only cancho-ui surface that crosses the
named boundary. A program that calls the underlying foreign
function directly is refused with `foreign-interface-missing`.

### 5.2 Phase 4 mobile adapters

Each mobile adapter declares its own foreign functions and the
permission it requires from the platform (e.g. iOS's
`NSLocationWhenInUseUsageDescription`). The Phase 4 spike
writes the iOS and Android adapters for the four Phase 1
capabilities first.

---

## 6. The build pipeline

The Phase 1 build pipeline:

```
cancho-ui build --target web <file.cui>
   |
   v
   parser
   |
   v
   type checker (capability, effect, accessibility, foreign)
   |
   v
   typed UI IR (see spec/ir-schema/README.md)
   |
   v
   web backend
   |
   v
   JavaScript + DOM + adapter imports
   |
   v
   optional: bundler (esbuild or rollup) for production builds
```

Phase 2 adds the test harness (`cancho-ui test`) and the
inspect commands (`cancho-ui inspect component`, `cancho-ui
inspect effects`). Phase 3 adds the replay harness and
`cancho-ui diff`.

### 6.1 The bundler

The Phase 1 build emits ES modules. A separate `cancho-ui
bundle` command (Phase 6 or Phase 1's last exit criterion)
runs `esbuild` or `rollup` for production builds.

The bundler is **not** the build pipeline; cancho-ui's
responsibility ends at emitting valid ES modules. The bundler
choice is up to the deployment.

---

## 7. The supported-browser measurement

The "Chrome ≥ 120, Firefox ≥ 120, Safari ≥ 17" line in §4 is a
placeholder. Phase 1 measures which features cancho-ui's
emitted JavaScript actually depends on (top-level await,
private class fields, AbortController, etc.) and the supported
target matrix is the answer.

**Source.** Cancho Core's
[`docs/wasm.md`](https://github.com/alpibrusl/cancho/blob/main/docs/wasm.md)
does this measurement for `wasm32-wasip1`, with a per-feature
matrix and a per-fixture pass/refused/wrong count.

Phase 1 lands a measurement of the same shape for the web
backend. The matrix is recorded in `eval/e1_supported_browsers.md`
once a Phase 1 fixture set is available.

---

## 8. What is deliberately not in the platform model

- A native iOS UI from cancho-ui source. Phase 6 only, gated
  on Phase 4 evidence.
- A native Android UI from cancho-ui source. Same.
- A WebGPU backend. Out of scope; the proposal's §9.4.
- A server-side rendering backend. Phase 6 if asked; Phase 1
  refuses `ssr` with `unsupported-on-target`.
- A cross-platform WebGL/WebGPU abstraction. Out of scope;
  per-effect adapters for canvas and WebGPU can land in Phase 6.

Each "not in the platform model" line is a refusal to claim
what the implementation does not deliver.