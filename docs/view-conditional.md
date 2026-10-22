# A conditional in the view: `If { when: cond ... }`

> **Status: BUILT** (alpibrusl/cancho-ui#90). Found building `examples/tickets` (`docs/tickets.md` section 3). The design came
> first; section 7 says what was built, what it measured and where the build corrected the design.

## 1. What was measured before the change

* A view cannot choose a node. `view { Column { if flag { Text { "shown" } } Button { label: "Go" on: Go } } }` is
  `error[parse-unexpected-token]: expected binding or view node, found "if"`. `if` (`docs/conditionals.md`) is an expression of a
  value; a view node is not one.
* So `examples/tickets` (sign-in, then the orders, then an order's detail) wrote a screen as a `List[String]` that holds `["x"]`
  while the screen is shown and `[]` while it is not, and drew it with `For { over: show_signin as: s Column { .. } }`. Counted in
  that file: 3 state fields that exist only for this, 6 assignments of them in 3 handlers (`SignIn` 2, `Pick` 1, `SignOut` 3),
  each of which must agree with `signed` and `picked`, which already say the same thing. Two of the three lists had to be kept in
  step with a third field by hand: the fault is a screen that is both shown and not.
* Replay does not know about the screens at all: it asks only whether a control that fires the event is `disabled`
  (`docs/controls.md` section 2). A trace can fire `Save` while the detail screen is not in the page.

## 2. The options

| | Option | What it adds | Against |
|---|---|---|---|
| A | **A view node `If { when: cond children }`** | one node; children in the page only while `cond` holds | a new reserved name (`If`, as `For` is) |
| B | `when:` on any element (`Column { when: signed .. }`) | no new node | hides one element, so a group of siblings needs a wrapper `div` that changes the layout (`Column` gap, a `Table`'s structure); every element kind grows a binding |
| C | **Lower `If` to `For` over `if cond { [0] } else { [] }`** (no new IR) | nothing in the IR, emitter or replay | `For` reads as a loop and binds a name nothing uses; replay would still not know a control is gone; the diagnostics name a `For`. Kept for one case (section 5) |
| D | `If { when: c .. } Else { .. }` / `else` inside the node | the complement without repeating the condition | the screens of the example are `!signed` and `signed`: one extra `!`. Additive later, so not now |
| E | Do nothing | | the trick stays; a reader must be told it |

**Chosen: A.** B was rejected for the wrapper, C for the lie it tells the reader and replay, D as not needed by the measured
program (it can be added without changing anything A means).

## 3. The form and its rules

```
If {
    when: signed
    Column { ... }          // any view nodes, as a For's body
}
```

* `when:` is the first and only binding, and is required. It is a pure expression: no effect, no `await`, no `sleep`
  (lowering refuses it as it refuses a `For`'s `over:`). It is evaluated at every render, so it is read from the state as it is
  then.
* It must be a `Bool`. A type the pass knows to be wrong is refused with `condition-not-bool` (the tag of
  `docs/conditionals.md`; the message names "the `when:` of an `If`"); a name that is not in scope is `name-unknown`. A type the
  pass cannot see (a `For` binding, a `match` arm) is not refused, as everywhere (`docs/conditionals.md` section 6).
* A missing or misplaced `when:` is `parse-unexpected-token`, as a `For` without `over:` is.
* The children are the view nodes a `For` allows: elements, text, `For`, `If`. An `If` may stand inside a `Table`, where it makes
  rows (or cells), like a `For`.
* **Absent, not hidden.** While `cond` is false the children are not built: no node in the DOM, so nothing for a screen reader to
  find and no listener (measured, section 7). The emitted code is `if (cond) { ... }`, with no style that hides.

## 4. How replay and the page agree

The two evaluators of the language are `cancho-ui test` (replay) and the emitted page. Replay does not draw, so what it can agree
on is **which controls exist**, and that decides one thing: whether an event can be delivered.

* **A new outcome, `hidden`.** A control that is inside an `If` whose condition is false is not in the page. If every control
  that fires the event is like that, the browser could not have delivered the event; replay answers `outcome: "hidden"`, runs no
  handler and changes nothing. `disabled` keeps its meaning (a control is in the page and `disabled:`); the two are different
  facts and a trace asserts which. An event no control fires (a schedule's) is still never refused.
* **Several controls.** The event is delivered while any control that fires it is in the page and enabled.
* **Enter in a field (`submit:`)** is blocked by a disabled `on:` button of the same event (`docs/controls.md` section 3), and the
  button must be in the page to block it: a hidden disabled button does not stop Enter. The emitted `__submit_blocked` guard
  therefore reads `(when && disabled)` for a button under an `If`.
* **What replay cannot see.** The condition is evaluated against the state as it is when the event is delivered. A condition
  that reads a `For`'s binding cannot be evaluated by the walk (replay does not iterate a `For` to decide delivery, today either),
  and counts as true; the guard of `__submit_blocked` ignores such a condition for the same reason. The page, which does iterate,
  is exact; the corpus keeps conditions to state.
* **Focus.** The view is drawn again from nothing at every render and focus is restored only to an element of the same place,
  tag and label. When the branch that holds the focus is removed there is no such element: the focus goes to the document body.
  That is the defined behaviour today (measured: `scripts/check-view-if.js`); a better one (a place to put it, an announcement
  of the screen change) is alpibrusl/cancho-ui#92 and is not designed here.

## 5. Component instances under an `If`

An instance keeps its state across renders (Tier 15 of the emitter: one persistent slot per instance site). Under an `If` it must also *end* when the condition turns
false: its pending requests are cancelled and the next time it is drawn it starts fresh, exactly as an instance under a `For` whose
list shrinks. That machinery belongs to `For`, so an `If` that contains an instance is emitted as a `For` over
`if cond { [0] } else { [] }` (option C, used where it is exactly right); an `If` without one is emitted as a plain `if`.
Measured: an instance counted to 2, its `If` closed and opened again, shows 0.

## 6. Cost

Emitted bytes, measured on the page:

| | before | after |
|---|---|---|
| `examples/tickets` page (`cancho-ui build`) | 44,485 | 43,706 (-779) |
| `examples/tickets/tickets.cui` | 9,063 bytes, 218 lines | 8,686 bytes, 207 lines |
| a 2-branch toggle, screen-list form | 4,432 | 4,070 (as an `If`) |

`if (c) {` replaces `for (const s of list) {`, and the three lists, six assignments and their initialisers are gone. Replay: one
extra arm in its view walk (`event_refused`), no new state. IR: `IrViewNode::If` is additive; the IR version did not change.

## 7. What was built, measured

* **Syntax and IR.** `If { when: expr children }` (`ViewNode::If`, `IrViewNode::If`), in `View`, `For` and `Table` positions.
  Checked by `condition-not-bool` and `name-unknown` in the pass of `crates/cancho-ui-ir/src/conditions.rs` (it types `when:` as it
  types an `if` condition). No new rule tag was needed; `spec/diagnostics/README.md` says where each already-catalogued tag now
  also applies.
* **Replay.** `Outcome::Hidden` (`"hidden"`), `docs/replay.md`.
* **Measured.** `tests/view_if_corpus.json`, 7 cases (a toggle, a comparison with `&&`, nesting, an `If` in a `For`, in a `Table`,
  Enter against a hidden and a visible disabled button, an event with two controls), 31 steps. It is replayed in-process by
  `cargo test` (`crates/cancho-ui/tests/view_if.rs`: `outcome` and the state) and run in the page in Chromium by
  `scripts/check-view-if.js` (the text, the buttons and the table rows after each step; a hidden control has no node; a disabled
  one is present and disabled), where the instance case and the focus case are also checked. Both agree on every step.
* **Deliberate breaks**, `node scripts/deliberate-breaks.js scripts/breaks/view-if.json` (a reusable harness: it applies each
  source edit, rebuilds, runs the checks, and restores the file): ten, each caught. The page keeping the branch when the
  condition is false (3 checks fail); replay never finding a control hidden, answering `disabled` for a hidden one, or not
  counting the controls under a false `If` (the corpus and the tickets trace fail); a hidden disabled button blocking Enter,
  an instance under an `If` not disposed, and an `If` in a `Table` not made rows (only the page can see these, so only
  `check-view-if.js` fails); `when:` not type-checked, allowed to read the world, and the semantic diff not seeing an `If`
  (`cargo test`).
* **`examples/tickets`** drops its three screen lists for three `If`s and loses its 16 trace lines about them; its trace gains
  three steps that fire a control of a screen that is not in the page (`hidden`), and `scripts/check-tickets.js` still passes.
* **Not built:** `else`/`Else` (option D); `If` as a component instance's child (an instance takes no children); replay iterating
  a `For` (so a condition on a loop binding is assumed true there); anything for focus (#92).
