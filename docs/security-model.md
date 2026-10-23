# Security model

> **Status: Phase 0.** This document is the contract the Phase 1
> compiler and runtime must enforce. It defines the five
> guarantee tiers, the capability/effect rules, the trust-boundary
> contract, and the refusal tests that exercise them.

---

## 1. The five guarantee tiers

cancho-ui never describes ordinary compilation or test success as
proof of complete correctness. Every guarantee is one of five
tiers, and the tier is named wherever the guarantee is invoked.

| Tier | What it means | Enforced by | Example |
|---|---|---|---|
| **static** | Compiled-time guarantee under documented assumptions | Compiler / verifier | Capability declaration reachability |
| **tested** | Exercised by a named test or test suite | Test harness | Cancellation semantics on dispose |
| **runtime** | Checked at execution time | Runtime / platform adapter | Capability grant / denial |
| **trust** | External or native; not verified by cancho-ui | An external party | Foreign JS implementation |
| **unknown** | Not yet established | — | Per-platform accessibility semantics |

The Cancho Core analogue is the
[`authority --output json`](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md)
report's `"bounded": false` for any program reaching foreign
code: a supervisor that reads nothing else refuses by default.

### 1.1 Where each tier appears

- **Static** — in the compiler's check phase and in the IR's
  invariant checks. A passing `cancho-ui check` reports every
  static check that ran, by rule.
- **Tested** — in the test harness. A passing `cancho-ui test`
  reports which tests ran and which failed, by name.
- **Runtime** — in the runtime and platform adapter. A
  `cancho-ui report` distinguishes what was checked at
  runtime from what was statically proved.
- **Trust** — in the evidence report. Every trust boundary is
  named, not hidden.
- **Unknown** — anywhere the implementation has not been
  measured. A claim that is `unknown` is acknowledged as such,
  not claimed as `static`.

### 1.2 What is never claimed

- A `static` guarantee about runtime behavior.
- A `tested` guarantee about untested inputs.
- A `runtime` guarantee about static correctness.
- A `trust` guarantee about any behavior of the trusted party.
- An `unknown` guarantee at all.

