# Page: the head of the document, a skip link, a sticky header, and the anchors it must not hide

> **Status: BUILT (2026-10-25), with its measurements (section 7); section 9 says what the build changed in this note.** Step 4 of [`www.md`](www.md) (5.3, "page-level"). It answers
> what is left open by `www.md` 5.3 and by [`structure.md`](structure.md) 2 ("not built in B1: the skip link ... `sticky`"), against the real cancho-www page (`index.html`, `assets/site.css`, commit
> `0579a80`; `index.html` is byte-identical at the later `7578537`, and differs at `4288356` only in the three head lines that name the site: canonical, `og:url` and `og:image`, now on `https://cancho.dev/`), read-only. Every number says how it was measured: Chromium (Playwright, desktop Chromium on Linux; the `Pixel 7` profile is a viewport and a
> user agent, not a phone), `scripts/measure-page.js` for the page, `wc`/`file` for the files.

## 1. What the real page does, measured

| | the page | measured by |
|---|---|---|
| head | 13 `meta` (charset, viewport, `description`, `og:title`, `og:description`, `og:type`, `og:url`, `og:image`, `og:image:width`, `og:image:height`, `og:image:alt`, `twitter:card`, `color-scheme`), 6 `link` (`canonical`, `icon`, `apple-touch-icon`, two `alternate`, the stylesheet), `<html lang="en">` | regular expressions over `index.html` |
| lengths | `<title>` 55 characters; **`description` 261**; `og:description` 111; `og:image:alt` 60 | same |
| `og:image` | an **absolute** `https://cancho.dev/og.png`, the file `og.png` at the site root: **1200 x 630** PNG, 351,549 bytes; `og:url` and `canonical` are `https://cancho.dev/` *(corrected 2026-10-28 for cancho-www `4288356`, which moved the site to cancho.dev; at `7578537` they were `https://alpibrusl.github.io/cancho-www/` and the card was 353,434 bytes)* | `file`, `ls` |
| icons | `logo-mark.png` (256 x 256 RGBA PNG) as `icon`, `logo.png` (256 x 256 RGB PNG) as `apple-touch-icon`: files of the site, relative | `file` |
| alternates | `agents.md` (`text/markdown`, 19,114 bytes) and `data/ecosystem.json` (`application/json`, 23,489 bytes), relative, each with a `title` | `ls` |
| CSP | none | no `Content-Security-Policy` in `index.html` |
| skip link | `<a class="skip" href="#main">` is the first element of `body`, hidden at `top: -4rem`, shown at `top: 8px` on `:focus`; `<main id="main">` has no `tabindex` | `index.html`, `site.css` |
| header | `header.top`: `position: sticky; top: 0`, `.wrap` 4rem high: **64 px at 1280 and at 412 px** (the nav is hidden under 62rem and a menu takes its place, which is how the page keeps one line) | Chromium |
| anchors | `html { scroll-padding-top: 5rem }` by hand: 22 in-page links, **9 distinct targets** (`main`, `top`, `try`, `trust`, `verifies`, `ecosystem`, `demo`, `agents`, `status`) | Chromium |

### 1.1 What `scroll-padding-top` is worth

