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
(`cui-row`, `cui-gap-N`, `cui-field`, `cui-table`, `data-tone`) are styled by the stylesheet that `build --pwa` writes
(`crates/cancho-ui/src/pwa.rs`); a program built without `--pwa` is styled by its host page, as before, and gets the
structure (the `<label>`, the `<table>`) either way.

## 2. `gap:` and `max_width:`

`Row { gap: 3 ... }`, `Column { max_width: 60 ... }`. Integer literals, checked: `gap` 0 to 4 (0, 0.25, 0.5, 1 and 2 rem;
`cui-gap-N`), `max_width` 10 to 200 characters (`style.maxWidth = 'Nch'` and `marginInline = auto`); anything else is
`layout-value-invalid`; on another element, `binding-unknown`. A name is not a literal (`parse-unexpected-token`, as for
`disabled: 3`).

Measured at 1280 px: the page is no wider than `90ch` (908 px) and is centred (186 px from the left), and so is every
input; at 420 px nothing scrolls sideways. The Previous and Next buttons were **0 px** apart (no rule for `.cui-row`) and are
8 px apart.

## 3. `tone:`

`Text { role: status tone: tone "result: {outcome}" }`: a `Text` takes `tone:` before its body, a string (`"ok"`, `"warn"`,
`"error"`) or the name of a `String` state field. The page writes `data-tone` and the stylesheet draws a prefix (a check mark,
a warning sign, a cross) and a colour, so colour is never the only signal. A value that is not one of the three (a state
field is not checked at run time) shows no tone. Refused: a literal that is not a tone, a field that is not a `String` state
field, anything but a literal or a name (`tone-invalid`); `tone:` on anything but a `Text` is `binding-unknown`.

The ops console computes its tone from the status the service answered, which has no `if` to say it
(`docs/conditionals.md`, #40): `join(filter(["ok"], t => starts_with(code, "2")), "") + ...`, one filter per tone.

Measured (Chromium): a 201 shows `data-tone="ok"`, the prefix `✓ ` and `rgb(26, 127, 55)`; a 409 `warn`, `⚠ `,
`rgb(154, 103, 0)`; a 503 `error`, `✗ `, `rgb(207, 34, 46)`. **Contrast** against the page's own background (`Canvas`), WCAG AA
is 4.5:1: light 5.08 (ok), 4.87 (warn), 5.36 (error); dark (the stylesheet has its own colours under
`prefers-color-scheme: dark`) 7.37, 7.42, 7.43.

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

Twelve deliberate breaks, each caught: any tone string accepted, a gap up to 9, a `Table` taking any child, no `<tbody>`, no
`scope`, the tone written as `tone` and not `data-tone`, no `<label>`, a button not wrapped in a cell (cargo, after a test
was added for it, and the browser), no prefix in the stylesheet, no gap in `.cui-row`, a pale warn colour (the contrast
check), a missing `max_width` style.

## 7. Not done

* **Cells wrap at a hyphen on a narrow screen** (`M-` / `106` at 420 px): no `nowrap`, because a long cell would then
  scroll the page sideways, which is worse. A column width belongs to the host page's CSS.
* No column width, alignment (numbers right-aligned), sorting from the header, or sticky header; a header cell is not a button.
* `tone:` on a `Button`, a `For`-made list, or a state field checked at run time; a tone is three words, not a palette.
* An empty `columnheader` text is not refused (the ops console names its button column `select`).
* The page is not restyled when `prefers-color-scheme` changes after load beyond what the media query does (checked by
  emulating each scheme, not by switching live).
* A real screen reader was not run; the accessibility tree is Chromium's.
