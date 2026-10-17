# AGENTS.md — writing cancho-ui

> **Status: stub.** This document is the contract for agents and
> humans writing cancho-ui source. Phase 0 has no source yet, so
> the agent-facing surface is sketched here as the contract
> Phase 1 lands against.
>
> When Phase 1 ships, every code block marked `cancho-ui` below
> compiles; every block marked `cancho-ui-refused` is refused
> with the rule that names the mistake.

---

## 0. The loop (placeholder, lands with Phase 1)

```sh
cancho-ui check <file.cui>...                  # does it parse and type-check?
cancho-ui check <file.cui>... --output json   # ... and which rule, as data
cancho-ui build --target web <file.cui>        # produce JS + DOM output
cancho-ui test <file.cui>...                   # run every `fn test_*`; exit 4 if one failed
cancho-ui inspect component <id>               # the component, as data
cancho-ui inspect effects <id>                 # reachable effects + capabilities
cancho-ui diff <base> <candidate>              # semantic diff
cancho-ui report                               # the evidence report
```

`check` reports **every** independent refusal, not the first.
On failure, read the `code` field rather than the message — it
is a stable kebab-case tag, and the rule catalog is in
[`spec/diagnostics/README.md`](../spec/diagnostics/README.md).

`cancho-ui skill` and `cancho-ui introspect` are how *this*
page and the full command surface stay the same age: both are
generated from one registration (lands with Phase 1), not
copied by hand. The real exit codes are `0` / `1` / `2` / `3`,
and `4` for `cancho-ui test` only, matching Cancho Core's
[`agent-errors.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md).

**Do not regenerate a body because it was refused.** Every rule
below names what to change.

---

## 1. Components own their state (placeholder)

A component is a unit of UI. State is owned by the component;
an event handler updates state; the view describes what is shown.

```cancho-ui
component Counter {
    state count: Int = 0

    on Increment {
        count = count + 1
    }

    view {
        Column {
            Text { "Count: {count}" }
            Button { label: "Increment" on: Increment }
        }
    }
}
```

For two-way input, `TextField` takes a one-way `value:` binding to a
state field and an `on:` handler that receives the new value:

```cancho-ui
on Change(value) { query = value; }
view {
    TextField { value: query on: Change }
}
```

`value` is a parameter — not a state field — and the JS emitter
passes `e.target.value` to the handler on every input event. A
re-render reflects the new value.

**What the rules guarantee.** `count` is owned by `Counter`.
The compiler refuses a transition that updates `count` from
outside this component (`state-not-owned`). It refuses a
`Counter` that declares `count` but never reads or updates it
(`state-unused`).

The full set of rules is in
[`spec/diagnostics/README.md`](../spec/diagnostics/README.md) §3.

---

## 2. Capabilities are declared, not inferred (placeholder)

```cancho-ui
component SearchForm {
    requires Network

    state query: String = ""
    state r: Result[List[String], String] = Ok([])
    state status: String = "idle"

    on Submit {
        status = "loading"
        r = await(net.search(query))
        status = match r {
            Ok(_) => { "done" }
            Err(_) => { "error" }
            Cancelled => { "cancelled" }
        }
    }
}
```

`net.search(query)` performs the `Network` capability; the
`await(...)` resolves it over the `Ok | Err | Cancelled` arms —
all three are mandatory when the field was assigned from
`await(...)`, which `match-non-exhaustive` checks at compile time.

A component that uses an effect without listing the capability
is refused with `capability-missing`. The same exact-row rule
as Cancho Core's effect rows.

```cancho-ui-refused
//~ RULE capability-missing
component Leaker {
    // No `requires Network`, yet uses net.search:
    on Submit {
        r = await(net.search(""))
    }
}
```

The reverse — declaring a capability the component does not
use — is refused with `capability-unused`, the same way Cancho
Core refuses a function that over-declares its effect row.

---

## 3. Effects return a result token; results are correlated (placeholder)

An effect returns a `Request[T]`. Awaiting it produces
`Ok(T) | Err(reason) | Cancelled` — that is the static type. At
runtime, Tier 10 delivers `Cancelled` as a **drop**: each
`await(...)` is a cancellation-tracked call in the group of its
effect (`net.search` → group `net_search`), starting a call
cancels the group's still-pending request, and a superseded
request resolving later throws a sentinel that the handler's
try/catch swallows — the rest of that handler never runs, so a
slow stale request cannot clobber a newer one's state
(state-after-dispose semantics, one granularity finer: per
supersede rather than per dispose). Because the drop happens at
the await, the `Cancelled` match arm is not dispatched in Tier
10 — but it remains **required**: the match must be exhaustive
by construction, so later tiers that deliver `Cancelled` to the
arm (component disposal, deadlines) find a program that already
handles it.

`Result[T, E]` is two constructors: `Ok(value)` and `Err(reason)`.
A `Request[T]` is awaited with `await(r)` and resolves to
`Ok | Err | Cancelled` — three constructors. `match` on any of
these must cover every variant. Missing an arm is a refusal with
`match-non-exhaustive` at compile time; without
compile-time coverage (a non-state scrutinee) the runtime throws
`no match arm matched`.

```cancho-ui
component Half {
    state result: Result[Int, String] = Ok(0);

    on Tick { result = Ok(42); }

    view {
        Text {
            match result {
                Ok(v) => { "value: {v}" }
                Err(e) => { "error: {e}" }
            }
        }
    }
}
```

A `match` on a `Result` is exhaustive; missing an arm is a
refusal. Same as
[Cancho Core's match rule](https://github.com/alpibrusl/cancho/blob/main/AGENTS.md#71-option-result-loops-and-text-what-a-program-that-returns-an-error-needs).

A `Request[T]` is awaited with `await(r)`. The handler becomes an
`async function`; a state change made before the `await` (a
loading status) is painted synchronously, and the resolved
value dispatches through `match`:

```cancho-ui
component Ticker {
    state status: String = "Idle";
    state r: Result[Int, String] = Ok(0);

    on Tick {
        status = "Waiting";
        r = await(sleep(50));
        status = match r {
            Ok(v) => { "Ok" }
            Err(e) => { "Err" }
            Cancelled => { "Cancelled" }
        };
    }

    view {
        Text { role: status "Status: {status}" }
    }
}
```

The compile-time check is scrutinee-aware: a field assigned from
`await(...)` holds all three variants at runtime even though its
declared type says `Result[T, E]`, so its `match` needs all three
arms — a missing `Cancelled` is refused the same way. A `_`
catch-all arm satisfies exhaustiveness.

---

## 4. `Option`, `List`, and `For` — what a collection needs (Tier 8)

`Option[T]` is two constructors: `Some(value)` and `None`. The
`match-non-exhaustive` rule applies the same way as `Result`:
both arms or a `_` catch-all, and a missing arm is a refusal.

```cancho-ui
component Roster {
    state picked: Option[String] = None;

    on Pick { picked = Some("Ada"); }

    view {
        Text {
            role: status
            match picked {
                Some(who) => { "picked: " + who }
                None => { "picked: none" }
            }
        }
    }
}
```

`List[T]` has a literal (`[a, b, c]`) and is rendered with
`For { over: <state list> as: <item> ... }`. Inside the `For`
body the `as:` binding is the current element:

```cancho-ui
state names: List[String] = ["Ada", "Grace"];

view {
    Column {
        For {
            over: names
            as: n
            Text { "row: {n}" }
        }
    }
}
```

Two know-nots (one healed by Tier 11, one still true):

- Interpolation applies inside `For` bodies (any text body) but
  **not** inside `match` arm bodies — an arm body is a plain
  expression, so strings are joined with `+` there.
- ~~`filter`/`map` do not exist yet~~ **Tier 11: they do.**

Since Tier 11 a list view can be *derived*, not just stored. Three
pure call forms — no capability required, they run synchronously
inside the render pass:

- `len(value)` — element count (or character count of a string).
- `filter(list, x => pred)` — keep the elements where `pred` holds.
- `map(list, x => body)` — transform each element.

```cancho-ui
state results: List[String] = [];
state min: Int = 0;

view {
    Column {
        Text { role: status "{len(results)} results matched" }
        For {
            over: filter(results, n => len(n) > min)
            as: row
            Text { "row: {row}" }
        }
    }
}
```

The lambda (`x => expr`) is the only function value the language
has, and it **never escapes**: it is accepted syntactically in any
argument position, but lowering refuses it anywhere except as the
second argument of `filter`/`map` (`lambda-unsupported`). A lambda
body must be pure — no `await(...)`, no effect calls
(`pure-lambda-required`); list operations run synchronously
inside the render pass. Since Tier 11 `For`'s `over:` is a general
pure expression, not just a state-field name.

---

## 5. Components compose (Tier 12)

A component's view can instantiate **other components of the same
file**. Each instance is its own unit — separate state, separate
handlers — mounted into its own wrapper element:

```cancho-ui
component HelloCard {
    state count: Int = 0;

    on Inc { count = count + 1; }

    view {
        Column {
            Text { "count: {count}" }
            Button { label: "card +" on: Inc }
        }
    }
}

component Page {
    view {
        Column {
            Text { "page" }
            HelloCard {}
            HelloCard {}
        }
    }
}
```

Two instances are two pieces of state: clicking one card's button
advances only that card.

An instance is **opaque** — the parent configures nothing yet
(properties arrive in a later tier):

- bindings on an instance are refused (`instance-opaque` — the
  same code for `role:`/`label:` and children),
- an unknown name is refused (`component-kind-unknown`),
- a cycle — `A` instantiating `B`, `B` instantiating `A` — is
  refused at compile time (`component-cycle`) because the emitted
  mount would recurse forever.

Same-file only for now: cross-file `import` and instance
*properties* are the next composition tiers.

---

## 6. Foreign functions cross a trust boundary (placeholder)

Every call to JavaScript, the DOM, or a native module is a
declared foreign function. The compiler refuses undeclared
calls with `foreign-interface-missing`.

```cancho-ui
foreign {
    fn fetch(url: String) -> Request[String] from "browser"
}

component SearchForm {
    requires Network

    fn do_search(url: String) -> Request[String] {
        return fetch(url)
    }
}
```

A typed wrapper does not prove the implementation safe. The
evidence report names every `from "scope"` as a trust
boundary. Same shape as Cancho Core's
[`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md).

