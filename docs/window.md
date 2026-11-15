# A table that draws only the rows in view: `Table { total: n first: first row_height: 2 on: Scrolled }`

> **Status: built (15 Nov 2026), measured in section 6; designed before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1).** Slice F7 of the gaps found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F7): there is no virtual scrolling, so a page that shows 3,000 rows draws 3,000 (the program's answer today is `take`/`skip`, `lists.md` section 6, which pages instead of
> scrolling). Every claim below is a **choice**, a **read** (with the place) or **open**; section 6 holds what was measured, and where it changed a line of the design the line is corrected in place.

## 1. The decision

```
component Rows {
    state rows: List[String] = [...];          // 3,000 of them, in memory
    state first: Int = 0;                      // the index of the first row in view

    on Scrolled(at) { first = at; }

    view {
        Table {
            label: "Machines"
            sticky: "header"
            max_height: 24                     // rem: the box (F5a); a window needs a height
            row_height: 2                      // rem: every body row is this high
            total: len(rows)                   // how many rows there are
            first: first                       // the index of the first row drawn
            on: Scrolled                       // fired with the index of the first row in view
            Row { Text { role: columnheader "name" } Text { role: columnheader numeric: true "size" } }
            For { over: take(skip(rows, first), 16) as: r      // the program draws the rows it wants drawn
                Row { Text { "{r}" } Text { numeric: true "{len(r)}" } }
            }
        }
    }
}
```

* **The compiler does the arithmetic of a scroll box and nothing else.** The program draws the rows from `first` on, however many it likes (`take(skip(rows, first), 16)`: the rows in view and a few more, *an overscan*); the table makes
  the box as tall as `total` rows of `row_height` would be, puts the rows where they belong, and says which row is at the top when the box is scrolled. It does not slice, sort or fetch: a page that gets its rows from a
  database fires its own request from `Scrolled` (`fetch.md`), and a page that has them all in memory slices a list. *Choice: the program owns what a row is; the table owns where it is.*
* **`row_height: N`** (an integer literal of rem, 1 to 20; any other expression is `window-row-height-invalid`, a `-1` too) is the height of **every** body row. A row is exactly that tall: its cells do not wrap (`white-space: nowrap; overflow: hidden; text-overflow: ellipsis`), so a
  longer text is cut short, not taller. *Choice: the arithmetic is only right if rows are one height; text that must wrap is not a case for this table (it pages).* In rem, so the height follows the reader's text size.
* **`total:`** and **`first:`** are `Int` expressions of the view (a state field, `len(rows)`, an arithmetic of them), read at each render. `first` is clamped to `0 ..= max(0, total - 1)` by the table, not refused: a
  `first` past the end draws the rows where they would be and the box shows its end.
* **`on: Event`** is an event of one parameter, the index of the first row in view: `floor(scrollTop / row_height)` in rows, clamped to the same range. It fires when that index **changes** from the one drawn, and never
  for a scroll that stays inside the row that is already first: a scroll box does not send an event for each pixel. *Choice.*
* **The five bindings go together:** `row_height`, `total`, `first`, `on` and a `max_height` (F5a) are all needed, or it is `window-incomplete`; a `row_height` that is not an integer from 1 to 20 is
  `window-row-height-invalid`; a `total:` or `first:` that is not a number is `window-count-not-number`; an `on:` event that does not take exactly one parameter is `event-arity-mismatch` (the existing
  rule). *Choice: refused, not guessed; a table that is half a window would scroll and say nothing.*
* **A window is a `Table`**, not a list: the sticky header (`table-static.md`), the numeric alignment and the sort buttons (`table-sort.md`) are the tools of a table, and a grid of 3,000 rows is a table. A window of a
  `Column` of items is **not here** (open).

## 2. What the web backend does

The box is the scroll box of F5a (`div.cui-table-scroll` with `max-height`, plus the attribute `data-cui-window` and the variable `--cui-row` that the stylesheet reads). *The first draft said the box is always `max_height` high; it is at most that, and a table whose rows are fewer than the box holds is shorter, as before.* The body gets two **spacer
rows**: one before the rows the program drew, `row_height * first` rem high, and one after, `row_height * (total - first - drawn)` rem high (`drawn` is how many body rows the program drew; never negative). Both are
`tr`s with `aria-hidden="true"` and one empty cell that spans the columns; the rows drawn are `row_height` high by the rule of section 1. So the scrollable height is `row_height * total`, the drawn rows sit where the scroll
position puts them, and nothing else about the table changes.

* **The table says what it is to assistive technology:** `aria-rowcount` is `total + 1` (the header row counts) and each row drawn has `aria-rowindex` (header 1, row `i` is `i + 2`), which is what the ARIA table
  pattern asks of a table that does not hold all its rows (the *read*: WAI-ARIA Authoring Practices, "Table"). A screen reader then says "row 1,502 of 3,001" for a row it can reach and not "row 1 of 17". *To be
  measured with the accessibility snapshot of Chromium; no screen reader is run.*
* **A redraw keeps the scroll.** The page is rebuilt at every render and a new box starts at the top, so before the build the page records `scrollTop` of each window box (by its place among them) and after it puts them
  back, with the same care the page already takes of the focus (`__cui_focus_save`, `adapters.rs`). Without this the first event would throw the reader to the top. *Choice; the check of section 4 measures it.*
