# The page: its head, its skip link and its sticky header (`www.md` step 4)

> **Status: built 2026-10-25.** `page { .. }`, the skip link and `Header { sticky: true }`. Every number below is measured and says how; what could not be
> measured is in section 8. Written after the primitives were built, against the real page (`alpibrusl/cancho-www` at commit `0579a80`); where the plan of
> [`www.md`](www.md) section 5.3 disagreed with that page, it was measured and corrected there too.

## 1. What was measured on the real page

Chromium, the page served locally (`scripts/check-page.js` repeats what can be repeated; the rest were one-off scripts):

| what | measured | what the plan said |
|---|---|---|
| the head | 16 `<title>`, `<meta>` and `<link>` elements between `<title>` and the stylesheet | "13 `meta`, canonical, icon, apple-touch-icon, 2 alternate, lang" |
| `description` | **261** characters | outside 50..160 refused: **would refuse the real page** |
| `og:description` | **111** characters, a different text from the description | one description |
| `<title>` | 55 characters | |
| the sticky header | **64 px** at 1280, 1024, 768, 600, 480, 390, 360 and 320; `scroll-padding-top` 5rem = 80 px; an anchor lands **16 px** below it, at every width | "the page sets 5rem by hand" |
| in-page links | **22** `href="#.."`, all resolve to an `id`; no `id` repeats | |

So the plan's 160 is wrong for this page and is corrected (section 2); `og:description` needs a setting of its own; and the real header is short enough
that a fixed 5rem happens to work for it. For a program's own header it does not (section 5).

## 2. `page { .. }`

A top-level declaration, one per program, like `theme` and `island` (`www.md` 5.3 wrote it as an element `Page { .. }`; head metadata is not view content,
a component is not the place for it, and a program that mounted two would have two heads). `page` is a keyword only at the top level, followed by `{`.

```
page {
    title: "cancho: tools built for agents, not adapted from humans";
    description: "cancho is a programming language and toolchain ..";
    social_description: "Trust without comprehension: ..";     // og:description; the description when absent
    lang: "en";                                                // default en
    canonical: "https://alpibrusl.github.io/cancho-www/";
    image: "https://alpibrusl.github.io/cancho-www/og.png";
    image_width: 1200;  image_height: 630;
    image_alt: "cancho: Tools built for agents. Trust without comprehension.";
    icon: "logo-mark.png";            apple_icon: "logo.png";
    alternate: "agents.md" mime: "text/markdown" title: "cancho, as an agent reads it";
    alternate: "data/ecosystem.json" mime: "application/json" title: "The cancho ecosystem as data";
    skip: "Saltar al contenido";      // required when lang is not English
}
```

It writes `<html lang>`, `<title>`, `description`, `og:title` (the title), `og:description`, `og:type` (website), `og:url` (the canonical address), the four
`og:image` tags, `twitter:card` (`summary_large_image` with an image, `summary` without), `canonical`, `color-scheme`, the icons and the alternates, in that
order. **The head `examples/page/page.cui` builds is, byte for byte, the head `cancho-www` writes by hand** (`tests/fixtures/page/cancho-www-head.html`; the
`cargo test` compares the text, `check-page.js` compares the elements and their attributes in Chromium): the one difference is that the compiler's own
`<meta charset>` and `<meta viewport>` come first, which the fixture does not include. A program with no `page` builds exactly the head it built before, and its stylesheet: the rules of the skip link and of a sticky header are written only for a program that declares a `page` or has a sticky header (the 13 pages of the gallery are byte-identical to a fresh build, `check-gallery.js` R2).

