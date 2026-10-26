# The port of the cancho-www home page, step 1: what cancho-ui still lacked (Q11, PR A)

> **Status: the six gaps below are built** (2026-10-26). The port itself is PR B and is not in this PR. Everything the page needed that was **not** a gap but a
> judgment about the page is section 7, with a recommendation each, and is **not** decided here. Measured in Chromium (Playwright, `Pixel 7` profile and desktop
> Chromium, not a device) and in `cargo test`; numbers say which. The page is `alpibrusl/cancho-www` at `7578537` (its `index.html` is the one of `0579a80`, 564 lines;
> `assets/map.js` and `assets/site.css` changed since, and `data/demos.json` is new); the clone is read-only and nothing was pushed to it.

## 0. The inventory, and how it was made

Not by reading the page and guessing: **by writing it.** The whole home page was written in cancho-ui as a spike (a scratch file, not committed: it is the list of what could
not be said, and a spike that is kept goes stale). Every section of the page was written down in the order of the page, with the real text for `#try`, `#trust`, `#verifies`,
`#agents`; the data-driven parts over the real `data/ecosystem.json` (SHA-256 `870d4eb8..`); the hero, the cards and the map as islands; the header with its switch and its menu;
the authority animation as state plus an island. Where the compiler said no, the construct was probed on its own in a ten-line program, and the message is the evidence. What it
said, in the order the spike met it:

