# A sortable table header: a button in the header cell and `aria-sort`

> **Status: designed (15 Nov 2026), written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Nothing is built.** Slice F5b of the gaps found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F5); F5a is [`table-static.md`](table-static.md). `presentation.md` section 4 lists it as not built: *a header cell is not a button*. Every claim below is a **choice**, a
> **read** (with the place) or **open**; none is measured yet, and a measurement will replace its line in place.

## 1. The decision

```
component Machines {
    state rows: List[String] = [...];
    state order: String = "";                  // "", "revenue" (ascending) or "-revenue" (descending)

    view {
        Table {
            label: "Machines"
            Row {
                Text { role: columnheader sort: order key: "name"    "name" }
                Text { role: columnheader sort: order key: "revenue" "revenue" }
            }
            For { over: sorted_rows as: r  Row { ... } }
        }
    }
}
```

* **`sort: field` and `key: "k"` on a `Text` with `role: columnheader`.** `field` is the name of a `String` state field (as `Button { copy: field }` names one); `key` is a text literal naming the column. The cell is drawn as
  `<th scope="col" aria-sort=".."><button type="button">label</button></th>`: the **button is the control**, and `aria-sort` is on the **`th`**, which is where the ARIA authoring practice puts it, and only on the
  header of the sorted column (`ascending` or `descending`); the other headers carry no `aria-sort` rather than `none`, which assistive technology reads the same and which keeps the page smaller. *Choice; read from
  the WAI-ARIA authoring practices' sortable-table example, not measured against a screen reader here.*
* **A tap moves the field through three states, and only the field.** `""` or another column's key → `"k"` (ascending) → `"-k"` (descending) → `""` (unsorted) → `"k"`. The program decides what the field *means*
  (`sort(rows, ..)` and `reverse`, or a different query): `docs/lists.md` already gives it `sort`, `reverse` and `exact`. The compiler sorts nothing. *Choice: the sort stays an expression the program writes, so a
  server-side sort (the pg-browser asks the database) and a client-side one use the same header.*
* **The label of the button is the text of the cell**, and its accessible name is that text; the state is carried by `aria-sort` only, not by a glyph in the name. A visual arrow is CSS on `th[aria-sort] button::after`
  (`↑`, `↓`), with `content` outside the accessibility name. *Choice.*
* **The key is a literal and the field a `String`, nothing else.** `sort:` without `key:` (or the reverse) is `sort-key-missing`; `sort:` on anything that is not a header `Text` is `sort-outside-header`; a field that is not
  a `String` state field is `sort-not-string`; a `key:` that is not a literal of letters, digits and `_ - .` (and not starting with `-`, which is the descending prefix) is `sort-key-invalid`. Two headers with the same
  key in one table are `sort-key-duplicate`. *Choice.*

## 2. What the web backend does

A tap on the button sets the field (one event, the next render redraws the headers). The change is a state update like any other, so replay records it: the trace shows the field's value after each tap, and the semantic diff
sees a new binding on the header. The prerenderer writes the same `th`, `button` and `aria-sort` for the initial value, and the button is a hole for the script to hydrate, as every control is (`prerender.md`).
The button gets the existing focus ring and is reached by Tab in document order; Enter and Space activate it because it is a `button`. *Read.*

## 3. The checks, written before the code

`crates/cancho-ui/tests/table_sort.rs` (compiler) and `scripts/check-table-sort.js` (Chromium):
* the five refusals, each with its code; the emitted shape for a header with `sort:`;
* a replay trace: three taps on one header give `"k"`, `"-k"`, `""`; a tap on another header after `"k"` gives that header's key;
* **in Chromium**: `aria-sort` is on exactly the sorted column's `th` and reads `ascending` then `descending` then is absent; the accessibility snapshot has `columnheader "revenue"` containing a `button "revenue"`;
  Tab reaches each header button, Enter and Space both sort; the rows after each tap are in the order the program's own expression gives (compared with a hand-computed order of the fixture, including a tie, so a
  stable sort is seen to be stable);
* **mutants**: `aria-sort` on the button instead of the `th`; the descending state skipped; the unsorted state skipped; `aria-sort` on every header; a tap on another column not resetting to ascending; the label
  carrying the arrow; the three refusals' checks removed.

## 4. Not here

Sorting by several columns; a sort that is stored in the address or in a database; column resizing and reordering; a header that is a button for something other than sort (any `Button` in a header cell already works
and is wrapped, `table.rs`). **Open:** whether `key:` should default to the label's text (it is a literal and a second place to spell the name; written out here so that a translated label does not change the key).
