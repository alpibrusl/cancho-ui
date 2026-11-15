# Tables that stay readable: numeric cells, a sticky header, a frozen first column

> **Status: designed (15 Nov 2026), written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Nothing is built.** Slice F5a of the gaps found while specifying a table browser in
> cancho-sheets (`docs/design.md` section 23, finding F5), which the table of `presentation.md` section 4 listed as not built: *no column width, alignment (numbers right-aligned), sorting from the header,
> or sticky header; a header cell is not a button* and *no sticky first column* (section 4b). This slice is the three that need **no state**, so the page the program draws and the page the prerenderer writes
> (`prerender.md`) agree by construction. **F5b**, a header that is a button with `aria-sort`, depends on state and has its own design later. Every claim below is a **choice**, a **read** (with the place) or
> **open**; none is measured yet, and a measurement will replace its line in place.

## 1. The decision

```
Table {
    label: "Machines"
    sticky: "both"            // "header", "column" or "both"
    max_height: 24            // rem: the box scrolls down inside this height
    Row { Text { role: columnheader "name" } Text { role: columnheader numeric: true "revenue" } }
    For { over: rows as: r
        Row { Text { json.get(r, "name", "") } Text { numeric: true json.get(r, "revenue", "") } }
    }
}
```

* **`numeric: true` on a `Text` that is a cell or a header cell of a table** aligns it to the end of the cell and gives its digits equal width: the class `cui-num`, which the baseline stylesheet defines as `text-align: end;
  font-variant-numeric: tabular-nums`. It does not parse the text or change it (a number is shown as written, as `fmt` is the way to change how it is shown). `numeric:` is `true` or `false`, anything else is
  `numeric-invalid`; on a `Text` that is not in a table it is `numeric-outside-table`; on any element that is not a `Text` it is the existing `binding-unknown`. *Choice.*
* **`sticky:` on a `Table` is `"header"`, `"column"` or `"both"`**, a text literal. `"header"` keeps the first row (the row of `columnheader`s) at the top of the box while the rows scroll; `"column"` keeps the first cell of every
  row at the left while the table scrolls sideways; `"both"` is the two, and the corner cell stays above both. Any other value is `table-sticky-invalid`. *Choice: one word, because the two together are the common case and a
  list of positions would be a small language of its own.* Only the first column: a second frozen column would need the width of the first, which the page does not know.
* **`max_height:` on a `Table` is an integer literal from 4 to 100, in rem**, as a `Code`'s is (`structure.md` section 8): the scroll box gets that height and scrolls down inside it, which is what makes a sticky header mean
  something. `sticky: "header"` or `"both"` **without** `max_height:` is `table-sticky-needs-height`: the box would never scroll down, so the header could never stick. `max_height:` alone is allowed (a short box). *Choice.*
* **Nothing about the accessibility tree changes.** The table is still `table`, `row`, `columnheader`, `cell` (the region around it already has its name, `presentation.md` section 4b); the scroll box keeps its `tabindex="0"`
  so a keyboard user scrolls it. *Read; to be measured.*

## 2. What the web backend does

The box (`div.cui-table-scroll`) gets the classes `cui-sticky-head` and/or `cui-sticky-col` and, with `max_height:`, `style="max-height: Nrem"`; a cell with `numeric: true` gets `cui-num`. The baseline stylesheet defines them:

```
.cui-table-scroll.cui-has-height { overflow-y: auto; }
.cui-sticky-head th { position: sticky; top: 0; background: Canvas; z-index: 1; }
.cui-sticky-col th:first-child, .cui-sticky-col td:first-child { position: sticky; left: 0; background: Canvas; z-index: 1; }
.cui-sticky-head.cui-sticky-col th:first-child { z-index: 2; }
.cui-num { text-align: end; font-variant-numeric: tabular-nums; }
```

The same classes and the same style attribute are written by the prerenderer from the same recipe functions (`crates/cancho-ui-ir/src/recipe.rs`), so a prerendered page and the page the script draws are one DOM (the check of `prerender.md`). A page built
without the baseline stylesheet gets the classes and no look, as for every other class here. *Choice; open: `Canvas` as the fill follows the page's colour scheme, and a page with its own background needs its own rule.*

## 3. Refusals (new)

`numeric-invalid`, `numeric-outside-table`, `table-sticky-invalid`, `table-sticky-needs-height`, `table-max-height-invalid`. Existing: `binding-unknown`.

## 4. The checks, written before the code

`crates/cancho-ui/tests/table_static.rs` (the compiler) and `scripts/check-table-static.js` (Chromium, outside the Rust gate as the other browser checks are):
* the classes and the style are emitted for a table that asks for them and not for one that does not; the prerendered DOM equals the script's (the prerender test of `prerender.md` is extended to this table);
* the five refusals, each with its code, and `binding-unknown` for `numeric:` on a `Button`;
* **in Chromium**: with `sticky: "header"` and rows taller than the box, after scrolling the box down by 200 px the header cells are still at the top of the box (their top equals the box's top, within 1 px) and the first
  body row has moved; with `sticky: "column"` and a table wider than the box, after scrolling right by 200 px the first cell of every row is still at the box's left edge and the second has moved; `"both"` does both and
  the corner cell is above the body cells (`z-index` order, read back from the computed style and from `elementFromPoint`); a `numeric` cell's text ends at the cell's right edge (the range's right equals the cell's content-box
  right, within 1 px) and a cell that is not numeric starts at the left; the accessibility tree still reads `table`, `columnheader` and `cell` with the same names and counts as without the bindings;
* **mutants** of the compiler and the stylesheet: the header class not emitted, the column class not emitted, the height ignored, the sticky offset `top: 1px`, the corner not raised, `numeric` not aligned, the check for
  `sticky` without a height removed.

## 5. Not here

A header that is a button, with `aria-sort` (F5b); more than the first column frozen; column widths; sticky rows at the bottom (a total row); a frozen column in a table that is not scrolled sideways (it is a no-op, not an error).
