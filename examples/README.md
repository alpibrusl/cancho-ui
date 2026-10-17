# Examples

> **Status: Phase 0.** Every fixture here is a **contract the
> Phase 1 compiler must meet**, written down before the compiler
> exists. The valid fixtures are programs that must compile and
> run; the invalid fixtures are programs that must be refused
> with the named rule.
>
> The fixtures do not exist as `.cui` source yet — Phase 1 lands
> them, with the parser, in the same commit.

---

## 1. Valid fixtures (must compile and run)

Ten programs that exercise the Phase 1 supported subset.

| # | File | What it exercises | Tier |
|---|---|---|---|
| 1 | `valid/01_counter.cui` | A minimal component with state and an event handler | Static + Runtime |
| 2 | `valid/02_two_components.cui` | Component composition and property passing | Static |
| 3 | `valid/03_text_with_binding.cui` | A `Text` primitive with an interpolated state binding | Static |
| 4 | `valid/04_textfield.cui` | A `TextField` bound to a state field, with accessibility | Static |
| 5 | `valid/05_button_with_event.cui` | A `Button` that fires an event, accessibility metadata | Static |
| 6 | `valid/06_each_loop.cui` | An `each` block over a state list | Static + Runtime |
| 7 | `valid/07_if_else.cui` | A `view` with a conditional branch | Static |
| 8 | `valid/08_slot.cui` | A component with a named slot for child content | Static |
| 9 | `valid/09_match_on_result.cui` | A `match` on a `Result[T, E]` with both arms | Static |
| 10 | `valid/10_async_with_cancellation.cui` | An async effect with `await` and lifecycle cancellation | Static + Runtime |

### 1.1 `valid/01_counter.cui`

The simplest component the language admits.

```text
component Counter {
    state count: Int = 0

    on Increment {
        count = count + 1
    }

    view {
        Column {
            Text("Count: {count}")
            Button(label: "Increment", on: Increment)
        }
    }
}
```

**What it exercises.** A `state` declaration, an `on` clause,
state mutation in a transition, a view tree with `Column`,
`Text` with interpolation, `Button` with `label` and an event
reference.

**Source mappings the Phase 1 IR must carry.** The `count`
state field's span; the `Increment` event's span; the `view`
block's span.

### 1.2 `valid/02_two_components.cui`

Component composition.

```text
component Counter {
    prop label: String = "Counter"
    state count: Int = 0

    on Increment {
        count = count + 1
    }

    view {
        Column {
            Text("{label}: {count}")
            Button(label: "+1", on: Increment)
        }
    }
}

component App {
    view {
        Column {
            Counter(label: "First")
            Counter(label: "Second")
        }
    }
}
```

**What it exercises.** Two components in one file; a parent
constructing two children; the `prop` declaration with a
default; passing a prop value at the call site.

**Refusal this must NOT trigger.** `name-reserved` (the word
`label` is a property name, not a reserved word).

### 1.3 `valid/03_text_with_binding.cui`

A `Text` primitive with an interpolated binding.

```text
component Greeting {
    state name: String = "world"

    view {
        Text("Hello, {name}!")
    }
}
```

**What it exercises.** String interpolation, the `{expr}`
syntax inside a `string_literal`, the binding to a state field.

### 1.4 `valid/04_textfield.cui`

A `TextField` with accessibility.

```text
component SearchInput {
    state query: String = ""

    on Change(value) {
        query = value
    }

    view {
        TextField(value: query,
                  role: searchbox,
                  label: "Search query",
                  keyboard: Enter -> Submit,
                  on: Change)
    }
}
```

**What it exercises.** `TextField` primitive, accessibility
block (`role`, `label`, `keyboard`), one-way binding
(`value: query`), event handler with a parameter
(`on Change(value)`).

**Refusals this must NOT trigger.** `accessibility-role-undefined`,
`accessibility-label-empty`, `accessibility-keyboard-missing`.

### 1.5 `valid/05_button_with_event.cui`

A `Button` that fires an event.

```text
component Submit {
    on Press {
        // no-op in this fixture; the search flow uses it
    }

    view {
        Button(label: "Submit",
               role: button,
               keyboard: Enter -> Press,
               on: Press)
    }
}
```

