# Structure: headings, sections, landmarks, links, details, figures, and the roles of a text

> **Status: B1, B2 and B3 of step 2 of [`www.md`](www.md), built** (sections 7 and 8 are B2: `Rich`, its inline children, `Summary`, `Code`, `Clipboard`; section 10 is B3: `Image`, `Media`, `ratio`, `Grid` and its `weights`). Designed in `www.md` 5.2 and 5.2b (which three real sections of the cancho-www home page were sketched in, and what the proposal
> could not say), decided by the user on 2026-10-24 (`www.md` section 10 items 9 and 10). Prerendering is P1.
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

**Not built in B1, and said so** (the skip link and `sticky` are built since, in [`page.md`](page.md)): `pressed:`, `role: group` (the human/agent switch needs it: to be checked then), dynamic
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

## 7. `Rich`: inline content (B2)

```
Rich { "A program opens a file. " Strong { "What this shows:" } " " Code { "check" } " found it. See " Link { to: "#docs" "the docs" } "." Break { } "Next." }
Heading { level: 1 "Tools built for agents." Break { } Em { "Not adapted from humans." } }
Figure { Caption { "20 seconds. " Link { to: "https://github.com/alpibrusl/cancho-ui" "cancho-ui" } "." } }
Details { Summary { "The same session as text " Small { "(a real run)" } } Code { "$ cancho check leak.cho" } }
```

| element | becomes | holds |
|---|---|---|
| `Rich { style: .. }` | `<p>` (`data-style` as a styled `Text`) | text and inline elements; `style:` is `"eyebrow"`, `"lead"`, `"note"`, `"label"` |
| `Strong`, `Em`, `Small` | `<strong>`, `<em>`, `<small>` | text and inline elements |
| `Code` inside inline content | `<code>` | text only |
| `Break` | `<br>` | nothing |
| `Link` (inside any of them) | `<a href>` | text and `Strong`, `Em`, `Small`, `Break`, `Code`; **not another `Link`** |
| `Summary` (first child of a `Details`, instead of `summary:`) | `<summary>` | the same inline content |

* **Inline content is also what a `Heading`, a `Caption` and a `Link` hold**: they take text and the same inline elements (the real page has a `<br>` and an `<em>` in its `h1`, `<small>` in a `summary`, links in both
  `figcaption`s: 5.2b item 6). A `Text { .. }` or any block element inside them is `rich-child-invalid` (B1 let a `Heading` hold a `Text` by accident).
* Text goes into the element as **text nodes between the inline elements** (`<p>A <strong>b</strong> c</p>`), not in a `<span>` each. A `For` or an `If` inside inline content is allowed and draws its text in a
  `<span>` per item, which a prerenderer would have to reproduce.
* **The same name, two meanings, by place**: `Code { .. }` directly inside inline content is the `<code>` of a sentence (`InlineCode` in the IR, decided when the view is lowered); anywhere else it is the box of
  section 8. `Strong`, `Em`, `Small` and `Break` anywhere else are `inline-outside-rich`.
* **`Span` was not added**, though the decision listed it: a `<span>` with no attribute says nothing. Measured on the live page: 85 `span`s, **42 of them inside text blocks, and every one of those carries a class**
  (`n`, `chip built`, ..: the numbers and the stage chips of the generated lists), none is a bare span; the three sections measured in 5.2b have none. What those 42 need is a styled inline element (a chip), which is
  not in the closed set and which a `Span` would not give. Whether the port (www.md Q11) needs a `Chip` is left to the port, which will say so by its parity report.
* **`Strong` is `<strong>`**, and the page writes `<b>` (16 times, mostly a name: `<b>cancho</b>`). Both are bold; a screen reader reads them alike; `<strong>` says importance. A parity check against the live page will
  list the 16 as a difference rather than hide it.
* Tags: `rich-child-invalid`, `rich-empty`, `inline-outside-rich`, `code-child-invalid`; a `Summary` outside a `Details`, or beside `summary:`, or not first, is `details-incomplete`.