| tag | refuses |
|---|---|
| `page-duplicate` | a second page, in the file or in an imported module |
| `page-entry-unknown`, `page-entry-duplicate`, `page-entry-type` | a setting the page does not have (or extra pairs after a one-value setting); a setting twice; a number for text or text for a number |
| `page-title-missing` | no title, or empty, over 120 characters, or on two lines |
| `page-description-length` | a description outside **50 to 300** characters, or a social description outside 20 to 300 |
| `page-lang-invalid` | not a language tag (`en`, `es`, `pt-BR`) |
| `page-canonical-not-https` | no canonical address, or not `https://host/..`, or with a `#`, a space, a quote or an angle bracket |
| `page-image-not-https`, `page-image-size-missing`, `page-image-size-invalid`, `page-image-alt-missing` | an image beside the program (a preview fetches it from outside); a width or a height missing (or either with no image); outside 1 to 8192; no or empty alt |
| `page-link-invalid` | an icon or alternate that is neither `https://` nor a file beside the program (no `..`, no leading `/`, no empty segment, no scheme) |
| `page-alternate-invalid` | not exactly one `mime:` (a media type) and one `title:`, another pair, or more than eight |
| `page-skip-text-missing`, `page-skip-text-invalid` | a page that is not English with no `skip:`; a `skip:` that is empty, over 60 characters or on two lines |
| `page-main-missing`, `skip-target-missing` | a program with a page and no `Main`; a `Main` with no `id:` |
| `page-file-missing`, `page-file-too-large` | a file an icon or alternate names that is not inside the program's directory; over 5 MB |
| `header-sticky-invalid`, `header-sticky-duplicate` | `sticky:` that is not `true` or `false`; two sticky headers |

**The description bound (corrected).** 50 to 300, not 50 to 160. The real description is 261 characters. The ceiling is against stuffing; **no number was
measured for any search engine's snippet** (there is no network access here), and nothing claims one.

## 3. The files a page names

An icon or an alternate that is a file beside the program is checked (`page-file-missing`, `page-file-too-large`) and **copied into the build at the path the
page uses** (`logo-mark.png`, `data/ecosystem.json`), as pictures are (`structure.md` 10.5). `og:image` is an address, never a file: nothing is copied for it.
Text from the program that goes into the head is escaped, and `@` becomes `&#64;`, so that the page template's own tokens cannot be replaced inside a title
(checked: a title that contains `@TITLE@` and `@PAGE_META@` is shown as written, once, in a browser).

**Found on the way, and fixed:** a program named without a directory (`cancho-ui build page.cui` run in its own directory; its parent is `""`) could not
find the files beside it: the check for a file inside the program's directory canonicalised `""`, which fails, so every `Image`, every island file and every
page file was refused as missing. It had been hidden because every test and script passed an absolute path. `pipeline::real_dir` resolves it; a test builds
from the program's own directory.

## 4. The skip link

A program with a `page` gets a link `Skip to content` (or `skip:`), **the first element of the component the page mounts**, going to the `Main`'s `id:`. It is
written into the IR (`page::apply`), so the prerendered page, the hydrated one and `inspect ir` all have it, and it is drawn by the baseline stylesheet above the
screen until it has focus (`top: -4rem`; `:focus` brings it to `0.5rem`, over the header, 44 px high). It is not an event: replay does not see it. With a
theme it is `accent_fg` on `accent`, a pair the theme already checks. The words are English unless `skip:` is given, and a page whose `lang` is not English
must give them (`page-skip-text-missing`): a skip link in the wrong language is a defect the compiler can see.

Measured in Chromium (`check-page.js` B): the skip link is the first link, entirely above the screen until focused; the first Tab focuses it, shows it
**44 px** high at **6.52:1** (light) and **7.26:1** (dark); Enter takes the page to `#main`, below the header; the same with script off.

## 5. `Header { sticky: true }`

The header gets the class `cui-sticky`. **Measured before the design was settled:** the example's header is **88 px** at 1280, 768 and 390 and **132 px** at 320
(its nav wraps), and with the stylesheet's `5rem` (80 px) alone an anchor landed at 80 px against a header bottom of 88: **8 px of the target hidden under
the header**. So the padding cannot be a number written in the stylesheet.

