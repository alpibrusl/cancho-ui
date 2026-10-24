# Structure: headings, sections, landmarks, links, details, figures, and the roles of a text

> **Status: B1 of step 2 of [`www.md`](www.md), built.** Designed in `www.md` 5.2 and 5.2b (which three real sections of the cancho-www home page were sketched in, and what the proposal
> could not say), decided by the user on 2026-10-24 (`www.md` section 10 items 9 and 10). `Rich` and `Code` are B2; `Image`, `Media`, `ratio` and `Grid` weights are B3; prerendering is P1.
> Measured in Chromium (Playwright, `Pixel 7` profile, desktop Chromium, not a device) and in `cargo test`; numbers say which.

## 1. What a program can say

```
Header { Link { to: "#top" "cancho" }  Nav { label: "Sections" Link { to: "#try" "Try it" } } }
Main {
    Section {
        id: "try"
        Text { style: "eyebrow" "Try it" }
        Heading { level: 2 "Three ways in" }
        Article { Heading { level: 3 "See a refusal" } Text { "..." } }
        Details { summary: "Build instructions" Text { "..." } }
        Figure { Heading { level: 3 "A leak, a rule, a fix" } Text { "(the recording)" } Caption { id: "cap1" "One minute." } }
        Row { Link { to: "https://alpibru.com/manifesto" appearance: "primary" "Read the manifesto" } }
    }
}
Footer { Link { to: "agents.md" "agents.md" } }
```

| element | becomes | takes (beside `label:` on the containers) |
|---|---|---|
| `Heading` | `<h1>` to `<h4>` | `level:` (1 to 4, required), `id:`; its text is its name |
| `Section`, `Article`, `Figure`, `Header`, `Nav`, `Main`, `Footer`, `Aside` | `<section>`, `<article>`, `<figure>`, `<header>`, `<nav>`, `<main>`, `<footer>`, `<aside>` | `id:` |
| `Link` | `<a href>` | `to:` (a string literal, required), `appearance:` (`"primary"`, `"secondary"`, `"quiet"`); its text is its name |
| `Details` | `<details>` with a `<summary>` first | `summary:` (required), `id:` |
| `Caption` | `<figcaption>` | `id:` |
| `Text { style: .. }` | `<p data-style="..">` (a `Text` without `style:` is still a `<span>`) | `style:` (`"eyebrow"`, `"lead"`, `"note"`, `"label"`) |

* **Accessibility keys come first, as everywhere in the language**: `Section { label: "Try it" id: "try" .. }`, not the other way round (a `label:` after a binding is a parse error).
* A heading, a link, a caption and a details are named by their text: `label:` and `role:` on them are `binding-unknown`. A landmark takes `label:` (an `aria-label`) and no `role:`.
* **`Link` text is written between the braces**, with `{n}` interpolation like a `Text`; a `Heading` and a `Caption` too. They are written as text directly inside the tag: `<h2>Step 0</h2>`, not a `<span>` in it
  (found by the first Chromium run: the text of a `Heading` was dropped until `holds_text` included the new elements).
* The names of the new elements are now the built-ins': a program that declares its own `component Link` and uses `Link { .. }` gets the primitive (a test), as `Table` did.
* Nothing here has behaviour of its own except the browser's: a `Details` opens and closes by itself (it is not program state), and a link goes where it goes. Replay has nothing to
  check beyond the lowering and the refusals; `examples/structure` replays its trace so that the handlers around the structure still run (4 steps, 9 checks).

## 2. The rules

Every refusal below has a rule tag in [`spec/diagnostics/README.md`](../spec/diagnostics/README.md), reached by a case of `tests/structure_corpus.json` (62 refusals).

| tag | refused |
|---|---|
| `heading-level-invalid` | no `level:`, or not an integer literal from 1 to 4 |
| `heading-level-skipped` | a level more than one deeper than the previous heading **of the same component's view**; an instance of another component resets the order (this view cannot see its headings) |
| `heading-empty` | no text, or only spaces |
| `id-invalid` | an `id:` that is not a letter then letters, digits, `-`, `_` (at most 64), or starts with `cui-` |
| `anchor-duplicate` | one `id:` on two elements of the program, or an `id:` inside a `For` |
| `anchor-unknown` | `Link { to: "#x" }` and no element has `id: "x"` |
| `link-scheme-refused` | `javascript:`, `data:`, `vbscript:`, `file:`, `ftp:`, any other scheme; `//host`; a space or control character; a user name before the host; no host; a `mailto:` with no `@`; an empty or bare `#`; a `#/route` (that is a `NavLink`) |
| `link-to-invalid` | no `to:`, or one that is not a string literal |
| `link-label-missing`, `link-label-generic` | no text; or "here", "click here", "click", "link", "this link", "this", "more", "read more", "learn more" |
| `link-appearance-invalid`, `text-style-invalid`, `details-incomplete` | a value outside its closed set; no or empty `summary:` |
| `caption-misplaced` | a `Caption` that is not a direct child of a `Figure`, or a second one |
| `landmark-duplicate`, `nav-label-missing` | a second `Main`; with more than one `Nav`, one without `label:` |

**Corrections to `www.md` 5.2, measured in 5.2b and built here:** there is no `section-label-missing` (the page has 5 landmarks and 0 regions: an unnamed `Section` is not a landmark); a `Link` has no `rel`
(128 external links, 0 `target`); and `heading-level-skipped` would refuse the real page, whose first three cards are `h3`s under the `h1`.

