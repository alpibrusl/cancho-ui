# Prerender: the static parts of a page, written at build time (P1)

> **Status: P1a and P1b built** (2026-10-24; section 12 is P1a, section 13 is P1b, and each says what the build changed in this note). It answers `www.md` 5.2c and section 10 item 7 (the user chose option B: prerender what is static, say plainly that an element with state is not).
> Every number below was measured on this machine in Chromium (Playwright, desktop Chromium; the `Pixel 7` profile is a viewport and a user agent, not a phone) unless it says *derived* or *not measured*.
> The scripts are throwaway (`/tmp`), their method is stated with each number; the checks that will keep the claims true are in section 8.

## 1. The problem, measured

| | measured |
|---|---|
| a `build --page` program with JavaScript off | `<div id="root"></div><noscript>This app needs JavaScript.</noscript>`: 3 elements (`www.md` 5.2c) |
| the cancho-www home page with JavaScript off | 738 elements, 16,006 characters of text; `index.html` is 49,368 bytes (14,027 gzipped) |
| first contentful paint of a page of 901 elements (`app.js` held back 1.5 s, a synthetic delay and not a network profile) | **1,580 ms without prerender, 60 ms with the same DOM written into `index.html`** (the same 25,489 characters of text visible) |
| the same page opened at `#s70` with `app.js` held back | without prerender the target does not exist for 1.5 s (`scrollY` 0); with it, the element exists and the page **still sits at `scrollY` 0 until the script arrives**; see 5 |

The point of the page being readable without scripts is that a crawler, a link-preview fetcher and an agent that fetches HTML (`agents.md`, `llms.txt`) read it; and that a person on a slow link reads while the script loads.

## 2. What is static

The compiler already knows, for every view node, whether it depends on anything that changes. **A node is static when it and all it contains are literals**: an element none of whose bindings is a state reference, an expression, an event or a `disabled`/`error`/`checked`/`value`; a `Text` or a `Heading` whose parts are all literal text; no `For`, no `If`, no component instance with properties.
A **hole** is a node that is not static, and its whole subtree: a `Text` with `{n}`, a `Button` (its click is JavaScript), a `For`, an `If`, a `TextField`, a `Chart`, a `Map`, an instance of another component. A static element **may contain holes** (a `Section` with a `Heading`, three paragraphs and a `For` is static; the `For` is the hole in it) **unless it is text**: a `Heading`, `Rich`, `Link`, `Caption`, `Summary`, `Strong`, `Em`, `Small`, `Text` or inline `Code` is *atomic*, written whole or not at all, because "Count: " without its number is a wrong sentence and not a shorter one (*added in the build, 12*). A `Text { role: heading "..." }` is static; a `Text` with another role (`status`, `alert`) is not, because its node is a live region the script keeps from one render to the next. A page whose static elements hold no text, picture, video or code (only empty layout boxes) has no static content: nothing is written for it and it keeps the old line (6).

* **Computed, not declared.** `www.md` 5.2c suggested `Section { static }` so that the program says so. It is dropped: the compiler has to compute staticness to check the claim anyway, and an annotation that can only be right or refused adds a way to be wrong. The compiler reports what it prerendered (`inspect`, section 6) instead.
* **The initial state is not used.** Option A (render the first view from the initial state) was not chosen. A `Text { "{n}" }` is a hole even though `n` starts at 0, and a `For` over data fetched at `start` is a hole: its content does not exist at build time, and one over a literal list is also a hole (one rule, no exceptions to explain).
* **Classified by reading, not by the compiler:** of the 29 valid views of `tests/structure_corpus.json`, 24 are fully static and 5 have a hole (an interpolation in a heading, a `Rich` with an interpolation, a copy button, a brace and an interpolation in a code block, a `For`).
* A control with JavaScript behaviour is a hole, and **a hole draws nothing without JavaScript**. The copy button of a `Code` is one: the box is prerendered (`data-copy` and all), the button is not, so a visitor without scripts is not shown a button that does nothing.

## 3. One recipe, so the two cannot differ