**What it exercises.** `Button` primitive, accessibility,
event declaration with no parameters.

### 1.6 `valid/06_each_loop.cui`

An `each` block.

```text
component ItemList {
    state items: List[String] = ["a", "b", "c"]

    view {
        Column {
            each item in items {
                Text("{item}")
            }
        }
    }
}
```

**What it exercises.** `List[String]` state, the `each` view
node, body with one binding.

### 1.7 `valid/07_if_else.cui`

A conditional view.

```text
component Toggle {
    state shown: Bool = true

    on Flip {
        shown = !shown
    }

    view {
        Column {
            if shown {
                Text("Visible")
            } else {
                Text("Hidden")
            }
            Button(label: "Flip", on: Flip)
        }
    }
}
```

**What it exercises.** `Bool` state, negation in a transition,
`if`/`else` view node.

### 1.8 `valid/08_slot.cui`

A component with a named slot.

```text
component Card {
    prop title: String

    view {
        Column {
            Text("{title}")
            slot body
        }
    }
}

component App {
    view {
        Card(title: "Hello") {
            Text("This is the body.")
        }
    }
}
```

**What it exercises.** Slot declaration, slot filling at the
call site.

### 1.9 `valid/09_match_on_result.cui`

A `match` on `Result[T, E]`.

```text
component Half {
    fn halve(n: Int) -> Result[Int, String] {
        if n < 0 {
            return Err("negative")
        }
        return Ok(n / 2)
    }

    fn halved_or_zero(n: Int) -> Int {
        match halve(n) {
            Ok(v) => { return v }
            Err(_) => { return 0 }
        }
    }
}
```

**What it exercises.** `Result[Int, String]` type, `Ok`/`Err`
constructors, exhaustive `match`.

**Refusals this must NOT trigger.** `match-non-exhaustive`.

### 1.10 `valid/10_async_with_cancellation.cui`

An async effect with cancellation.

```text
component SearchForm {
    requires Network

    state query: String = ""
    state status: SearchStatus = Idle

    on Change(value) {
        query = value
    }

    on Submit {
        status = Searching
        let token = net.search(query)
        match token.await() {
            Ok(results) => { status = Done(results) }
            Err(reason) => { status = Failed(reason) }
            Cancelled => { /* no state update; component may be disposed */ }
        }
    }

    view { /* ... */ }
}
```

**What it exercises.** `requires Network`, an async effect
(`net.search`), the `await` syntax, `match` on the three
result variants (`Ok` / `Err` / `Cancelled`).

**Refusals this must NOT trigger.** `capability-missing`,
`match-non-exhaustive`.

---

## 2. Invalid fixtures (must be refused)

Ten programs that exercise every Phase 1 refusal code. Each
fixture's header declares the expected rule.

### 2.1 `refusal/state_unused.cui`

```text
//~ RULE state-unused
component Quiet {
    state silent: Int = 0

    view {
        Text("never read")
    }
}
```

The `silent` state is declared but never read or updated.

### 2.2 `refusal/capability_missing.cui`

```text
//~ RULE capability-missing
component SearchForm {
    state query: String = ""

    on Submit {
        // No `requires Network`, yet uses net.search:
        let token = net.search(query)
    }
}
```

`net.search` is reached without `requires Network`.

### 2.3 `refusal/capability_unused.cui`

```text
//~ RULE capability-unused
component Overreacher {
    requires Network
    requires Storage

    view {
        Text("nothing actually uses either capability")
    }
}
```

Both `requires` declarations have no reachable use.

### 2.4 `refusal/effect_not_declared.cui`

```text
//~ RULE effect-not-declared
component Leaker {
    // Foreign function declared in scope, but no `requires`:
    on Click {
        let r = foreign.fetch("https://example.com")
    }
}
```

The foreign function is reached without a `requires` declaration
on the component.

### 2.5 `refusal/state_not_owned.cui`

```text
//~ RULE state-not-owned
component Other {
    on Tick {
        // `count` is not declared on this component:
        count = count + 1
    }
}
```

`count` is referenced as a state field but not declared on the
enclosing component.

### 2.6 `refusal/match_non_exhaustive.cui`