---

## 7. View primitives carry accessibility metadata (placeholder)

A `TextField` takes `label:`, `value:` and `on:`. Order matters
differently per key today: `label:` must come **before** `value:`
(the parser treats everything after the first `value:`/`on:` pair
as bindings). Phase 2 will lift the ordering rule.

```cancho-ui
component SearchInput {
    view {
        TextField {
            label: "Search query"
            value: query
            on: Change
        }
    }
}
```

A primitive with no `role` is refused with
`accessibility-role-undefined`. An interactive primitive with
empty `label` is refused with `accessibility-label-empty`.
A `Button` or `TextField` without a keyboard action for Enter
is refused with `accessibility-keyboard-missing`.

Those three a11y refusals are **contract, not implementation**:
Tier 9 emit-time sets `role` / `aria-label` from the bindings but
does yet not refuse missing ones — `keyboard:` bindings do not
exist at all.

---

## 8. Reserved words are reserved (placeholder)

`component`, `state`, `view`, `on`, `requires`, `effect`,
`capability`, `foreign`, `module`, `import`, `pub`, `target`
are reserved everywhere. Using any as a binding name is refused
with `name-reserved`.

```cancho-ui-refused
//~ RULE name-reserved
component Hover {
    state view: Bool = false   // `view` is reserved
    on Toggle { view = !view }
}
```

---

## 9. Where to read more (placeholder)

The full design is under [`docs/`](../docs/). The Phase 0
deliverables — semantics, security model, IR schema, diagnostic
format — are the things Phase 1 enforces; this page is the
narrative entry point for the language itself.

---

## What Phase 1 must add to this stub

- The full loop in §0, with concrete command outputs.
- The rule catalog cross-referenced with code examples for
  every rule in §3 of
  [`spec/diagnostics/README.md`](../spec/diagnostics/README.md).
- A worked searchable-list demo with annotations.
- The cross-references to
  [`docs/semantics.md`](../docs/semantics.md) for each rule.
- The checked code blocks (every block above marked
  `cancho-ui` must compile; every `cancho-ui-refused` must
  refuse with the named rule) — Phase 1 lands the test
  harness that runs them.

Until Phase 1, this document is the **contract**, not the
working surface.