* **Script on:** a small runtime sets `scroll-padding-top` on `<html>` to **the header's height + 16 px** and follows a resize (`ResizeObserver`; the same
  16 px of air the real page leaves). Measured: 104 px at 1280, 768 and 390; **148 px at 320**; a resize from 1280 to 320 changes the header from 88 to 132 px and
  the padding to 148 px, and an anchor still lands below it. All 8 anchors of the page land just below the header at every width.
* **Script off:** the header is **not sticky** (`@media (scripting: enabled)` holds the sticky rule). Without the script nothing knows the header's height; a
  header that scrolls away hides nothing. Measured with script off: the header is static, an anchor lands at 0 with the header above it.
* **Focus not obscured** (WCAG 2.2, 2.4.11): `scroll-padding` also applies when Tab scrolls a focused control into view. Measured at 320 px: tabbing through the
  page focused 19 controls outside the header and none was under it.
* The 5rem stays in the stylesheet only for the first paint of a page whose script has not run yet, and for a browser that has `:has` and `scripting` but has
  not run the module.

## 6. Authority, `diff`, replay

A page has **no authority**: `requires` is unchanged, `inspect effects` is unchanged, and `diff` reports `authority: unchanged` for any change to it. A page change is
a change of the program, so `diff` gives one line per setting (`~ page title: a -> b`, `+ page alternate.agents.md = agents.md`, `- page ..`), counts them in
`change_count`, and exits 5; two equal pages print nothing and the JSON has no `page` key. `inspect ir` has a `page` object of the same settings. The skip link is
a `Link` in the mounted component's view, so adding a `page` to a program also shows as a view change of that component. Replay (`cancho-ui test`) runs
`examples/page` with its trace and does not mention the skip link.

## 7. Checked

* `crates/cancho-ui/tests/page.rs` (15 tests) over `tests/page_corpus.json` (11 valid programs, 53 refusals reaching all 22 tags, each documented in
  `spec/diagnostics/README.md`): every refusal by its tag; the head equal to the real head; no page, no change to the head; hostile text; the skip link first and
  in the page's language; the sticky class, CSS and runtime, and none for `sticky: false`; `inspect ir` and `diff`; replay; a program cut at every fifth character is
  refused and never a panic; the program-directory fix.
* `scripts/check-page.js`, Chromium: sections 1, 4 and 5 above plus no request to any origin but the page's and no console error.
* `scripts/breaks/page.json`: **57 deliberate breaks, every one caught by a check failing on its own** (COUNTS). **One was missed on the first run:** the twitter card could always be `summary_large_image` and nothing noticed, because no test built a page without
  an image; `a_page_with_no_image_is_a_summary_card` was added and the break is caught. One break is not in the file because it is not a difference: removing the
  leading-`/` test changes nothing, since the empty first segment of `/logo.png` is refused by the empty-segment test first.

## 8. Not measured, not built

* **Safari and Firefox, and any real device.** `:has()` and `@media (scripting: enabled)` are what the stylesheet relies on; they are in current Chromium (measured) and
  are believed to be in current Firefox and Safari, **not verified here**. In a browser without `scripting` the header is simply not sticky: the failure is a
  header that scrolls away, never an anchor hidden under one.
* **A screen reader reading the skip link.** Counted in the accessibility tree and tabbed in Chromium, not heard.
* **A link preview.** The `og:*` and `twitter:*` tags are the ones the real page writes, compared as text and as elements; no scraper was run.
* **`hreflang` alternates, `theme-color`, JSON-LD, `robots`**: not settings. `agents.md` and `llms.txt` stay written by the Python script (`www.md` 10, decision 4).
* **A changed icon or alternate file is not a `diff` line** (pictures are hashed in `assets`; these are not). The path is.
* **The skip link in a program whose mounted component's root is not an element** is wrapped in a `Column`; no test builds that shape.
* **`build` without `--page`** writes only JavaScript: there is no head and no stylesheet, so no page, skip-link CSS or sticky rule; the runtime is still emitted.
