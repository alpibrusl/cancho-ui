# What the page shows, checked in a trace: the outline of the view

> **Status: designed (15 Nov 2026), written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Nothing is built.** Slice F9 of the gaps found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F9): `cancho-ui test` replays handlers and reports state, so *"the grid shows what the state says"* has no check written in Cancho; it is checked by `inspect` and by eye.
> `replay.md` section 9 lists `view_rendering` among the things a passing replay does **not** claim, and every report prints it (`NOT_CLAIMED`, `crates/cancho-ui-replay/src/report.rs`). Every claim below is a
> **choice**, a **read** (with the place) or **open**; none is measured yet, and a measurement will replace its line in place.

## 1. The decision

A step of a trace may say what the page shows after it, as an **outline**: one line for each node the person or the assistive technology meets, written the way the accessibility tree names them.

```json
{ "event": "Scrolled", "params": [1500],
  "state": { "first": 1500 },
  "view": [
    "table \"Machines\" rows=3001",
    "  row",
    "    columnheader \"name\" sort=ascending",
    "      button \"name\"",
    "  row",
    "    cell \"row-1500\"",
    "    cell \"8\" numeric" ] }
```

and `cancho-ui inspect view <file> [<component>] [--state '{"first": 1500}']` prints the outline of the component for its initial state, or for the state fields it is given (the others keep their initial value).

* **`view` is exact; `view_has` is a list of lines that must all appear, in that order, with anything between them.** A page that is long, or that holds a part the program does not care about, says `view_has`. A line is
  compared as written, indentation included for `view`, and with the indentation ignored for `view_has` (it names a node, not its depth). A difference is reported as the lines that differ, with the step's number.
  *Choice: two forms because exact is the strong check and a 3,000-row table has no exact outline worth writing.*
* **The outline is a pure function of the state after the step.** It is what `cancho-ui-replay` already does for `charts`, `sortlists` and `maps` (`sortlist.rs`: the page is walked with the state, an `If` that is false is
  not in it), extended to every element the list below names, with `For` expanded once for each element of its list (the loop variable is a local of the evaluation, as in a handler). No clock, no request, no
  storage: a view expression that needs one is `view-unsupported` (below), as it is a refusal in the emitter.
* **The lines** are `role`, then the name in quotes when the node has one, then the properties that are true of it, in a fixed order. The roles and what they stand for:

  | Source | Line |
  |---|---|
  | `Column`, `Row` (not in a table) | not a line: a layout box has no role; its children are the lines, one level in |
  | `Text` | `text "the text"` (a `Text` whose role is `status` or `alert` says it: `status "…"`) |
  | `Heading` / `Text { role: heading }` | `heading "…" level=N` |
  | `Button` | `button "label"`, then `pressed`, `disabled` when they are true |
  | `TextField`, `TextArea` | `textbox "label" value="…"`, `disabled`, `invalid` |
  | `Checkbox` | `checkbox "label"`, `checked` |
  | `Select` | `combobox "label" value="…"` |
  | `Table`, `Row`, a header cell, a cell | `table "name"` (with `rows=N` for a window: `aria-rowcount`), `row`, `columnheader "…"` (with `sort=ascending` or `descending` when it is sorted), `cell "…"` (with `numeric`) |
  | `List`, `Item` | `list`, `listitem` |
  | `Link`, `NavLink` | `link "…" to="…"` |
  | `Chart`, `Map`, `Image`, the rest | the line the replay already reports for it (`charts`, `maps`), or `image "alt"`, and for anything else `? Kind` (see below) |

  A `Key` draws nothing and has no line. Text that is not one node of its own (a `Rich`) is one `text` line. *Choice: the vocabulary of the accessibility tree, because that is what the page is to a reader who does not
  see it, and because the browser can be asked for the same words (section 3).*
* **`? Kind` is a node the outline cannot say.** A step that asserts a `view` containing one is `view-unsupported`, naming the kind; an outline printed by `inspect view` shows it so that the gap is seen. The list grows by
  adding a row to the table above with its check, never by guessing. *Choice: a view check that silently skips what it cannot read would say "the page is right" about a page it did not read.*
* **What a line is not.** It is not the DOM, not the layout, not a pixel, not a colour or a size, not the order of focus, and not the page of a browser that differs from Chromium's accessibility tree in a name. `NOT_CLAIMED`
  keeps `view_rendering`, now worded `view_rendering (beyond the outline a step asserts)`, and a report says which steps asserted one.

## 2. Refusals

`view-bad-line` (a line that is not `role`, an optional quoted name and known properties), `view-mismatch` (the outline differs; the report carries both), `view-unsupported` (an asserted view with a `? Kind`),
`view-expression-unsupported` (a view expression the replay cannot evaluate: a request, the clock, storage). The first two are failures of a step as a wrong `state` is; the last two are refusals of the trace.

## 3. The oracle: the browser's own accessibility tree

A claim that a Rust evaluator of the view agrees with the page that is drawn needs the page. `scripts/check-view.js` (Chromium, outside the gate as the other browser checks are) takes a list of examples and, for
each, a short list of **interactions**: a real click, a real typing, a real key or scroll, paired with the trace step that is the same event. After each interaction it reads the outline out of the DOM with a function
that applies the table above to the document (`table` -> `table`, `tr` -> `row`, `th scope=col` -> `columnheader`, `td` -> `cell`, `button`, `input` -> `textbox` ..., with `aria-sort`, `aria-rowcount`, `pressed`,
`disabled` and `checked` read from the attributes), and compares it with the outline `cancho-ui test` gives for the same trace prefix. A difference is a bug in the evaluator or in the emitter, and the check says which line.
`ariaSnapshot` of the same page is read too, and the names of the lines must be the names it gives (a second witness that the vocabulary is the browser's, not ours). *Choice: the examples of this repository
are the corpus (`counter`, `rows`, `keys`, `table-sort`, `table-static`, `window`, `clipboard`, `exact`, `ops-console` .. as the table covers them); the claim is for those pages and those interactions, and says so.*

## 4. The checks, written before the code

`crates/cancho-ui/tests/view_test.rs` (compiler, replay) and `scripts/check-view.js` (Chromium):
* the outline of each example in its initial state, written by hand in the test (so the evaluator is not its own oracle), and after the steps of its trace;
* `view` and `view_has` passing, and failing with the lines that differ; a bad line; an `? Kind`; an expression that needs a request; a state field the step did not name keeping its value;
* a trace of the sortable header (`table-sort`) with `aria-sort` through its three states, of the window (`rows=3001`, the rows drawn at `first`), of the keys example (no line for a `Key`);
* **in Chromium**, the interactions of section 3 for at least eight examples, every line compared, and the names compared with `ariaSnapshot`;
* **mutants**: an `If` that ignores its condition; a `For` that draws one element too few or too many; a name that is the text and not the label; `sort=` missing; `numeric` missing; `pressed` for `disabled`; `view_has` that
  ignores the order; `view` that ignores one line; `? Kind` that is skipped instead of refused; the `rows=` of a window computed without the header.

## 5. Not here

An outline of a child component instance (the replay does not create them: `replay.md` section 9); an outline of the page the prerenderer writes (it is the same elements; **open**: whether to check it against
the outline too); a `view` for the **initial** state in a trace header (the first step's report is enough); a diff of two outlines over time (`semantic-diff.md` is the diff of programs); focus (which node holds it);
a screenshot or a visual comparison of any kind.
