# IR schema sketch

> **Status: Phase 0 sketch.** This document describes the typed
> UI IR in enough fidelity that the Phase 1 backend cannot back
> itself into a corner in two weeks. It is not yet a formal
> schema — Phase 1 produces one and pins a version number.

---

## 1. Position

The IR sits between the language's semantic checks and the
backend's code generation. It is:

- **Typed** — every node carries its type, no inferred-`any`.
- **Canonical** — its serialization is deterministic per
  declaration, the way Cancho Core's
  [`canonical-ast.md`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)
  is.
- **Versioned** — the schema has a version number, and a
  consumer that does not understand the version is refused.
- **Source-mapped** — every node carries a span back to the
  source file.

The IR is the artifact every cancho-ui tool reads. `cancho-ui
inspect component`, `cancho-ui inspect effects`, `cancho-ui
diff`, and `cancho-ui report` all read the IR, not the source.

---

## 2. The top-level shape

The IR is a sequence of **modules**, each containing
**declarations**. A declaration is one of:

- `Component` — a UI unit.
- `PlatformAdapter` — a foreign interface wrapped as a
  capability.
- `Foreign` — a single foreign declaration (no body).
- `Module` — a nested namespace (Phase 1 limits nesting to
  one).

```text
Module {
    name: String,
    span: Span,
    declarations: Vec<Declaration>,
}

Declaration = Component | PlatformAdapter | Foreign
```

Every declaration carries a **stable ID** — the content hash of
its canonical text plus its enclosing module's path. The hash
follows Cancho Core's [`cancho ids`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)
shape; per-declaration, not per-file.

---

## 3. `Component`

The shape a UI unit lowers to.

```text
Component {
    id: StableId,
    name: String,                       // PascalCase
    span: Span,
    properties: Vec<Property>,
    state: Vec<StateField>,
    requires: Vec<CapabilityRef>,       // declared capabilities
    events: Vec<EventHandler>,
    schedules: Vec<Schedule>,           // `start E;` / `every N E;` (docs/schedules.md); [{kind, event, period_ms?, span}]
    view: ViewBlock,
    lifecycle: LifecycleHooks,
    target_overrides: Vec<TargetOverride>,
}

Property {
    name: String,
    type: Type,
    optional: bool,
    default: Option<Expr>,
}

StateField {
    name: String,
    type: Type,
    initial: Expr,
}

CapabilityRef {
    name: String,                       // "Network", "Storage", ...
    narrowing: Option<NarrowingLiteral>,
}

EventHandler {
    event: EventRef,
    body: StmtBlock,
    reachable_effects: EffectRow,       // for capability checking
    reachable_state: BTreeSet<StateFieldRef>,
}

ViewBlock {
    body: ViewNode,                     // the root
    bindings: Vec<Binding>,
    accessibility: AccessibilityMetadata,
}

Binding {
    target: BindingTarget,             // a state field, a property
    source: BindingSource,              // an event, an effect result
    strategy: BindingStrategy,          // OneWay | TwoWay
}
```

### 3.1 What `Component` carries that no source carries

- `reachable_effects` — the union of effect rows of the
  component's transition handlers, used by the capability
  checker.
- `reachable_state` — the set of state fields any handler can
  update, used by `state-not-owned` (Question 2).
- `id` — the stable hash, used by every tool that wants to
  reason about a specific component.

These fields are **derived during the semantic-check phase**;
the IR is the output of that derivation.

---

## 4. `PlatformAdapter`

The shape a platform adapter lowers to.

```text
PlatformAdapter {
    id: StableId,
    name: String,                       // lower.case.module.path
    span: Span,
    requires: Vec<CapabilityRef>,       // capabilities the adapter needs
    foreign: Vec<ForeignDecl>,
    surface: Vec<FnDecl>,               // cancho-ui surface it exposes
}

ForeignDecl {
    name: String,
    signature: FnSignature,
    from: String,                       // "browser", "browser:storage"
    promise_behavior: PromiseBehavior,  // for async returns
}

FnDecl {
    name: String,
    signature: FnSignature,
    body: StmtBlock,
    reachable_effects: EffectRow,
}
```

A `PlatformAdapter`'s `foreign` block names every trust
boundary the adapter crosses. The evidence report iterates the
adapter's `foreign` field, not the source's, so the report is
always derivable from the IR.

---

## 5. `Foreign`

The shape a bare foreign declaration lowers to. Used for
declarations that are not part of a `PlatformAdapter` (Phase 0
allows them; Phase 1 may tighten the rules to require an
adapter).

```text
Foreign {
    id: StableId,
    name: String,
    signature: FnSignature,
    from: String,
    promise_behavior: PromiseBehavior,
    span: Span,
}
```

A `Foreign` without a `PlatformAdapter` is reported in the
evidence report as a trust boundary not gated by an adapter.
Phase 1 may refuse such declarations with
`foreign-interface-missing` for the supported subset.

---

## 6. The view tree

The `ViewBlock`'s body is a tree of `ViewNode`s.

```text
ViewNode =
    | Element { kind: ElementKind, children: Vec<ViewNode>,
                bindings: Vec<Binding>, accessibility: AccessibilityMetadata }
    | Text { value: TextExpr, bindings: Vec<Binding> }
    | Conditional { condition: Expr, then: Box<ViewNode>,
                    else_: Option<Box<ViewNode>> }
    | Each { iterable: Expr, item: String, body: Box<ViewNode> }
    | Slot { name: String, fallback: Option<Box<ViewNode>> }

ElementKind = Column | Row | TextField | Button | List | Image | NavLink | ...
```