## 8. `Code`, and the `Clipboard` capability (B2)

```
component Install {
    requires Clipboard;
    view { Code { copy: true max_height: 28 "git clone https://github.com/alpibrusl/cancho\ncargo build -p cancho --release" } }
}
```

* **The DOM**: `<div class="cui-code" data-copy=""><pre tabindex="0" style="max-height: 28rem;"><code>..</code></pre><button type="button" class="cui-copy" aria-label="Copy the code">Copy</button></div>`. The `<pre>` is
  focusable (`tabindex="0"`) so a keyboard user can scroll it; without `copy:` there is no `data-copy` and no button; without `max_height:` no `style`. `max_height:` is an integer from 4 to 100 (rem):
  `code-max-height-invalid`; `copy:` is `true` or `false`: `code-copy-invalid`; the box holds text only: `code-child-invalid`. A brace in the text is written `\{`, as in a `Text`.
* **`copy: true` is the `Clipboard` capability**: the component declares `requires Clipboard;` (`clipboard-not-declared` if it does not; the existing `capability-unused` if it declares it and has no copy button).
  It is **write only**, and only from a tap by construction: the compiler writes the one call to `navigator.clipboard.writeText` inside the click of the button it draws, and no other code touches the clipboard.
  `inspect effects` lists `Clipboard` under `requires` and `diff` adds it as `+ requires Clipboard` with `authority: expanded` (tests).
* The tap writes the text **as shown** (`pre.textContent`), says "Copied" for 1.5 s, and where the write is refused ("Select and copy") or the clipboard is absent ("Press Ctrl+C") it selects the box so that the keyboard or
  the long-press menu finishes the job. The cancho-www page strips trailing `# comments` from what it copies; that is the page's own rule and is not reproduced: a parity check will list it.
* **Measured** (Chromium, `Pixel 7` profile, clipboard permission granted): nothing writes to the clipboard at load or at a render; a tap writes the text of the box once and reading it back gives the same text; the page
  calls `readText` 0 times; with `writeText` rejecting, the label is "Select and copy" and the text is selected; with `navigator.clipboard` undefined, "Press Ctrl+C" and the text is selected; the button is at least
  44 x 44 and named. The emitted module of a program with no copy button has no clipboard code at all, and one with it has one `writeText(` (tests).
