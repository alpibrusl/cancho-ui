# Row actions: an expression label and `on: Event(args)`

> **Status: built (issue #41, found building `examples/ops-console`, `docs/ops-console.md` 2.7).** A row of a list could
> not be clicked to select it, because a `Button` label had to be a string literal and `on:` could not give its event
> the element of the row. The operator typed the machine id into a field.

## 1. The decision

```
Button { label: "Select " + n on: Pick(n, len(names) - 1) }
```

* **`label:` is a string literal or an expression.** A lone string literal is the static label it always was (a `{` in it
  is still refused). Anything else is an expression of the view (a name, a concatenation, a call), evaluated at each
  render: the accessible name (`aria-label`) and a button's visible text are one `String(...)` of it, computed once.
  Any element takes it; the IR has `label_expr` beside `label`, present only for an expression label, so the IR of a
  program that has none is byte-for-byte what it was (checked).
* **`on: Event(a, b)` passes arguments.** Each argument is an expression of the view, evaluated **at render time** into a
  constant of that render; the click calls `__on_Event(a, b)` with those constants. A row therefore names the element it
  was drawn for, not whatever the loop variable holds when the click lands (checked: after `Reverse`, the first row picks
  the former last element). Arguments may read state and `For` bindings; they run no effect that suspends.
  `Event` is declared by the component with exactly that many parameters.
* **A click has no arguments of its own.** `on: Pick` (bare) on a `Button` for a handler that takes parameters used to
  hand the DOM `MouseEvent` to its first parameter, silently. It is refused now (`event-arity-mismatch`: write the
  arguments). `examples/match/match.cui` did exactly that (`on: Set` for `on Set(value)`, which stored a `MouseEvent`);
  it says `on: Set(5)` now, which is what its label always claimed.
* **A field still gives the typed value.** `TextField { on: Change }` passes the value to a handler with one parameter; a
  handler with two is `event-arity-mismatch`. Arguments are for buttons (`event-args-unsupported` elsewhere).
* **Replay needed no model.** It never rendered the view: a trace fires `Pick` with `params`, by name. The one change is
  that `disabled:` of a control with `on: Pick(..)` disables `Pick` like the bare form does.

## 2. Refusals

| tag | when |
|---|---|
| `event-unknown` | `on: X` (with or without arguments) where the component has no `on X`. It used to compile, and the page died on the first render (`__on_X is not defined`). |
| `event-arity-mismatch` | the handler's parameters are not the arguments given: `on: Pick("a")` for `on Pick(id, pos)`, a bare `on: Pick` on a button for a handler with parameters, a handler with two parameters on a field |
| `event-args-unsupported` | `on: X(..)` on anything but a `Button` |

An instance of a component takes neither a `label:` expression nor `on:` with arguments (`InstanceTakesAccessibility`,
`InstanceTakesBindings`, as for the literal forms). `submit: X(..)` is `parse-unexpected-token`.

## 3. Measured

`scripts/check-rows.js` (Chromium, `examples/rows` generated with 100 and with 1000 rows):

* 100 buttons, each named by its element; the visible text is the accessible name for every one; a click names its own row
  (element and position); a click after a re-render still does; after `Reverse` the first row is the former last; an
  element holding `a"b` and one holding `<img src=x onerror=alert(1)>` are button names and arguments, not markup (no
  `<img>` in the page, no dialog).
* **Cost.** One closure per button, no event delegation. The median of 30 clicks that re-render every row (the render is
  synchronous with the click, checked in the page): **100 rows 0.5 ms with buttons and 0.2 ms as text; 1000 rows 2.6 ms
  with buttons and 4.3 ms as text**: the buttons are within the noise of the text at 1000 rows, so delegation would buy
  nothing measurable and is not built. The script fails above 250 ms for 1000 buttons.
* The ops console has a `Select M-xxx` button on every row of the page: it fills the machine id (`scripts/check-ops-console.js`),
  and its 38-step trace has the `Pick` step.

Eight deliberate breaks, each caught (cargo `row_actions`, and `check-rows.js` where it matters): the emitter drops the
arguments (browser 5 checks), an empty `aria-label` (browser 1), no arity check, no `event-unknown`, replay ignoring
`EventCall` for `disabled:`, a label with `+` parsed as a literal (3 tests), `label_expr: null` always written (the IR
stability test), the diff not showing the arguments.

## 4. Found while building it

1. **A bare `on:` on a button passed the `MouseEvent`.** See 1. Fixed by `event-arity-mismatch`; one example corrected.
2. **An unknown event compiled and killed the page on render.** Fixed (`event-unknown`).
3. **A name that is not in scope in a view expression passes `check` and kills the page** (`String(__state_nothere)`).
   Not fixed here; the same kind of defect as `call-unknown` (2.1 of `docs/ops-console.md`). Issue #55.

## 5. Not done

* An empty dynamic label is not refused: `accessibility-label-empty` is a contract that was never implemented (`AGENTS.md`
  section 7), for literals either, and replay does not render the view. A label that evaluates to the empty text gives a
  button with no name.
* An event argument is not typed: a handler's parameters are not (`on Pick(id, pos)`), so `Pick(n)` where it wants a
  number is not caught.
* `on:` with arguments on `TextField` (the typed value plus extra arguments) and `submit:` with arguments.
* Keys (`key:` for a row), so a re-sorted list keeps a row's focus: the rows are rebuilt on every render, as they were.