The view tree is data, not code. The web backend lowers it to
direct DOM construction; the mobile backend lowers it to the
platform's native tree. Phase 1 does not include a virtual DOM;
the IR's tree structure is the equivalent.

### 6.1 `AccessibilityMetadata`

The accessibility block is per-node, not per-component.

```text
AccessibilityMetadata {
    role: Option<Role>,                 // button, textbox, link, ...
    label: Option<TextExpr>,
    focus_order: Option<i32>,           // explicit ordering
    keyboard: KeyboardAction,
    live_region: LiveRegionPolicy,      // off | polite | assertive
}

KeyboardAction =
    | None
    | Enter { on_activate: EventRef }
    | Custom { mapping: BTreeMap<Key, EventRef> }
```

The metadata is required (not optional) in Phase 1 for
interactive primitives (`Button`, `TextField`). A node with
`role: None` on an interactive kind is refused with
`accessibility-role-undefined`.

---

## 7. The type system

The IR types are an extension of Cancho Core's type vocabulary,
with additions for the UI-specific kinds.

```text
Type =
    | Core(Type)                        // inherited from cancho-core
    | Capability(String)                // Network, Storage, ...
    | Request(Box<Type>)                // Request<T>
    | Result(Box<Type>, Box<Type>)      // Ok | Err
    | List(Box<Type>)
    | Option(Box<Type>)
    | Event(String)                     // SearchRequested, Increment, ...
    | View(String)                      // Component name, for slot types
    | StateRef(String, String)          // (component, field) for borrowed refs
```

### 7.1 What is inherited from Cancho Core

- The basic types: `int`, `byte`, `bool`, `float`, `f32`.
- Struct types.
- `Option[T]`, `Result[T, E]`, `List[T]`.
- The mode distinction (`res` vs `val`).

The Cancho Core
[`linearity-and-effects.md`](https://github.com/alpibrusl/cancho/blob/main/docs/linearity-and-effects.md)
rules apply: a `res` value is consumed exactly once on every
path, a `val` value can be copied.

### 7.2 What is net-new

- `Capability(String)` — the IR's representation of a
  declared capability. The string is the capability's name.
- `Request(T)` — the type of an asynchronous effect's token.
  Carries the result type.
- `Event(String)` — an event type, named by its identifier.
- `StateRef(component, field)` — a borrowed reference to a
  state field, used for parent-child state sharing.

Phase 2+ refines each of these with whatever the implementation
finds missing.

---

## 8. The effect row

The IR carries effect rows the same way Cancho Core does — as
a canonically ordered set of labels.

```text
EffectRow {
    labels: BTreeSet<EffectLabel>,
    capabilities: BTreeSet<CapabilityRef>,
}

EffectLabel =
    | Builtin(String)                   // e.g. "net_out"
    | Foreign(String, String)           // (scope, symbol) — e.g. ("browser", "fetch")
    | Capability(String)                // narrow form: capability name
```

The same "exact in both directions" rule applies: a label the
body performs must appear, a label that appears must be
performed.

---

## 9. Source mapping

Every IR node carries a `Span` back to the source file. The
span is what `cancho-ui check` reports when a refusal fires,
and what `cancho-ui diff` shows when a node moves between
revisions.

```text
Span {
    file: String,
    start: Position,                    // line, column
    end: Position,
}
```

Phase 1 uses line/column only; byte offsets are added in Phase
2 if a tool needs them.

---

## 10. The canonical serialization

The IR has **one** canonical serialization. Two IRs with
different byte representations but identical structure must
serialize to the same bytes. The serialization rule:

1. **Stable field order.** Fields appear in declaration order;
   maps are sorted by key.
2. **No insignificant whitespace.** No indentation; separators
   are explicit.
3. **Stable ID encoding.** Stable IDs are encoded as their
   32-byte hash, hex.
4. **Source spans are stripped.** Spans are kept in the
   in-memory IR but not in the canonical text (they are
   kept in a sidecar `source-map` field for tools that need
   them).

The shape is JSON in Phase 1 (so a human can read it); Phase 2
adds a binary form if the size becomes a problem.

### 10.1 Why one serialization

The same reason Cancho Core has one
([`canonical-ast.md`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)):
the per-declaration content hash, the per-IR diff, the agent's
tool input — all of them depend on a canonical form. Two
serializations is two ways to be wrong.

---

## 11. Versioning

The IR schema has a **version number**. The format:

```text
{
  "schema": "cancho-ui-ir",
  "version": "0.1.0",
  ...
}
```

A consumer that does not recognize the version refuses with
`ir-version-unsupported`. A schema change that breaks the
shape is a major-version bump; an additive change is a minor
bump. Phase 1 ships at `0.1.0`.

---

## 12. Conformance fixtures (Phase 2)

The IR conformance suite is a set of fixtures where each
fixture's IR is the canonical expected output. The Phase 2
suite covers:

- A Counter component (the simplest component with state).
- A searchable-list demo component (the proposal §9.3 demo).
- Each refusal rule from [`docs/security-model.md`](../docs/security-model.md)
  §4, with the expected refusal node in the IR.
- A platform adapter for each Phase 1 capability.

The suite is run by `cancho-ui test ir`, which compares the
produced IR against the expected IR, byte-for-byte, for the
supported fixtures.

---

## 13. What Phase 1 must add to this sketch

The Phase 1 delta is small and explicit:

- A serializer producing the canonical form.
- A deserializer that round-trips it.
- The conformance suite's "Counter" and "platform adapter"
  fixtures.
- A `cancho-ui inspect ir` command that prints the IR for a
  given component.

Anything else is Phase 2+. The IR schema is the contract the
backend meets; the schema is not modified by Phase 1 except to
fix what Phase 1 itself finds broken.