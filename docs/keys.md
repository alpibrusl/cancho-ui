# Keys a program binds: `Key { press: "Shift+ArrowDown" on: Extend }`

> **Status: designed (15 Nov 2026), written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Nothing is built.** Slice F6 of the gaps found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F6): the built-in elements handle their own keys (Enter in a field, `controls.md` section 3; arrows in a `SortList`, `crates/cancho-ui-codegen-web/src/sortlist.rs`), so a page cannot
> write a grid that moves with the arrows or a range that grows with Shift and the arrows. Every claim below is a **choice**, a **read** (with the place) or **open**; none is measured yet, and a measurement will replace its
> line in place.

## 1. The decision

```
Column {
    label: "Cells"                                  // the container that holds the focus: it must have a name
    Key { press: "ArrowDown" on: Move(0, 1) }
    Key { press: "ArrowUp" on: Move(0, -1) }
    Key { press: "Shift+ArrowDown" on: Extend(0, 1) }
    Key { press: "Control+Home" on: First }
    TextField { label: "Filter" value: query on: SetQuery }     // typing here never fires a Key
    ...
}
```

* **`Key { press: "<combo>" on: Event }` is an element that draws nothing.** It binds the key to an event of the component for the element that **holds** it (its parent). `on:` is an event, or an event with constant or
  row-bound arguments as everywhere (`row-actions.md`): `Move(0, 1)`. The parent is made focusable (`tabindex="0"`) unless it already is, and **must have a name** (`label:`), or it is `key-parent-unnamed`: a place that
  takes the keyboard and is not named is a trap for a screen reader. A `Key` outside a container (the component's top level) is `key-outside-container`. `disabled:` is a `Bool` expression as on a button: a disabled key is
  not bound at that render. *Choice: a child element, not a map on the parent, because the grammar has no map literal and one binding per element; the cost is one element per key.*
* **`press:` is a text literal of a closed grammar:** `[Shift+|Control+]` then one of `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown`, `Enter`, `Space`, `Escape`, `Delete`,
  `Backspace`, or one lowercase letter or digit. `Control+` goes only with an arrow, `Home` or `End` (the keys of the ARIA grid pattern); a letter, `Enter`, `Space` and the rest take no modifier but `Shift+` where it
  means something (`Shift+Space` is allowed, `Shift+a` is not: that is a capital letter, typed). Anything else is `key-combo-invalid`; `Tab` is never bindable (a program must not trap the focus); `Alt+` and `Meta+` are not
  bindable (the browser's and the system's). Two `Key`s of one container with the same combo are `key-duplicate`. *Choice: a closed list so that a combination is a thing the checks can read, and a page cannot take over
  the browser's own shortcuts.*
* **When a bound key fires.** A `keydown` reaching the container, from the container or from inside it, fires the event and calls `preventDefault` (so an arrow does not also scroll). It does **not** fire: when the key
  came from an editable element (`input`, `textarea`, `select`, `contenteditable`) inside the container, so a field in a grid still types; while an IME composes (`isComposing`); for an auto-repeat of a key that is not
  an arrow, `PageUp` or `PageDown` (one `Delete` held down is one deletion, and an arrow held down walks); with a modifier the combination does not name (`Shift+ArrowDown` is not `ArrowDown`, and
  `ArrowDown` is not fired by `Control+ArrowDown`); nor from outside the container. *Choice; the shape follows `submit:` (`controls.md` section 3).*
* **A key is a way to fire an event, as `submit:` is** (`controls.md` sections 2 and 3): the event is the same one a button would fire, replay fires it by name, and the rule that an event no control fires is refused
  (`event-unused`) counts a `Key` as a control. A `Key` naming an event with a parameter list the call does not match is the existing arity refusal.
* **Nothing about the accessibility tree changes except what the program writes:** the container has the role it was given (a `grid` or a `list` is `role:`), and the active cell is the program's to say with `pressed:`
  or text. Moving focus to a cell, or `aria-activedescendant`, is **not here** (open): the container keeps the focus and the page says which cell is current.

## 2. What the web backend does

One `keydown` listener on the container for all its `Key`s, written once, with the combos as a literal table `{ "Shift+ArrowDown": [handler] }` read at each render for the `disabled:` ones. The listener reads
`e.key`, `e.shiftKey`, `e.ctrlKey` and builds the combo name from those only; `e.altKey` or `e.metaKey` set means no match. The prerenderer writes the container with its `tabindex` and name and leaves the listener to the
script, as for every control (`prerender.md`). *Read; to be measured.*

## 3. Refusals

`key-parent-unnamed`, `key-outside-container`, `key-combo-invalid`, `key-duplicate`; the existing `event-unknown` for an event that is not declared, `binding-unknown` for `press:` on anything but a `Key`.

## 4. The checks, written before the code

`crates/cancho-ui/tests/keys.rs` (compiler, replay) and `scripts/check-keys.js` (Chromium):
* the four refusals and the combo grammar with each way to leave it (an uppercase letter, `Shift+a`, `Alt+x`, `Tab`, `Control+Enter`, an empty text, two modifiers in the wrong order); the emitted listener only for a
  program with a `Key`; a trace that fires the events by name and moves a cursor;
* **in Chromium**: ArrowDown moves the cursor and the page does not scroll; `Shift+ArrowDown` extends a range and is not `ArrowDown`; `Control+ArrowDown` is neither; a key typed in a `TextField` inside the container
  fires nothing and the text arrives; a key pressed outside the container fires nothing; a held ArrowDown (three auto-repeats) fires three events and a held Delete fires one; a key during an IME composition fires
  nothing; a disabled `Key` is not bound; the keys of two containers do not cross;
* **mutants**: the editable-target rule removed; `preventDefault` removed; repeat allowed for every key; repeat refused for arrows; `Shift` ignored; `Control` ignored; the container not required to have a name; the
  duplicate check removed; the listener on `document` instead of the container.

## 5. Not here

Moving the focus to a cell, `aria-activedescendant`, roving `tabindex`; key sequences (`g` then `g`); a key bound for the whole page; `Alt+` and `Meta+`; remapping by the user; `keyup`.