**Not built here, and said so:** the skip link that a `Main` was to write (a page-level element: step 3, 5.3), `sticky`, `pressed:`, `role: group` (the human/agent switch needs it: to be checked then), dynamic
links built from data (`link-to-invalid` says so), and `landmark-misplaced` (a `Main` inside an `Article` is not refused).

## 3. Authority and `diff`

* A `Link` is **not an effect**: `requires` is unchanged, the program needs no capability to put a link in a page, and `authority` in `diff` is unchanged (a test: adding two links gives `authority: unchanged`).
* The hosts a link names are listed because they are where a reader can be sent: `cancho-ui inspect effects <file> <Component>` has `"links": ["evil.example", "github.com", "mailto:example.org"]` (sorted, lower-cased,
  a port dropped, `mailto:` by its domain; absent when there are none), and `diff` prints `+ link evil.example` / `- link ..` as lines of their own, counts them in `change_count` (exit 5), and has a `links` delta in the JSON
  of a changed component (absent when equal, so no existing report changes). A relative link and a `#id` have no host and are not listed.
* A user name before the host (`https://bank.example@evil.example/`) is `link-scheme-refused`, so the host that is listed is the host a browser goes to.

## 4. The DOM, and why the recipe is in the IR crate

The tag, the attributes and the class of each element come from `cancho_ui_ir::structure` (`tag`, `attrs`, `class`, `summary`), which the JavaScript emitter reads. The prerenderer ([`www.md`](www.md) 5.2c,
P1) will read the same functions, so the two renderers cannot disagree about what a `Link` is. `tests/structure_corpus.json` holds, beside the refusals, **8 views and the DOM each must become**, serialised the way
the page serialises it (tags lower case, attributes sorted, double quotes, no whitespace between tags); `scripts/check-structure.js` builds each, loads it in Chromium and compares: 8 of 8 equal. The prerenderer must pass
the same 8.

The baseline stylesheet gives them a look and no more (`crates/cancho-ui/src/pwa.rs`): a link in a `header`, a `nav` or a `footer` is an inline-flex target at least 44 px high; the three appearances are at least 44 x 44
(`LinkText`/`Canvas` system colours, so no hue is chosen for a program that has no theme); a `summary` is 44 px; a styled `Text` has a size and a case, no colour of its own except the eyebrow's. **With a theme** the
appearances and roles take its colours: `primary` is `accent_fg` on `accent`, `secondary` and the eyebrow `accent`, `lead`, `note` and `quiet` `muted`, all pairs the theme already checks at 4.5:1.

## 5. Measured

* **The corpus**: 8 of 8 valid views are the DOM the corpus says; 62 refusals reach their tags; `check-structure.js` (5-minute watchdog) passes: A the 8 DOMs; B the example through the accessibility tree: one
  `banner`, one `main`, one `contentinfo`, one named `navigation`, one named `complementary`, **no `region`**, 6 headings in order with no skipped level, 8 links all named and none generic, the details closed with its text
  hidden, the figure with its caption, a link to `#demo` moving the page there, every link of a header, nav or footer and every appearance at least 44 px high (and 44 wide), Tab reaching a link that has a focus ring;
  C a routed screen's `Heading` takes the focus as a heading text does (the selector of `screen-focus.md` now reads `h1` to `h4` as well as `role="heading"`; a break that restores the old one is caught); D a themed page draws
  each appearance and role at 4.5:1 or more in both schemes (lowest 5.93:1).
* **The nesting bound of 32 applies**: one bound over view nodes, expressions and types, and the text of a heading is an expression of its own, so `Heading` with a text is 2 deep and `Details { Text { .. } }` is 3;
  `32 - d` sections around an innermost of depth `d` checks and one more is `parse-nesting-too-deep`, tested for a `Heading`, a `Link`, a `Details`, a `Figure` with a `Caption` and an `Article`.
* **Breaks**: `scripts/breaks/structure.json`, 21, each caught by a check failing on its own (none `HUNG`, none `BUILD`): a level 1 heading written as an h2, a nav as a div, a link with no `href`, an appearance with no
  class, a details with no summary, a skipped level allowed, any scheme accepted, no generic words, an anchor to nothing, a repeated id, two mains, two unnamed navs, a user name before the host, a host not lower-cased,
  a link not counted by `diff`, a small target in a header, a new screen not looking for an `h2`, a styled text as a span, the compiler's `cui-` ids, an empty summary, a caption anywhere. The first run missed 3 and
  had one `BAD`: `diff` was caught by an exit code another change already gave (the test now changes only a host: 4 changes, 2 if links are not counted), the screen-focus break was equivalent for a heading that is a
  direct child (the check now puts the heading inside a `Section`, so only the selector finds it), and a `from` text had changed with the formatter.
* **Garbage**: 13 malformed views (unterminated, repeated keys, a NUL in an address, a level of 2^63-1 or -1, a `Caption` in a `Caption`) are refusals and not panics.

## 6. Not measured

Safari, Firefox, a real phone; a screen reader reading any of it (the accessibility tree is Chromium's); the look of the elements beside the cancho-www page's own (no pixel comparison); the byte cost of the new
stylesheet rules on a large page; whether a `Link { appearance }` reads as a button to a user of the page (it is an `<a>`, and says so in the tree: `link`). `Rich` (links inside a sentence) is B2, so a paragraph with an
inline link cannot be written yet: 82 of the page's 162 text blocks wait for it.
