# The operator console: one page that uses everything

> **Status: built and measured (2026-10-21).** Until now every feature of the last weeks had its own test page or none:
> `net.post`, `sort`, `take`/`skip`, `lines`, `num`, `start`/`every`, the hidden-page pause, abort on supersede. Nobody had
> written one realistic page that needs them together. `examples/ops-console/ops-console.cui` is that page, and this
> document is what writing it showed, in the order of how much it hurt.

## 1. The page

A machine list for a plant-floor operator, 23 machines in the stand-in service.

| Feature | How the page does it |
|---|---|
| Read | `net.get("api/machines")`: a text with **one JSON record per line**, taken apart by `lines`, kept only if `json.get(line, "id")` is not empty |
| Search | a `TextField`; `contains(lower(name), lower(q))` inside `filter` |
| Order | `sort(.., x => dir * num(json.get(x, "temp", ""), dir * 1000000000))`; the button flips `dir` between 1 and -1 |
| Paging | `take(skip(shown, page * 5), 5)`; Next and Previous never leave the list |
| Refresh | `start Refresh; every 5000 Refresh;` (paused while the page is hidden, one catch-up when it is shown) |
| Write | two fields and **Apply**: `net.post("api/setpoint", "[\"M-101\", 80]")`; the answer's status and body are shown (201, 409, 503, 400, or `not sent: network error`) |
| Supersede | a refresh still waiting when the next one starts is aborted: the service sees the first connection close |

It asks for exactly two capabilities, `Network` and `NetworkWrite`; either alone is `capability-missing`.

Checked three ways: `cargo test` (`tests/ops_console.rs`: the program checks, its trace passes and a wrong expectation
fails, neither capability is spare, the built page carries every helper it calls), a replay trace with 38 steps and 57
checks (`examples/ops-console/ops-console.trace.json`), and **the built page in Chromium** against a stand-in service
(`scripts/check-ops-console.js`, `scripts/ops-console-server.js`: 52 checks, six screenshots).

## 2. What it showed, ranked

### 2.1 A call to a function that does not exist killed the whole page, with no diagnostic. **Fixed here.**

The first draft needed `max` and `floor` (a page count). `cancho-ui check` answered **exit 0**; `build` wrote
`__state_n = /* unsupported call max.(__state_n, 1) */;`, which is a JavaScript **syntax error**: the module does not
load, the page is blank, and nothing in the toolchain said why. The same for a misspelt builtin (`contians`) or any made-up
name. Now `call-unknown` (`spec/diagnostics/README.md`) refuses it at check time, with a span and a remediation that lists
every function the language has (`tests/call_unknown.rs`: ten refused spellings, the remediation, nothing written by
`build`, every known call still accepted, every example still checks). Found while building; the only defect fixed in this
change because it is small and the rest are design decisions.

### 2.2 No conditional anywhere, and the workarounds are not readable. Issue #40

