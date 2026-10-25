# The home page of cancho, written in cancho-ui: an inventory, a map, and an order

> **Status: DESIGN; step 1 (the theme) is built, nothing else is** *(updated 2026-10-24: [`theme.md`](theme.md))*. Written 2026-10-24, before any code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). The goal, set by the
> user: evolve cancho-ui until the home page in `alpibrusl/cancho-www` can be written in it. This note inventories that page, maps every part of it
> to what cancho-ui does today, to a new primitive, or to a declared plain-JS island, and proposes the order in which to build. **Nothing here is a
> commitment to build a step**: each step is its own draft PR after the user confirms the order. Every number is measured and says how; what could
> not be measured is in section 8.

## 1. What was measured, and how

`https://github.com/alpibrusl/cancho-www`, shallow clone, **read-only** (this session has no write access to it and none is wanted), commit
`0579a80` ("Map: a walk to each rock, with a recorded session replayed beside it (#12)"). cancho-ui at main `01d919e`. Counts come from
`wc`, regular expressions over the files, and Chromium (Playwright) loading the page from a local static server: desktop 1280 px, the `Pixel 7`
profile, `reducedMotion: 'reduce'`, and JavaScript disabled. **Desktop Chromium on Linux only**: no other browser, no real phone, fonts rendered by
this machine's Chromium (section 8).

The page as served: **0 requests to another host and 0 console errors** in all four runs (20 requests with JavaScript, 15 without), load plus 1.5 s
in about 2 s on this machine.

## 2. The inventory

### 2.1 Files

| file | lines | bytes | what it is |
|---|---:|---:|---|
| `index.html` | 564 | 49,368 | the page; 4 regions between `<!-- name:start -->` / `<!-- name:end -->` markers are **written by `scripts/gen_agent_files.py`** from `data/ecosystem.json` |
| `assets/site.css` | 494 | 43,597 | 453 rules, 475 selectors, 1,498 declarations, 41 custom properties |
| `assets/site.js` | 261 | 16,492 | hero, cairns, authority animation, proof waffles and count-ups, reveal, agent/human switch, copy buttons, data fetch |
| `assets/map.js` | 287 | 20,253 | the ecosystem map: layout, filters, detail pane, the "walk" with a recorded session |
| `assets/rocks.js` | 113 | 5,524 | seeded random numbers, a procedural granite boulder, ridges, shared SVG defs |
| `assets/scene.js` | 65 | 3,304 | the hero scene (sun, ridges, 26 pines, 8 boulders, shrubs) |
| `ui/agent-files.cui` + `assets/agent-files.js` | 61 + 228 | 2,457 + 12,640 | **the one part already written in cancho-ui** (`requires Network;`); the JS is its compiled output |
| `data/ecosystem.json`, `data/demos.json` | 562, 47 | 23,489, 2,051 | 24 projects, 3 measured figures, `verifies` (3 lists of claims, a stamp and a disclaimer), 3 `paths`; the recorded sessions the map replays |
| fonts, images, video | | 142,684 (3 variable woff2); 4 jpg + 4 webp pairs; 2 mp4 + 2 webm + 2 posters | Plus Jakarta Sans, JetBrains Mono, Caveat (SIL OFL), self-hosted |

Hand-written JavaScript: **726 lines, 45,573 bytes** (site, map, rocks, scene), against 61 lines of cancho-ui for the pane that is already ported.

### 2.2 The HTML (`index.html`, counted by regular expression)

| category | measured |
|---|---|
| elements | 738 start tags (the same as the 738 DOM elements without JavaScript, 2.3); 164 `a`, 87 `div`, 85 `span`, 71 `p`, 68 `li`, 30 `code`, 22 `h3`, 12 `h2`, 12 `section`, 17 inline `svg` (39 drawing children), 6 `article`, 5 `button`, 5 `figure`/`figcaption`, 3 `details`/`summary`, 2 `video`, 4 `picture` + 5 `img`, 8 `pre`, 2 `aside`, 1 each of `header`, `nav`, `main`, `footer`, `h1`, `blockquote`, `ol` |
| links | 164: **128 external** (`https://`), 22 in-page anchors (`#…`, including the skip link), 14 relative files |
| **inline markup in prose** | **162 text blocks** (`p`, `li`, `h2`, `h3`, `figcaption`, `blockquote`, `summary`, `small`); **82 (51 %) contain inline markup**: 116 links, 22 `code`, 5 `b`, 1 `cite`. A paragraph with a link in the middle of a sentence is the commonest thing on the page |
| head | 13 `meta` (charset, viewport, description, 8 Open Graph, `twitter:card`, `color-scheme`), `canonical`, `icon`, `apple-touch-icon`, two `rel="alternate"` (the page as markdown and as JSON) |
| accessibility attributes | 30 `role`, 22 `aria-hidden`, 18 `aria-label`, 2 `aria-pressed`, 2 `aria-live`, 2 `aria-describedby`; 14 inline `style` attributes; 106 distinct classes; 40 ids |
| scripts | one inline (`<html class="js">`), five external (`rocks`, `scene`, `map`, `site`, and `agent-files` as a module) |
| text | 23,369 bytes outside tags |

### 2.3 What the page is, in the browser (Chromium, counted after load)

| | with JS | without JS |
|---|---:|---:|
| DOM elements | **2,951** | 738 |
| of which inside an `svg` | **2,189 (74 %)** | 39 |
| hero scene | 288 | 0 |
| ecosystem map | 426 | 0 |
| proof waffles (3 charts, 1,099 cells) | 1,128 in the whole proof section | 26 |
| authority animation | 117 section (80 drawn) | 29 |
| cairn dividers (10) | 210 | 0 |
| everything that is not SVG | 762 | 699 |
| running CSS animations after load | 30 of 33 | 14 |
| ... under `prefers-reduced-motion: reduce` | **0** | n/a |
| page height | 12,459 px at 1280 px; 22,092 px at 412 px | |

So three quarters of the DOM is generated drawing; the readable content (762 nodes, 16,006 characters of text without JS, 37,220 with) is plain HTML
that works without JavaScript. The page is usable with scripts off today (section text, links, `<details>`, videos); that is a property to keep. *(5.2c, 2026-10-24: no step of this plan keeps it; a cancho-ui page is empty without scripts.)*

### 2.4 The CSS (counted, comments removed)

| | measured |
|---|---|
| declarations by kind (approximate: a property is counted once, in the kind it mostly serves) | layout 507 (33 %), colour and surface 313 (20 %), SVG paint (`fill`, `stroke`, `opacity`, `filter`, `transform-origin`) 251 (16 %), type 208 (13 %), motion 45 (3 %), other 174 |
| custom properties | 41: **16 semantic** (`bg fg muted line surface soft ink accent accent-fg accent-soft good warn warn-bg warn-line bad bad-bg`), 3 type (`font mono hand`), and **22 for the art** (granite, ridges, sky, map paper, forest, river, road, trail, terminal) |
| dark mode | `@media (prefers-color-scheme: dark)` redefines the whole palette in `:root:not([data-theme="light"])` (a manual `data-theme="light"` escape exists; nothing sets it), plus 3 smaller dark blocks |
| `@media` | 27 blocks: 4 dark, 4 `prefers-reduced-motion`, 19 width queries (`min-width` 60, 62, 64 rem; `max-width` 30, 40, 46, 62, 63.99 rem) |
| layout | 46 `grid`, 48 `flex`, 18 `grid-template-columns`; 5 `clamp()`, 6 `calc()`, 3 `aspect-ratio`; 3 `position: sticky` (the header, the authority controls) |
| effects | 6 `color-mix()`, 3 `backdrop-filter`, 6 `filter`, 62 `transform`, 0 `mask`, 0 `@supports`, 0 `@container` |
| motion | **14 `@keyframes`** (`sunrise breathe drift glide settle glint dash ring blink warn lit flow flowv hk`), 24 `animation`, 21 `transition`; 4 reduced-motion blocks that switch them off |
| fonts | 3 `@font-face`, variable woff2, `font-display: swap`, self-hosted; `font-feature-settings: "ss01"` once |
| interaction | 14 `:hover`, 5 `:focus-visible` (a 3 px outline), `scroll-behavior: smooth`, `scroll-padding-top: 5rem` |
| state hooks | `.js` (set before first paint), `.reveal.in`, `[data-view="agent"]`, `aria-pressed`, `.sel`, `.dim` |

