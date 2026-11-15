# Keys a program binds: `Key { press: "Shift+ArrowDown" on: Extend }`

> **Status: built (15 Nov 2026), measured in section 6; designed before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1).** Slice F6 of the gaps found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F6): the built-in elements handle their own keys (Enter in a field, `controls.md` section 3; arrows in a `SortList`, `crates/cancho-ui-codegen-web/src/sortlist.rs`), so a page cannot
> write a grid that moves with the arrows or a range that grows with Shift and the arrows. Every claim below is a **choice**, a **read** (with the place) or **open**; section 6 holds what was measured, and where it
> changed a line of the design the line is corrected in place.

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
  takes the keyboard and is not named is a trap for a screen reader. A `Key` outside a container (the component's top level, or a direct child of a `For` or an `If`) is `key-outside-container`; one with children is `key-child-invalid`; one without `on:` is `key-event-missing`, and one without `press:` is `key-combo-invalid`. `disabled:` is a `Bool` expression as on a button: a disabled key is
  not bound at that render. *Choice: a child element, not a map on the parent, because the grammar has no map literal and one binding per element; the cost is one element per key.*
* **`press:` is a text literal of a closed grammar:** `Control+` then `Shift+` (each optional, in that order) then one of `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown`, `Enter`, `Space`, `Escape`, `Delete`,
  `Backspace`, or one lowercase letter or digit. `Control+` goes only with an arrow, `Home` or `End` (the keys of the ARIA grid pattern, and `Control+Shift+ArrowDown` is a block selection in a grid); `Shift+` goes with any named key but not with a
  letter or a digit (`Shift+a` is a capital letter, typed, not a key to bind); a letter or a digit takes no modifier. Anything else is `key-combo-invalid`; `Tab` is never bindable (a program must not trap the focus); `Alt+` and `Meta+` are not
  bindable (the browser's and the system's). Two `Key`s of one container with the same combo are `key-duplicate`. *Choice: a closed list so that a combination is a thing the checks can read, and a page cannot take over
  the browser's own shortcuts.*
* **When a bound key fires.** A `keydown` reaching the container, from the container or from inside it, fires the event and calls `preventDefault` (so an arrow does not also scroll). It does **not** fire: when the key
  came from an editable element (`input`, `textarea`, `select`, `contenteditable`) inside the container, so a field in a grid still types; while an IME composes (`isComposing`); for an auto-repeat of a key that is not
  an arrow, `PageUp` or `PageDown` (one `Delete` held down is one deletion, and an arrow held down walks); with a modifier the combination does not name (`Shift+ArrowDown` is not `ArrowDown`, and
  `ArrowDown` is not fired by `Control+ArrowDown`); nor from outside the container. **The innermost container that binds the combination takes it**: after firing, the listener stops the event, so a key of a container inside another does not also fire the outer one's
  (a container that does not bind the combination lets it through). *Choice; the shape follows `submit:` (`controls.md` section 3). The nesting rule was found by the first run of the browser check, which had written the
  second container inside the first and saw both fire.*
* **A key is a way to fire an event, as `submit:` is** (`controls.md` sections 2 and 3): the event is the same one a button would fire, replay fires it by name, and the rule that an event no control fires is refused
  (`event-unused`) counts a `Key` as a control. A `Key` naming an event with a parameter list the call does not match is the existing arity refusal.
* **Nothing about the accessibility tree changes except what the program writes:** the container has the role it was given (a `grid` or a `list` is `role:`), and the active cell is the program's to say with `pressed:`
  or text. Moving focus to a cell, or `aria-activedescendant`, is **not here** (open): the container keeps the focus and the page says which cell is current.

## 2. What the web backend does

One `keydown` listener on the container **for each `Key`** (the first draft said one for all of them; one each is simpler and costs a function per key), written by `crates/cancho-ui-codegen-web/src/keys.rs`. The
listener compares `e.key` (a single letter lowercased, a space as `Space`) with the key, and `e.ctrlKey` and `e.shiftKey` with the two modifiers, exactly; `e.altKey` or `e.metaKey` set means no match. The arguments
of `on: Move(1)` are constants of the render, as for a button's. **A container that holds `Key`s is not prerendered** (`recipe.rs`): the page draws the whole of it with script, since the listeners are on it; the first
build wrote the container into the page and left each `Key` a hole outside it, which threw at run time. Closing that is one rule in the prerenderer and a test in the browser check.

## 3. Refusals

`key-parent-unnamed`, `key-outside-container`, `key-combo-invalid`, `key-duplicate`, `key-child-invalid`, `key-event-missing`; the existing `event-arity-mismatch` and `disabled-not-bool`; the existing `event-unknown` for an event that is not declared, `binding-unknown` for `press:` on anything but a `Key`.

## 4. The checks, written before the code

`crates/cancho-ui/tests/keys.rs` (compiler, replay) and `scripts/check-keys.js` (Chromium):
* the six refusals and the combo grammar with each way to leave it (an uppercase letter, `Shift+a`, `Alt+x`, `Tab`, `Control+Enter`, an empty text, two modifiers in the wrong order); the emitted listener only for a
  program with a `Key`; a trace that fires the events by name and moves a cursor;
* **in Chromium**: ArrowDown moves the cursor and the page does not scroll; `Shift+ArrowDown` extends a range and is not `ArrowDown`; `Control+ArrowDown` is neither; a key typed in a `TextField` inside the container
  fires nothing and the text arrives; a key pressed outside the container fires nothing; a held ArrowDown (three auto-repeats) fires three events and a held Delete fires one; a key during an IME composition fires
  nothing; a disabled `Key` is not bound; the keys of two containers do not cross;
* **mutants**: the editable-target rule removed; `preventDefault` removed; repeat allowed for every key; repeat refused for arrows; `Shift` ignored; `Control` ignored; the container not required to have a name; the
  duplicate check removed; the listener on `document` instead of the container; the container not made focusable; `stopPropagation` removed; the container left in the prerendered page.

## 5. Not here

Moving the focus to a cell, `aria-activedescendant`, roving `tabindex`; key sequences (`g` then `g`); a key bound for the whole page; `Alt+` and `Meta+`; remapping by the user; `keyup`; a `Key` inside a `Table` (its children are rows: put the keys on a container around the table).

## 6. Measured (2026-11-15)

Machine: the development container, Chromium from the Playwright installed there, one run each.

* `cargo test -p cancho-ui --test keys` (4 tests): the grammar, with 20 combinations accepted and 21 refused (`Tab`, `Alt+`, `Meta+`, `Control+Enter`, `Shift+a`, uppercase, the modifiers in the wrong order, a trailing space ..); one
  listener for each key of the example and none in a program without keys; the trace `examples/keys/keys.trace.json` replays (6 steps, with arguments `Move(-1)`); each refusal with its code, including the event checks
  a key shares with a button (`event-unknown`, `event-arity-mismatch`).
* `node scripts/check-keys.js`: 29 checks pass on `examples/keys`. ArrowDown moves the cursor and **the page does not scroll**; Shift+ArrowDown moves the cursor and keeps the anchor; Control+ArrowDown,
  Control+Shift+ArrowDown, Alt+ and Meta+ fire nothing; typing in the field inside the container, and ArrowDown and Delete in it, fire nothing; a key outside the container fires nothing; the key of the inner
  container fires its event and not the outer one's; three auto-repeated ArrowDowns walk three rows and auto-repeated `x` fires nothing; an IME composition, a capital `X` with Shift, `x` with Alt and an unbound key
  fire nothing; a disabled key fires nothing and the others still do; the container keeps the focus after each redraw.
* **A defect found by the scroll check, in code that is not new:** giving the focus back after a redraw (`__cui_focus_restore`) used `focus()`, which scrolls the element into view, and the page's own scroll is
  put back only when it was not zero. A control a little out of view at the top of a page moved the page by 16 px at every redraw. It now uses `focus({ preventScroll: true })`; `check-lists`, `check-controls`
  and `check-focus` still pass.
* `node scripts/check-keys-mutants.js`: **17/17 killed** (the listing of section 4, with the two for `Control` and `Shift` on letters and the three for Alt/Meta, an IME composition and a disabled key). Three runs were wrong first
  and are kept here: `preventDefault` removed **survived** (the redraw replaces the focused element, so the browser's own scroll is not a witness; the check now reads `defaultPrevented` after every listener, in the
  capture phase of the window because a bound key stops before it bubbles that far); the pattern for the Alt/Meta mutant did not match the formatted source and was skipped (a skipped mutant counts as a survivor);
  and the mutant that moved the listener to the `document` **hung** the page, which hung the run (each browser run now has two minutes, and a timeout is a kill).