* **Looks**: the box is dark in both schemes, `#12151a` with `#d6dde8` (13.39:1) by default and `code_bg`/`code_fg` with a theme (drawn at 13.39:1 in both schemes of the site's palette); the copy button keeps the
  baseline's 44 px target and sits in the corner; `pre` has room for it (`padding-right`).
* **Not measured**: a real phone's long-press menu; iOS Safari's rule that `writeText` needs the tap itself (the call is inside the click handler, which satisfies it, but no iPhone ran it); a screen reader announcing
  "Copied" (the label changes text, with no live region: a person who cannot see it is not told: **a gap**, noted for a later pass).

## 9. Checked (B2)

* `tests/structure_corpus.json` now has 18 valid views with the DOM each must become (10 added for B2, all equal in Chromium) and 95 refusals (33 added; every tag of this section is reached); 7 new rule tags in
  `spec/diagnostics/README.md`.
* `scripts/check-structure.js` part E: the clipboard behaviour above; part D adds the code box (13.39:1 in both schemes with the site's palette).
* `scripts/breaks/structure-b2.json`: 20 deliberate breaks, **each caught by a check failing on its own, on the first run** (none `HUNG`, none `BUILD`): a `Rich` as a `div`, a `Strong` as a `b`, a `Break` as an `hr`,
  inline code as a box, text in a `Rich` in a `span` each, a `Link` in a `Link`, a `Text` in a `Rich`, an empty `Rich`, a `Strong` anywhere, any code height, a copy button with no `Clipboard`, a `Clipboard` unused,
  the page writing at once instead of on a tap, the page also reading the clipboard, copying more than is shown, a code box with no `tabindex`, a height in px, the copy code left out, a `Summary` anywhere, a `Code`
  that holds elements. The 21 breaks of B1 were run again on this branch: all caught.
* Gate: fmt, clippy `-D warnings`, `cargo test --workspace` 623 passed, 0 failed, all 54 `scripts/check-*.js`, and both `--self-test` runs.

## 10. `Image`, `Media`, `ratio`, `Grid` (B3)

```
Grid {
    min: 17
    gap: 3
    Figure {
        Image { src: "assets/img/field.webp" fallback: "assets/img/field.jpg" alt: "A field of boulders." width: 1024 height: 768 ratio: "4:3" }
        Caption { "A field of boulders." }
    }
    Figure {
        Image { src: "assets/img/dawn.webp" fallback: "assets/img/dawn.jpg" decorative: true width: 1024 height: 768 ratio: "4:3" }
        Caption { "First light." }
    }
}
Grid { min: 19 weights: [1, 1.75] gap: 3 Column { Text { "the text" } } Column { Code { "the code" } } }
Figure {
    Heading { level: 3 "cancho-ui: a component declares what it may do" }
    Media {
        label: "cancho-ui demo"
        kind: "video"
        src: ["assets/media/ui-demo.mp4", "assets/media/ui-demo.webm"]
        poster: "assets/media/ui-demo.poster.jpg"
        ratio: "16:9"
        caption: "cap2"
        transcript: "demo-text"
    }
    Caption { id: "cap2" "20 seconds: real compiler output beside the compiled counter running." }
}
Details { id: "demo-text" summary: "The same session as text" Code { "$ cancho-ui check counter.cui" } }
```

### 10.1 Corrections to `www.md` 5.2, made there too

The design had four things that the build showed to be wrong or unfinished; each is corrected where it was said (`www.md` section 5.2 and table 5.2b row 4):

* `fallback: ".jpg"` (a suffix) is **`fallback: "assets/img/field.jpg"`, a path**: the compiler has to find the file to check it exists, to hash it, and to copy it, and a suffix is not a file.
* `alt-source:` does not exist. A video's text alternative is **`caption:`** (the `id` of a `Caption`, which becomes `aria-describedby`) and/or **`transcript:`** (the `id` of any element with an `id:`, which becomes
  `aria-details`), at least one: `media-text-alternative-missing`. Both are text literals naming an id of the program, not bare names.
* A `Media` also has a required static **`label:`**, which the design did not have: it is the accessible name of the player and the text of the one link inside `<video>` (see 10.3). Without it a video is named by
  nothing; `media-label-missing`.
* The weighted grid's `min:` is not the `min` of the equal one: with `weights: [1, 1.75]` it is the width of the *weight 1* column, so the line needs `Σ weights x min` rem. 5.2b row 4 guessed `min: 25` from
  arithmetic on equal columns and said so (not measured); the measurement (10.4) is **`min: 19`**.

### 10.2 `Image`

* **The DOM** (one recipe, `cancho_ui_ir::media::image_dom`; the prerenderer will read the same function):
  `<picture class="cui-img" data-ratio="4:3" style="max-width: 1024px;"><source srcset="assets/img/field.webp" type="image/webp"><img src="assets/img/field.jpg" width="1024" height="768" alt=".." loading="lazy" decoding="async"></picture>`.
  `<source>` only when there is a `fallback:`; `data-ratio` only with a `ratio:`; `loading="eager" fetchpriority="high"` instead of `lazy` with `priority: true`.
  The page's own markup is the same (`<picture><source type="image/webp" srcset=..><img src=.jpg width height alt loading="lazy" decoding="async"></picture>`, 5 of them); the `class` and the `style` are ours.
* **`alt` or `decorative: true`, never neither, never both**: `image-alt-missing` (neither, an `alt` of spaces, `decorative: false`), `image-alt-conflict`. A decorative image is `alt=""`, which is how
  Chromium leaves it out of the accessibility tree (measured). `width` and `height` are required integers from 1 to 10000: `image-size-missing`, `image-size-invalid`. `ratio:` is one of `1:1 4:3 3:2 16:9 21:9 3:4 2:3`
  (`ratio-invalid`). `priority: true` is for the one picture the reader sees first (`image-priority-invalid` for anything but `true`/`false`); every other image is lazy.
* **`src:` and `fallback:` are files of the site, written relative to the program**: letters, digits, `. _ - /`, no `..`, no empty segment, no leading `/`, at most 200 characters, in png, jpg, jpeg, webp, avif
  or gif (**not SVG**, which can carry script), and the fallback is another file in another format (`image-src-invalid`). An address of another host is refused with the reason: a page makes no request to another host.
  A `src:` that is a state field is a parse error: the value is a literal (a picture chosen by data is not offered).
* **Looks**: the picture is a block, as wide as its column and never wider than the file (`max-width` is the `width:`); with a `ratio:` it is cropped to the shape (`aspect-ratio` on the `picture`, `object-fit: cover`
  on the `img`), with rounded corners and no other chrome. The theme does not touch it.
* **No layout shift** (measured): with a picture that arrives 1.2 s late the box is the same height before and after (`width` and `height` give the browser the aspect ratio before the file is there).

### 10.3 `Media`

* **The DOM**: `<video class="cui-media" data-ratio="16:9" aria-label="cancho-ui demo" controls playsinline preload="metadata" poster=".." aria-describedby="cap2" aria-details="demo-text"><source src=.. type="video/mp4"><source src=.. type="video/webm"><a href="first source">cancho-ui demo</a></video>`.
  `controls`, `playsinline` and `preload="metadata"` are always there; **there is no way to write `autoplay`, and no key for it** (a key that does not exist is not a rule: `binding-unknown`), and a video the program does not
  start does not start (measured: after its metadata loads, `paused` and `currentTime` 0). `kind:` is `"video"` (`media-kind-invalid`; audio is not offered). `src:` is one text or a list of one to four files
  in mp4 or webm, no repeats (`media-src-invalid`); `poster:` an image of the site (`media-poster-invalid`); `ratio:` as for an image.
* **The text alternative is required**: `media-text-alternative-missing`. `caption:` must be the `id` of a `Caption` of the program, `transcript:` the `id` of any element with an `id:` of it, both checked over
  the whole program (`media-reference-invalid`: no such id; or a caption that names a `Details`). The `Caption` stays a child of its `Figure`, where `caption-misplaced` already holds it.
* The `<a>` inside `<video>` is the fallback for a browser that cannot play video at all; it names the video and leads to the first file. **Not measured** in a browser without video support (none was available).
* Offline: `--pwa` keeps the pictures and the poster in the cache and **not the videos** (10.5).

### 10.4 `Grid`, measured against the cancho-www page

`Grid { min: M }` is `display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, Mrem), 1fr))` (`min:` an integer from 8 to 60, `grid-min-invalid`; `gap:` 0 to 4 as on a `Row`). With
`weights: [..]` (two to four numbers from 1 to 4 with at most two decimals, **one for each child, and the children written out**: no `For`, no `If`: `grid-weights-invalid`) it is a wrapping flex line whose cell *i* has
`flex: w_i 1 min(100%, w_i x M rem)`: when the line holds `Σ w_i x M` rem and the gaps the cells share the width in proportion to the weights, and when it does not they stack, one under another, each the full width.

Measured in Chromium (`Pixel 7` profile) by `scripts/measure-www-grid.js` on the cancho-www clone at `dbbf7ae`: for 16 viewport widths from 320 to 1440 the page's grid is measured (the width of its content box, its column
gap, its columns) and ours is put in a box of that width with that gap:

| the page | ours | 16 widths |
|---|---|---|
| `.videos`: `repeat(auto-fit, minmax(22rem, 1fr))` and a 30 rem rule | `Grid { min: 22 }` | **same column count at 16 of 16; column widths equal to 0.0 px** |
| `.gallery`: `repeat(auto-fit, minmax(17rem, 1fr))` | `Grid { min: 17 }` | **16 of 16; 0.0 px** |
| `.try article`: `1fr 1.75fr` from 60 rem (a viewport rule) | `Grid { min: 19 weights: [1, 1.75] }` | **14 of 16 the same count, 0.1 px at most in a width** (308 + 539 at 960, 331 + 579 at 1024, 378 + 661 at 1280 and 1440, the page's numbers); at 950 to 959 ours is two columns where the page is still one |

So the weighted grid reproduces the page's `.try` columns to a tenth of a pixel, and **its breakpoint is 10 px early** (viewport 950 for 960): the page changes at the viewport, `60rem`; ours changes when the
grid is `2.75 x 19 rem + gap` wide, and `min` is a whole number of rem (`min: 20` would be 34 px late, at 994). The two were not made to agree by adding a viewport rule, because a `Grid` has none; the difference is reported,
not hidden. Other differences, known: one `gap` for both directions (the page's `.try article` has a row gap of 1.1rem and a column gap of 1.8rem); the page's `.videos` has its own rule below 30 rem that `min(100%, ..)`
makes unnecessary (same widths at 320 to 480 in the table).

Also measured by `check-structure.js` part F against a formula written in the check (not the CSS's own algorithm): the number of columns of a `Grid { min: 17 }` of five items and of a `Grid { min: 22 }` of two
(the row is filled: empty tracks collapse) at 12 widths from 320 to 1440; the weighted grid is 1 : 1.75 within 1.75 px side by side and stacked full width below `Σ w x min` plus the gap, and **changes at exactly the width
the formula says (within 1 px)** found by bisection over the viewport; three weights `[1, 2, 4]` are 1 : 2 : 4 when they fit on one line; at 320 px nothing scrolls sideways. **Three or four weights between "all on a
line" and "stacked" wrap as flex does (two and one, say), which `auto-fit` also does** and no check pins; the page's six asymmetric grids with a breakpoint at a viewport width are for the cases of 5.3.

### 10.5 Files: the asset pipeline

* `check` and `build` read every file the views name, once: it is a file **inside the directory of the program** (a symbolic link out of it is `asset-missing`: the real path is what is compared), at most **20 MB**
  (`asset-too-large`), and begins as its extension says (`asset-type-mismatch`: the first bytes of png, jpg, gif, webp, avif, mp4 and webm; a file only renamed is caught, a corrupt one that begins correctly is not).
  `asset-missing` also covers a directory named like a file and an unreadable file. Paths are relative to the **entry file's** directory, also for a component imported from another file.
* The SHA-256 and size of each file are in the IR (`assets`, only when there are some) and `diff` reports `~ file assets/a.webp: <12 digits> -> <12 digits>`, `+ file`, `- file`, in JSON `assets: [{path, base, candidate}]`
  (absent when none changed); a changed file is a change of the program (`change_count`) and is not a change of authority. Replacing a picture with another of the same name is therefore a line in review.
* `build --page` and `--pwa` copy the files the views name, at the same paths, into the output (a file no view names is not copied). `--pwa` puts the pictures and posters in the offline cache and **not the videos**
  (a 20 MB video would be most of the install), and the cache name still moves when a video changes (tests).
* **Cost**: `check` reads and hashes what it names every time: a 20 MB mp4 takes 2.0 to 2.2 s in the debug binary (three runs, this machine); a release build was not measured. The cancho-www home page names (`ls`) 8 picture files (924 kB), 2 posters (187 and 58 kB) and 4 video files (124 to 617 kB, 1,443 kB together): none near the bound, and 2.6 MB of files for `check` to read.
* **Authority**: none. A picture or a video is a file of the site, requested by the page of its own origin; no `Network` is declared or needed. The built module of the example has no `fetch(` and no address of another
  host; a Chromium run of part F records every request of the page and finds none to another origin. `inspect effects` and `diff` show `authority: unchanged` for a program that gains pictures, video and grids (tests).

### 10.6 Checked (B3)

* `tests/structure_corpus.json`: 29 valid views (11 added) with the DOM each must become, all equal in Chromium; 166 refusals (71 added); **20 new rule tags**, in `spec/diagnostics/README.md`, each reached:
  `image-src-invalid`, `image-alt-missing`, `image-alt-conflict`, `image-size-missing`, `image-size-invalid`, `image-child-invalid`, `image-priority-invalid`, `ratio-invalid`, `media-kind-invalid`,
  `media-src-invalid`, `media-poster-invalid`, `media-label-missing`, `media-text-alternative-missing`, `media-reference-invalid`, `media-child-invalid`, `grid-min-invalid`, `grid-weights-invalid`,
  `asset-missing`, `asset-too-large`, `asset-type-mismatch` (and `layout-value-invalid` now covers a `gap:` on a `Grid`). The corpus names the files it needs (`assets`); both runners write them (the first bytes of each
  format; a sparse file just over 20 MB; a text file named `.png`), and Chromium gets a PNG written by the check and a 658-byte VP8 file, `tests/fixtures/tiny.webm`.
* `cargo test --test structure` 13 tests, 6 added: authority unchanged; a changed file in `diff`; a build copies the files and `--pwa` does not keep videos; a link out of the directory; odd files (a directory named
  like a file, an empty file); the message of a refusal says what to change; 20 more malformed views are refusals and not panics; `Grid` at the nesting bound (3 deep).
* `scripts/check-structure.js` part F (Chromium): everything listed in 10.2 to 10.4 and: the accessibility tree names a picture by its `alt`, leaves the decorative one out, and names a video by its `label` with the
  caption as its description (**on a clip that plays**: a video whose file cannot load is named "Unable to play media." by Chromium, measured, and the check says so rather than assert on a broken file).
* `scripts/breaks/structure-b3.json`: 49 deliberate breaks (a priority image lazy, no size attributes, alt on a decorative image, a picture wider than its file, a webp declared png, no async decoding, autoplay, preload all, no `playsinline`, an unnamed video, a caption described by the wrong attribute, a transcript not linked, the fallback link to the last source, no class for weights, a minimum in px, weights dropped, a Grid gap skipped, a picture needing no alt, alt and decorative together, optional sizes, an svg, a path going up, a path absolute or with an empty segment, the general message for another host, any ratio, any kind, eight sources, a repeated source, no text alternative, no label, any minimum, any weight, the wrong number of children, a caption that is not a `Caption`, a reference to nothing, a Grid gap refused, a png that is any file, any file size, a link out of the directory, a changed file not a change, a video kept offline, no files copied, an Image lowered as a Media, the second weight not growing, `auto-fill`, a column wider than the screen, a stretched crop, 16:10, an image at its file width). **48 were caught on the first run, each by a check failing on its own (none `HUNG`, none `BUILD`); one was missed**: removing the empty-segment test changed nothing because the leading-`/` test refused the same inputs first, so no case reached it. The corpus now has `assets//a.webp`, and the break is caught by `cargo test structure`.
* Gate: fmt, clippy `-D warnings`, `cargo test --workspace` 629 passed, 0 failed (before the last corpus case), all 54 `scripts/check-*.js` and both `--self-test` runs pass. A first run of the scripts failed 38 of them with `ERR_INSUFFICIENT_RESOURCES` and `Page crashed`: the machine's `/tmp` held 125,660 entries of `cancho-ui-*` scratch directories that the existing tests (`routing`, `stream`, `file`, ..) have never removed; after removing them every script passed. That is an environment fault, not a result, and is noted so that nobody reads the first run as one.

### 10.7 Not measured, and gaps

* Safari, Firefox, a phone, a screen reader; a video in a browser without video support; a corrupt file that begins correctly; `avif` in any browser (the file is checked for its first bytes only; its `type` is declared);
  the pixel look of a picture beside the page's (no pixel diff yet: the parity check of the port); a release-build cost of hashing; a video's `poster` over a slow link; **hover zoom on a gallery picture** (a motion
  effect, step 5: not expressible).
* **A gap found in B2 and not yet closed**: the label of the copy button changes to "Copied" with no live region, so a person who cannot see it is not told (`structure.md` section 8 says so; it stays for a later pass
  and is not part of B3).