**Contrast of the palette, computed (WCAG 2.x relative luminance) for 15 foreground/background pairs in each scheme (13 of the 16 tokens, 2 of the art palette): all 30 are at least 4.5:1**
(the lowest is light `--good` on `--bg`, 4.75). This is the measured example for the
contrast check in section 4.1. It is the page's own claim made checkable, not a new finding about the page.

### 2.5 The JavaScript, by behaviour

`addEventListener` 15, `IntersectionObserver` 6, `requestAnimationFrame` 5, `setInterval` 1, `setTimeout` 5, `matchMedia` 4, `fetch` 3, `createElement` 23,
SVG `el(...)` 113, `getBoundingClientRect` 1, `history.replaceState` 1, `navigator.clipboard` 3, `getTotalLength` 1; one `innerHTML` (clearing). Events
handled: `click` 13, `pointermove` 1, `keydown` 1.

| behaviour | where | what it needs |
|---|---|---|
| progressive enhancement: `<html class="js">` | inline | a flag before first paint |
| **reveal on scroll** (14 `.reveal`, 10 cairns, the strike-throughs) | `observers()` | `IntersectionObserver`, a class once, no-op without it |
| **count-ups** (3 `data-count`), proof waffles lighting up | `proofAnim()` | `IntersectionObserver` + `requestAnimationFrame` easing over 1,300 ms; reduced motion writes the final number |
| **pointer parallax** on the hero | `hero()` | one `pointermove` listener, rAF-throttled, sets two CSS variables; off under reduced motion |
| hero scene, cairns | `scene.js`, `rocks.js` | seeded procedural drawing: `rng(seed)`, `rock()`, `ridge()`, shared gradients |
| **authority animation** | `authority()` | 3 scenarios, 4-5 steps each; a stepper (prev, play, next), tabs, a 2.6 s timer, auto-play when half visible; each step moves 5 rocks (CSS transitions on `transform`) |
| proof waffles | `waffle()` | 3 SVG grids from `data.measured.figures` (44 columns, 37 columns, 16 columns) |
| **ecosystem map** | `map.js` | parchment, forest blobs, river and road with `<textPath>` names, `feTurbulence` grain, a village, 24 rocks placed from a table of 24 spots, footpaths, 1 meaningful arrow type (`rests_on`), kind chips, "only what exists", count, detail pane, keyboard (`Enter`/`Space`) |
| **the walk** | `map.js` | a hiker follows a path (`getTotalLength`, rAF), the `viewBox` tweens (rAF, ease in-out), a recorded session types out (`setTimeout`), cancelled by a counter |
| **agent / human switch** | `agents()` | `data-view` on `body` hides and shows sections, `fetch('agents.md')` on first use, `history.replaceState('#agent')`, reads `location.hash` |
| agent-files pane | `agent-files.cui` | **cancho-ui** (3 buttons, `net.get`, a status line) |
| copy buttons (3 `data-copy`) | `copyButtons()` | `navigator.clipboard.writeText`, a fallback that selects the text |
| data | `Promise.all(fetch ecosystem.json, fetch demos.json)` | fills the map, the waffles, the counts, the date; on failure opens the `<details>` list |
| menu closing | 1 line | `details.menu.open = false` on a link click |

`data/ecosystem.json` is "the one source of truth" (the README says so): the map, the proof figures, the list under the map, `agents.md`, `llms.txt`
and the verification and path sections are all derived from it, three of them by a **Python generator at build time into `index.html`**.

## 3. What cancho-ui does today, measured against that

