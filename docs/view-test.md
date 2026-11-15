# What the page shows, checked in a trace: the outline of the view

> **Status: built (15 Nov 2026), measured in section 6; designed before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1).** Slice F9 of the gaps found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F9): `cancho-ui test` replays handlers and reports state, so *"the grid shows what the state says"* has no check written in Cancho; it is checked by `inspect` and by eye.
> `replay.md` section 9 lists `view_rendering` among the things a passing replay does **not** claim, and every report prints it (`NOT_CLAIMED`, `crates/cancho-ui-replay/src/report.rs`). Every claim below is a
> **choice**, a **read** (with the place) or **open**; section 6 holds what was measured, and where it changed a line of the design the line is corrected in place.

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

and `cancho-ui inspect view <file> [<component>] [--state '{"first": 1500}']` prints the outline of the component for its initial state, or for the state fields it is given (the others keep their initial value). `cancho-ui test <file> <trace> --view` puts the outline in the report of **every** step, not only of those that assert one (`steps[i].view` in `--output json`), which is how the browser check of section 3 reads it.

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
  | `Heading` / `Text { role: heading }` | `heading "…" level=N` (a `Text` with `role: heading` is level 2, which is what the page's `role="heading"` without `aria-level` means) |
  | a `Column` or `Row` given a `role:` (`role: banner label: ".."`) | `banner ".."`, and its children one level in: a box with a role is a node, not a layout box *(found by the browser check: the first outline left it out)* |
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

`view-bad-line` (a line that is not a role, an optional quoted name and the properties of the outline, indented two spaces for each level: a **refusal of the trace** before it runs, as `trace-bad-step` is), `view-mismatch` (the outline differs; the report carries both), `view-unsupported` (an asserted view with a `? Kind`) and `view-expression-unsupported` (a view expression the replay cannot evaluate, or cannot say exactly: a request, or a number the page writes differently from Rust, such as `0.0000001`): **failures of the step**, as a wrong `state` is, and the trace goes on. A component that `requires Storage`, `Router` or `Credential` is refused whole (`view-expression-unsupported`): its view may read what the browser holds, which a trace does not give the outline, so an outline of it would say what the page shows for an empty browser, not for the one it is in. The fake clock and the language in use are what the replay already has, and a view that reads them is outlined.

## 3. The oracle: the browser's own accessibility tree

A claim that a Rust evaluator of the view agrees with the page that is drawn needs the page. `scripts/check-view.js` (Chromium, outside the gate as the other browser checks are) takes a list of examples and, for
each, a short list of **interactions**: a real click, a real typing, a real key or scroll, paired with the trace step that is the same event. After each interaction it reads the outline out of the DOM with a function
that applies the table above to the document (`table` -> `table`, `tr` -> `row`, `th scope=col` -> `columnheader`, `td` -> `cell`, `button`, `input` -> `textbox` ..., with `aria-sort`, `aria-rowcount`, `pressed`,
`disabled` and `checked` read from the attributes), and compares it with the outline `cancho-ui test` gives for the same trace prefix. A difference is a bug in the evaluator or in the emitter, and the check says which line.
*The first draft said `ariaSnapshot` of the same page would be read too, as a second witness that the vocabulary is the browser's; **that is not built**: the DOM function applies the table of section 1 and nothing else, so the check says the outline equals the table applied to the DOM, which is a weaker claim than "equals the accessibility tree". Open.* *Choice: the examples of this repository are the corpus; the claim is for those pages and those interactions, and section 6 lists them and the ones skipped.*

## 4. The checks, written before the code

`crates/cancho-ui/tests/view_test.rs` (compiler, replay) and `scripts/check-view.js` (Chromium):
* the outline of each example in its initial state, written by hand in the test (so the evaluator is not its own oracle), and after the steps of its trace;
* `view` and `view_has` passing, and failing with the lines that differ; a bad line; an `? Kind`; an expression that needs a request; a state field the step did not name keeping its value;
* a trace of the sortable header (`table-sort`) with `aria-sort` through its three states, of the window (`rows=3001`, the rows drawn at `first`), of the keys example (no line for a `Key`);
* **in Chromium**, the interactions of section 3 for five examples and the state at rest of every other example the outline can say, every line compared (the names compared with `ariaSnapshot`: not built, section 3);
* **mutants**: an `If` that ignores its condition; a `For` that draws one element too few or too many; a name that is the text and not the label; `sort=` missing; `numeric` missing; `pressed` for `disabled`; `view_has` that
  ignores the order; `view` that ignores one line; `? Kind` that is skipped instead of refused; the `rows=` of a window computed without the header.

## 5. Not here

An outline of a child component instance (the replay does not create them: `replay.md` section 9); an outline of the page the prerenderer writes (it is the same elements; **open**: whether to check it against
the outline too); a `view` for the **initial** state in a trace header (the first step's report is enough); a diff of two outlines over time (`semantic-diff.md` is the diff of programs); focus (which node holds it);
a screenshot or a visual comparison of any kind.

## 6. Measured (2026-11-15)

Machine: the development container, Chromium from the Playwright installed there, one run each.

* `cargo test -p cancho-ui --test view_test` (8 tests), outlines written by hand: the counter, the keys example, the sortable table in a descending state (the rows in the order the program sorts them, `sort=descending` on one
  header and `numeric` on the cells), a window (`rows=61`, 16 rows drawn from `first`, the last rows near the end), a false and a true `If`, a `For` over three elements and over one, a button pressed and disabled and a
  box checked, `view` and `view_has` passing, and five ways of being wrong (a wrong count, the order, a missing line, only the last line wrong, a line too many) each failing as `view-mismatch` with both outlines in the
  report; `? Rich` refused as `view-unsupported`; `0.0000001` refused as `view-expression-unsupported`; nine bad lines refused as `view-bad-line` and six good ones not; a component that `requires Storage` refused whole.
* `node scripts/check-view.js`: **33 outlines compared** with the DOM, every line: 5 examples driven by real interactions (counter: 3 clicks; rows: 3 clicks with arguments; table-sort: 5 clicks on the headers through three
  states and two columns; keys: typing in a field, ArrowDown, Shift+ArrowDown, a click; window: two scrolls) and **16 examples at rest** (the five above, table-static, and `await`, `clipboard`, `dashboard/cards`, `exact`,
  `hello`, `list`, `match`, `near`, `theme`, `where`). **29 examples are skipped, each with its reason printed**: 19 have a node the outline cannot say (charts, maps, images, icons, islands, the page head, a waffle ..),
  4 are components the outline refuses whole because they `require` `Storage`, `Router` or `Credential`, and 6 do something before the first event or read the browser (`start`, `every`, `subscribe`, a `Locale`, a `Network`). This is the claim: for those 16 pages and those interactions the
  outline is what the DOM says; it is not a claim about the other 29.
* **The check found two things the first outline got wrong**, both now fixed and in the table of section 1: a `Text { role: heading }` is level 2 in the page and the outline wrote it as `text`; and a `Column` given
  `role: banner` is a node of the accessibility tree and the outline left it out.
* `node scripts/check-view-mutants.js`: **15/15 killed** in one run: an `If` that ignores its condition; a `For` one element short and one that draws each twice; a button with no name; `sort=`, `numeric` and the value of a
  text field missing; `pressed` written as `disabled`; the rows of a window without the header; a role of a box dropped; `view_has` out of order; `view` ignoring its last line; a `? Kind` skipped; an odd indentation and an
  unknown word accepted. **Three earlier runs were wrong and are kept here:** the first run left `an If ignores its condition` alive (no test had a false `If`), `pressed says disabled` alive (no test had a pressed
  button) and `view ignores the last line` alive (no wrong trace differed only in its last line); each got a test, and the whole set was run again. A run that is interrupted leaves its mutant in the file: twice that
  was found by `git diff` before a commit and reverted.
* **Not measured:** the names against `ariaSnapshot`; an outline of a component instance, a chart, a map or an image; a `Router`, `Storage` or `Credential` page.