The discipline is the same as Cancho Core's
[`defined-behaviour.md`](https://github.com/alpibrusl/cancho/blob/main/docs/defined-behaviour.md):
"every C hole named and closed."

---

## 2. The capability model

A capability is **authority required to perform an effect**. A
component declares the capabilities it needs (`requires
Capability`); the compiler checks the declaration matches the
reachable use; the runtime checks the declaration was granted.

### 2.1 The Phase 0 vocabulary

The Phase 1 vocabulary is the smallest set a searchable-list
demo needs. Phase 2+ adds more, each with its own design entry.

| Capability | What it grants | Tier |
|---|---|---|
| **Network** | Outbound HTTP requests, and the Server-Sent Events stream of `subscribe` (`streams.md`); WebSocket is not built | static + runtime |
| **Storage** | Read/write key-value storage on the device | static + runtime |
| **Clock** | Read the current time | static + runtime |
| **Location** | Read the device location | static + runtime |

Each capability is declared once in this document, named
exactly as it appears in source (`Network`, `Storage`, `Clock`,
`Location`), and exposed to the program through a Phase 1
platform adapter (see [`docs/platform-model.md`](platform-model.md)).

### 2.2 The effect model

An effect is **a borrowed capability**. A component declares the
capabilities it needs; transitions and view blocks borrow those
capabilities for the duration of the call; the borrow is
discharged when the call returns.

**Tier.** **static** (declaration, reachability) + **runtime**
(grant / denial).

**Source.** Cancho Core's
[`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md#3-effects):
"an effect *is* a borrowed capability."

**The exact-row rule.** A capability declaration is exact in both
directions:

- A transition body that performs a capability not declared on
  the enclosing component: refused with `capability-missing`.
- A capability declared on the enclosing component with no
  reachable use: refused with `capability-unused`.

The same rule Cancho Core applies to function effect rows. The
cancho-ui equivalent is the per-component row, derived from the
union of its transitions' rows.

### 2.3 Capability narrowing (Phase 2+)

> **Built 2026-10-20 for `Network` only, with a different meaning from the one sketched below** (the bare
> capability is already the page's own origin, so a bound *adds* hosts): [`adapters.md`](adapters.md) section 10.
> `Location("coarse" | "fine")` is built the same way (the bare form is the coarse one, a bound asks for more); `Storage("ns")` is not.

Phase 2 refines the capability with narrowing, the same way Cancho
Core narrows `Fs(prefix)` and `Net(bound)`:

- `Network("api.example.com")` — restrict outbound requests to
  one host.
- `Storage("user-prefs")` — restrict storage reads to one
  namespace.
- `Clock` is not narrowed (no equivalent).
- `Location` is narrowed by precision
  (`Location(coarse)` vs `Location(fine)`).

Phase 2 refusal: `capability-narrowing-illegal` when the
narrowed literal is not a string literal at compile time, or
when the narrowing is structurally narrower than the capability
itself.

**Phase 1.** No narrowing. The four capabilities above are
declared and checked as a unit. Phase 2+ refines.

---

## 3. The trust-boundary contract

A trust boundary is a place where cancho-ui's guarantees end.
Every cross-boundary is declared, named, and reported.

### 3.1 The Phase 0 boundaries

| Boundary | What crosses it | Tier | Reported in evidence? |
|---|---|---|---|
| **JavaScript call** | A `foreign` function's body | trust | yes |
| **Browser DOM API** | Anything reached through `window` / `document` | trust | yes |
| **Native module** | A platform adapter's underlying native call | trust | yes |
| **Network** | Bytes sent and received | trust (transport) + runtime (caller's grant) | yes |
| **Storage** | Bytes read and written | trust (transport) + runtime (caller's grant) | yes |
| **Third-party package** | Anything in `node_modules` or equivalent | trust | yes |

The Phase 0 contract: **every boundary is named, not hidden**.
The implementation does not erase a boundary by typing it; the
evidence report surfaces it.

**Source.** Cancho Core's [`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md).
A foreign function with no capability was accepted, ran a shell,
and reported `bounded: true` — the hole was closed by requiring
a declared capability at the call site. The same mechanism
applies to cancho-ui's `foreign` blocks.

### 3.2 The trust-boundary rule

> **A typed wrapper does not prove the wrapped implementation
> safe.**

A foreign function whose body is JavaScript is **trust**, not
`static`, regardless of its declared type. The evidence report
names it as `trust: foreign-javascript`, the same way the cancho
authority report names `bounded: false` for any program reaching
foreign code.

### 3.3 The platform-adapter contract

A platform adapter is a small cancho-ui file that:

1. Declares the capability it requires (e.g. `requires Network`).
2. Declares the foreign functions it wraps
   (`foreign { fn fetch ... } from "browser"`).
3. Exposes a cancho-ui surface (e.g. `net.request`) that the
   rest of the program uses.

The adapter is the **only** cancho-ui surface that crosses a
boundary. A program that calls `window.fetch` directly, without
an adapter, is refused with `foreign-interface-missing`.

**Source.** Cancho Core's
[`packages/net-sockets/`](https://github.com/alpibrusl/cancho/tree/main/packages/net-sockets),
[`packages/net-connect/`](https://github.com/alpibrusl/cancho/tree/main/packages/net-connect),
[`packages/http-request/`](https://github.com/alpibrusl/cancho/tree/main/packages/http-request) —
each is a small file that turns an `extern fn` into a typed
operation on a capability.

---

## 4. The refusal tests

The Phase 1 refusal tests live in
[`examples/refusal/`](../examples/refusal/) (Phase 0: written
down, not compiled). Each refusal rule has at least one fixture
that must be refused with that rule.

| Rule | Fixture |
|---|---|
| `state-unused` | A component with a `state` declared but never read or updated |
| `state-after-dispose` | A transition that updates state in a disposed component |
| `state-escapes` | A component that returns its owned state as an owned value to a caller |
| `effect-not-declared` | A transition that calls an effect the component has not declared |
| `state-not-owned` | A transition that updates a state field not declared on the component |
| `test-asserts-unspecified` | A test that asserts an ordering the language does not guarantee |
| `match-non-exhaustive` | A `match` on an effect result missing an `Err` or `Cancelled` arm |
| `capability-missing` | A component that uses `net.request` without `requires Network` |
| `capability-unused` | A component with `requires Network` but no reachable network use |
| `capability-insufficient` | A component with `requires Network` calling `net.request.whenAuthenticated` |
| `accessibility-role-undefined` | A view primitive without a `role` |
| `accessibility-label-empty` | An interactive primitive with empty `label` |
| `accessibility-keyboard-missing` | An interactive primitive without a keyboard action for Enter |
| `unsupported-on-target` | A `target mobile { ... }` block built for `--target web` |
| `target-not-in-build` | A `target` block for a target not in the build configuration |
| `foreign-scope-missing` | A `foreign` function without a `from "scope"` |
| `foreign-interface-missing` | A program that calls `window.fetch` directly without an adapter |
| `name-reserved` | A binding named `component`, `state`, `view`, etc. |
| `evidence-over-claim` | A report claiming a guarantee the implementation does not enforce |

Each fixture's header declares its expected diagnostic code, the
same way Cancho Core's reject fixtures use `//~ RULE
<rule-name>`. Phase 1 expands the suite as the compiler and
runtime land.

---

## 5. The evidence report

`cancho-ui report` produces an evidence report. The report is
the program reviewer's source of truth: it names what was
checked, by what, with what outcome, and what was trusted.

### 5.1 The five sections

The report has five sections, one per tier:

1. **Static checks.** Every rule the compiler enforced; every
   refusal it produced; the rule tags.
2. **Tested behaviors.** Every test that ran; the result of each;
   the names of the fixtures.
3. **Runtime checks.** Every runtime assertion that fired during
   the test runs; what it caught.
4. **Trust boundaries.** Every cross-boundary call; the
   `from "scope"`; the implementation's `tier: trust`.
5. **Unknown.** Everything not covered above, named with the
   reason.

A reviewer who reads the five sections sees what was and was not
guaranteed, and can decide whether the program is fit for the
purpose they have in mind.

### 5.2 What the report never claims

- That a `trust` boundary was verified by cancho-ui.
- That a `tested` behavior is exhaustive over untested inputs.
- That a `static` check was exhaustive over all source paths
  (a `static` check is what the source *proves*; untested code
  paths are `unknown`).
- That a `runtime` check was checked at all when the
  application did not exercise it.

The same discipline Cancho Core applies to its authority
report: the report names what is bounded, refuses what is not,
and corrects a wrong number in place when the implementation
outgrows it.

### 5.3 The `--output json` shape

The JSON form of the report is the contract the supervisor (and
an AI agent) reads. The shape is in [`spec/diagnostics/README.md`](../spec/diagnostics/README.md)
§3 — every report section is a list of structured objects with
stable field names.

---

## 6. The static check budget

A static check that costs more than the program it checks is
a check the developer will turn off. The check cost must be
measured and reported, the same way Cancho Core's
[`check-cost.md`](https://github.com/alpibrusl/cancho/blob/main/docs/check-cost.md)
measures every check.

**Phase 0 commitment.** Every Phase 1 static check has a
measured cost on the searchable-list fixture. Phase 2 expands the
budget to every Phase 2 check.

**What is not in the budget.** A check that *cannot* be done
statically (Question 8 in [`docs/semantics.md`](semantics.md))
is not budgeted as static; it is `runtime`.

---

## 7. What is deliberately not in Phase 0

- **A formal verification of the whole application.** Out of
  scope for the MVP per proposal §9.4.
- **Proof that third-party libraries are safe.** Trust
  boundaries, per §3.
- **Sandboxing at the OS level.** The platform's own
  permissions model grants / denies at runtime.
- **Defense against a malicious compiler.** The compiler is
  trusted; its output is verified against its input by the test
  harness.

Each "not in Phase 0" line is also a refusal to claim a
guarantee the implementation does not have.