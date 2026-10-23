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
cancho-ui build <file.cui> --pwa -o <dir>      # an installable web app: page, manifest, offline worker, icons (docs/mobile-backend.md)
cancho-ui build <file.cui> --page -o <dir>     # a plain page, not an app: app.js + index.html, no manifest or worker (gallery demos)
cancho-ui test <file.cui> <trace.json>         # replay an event trace; exit 4 if a test failed (docs/replay.md)
cancho-ui inspect component <id>               # the component, as data
cancho-ui inspect effects <id>                 # reachable effects + capabilities
cancho-ui diff <base> <candidate>              # semantic diff; exit 5 = differs, 0 = no change
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

`storage.get(key, default)` and `storage.set(key, value)` perform the `Storage` capability (strings
only; `set` answers whether it persisted; see `docs/adapters.md`, and note that a string interpolation
cannot contain a string literal, so put the call in `Text { storage.get("k", "d") }`).

`net.get(url)` also performs `Network`: a GET of this site, or of a host named in `requires Network("host")` (a
literal URL on another host is `capability-bound-violated`), awaited like `net.search`, answering
`Ok(text) | Err(reason) | Cancelled` (see `docs/adapters.md` section 9). `"error: {why}"` interpolates in a
`match` arm as in a `Text`; a `{` in a label or other static text is refused.

`net.post(url, body)` performs `NetworkWrite`, a capability of its own (`Network` is read-only): a POST of a JSON text to this
site only (no host bound; a literal URL naming a host is `capability-bound-violated`), answered like `net.fetch` with the
envelope `{"status":N,"body":"..."}` for any HTTP status, never retried and not aborted when superseded (`docs/adapters.md`
section 17).

`net.put(url, body)` and `net.delete(url)` perform the same `NetworkWrite` (no new capability, no host bound, same origin only): a PUT
of a JSON text, and a DELETE with no body, answered like `net.post` with the envelope `{"status":N,"body":"..."}` (a `204` is
`Ok` with status 204), never retried, not aborted when superseded, each in its own cancellation group. `net.put` takes exactly
two arguments and `net.delete` one (`builtin-shape-mismatch`); a declared `Credential` is sent by both (`docs/adapters.md`
section 21). There is no `PATCH`.

A component that sends a secret declares `requires Credential("api", "host"..)` (with `Network` for any host it names): `net.get`,
`net.fetch`, `net.post`, `net.put` and `net.delete` then add `Authorization: Bearer <token>`, to this site and the named hosts only. Build with
`--no-auto-mount` and hand the token to `mount_X(root, { credentials: { "api": token } })`; auto-mount is refused
(`credential-needs-mount-options`). A trace lists `"credentials": ["api"]`; a missing one answers `Err("blocked: no credential api")`
(`docs/adapters.md` section 18).

`Button { label: "Select " + n on: Pick(n, 1) }`: `label:` is a string literal or an expression, and `on: Pick(a, b)` calls the handler `on Pick(x, y)` with the arguments, evaluated when the view is drawn, so a button in a `For` names its row (`docs/row-actions.md`). A click has no arguments of its own: a bare `on: Pick` for a handler with parameters is `event-arity-mismatch`, an event the component does not declare is `event-unknown`, and arguments on anything but a `Button` are `event-args-unsupported`.

A `TextField` can be a password, email, search, telephone or url field: `TextField { label: "Password" value: pass on: Pass type: "password" }` (`docs/controls.md` section 5; `type:` is one of `text` (the default), `password`, `email`, `search`, `tel`, `url`, as a string literal, or `input-type-invalid`). The browser changes what some types report: an `email` or `url` field drops the whitespace around what is typed, so a trace gives the value the browser would. `number` is not offered (typing `1e5` key by key ends as `5`).

A `TextField` also takes `autocomplete: "username"` (`on`, `off` or one field name of the HTML autofill list), `inputmode: "numeric"` (none, text, decimal, numeric, tel, search, email, url), `maxlength: 200` (an integer literal, 1 to 100000: the browser stops typing and pasting there, UTF-16 code units; the program can still set a longer value) and `name: "username"`; each is `input-attribute-invalid` when wrong (`docs/controls.md` section 6). `cancho-ui test` stops with `trace-value-too-long` on a step that gives a field's handler a string longer than its `maxlength`.

