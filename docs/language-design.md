# Language design

> **Status: Phase 0 sketch.** This document records the design
> decisions behind the syntax in [`docs/proposal.md`](proposal.md)
> §5.2. None of it is implemented. Phase 1 lands the parser and the
> type checker; this document is the contract they must meet.
>
> Every decision cites the Cancho Core document it inherits from,
> where one exists, or labels itself as net-new with the source
> that backs it.

---

## 1. Position

Cancho-ui's source language is designed for two audiences
simultaneously:

1. **AI agents**, who read and write the language to make changes
   to an application, and who must learn its surface cheaply.
2. **Humans**, who review the changes, and who must understand
   the consequences without reading framework-specific runtime
   code.

Both audiences need a language that is **readable, has a small
vocabulary, and makes authority and behavior explicit**. The same
discipline Cancho Core applies to its systems language —
"a diagnostic is a refusal" — is the discipline this document
inherits.

The language is **not**:

- A JavaScript or TypeScript dialect.
- A superset of an existing frontend language.
- A domain-specific markup language for designers.

It is a new declarative language whose programs compile to one or
more established platform targets.

---

## 2. The minimal vocabulary

The Phase 0 vocabulary is the smallest set of concepts a UI
language can have without being a thin syntax layer over an
existing framework. Every concept below is in
[`docs/semantics.md`](semantics.md) with its guarantee tier.

| Concept | What it is | Source |
|---|---|---|
| **Component** | Named, composable UI unit with typed properties, state, view, events, lifecycle | New for UI |
| **Property** | Typed input to a component | Inherits Cancho Core's type system ([`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)) |
| **State** | Explicitly owned state with defined update rules | Inherits Cancho Core's `res`/`val` distinction |
| **Event** | User or platform event that triggers a transition | New for UI |
| **Effect** | Asynchronous work or interaction with the outside world | Inherits Cancho Core's effect rows |
| **Capability** | Authority required for an effect | Inherits Cancho Core's capability model |
| **Lifecycle** | Defined behavior on active, inactive, dispose | New for UI (the system-language equivalent is `release`) |
| **Platform adapter** | Explicit interface to a browser/native API | Inherits the Cancho Core foreign-interface contract ([`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md)) |
| **Stable identity** | Deterministic ID for a component or event | Inherits Cancho Core's `cancho ids` ([`canonical-ast.md`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)) |

Phase 1 may add a small number of additional concepts (e.g.
"binding" for two-way data flow, "slot" for child composition),
each with its own design entry in this document and its own
guarantee tier in semantics.md.

---

## 3. The illustrative syntax

The proposal §5.2 sketch is the starting point. Three decisions
drive the sketch; each is recorded here so a future reader can
see why the syntax is the way it is.

### 3.1 `component` declares a unit

A `component` block is the only way to introduce a UI unit. The
keyword is deliberate:

- It is one word, lowercase.
- It is not `class`, `struct`, or `function` — the three would
  carry framework-specific baggage.
- The block is closed with `}` and is the unit of composition.

```text
component Counter { ... }
```

The keyword reserves the name `component` everywhere in the
language; using it as a state, event, or capability name is
refused with `name-reserved`.

### 3.2 `state` declares owned data, `requires` declares authority

Two keywords, two different ideas. `state` is about ownership; a
component owns its state, exactly as a Cancho Core function owns
its `res` values. `requires` is about authority; a component must
list every capability it needs, the same way a Cancho Core
function's effect row lists what it borrows.

```text
component NearbySearch {
    requires Network
    requires LocationWhenEnabled

    state query: String = ""
    state results: List<Result> = []
    state status: SearchStatus = Idle
    ...
}
```

A component that uses an effect without listing the capability is
refused with `capability-missing`. A component that lists a
capability it never uses is refused with `capability-unused`,
because an over-declared capability makes the authority report
unreliable — the same reasoning Cancho Core applies to
"narrow the body, never widen the row"
([`linearity-and-effects.md §3.1`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)).

### 3.3 `on Event` is a transition, `view` is a description

Two more keywords:

- `on Event { ... }` — a transition handler. The block is run
  when the named event fires; the body is the only place state
  can be updated. A transition that violates the state machine
  defined in [`docs/semantics.md`](semantics.md) §2 is refused
  statically when possible, trapped at runtime otherwise.
- `view { ... }` — a *description* of the rendered tree. The
  body is declarative; it does not run imperatively. The Phase 1
  web backend lowers `view` blocks to direct DOM construction.

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

The decision to call the block `view` rather than `render` is
intentional: `render` implies imperative output, `view` is a
description of what is shown. The semantic difference matters
for the IR — a `view` block is data, not code — and the
[Cancho Core `docs/canonical-ast.md`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)
distinction between canonical tree and executed code is the
precedent.

---

## 4. Naming, identifiers, modules

The Phase 0 rules:

- **Components:** `PascalCase`.
- **State, events, effects, capabilities, helpers, foreign
  bindings:** `snake_case`.
- **Module paths:** `lower.case`. The Cancho Core convention
  ([`canonical-ast.md`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)).
- **File extension:** `.cui` (proposed; lands with Phase 1).
- **Reserved names:** `component`, `state`, `view`, `on`,
  `requires`, `effect`, `capability`, `foreign`, `module`,
  `import`, `pub`. Using any as a binding name is refused with
  `name-reserved`.
- **Builtins:** none at the language level in Phase 0. The web
  backend exposes DOM operations as platform adapters (see
  §7), not as language-level primitives.

A file can contain more than one `component`; the unit of identity
for hashing is each declaration, the same way Cancho Core's
`cancho ids` hashes each declaration in a file rather than the
file as a whole.

---

## 5. Modules and imports

`module` and `import` follow Cancho Core's
[`docs/modules.md`](https://github.com/alpibrusl/cancho/blob/main/docs/modules.md)
shape: a module is a namespace, not a trust boundary.

```text
module search

import net            // brings `net.request` into scope
import std.io as io   // namespaced under `io`

component ResultsList {
    requires Network

    state items: List<Item> = []

    on Load {
        items = net.request("GET", "/api/items").await()
    }

    view { ... }
}
```

Phase 0 does not specify the module resolution algorithm; Phase 1
inherits Cancho Core's `cancho-vcs` and package system when those
are reused for cancho-ui packages. Until then, Phase 0 modules
are flat (one directory, one file = one module).

---

## 6. Foreign interfaces

Every effect that crosses the cancho-ui boundary — calls into
JavaScript, into a browser API, into a native module — is
declared at the call site as a foreign interface. The shape
follows Cancho Core's [`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md):