The elements of B1 to B3 already have their DOM in the IR crate and the JavaScript emitter only writes calls for it: `structure::{tag, attrs, class, summary, code_spec}` and `media::{image_dom, media_dom, class, grid_style}`. The prerenderer **calls the same functions**; it has no table of its own.

What is *not* yet in a recipe: the generic elements (`Column`, `Row`, `Text`, `Table`, `Cell`) and the attributes common to all elements are written inline by `codegen-web/src/view.rs` (the layout class, `role`, `aria-label` from `label:`, `data-tone`, the `cui-gap-N` class, `max-width`, text as a text node or a `span`). **P1 moves those into `cancho_ui_ir::recipe::static_dom(node) -> Option<Dom>`** (`media::Dom` already is a tag, ordered attributes, children and text), calls it from `view.rs` for every static node, and from the prerenderer. After that the JavaScript emitter does not decide what a static element is: a break that changes a tag in the recipe changes both, and a break in `view.rs` that bypasses the recipe is caught by the equality check of section 8.

## 4. Hydration: what the script does to what the page already shows

Measured, on `examples/structure` (46 elements) with its own rendered DOM written into `index.html` and `app.js` held back 1.5 s; before the script a user tabs to the first link, selects a paragraph, and opens the `<details>`:

| after the script ran (today's emitter, which empties `#root` and rebuilds) | |
|---|---|
| nodes | 46, the same (no duplicates) |
| pixels | **0 of 2,382,564 differ** between the screenshot before and after; layout shift 0 |
| the focused link | **kept in this measurement, because the DOM written into the page had the same shape as the one the script builds** (the emitter saves the focus by path and restores it). *Corrected in the build, 12: a page that omits the holes does not have that shape: a button before a link changes the link's place among its siblings, the path points elsewhere, and the focus is lost (measured by `check-structure.js` part G (c) with the lookup below switched off: the link has the focus before the script and the body after it); P1a adds a lookup by address and text to the emitter's restore* |
| the text selection | **lost** |
| `<details open>` | **closed again** |

And what any re-parenting does, measured on a focused link, an open `details`, a selection, a scrolled `pre` and a playing video: `appendChild` into another parent and `replaceChildren` of the same nodes both lose the **focus**, the **selection** and the `pre`'s **scroll position**, and keep `details.open` and a playing video (it was still playing, 0.5 s in); `insertBefore` of an unrelated node keeps everything but the moved node's own focus. So *adopting* prerendered nodes by moving them into a new tree does not help: **only a node that is never detached keeps what the visitor did to it**.

Three designs, from the measurement:

| | what happens | cost |
|---|---|---|
| H1. **replace** (the emitter unchanged; `#root` is emptied and rebuilt) | the first render replaces the prerendered nodes with equal ones | nothing to build; loses the selection and the `details` state of a visitor who acted before the script; every later render does the same, as today |
| H2. **holes** | the JavaScript builds **only the holes**, into the places the prerendered HTML marks (`<!--cui:h3-->`), and never touches a static node; a render rebuilds the holes and nothing else | the emitter's render changes from "rebuild the tree" to "rebuild the holes"; static elements stop being built by JavaScript at all (derived saving, 8) |
| H3. adopt by moving | see above: loses focus, selection and scroll | rejected |

**Recommendation: P1a = prerender with H1; P1b = H2.** P1a is what the crawler, the agent and the first paint need and changes nothing in the emitter's render; its known cost is the two losses above, for a visitor who acts before the script runs. P1b removes them, and makes every later render cheaper, but it changes the render function of every program, so it is a PR of its own with its own breaks. (Splitting is the coordinator's call; if P1b is not wanted, P1a stands alone.)

## 5. Focus, ids and anchors