`==` and `!=` compare two numbers, two strings or two bools; `<`, `<=`, `>`, `>=` order two numbers or two strings; anything else whose types the compiler knows is `comparison-type-mismatch` (`docs/comparisons.md`). `json.get` answers a string, so a status is `json.get(reply, "status", "") == "200"` or `num(json.get(reply, "status", ""), 0) == 200`, never `== 200`. Lists and `Result`s are not compared with `==`: use `len(a) == len(b)`, `join`, or `match`.

A `TextField` shows its validation message with `error: msg` (the name of a `String` state field; `error-binding-invalid` otherwise): the field draws the message next to itself, and while `msg` has text the field is `aria-invalid` and `aria-describedby` points at the message, while it is empty the field is valid and the message draws nothing (`docs/controls.md` section 7). Prefer it to a separate `Text { role: status .. }` after the field, which a screen reader does not tie to the field.

`TextArea { label: "Note" value: note on: Note rows: 4 maxlength: 200 error: msg }` is a `TextField` for several lines (`docs/controls.md` section 8): the handler gets the text with LF line breaks, Enter inserts a line break (so no `submit:`, and no `type:`), `rows:` is 1 to 30, and `maxlength` counts a line break as one character.

`Checkbox { label: "I agree" checked: agreed on: Agree }` is a box bound to a `Bool` state field (`checked-binding-invalid`; both `checked:` and `on:` are required, `checkbox-incomplete`): the handler `on Agree(v) { agreed = v; }` gets the new state as a `Bool`, or takes no parameter and flips the field. It takes only `label`, `role`, `on`, `checked`, `disabled`, `error`, `name` (`docs/controls.md` section 9); the label is the touch target.

`Select { label: "Status" options: ["open", "doing", "done"] value: status on: Pick }` is a choice of one string: `options:` a list of string literals or a `List` state field, `value:` a `String` state field, `on:` the event (the handler gets the chosen string); all three are required (`select-incomplete`, `select-binding-invalid`). A value that is not in the list shows a blank select, and `cancho-ui test` stops with `trace-value-not-an-option` on a step that gives the handler a string that is none of the options (`docs/controls.md` section 10).