```text
foreign {
    fn fetch_text(url: String) -> Promise[String] from "browser"
    fn local_storage_get(key: String) -> ?String from "browser:storage"
}
```

The `from "scope"` annotation is the capability. A function that
is declared but never imported is reported as
`foreign-declared-unused`. A call to a foreign function without
a declared interface is refused with `foreign-interface-missing`.

The IR carries every foreign declaration so the evidence report
can name each one as a trust boundary.

---

## 7. Platform adapters

A platform adapter is a cancho-ui component that wraps a
specific runtime API. Adapters are declared, not inferred — a
program that uses `fetch` does so through an adapter, never
through a raw global.

Phase 0 names the shape; Phase 1 implements the web backend's
adapter for `Network`, `Storage`, `Location`, and `Clock`. Each
adapter is a thin file that:

1. Declares the capability it requires (e.g. `requires Network`).
2. Declares the foreign functions it wraps
   (`foreign { fn fetch ... }`).
3. Exposes a cancho-ui surface (e.g. `net.request`) that the rest
   of the program uses.

The shape is the same as Cancho Core's packages:
[`packages/net-sockets/`](https://github.com/alpibrusl/cancho/tree/main/packages/net-sockets),
[`packages/net-connect/`](https://github.com/alpibrusl/cancho/tree/main/packages/net-connect),
[`packages/http-request/`](https://github.com/alpibrusl/cancho/tree/main/packages/http-request) —
each is a small file that turns an `extern fn` into a typed
operation on a capability.

---

## 8. View tree primitives

The Phase 0 view tree has the smallest set of primitives a
searchable-list demo needs. Each primitive is a cancho-ui
component itself; the built-in list is not a special language
form.

| Primitive | Phase | Notes |
|---|---|---|
| `Column` | 1 | Vertical layout |
| `Row` | 1 | Horizontal layout |
| `Text` | 1 | Static or interpolated text |
| `Button` | 1 | Triggers an event |
| `TextField` | 1 | Bound to a state value |
| `List` | 2 | Indexed over a state list |
| `Image` | 2 | With `alt` text |
| `NavLink` | 3 | Cross-screen navigation |

Each primitive has accessibility metadata (role, label, focus
order) declared at the language level, not inferred from CSS or
markup. The web backend lowers them to ARIA attributes; the
mobile backend lowers them to platform-native accessibility
roles.

A primitive missing accessibility metadata is refused with
`accessibility-missing`. A primitive whose accessibility
metadata disagrees with the role it implies is refused with
`accessibility-inconsistent`.

---

## 9. What is deliberately not in the syntax

- **No JSX, no template literals.** The view tree is a structured
  block, not a string of HTML.
- **No CSS-in-language.** Styles are external to the language in
  Phase 1; a Phase 6 follow-up may add `style` blocks.
- **No `props.spread`.** Composition is explicit.
- **No `key` prop.** Stable identity is per-declaration, per the
  hash; reconciliation uses the identity, not a developer-supplied
  string.
- **No `ref` or imperative handle.** Components do not expose
  imperative handles; effects are declared, not called imperatively.
- **No class component / function component distinction.** There
  is only `component`.
- **No higher-order components or render props.** Composition is
  via children slots, added in Phase 2 if a real asker appears.
- **No macros.** Same reason as Cancho Core: macros break stable
  content-addressable identity.

Each "not in the syntax" line above is a deliberate scope cut.
Adding any of them back requires a real asker in the corpus, the
same two-asker bar Cancho Core applies
([`CONTRIBUTING.md`](https://github.com/alpibrusl/cancho/blob/main/CONTRIBUTING.md)).

---

## 10. Decision log

| # | Decision | Reason | Reversible? |
|---|---|---|---|
| 1 | `component` is a keyword, not a context-dependent identifier | Removes a class of parser ambiguities; makes the language easier to read | Yes (pre-Phase 2) |
| 2 | `requires` declares capability at the component level | Mirrors Cancho Core's effect row; authority is visible in the syntax | Yes (pre-Phase 2) |
| 3 | `view { ... }` is data, not code | Keeps the IR small and the canonical tree serializable | No |
| 4 | File extension `.cui` | Distinct from `.cho`, `.ts`, `.tsx`, `.js`, `.jsx` | Yes (pre-Phase 2) |
| 5 | Foreign interfaces are declared, not inferred | Closes the same hole Cancho Core closed with `Ffi(lib)` | No |
| 6 | No JSX, no CSS-in-language | Keeps the language small and the IR canonical | Yes (Phase 6 only) |
| 7 | Accessibility metadata is language-level, not CSS-level | Accessibility is a property of the program, not of its styling | No |
| 8 | No macros | Macros break stable content-addressable identity | No |
| 9 | No higher-order components in Phase 0 | One way to do composition; revisit when an asker arrives | Yes |
| 10 | Modules are flat in Phase 0 | The package system is a Phase 1+ concern | Yes (Phase 1+) |

A "Yes (pre-Phase 2)" cell means the decision can still be
changed before Phase 2 ships. Once a syntax rule lands in the
parser, every fixture that exercises it locks the rule in place
the same way Cancho Core's reject fixtures lock its refusal
rules.

---

## 11. What Phase 1 must add to this sketch

This document is the contract Phase 1 lands against. The Phase 1
delta is small and explicit:

- A parser producing the typed AST (the source of the canonical
  hash).
- The reserved-name set in §4 enforced.
- The capability-usage rules in §3.2 enforced
  (`capability-missing`, `capability-unused`).
- The state-transition rules in [`docs/semantics.md`](semantics.md)
  §2 enforced for the supported subset.
- The accessibility-metadata rules in §8 enforced for the view
  primitives.
- The foreign-interface rules in §6 enforced.

Anything else is Phase 2+ and is not part of this contract.
Phase 1 does not add a syntax rule that is not in this document.