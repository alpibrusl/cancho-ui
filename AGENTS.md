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

    on Submit {
        let token = net.search(query)
        match token.await() {
            Ok(results) => { /* ... */ }
            Err(reason) => { /* ... */ }
            Cancelled   => { /* ... */ }
        }
    }
}
```

A component that uses an effect without listing the capability
is refused with `capability-missing`. The same exact-row rule
as Cancho Core's effect rows.

```cancho-ui-refused
//~ RULE capability-missing
component Leaker {
    // No `requires Network`, yet uses net.search:
    on Submit {
        let token = net.search("")
    }
}
```

The reverse — declaring a capability the component does not
use — is refused with `capability-unused`, the same way Cancho
Core refuses a function that over-declares its effect row.

---

## 3. Effects return a result token; results are correlated (placeholder)

An effect returns a `Request[T]`. Awaiting it produces
`Ok(T) | Err(reason) | Cancelled`. The `Cancelled` variant is
delivered when the component has been disposed; the language
drops the result and the `state-after-dispose` rule applies.

```cancho-ui-refused
//~ RULE match-non-exhaustive
component Half {
    fn halved_or_zero(n: Int) -> Int {
        match halve(n) {
            Ok(v) => { return v }
            // Err arm missing
        }
        return 0
    }

    fn halve(n: Int) -> Result[Int, String] {
        return Ok(n / 2)
    }
}
```

A `match` on a `Result` is exhaustive; missing an arm is a
refusal. Same as
[Cancho Core's match rule](https://github.com/alpibrusl/cancho/blob/main/AGENTS.md#71-option-result-loops-and-text-what-a-program-that-returns-an-error-needs).

---

## 4. Foreign functions cross a trust boundary (placeholder)

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

## 5. View primitives carry accessibility metadata (placeholder)

```cancho-ui
component SearchInput {
    view {
        TextField(value: query,
                  role: searchbox,
                  label: "Search query",
                  keyboard: Enter -> Submit,
                  on: Change)
    }
}
```

A primitive with no `role` is refused with
`accessibility-role-undefined`. An interactive primitive with
empty `label` is refused with `accessibility-label-empty`.
A `Button` or `TextField` without a keyboard action for Enter
is refused with `accessibility-keyboard-missing`.

---

## 6. Reserved words are reserved (placeholder)

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

## 7. Where to read more (placeholder)

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