* **Ids are the program's own** (`id:` on a structure element, `cui-` is the compiler's and refused to authors): prerendered as they are, so `#try` exists before the script. A hole's marker is a comment, not an element: no id, no duplicate.
* **Fragment links.** Measured: with the DOM prerendered and a deferred module script held back, Chromium **does not scroll to `#s70` until the script arrives** (the document is not finished parsing), and then jumps. With `async` on the module it scrolls at once, but then a page with no prerender never scrolls to its fragment at all (`scrollY` stays 0 after the script: a defect of today's pages if the script were `async`; they are not). With **`<script>var h=decodeURIComponent(location.hash.slice(1)),e=h&&document.getElementById(h);if(e)e.scrollIntoView()</script>` written right after the prerendered root** (about 120 bytes) the prerendered page is at the target before the script (`scrollY` 18,946 at 700 ms) and stays there after it. P1 writes this script when the HTML it prerendered has an `id`. It is inline, so a page that sets its own Content-Security-Policy without `'unsafe-inline'` would block it; the compiler's own CSP (a map's `img-src`) does not.
* **Focus.** A prerendered link is focusable before the script. Under H1 the emitter's save-and-restore keeps it **only if it can find the link again**: see 12 for the lookup P1a adds and for the browser's own fragment rule; under H2 the node is never touched.
* **Screen focus** (`screen-focus.md`): the first render of a program with `route` moves focus to a heading as today; prerender does not change when that happens.

## 6. What a visitor without JavaScript sees, declared

| part of the program | without JavaScript |
|---|---|
| a static element | drawn, with its text, links, images, videos with controls, `details` that open |
| a hole | **nothing** (no placeholder text) |
| a program with static content and any hole | `<noscript>` says "Some parts of this page need JavaScript." |
| a program with static content and no hole | no `<noscript>` line: nothing is missing |
| a program with no static content | the old line, "This app needs JavaScript.", and an empty root |
| an island (step 3) | its `label:` (already required) is written as the text of its placeholder, so what the picture is called is not lost; an open island (the map) keeps its list as the text alternative, which is already the plan |
| a control (`Button`, `TextField`, the copy button) | not drawn: a visitor without scripts is not shown a control that does nothing |

`cancho-ui inspect prerender <Component>` (new) lists, per component, the static roots written into the page and the holes, with the reason for each hole ("`Text` reads `n`", "`Button`"), so that a person sees what an agent will read.

## 7. What the build writes

`build --page` and `--pwa` write the prerendered nodes **inside** `<div id="root">` of `index.html` (the offline worker already caches `index.html`). Only the **entry** component of the program (the one auto-mounted into `#root`) is prerendered; instances of other components are holes, as above. (`--no-auto-mount` cannot be combined with `--page` or `--pwa`, which the compiler already refuses, so the root is always the mounted component's.) Prerendered text and attributes are escaped by one function (`& < > "`), and a `Code` or a `Text` containing `</script>` or `<!--` stays text (a case in the corpus).

## 8. Costs, measured

Programs of N repeated blocks (a `Section` with a `Heading`, a `Rich` with a `Strong`, a `Link` and a `Code`, and a `Code` block: 9 elements) built with `--page`; the HTML is `#root.innerHTML` as the script makes it:

| elements | HTML bytes | `app.js` bytes | HTML per element | `app.js` per element |
|---:|---:|---:|---:|---:|
| 10 | 450 | 7,519 | 45 | (the program itself) |
| 451 | 21,943 | 115,417 | 48.7 | 239 |
| 901 | 43,893 | 226,068 | 48.7 | 243 |
| 1,801 | 87,993 | 449,805 | 48.9 | 246 |

So prerendering costs **about 49 bytes of HTML per static element** and the JavaScript that builds the same element costs **about 240**. *Derived, not measured*: a page of the cancho-www page's 738 static elements would add about **36 kB** of HTML; under H1 it still carries about **177 kB** of JavaScript that rebuilds them (the whole `app.js` of the page would be larger than the page's `index.html`, `site.js` and `site.css` together, 109,572 bytes); under H2 that JavaScript is not written at all, because static elements are not built by it *(measured in P1b, 13: a 901-element static page's `app.js` goes from 226,946 to 7,283 bytes; the derived 177 kB for the cancho-www page is still a derivation from 240 bytes an element)*. Gzip is not given for these programs: their blocks repeat, and they compress to 1,760 bytes for 87,993, which says nothing about a real page. The real page's `index.html` is 49,368 bytes, 14,027 gzipped.

## 9. The check that keeps it true

A new part G of `scripts/check-structure.js` (and a Rust test) over **the same corpus** (`tests/structure_corpus.json`, all `valid` views, including the B3 ones with files):

1. each valid view is built with `--page`; the page is opened **with JavaScript disabled** and `#root` serialised by the check's own serialiser (the one of part A); for a fully static view it equals the corpus `html` exactly; for a view with a hole it equals the corpus `html` **with the holes removed** (the corpus gets an `html_static` field for those 5 views, written by hand);
2. the same page opened with JavaScript, after the script has run: the static parts of the DOM equal the same string (hole contents replaced by their marker before serialising);
3. the page opened with `app.js` held back and the script released: no node is duplicated (the count of every `id` is 1), `scrollY` at a fragment is the same before and after, the focused element is the same;
4. a no-JS page's text is what 6 says (a hole contributes nothing; no control is drawn);
5. the byte cost: `HTML per static element` stays under 100 (a ceiling a wrong recipe would break), reported with the table of 8.

Deliberate breaks planned (each must fail a check on its own): a tag changed in the recipe; the prerenderer using its own escape; a hole's content written into the static HTML; a control drawn without JavaScript; the hash script left out; `<noscript>` kept on a fully static page; a static node emitted twice; `view.rs` bypassing the recipe for `Text`; an id duplicated by a marker; and under H2 a static node rebuilt by a render (caught by the identity check of 3).

## 10. Not measured, and what could make this wrong

* Safari and Firefox (fragment scrolling while a module script is pending is Chromium's behaviour here); a real phone; a slow network (the 1.5 s hold is a synthetic delay on `app.js`, not a throttled connection); a screen reader reading the prerendered page before the script.
* The 738-element figure for the port is *derived* from 49 B/element; the port (Q11) will produce the real number.
* Whether `async` module scripts are worth having once the hash script exists: not measured.
* H2's saving (the JavaScript of static elements not written) was derived here and is measured in 13: it is large where static content dominates, and **negative for a page that is mostly holes**.
* A visitor who acts before the script loads under H1 loses a selection and an opened `details`; a playing video kept playing in the move test but was not tested under a rebuild of the real emitter with a `Media` element.

## 11. Decisions for the coordinator

1. **P1a (H1) first, P1b (H2) as its own PR**, or both in one. Recommendation: split, as above.
2. A **new subcommand** `inspect prerender` and a **`recipe` module** moved out of `view.rs`: part of P1a.
3. The `<noscript>` line is reworded and is absent from a page with static content and no hole. (Decided 2026-10-24. The 13 gallery demos were checked one by one in the build: 12.)

## 12. What P1a built, and what the build changed in this note

* **`cancho_ui_ir::recipe`** (`shell`, `code_skeleton`, `prerender`): the shell of every static element (tag, attributes, class, style, a `Details` summary) is one function. `codegen-web/view.rs` calls it for every element it has a shell for and writes the script from it; the prerenderer calls it and writes HTML. The `Code` box is `recipe::code_skeleton` in both. Elements with a recipe of their own (`structure::*`, `media::*`) are called, not copied. The rest of `view.rs` is unchanged: a node with no shell (a control, a table, a chart ..) is a hole.
* **`build --page` / `--pwa`** write the prerendered HTML inside `#root`, the fragment script when it has an `id`, and the `<noscript>` line by the table of 6. The program's text is put in after the page's template is filled, on both sides of it, so the text `@TITLE@` or `@ROOT@` in a program or a page name stays text (a test).
* **`cancho-ui inspect prerender <file> <Component>`**: `mounted`, `static_elements`, `content`, `html_bytes` and the `holes`, each with what it is and why.
* **Corrections the build made to this note:**
  1. *Focus.* Section 4 said the emitter's save-and-restore keeps the focused link. That was measured on a page whose prerendered DOM had every node of the script's DOM. A page that omits the holes has other child indices, and the link was **lost** (measured, with the lookup below switched off: a link focused before the script, with a button before it in its parent, was the body after it; a first check on a footer link looked like the same failure and was not: that one was the browser's fragment rule, item 2). The emitter's restore now falls back, for links, summaries and code boxes, to the element with the same address and text and the same place among those (`nth`); with it the same check passes. This also fixes a case that existed without prerender: a link whose place in the tree moved because an `If` appeared above it.
  2. *A fragment in the address.* The browser runs its fragment navigation again when the page has loaded, and that **clears the focus** (measured on a page with no prerender too: the focus is lost at load). So a visitor who tabbed before the script on a page opened at `#id` loses the focus. Nothing the compiler writes can stop the browser doing it; the check looks at focus on a page opened without a fragment, and at the scroll position on one opened with it.
  3. *Text is atomic* and *a page of empty shells is blank* (section 2): both were found by the gallery. Before them `counter`, `clock` and the other eleven would have had an empty `<div class="cui-col"></div>` and the line "Some parts of this page need JavaScript." on a page that has nothing without script.
  4. *The scroll position* after the script on a page opened at `#demo` is not always the same as before it: the script fills the holes and the page gets taller, so a target near the end can move to a lower scroll position. The check asks that the target is at the top or the page is scrolled as far as it goes.
  5. *A bug that was there before this work*: a component with an `If` and no focusable element (no button, no field, no route) referred to `__cui_appeared` without the adapter that defines it, and **threw a `ReferenceError` the first time it rendered**. The corpus case "an If is the script's" found it; the adapter is now included when a view has an `If`.
* **The 13 gallery demos**, with `inspect prerender` on each and the sources of `counter`, `notes`, `routes`, `i18n` and `ops-console` read against it: every demo has a control or a state-bound text, so every demo has a hole and keeps a line. Five have static text of their own and get "Some parts of this page need JavaScript." (`notes`: a label; `ops-console`: its headings; `upload`, `charts`, `reorder`: a heading or a label); the other eight (`counter`, `clock`, `search`, `cards`, `match`, `routes`, `i18n`, `map`) are controls, charts and state only and keep the old line and an empty root. A test pins this.
* **Checked.** The same corpus as the structure elements: `check-structure.js` part G builds each of the 35 valid views and reads the page **with JavaScript off**: 26 are the corpus's DOM exactly and 9 (a heading with a number, two sentences that read state, a copy button, a code block that reads state, a grid with a `For`, a button among static siblings, a `details` with a `For`, an `If`) are the DOM with the holes left out (`html_static`, written by hand). Then the example page with `app.js` held 1.5 s: no control without script, headings and text present, the fragment reached before the script, nothing duplicated after, no layout shift (0), the footer link focused before the script focused after. `cargo test --test prerender` (11) covers the build, the tokens, the numbers (under 100 bytes an element on a 540-element page), the `inspect` output, garbage text, and the demos; unit tests in `recipe.rs` cover a carriage return (`&#13;`, because the parser would make it a line feed) and a NUL (a hole: HTML cannot hold it; the lexer already refuses a NUL in a source file, so only the IR can reach it).
* **Breaks** (`scripts/breaks/prerender.json`, 27): a Column without its class, a gap that is not a class, a max width in px, `<`, `&` or a quote written unescaped, a carriage return written raw, a NUL written, a hole in a sentence leaving the rest of it, a status text taken as static, a page of empty boxes taken as content, text in a sentence as a span each, a `details` without its summary line, ids not counted, a Heading one level off, a code box without `tabindex`, the line kept on a page with nothing missing, the old line on a page with holes, the fragment script left out and written on every page, the program's text read for tokens, an empty box written for a page with no content, the script not writing a shell's class or a details' summary, an `If` without its adapter, a link that moved not found again, and `inspect` calling every component mounted. **24 were caught on the first run, each by a check failing on its own (none `HUNG`, none `BUILD`); 3 were missed**, each because no case used what the break spoiled: a `max_width` (no corpus view had one), a quote in an attribute (none had one), and the lookup for a link that moved (the footer link of the first test kept its place; the test's own program now has a button before the link). The corpus has the first two (`a column with a maximum width`, `a quote, an ampersand and a bracket in an attribute`; the check's serialiser leaves `<` and `>` in an attribute as they are, and the corpus says so), the check has the third, and the three are now caught.
* **A check that had to change.** `check-mobile-demos.js` decided a demo had rendered when its root had text; with the page written at build time that is true before the script runs (its self-test break "a demo is blank" was no longer caught). It now asks for the root to differ from the root in the page's own HTML, and the self-test catches the blank demo again. The pinned snapshot tests and `docs/demo/counter.js` follow the recipe (`setAttribute('class', ..)` for `classList.add`), and the gallery was rebuilt.
* **Gate.** fmt, clippy `-D warnings`, `cargo test --workspace` 643 passed 0 failed, all 54 `check-*.js` and both `--self-test` runs; run on the code before the last three corpus cases and one check addition (part G (c)); after them `cargo test --test structure` (13) and `check-structure.js` were run again and pass.
* **Not done in P1a, done in P1b (13):** the holes. A visitor who selects text or opens a `<details>` before the script loses both when the script runs, as measured in 4. The script is still the whole of `app.js`: its size does not shrink yet.

## 13. What P1b built (holes): static nodes are never touched

* **The skeleton.** `recipe::skeleton` is `recipe::prerender` with a marker pair, `<!--cui:N--><!--/cui:N-->`, where each hole goes, and a hash of the HTML (FNV-1a 64, a label for "the same build", not a security measure). `recipe::hydrating` says which component is hydrated: the **last** component of the program, when it has static content (section 2) and no other component draws it as an instance; any other build is as in P1a. A `Code`'s copy button is a hole of its own, in the box the page already has, after its `pre`.
* **`build --page` and `--pwa`** write the skeleton inside `<div id="root" data-cui-skeleton="HASH">` and tell the emitter to **hydrate** (`EmitOptions.hydrate`, set by those two flags only; `build` to a file or to the screen still writes a module that builds everything, because the page it is put in is not known). A hydrated script:
  1. at mount, if `data-cui-skeleton` is not its own hash, **leaves the page as it is**, writes one line to the console, and throws nothing (a script from one build on the page of another: measured; the static part stays readable);
  2. renders by building each hole into a fragment and putting it between its markers (`__cui_fill`), removing what was there; it **never creates, moves or removes a static node**, and it does not empty `#root`.
* **A bug that this found and fixed, present before P1a:** the record of the screens an `If` drew (`__cui_ifs_now`) kept `[parent, from, to]`, and a hole is built in a fragment that is emptied into the page, so `__cui_appeared` read an emptied fragment and threw. It now keeps the nodes.

### What the visitor keeps (measured)

`scripts/measure-hydration.js` runs one program (a heading, a sentence with a `Strong`, a `details`, a code box scrolled to 40 px, a link, a button, a count) built by two compilers: P1a (`5fad649`) and this one. The script is held back 1.5 s; before it runs the visitor selects the sentence, opens the `details`, scrolls the code box and focuses the link. Chromium, `Pixel 7` profile:

| compiler | `app.js` | `index.html` | after the script: selection | details open | pre scroll | link focus | static nodes the same | after a click: details open | pre scroll | static nodes the same | count | layout shift |
|---|---:|---:|---|---|---:|---|---|---|---:|---|---|---:|
| P1a | 10,811 | 7,471 | **lost** | **lost** | 0 | kept | **no** | **lost** | 0 | **no** | Count: 1 | 0 |
| P1b | 9,172 | 7,558 | kept | kept | 40 | kept | yes | kept | 40 | yes | Count: 1 | 0 |

"The same" is `===` on the eight static elements (`h1`, `p`, `strong`, `details`, `summary`, `pre`, `code`, `a`) held before the script ran. The check that keeps this true is `check-structure.js` part H (selection, details, scroll, focus and identity after the script and after a click; the count changes; a script on the wrong page).

### Bytes (measured, this machine, debug binary)

`app.js` of the same program built by P1a and by P1b (`build --page`); the first four are the repeated-block programs of section 8, which are all static:

| program | static elements | P1a `app.js` | P1b `app.js` | `index.html` P1a → P1b |
|---|---:|---:|---:|---|
| 1 block | 10 | 8,694 | 7,281 | 6,722 → 6,759 |
| 50 blocks | 451 | 116,445 | 7,282 | 28,216 → 28,253 |
| 100 blocks | 901 | 226,946 | 7,283 | 50,167 → 50,204 |
| 200 blocks | 1,801 | 450,383 | 7,283 | 94,267 → 94,304 |
| `examples/structure` | 42 | 21,180 | 12,686 | 7,932 → 8,094 |

**The script shrinks where static content dominates, and a fully static page's script is the runtime and nothing else (7.3 kB); it grows where a page is mostly holes.** The 13 gallery demos (`app.js`, P1a → P1b; the last column is `index.html`):

| demo | P1a | P1b | change | `index.html` |
|---|---:|---:|---:|---:|
| `counter`, `clock`, `search`, `cards`, `match`, `i18n`, `map` | | | −142 each | +0 |
| `routes` | 18,230 | 18,127 | −103 | +0 |
| `notes` | 10,281 | 11,329 | **+1,048** | +137 |
| `upload` | 28,206 | 29,195 | **+989** | +137 |
| `charts` | 20,839 | 21,896 | **+1,057** | +187 |
| `reorder` | 20,292 | 21,292 | **+1,000** | +137 |
| `ops-console` | 51,855 | 53,436 | **+1,581** | +341 |

Eight demos have no static content and are not hydrated: they are 142 bytes smaller (seven) or 103 (`routes`), a shorter comment in the screen-focus helper and nothing else; the five with a little static text (a label, a heading) are hydrated and **grow by about 1 kB**: the hydration helpers (737 bytes) and a fragment around each hole cost more than the three or four static elements save. So the claim "`app.js` shrinks" is true for a content page and false for a small app; P1b does not claim more. (The cancho-www page is the first kind; the port will give its real number.)

### Checked

`check-structure.js` part H (8 checks), the corpus of part G and A unchanged (the markers are comments, which the check's serialiser leaves out, and every one of the 37 views is still the DOM the corpus says, with script and without), `cargo test --test prerender` (16: the markers, the hash, the copy button's place, no static element in a hydrated script, the plain module and the content-free page unchanged, the component another draws), `scripts/breaks/hydration.json`. The 14 deliberate breaks of `scripts/breaks/hydration.json` (the script clears the page before filling; a hole keeps what it had and gets more; a hole filled at the end of its parent or before its marker; no check of the page the script is on; one hash for every skeleton; a skeleton with no markers; a hole looked for at the wrong place; a component another draws hydrated; a page with no content hydrated; `--page` building a script that rebuilds; the copy button put after the wrong element; the screens an `If` drew not kept; an unmarked node built) were **all caught on the first run, each by a check failing on its own** (none `HUNG`, none `BUILD`). The breaks of `prerender.json` (P1a) were not run again: their code is unchanged except where P1b added to it. No new refusal tag: a hydrated page that does not match its script is not a refusal but a console line.

### Not measured

A page whose script is old and whose HTML is new at once through a service worker (the offline cache is versioned by one hash over all files, so they change together; the skew check is for a server that serves them from two deploys); a long page's cost of finding the markers (one `TreeWalker` pass, not timed); Safari and Firefox; a screen reader on a page that is hydrated while it is read.

## 12. `Table` (added 2026-10-26)

Found by the port of the cancho-www page ([`www-port/README.md`](www-port/README.md), open defect 1): a `Table` was always a hole, so a page whose comparison is a table had none without script. **Now a `Table` is static when its box, its rows and
its cells are**: `div.cui-table-scroll` (focusable, a named `region` when the table has a name) `> table.cui-table > tbody > tr > th[scope=col] / td`, from `recipe::table_scroll` and `recipe::shell` (the script's emitter writes the box
and the cells from the same functions). A `For` or an `If` among the rows or the cells, a cell that reads state, or a name that is an expression makes **the whole table a hole**, as a sentence is: a table with half its rows is a wrong table.
A table whose rows a `For` over a data file wrote out is static, because the expansion happens before (docs/data.md). **Measured** (Chromium): the corpus has 5 tables (literals, no name, a `For`, a cell that reads state, a name read from state), each equal in the page's HTML and in the script's DOM and, without script, the static part
(`check-structure.js` G); `examples/port` has a table of the data's claims and `check-port.js` reads it with script off; the port's comparison is a table in the accessibility tree with script off (`check-www-port.js`). `scripts/breaks/table-prerender.json`: 8 breaks, each caught by a check failing on its own.