```text
//~ RULE match-non-exhaustive
component Half {
    fn halved_or_zero(n: Int) -> Int {
        match halve(n) {
            Ok(v) => { return v }
            // `Err` arm missing
        }
        return 0
    }

    fn halve(n: Int) -> Result[Int, String] {
        return Ok(n / 2)
    }
}
```

A `match` on a `Result` missing an `Err` arm.

### 2.7 `refusal/accessibility_label_empty.cui`

```text
//~ RULE accessibility-label-empty
component Mystery {
    view {
        Button(label: "", keyboard: Enter -> Submit, on: Submit)
    }
}
```

An interactive primitive (`Button`) with an empty `label`.

### 2.8 `refusal/accessibility_keyboard_missing.cui`

```text
//~ RULE accessibility-keyboard-missing
component MouseOnly {
    view {
        Button(label: "Submit", on: Submit)
    }
}
```

An interactive primitive (`Button`) without a `keyboard` action
for Enter.

### 2.9 `refusal/foreign_interface_missing.cui`

```text
//~ RULE foreign-interface-missing
component Raw {
    on Load {
        // Calling `window.fetch` directly without an adapter:
        let r = window.fetch("/api/items")
    }
}
```

A foreign call without a `foreign` declaration and without a
platform adapter. The implementation must reject this even if a
`foreign` block later in the file declares `fetch` — the rule
is about the call site, not the existence of any declaration.

### 2.10 `refusal/name_reserved.cui`

```text
//~ RULE name-reserved
component Hover {
    state view: Bool = false   // `view` is a reserved word

    on Toggle {
        view = !view
    }
}
```

A binding named `view`, which is reserved at the language level.

---

## 3. The fixture catalog at a glance

### 3.1 Valid

| File | Tier | Notes |
|---|---|---|
| `01_counter.cui` | Static + Runtime | Minimal viable component |
| `02_two_components.cui` | Static | Composition + props |
| `03_text_with_binding.cui` | Static | String interpolation |
| `04_textfield.cui` | Static | `TextField` + accessibility |
| `05_button_with_event.cui` | Static | `Button` + accessibility |
| `06_each_loop.cui` | Static + Runtime | `each` view node |
| `07_if_else.cui` | Static | Conditional view |
| `08_slot.cui` | Static | Slot composition |
| `09_match_on_result.cui` | Static | `match` on `Result` |
| `10_async_with_cancellation.cui` | Static + Runtime | Async + cancellation |

### 3.2 Refusal

| File | Rule | Source |
|---|---|---|
| `state_unused.cui` | `state-unused` | [`docs/security-model.md`](../docs/security-model.md) §4 |
| `capability_missing.cui` | `capability-missing` | Q7 |
| `capability_unused.cui` | `capability-unused` | Q7 |
| `effect_not_declared.cui` | `effect-not-declared` | Q2 |
| `state_not_owned.cui` | `state-not-owned` | Q2 |
| `match_non_exhaustive.cui` | `match-non-exhaustive` | Q6 |
| `accessibility_label_empty.cui` | `accessibility-label-empty` | Q9 |
| `accessibility_keyboard_missing.cui` | `accessibility-keyboard-missing` | Q9 |
| `foreign_interface_missing.cui` | `foreign-interface-missing` | Q11 |
| `name_reserved.cui` | `name-reserved` | [`docs/language-design.md`](../docs/language-design.md) §4 |

Each fixture's `RULE` header is parsed by the test runner the
same way Cancho Core's reject fixtures are parsed
([`tests/reject`](https://github.com/alpibrusl/cancho/tree/main/tests/reject)):
the runner executes the compiler, asserts the named rule was
the rule that fired, and asserts the diagnostic's span points
to the offending construct.

---

## 4. What Phase 1 must add to this sketch

- A `tests/valid.rs` test that runs each valid fixture through
  `cancho-ui check` and asserts exit code 0.
- A `tests/refusal.rs` test that runs each refusal fixture
  through `cancho-ui check` and asserts exit code 1 with the
  named rule.
- An IR conformance test for the four net-new IR shapes
  (Capability, Request, Event, StateRef), pinned against the
  fixtures' output.

Phase 1 expands the suite as new rules or new syntax land; the
catalog above is the **minimum** the Phase 1 exit gate checks.