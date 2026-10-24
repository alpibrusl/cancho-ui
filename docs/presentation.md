# Presentation: a visible label, a table, a tone, a gap and a width

> **Status: built (issue #42, found building `examples/ops-console`, `docs/ops-console.md` 2.9).** Not a styling language: the
> smallest set that fixes what an operator's console showed in its screenshots. Each piece is measured with a screenshot
> check and an accessibility check (`scripts/check-ops-console.js`).

## 1. What was wrong, and what is decided

| seen | decided |
|---|---|
| the two boxes had no visible label (`label:` was an `aria-label` only) | a `TextField`'s `label:` is drawn: the field is wrapped in a `<label class="cui-field">` with the label text above it; the field keeps its `aria-label`, so nothing that found it by that name stops finding it |
| a list row is one `Text`: columns do not align and runs of spaces collapse | `Table`: real `<table>`, so assistive technology reads columns |
| `result: 409 ...` and `result: 201 ...` look the same | `tone: "ok" \| "warn" \| "error"` (or a `String` state field) on a `Text`: a colour **and** a prefix |
| `Row` children touch | the default stylesheet gives `Row` a gap (0.5 rem, like `Column`), and `gap: 0..4` chooses another |
| every control stretches to the width of the window | `max_width: 10..200` (characters) on a `Column` or `Row`: at most that wide and centred |

The program says what a thing *is*; the stylesheet says how it looks. The classes and attributes the page writes
(`cui-row`, `cui-gap-N`, `cui-field`, `cui-table`, `data-tone`) are styled by the stylesheet that `build --pwa` **and `build --page`** write
(`crates/cancho-ui/src/pwa.rs`; it is one inline `<style>` in the `index.html` both write); a program built with neither (`build -o app.js`, which writes
only the script) is styled by its host page, as before, and gets the structure (the `<label>`, the `<table>`) either way.

**Corrected 2026-10-23 (#116, found while measuring what a program can say about its looks).** This paragraph said the stylesheet comes from `--pwa`. It is
the page template, so `--page` (the gallery demos, `docs/mobile-backend.md` section 5.2) has it too: measured, the counter built with `--page` has one
`<style>` with the same baseline, and a build with no flag has no CSS in `app.js` (`<style`, `<link` and `css` do not occur in it).

## 2. `gap:` and `max_width:`

`Row { gap: 3 ... }`, `Column { max_width: 60 ... }`. Integer literals, checked: `gap` 0 to 4 (0, 0.25, 0.5, 1 and 2 rem;
`cui-gap-N`), `max_width` 10 to 200 characters (`style.maxWidth = 'Nch'` and `marginInline = auto`); anything else is
`layout-value-invalid`; on another element, `binding-unknown`. A name is not a literal (`parse-unexpected-token`, as for
`disabled: 3`).

Measured at 1280 px: the page is no wider than `90ch` (908 px) and is centred (186 px from the left), and so is every
input; at 420 px nothing scrolls sideways *(corrected 2026-10-24, #144: true at 420 px and at 412, **not under 412**: the ops console's table, 382 px at its narrowest, made the page scroll 3 px sideways at 380 px, 22 at 360 and 62 at 320; section 4b)*. The Previous and Next buttons were **0 px** apart (no rule for `.cui-row`) and are
8 px apart.

## 3. `tone:`

`Text { role: status tone: tone "result: {outcome}" }`: a `Text` takes `tone:` before its body, a string (`"ok"`, `"warn"`,
`"error"`) or the name of a `String` state field. The page writes `data-tone` and the stylesheet draws a prefix (a check mark,
a warning sign, a cross) and a colour, so colour is never the only signal. A value that is not one of the three (a state
field is not checked at run time) shows no tone. Refused: a literal that is not a tone, a field that is not a `String` state
field, anything but a literal or a name (`tone-invalid`); `tone:` on anything but a `Text` is `binding-unknown`.

The ops console computes its tone from the status the service answered, which has no `if` to say it
(`docs/conditionals.md`, #40): `join(filter(["ok"], t => starts_with(code, "2")), "") + ...`, one filter per tone.

**Corrected 2026-10-23 (#94): a toned text with nothing to say has no tone.** The paragraph above said the page writes
`data-tone` and the stylesheet draws the prefix. It did so even for an empty text: `Text { role: status tone: "error" "{msg}" }`
with `msg = ""` was `<span role="status" data-tone="error">` with the `✗ ` mark drawn (`::before` content `"✗ "`, 24 px tall) beside
no message, so a form that shows a message only when a field is wrong showed a cross beside every valid field. Two repairs were
measured against the same page (`scripts/check-tone.js`; a message that is empty, a single space, filled, cleared, filled again),
each with the accessibility tree read through Chromium's DevTools protocol:

| | empty `""` | `" "` | a message |
|---|---|---|---|
| before (`data-tone` always) | mark drawn, 24 px | mark drawn, 24 px | mark |
| A: CSS `[data-tone]:not(:empty)::before` | no mark, 0 px | **mark drawn, 24 px** (a space is not `:empty`) | mark |
| B: the page drops `data-tone` when the text has no visible characters | no mark, 0 px | no mark, 0 px | mark |

In all three the element is still there with `role="status"` and the accessibility tree has it as a polite, atomic live region
whether it is empty or not, so a message put there by a later render is in a live region as before. **B was built**: after the
text is appended, `if (!el.textContent.trim()) { el.removeAttribute('data-tone'); }` (only for a `Text` with a `tone:`). It needs
no stylesheet (a page built without `--pwa` has none) and it is exact for a text of spaces, which `:empty` is not. What was *not*
established: whether a screen reader announces a message put into a live region that was drawn again in the same render. The
page replaces every status node at every render (before and after this change), which is its own issue (#106). *(Corrected, #106, section 3a: it no longer does.)*
`examples/tickets` uses a constant `tone: "error"` on its messages again: three state fields and their assignments are gone.

Measured (Chromium): a 201 shows `data-tone="ok"`, the prefix `✓ ` and `rgb(26, 127, 55)`; a 409 `warn`, `⚠ `,
`rgb(154, 103, 0)`; a 503 `error`, `✗ `, `rgb(207, 34, 46)`. **Contrast** against the page's own background (`Canvas`), WCAG AA
is 4.5:1: light 5.08 (ok), 4.87 (warn), 5.36 (error); dark (the stylesheet has its own colours under
`prefers-color-scheme: dark`) 7.37, 7.42, 7.43.

## 3a. A status keeps its node from one render to the next (issue #106)

> **Status: BUILT.** Measured 2026-10-24 on main at d90b3a0 (Chromium, `Pixel 7` profile, accessibility tree through the DevTools protocol).

**The problem, measured** (`scripts/check-status.js <older compiler> --report`, and `scripts/measure-status.js`). The page is drawn again from nothing at every render. In a page with seven
live regions (a toned status, three rows of a `For`, one under an `If`, the message of a field, an alert), a render in which only a counter changed made **all seven new DOM nodes
and new accessibility nodes** (ids `43 47 48 49 50 51 52` before, `78 82 83 84 85 86 87` after), so a reference to a status taken before the render was `isConnected === false`
and a `MutationObserver` on it saw nothing. A screen reader announces a change in a live region that already exists; a region that is inserted with its text already in it is announced
inconsistently (the usual advice, **not measured here**).

**Two options, measured** (`scripts/measure-status.js`: four small pages drawn from nothing at every render, as the compiler's pages are; the same four renders: empty, filled, the
same text again, changed):

| | the span is the same DOM node | its accessibility node id | what an observer on it sees: filled / same text again / changed | focus of a text field typed in | a render with 200 statuses (median of 15) |
|---|---|---|---|---|---|
| today: everything is new | no | **changes at every render** (11, 17, 25, 33) | nothing / nothing / nothing (it was removed first) | kept | 3.3 ms |
| **option 1**: the span is built as before and the one this position had is put into the new tree, written only if its text changed | **yes** | **stays** (11, 11, 11, 11) | `childList +1 -0` / **nothing** / `childList +1 -1` | kept | 2.6 ms |
| ideal of option 1: nothing is removed, the text is written in place | yes | stays | the same as option 1 | kept | 0.6 ms |
| option 2: one hidden `role=status` element outside the page, written when a text changes | yes (the announcer) | stays | the same | kept | 2.8 ms |

Option 1 and option 2 both give a live region that is a change when the text is written; the accessibility node survives the **move** of a node into the new tree in the same task (that was the thing to
measure, since it is what the browser reports to a screen reader). **Option 1 was chosen**, for reasons that are not measurements and are said as such:
* the status the program declared with `role: status` stays the node that carries the message, in the place the person sees it; option 2 announces from a different, invisible node, so either the visible one
  stops being a live region (the program's `role: status` is no longer what the tree says) or both announce (twice);
* each status is its own region: option 2 funnels every status of the page through one announcer, which loses which region a message belongs to and orders what are separate announcements;
* it costs nothing that option 2 does not (render time is the same), and no hidden text is duplicated for readers of the page's text.

**What was built.** A `Text { role: status }` or `role: alert`, and the message that `error:` draws beside a field, are built as before and then reconciled with the node the same position had at
the last render (`__cui_status`): the old node takes the new attributes and the new text, **each only if it differs**, and it is the node that goes into the new tree. A position is its place in the view and,
in a `For`, the order of the row. A status that is not drawn in a render (under an `If` that is false, a row that is gone) is forgotten, and a new one starts from the text it has. The field's message keeps its `id`
(the field's `aria-describedby` points at it). Only a module with such a text carries the 1.5 KB adapter: `examples/counter` 7,235 to 8,719 bytes, `examples/ops-console` 48,524 to 50,037, `examples/tickets` 48,919 to 50,833.

**Checked** (`scripts/check-status.js`, Chromium, `Pixel 7`): a render that changed only something else keeps every live region's DOM node and accessibility node id, with role, `live` (polite; assertive for an alert)
and `atomic` as before, and an observer on each sees **no** change (nothing is announced again); a message filled later reaches the observer as `childList +1`, a changed one as `+1 -1` on the same node, an emptied one
leaves the same node in the tree as a live region with no tone; a change of tone is an attribute change of the same node; typing in a text field whose handler writes a status at every key keeps the focus, the caret and
every character, and the status node is written once per key; the field's message is the same node with the same id, `aria-invalid` and `aria-describedby` following its text; a row that goes is gone and the others keep their nodes;
a status under an `If` that hides it comes back as a new node; `role: alert` is kept the same way. `scripts/breaks/status.json`: nine breaks, each caught by the check itself.

**Not measured, and not claimed.** Whether a **real screen reader** announces a message put into one of these regions: no screen reader was available (the same gap as #65 for real devices). What is measured is what such a reader is
given: a live region that exists before the message and receives it as a change. Firefox and Safari; how a reader treats a node that is moved within a task (Chromium keeps the accessibility node; the reader's behaviour is not
measured); a region that **first appears** with its text (the first render of a status that already has a message, a row that is added, a status under an `If` that becomes true) is inserted with its content, which is the case
the advice calls unreliable, and is not improved by this. **Not built:** an assertion of "the text that would be announced, by region" in a replay report (the third question of the issue): replay does not evaluate the text of a
view, so it would be a larger change; the page's behaviour is what is asserted here.

## 4. `Table`

```
Table {
    label: "Machines"
    Row { Text { role: columnheader "id" } Text { role: columnheader "name" } }
    For { over: paged as: m
        Row { Text { json.get(m, "id", "?") } Text { json.get(m, "name", "?") }
              Button { label: "Select " + json.get(m, "id", "?") on: Pick(json.get(m, "id", "")) } }
    }
}
```

The same three elements as everywhere; lowering says what they are in a table (`crates/cancho-ui-ir/src/table.rs`): a `Row`
of a table is a `TableRow` (`<tr>`), a `Text` in it a `Cell` (`<td>`), or with `role: columnheader` a `HeaderCell` (`<th
scope="col">`), and any other element of the row (a button, a field) is wrapped in a `Cell`. The rows go in a `<tbody>`; the
`label:` is the table's accessible name. A `Table` holds `Row`s and `For` loops that make them: anything else is
`table-child-invalid`. A component called `Table` would now be the primitive; none of the examples is.

Measured: `getByRole('table', { name: 'Machines' })` finds it; five `columnheader`s (id, name, state, temperature (C), select);
a `row` for the header and one per machine on the page, five `cell`s each; every cell starts where its column header starts
(< 1 px) on every page of the list; the accessibility snapshot reads `table "Machines"`, `columnheader "id"`, ...

## 4a. Charts

`Chart` is drawn from data with a text alternative: [`charts.md`](charts.md). The stylesheet defines its colour, `--cui-chart`, for both schemes.

## 4b. A table wider than the page scrolls in its own box (issue #144)

**Measured before** (desktop Chromium, `Pixel 7` profile with only the viewport width changed; the page's scroll width minus its client width): the ops
console at 412 px 0, 380 px 3, 375 px 7, 360 px 22, 320 px 62. The table's columns at their minimum content widths add up to 367 px
(`id` 43, `name` 60, `state` 74, `temperature (C)` 111, `select` 79) and a table cannot be narrower than that. `scripts/check-mobile-demos.js` runs at 412
only, so it could not see it. The other 12 demos had no overflow at any of the five widths (`scripts/check-overflow.js`, 13 demos x 320, 360, 375, 380, 412: 61 of 65 passed, the 4
failures all the ops console).

**Two repairs tried, both measured in Chromium at 320 and 360 px:**

| | page overflow | the table |
|---|---|---|
| `overflow-wrap: anywhere` on `th`, `td` | 0 | fits, but breaks words mid-word: `M-106` became `M-` / `106`, `running` became `run` / `nin` / `g`, `name` `na` / `me` (screenshot read) |
| a box with `overflow-x: auto` around the table | 0 | readable; the `select` column is off screen to the right until the box is scrolled (78 px at 320, 38 at 360) |
| `display: block; overflow-x: auto` on the table itself (CSS only) | 0 | Chromium kept `table`, 6 `row`s and 25 `cell`s in its accessibility tree; **not built**, because WebKit has dropped table semantics for a `display: block` table in the past and no WebKit is available here to measure it |

**Decided: the box.** A table is emitted inside `<div class="cui-table-scroll" tabindex="0" role="region" aria-label="…">` (the label is the table's; with no label, no role and no name),
and the baseline stylesheet gives the box `overflow-x: auto; max-width: 100%`. Wide data tables are the case WCAG 1.4.10 (Reflow) allows to scroll in two dimensions; a keyboard
user focuses the region (it has a name and `tabindex="0"`) and scrolls with the arrow keys. A page built without the baseline stylesheet gets a plain `div`. The table keeps its own
role, name and rows (`getByRole('table', { name: 'Machines' })` and 6 `row`s still found).

**Measured after:** 65 of 65 (13 demos x 5 widths) with no horizontal page overflow, at the first look and 1 s later; at 320 px the ops console's box scrolls (`scrollWidth > clientWidth`), is a
region named `Machines`, and the right arrow moved `scrollLeft` from 0 to 40. **Not measured:** a screen reader reading the region (the accessibility tree is Chromium's), Safari and Firefox, a real
phone, and a table with no `label:` (the region then has no role and no name, by construction, not by a test of a screen reader). The first column stays visible only if the box is not scrolled:
no sticky first column.

## 5. Refusals

| tag | when |
|---|---|
| `tone-invalid` | a `tone:` that is not `"ok"`, `"warn"`, `"error"` or a `String` state field |
| `layout-value-invalid` | a `gap:` outside 0 to 4 or a `max_width:` outside 10 to 200 |
| `table-child-invalid` | a `Table` child that is not a `Row` or a `For` of `Row`s |
| `binding-unknown` | `tone:` off a `Text`, `gap:` or `max_width:` off a `Column`/`Row` |

## 6. Checked

`cargo test` (`tests/presentation.rs`, 12 tests): what is accepted and refused, the IR of a table, the page (the `<table>`,
`<tbody>`, `<th scope>`, the cell around a button, `data-tone`, `cui-gap-N`, the width, the `<label>`), a program that uses
none of it emitted without any of it, the stylesheet having a rule for every class, a trace, the diff of a changed tone.
`scripts/check-ops-console.js`: everything above in Chromium, with screenshots (`5-wide.png`, `6-narrow.png`).

`scripts/check-overflow.js` (#144): 13 demos at 320, 360, 375, 380 and 412 px, no horizontal page overflow, and at 320 px the ops console's table box is a focusable, named region that the right arrow scrolls (`scripts/breaks/overflow.json`: 6 breaks, each caught by the check failing on its own; one first written did not compile and was rewritten); `cargo test` `a_table_scrolls_in_its_own_named_focusable_box`.

Twelve deliberate breaks, each caught: any tone string accepted, a gap up to 9, a `Table` taking any child, no `<tbody>`, no
`scope`, the tone written as `tone` and not `data-tone`, no `<label>`, a button not wrapped in a cell (cargo, after a test
was added for it, and the browser), no prefix in the stylesheet, no gap in `.cui-row`, a pale warn colour (the contrast
check), a missing `max_width` style.

## 7. Not done

* **Cells wrap at a hyphen on a narrow screen** (`M-` / `106` at 420 px): no `nowrap`, because a long cell would then
  widen the table; a table wider than the page now scrolls in its own box (section 4b), not the page. A column width belongs to the host page's CSS.
* No column width, alignment (numbers right-aligned), sorting from the header, or sticky header; a header cell is not a button.
* `tone:` on a `Button`, a `For`-made list, or a state field checked at run time; a tone is three words, not a palette.
* An empty `columnheader` text is not refused (the ops console names its button column `select`).
* The page is not restyled when `prefers-color-scheme` changes after load beyond what the media query does (checked by
  emulating each scheme, not by switching live).
* A real screen reader was not run; the accessibility tree is Chromium's.