Elements today: `Column`, `Row`, `Text` (with `role`, `tone`), `Button`, `TextField`, `TextArea`, `Checkbox`, `Select`, `FileField`, `Chart`
(line, bar), `SortList`, `Map` (OSM tiles), `NavLink` (a **route** of the same page), `Table`; `For`, `if`; state, events, `net.get`/`net.post`
(own origin), `json.get`/`json.items`, `Storage`, `every`, `subscribe`, i18n; `build --page` writes `index.html` with `<title>` and one inline baseline
`<style>`; `gap`, `max_width`, `tone` are the whole styling vocabulary (`presentation.md`, #116).

**Spike, measured now:** a 16-line `.cui` (`net.get("data/ecosystem.json")`, `json.items(text, "projects")`, `json.get`, a `Table` with `For`) built with
`--page` and loaded in Chromium beside the site's real `ecosystem.json` shows **24 rows plus the header, "24 projects", first row "cancho (built)", 0
script errors**. So the *data-driven list* part of the page needs nothing new. And the compiler today accepts and replays the site's `ui/agent-files.cui`
unchanged (`check` exit 0, `test` PASS 7 steps, 17 checks). One cost: the compiler's output for those 61 lines is **21,897 bytes today** against the
**12,640 bytes** deployed (the deployed file was built by an older compiler, and the page is now built from a larger emitted runtime): a page of many
components would carry that runtime once, but the figure is worth watching (section 8).

## 4. The map: every part of the page, to (a), (b) or (c)

(a) exists today; (b) needs a new primitive; (c) stays a plain-JS **island**, declared (section 6). "Share" is of the 2,951 DOM nodes or, for CSS,
of the rules, so the order in section 7 can be argued from numbers.

| part of the page | measured weight | | where it goes |
|---|---|---|---|
| layout of sections: stacks, rows, grids, widths | 507 layout declarations, 46 `grid`, 48 `flex`, 19 width queries | (a) partly | `Column`/`Row`/`gap`/`max_width` exist; **responsive columns** (`1fr 1fr` from 60 rem, 3 columns from 64 rem) and **grids** are not expressible: (b) `Grid { min: .. }` (section 5.2) |
| headings `h1`-`h3`, landmarks `header nav main footer section aside` | 35 headings, 12 sections, 6 landmarks (`header`, `nav`, `main`, `footer`, 2 `aside`) | (b) | `Heading`, `Section`, landmarks (5.2) |
| **prose with inline links, code, emphasis** | 82 of 162 text blocks; 116 links, 22 `code`, 5 `b` | **(b), the biggest** | `Rich` text (5.2) |
| external links | 128 | (b) | `Link` (5.2) |
| in-page anchors, skip link, section nav, menu | 22 anchors, 12 ids | (b) | `Anchor`, `Link { to: "#id" }` (5.3); `NavLink` is a route |
| `details`/`summary` (menu, map list, transcript) | 3 | (b) | `Details` (5.2) |
| images (`picture` webp + jpg, 5 `img`), videos with posters and captions | 4 + 5 + 2 | (b) | `Image`, `Media` (5.2) |
| code blocks (8 `pre`) and the 3 copy buttons | 8 + 3 | (b) | `Code { copy: true }` (5.2), a `Clipboard` capability |
| `<title>`, 13 `meta`, `canonical`, icons, 2 `alternate` links, `lang`, `color-scheme` | 1 title + 13 + 6 links | (b) | `Page` (5.3); today `--name` sets only `<title>` |
| sticky header with blurred translucent background | 3 `sticky`, 3 `backdrop-filter`, 6 `color-mix` | (b) | a `Header { sticky }` binding; the glass is the compiler's stylesheet |
| palette, light and dark, 3 fonts | 16 semantic tokens + 3 type + 22 art; 30 pairs checked | (b) | `Theme` (5.1); the 22 art tokens belong to the islands |
| stateful widgets: tabs, pressed buttons, switch, chips, filters, detail pane, count | `aria-pressed` 2; chips, tabs | **(a)** | state, events, `if`, `For`, `Button`, `Select`; `aria-pressed` is not a binding (a `Toggle` exists as `Checkbox`; a pressed `Button` is (b), small: 5.2) |
| the agent / human switch and the agent files pane | 1 switch + `agent-files.cui` | **(a)** | already half written; the switch is `if`; the fetch is `net.get` |
| data-driven lists: projects, verifies, paths, the list under the map | 24 projects, 17 claims in 3 lists, 3 paths of 3 entries, the `<details>` list | **(a)** | measured above (24 rows); the Python generator's job moves to a `For` |
| proof figures (3 numbers, 3 waffles) | 1,128 nodes | (b) | `Chart { kind: "waffle" }` (5.5); the numbers are `Text` |
| reveal on scroll | 14 + 10 + strike-throughs | (b) | `Reveal` (5.4) |
| count-up numbers | 3 | (b) | `CountUp` (5.4) |
| pointer parallax | 1 listener | (c) | stays in the hero island (section 6) |
| **hero scene** | 288 nodes; 178 lines of JS | **(c)** | island, section 9 |
| **cairn dividers** | 210 nodes; shares `rocks.js` | **(c)** | island |
| **authority animation** | 80 nodes drawn; 133 lines of JS | **(c)** (state machine in (a), drawing in the island) | section 9 |
| **ecosystem map and the walk** | 426 nodes; 287 lines | **(c)** for drawing; the filter, the detail pane, the count, the transcript are (a) | section 9 |
| reduced motion | 4 CSS blocks + 4 `matchMedia` | (b) | by construction in `Reveal`/`CountUp`; islands get the flag (5.4) |

## 5. The new primitives (the smallest closed set of bindings for each)

Every refusal below has a rule tag that would be documented in `spec/diagnostics/README.md` with a test that reaches it; every binding is a closed
set or a literal checked at compile time ([`controls.md`](controls.md) section 1: an unknown key is refused). **None of this is built.**

### 5.1 `Theme` (#116 with this page as the measured example)

* **What:** one `theme { light { ... } dark { ... } }` block per program with the **16 semantic tokens** above *(18 since 2026-10-24: `code_bg` and `code_fg` were added for the code boxes, section 10 item 8; their names have underscores because a dash is a minus to the lexer)* as `#rrggbb` literals, and `font:` / `mono:` /
  `hand:` as **declared font files** (`Font { family: "Plus Jakarta Sans" file: "fonts/pjs.woff2" sha256: ".." licence: "OFL-1.1" }`).
* **Why not the closed enums #116 first suggested:** this page needs its own palette (a granite and blue-sun one); an enum cannot carry it. What stays closed
  is the *set of token names*; the *value* is a colour whose contrast against its partners the compiler checks.
* **Rule tags:** `theme-token-unknown`, `theme-colour-invalid`, `theme-scheme-missing` (a theme with `light` and no `dark`, or the reverse),
  **`theme-contrast`** (a pair under 4.5:1, naming the pair, the scheme and the ratio), `font-remote` (a font by URL: the page makes no third-party
  request, as today), `font-licence-missing`, `font-hash-mismatch`.
* **The contrast check:** the 13 pairs of the 16 tokens, measured in 2.4 *(corrected 2026-10-24: this said "15 pairs", which counted the two pairs of the site's art palette, `term-fg`/`term-bg` and `map-ink`/`map-paper`, that are not tokens)* (`fg/bg`, `muted/bg|surface|soft`, `accent/bg|surface|accent-soft`, `accent-fg/accent`,
  `good/bg`, `warn/warn-bg`, `bad/bad-bg|bg`, `fg/surface`), each in both schemes. The check is run on the site's own palette first; a deliberate break
  that moves `--muted` to `#9a9a9a` must fail with the tag.
* **Authority:** none. `diff` reports a theme change as its own line (a recolour is a change of the program, not an invisible one), as #116 asked.
* **Replay:** unaffected (replay does not draw). **Accessible form:** the baseline's 44 px targets and focus ring stay the compiler's; a theme cannot
  remove them (there is no key for it). Free CSS is **not** part of this step.
* **The 22 art tokens** (granite, ridge, sky, map colours): not themeable by the program; an island reads the 16 tokens and carries its own art palette.

### 5.2 Structure

> **B1, B2 and B3 are built: [`structure.md`](structure.md)** (2026-10-24): `Heading`, `Section`, the landmarks, `Article`, `Link` with `appearance` and the host listing, `Details`, `Figure`/`Caption`, `Text { style }`; `Rich` with `Strong`, `Em`, `Small`, `Break`, inline `Code` and `Summary`; `Code { copy, max_height }` and the `Clipboard` capability (`Span` was not added: structure.md section 7; `clipboard-write-not-from-tap` is not a tag, because the write is only in the button's click by construction and Chromium measures it). Where the bullets below differ from it, `structure.md` is right and the difference is marked here. `Image`, `Media`, `ratio` and `Grid` with `weights` are B3 (structure.md section 10). Not yet: the skip link (step 3), `pressed:`, and a live region for the copy button's "Copied" (a gap found in B2, structure.md sections 8 and 10.7).

* **`Heading { level: 1..4 "text" }`**, **`Section { id: "try" label: "..." ... }`**, landmarks `Header`, `Nav`, `Main`, `Footer`, `Aside`. A `Main` makes the compiler
  write the skip link *(not built in B1: it is a page-level element, step 3; corrected 2026-10-24)*. Tags: `heading-level-skipped` (an `h3` directly under an `h1`), `heading-empty`, `landmark-duplicate` (two `Main`s).
  *(Corrected 2026-10-24, 5.2b: this list had `section-label-missing`. The page has 12 `section`s and **0 `region` landmarks**, measured in the accessibility tree: an unnamed
  `<section>` is not a landmark, and naming all 12 would add 12 landmarks to a page that has 5. `label:` on a `Section` is therefore optional, and a named one is a region; a `Nav`
  needs a label when there is more than one. Also: the real page's headings run h1, **h3**, h3, h3, h2 ...: the three cards under the hero are `h3`s directly under the `h1`, so
  `heading-level-skipped` would refuse the page as it is. That is the rule working; the page's cards would become `h2`s.)*
* **`Rich { ... }`: inline content.** Children only `Span "text"`, `Link`, `Code "text"`, `Strong "text"`, `Em "text"`; interpolation as in `Text`. This is the
  largest single item (82 of 162 blocks). It lowers to one `<p>` with inline children; a `Text` stays plain. Tags: `rich-child-invalid`, `rich-empty`.
* **`Link { to: "https://..." "label" }`**: a literal URL only; `https:` or `http:` or `mailto:` or a `#id` (`link-scheme-refused`: `javascript:`, `data:`);
  written with **no `rel`** *(corrected 2026-10-24, 5.2b: this said `rel="noopener noreferrer"`. The page has 128 external links, 0 `target`s and 0 `rel`s on links; `noopener` only matters
  with `target="_blank"`, which `Link` has no binding for, and `noreferrer` would remove the `Referer` the linked sites now receive: a change of behaviour nobody asked for)*. An accessible name is the label (`link-label-missing`, and `link-label-generic` for "click
  here", "here", "link"). **Authority: external links are reachable destinations, not effects**, but they are listed: `authority` shows `links:
  alpibru.com, github.com`, and **`diff` reports a new host** (the page has 128 external links to a handful of hosts: a new one is a review item). A link built from fetched data (the
  projects' `url`) must pass the same scheme check at run time and is not host-listed (`link-dynamic`, shown by `authority` as "dynamic links").
* **`Details { summary: "..." ... }`**: a `<details>`; open state is the browser's, not program state (the page has three and needs no more).
* **`Image { src: "assets/img/field.webp" fallback: "assets/img/field.jpg" alt: ".." width: 1024 height: 768 }`** *(corrected 2026-10-24, B3: `fallback:` is a path, not a suffix: the compiler must find the file; structure.md 10.1)*: `picture`/`source` for the pair; `alt` is required, and
  `decorative: true` is the explicit way to say none (`image-alt-missing`); `width` and `height` are required (no layout shift: `image-size-missing`);
  `loading="lazy"` below the fold is the compiler's. The file must exist in the build (`asset-missing`), is in the offline precache if `--pwa`, and its hash is
  in `diff`.
* **`Media { label: ".." kind: "video" src: ".." poster: ".." caption: "id" transcript: "id" }`** *(corrected 2026-10-24, B3: there is no `alt-source:`; `label:` is required; `caption:` and `transcript:` are text literals naming an `id`; structure.md 10.1 and 10.3)*: `controls`, `playsinline`, `preload="metadata"` always;
  **no autoplay** (`media-autoplay-refused` is not a key you can set: it does not exist); `caption:` must name a caption element (`media-caption-missing`), the page's
  `aria-describedby`. A transcript or text alternative is *required to be declared* (`media-text-alternative-missing`), as the page's "the same session as text".
* **`Code { copy: true "text" }`** *(5.2b: the page draws code on `--term-bg`/`--term-fg`, which are two of the 22 art colours and not tokens; a decision is needed)*: a `<pre tabindex="0"><code>`; `copy:` adds the Copy button; the **`Clipboard` capability** (`requires Clipboard;`, write only,
  on a user tap only) is declared and shown by `authority`. Tag: `clipboard-write-not-from-tap`.
* **`pressed:` on `Button`** (`aria-pressed`, a `Bool` state field) for the tabs, chips and switch (`button-pressed-type`).
* **`Grid { min: 18 }`** *(built in B3 with `weights:`; measured against the page's `.videos`, `.gallery` and `.try`: structure.md 10.4)*: a responsive grid with a column minimum in `rem` (`grid-min-invalid` outside a range), written `repeat(auto-fit, minmax(min(100%, 18rem), 1fr))`;
  `columns: 1 | 2 | 3` per width breakpoint is not offered. *(**The claim here was wrong and is replaced** by 5.2b: of the page's 19 width queries `min:` reproduces **1**, and 3 more
  approximately; 6 are asymmetric columns, 5 show or hide something, 3 scroll or collapse. A `weights:` binding is proposed for the asymmetric ones.)*
* **Replay:** none of these has behaviour but `Button.pressed` and `Code.copy` (a handler); replay checks the lowering and the refusals. **Accessible form:** the
  elements *are* the semantic HTML (heading levels, landmarks, `alt`, link names), and `check` refuses the cases above instead of an audit finding them later.

### 5.2b What three real sections showed (Phase A of step 2, 2026-10-24)

**Method.** cancho-www re-cloned read-only, shallow, at **`dbbf7ae`** ("Map: a trail of roles passing a file", #13), newer than `0579a80`: `index.html`, `site.js` and `ecosystem.json` are
byte-identical to `0579a80`; `site.css` is +2 lines and `map.js` +15. Three sections, written as **sketches in the proposed syntax, not compiled** (`docs/www-sketch/`): `try`
(`index.html` 93-151, prose with links and code), the header with `#name` (28-45, 533-547: landmarks, a nav, a menu, a pressed-button switch, three images in figures), and `demo`
(377-466: two videos with captions and a transcript). Each was compared element by element with the page's DOM. **The "compiled DOM" in the tables is derived by hand from the proposed lowering,
not produced by a compiler**; the page's own numbers are counted. Layout was measured in Chromium at 12 widths (320 to 1440) on the real page.

#### What the existing sections are

| | `try` | header | `name` | `demo` |
|---|---|---|---|---|
| elements | 58 | 35 | 22 | 26 |
| text blocks with inline markup | 5 of 10 `p`; 10 `code`, 1 `b`, 9 `a` | n/a | 1 `b` | links inside both `figcaption`s, a `small` inside a `summary` |
| classes the CSS depends on | `eyebrow sub try reveal tx when links btn(primary, ghost) cd codebox what note` (14) | `top wrap brand prever opt hright switch ghbtn menu` | `eyebrow sub gallery reveal note` | `eyebrow sub videos demo-text code` |
| ids / anchors | `try` | `top`, `view-human`, `view-agent` | `name` | `demo`, `cap1`, `cap2` |
| notable attributes | `tabindex=0` on 5 `pre` | `aria-label` x3, `role=group`, `aria-pressed` x2, `aria-hidden` x2 (icons) | `srcset`, `width`, `height`, `alt` (long), `loading=lazy`, `decoding=async` | `preload`, `poster`, `aria-describedby`, 4 `source`, a fallback `a` inside each `video` |

#### What the proposed bindings can say, and what they can not

| # | not expressible, or wrong in 5.2 | measured | what it needs |
|---|---|---|---|
| 1 | **The page works without JavaScript; a cancho-ui page does not** | page without JS: 738 elements, 16,006 characters of text; a `build --page` counter without JS: **3 elements, "This app needs JavaScript."** | **material: section 5.2c** |
| 2 | a link drawn as a button (`.btn` with `primary`, `ghost`, `text`) | 9 `a.btn` (6 ghost, 2 primary, 1 text) | `Link { appearance: "primary" \| "secondary" \| "quiet" }`, a closed set; colours from the theme tokens |
| 3 | text roles (`eyebrow`, `sub`, `note`, `when`, `what`) | `eyebrow` 13, `sub` 10, `note` 9, `what` 6, `when` 3 | `Text`/`Rich { style: "eyebrow" \| "lead" \| "note" \| "label" }`, closed; an accessible reading is unchanged (all are paragraphs) |
| 4 | asymmetric columns (`1fr 1.75fr`) | `.try article`: 308 + 539 px at 960 px, 378 + 661 at 1280; auto-fit `min` can match the *breakpoint* (by arithmetic on the measured widths, not measured: a `min` of about 25 rem would put the change between 900 and 960 px, as the page does) but gives equal columns: about 420 px for the code column (derived), not 539 to 661 | `Grid { min: 25 weights: [1, 1.75] }`, weights from a short list of ratios. **Built and measured in B3 (2026-10-24): `Grid { min: 19 weights: [1, 1.75] }` gives 308 + 539 at 960, 331 + 579 at 1024, 378 + 661 at 1280, the page's numbers to 0.1 px; its breakpoint is at viewport 950, 10 px before the page's 960 (`min` is a whole number of rem and a `Grid` has no viewport rule). The `min: 25` guess above was wrong: with weights, `min` is the width of the weight-1 column (structure.md 10.1 and 10.4).** |
| 5 | `<article>` | 6 `article`s in the AX tree, 0 `region`s | `Article { }`: `Section` would add a landmark per card |
| 6 | inline content outside `Rich`: the `<img alt="">` logo inside the brand link, the star `<svg aria-hidden>` in `GitHub`, an icon-only `summary` named by `aria-label`, `<small>` in a `summary`, `<br>` and `<em>` in the `h1` | 19 `a` in the header, 2 `svg`, 1 `br`, 1 `em`, 1 `small` | `Heading`, `Link`, `Details.summary` take inline children; `Rich` gets `Break` and `Small`; an icon is `Link { icon: "star" }` from a closed set, or stays an island (step 3) |
| 7 | **responsive visibility and swapping** (the nav is shown from 62 rem and a `Details` menu with the same links from below) | 5 of the 19 width queries (46, 40, 30, 62 and 62 rem); the 9 menu links are a **second copy** of the nav | `Nav { collapse: "menu" }` emitting one list; belongs to step 3 (5.3, the header); until then two lists and no hiding |
| 8 | image crop to a ratio, rounded corners, hover zoom | `.gallery picture { aspect-ratio: 4/3; overflow: hidden; border-radius: 16px }` and a 1.04 scale on hover, with a reduced-motion override | `ratio: "4:3"` is a binding; corners and shadow are the baseline's; the zoom is **not** expressible (a motion effect, step 5) |
| 9 | an `h3` inside a `<figure>` before the media | 2 `figure`s have one | `Figure { .. }` and `Caption { id }` as containers (the proposal had `Image`/`Media` own the `figure`); `Media.caption:` names a `Caption` |
| 10 | a scroll box with a height limit (`pre.code`, 28 rem) | 1 | `Code { max_height: 28 }` (a closed range) |
| 11 | **`Code` colours**: the page's code boxes are `--term-bg` `#12151a` with `--term-fg` `#d6dde8` in both schemes (13.39:1) | those are two of the 22 art colours, **not** in the 16 tokens; `fg` on `soft` is 15.6:1 (light) and 13.91:1 (dark) | **a decision**: (a) add `code_bg`, `code_fg` as tokens 17 and 18 and a pair; or (b) draw code in `fg` on `soft`, which loses the dark box in the light scheme |
| 12 | `Section` is not the only anchor target | the hero is a `div id="top"` | `Section` can be any block and carry `id`; `anchor-unknown` still applies |
| 13 | `role: group` on a `Row` (the human/agent switch) | not tested | to be checked in B1; a `Group { label }` if `role:` does not allow it |
| 14 | `.reveal` (14 uses) and `style="--i:0"` per item | 14 + 10 cairns | step 5, not step 2 |

**What expressed cleanly:** `Section`, `Heading`, the `Rich` paragraphs with `Link`, `Code` and `Strong` (every inline element in the three sections: 9 `a`, 10 `code`, 1 `b`), `Image` with `webp` + `jpg`,
`width` and `height`, `alt`, and `loading`; `Media` with two sources, a poster, `controls`, `playsinline`, `preload`, and a described-by caption; `Details`; the footer. The compiled elements were **not counted** (nothing compiles yet); `Column` and `Grid` would supply the page's `div.tx`/`div.cd` wrappers, and none of the author's class names would exist.

**The 19 width queries, classified** (the claim in 5.2 was that `min:` covers "mostly 1 column under a width, N above"):

| kind | queries | `Grid { min }` |
|---|---:|---|
| auto-fit equal columns | 1 (`.videos` under 30 rem) | **yes**: the same column count as the page at all 12 widths, for `.videos` (`min: 22`) and `.gallery` (`min: 17`, which the page has no query for); `.videos` needs `min(100%, 22rem)` or it overflows below 22 rem, which is why the page has its 30 rem rule |
| equal columns from one breakpoint (`.chain` 5, `.verif` 3, `.paths` 3) | 3 | approximately: auto-fit gives 2 and 4 columns between, where the page has 1 |
| a decoration that turns with the layout (`.chain` connector) | 1 | no |
| asymmetric or fixed-side tracks (`.hero` twice, `.stage`, `.mapwrap`, `.agents`, `.try`) | 6 | **no**: needs `weights:` or a fixed side |
| show or hide something by width (`nav a.opt`, `.prever`, `.ghbtn`, the nav/menu swap, its margins) | 5 | no |
| scroll or collapse (`.map`, `.canvas`, `.compare`) | 3 | no (the first two are islands) |

So of 19, `min:` is right for 1, roughly right for 3, and not meant for 15. In the three sections: the header has 5 queries, all visibility; `try` 1, asymmetric; `demo` 1, the `min:` one.

**A side finding about the page itself (not touched):** at 1024 px wide the page's scroll width exceeds its client width by **63 px** (the header's nav is shown from 62 rem and does not fit; the
body hides `overflow-x`, so no scroll bar shows) and at 320 px by 4 px. Measured on the clone served locally, in Chromium, as scroll width minus client width (a script of this session, not a repository script).

**Landmarks and headings, measured on the page** (Chromium accessibility tree, with and without JavaScript): `banner` 1, `navigation` 1, `main` 1, `contentinfo` 1, `complementary` 1, `region` 0, `article` 6,
`figure` 5, 35 to 36 headings, 111 links, `DisclosureTriangle` 2, `Video` 2, a `table` 1. Heading levels in order: `1 3 3 3 2 ...`: one skip (h1 to h3).

### 5.2c Static first paint: the page is readable without JavaScript, and a cancho-ui page is not

**Measured.** With JavaScript off, the cancho-www page has 738 elements and 16,006 characters of text (1,507 accessibility nodes, 5 landmarks); a program built with
`cancho-ui build --page` has `<div id="root"></div><noscript>This app needs JavaScript.</noscript>`: **3 elements**, one sentence. With JavaScript on, the home page is 2,951 elements.
`docs/platform-model.md` lists "a server-side rendering backend" as a Phase 6 "if asked" item; nothing in the compiler renders HTML.

**Why it matters for this page in particular.** The page is also *for agents* (`agents.md`, `llms.txt`, `<link rel="alternate">`): a crawler, a link-preview fetcher or an agent that
fetches HTML without running scripts would see "This app needs JavaScript." Writing the page's structure in cancho-ui (step 2) with no static output is a regression for them, and 2.3 above listed
"works with scripts off" as "a property to keep" without noticing that no step of the plan keeps it.

**Options, none built:**

| option | what it is | cost |
|---|---|---|
| A. **Render the first view at build time** | `build --page` writes the initial DOM, from the initial state, into `#root`; the script then rebuilds it (it already empties `#root` and rebuilds at every render) | a second renderer in Rust beside the JS emitter for each element: the structure elements are static and simple, the controls are not; the two must agree on a **shared corpus** (serialise both DOMs and compare), the way replay and the page do. Data fetched at `start` (a `net.get`) is absent from the static HTML. Medium to large |
| B. **Static elements only** | only the elements with no state-dependent part are prerendered *(corrected 2026-10-24, prerender.md section 2: the compiler computes which, so the program does not say so: `Section { static }` is dropped; the hole rule and the measurements are in `prerender.md`)* | smaller; the page's prose, headings, images and videos are exactly these |
| C. **Accept it** | keep the HTML twin the site already has (`agents.md`, `llms.txt`) for crawlers and agents, and require JavaScript for humans | none to build; the home page stops being readable by tools that do not run scripts, and the first paint is an empty page until `app.js` runs |
| D. **A hybrid** | the program is the source of truth; a separate generator writes the structure's HTML into `index.html` between markers (the way `gen_agent_files.py` does for four regions today) | a second generator, which is the thing step 2 was meant to remove |

**This changed the plan materially; the user chose B on 2026-10-24 (section 10 item 7).** The recommendation is **B** (prerender what is static, measure it, and say plainly that an element with state is
not prerendered): it keeps the readable content and the agent-facing HTML, it is checkable by a shared corpus, and it does not need the rest of the compiler to change. The cost of choosing C is the one
sentence above.

### 5.3 Page-level

* **`Page { title: ".." description: ".." lang: "en" canonical: ".." image: ".." alternates: [...] icon: ".." }`**: the 13 `meta`, `canonical`, `icon`,
  `apple-touch-icon`, the two `alternate` links and `<html lang>`. Tags: `page-title-missing`, `page-description-length` (outside 50..160 chars),
  `page-lang-invalid`, `page-canonical-not-https`, `page-image-size-missing` (the OG image's width and height), `anchor-unknown` (`Link { to: "#x" }` with no `Section { id: "x" }`),
  `anchor-duplicate`. Authority: none; `diff` shows the metadata change. The generated `agents.md`/`llms.txt` URLs would still come from the data file.
* **`Header { sticky: true }`**: sticky with `scroll-padding-top` set by the compiler so anchors are not hidden under it (the page sets `5rem` by hand).
* The Python generator keeps writing `agents.md` and `llms.txt` (they are not the page). Whether `index.html` and `agents.md` can still not disagree once the
  page is not generated from the same file by the same script is a **decision for the user** (section 10).

### 5.4 Scroll, viewport, motion (reduced motion is the compiler's, not the author's)

* **`Reveal { ... }`**: a container that gets `data-in` once, when 25 % of it has been seen (`IntersectionObserver`, threshold fixed; the page uses 0.25, 0.4, 0.5
  in three places). With no `IntersectionObserver`, or under `prefers-reduced-motion`, it is shown immediately. **The author cannot opt out**: there is no
  key for it, so every use is safe by construction.
* **`CountUp { value: n }`** on a number `Text`: an eased tween over a fixed 1.3 s to `n`; under reduced motion, or with no JS, the final number is written
  at once. The tag `countup-not-number` refuses a non-number. The final value is always in the DOM text (an assistive technology reads the number, not the
  frames).
* **`on Visible` / `on Hidden`** events on a `Section`: for the authority animation's "start when half visible". The handler is an ordinary handler; the
  replay trace carries the event (`{"event":"Visible"}`), so the state machine is replayable without a viewport. Tag: `visible-threshold-invalid`.
* **Pointer parallax is not a primitive**: continuous `pointermove` data is input a program would have to declare and replay (a `Pointer` capability); one
  decoration does not earn it. It stays in the hero island, which receives the pointer as a message (section 6).
* **Reduced motion for islands:** an island receives `reducedMotion: true|false` as a prop and is told when it changes; an island that ignores it is a defect of that island,
  visible in the island's declaration review, not hidden.
* **Authority:** `Reveal`/`CountUp`/`on Visible` reach nothing outside the page. Replay: `Visible` is a trace event; `CountUp`'s end state is what replay
  shows. **Measured today:** 33 CSS animations on the page, 30 running after load, **0 under `reducedMotion: 'reduce'`**; the same page without that setting
  runs them, so the existing hand-written handling works. What the compiler adds is that it can no longer be forgotten.

### 5.5 Drawing, kept small

* **`Chart { kind: "waffle" of: 1472 value: 1380 columns: 44 label: ".." }`**: the proof section's three charts (1,099 of the 2,951 nodes) are a grid of cells. This extends
  the **existing `Chart`** (`charts.md`, one SVG, `role="img"` with a text alternative and a collapsed data table), not a new drawing language. Tags:
  `chart-kind-invalid` (exists), `chart-waffle-of-invalid`. Replay: the cell counts. Accessible form: the label plus the table, already the `Chart` contract; the
  site's `aria-label="passed of total"` is the same information.
* **A general `Svg`/`Path`/`Circle` vocabulary is not proposed.** It would be a second UI language inside the first; the program would say *how it is
  drawn* (the opposite of "the program says what a thing is"), replay would have nothing to check, and every drawing would need its own accessible form.
  Section 9 argues the hero and the map should not be rewritten this way.

## 6. Islands: how plain JavaScript is declared, so it cannot be smuggled in

An island is an **escape hatch**: it is code the compiler does not read. The rule is that it is *declared, pinned, visible to `authority` and `diff`, and
sealed by default*.

```cui
island Hero {
    source: "assets/island-hero.js"
    sha256: "9f2c...e1"            // checked at build; a changed file is a changed program
    sealed: true                   // default; `sealed: false` is the open form below
    size: Responsive { ratio: 640 / 600 }
    props { reducedMotion: Bool, palette: Theme, pointer: Pointer }
    events { }
    label: "A stack of granite boulders in front of a blue sun"
    requires Draw;                 // the only authority a sealed island has
}
// in a view:  Hero { reducedMotion: motionReduced }
```

* **Declaration, not use, is the unit.** An island must be declared with `island` in the program (or an imported file) and used by name; the build emits **only**
  script that is the compiler's own runtime or a declared island's file, matched by `sha256` (`island-hash-mismatch`, `island-undeclared` for a script tag, an
  inline `<script>`, or `javascript:` smuggled through a string; `island-source-missing`). A `Link`/`Rich`/`Code` containing markup stays text (they take text).
> **Built 2026-10-25 (sealed islands only, [`islands.md`](islands.md) section 11):** the declaration is `island Name { script: ".." sha256: ".."; .. }` at the top level, not a `source:` entry; the frame's own policy names each script **by hash** (`script-src 'sha256-..'`, no `'unsafe-inline'` for scripts: an inline script, a `document.write`, an inline handler and a `javascript:` address the island adds do not run, measured); `island-undeclared` is **not a tag** (a string never becomes markup or script: no `innerHTML`, `eval` or `new Function` in the output, `link-scheme-refused` refuses `javascript:`, and a script written as `Text` is text, checked); the props of a sealed island are literals (the pointer is fed by the runtime). The open island is not built.
> **Built 2026-10-25, the open island ([`islands.md`](islands.md) section 12):** `sealed: false;`, `requires Dom;`, `UNBOUNDED` in `inspect effects`, `authority: expanded` in `diff`, a text alternative named with `text:`; the map is the worked example, with the filters, the detail pane and the list in cancho-ui. No sandbox claim is made for it.
> **Measured again 2026-10-25 ([`islands.md`](islands.md), `scripts/measure-islands.js`), and this section corrected by it:** the CSP below does **not** seal the frame (the island navigated its own frame to another origin, and a `<meta refresh>` did the same; what closes both is the *page's* CSP `frame-src about:`); WebRTC sent 3 UDP packets to a STUN server in every configuration until a shim removed the constructors; `requires Draw` is dropped (a sealed island reaches nothing but the page); `size: Responsive { ratio: 640 / 600 }` is `width: 640 height: 600`; a sealed island can still stall the page for as long as it loops (1,983 ms measured); and 11 sealed frames for the cairns cost about 50x the time of drawing them in the page.

* **Sealed, measured (Chromium, local servers):** the island runs in `<iframe sandbox="allow-scripts" srcdoc="…">` with a `<meta http-equiv="Content-Security-Policy"
  content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'">` that the compiler writes into the `srcdoc`. Measured result: the island
  script **cannot read the parent** (`parent.document` throws `SecurityError`), **cannot reach `localStorage`**, **its `fetch` to another local server failed
  with `TypeError` and that server received no request**, and a created `Image` made no request either; it **can** build SVG nodes and post a message to the
  page. Without the CSP, the same sandboxed frame's `fetch` **reached** the other server. This works on a plain static host: the `csp=` iframe attribute
  needs an `Allow-CSP-From` response header (measured: it blocked the frame without it), which GitHub Pages cannot send, so the CSP is a `<meta>` inside the srcdoc.
* **What sealing costs, so the choice is honest:** the island is its own document. It does not inherit the page's CSS variables (the compiler passes the theme
  as props and a `<style>` it writes), the pointer and scroll do not cross the frame (the compiler forwards `pointer` as a message only if the island declares it), the frame needs
  an explicit size (`size:`), a screen reader sees an `iframe` with a `title` (the declared `label:`) and **not** the drawing's nodes (the 288-node hero is decoration; the
  map is not, see section 9), and a frame is heavier than a group of SVG nodes. Those are the costs of an enforced authority bound.
* **`sealed: false` (an open island):** same document, full DOM, can read and write the page and call `fetch`. It is allowed, but `authority` prints it as
  **`island Map: UNBOUNDED (runs plain JavaScript in the page's origin)`**, `diff` marks its addition, hash change and the change of `sealed` as the highest-severity lines,
  and the CI of the page would pin it. Use it only where a frame cannot work (section 9: the map's keyboard focus ring and `textPath` need the page's document).
* **Authority:** *(corrected 2026-10-25: no `requires Draw`; a sealed island is listed, not granted anything)* `requires Dom` for an open one. They are capabilities like
  `Network`, listed by `cancho-ui inspect effects`. `diff` prints a new island, a changed `sha256`, and a changed prop or event list.
* **Events out:** `events { Selected(String) }` are ordinary program events with typed payloads, validated at the boundary (a message that is not a declared event, or has
  the wrong type, is dropped and counted: `island-event-undeclared`, `island-event-type`).
* **Replay:** an island is **not run** by `cancho-ui test`. The trace records the props sent and the events emitted; replay checks the program's behaviour *around* the
  island (the filter, the selection, the detail pane). The island's own behaviour is checked by a Chromium script the page owns, and the compiler **says so**: a replay report lists
  `island Hero: not replayed`. Nothing is claimed about what an island draws.
* **Accessible form:** `label:` is required (`island-label-missing`); `decorative: true` marks a pure decoration (the hero, the cairns) and writes `aria-hidden`; an island
  that conveys information (the map) must also declare a text alternative element (`island-text-alternative-missing`), which on this page is the list under the map that already exists.

## 7. The order, with what each step is worth

Counted against the page as it is. "Unblocks" is the share of the **page** that could then be written in cancho-ui (not the share that *is*).

| step | what | unblocks (measured) | risk | argument |
|---|---|---|---|---|
| **1** | **Theme** (5.1) | nothing visible alone; every later step needs it to look like the page. 16 tokens, 30 contrast checks | **low**: closed set, a pure function, no runtime | cheap, independent, and the measured example already exists (all 30 pairs pass). Also settles who owns the 44 px/focus-ring guarantees (#116's open question) before anything is built on it |
| **2** | **Structure and `Rich`** (5.2) | **all of the readable content**: 762 non-SVG nodes, 23,369 bytes of text, 128 of 164 links, 82 of 162 text blocks, 9 images, 2 videos, 8 code blocks | **medium**: `Rich` is a new kind of element with children; a11y rules to get right | without `Rich` and `Link` **no prose section can be written at all**. This is where the page starts to be *in* cancho-ui |
| **3** | **Islands** (6), *moved up from step 5* | **the other 74 %** (the hero, the map, the authority drawing, the cairns), as sealed or open islands | **medium**: the compiler learns to refuse an undeclared script; a frame protocol | after steps 2 and 3 the **whole page is a cancho-ui program** and every opaque part is declared and visible in `authority`/`diff`. Doing it before motion and SVG means the page can be finished and *then* improved piece by piece, and the escape hatch is measured against the real page rather than designed in the abstract |
| **4** | **Page-level** (5.3) | head: 13 `meta`, `canonical`, 2 `alternate`, `lang`; anchors; sticky header | **low** | needed before the page can *replace* `index.html` (link previews and agent discovery would otherwise silently disappear) |
| **5** | **Scroll, viewport, motion** (5.4) | `Reveal` 24+ uses, `CountUp` 3, the authority animation's start | **medium**: `IntersectionObserver` in Chromium only here | replay carries `Visible`; reduced motion becomes impossible to forget |
| **6** | **Data-driven sections and `Chart { kind: "waffle" }`** (5.5) | the 3 waffles (1,099 nodes), the lists the Python generator writes (17 `verifies` claims, 9 path entries, 24 projects) | **low**: mostly (a) already (spike: 24 rows) | last because it needs the least: the list part is **already possible today** |

**Where this differs from the expected order, with the number:** islands move from 5 to 3 because (1) 74 % of the DOM is drawing that no primitive in steps 1, 2, 4 or 6
covers, so until islands exist nothing can *finish* the page; (2) the island mechanism is the part with the highest **unknown** (the iframe costs in section 6 are measured
only on a toy), so it should meet the real hero early; (3) it makes steps 5 and 6 optional improvements rather than blockers. The risk of moving it up is the opposite
one: an escape hatch available early removes pressure to build the real primitive. The counter is the rule in section 6: an open island is the highest-severity line in `diff`,
and the sealed form has a real cost, so neither is free to reach for.

If the user prefers the expected order, steps 3 and 5 swap with no other change: nothing in 5.4 depends on islands except the reduced-motion prop.

## 8. What could NOT be measured here

* **Any real device**, any browser but Chromium on Linux (**Safari, Firefox**): `IntersectionObserver` behaviour, `backdrop-filter`, `color-mix()`, `<picture>` webp, sticky positioning, and
  the sandboxed-iframe and `<meta>` CSP behaviour of section 6 were observed in Chromium only.
* **Fonts rendering**: the three variable fonts were served, but how they look (hinting, `ss01`, the Caveat hand) was not compared; no pixel comparison between the existing page and a cancho-ui page exists yet.
* **The cost of an island on a phone**: the iframe measurements in section 6 are on a 40-line toy, not on the 288-node hero or the 426-node map; frame creation time,
  memory and scroll performance with several frames are unmeasured. The map walk's frame rate was not measured.
* **Whether a screen reader reads the drawings**: the site's `role="img"`/`aria-label` on the scene and the map's `role="button"` per rock were counted, not heard (VoiceOver and TalkBack are manual steps, as in `mobile-backend.md` section 8.1).
* **Lighthouse and Core Web Vitals** (no network access to run them against the deployed site; the page was loaded locally).
* **The size of a cancho-ui page**: the compiler's output for the 61-line `agent-files.cui` is 21,897 bytes today (12,640 deployed). How much of it is a fixed runtime and how much grows with the
  program has not been measured; it matters if the whole page becomes components.
* **That `Rich`, `Link` and `Grid { min: }` cover the page**: the claims "82 of 162 blocks", "19 width queries are mostly one-column-under-a-width" are counts of the existing markup; whether the proposed bindings reproduce the layout
  was not tested. Step 2 must start by writing three real sections and comparing them with the existing ones in Chromium.
* **The deployed site, Pages and the custom domain**: the page's own `check_public.py` and CI were not run.

## 9. Should the hero scene and the map be rewritten in cancho-ui at all?

**No, not as drawing. Keep them as islands, and move their *logic* into cancho-ui.** The reasons are measured, and each choice has a cost.

| | hero scene | cairns | authority animation | the map |
|---|---|---|---|---|
| size today | 288 nodes, 65 + 113 lines (`scene.js` + shared `rocks.js`) | 210 nodes, shares `rocks.js` | 80 nodes drawn, 133 lines | 426 nodes, 287 lines |
| what it computes | seeded random shapes, pointer parallax | seeded random shapes | positions of 5 rocks per step | seeded shapes, 24 hard-coded spots, trails, `textPath`, `feTurbulence`, a rAF `viewBox` tween |
| informational? | decoration | decoration | the content (a state machine over 3 scenarios) | **yes**: 24 projects, the `rests on` arrows, keyboard operable |

* **Rewriting the drawing in cancho-ui would need a drawing vocabulary** (paths, gradients, filters, `textPath`, transforms, animation) that section 5.5 argues against: it
  would make the program say *how* it is drawn, replay would have nothing to check (the hero is a deterministic function of a seed; checking it means comparing pixels, which is not
  what replay is), and each shape needs an accessible form. Cost of the other choice: **the drawing stays 74 % of the page's DOM and is opaque to the compiler** (declared, sealed or
  open, listed by `authority`, not read).
* **The hero and the cairns: sealed islands**, `decorative: true`, a `pointer` prop, a `reducedMotion` prop. No information is lost to a frame: it is decoration. Cost: the
  frame, no pointer without the forwarded message, and the page's CSS variables must reach the frame as props.
* **The authority animation: the state machine in cancho-ui, the drawing in a sealed island.** The scenarios (code lines, captions, steps, positions) are *data*; the
  stepper (tabs, prev/play/next, the verdict, the caption in an `aria-live` region, the 2.6 s timer, auto-play on `Visible`) is state and events, all (a) today except
  `Visible` (5.4) and `pressed` (5.2). The island gets `scenario`, `step` and draws the five rocks. This is the part where moving the logic **improves** the page: the captions and
  verdicts become replayable and accessible text outside a frame.
* **The map: logic in cancho-ui, drawing in an open island, the list as the text alternative.** The filter chips, "only what exists", the count, the detail pane (stage chip, summary, the `dl`), the
  repository and landing links, and the recorded session text are (a) with `For` and the new `Link`, fed by `net.get("data/ecosystem.json")` (spiked: 24 rows today). The island
  draws 24 rocks and the trails and reports `Selected(id)`. It should be **open (`sealed: false`)** because its rocks must be keyboard-focusable in the page's focus order and the walk's
  `viewBox` tween and `textPath` labels are cheap in the page's document and awkward across a frame; the cost is that `authority` prints **`island Map: UNBOUNDED`**, which is true and
  is the point. The alternative (sealed) is possible if each rock's focus is moved to a real `Button` list outside the frame (the list that exists under the map), at the price of the
  map being a picture rather than an interface for keyboard users, which the page does not do today.
* **What would change my answer:** a user who wants the drawings to be *editable from the program* (a different seed, a new project drawn without touching JS) would justify a small
  `Rock`/`Ridge` vocabulary specific to this page; that is a design for a later note, and it would be one of the first things to measure in the first island.

## 10. Decisions (**Decided 2026-10-24 by the user**, relayed by the coordinator)

1. **Order:** as in section 7, islands at step 3. *Decided 2026-10-24 by the user.*
2. **Themes:** free colours with a compiler-checked contrast (5.1), not closed enums. *Decided 2026-10-24 by the user.* Built: [`theme.md`](theme.md).
3. **The map is an OPEN island** (`authority` prints UNBOUNDED); the decorative hero and the cairns are **SEALED** islands. The list under the map stays the text alternative. *Decided 2026-10-24 by the user.*
4. **`agents.md` and `llms.txt`** stay generated by the Python script from `ecosystem.json` for now; to be revisited after the page is built. *Decided 2026-10-24 by the user.*
5. **Fonts:** self-hosted files declared in the program with their licence and SHA-256 (5.1), no remote fonts. *Decided 2026-10-24 by the user.* Built with the theme.
6. **Where the page lives:** `ui/` of cancho-www, built with `cancho-ui build --page`. This session has no write access to cancho-www and does not touch it; the user will be asked for access only when there is a page to propose. *Decided 2026-10-24 by the user.*

7. **Without-JavaScript content (5.2c): prerender the static parts of a `--page` build** (option B). *Decided 2026-10-24 by the user.* To be designed first (`prerender.md`), then built.
8. **Code boxes: two theme tokens, `code_bg` and `code_fg`**, with their contrast pair (`code_fg` on `code_bg`, both schemes, 4.5:1), keeping the dark code boxes; 18 tokens. *Decided 2026-10-24 by the user.* Built ([`theme.md`](theme.md)): the site's `--term-bg #12151a` and `--term-fg #d6dde8`, 13.39:1 in both schemes.
9. **The additions of 5.2b, as closed sets:** `Article`, `Figure`/`Caption`, link `appearance`, text `style`, inline content in `Heading`/`Link`/`Details` summary, `Code { max_height }`, `ratio` on `Image`/`Media`, and `weights:` on `Grid` (`min` stays). Any may be argued against by measurement in its PR. *Decided 2026-10-24 by the user.*
10. **Order:** P0 the code tokens; B1 `Heading`, `Section`, landmarks, `Article`, `Link` (+ `appearance`, host listing), `Details`, `Figure`/`Caption`, text `style`; B2 `Rich` and `Code`; B3 `Image`, `Media`, `ratio`, `Grid` weights; then P1 prerender (design note first). *Decided 2026-10-24 by the user,* with leave to re-order if prerender must come first, reported before doing so.

Step 1 is built and is the only step built.

## 11. Rules kept

The program says what a thing is (every proposed binding is a closed set or a literal; the one drawing extension reuses `Chart`). Authority is declared in `requires` and printed
by `authority` and `diff` (`Clipboard`, `Draw`, `Dom`, link hosts, islands). Every refusal has a rule tag (listed per primitive). Replay and the page agree on a shared corpus for
each primitive, and an island is **reported as not replayed** rather than claimed. No new dependency (the islands are the page's own files, hashed). **Only step 1 is
built (`theme.md`); nothing was pushed to `cancho-www`.**