A program can take a bearer token from a login response (`docs/bearer-token.md`, #83; prefer an HttpOnly cookie the service sets when you can, `examples/tickets`): declare `requires Credential("api");` (with the usual `Network`), then `signed = credential.set("api", json.get(json.get(reply, "body", ""), "token", ""));` inside the `match` of the login's result, and overwrite the field that received the response (`login = Ok("");`) in the same handler so the token is in no state. `credential.set` answers `false`, and keeps nothing, for a value that is not a plain token (1 to 4096 visible ASCII characters); `credential.clear("api")` signs out and answers `true`. The name is a string literal that is the declared credential (`credential-name-invalid`). The token is sent as `Authorization: Bearer` to this origin and the hosts the declaration names, is held in memory only (a reload forgets it), and until the component holds one its requests go out without a header. Replay's step report lists the names held in `credentials` and `expect` can assert it; a value is never in a report.

A page can have several screens at their own URLs (`docs/routing.md`, #75; `build --page` and `--pwa` both emit it, and a deep link and a reload work on a static host and offline). The route is the fragment of the URL (`#/items/valve`). `requires Router;` and `route Navigated;` run `on Navigated(path)` with the path when the page loads and whenever the fragment changes (a link, Back, Forward, a typed URL, `router.go`); one `route` per component (`route-duplicate`), naming an event with exactly one parameter (`route-event-unknown`, `route-event-params`). `NavLink { label: "Item" to: "/items/valve" }` is a link to a route of this page and needs no capability; `to:` is a `String` expression, a literal one must be a path: it starts with `/`, holds no space, `#`, `"`, `<`, `>`, backtick or non-ASCII character, at most 2,000 characters (`navlink-incomplete`, `navlink-target-invalid`; `to:` anywhere else and `on:`/`disabled:` on a `NavLink` are `binding-unknown`). `router.go(path)` pushes a history entry and `router.replace(path)` replaces the current one; both answer a `Bool` (`false` for a path the fragment cannot carry, `true` and no event for the current path) and need `requires Router;` (`capability-missing`). Keep the path in a state field and draw the screen with `If { when: .. }`; parse the path with `split`, `skip`, `take`, `join`. A trace may start at `"url": "/items/valve"` and use the steps `{"navigate": "/items/gauge"}`, `{"back": true}` and `{"forward": true}`; `expect` may assert `routes` (what the route event heard in the step), `url` and `history`. `examples/routes` is a list and a detail.

When a screen changes the page moves focus for you (`docs/screen-focus.md`, #92): if the control that had focus is replaced with its screen, or a `route` event runs because the fragment changed, the first `role: heading` of the screen that appeared (an `If` that became true) takes focus, and a component with `route` keeps `document.title` as `<heading> – <the title it loaded with>`. So give every `If` screen its own `Text { role: heading ".." }`, and keep the application's title out of `role: heading` or above the screens (it is skipped); a screen with no heading gets no move. Replay cannot assert focus or the title.

A view chooses what to draw with `If { when: cond  <view nodes> }` (`docs/view-conditional.md`): the children are in the page only while the pure `Bool` expression `cond` is true, and have no node at all (not a hidden one) while it is false. Use it for a screen or a panel instead of a list that holds zero or one element. `cond` must be a `Bool` (`condition-not-bool`) of names in scope (`name-unknown`) with no effect. A control inside an `If` that is not drawn cannot be used: `cancho-ui test` answers `outcome: "hidden"` for an event whose every control is in such an `If`. There is no `else`; write a second `If { when: !cond .. }`.

A handler branches with the statement `if cond { stmts } else { stmts }` (`else if` and a missing `else` are fine) at the start of a statement, and its branches may hold effects: `if signed { r = await(net.get("orders")); }` asks for the orders only when `signed` is true (`docs/handler-sequences.md`). Its condition is pure and a `Bool` (`if-effect-in-expression`, `condition-not-bool`); `x = if c { 1 } else { 2 };` is still the expression. Statements that several handlers share are written once as `block Name { stmts }` and run with `run Name;` in a handler, a block or a branch: `run` is replaced by the block's statements before anything else sees the program, a block has no parameters (a handler's parameters are not in scope in it, `name-unknown`), and a `run` of nothing, two blocks of one name, a block named like a state field or event, and a block that runs itself are `block-unknown`, `block-duplicate`, `block-name-conflict` and `block-cycle`. `block` and `run` are words only in those two places; `if` statements nest at most 32 deep (`parse-nesting-too-deep`). The page paints before every `await`, not only the first.

Presentation (`docs/presentation.md`): a `TextField`'s `label:` is shown above the field; `Table { label: "Machines" Row { Text { role: columnheader "id" } } For { over: l as: m Row { Text { m } Button { .. } } } }` is a real table (a `Row` is a row, a `Text` a cell, a button is wrapped in one; anything else in a `Table` is `table-child-invalid`); `Text { role: status tone: "error" ".." }` or `tone: some_string_field` is `"ok"`, `"warn"` or `"error"` (a prefix and a colour; `tone-invalid` otherwise; a toned text with no visible characters has no tone and no mark, so a constant `tone: "error"` on a message that is empty while the field is valid is right); `Row { gap: 0..4 }`, `Column { max_width: 10..200 }` (`layout-value-invalid` otherwise).

`Button` and `TextField` take `disabled: true | false | <a Bool state field>` (the native attribute, so a double click on a
`disabled: sending` button that sets `sending = true` before its `await` sends one write); any other key that is not `label`,
`role`, `on`, `value`, `disabled`, `submit` or `placeholder` is `binding-unknown` (`docs/controls.md`). `TextField { ... submit: Apply }`
fires the parameterless event `Apply` on Enter (not while composing, not on key repeat, and not while a button `on: Apply` is
disabled), so `Enter` submits a form once (`submit-event-invalid` for an unknown event or one with parameters).

A string writes a literal brace as `\{` and `\}` (`"\{\"a\":1\}"` is `{"a":1}`); a `{` starts an interpolation, and anything left in
it after one expression is `parse-unexpected-token`. `json.quote(text)` is the JSON string literal of a text, quotes included, so
`"\{\"id\":" + json.quote(id) + "\}"` is a body that the text of `id` cannot change (`docs/language-design.md` decision 15).

`await(location.current())` performs `Location` and resolves to `Ok("lat,lon")` (2 decimals; `requires
Location("fine");` for 5), `Err(reason)` or `Cancelled`; the browser asks the user
(`docs/adapters.md` section 11).

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
runtime, cancellation reaches the handler in two ways:

- **Supersede is a drop** (Tier 10, unchanged): each `await(...)`
  is a cancellation-tracked call in the group of its effect
  (`net.search` → group `net_search`), starting a call cancels the
  group's still-pending request, and a superseded request
  resolving later throws a sentinel that the handler's try/catch
  swallows — the rest of that handler never runs, so a slow stale
  request cannot clobber a newer one's state. The `Cancelled` arm
  is *not* dispatched on supersede — it stays required.
- **Disposal dispatches the arm** (Tier 17): when a component
  instance is disposed — its slot pruned from a shrinking `For`, or
  its parent's disposal cascading to it — its pending await
  *resolves* `{__cui_tag: "Cancelled"}` (when the underlying request
  completes, not at the instant of disposal; `docs/replay.md` §6), so the mandatory
  `Cancelled` arm runs: the program's own cancellation design
  executes, then the rest of the handler continues on the detached
  subtree (rendering into detached DOM, harmless). The arm is
  compile-time mandatory for an awaited field, so a disposed
  component always has that design.

Cancellation groups are **per component instance** (Tier 17): the
inflight table lives in the mount closure, so two components
awaiting the same effect never cancel each other's requests, and a
dispose marks exactly its own table. Superseded-then-disposed
tokens drop (the sentinel takes precedence); only the *current*
pending request per group resolves `Cancelled` on disposal.

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

Four more pure builtins (`docs/lists.md`): `sort(list, x => key)` (stable, ascending; numbers before strings, strings by
UTF-16 code units), `reverse(list)`, `contains(text, needle)` (case-sensitive) and `lower(text)` (ASCII only), so
`sort(filter(rows, r => contains(lower(r), lower(q))), r => lower(r))` is a case-insensitive search that stays ordered.
Descending is `reverse(sort(..))`. `take(list, n)` and `skip(list, n)` page a list (`n` cut toward zero; negative or NaN is 0; more than the list is all of it), so `take(skip(rows, page * size), size)` is a page (`docs/lists.md` section 6). Text to list and back (`docs/lists.md` section 7): `lines(text)` (splits on newline, tolerates CRLF, no final empty line, at most
10,000), `split(text, sep)` (empty separator does not split), `join(list, sep)`, `trim(text)` and `upper(text)` (ASCII only),
`starts_with(text, prefix)`, `ends_with(text, suffix)`: so `map(lines(answer), l => trim(l))` turns a one-record-per-line
endpoint into rows for `For`. `num(text, default)` reads a JSON number from a text (or answers `default`), so `sort(rows, r => num(json.get(r, "n", ""), 0))` sorts by a number. Six numeric builtins (`docs/lists.md` section 8): `min(a, b)`, `max(a, b)`, `abs(n)`, `floor(n)`, `ceil(n)`, `round(n)`, exactly JavaScript's `Math` (`round` takes halves toward +Infinity, so `round(-2.5)` is `-2`), so a page count is `max(ceil(len(rows) / size), 1)`; a non-number argument is `replay-type-error` in replay. There are no literal patterns: a `match` arm `true` or `false` is refused with `pattern-literal-unsupported` (a bare name in a pattern binds and matches everything). Branch on a Bool with `if cond { a } else { b }` (an expression, with `else if`; both sides required, `if-else-required`; pure, so no effect or `await` in it, `if-effect-in-expression`; the condition a `Bool`, `condition-not-bool`; the branches one type, `if-branch-type-mismatch`) and combine Bools with `&&`, `||`, `!`, which evaluate the right side only when they must: `b != 0 && a / b > 1` never divides by zero (`docs/conditionals.md`). `+ - * /` never answer `Infinity` or `NaN`: a division by zero or an overflow is `replay-non-finite` in `cancho-ui test` and a thrown error in the page. A name that is not a state field or a binding in scope is `name-unknown`.

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
    state title: String = "card";
    state count: Int = 0;

    on Inc { count = count + 1; }

    view {
        Column {
            Text { role: heading "{title}" }
            Text { "count: {count}" }
            Button { label: "card +" on: Inc }
        }
    }
}

component Page {
    view {
        Column {
            Text { "page" }
            HelloCard { title: "first card" }
            HelloCard { title: "second card" }
        }
    }
}
```

Two instances are two pieces of state: clicking one card's button
advances only that card. Tier 13 adds **properties**: a binding
whose key names a state field of the child and whose value is a
constant (`"Adobe"`, `3`, `true`) is the child's *initial value*
for that field. Each field not passed falls back to the child's
declared default. Tier 14 adds **dynamic properties**: the value
may be read from the parent's state — or an enclosing `For`
binding — and is evaluated on every parent render:

```cancho-ui
state names: List[String] = ["Ada", "Grace"];

view {
    Column {
        For {
            over: names
            as: t
            HelloCard { title: t }
        }
    }
}
```

Property refusals:

| rule | misuse |
|---|---|
| `property-unknown-key` | key is not a state field of the child |
| `property-source-unknown` | a dynamic value is not a parent state field or `For` binding |
| `property-type-mismatch` | constant's kind does not match the field's declared type |
| `duplicate-property-key` | same key passed twice |

Know-not for dynamic sources: a `For` binding's element type is
not tracked yet, so a local source skips the type match (the
parent-state field sources are fully type-checked).

What an instance still takes nothing of: an `on:` handler
(`instance-opaque` — the child wires its own events), children,
and `role:`/`label:`.

- an unknown name is refused (`component-kind-unknown`),
- a cycle — `A` instantiating `B`, `B` instantiating `A` — is
  refused at compile time (`component-cycle`) because the emitted
  mount would recurse forever.

Since Tier 15, a parent re-render does **not** reset its
instances: each instance lives in a *slot* — one persistent
wrapper per site (static sites get one key; `For` bodies get one
per list index, pruned when the list shrinks). A re-render moves
the same wrapper element into the fresh subtree — a DOM move
keeps the child's whole tree, listeners, and closure state —
so a child's interactions survive the parent's renders. Constant
properties are initial values only, never re-applied; dynamic
properties re-apply on every parent render through the child's
returned `__cui_update` handle.

```cancho-ui
component Page {
    state extra: String = "extra A";
    state bump: Int = 0;
    on Bump { bump = bump + 1; }
    view {
        Column {
            Text { "page ({bump})" }
            Button { label: "bump page" on: Bump }
            HelloCard { title: extra }   // survives the bump, title re-applies
        }
    }
}
```

Three know-nots remain (one healed above):

- **Slot keys are positional.** A `For` slot is keyed by list
  index, not by element identity (there is no `key:` binding
  yet); reordering a list updates the display but keeps the
  per-position state.
- ~~**Slot removal does not cancel work.**~~ **Tier 17: it does.**
  Slots pruned by a shrinking list are *disposed* — their pending
  awaits resolve `Cancelled` so the component's own `match` arm
  runs (§3), their children dispose transitively, and their slot
  keys are **forgotten**: growing the list back mounts a fresh
  component rather than resurrecting the disposed one.
- **A `For` body records one instance per index.** The prune
  disposes the last instance site recorded at an index; two
  component instances in one `For` body would leave the first
  un-disposed. Single-instance `For` bodies (the normal shape)
  are fully covered.

A component that awaits or hosts instances returns a
`__cui_dispose` handle alongside `__cui_update` (only
prop-consuming components return the update). A component that
neither awaits nor hosts instances and receives no properties
returns nothing — there is nothing to cancel in it, and its
parent's prune skips it (`if (inst)`).

Cross-file `import` (Tier 16) is below.

### Cross-file composition: `import` (Tier 16)

A component of a sibling module file can be imported and
instantiated like a same-file one:

```cancho-ui
// dashboard.cui — beside cards.cui, which declares `module cards;`
module dashboard;

import cards.HelloCard;

component Page {
    view {
        Column {
            HelloCard { title: "imported" }
            HelloCard { title: "local" }
        }
    }
}
```

The import is compile-time: the imported file's components merge
into the one emitted JS module (imported first, entry last — the
auto-mount stays the entry file's last component), and the entry
file sees the imported component's state fields, so every property
rule applies identically across the boundary
(`property-unknown-key`, `property-type-mismatch`, dynamic
sources). Tier 15 slots work across the boundary too. Resolution
is a sibling adjacency, deterministic and dependency-ordered:

| rule | misuse |
|---|---|
| `import-missing-module` | `import cards.X;` names a sibling `cards.cui` that is missing — or does not declare `module cards;` |
| `import-unknown-component` | the module file does not declare that component |
| `import-cycle` | the import chain reaches a file already on the resolution stack |
| `component-name-collision` | the same component name is declared in two files of the graph — both would emit a `mount_<Name>` |

Refusals inside an imported file cite that file's path in their
span; refusals at the import site cite the importing file.

Know-nots:

- **No paths, no packages.** `<module>.cui` beside the importing
  file is the only resolution rule; import directories, `as`
  aliases, and pub/visibility come with Phase 6.
- **Bring what you instantiate.** A file may instantiate exactly
  the components its own `import` lines name — a component used by
  a transitive module is not reachable from an importing file
  unless that file imports it by name from its own sibling
  directory.

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