Following each of the 9 targets in Chromium (`reducedMotion: 'reduce'`; the page's `scroll-behavior: smooth` otherwise delays the position), the target's top edge against the header's bottom edge (64 px):

| | desktop 1280 | `Pixel 7` |
|---|---|---|
| the page's own `scroll-padding-top: 5rem` | 9 of 9 below the header (tops 64 and 80 px) | 9 of 9 (tops 64 and 80) |
| `scroll-padding-top` removed | **0 of 9: every target at top 0, under the header** | **0 of 9** |

It is not only for fragments. Pressing `Shift+Tab` 120 times from the end of the page on the `Pixel 7` profile: with the page's padding **1** element ended under the header (a 350 px tall block taller than the room it was given); with the padding removed **23 of 120** did, among them links, buttons and the map's rocks. Tabbing forwards left none under the header in either case (the browser puts a focused element it scrolls to in the middle). So the padding is what WCAG 2.2's *Focus Not Obscured* (2.4.11) asks for, in Chromium, for the backward direction too.

### 1.2 The skip link in Chromium

The first `Tab` focuses the skip link at 8 px from the top, on both profiles. `Enter` sets the hash to `#main` and puts `main` at the header's bottom edge (64 px); `document.activeElement` is then `body` (`main` has no `tabindex`), **and the next `Tab` goes to the first link inside `main`**, not to the header's links: the browser starts sequential navigation from the target. So in Chromium the page does not need `tabindex="-1"` on `main`. Safari and Firefox were not tried (section 10).

## 2. What `www.md` 5.3 said, and what the page disagrees with

| 5.3 said | the page | decision |
|---|---|---|
| `page-description-length` outside 50..160 characters | the description is **261** characters; the shorter text is the one for social cards (`og:description`, 111). A 50..160 bound would refuse the page it was written for | **corrected in place**: one description of 1 to 300 characters, and an optional `share_description:` for the card (same bounds). 300 is not a measured limit of any search engine (none was measured); it is the smallest round number that admits the page's 261 |
| `page-image-size-missing` (the OG image's width and height declared) | the declared pair is `1200`/`630`, and the file is 1200 x 630: a number that must equal the file is a number the compiler can read | **replaced**: the compiler reads the size from the file (PNG, JPEG, WebP); `page-image-size-unreadable` if it cannot. Nothing to declare, nothing to get wrong |
| `image: ".."` as a URL | the card needs an **absolute** URL, and the file is a file of the site | `image:` is a **file** (relative, copied into the build, hashed for `diff`); the card's URL is the file's path joined to the directory of `canonical:` |
| `alternates: [...]`, `icon: ".."` | `alternate` is a `type`, a `title` and a file; there are two icons | `icon:` and `touch_icon:` are files; `alternate: "agents.md" title: ".."` is a file with a title and the media type its extension says (`md`, `json`, `txt`, `xml`) |
| `anchor-unknown`, `anchor-duplicate` | | already built in B1 ([`structure.md`](structure.md) 2) |
| `Header { sticky: true }` with `scroll-padding-top` "set by the compiler" | the compiler cannot know a header's height: it is its content's | see 4: a measured fallback of 4rem, replaced by the real height by a script that is in the page itself |
| "`diff` shows the metadata change", "authority: none" | `canonical` and the card image are absolute URLs on a host | the host is listed in `inspect effects` and `diff` as a `Link` host is; the page still makes **no request** to it (the browser does not fetch `canonical` or `og:image`; a crawler does) |

## 3. The declaration

```
page {
    title: "cancho: tools built for agents, not adapted from humans";
    lang: "en";
    description: "cancho is a programming language and toolchain for software built by AI agents and checked by machines. Pre-1.0.";
    share_description: "Trust without comprehension: a language where what code may do is in its type.";
    canonical: "https://cancho.dev/";
    image: "og.png" alt: "cancho: Tools built for agents. Trust without comprehension.";
    icon: "logo-mark.png";
    touch_icon: "logo.png";
    alternate: "agents.md" title: "cancho, as an agent reads it";
    alternate: "data/ecosystem.json" title: "The cancho ecosystem as data";
    skip_link: "Skip to content";
    measure: "72";
    scale: "display";
}
```

A top-level declaration like `theme { }` (`page` is a keyword only at the top level, followed by `{`), one per program (`page-duplicate`), in the entry file. Each entry is `key: "string"` pairs ended by `;`: the first pair is the entry, the others are its attributes.

| entry | becomes in `<head>` / `<body>` | required |
|---|---|---|
| `title:` | `<title>`, `og:title` | yes |
| `lang:` | `<html lang>` | yes |
| `description:` | `<meta name="description">`, and `og:description` unless `share_description:` | no |
| `share_description:` | `og:description` | no |
| `canonical:` | `<link rel="canonical">`, `og:url` | no |
| `image:` + `alt:` | `og:image` (absolute), `og:image:width`, `og:image:height` (read from the file), `og:image:alt`, `twitter:card` = `summary_large_image` | `alt:` with it; `canonical:` with it |
| `icon:`, `touch_icon:` | `<link rel="icon" type="..">`, `<link rel="apple-touch-icon">` | no |
| `alternate:` + `title:` | `<link rel="alternate" type=".." href=".." title="..">` | `title:` with it |
| `skip_link:` | the skip link, the first element of `<body>`, to the `Main`'s `id` | no |
| `measure:` | **added 2026-10-29** ([`www-port/look.md`](www-port/look.md)): the width in rem of the page's centred column, a whole number from 30 to 100 as a string. The page's `header`, `main` and `footer` are centred in it with a gutter of 1.25 rem (20 px) that is never less on a narrow window; the sticky header keeps its full-width background. Written as a `<style id="cui-look">` after the baseline and before the theme; there is none when neither `measure` nor `scale` is written | no |
| `scale:` | **added 2026-10-29**: `"display"`, the one value: h1 `clamp(2.1rem, 4.6vw, 3.05rem)` at 800, h2 `clamp(1.6rem, 3.4vw, 2rem)`, h3 and a card's heading 1.05 rem, the `Em` of a heading upright and in the accent colour, line height 1.6, 5 rem above a section, a lead of 1.125 rem and at most 34 rem | no |

Always, with a `page`: `og:type` = `website` and `<meta name="color-scheme" content="light dark">` (the page has both; the second restates what the baseline stylesheet already sets, so it is there for parity and its effect before the stylesheet loads was **not** measured). Everything is written into `index.html` by the build, **in the static HTML, before any script**; `title` replaces `--name` (which stays the fallback for a program with no `page`).

Authority and the network: a file is a file of the program's directory (the rules of `Image`: relative, inside the directory, no `..`, no address, at most 20 MB, the format its extension says), is copied into the build, and its SHA-256 is in the IR so that `diff` sees a changed picture. **No new network access**: nothing in the head is fetched by the page from another host (`icon` and `alternate` are relative; `canonical` and the card image are named for others to read). The one host the metadata names is listed: `inspect effects` has `"page_hosts"`, `inspect page` prints the whole resolved head, and `diff` prints `+ page host ..`.

## 4. The sticky header and the skip link

* **`Header { sticky: true }`** becomes `<header class="cui-sticky">`: `position: sticky; top: 0`, above the page, on the theme's `bg` with the `line` under it. `sticky:` is a literal `true` or `false` (`header-sticky-invalid`); one per program (`header-sticky-duplicate`); not inside a `Section`, `Article`, `Aside`, `Main`, `Nav`, `Footer`, `Figure`, `Details` or another `Header` (`header-sticky-misplaced`: such a header is not the banner, and it would stick only inside its parent). The look of the page's header (the blur, the translucent `color-mix`) is **not** offered: a translucent bar over text is a contrast risk the compiler would then have to check.
* **Anchors are not hidden**: the page gets `html { scroll-padding-top: calc(var(--cui-header-h, 4rem) + 1rem) }`. A small inline script **in the page's own HTML** (no `app.js` needed, it runs before the module is fetched) sets `--cui-header-h` to the header's measured height at load, on resize and when a font arrives. Without script the fallback holds exactly while the header fits in 4rem (the page's header is 4rem and its own padding is `5rem`, the same number). Section 7 measures the header that is taller than that, with and without the script.
* **`skip_link:`** is the first element of `<body>` (before `#root`, so before the prerendered content and before any script), `<a class="cui-skip" href="#main">`; it is visible only while focused, and then above the sticky header. It needs a `Main { id: ".." }` that is **in the prerendered HTML** (`skip-link-target-missing`: no `Main`, or one without an `id`; `skip-link-target-dynamic`: a `Main` inside an `If` or another component (inside a `For` an `id` is already `anchor-duplicate`), which is not in the page until a script has run, and a link to nothing is the first thing a keyboard user would find). A `Main` that has an `id` also gets `tabindex="-1"` and no outline when focused: it is what Safari and Firefox need to start from the target, and it is **not** needed in Chromium (1.2), so its only measured effect is that Chromium makes `main` the active element and goes on to the same next link.

## 5. Refusals

| tag | refused |
|---|---|
| `page-duplicate` | a second `page { }` in a file, or one in the entry and one in an imported module |
| `page-entry-unknown` | an entry that is not one of `title`, `lang`, `description`, `share_description`, `canonical`, `image`, `icon`, `touch_icon`, `alternate`, `skip_link`, `measure`, `scale`; an attribute that its entry does not take (`alt` on an `icon`, `title` on an `image`) |
| `page-entry-duplicate` | a second `title`, `lang`, `description`, `share_description`, `canonical`, `image`, `icon`, `touch_icon`, `skip_link`, `measure` or `scale` |
| `page-title-missing` | no `title:`, or one that is blank |
| `page-lang-missing`, `page-lang-invalid` | no `lang:`; one that is not a BCP 47 shape (`en`, `es-419`, `zh-Hant-TW`: letters and digits in 1 to 8 character parts, a 2 or 3 letter language first) |
| `page-description-length` | a `description:` or `share_description:` that is blank or over 300 characters |
| `page-text-invalid` | a title, description, alt, alternate title or skip link text with a control character (a line break in a `<meta>` is a different meta) |
| `page-canonical-not-https` | a `canonical:` that does not start with `https://` |
| `page-canonical-invalid` | an `https://` address with no host, a user name, a space or control character, a `#` fragment, a host that is not lower case, or `<`, `>` or `"` |
| `page-image-alt-missing`, `page-image-needs-canonical` | an `image:` with no `alt:` (or a blank one); an `image:` with no `canonical:` to make its URL absolute |
| `page-file-invalid` | a path that is an address, absolute, with `..`, with characters outside letters, digits, `.`, `_`, `-`, `/`, or an extension the entry does not take (image: `png`, `jpg`, `jpeg`, `webp`; icon: `png`, `ico`, `jpg`, `jpeg`, `webp`, `gif`; alternate: `md`, `json`, `txt`, `xml`) |
| `page-image-size-unreadable` | an image file whose width and height cannot be read from its header |
| `page-alternate-title-missing` | an `alternate:` with no `title:` (a feed reader or an agent is shown the title, not the file name) |
| `page-skip-link-empty` | a blank `skip_link:` |
| `page-measure-invalid`, `page-scale-invalid` | a `measure:` that is not a whole number from 30 to 100 (a unit, a sign, a space, empty); a `scale:` that is not `"display"` (*added 2026-10-29*) |
| `skip-link-target-missing`, `skip-link-target-dynamic` | see 4 |
| `header-sticky-invalid`, `header-sticky-duplicate`, `header-sticky-misplaced` | see 4 |
| `asset-missing`, `asset-too-large`, `asset-type-mismatch` | the three existing tags, now also for the files of a `page { }` |

## 6. `diff`

`page` in the JSON of `diff` is a list of `{ setting, base, candidate }` (as `theme` is): `title`, `lang`, `description`, `share_description`, `canonical`, `canonical.host`, `image`, `image.alt`, `image.sha256`, `icon`, `icon.sha256`, `touch_icon`, `touch_icon.sha256`, `alternate agents.md` (its title), `alternate agents.md.sha256`, `skip_link`, `measure`, `scale`. Absent when equal. Human form `~ page title: .. -> ..`, `+ page host example.org`. It counts in `change_count` (exit 5) and **`authority` does not move**.

## 7. Measured in the build

`scripts/check-page.js` (Chromium, `Pixel 7` profile and desktop 1280, `examples/page`; 42 checks, all pass):

* **The head with JavaScript off**: language, title, description, the card's own text, `og:*`, `twitter:card`, `color-scheme`, canonical, icon, touch icon and both alternates are in the document; the card's `1200x630` equals the PNG header read by the check itself; every file the head names is served with a 200; no console error; **0 requests to another host** in the whole run.
* **Skip link** (script on, script off, desktop): the first focusable element, off the window (`bottom <= 0`) until focused, then inside it, above the sticky header (`elementFromPoint` is the link), 44 px high; `Enter` sets `#main`, the Main lands below the header, the Main is the active element with no outline, and the next `Tab` is inside it. Contrast of the link: 6.52:1 (light) and 7.26:1 (dark) with the example theme (`accent_fg` on `accent`), 9.40:1 and 7.84:1 with no theme.
* **Sticky header and anchors**: `position: sticky`, `top: 0` after scrolling 1,500 px; `--cui-header-h` is its height (64 px) and `scroll-padding-top` 80 px; 6 of 6 in-page links and a page opened at `#anchors` (script on and off) put the target at 80 px, below the header; Shift+Tab through 19 focus stops from the end of the page leaves 0 under the header.
* **A header that wraps** (14 links at 320 px): 221 px high. With the script **0 of 14** targets are under it. **Without the script, 13 of 14 are**: the 4rem fallback holds only for a header that fits in 4rem, as section 4 says. This is a limit, measured, not removed.
* **Nothing is touched**: with `app.js` held back 1.5 s the skip link, the header, the Main and the head are the same nodes after the script ran.
* A program with no sticky header has `scroll-padding-top: auto`, a static `header` and no measurement.

`cargo test --test page` (14): every one of the 11 valid programs and 91 refusals of `tests/page_corpus.json` by its tag; the 24 tags (21 new, `asset-*`, `anchor-duplicate`) documented in `spec/diagnostics/README.md` and each reached; a program cut at every prefix and with every character deleted is a refusal and not a panic; the head as text; escaping and tokens in a title; `--pwa` keeps the page's icons; the size of PNG, JPEG, lossy and lossless WebP read from the header (Pillow wrote the fixtures); `inspect`, `diff` and an import with a second page.

`scripts/breaks/page.json`: 38 breaks, each caught by at least one check failing on its own (none `HUNG`, `BUILD`, `BAD`). **The first run missed 2**: a user-name check in `canonical:` that no input could reach (a `@` is not a host character, so the host check refused it first: the dead check was deleted, and so was its break), and a quote in `og:title` written raw (the test escaped-text case only looked at `<title>`; it now looks at `og:title`).

## 9. What the build changed in this note

* `description` bound corrected from 50..160 to 1..300 (2); the card size is read from the file (2); a `For` around a `Main` with an `id` is `anchor-duplicate`, not `skip-link-target-dynamic`, which is reached by an `If` and by another component; `sticky:` read from state is a parse refusal (a binding value there is a literal), so `header-sticky-invalid` is a number or text.
* The page's text is substituted in one pass (`expand`): before it, a `--name` containing `@THEME_CSS@` would have been replaced by the stylesheet (found while writing the escape test; not measured on `main`).
* The 13 gallery pages embed the baseline stylesheet, which gained the page rules, so `docs/demo/*/index.html` were rebuilt (`gallery_demos` failed until then).
* `examples/page`, 32 programs and 27 replaying a trace now (`docs/index.html`, `check-gallery.js`).

## 10. Not measured

A header that wraps **without** the script (section 7: 13 of 14 hidden; not fixed); a sticky header that a program builds in a component other than the mounted one (the script looks again at `DOMContentLoaded` and `load`; not tested); `tabindex="-1"` on the Main in Safari and Firefox; Safari, Firefox and a real phone (the skip link's focus, `scroll-padding` for focus, `position: sticky`); a screen reader announcing the skip link; what a search engine or a link-preview fetcher does with the head (nothing was fetched from outside this machine); any limit on the length of a description other than the page's own 261; whether `color-scheme` as a `<meta>` changes the first paint.