There is no `if`, no `&&`, `||` or `!`, no literal patterns (`pattern-literal-unsupported`, #26), and `match` is only over
`Result` and `Option`. Everything the page branches on is arithmetic on lists:

* **Next without running off the end:** was `page = page + len(take(skip(shown, (page + 1) * 5), 1))`; since #43 it is
  `page = min(page + 1, pages - 1)`.
* **Previous without going below 0:** was `page = page - len(take(["x"], page))`; since #43, `page = max(page - 1, 0)`.
* **Clamp after the list shrinks:** `page = page * len(take(skip(shown, page * 5), 1))` (0 if the page is empty).
* **"Is this text a number?"**: `num(setpoint, 1) == num(setpoint, 2)` (the two defaults differ, so they only agree when the
  text parsed).
* **Which message for a status:** the page shows `status + " " + body` instead of choosing text for 409.

These work, are tested, and nobody would find them without this document. The same handler body (filter, sort, page) is
**written out four times** (Refresh, Query, Flip, and again) because a handler cannot call another and there is no derived
state or local `let`.

### 2.3 A page cannot refuse to send, or stop a second send. Issue #35. **Fixed 2026-10-21 (`docs/controls.md`).**

> *Correction:* `Button` and `TextField` take `disabled:` (`true`, `false` or a `Bool` state field) and an unknown key is
> `binding-unknown`. The page now has `state sending: Bool` set around the POST and `Apply` is `disabled: sending`; a double
> click sends **one** write (`scripts/check-ops-console.js`, 52 checks and the trace's 37th step). What follows is the finding as it was made.

`Button { disabled: true }` **compiles and is ignored**: no attribute is emitted (an unknown binding key is accepted
silently). A double click on Apply sends **two** POSTs (measured: 2 in 400 ms); each is a real write. There is no way to say
"not while one is in flight" nor "not if the setpoint is not a number": the page sends `["", 0]` for a bad setpoint and
lets the service refuse it with 400.

### 2.4 Enter does not submit. Issue #36. **Fixed 2026-10-21 (`docs/controls.md` section 3).**

> *Correction:* `TextField { ... submit: Apply }` fires `Apply` on Enter. The setpoint field of the page has it, and Enter sends the
> write **once** (the guard of the disabled `Apply` button keeps a second Enter out while the first is in flight). What follows is the finding as it was made.

`TextField { on: Apply enter: Apply }` is `parse-unexpected-token`. An operator who types a setpoint and presses Enter does
nothing (measured: no request). `AGENTS.md` section 7 names `accessibility-keyboard-missing` as a contract; it is not
implemented.

### 2.5 JSON bodies cannot be written or built safely. Issues #38, #39. **#38 fixed 2026-10-21 (`docs/language-design.md` decision 15).**

> *Correction:* `\{` and `\}` write a literal brace in a string, what is left in an interpolation after its expression is
> `parse-unexpected-token` (it was dropped), and `json.quote(text)` is the JSON string literal of a text. The page itself
> still sends the array its stand-in service expects (the id comes from the rows, so typed text never reaches the body); a service
> that wants an object can now be spoken to safely (`crates/cancho-ui/tests/braces.rs`, `scripts/check-lists.js`). `l + l` on two lists (#39) is not fixed. What follows is the
> finding as it was made.

* A string with a brace is **silently wrong**: `"{\"a\":1}"` compiles to `"" + "a"`; the interpolation `{\"a\":1}` is read
  as the expression `"a"` and the rest dropped. `\{` is `lex-unknown-escape`. So an object body cannot be written; the page
  sends a JSON **array**, `["M-101", 80]`.
* There is no `json.quote`. Building a body from text the operator typed lets the text change the payload: measured on the
  first draft, an id of `M-102", 99, "x` posted `["M-102", 99, "x", 55]` and the service applied setpoint 99. The page now
  takes the id from the **rows** it holds (`join(map(filter(rows, id == target), id))`), so typed text never reaches the
  body; an id nobody has becomes `""`.
* `l + l` on two lists **type-checks** and is emitted as JavaScript `+`, a string concatenation (`"a" + "a"`). There is no
  list concatenation either.

### 2.6 `lower` and `upper` are ASCII only; a European plant cannot search. Issue #37

Measured with `Pumpe Ünit` and `pumpe ünit`: searching `ünit` finds only the lower-case record, and `ÜNIT` only
the upper-case one. This is the deliberate stance of `docs/lists.md` section 1 (a page and its replay must agree), and it
is wrong for names with accents, which is most of Europe. It needs a decided policy, not a workaround.

### 2.7 No per-row action, no dynamic label. Issue #41

`Button { label: m on: Pick(m) }` is `parse-unexpected-token`: a button's label must be a **string literal**, and `on:` takes
an event name with no argument. So a row cannot be clicked to select the machine; the operator types the id. The click
would be the natural interface.

**Fixed** (`docs/row-actions.md`): `label:` takes an expression and `on: Pick(m)` passes arguments, evaluated at render time.
Every row on the page now has a `Select M-xxx` button that fills the machine id (`scripts/check-ops-console.js`; the trace
has a `Pick` step, 38 steps). Measured with 100 and 1000 rows: no event delegation needed. Found on the way: a bare `on:`
on a button used to hand the DOM event to a handler with parameters, and an unknown event compiled and killed the page;
both are refused now.

### 2.8 No numeric builtins. Issue #43

No `min`, `max`, `floor`, `ceil`, `abs`: a page count needs `ceil`, so the page showed "page 4" and not "page 5 of 5".
(Since 2.1 a call to one was refused with a message that said so.)

**Fixed** (`docs/lists.md` section 8): `min`, `max`, `abs`, `floor`, `ceil` and `round`. The page now says
`23 of 23 machines, page 1 of 5`, from `pages = max(ceil(len(shown) / 5), 1)`; Next and Previous clamp with `min` and
`max`. The refresh that lands beyond the new last page still returns to page 1 (that line is unchanged: clamping to the
last page would be a behaviour change this issue does not make). Four deliberate breaks of the page (Next not clamped,
`floor` for `ceil`, Previous not clamped, the page count without `max(.., 1)`): the browser script fails on all four
(2, 13, 1 and 1 checks); the 38-step trace fails on the first three (3, 4 and 1 failures) and **passes the fourth**,
because no step ends on an empty list, so only the browser script covers an empty list's "page 1 of 1".

### 2.9 Layout and presentation. Issue #42

Looked at in the screenshots (`5-wide.png`, 1280 px):

* every control stretches to the full width of the window; there is no way to say otherwise;
* the rows are one `Text` each, so **columns do not align**, and runs of spaces collapse in HTML (`id  name  state` shows
  as `id name state`): `M-106 Lathe 3 running -4.5` cannot be read as columns;
* an error looks like a success: `result: 409 locked by maintenance` and `result: 201 applied` are the same text weight and
  colour (a `role: status` text has no visual state);
* the Previous and Next buttons touch (a `Row` has no gap);
* the two text boxes had **no visible label** on the first draft: `label:` is an `aria-label` only. The page now sets
  `placeholder:` as well, which is visible; it vanishes once the operator types.

**Fixed** (`docs/presentation.md`): a visible `<label>` for every field, the machines in a real `<table>` (five column
headers, cells aligned to them, a `Select` button per row), a status text with a `tone` (a prefix and a colour: 201 `ok`, 409
`warn`, 503 `error`, contrast 4.87:1 to 7.43:1), a gap between Previous and Next (0 px to 8 px), and a page at most `90ch`
wide and centred at 1280 px. The browser script checks each one and takes `5-wide.png` and `6-narrow.png`. Still open: cells
wrap at a hyphen on a narrow screen, no column widths or alignment.

### 2.10 Offline, the list is empty. Issue #44. Fixed in the program, not in the service worker

With the service stopped, the installed page opened (the service worker precaches the page files) and said
`status: error: network error` and `0 of 0 machines`: the service worker does not cache API answers, and the program held the
list only in memory. The console now keeps the last good answer in `Storage("ops-console")` and shows it when the read fails,
with a status that says so (`error: network error (showing the last saved list, if any)`). Why the program and not the worker:
`docs/adapters.md` section 19. (A test note: Playwright's `setOffline` does not reach a service worker's own requests, so the
checks stop the server.)

### 2.11 Replay and the browser disagree about NaN. Not an issue: a documented refusal.

The first draft sorted a missing temperature last with `num(text, 0 / 0)`: `NaN` sorts last in both directions in the
browser (`docs/lists.md` section 1), but replay refuses `0 / 0` as `replay-non-finite`, so the page could not be
replayed. The page uses a finite sentinel (`dir * 1000000000`, added after the direction is applied so it is the same in
both directions) and the trace and the browser now agree, including a record with no temperature last in both directions.

### 2.12 The human replay report prints every list in full. Issue #45

A state that holds a list of JSON lines made one step of the report a screenful. The JSON report is fine; the human
rendering has no limit on a value.

**Fixed** (`docs/replay.md` section 7): the human report cuts a value on a step line to 120 characters and says its
length; `--full` prints it whole; a failure and the JSON report are never cut. On this page's 38-step trace the report
went from 15,771 to 8,417 bytes and the longest line from 3,666 to 984 characters.

## 3. What worked the first time

* The whole page, all eight features, **checked on the first try** and the first browser run passed. The builtins compose.
* Markup in a name (`<img src=x onerror=...>`) is shown as text and runs nothing (checked).
* A truncated line, a line that is not JSON and a blank line are dropped by one `filter` (checked: 23 + 6 records).
* CRLF line endings, a slow request superseded by a refresh (the service sees the connection close), a hidden page
  (no polls, one catch-up refresh), typing across a refresh that lands in the middle (text and focus survive), a
  409, a 503, a 400, the service down: all as designed.
* The 38-step trace passes and, with each of seven deliberate breaks of the program (sentinel, page clamps, line filter,
  id and validity filters, case-insensitive search, direction), **fails**; the browser script fails on each too.

## 4. Checks, and how they were made to fail

Seven deliberate breaks of the page, each run against the browser script, the trace and the cargo tests: the
missing-temperature sentinel (browser 3, trace 7), no clamp after a refresh (browser 3, **trace 2 after the shrinking step
was added**; the first trace did not cover it), broken lines kept (1, 6), the id and validity filters removed (5, 7), the
search made case-sensitive (2, 2), Next not clamped (2, 4), Flip doing nothing (2, 6). And `is_known_call` made always true:
the `call-unknown` tests fail (10).

Two checks assert a gap **as it is today** ("KNOWN GAP": Unicode search; the double submit and Enter were fixed, #35 and #36, and the offline list, #44), so
that fixing one fails the check and forces this document to be updated.

## 5. Not done

A longer run (the page was polled for seconds, not hours); more than 23 machines (the 10,000-line bound of `lines` was
checked in `docs/lists.md`, not here); a touch device; a screen reader (the markup has roles and labels; nobody listened);
iOS Safari; the credential for a service that wants a bearer token (#17).