| # | the page does | cancho-ui said (first error, verbatim) | decision |
|---|---|---|---|
| 1 | **10 `ul`, 1 `ol`, 68 `li`** (17 + 9 + 24 of them written by `gen_agent_files.py` from the data); the five links of the trust chain, the three evidence columns, the three paths, the 24-project list, the endpoints, the status list | `component-kind is not declared: List` | **built, section 1** |
| 2 | **tabs (3), chips (6 + 1), the human/agent switch (2)** with `aria-pressed` | `parse-unexpected-token ... found "identifier"` on `pressed: v == "human"` (designed in `www.md` 5.2, never built) | **built, section 2** |
| 3 | text the Python generator writes from the data: `measured.date` and `platform`, `verifies.disclaimer` and `stamp` (a paragraph under the three columns), `tagline`, `site` | `data-use-invalid: data.get is not a function of data: there is data.items(Name, "path")` | **built, section 3** |
| 4 | **6 `<code>` inside text that comes from the data** (5 in the evidence claims, 1 in the paths) written by the generator from backticks; 7 texts of the data have backticks | none: the backticks were **shown as backticks**, silently | **built, section 4** |
| 5 | the authority animation: tabs, steps, a verdict, and five rocks that move to a position that depends on the step | `island-prop-dynamic: seed: on Stones is read from state; a prop of a sealed island is a literal` | **built, section 5** |
| 6 | links to files of the site: `agents.md`, `llms.txt`, `llms-full.txt`, `data/ecosystem.json`, `demo/demo.cast`, `demo/demo.mp4`, `assets/media/ui-demo.mp4` (7 distinct files; the head's `site.css`, `logo-mark.png` and `logo.png` are three more relative `href`s that `page { }` and the theme carry) | **none**: `Link { to: "llms.txt" }` built, the file was **not in the build**, and no check could tell | **built, section 6** |
| 7 | h1, then three h3 cards, then an h2 | `heading-level-skipped: a level 3 heading follows a level 1 heading` | judgment call J1, section 7 |
| 8 | the brand is an `<a>` around an `<img>`; the menu button is a `<summary>` around an SVG; the GitHub link has a star | `rich-child-invalid: Link holds text and Strong, Em, Small, Break, Code and Link` | judgment call J2 |
| 9 | 17 inline `<svg>`: the scene, three card drawings (animated by CSS), five icons, a star, a hamburger, the authority canvas, the map, four glyphs of the legend | `component-kind is not declared: Icon`/`Drawing`; `image-src-invalid: .svg is not a format of this element` | judgment call J3 |
| 10 | the human/agent switch hides every section and shows `agents.md` | `parse-unexpected-token` on `hidden: v == "agent"`; an `If` around the sections makes them a hole (nothing without script) | judgment call J4 |
| 11 | `nav a.opt { display: none }` under 46 rem, and a `details.menu` that has the same links | no way to say it | judgment call J5 |
| 12 | `blockquote` + `cite` (once) | `component-kind is not declared: Quote` | judgment call J6 |
| 13 | buttons whose text is a glyph (`◀`, `▶`, `▶\|`) and whose name is another | `binding-unknown: description:` | judgment call J7 |
| 14 | the map's detail pane: a `dl` with 4 rows, a recorded session typed line by line, a hiker that walks (`map.js` and `demos.json`, new since the plan) | no `dl`; no timer-driven typing | judgment call J8 |
| 15 | the look: a two-column hero, a 72 rem column, cards with a shadow, a blurred sticky header, 106 classes, 19 viewport rules, 15 `:hover` rules | not a gap: the theme sets colours and fonts, the baseline sets the rest (`presentation.md`) | judgment call J9 |

What was **not** a gap, because the spike said it could be written: the head (`page { }`, 13 metas, 2 alternates, the skip link); the sticky header and the anchors; `Reveal`
(14) and `CountUp` (3); the three waffles and the chips; `Grid` with `weights` for `.try`, `.videos`, `.gallery`; `Media` with captions and transcript, `Image` with webp and jpg;
`Details` and `Summary` for the transcript and the map list; `Code` with the copy button; the open island for the map (`requires Dom`, UNBOUNDED, with a list as its text); the
sealed islands for the hero and the 10 cairns; `Table` for the comparison; `Figure`; `Aside`; `Footer`; the agent pane (`ui/agent-files.cui` of the page is already a cancho-ui
program: one component, `requires Network`, `net.get` of the page's own files). The three fonts are the theme's `body`, `mono` and `hand`.

**Rule kept while building: nothing here was added because it might be useful.** Each of the six was found by an error or by a silence in the spike, and each is
used by `examples/port/port.cui`, which is that spike cut down to the six.

## 1. `List` and `Item`

```
List {
    Item { "No ambient authority" }
    Item { Rich { "Diagnostics as " Strong { "data" } ", with " Code { "--output json" } } Link { to: "notes.md" "notes.md" } }
}
List { ordered: true  Item { Heading { level: 3 "Generate" } Text { "An agent writes." } }  Item { .. } }
List { For { over: data.items(Claims, "claims") as: c  Item { Rich { "{json.get(c, \"claim\", \"\")}" } } } }
```

* **`List` is a `<ul>`** (`ordered: true`: an `<ol>`), **`Item` is a `<li>`**. An `Item` holds what an `Article` holds: text, `Text`, `Rich`, `Heading`, `Link`, another `List`, a `For`, an
  `If`. A `List` holds `Item`s and the `For`s and `If`s that write them, nothing else. They are structure elements: static in the prerendered page (a `ul` with a `For` of state is written
  with the `For` as its hole), with the DOM recipe in `structure::tag` for the script and the prerenderer alike. No class, no style, no key but `ordered:`: the look is the browser's and
  the theme's; a list that is a row of chips, or a numbered chain with connectors, is the page's CSS and is J9.
* **No `label:`, no `role:`, no `id:`** on either (`binding-unknown`): a list is named by what it holds, and an anchor is a `Section`.
* **Why not `Column` and `Row`**: a `ul` of `li` is what a screen reader counts as "list, 5 items"; a `<div>` per row says nothing. *(That the accessibility tree has the list roles was **not** read in this PR: the DOM is measured, the tree is the parity check's, PR B.)*
* Tags (each reached by `tests/structure_corpus.json`): `list-child-invalid` (text, a string, a `For` of text, or another element straight in a `List`), `item-outside-list`,
  `list-empty` (nothing in it: also what a `For` over a data file with no rows writes, so an empty file is a refusal at build and not an empty bullet list), `item-empty`,
  `list-ordered-invalid` (`ordered:` that is not `true` or `false`). A `List` inside inline content is `rich-child-invalid`; `label:` and `id:` are `binding-unknown`.
* **Measured** (Chromium): the prerendered HTML of `examples/port` has 2 `ul`, 1 `ol` and 7 `li` with JavaScript off, and they are the same nodes once `app.js` has run (they are never rebuilt:
  marked before the script, compared after); the 7 DOMs of the corpus are equal in the page's own HTML and in the script's; nested to 200 deep a list is a refusal by the bound of nesting
  (33 and 200: exit 1; 8 and 31: accepted) and never a panic.

## 2. `Button { pressed: <Bool expression> }`

`Button { label: "human" on: Human pressed: mode == "human" }`: a toggle button says whether it is pressed. The value is a pure `Bool` expression of the view, read at every render
(as an `If`'s `when:`), so a tab is `pressed: tab == 0` and no extra state field is needed. The script writes `aria-pressed="true"` or `"false"`; a button with no `pressed:` has no
`aria-pressed` at all (it is not a toggle). Only on a `Button`: elsewhere `binding-unknown`. A number or a text is `condition-not-bool` (the tag of every `Bool` that is not one; the
message says "the `pressed:` of a Button"), a name that is not in scope is `name-unknown`. Nothing new in `requires`; replay does not read it (it has no effect on an event).

**Measured**: the accessibility tree of Chromium (`Accessibility.getFullAXTree`) says `pressed: true` for the pressed button and `false` for the other, and nothing for a button with no
`pressed:`; a click moves it (the status text says which); Space on the focused button moves it and focus stays on the button (the existing focus restoration holds); the static lists are
the same nodes after the buttons were drawn again.

*Not measured*: a screen reader announcing "pressed", or "toggle button" (the tree says it; nobody listened).

## 3. `data.value(Name, "path")`

`data.items(Name, "path")` answered only an array. The page's generated text is also single values (`measured.date`, `verifies.stamp`). `data.value(Name, "path")` is the **text of the one value
at a path**, written out at build like `data.items`: a string as it is, a number or a boolean as written, an object or an array as its compact JSON text (so
`json.get(data.value(Ecosystem, "measured"), "date", "")` reads one field of an object, with the language `json.get` already has). A path with nothing there, or `null`, is
`data-shape-invalid`; a name that is not declared is `data-unknown`; one argument, or a path that is not a literal, is `data-use-invalid`. It counts as a use of the file (`inspect data` says
how many) and **adds no authority and no request**: the page has the text, `app.js` has neither the file's name nor `data.value`.

*Added in B2b (2026-10-26)*: `data.value(..)` is also accepted as the value of a **prop of an island or of a component instance** (`Places { places: data.value(Ecosystem, "projects") ... }`). The map island is handed the 24 projects that way, as the text of the array: the parser took an expression as a prop only after a few names (`pressed`, `to`, ...) and now takes `data.` in any prop of a named element; the expansion writes it out like any other, so by the time anything but the parser sees it, it is a string literal (the island's `props { places: String; }` is checked against it as before). Corpus: a valid case (an open island with the prop) and a refusal (`data-unknown`, a name not declared) in `tests/islands_corpus.json`; break `parser-prop-data-value` of `scripts/breaks/www-port-map.json`.

## 4. Backticks in text from a data file are `Code`

The generator wrote `` `x` `` as `<code>x</code>`. Here: **in a text that comes from a data file and is written inside inline content** (`Rich`, `Heading`, `Caption`, `Summary`, `Link`, `Strong`,
`Em`, `Small`), a pair of backticks makes a `Code` (a `<code>`, the `InlineCode` of `structure.md` section 7). The text of the author is not read this way (an author who wants code writes
`Code { }`), and **neither is a text outside inline content**: a data text in a bare `Text` or `Item` keeps its backticks as characters (measured, below, and said in the corpus). An odd number of
backticks, or a pair with nothing between, is `data-code-unbalanced` and refuses the build: the generator left a stray backtick in the page without a word, and a build that stops is better than
a sentence with a mark in it that nobody sees.

*A deliberate limit*: only a pair. No markdown (no `*`, no `[](..)`): the 7 texts of the real data need nothing else, and a markup language inside a data file is a second language to check.

**Measured** (`scripts/check-port.js`, JavaScript off): the three `<code>` of the claims are `--output json`, `linear-value-unconsumed`, `cancho authority`, and no backtick is left in any item. The
real `ecosystem.json`: 6 `<code>` in the page's generated lists, 7 data texts with backticks (one of them, `projects[1].authority`, appears in the map's detail pane, where the page itself
shows the backticks as characters, because the map writes it with `textContent`: the port keeps that, since the pane's text is not inline content here either).

## 5. A sealed island may read the state

`island-prop-dynamic` is gone as a tag. `Meter { step: step note: note }` is accepted for a sealed island, as it always was for an open one: a prop may be a `state` field of the component, of the
island's declared type (`island-prop-type` otherwise; a name that is not in scope or an expression is `island-prop-source-unknown`; the same island twice with props from state is
`island-open-instance-twice`). **Why**: the authority animation of the page is a state machine (3 scenarios, up to 5 steps, a verdict) whose captions, code lines and verdicts are text and must be
*in the page* (replayable, readable, in the accessibility tree), and whose canvas is a drawing that needs `scenario` and `step` (`www.md` section 9 decided this split). With literal props only, the
whole stepper had to go into the frame, where none of that text is the page's.

How it works, and what bounds it (all measured):

* The frame is the same static element; the prop is **not** in the HTML it was written with (the page's script gives it the value; before that the island has no `step`, and the example's script says
  "step none"). `data-cui-island` gains `"typed": {"note": "String", "step": "Int"}`, the props the runtime gives again.
* The runtime gives a frame the state's values **when it registers it, and after a change at most once in 50 ms** (the latest value; a burst is one message): a hundred clicks in one burst
  made **1 message** (1 posted, 1 heard by the frame, and the last value, 101, arrived), and the frame was not flooded.
* **A value has the type the island declared** or it is not sent: a `String` over 4096 characters is dropped and counted in `window.__cui_island_stats.dropped` (as an event the page does not hear is),
  and the frame keeps the last value it had: 2560 characters arrived, 5120 did not.
* **Authority does not change** (tests): `requires` is empty before and after, `diff` of a literal prop and a state prop says `authority: unchanged`, `inspect effects` lists the island by its files and
  hashes as before ("reaches: nothing but this page, by messages"), and a frame that reads the state still gets a `SecurityError` for `parent.document`. A sealed island can do **less with a value
  than a script of the page can**: draw it. What state the program lets it read is what the program wrote after `:`; the island cannot ask for another field.
* *Not claimed*: that nothing sensitive is in the state field the program names: that is the program's choice, as it is for any text it shows. `inspect effects` does not list which fields an
  island reads (it lists the island, not the instance); `diff` shows the line of the view that changed.

## 6. A link to a file of the site is a file of the build

Before: `Link { to: "llms.txt" }` was accepted and the build did not contain `llms.txt`. The page has seven such links. Now a **relative `to:`** (no scheme, no `#`, no `//`) **is a file of the site** and is
checked like a picture: a path of letters, digits, `.`, `_`, `-` and `/` (no `..`, no empty segment, no leading `/`: `link-file-invalid`), inside the directory of the program, at most 20 MB, read and
hashed, copied into the build at the same path (`build --page` and `--pwa`; `--pwa` does not cache a video), and `diff` reports `~ file notes.md: <12 digits> -> <12 digits>` as for a picture (a changed
file is a change of the program, not of authority).

**Which files**: md, txt, json, cast, csv, pdf, the picture formats of `Image` and mp4, webm. **Not** a page, a script, a style or an SVG (`html`, `svg`, `xml`, `js`, `css`, `wasm`): a browser runs those as the
site's own, and the program has `Image`, `island` and `theme` for the ones it means. Text is checked to be UTF-8 with no NUL (`asset-type-mismatch` for bytes with a text name), a PDF begins `%PDF-`.

**This is a stricter rule for programs that already linked a relative path**: `examples/structure` and the structure corpus did; they now carry the file (`examples/structure/agents.md`, and `agents.md` in
the corpus's assets). A link to `/` or to `../x` is `link-file-invalid` (it was accepted and meant a path of the host, which the program does not own).

**Measured** (Chromium, JavaScript off): the two files of `examples/port` answer 200 from a static server with the bytes of the program's own files (SHA-256 equal); an unlinked file is not copied.

## 7. Judgment calls: not decided here, each with a recommendation

These are the places where the page and cancho-ui disagree and **either could change**. None is built. PR B will follow the recommendation unless told otherwise.

| | the question | recommendation |
|---|---|---|
| **J1** | The page's outline is h1, h3, h3, h3, h2 (the three cards use h3 under the h1; `heading-level-skipped` refuses it). | **Write them as h2** in the port and say so in the parity report: a defect of the page that the compiler found, fixed in the port. Needs the user's word, because the outline parity check will list three headings whose level differs. |
| **J2** | An image or an icon **inside a `Link` or a `Summary`** (the brand logo in its link; the hamburger in the menu's summary; the star in the GitHub link). | **Do not build.** Put the logo `Image` beside the brand `Link` (the logo is then not part of the link: the link is "cancho"), give the menu summary the text "Menu" and the GitHub link no star. Cost: three small visual differences, listed. |
| **J3** | The 17 inline SVGs (icons, glyphs, card drawings). `Image` refuses SVG (it can carry script). | **Cards: a sealed island each** (they are drawings with their own CSS animation, which a sealed frame's `style:` carries; 3 more frames, 14 in all; measured cost per frame is in `islands.md` section 3). **Icons (5 + star + hamburger + 4 legend glyphs): leave them out.** They are `aria-hidden` decoration. A hash-pinned SVG file with an allow-list of elements (`Drawing`) would bring them back; it is a real feature with a parser to get right, so it should be asked for, not slipped in. |
| **J4** | The **human/agent switch** (hides the whole page and shows `agents.md`). `If` around the sections would make them a hole, so the page would be empty without JavaScript. | **Drop the switch; keep `#agents`, its endpoints and the agent pane** (which already reads the same files). The page's own text says "Flip the switch in the header": that sentence changes. The alternative is a `hidden:` on static elements that prerender writes in its initial state and the script updates; it touches the prerenderer's rule that a static node is never written by script, so it is a design note first. |
| **J5** | Nav links hidden under 46 rem, with a `details` menu that has the same links. | **Show the menu at every width and the nav links only in the menu** (one list of links, no duplicate): the parity check then lists 8 links fewer in the header on desktop. Or a `Nav { collapse: .. }`; one use does not make a feature. |
| **J6** | A `blockquote` with a `cite` (the pull quote, once). | **`Figure` with a `Caption`** (the quote is the figure, the citation the caption): no new element; the accessibility tree has a figure where the page has a blockquote. |
| **J7** | `◀ ▶ ▶\|` as the visible text with another accessible name. | **Visible text is the name**: "Previous step", "Play", "Next step". It is what WCAG 2.5.3 asks (the visible label is in the name). The icons-only look is J3's. |
| **J8** | The map's `dl`, the typed recorded sessions and the walking hiker (`map.js` 346 lines, `demos.json` 624). | **Port the detail pane as text** (name, stage chip, summary, the rows as a `List`, the links, and the recorded session as a `Details` with `Code`), **without** the typing animation and the hiker. They are a second application inside the page; the open island can keep the hiker if the user wants it. |
| **J9** | The look: the page's 106 classes and 19 viewport rules against the baseline plus theme. | **Accept the baseline's look, with the theme's colours and fonts, and let the pixel-diff report say how far it is** (PR B). Each difference is classified; if the user then wants the page's look, that is a design (a handful of roles, `Card`, the header's arrangement), not a patch. 15 `:hover` effects, the stagger of `Reveal`, the rounded corners of the waffle squares and the page's copy button stripping `# comments` are in the same box: **intentional differences**, listed one by one. |

## 8. Checked

* **Corpora**: `tests/structure_corpus.json` 38 to **44** valid views (each with the DOM it must become, equal in Chromium in the page's own HTML and in the script's, and the part a visitor without script gets) and 166 to
  **194** refusals; `tests/data_corpus.json` 17 to **20** valid and 38 to **45** refusals; `tests/islands_corpus.json` 12 to **13** valid and 74 to **75** refusals (a tag went: `island-prop-dynamic` had two cases, now
  `island-prop-type` for a field of another type, `island-open-instance-twice` for a sealed island twice, and `island-prop-source-unknown`). **7 new rule tags** (`list-child-invalid`, `item-outside-list`,
  `list-empty`, `item-empty`, `list-ordered-invalid`, `link-file-invalid`, `data-code-unbalanced`), each documented in `spec/diagnostics/README.md` and reached by a case; one tag removed
  (`island-prop-dynamic`), and the README has no row for it (a test).
* **`crates/cancho-ui/tests/port.rs`** (7): the example needs no capability and no effect, names its data file and island by hash, and its report has the island as its one trust boundary; the build carries the files the
  links name, byte for byte, and the script holds nothing of the data; a changed file is a `~ file` line of `diff` and `authority: unchanged`; a prop read from state changes no authority; a program cut anywhere
  and a list nested to 200 are refusals and not panics; the new tags are documented.
* **`scripts/check-port.js`** (Chromium, Pixel 7 profile; 27 checks): everything measured above.
* **`scripts/breaks/port.json`: 36 deliberate breaks, each caught by a check failing on its own** (none `HUNG`, none `BUILD`, none `BAD`). **One was missed on the first run**: removing the first merge of the state
  into a frame changed nothing the check saw, because the page draws twice and the second draw gave the frame the value; so the example's island now records the *first* message it was sent (`data-first`), the
  check reads it, and the break is caught (the island's hash in `examples/port/port.cui` moved with it).
* **Changes to checks that already existed**, each because a measurement of the old behaviour became false: `scripts/check-data.js` compared the 17 claims to the file's text *with* its backticks (the gap this PR closes), and
  now expects the text without them and the 5 `<code>` elements; `scripts/check-structure.js` and the structure corpus carry `agents.md` (a link to a file is now a file); `examples/structure/agents.md` is new;
  `scripts/breaks/islands.json` lost the one break whose code is gone (`island-prop-dynamic`). `docs/index.html` and `scripts/check-gallery.js` say 36 programs (`examples/port`).
* **A finding about checks that already existed, not fixed here**: of the 769 breaks of the other files of `scripts/breaks/`, **32 have a `from` that no longer occurs exactly once in the source on `main`** (checked
  with `git show origin/main:..`): the harness reports `BAD` for them, which does not count as caught. They are in `checkbox`, `field-error`, `handler-seq`, `i18n`, `input-attrs`, `input-types`, `map`, `net-write`,
  `prerender`, `select`, `structure` (1), `structure-b2` (3), `structure-b3` (1), `textarea` and `theme` (3). One more (`data.json`, "any name is the data") became two occurrences with this PR's `data.value` and was
  fixed here by spelling the lookup of `data.value` differently.
  **Corrected 2026-10-26 (the breaks hygiene PR, `ccr-breaks-hygiene`): all 32 are fixed**, each `from` rewritten against the source of today and the break run again, caught by a check failing on its own. Seven moved
  because the code moved (the copy box is `recipe.rs` and `copy_button`, not `code.rs`'s three lines; `is_focusable` is one `matches!` per line; `net-write`'s list is indented one more level; the page's tokens are
  expanded in one pass), and **three were not caught once fixed**: `number is offered` (the input type `number` exists now; the break became `number is not offered` and `check-number.js` and `cargo test number`
  were added to its checks), `a details has no summary line` (the code it spoiled was the path of an element with no recipe, which no `Details` takes; it now spoils the live path, and `check-structure.js` G2,
  a `Details` under an `If`, catches it) and, in the full run of `structure-b3.json`, `a gap of a Grid is left to nobody`, an **equivalent mutant**: with the edit the `index.html` and `app.js` of a program with `Grid { gap: 3 .. }` are byte-identical to the unedited build, static or in a hole, so no check can catch it; it is moved to the `equivalent` list of that file, with the measurement. The aborted regression runs of
  `islands.json`, `islands-open.json`, `data.json`, `structure.json`, `structure-b2.json` and `structure-b3.json` were run to the end: every break is caught except that one.
* **Gate**: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (721 passed, 0 failed), all 60 `scripts/check-*.js` and the two `--self-test` runs; once in the working tree, once in
  a fresh clone of the pushed branch (the PR body has the numbers).

## 9. Not measured, and limits

* Safari, Firefox, a phone, a screen reader (none was available); `backdrop-filter`, `:has()`, `color-mix()` are not used by anything built here.
* The spike was compiled, not run in a browser: the spike's job was to find what the compiler refuses; whether the whole port looks and behaves like the page is PR B's pixel diff.
* A list nested past 32 views is refused by the bound of nesting, with that rule's message, not one about lists.
* `data.value` has no way to read a number as a number: it is text (the same as `json.get` answers), so `num(..)` is used where the program wants one.