* **The listener** is on the box: it reads `scrollTop`, divides by `row_height` in pixels (the rem of the document's root, read when the event comes), clamps, and calls the event if the index is not the one drawn at this
  render. The index drawn is a constant of the render, so a scroll that the redraw itself causes (putting `scrollTop` back) fires nothing.
* **A table with these bindings is not prerendered** (`recipe.rs`): its rows are the program's expression of `first`, so the page draws it with script, as every table that reads state.

## 3. Refusals

`window-incomplete`, `window-row-height-invalid`, `window-count-not-number`; the existing `event-arity-mismatch` and `event-unknown`, and `binding-unknown` for `row_height` (a literal) on anything but a `Table`. *The first draft said the same of `total:` and `first:`; on any other element they are not read as expressions at all, so the parser stops them first (`parse-unexpected-token`).*

## 4. The checks, written before the code

`crates/cancho-ui/tests/window.rs` (compiler, replay) and `scripts/check-window.js` (Chromium):
* each refusal, and each of the five bindings missing in turn; `first` and `total` as expressions; a trace that fires `Scrolled` with indexes, with `first` past the end and below zero (clamped);
* **in Chromium**, on a 3,000-row table (16 rows drawn, a box of 24 rem at 2 rem a row): the scroll height is `3,000 * 2 rem` to within 1 px; the DOM holds 16 body rows and two spacers, not 3,000; after scrolling to
  row 1,500 the first drawn row is the one at the top of the box (its text is the 1,500th) and its top is within 1 px of the box's top plus the header; the last row of the list is reachable (scrolled to the end, the
  last row is drawn and the box's bottom is the list's); `aria-rowcount` is 3,001 and the first drawn row says `aria-rowindex` 1,502; the page redrawn (a state change that is not a scroll) does not move the box; a
  scroll of less than a row fires no event; the sticky header stays at the top of the box through all of it; the number of events fired by a scroll from the top to the end by 100 steps is not more than the number of
  rows it crossed;
* a **measurement** of what the window buys: the time to build and lay out 3,000 rows against the 16 of the window, in the same Chromium, minimum of 5 runs, written here as a measured line (and nothing said about any
  other library);
* **mutants**: the top spacer too short by a row; the bottom spacer missing; the index rounded up and not down; the event fired for every scroll; the scroll not put back after a redraw; `first` not clamped; the
  `aria-rowindex` off by one; the header counted out of `aria-rowcount`; the five-bindings check removed.

## 5. Not here

A window of anything but a `Table`; rows of different heights; a row that wraps; horizontal windowing of columns; a scroll to a given row from the program (`Scroll to row 1,500` is a state change the program makes
with `first`, and the box does not move until a binding says so: **open**, and the sheets viewer will want it); keeping a selection across a window (the program's: it is a list of indexes); a drag scrollbar that
shows a progress while the rows are being fetched.

## 6. Measured (2026-11-15)

Machine: the development container, Chromium from the Playwright installed there, one run each unless said.

* `cargo test -p cancho-ui --test window` (4 tests): the emitted shape (one scroll listener, the row numbers, two hidden spacers, the box's attribute, the scroll kept) and none of it for a plain table; the trace
  `examples/window/window.trace.json` replays (3 steps); the five bindings together, each missing in turn; row heights 0, 21, -1, a text and a name refused and 1 and 20 accepted; `total:` and `first:` that are a
  text, a list or a comparison refused, and arithmetic of state accepted; the event with no parameter or an unknown one; `row_height:` on a `Button`.
* `node scripts/check-window.js`: 16 checks on a 3,000-row table, 16 rows drawn, a 12 rem box at 2 rem a row. The DOM holds the header, 16 rows and 2 spacers; every body row is **exactly** 2 rem high (the
  stylesheet's `nowrap` and `overflow: hidden` do what section 1 says); the box scrolls the height of 3,000 rows and the header to within 1.5 px; `aria-rowcount` is 3,001 and the header is row 1; at row 1,500 the first
  row drawn is `row-1500`, is the first under the header to within 1 px, and says `aria-rowindex` 1,502; the sticky header is at the top of the box; the scroll survived the redraw; a state change that is not a scroll
  does not move the box; **half a row of scrolling fires nothing**; 99 steps of 30 rows fire 99 events; a `first` of 5,000 is clamped and the box keeps its height; at the end the last row of the list is drawn at the
  bottom of the box.
* **What the window buys, measured:** time from the start of the page to the table in the document and laid out, minimum of 5 loads in a headless Chromium of this container: **the window 49 to 57 ms, all 3,000 rows drawn
  at once 97 to 105 ms** (three runs of the check gave 49/97, 50/105 and 57/104) (the 3,000-row list itself is in the program in both, so what differs is the 3,000 rows against 16). One machine, one browser; no other library or page was measured.
* `node scripts/check-window-mutants.js`: **13/13 killed**: the top spacer a row short, the bottom spacer missing, the index rounded up, the event fired for every scroll, `first` not clamped, the row number off by one, the
  header out of `aria-rowcount`, the spacers visible to assistive technology, the scroll not put back, the row height not applied (the stylesheet), and three refusals removed or renamed.
* **Not measured:** a screen reader with `aria-rowindex`; a table whose rows are fetched while the box is scrolled; a header that is not sticky (the index is `scrollTop / row height`, which assumes the header covers
  the first `header height` of the box, as `sticky: "header"` does).

