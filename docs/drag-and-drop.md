# Drag and drop, and reordering with a keyboard (issue #80)

> **Status: BUILT** (2026-10-24, issue #80). The design (sections 1 to 5) was written first and merged as #131; sections 6 to 8 say what the build made, what it measured and what it corrected. A sentence of the design that the build contradicted is corrected
> in place and says so. Numbers of section 2 measured with `scripts/measure-dnd.js` (Chromium 141, `Pixel 7` profile: touch, 412 px) on main at d31135a. What was not measured is labelled **assumption**.

## 1. What exists

A page reacts to a button's activation and to a text field's input; nothing else. `Button { label: "Drag" ondrop: Go }` is `parse-unexpected-token`. The page is rebuilt from scratch after every state change (`__render`), keeping the focus only for a control that comes back with the
same tag and the same accessible name (`screen-focus.md`, rule 1). A list can be reordered today only by writing a button per row and moving things with `take`, `skip` and `concat`; there is no builtin that moves an element and no index in a `For`.

## 2. Measured

| question | measured |
|---|---|
| **HTML5 drag and drop with a mouse** (`draggable="true"`, `dragover` and `drop` handlers) | works: `pointerdown`, `dragstart`, `pointercancel`, `dragenter`, `dragover`, `drop`, `dragend` reach the right elements |
| **HTML5 drag and drop with a finger** (touch sequence sent by the DevTools protocol on `draggable` items, held 0 ms and 700 ms before moving) | **no `dragstart`, no `drop`**: `pointercancel` at the first move (the browser takes the gesture to scroll), then `touchend`. Emulated touch is not a real Android Chrome (**assumption**: a real long press may start a drag; not measured) |
| **Pointer events with a finger**, `touch-action: none` on the list | the whole sequence: `pointerdown`, five `pointermove`, `pointerup`; with the default `touch-action` the same drag is `pointercancel` at the first move and the page scrolls |
| **The view is rebuilt while a mouse drag is in progress** (what any state change does: a timer, a stream message, a response) | the list under the pointer is replaced 50 ms into the drag: **no `drop` reaches the new item, and `dragend` reaches only the removed source** (not the new one). A drag does not survive a rebuild |
| **Keyboard on a `draggable` item** (Space, ArrowDown, Enter) | no drag event: HTML5 drag has no keyboard path at all |
| **A button that moves a row up, in a page rebuilt from scratch** (plain DOM, three rows) | the focus is on the `body` afterwards |
| **The same in a compiled page**, three rows with a `Button { label: "Up " + name }` that reorders three state fields, Enter on the button | **the focus is on the `body` after 3 presses of 3**: the button is rebuilt with another label (the name of what is now in that row), and the focus is only given back to a control with the same name |

Two facts from the table decide the design. A drag that the page does not own is lost at the next rebuild, so the page must own the drag (pointer events, state kept outside the rebuilt tree) and must not rebuild under it. And moving a row with a labelled button strands the keyboard user,
because the label follows the content: the page must put the focus where the moved row went.

## 3. The decision

1. **One element, the minimal useful case: reordering a list.**

   ```
   SortList { label: "Tasks" items: tasks on: Moved }
   ```

   `items:` is the name of a `List[String]` state field (the rows' text); `label:` names the list for a screen reader; `on:` is an event that takes exactly two parameters, the **indexes** `from` and `to` (0-based `Int`, never DOM objects), `on Moved(from, to) { tasks = move(tasks, from, to); }`.
   The program owns the list; the page proposes a move and the program accepts it by changing the state, so a program can refuse (not change the list) and the page then shows the list as it is. Tags: `sortlist-incomplete` (no `label:`, `items:` or `on:`),
   `sortlist-binding-invalid` (`items:` that is not a `List` field, an `on:` event that does not take two parameters, a `label:` that is not text), `sortlist-in-loop` (one list per place, as for `Chart`).
   (Corrected in the build: an `on:` event that does not take two parameters is the existing `event-arity-mismatch`, as for every control, and `sortlist-binding-invalid` is only for `items:`; a missing `label:` is `sortlist-incomplete`; `items:` on another element and any key a `SortList` does not take are `binding-unknown`.)
2. **A builtin to make that one line.** `move(list, from, to)`: the list with the element at `from` taken out and put at `to` (an index out of range, or `from == to`, leaves the list unchanged). Pure; replay and the page share one definition and the corpus holds it.
3. **The keyboard and screen-reader path is the element's, not the program's.** Every row has two buttons, "Move *name* up" and "Move *name* down" (the first row's *up* and the last row's *down* are disabled), reachable with Tab, activated with Enter or Space, at least 44 px.
   They fire the same `on:` event with `(i, i - 1)` or `(i, i + 1)`. This is the whole of the 2.1.1 (keyboard) and 2.5.7 (a dragging movement has a single-pointer alternative) answer; there is no gesture that exists only with a pointer.
4. **Focus follows the row.** After a move made with a button, the focus goes to the same button (up or down) of the row's **new** position, so that pressing it again keeps moving the same row; at the end of the list, to the opposite button. (Measured above: without this the focus is lost every time.) After a drag, the focus is on the dropped row's handle.
   (Corrected in the build: the handle is decorative, `aria-hidden`, and not a tab stop, so there is nothing to focus: **a pointer gesture moves no focus**. Only a move with a button moves it.)
5. **Announcement.** A polite live region that outlives the rebuild (it is not inside the tree the page rebuilds; #106) says "*name*, position 2 of 5" after every accepted move. **Assumption**: that a screen reader speaks it; no screen reader was run. (Built: the text, the buttons' names and the handle's are English; a localized program cannot change them yet, section 8.)
6. **Pointer drag, for mouse, pen and finger, with Pointer Events** (measured: complete for a finger only with `touch-action: none`). The handle at the start of a row (44 by 44 px, `touch-action: none` on the handle alone, so that the rest of the list still scrolls the page) starts the drag; the row follows the pointer; the target
   index is the row whose middle the pointer has last passed; releasing fires `on:` once with `(from, to)`; Escape or `pointercancel` cancels it and fires nothing. HTML5 drag and drop is not used (measured: it is not delivered to a finger, and has no keyboard path).
7. **The page does not rebuild under a drag.** While a drag is in progress `__render` is deferred and runs once, after the drop or the cancel (measured: a rebuild mid-drag loses the drop). State changes that arrive meanwhile are applied then.
8. **Replay.** A move is the plain event `{"event": "Moved", "params": [2, 0]}`; replay refuses a move the page could not make: an event fired only by a `SortList` whose parameters are not two different integers inside the list as it is now (`trace-move-invalid`), as it refuses a value not among a `Select`'s options. `expect` may assert `sortlists`
   (label, count and the rows in order, which is more than `charts` reports: a trace can say what order a drag left). Replay cannot see a drag or a focus; Chromium checks both.
9. **Authority.** None: no capability, nothing leaves the page.

## 4. What the build must have

The element, `move`, the three element tags and `trace-move-invalid`, each reached by a test; a shared corpus for `move` (every position, out of range, empty and one-element lists, equal indexes) run in replay and in Chromium; a Chromium check with the Pixel 7 profile: a mouse drag, a touch drag (DevTools touch events
on the handle), the buttons with the keyboard (Tab, Enter, Space, the disabled ends, the focus on the right button after each move, three moves in a row), Escape cancelling, a state change in the middle of a drag (a timer) not losing the drop, the live region's text, the page still scrolling when a finger moves on a row outside its handle,
a list of 200 rows, a right-to-left page (the handle at the start); deliberate breaks (the index off by one, the drop fired twice, the render not deferred, the focus not moved, `move` clamping the wrong way, the disabled ends enabled); `examples/reorder`; a watchdog in every script.

## 5. Not measured, and not done

A real finger on a real phone (everything above is DevTools-protocol touch in a desktop Chromium); a real Android long press starting an HTML5 drag; a screen reader speaking the live region or the buttons' names; a drag across a scroll (auto-scroll at the edge of the viewport, a list longer than the screen);
dropping an item **into one of several targets** (the issue's second case: left out of this build, the keyboard alternative of a target is a `Select` or buttons the program already writes; a design for `DropZone` is a follow-up if it is wanted); reordering rows that are not text (a row of several elements); dragging between two lists; undo; Firefox and Safari.

## 6. What was built

* **Language.** `SortList { label: .. items: .. on: .. }` (the element kind, `ir/src/sortlist.rs` for its keys and tags), `move(list, from, to)` (a pure builtin; replay's `move_item` and the page's `__cui_move` share `tests/move_corpus.json`, 24 cases written by hand). Three tags; `trace-move-invalid` in replay.
* **Page** (`codegen-web/src/sortlist.rs`): a `<ul aria-label>` of rows with a handle, the text, and the two buttons; Pointer Events for the drag (capture on the handle, `touch-action: none` on the handle alone); a `__render` that waits while a drag is in progress; one polite live region outside the rebuilt tree;
  its style is injected once by the adapter, so the element looks the same in a bare module as in a built page. Logical properties only (`padding-inline-end`), so the handle is at the start of the row in both directions.
* **Replay** (`replay/src/sortlist.rs`): the `sortlists` report and the check of a move against the list as it is now (a list under an `If` that is false is not in the page; a move is not checked when a different control also fires the event, because that control could give it anything).
* **Example and gallery:** `examples/reorder`, now a tile of the landing page; the coming-soon tile for #80 is removed.

## 7. Measured at the build

`scripts/check-sortlist.js` (Chromium, parts A to F) passed 6 of 6 runs after
one fix: part E raced the page's 100 ms tick, so it read the list before the
tick had finished and failed intermittently; it now waits for the state it
asserts instead of for time. `scripts/breaks/sortlist.json` holds 20 breaks
(replay move, page move, trace acceptance of bad moves, the three rule tags,
arity, rebuild under a drag, focus, disabled ends, Escape, `touch-action`,
double drop, announcement, nearest row, handle size, button names, handle
order). All 20 were caught by a check failing on its own; none was a kill,
`HUNG` or `BUILD`. Real finger long press, screen reader speech, auto-scroll
and Firefox/Safari remain assumptions (section 8).

## 8. Not done

The second case of the issue, **dropping an item into one of several targets** (a follow-up if wanted); rows that are not text; dragging between two lists; auto-scroll at the edge of the viewport while dragging (**assumption** that a list longer than the screen is reordered with the buttons, which scroll the page by focusing the row, or by a drag within the screen; a long drag
across a scroll was not measured); the built-in words ("Move *x* up", "position 2 of 4") are English and not the program's to change (the `messages` of `i18n.md` do not reach them yet); a real finger on a real phone and a real long press (**assumption**, not measured); a screen reader speaking the live region and the button names (**assumption**);
Firefox and Safari (only Chromium here); undo.
