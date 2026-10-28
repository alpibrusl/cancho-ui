# Status of the port of the cancho-www home page (B3)

**This is a status, not a recommendation to replace the page, and nothing here says the port is ready to.** Whether the page is replaced is the user's decision, and this document says what the decision would be made on. Every number is measured by a script in `scripts/` against cancho-www `4288356` (see the links); the machine, the number of runs and the spread are in [`cost.md`](cost.md).

## What is equal

Measured by [`parity.md`](parity.md) (`scripts/parity-www.js`, text, links, headings, landmarks, ids, anchors, accessibility roles and the page with JavaScript off, desktop and Pixel 7 profile, reduced motion): **126 differences, every one classified, none a defect** (the script exits 1 on one that is not). In particular:

* the **text** of every section: the same sentences, in the same order, except for the rows of `dropped.md` (the data, the 17 claims, 9 path entries, 24 projects, the three numbers and the date are the data file's, and a check compares them);
* the **outline** (one h1, no skipped level; the page's own h3-under-h1 is an h2 in the port), the landmarks, every `#anchor` the page links to, 84 of the page's 85 distinct link targets;
* the **fonts** (three, self-hosted, SIL OFL, with hashes), the **palette** in light and dark, the pictures and videos (the same files, byte for byte), the head (13 metas, 2 alternates, skip link);
* **JavaScript off**: lists, claims, projects, final numbers, waffles, pictures, videos, links, and the comparison table;
* the **map**: the page's own drawing code, the same 24 rocks and arrows, filters, trails, count, detail and list ([`dropped.md`](dropped.md) 13 to 15 for what is not the same);
* layout shift (CLS 0.0000 to 0.0001 on both), and what the page asks the network for: only its own origin.

## What differs on purpose

[`dropped.md`](dropped.md), 15 rows, each with what the page does, what the port does, why and how to undo it, and each a decision the user can overturn: the human/agent switch (1), icons and the logo in links (2), the inline header links and the menu (3), the pull quote (4), the pane as part of the program (5), the numbers of the trust chain (6), the comparison table as a real table (7), the h3s under the h1 (8), hover effects (9), **the look** (10: the compiler's baseline with the page's palette and fonts), the authority animation's stepper and dots (11), the drawings as sealed frames, the cairns not settling (12), the map's detail pane, the typed sessions, the hiker and the charts (13 to 15).

The look (row 10) is the largest difference and is **a choice of the first plan, not a finished design**: [`pixel.md`](pixel.md) compares the two at 360, 412, 768, 1024, 1280 and 1536 px, light and dark, section by section (15 sections, 180 comparisons). A section differs in **6 % to 82 % of its pixels** (a header 6 to 15 %, the hero 30 to 69 %, most sections 20 to 50 %, the agents section 35 to 82 % because of a picture the page uses as a banner), because the layouts are not the same: no card chrome, bordered default buttons, tighter vertical rhythm, content that fills the width with a 16 px gutter where the page centres a 1112 px column and puts the hero beside its text. Each section has a class and a reason in [`pixel-rules.json`](pixel-rules.json), after looking at the crops it names; the contact sheets are in [`pixel/`](pixel/) (12 WebP files, 2.6 MB together, low quality on purpose: they show where, not what; the script regenerates full-size crops with `--crops=<dir>`, which are not committed).

**The pixel diff found three defects of the port, fixed in B3** and now held by `check-www-port.js` and three deliberate breaks (`scripts/breaks/www-port-b3.json`): the map's **legend was clipped** by the island's aspect box (drawn, in the DOM, invisible); the map's **region names were 12 px mono** instead of the hand font at 40 px (a generic rule shadowed the page's); and the **agents pane showed the whole `agents.md` as text, 7,000 px tall**, where the page's pane is a bounded box that scrolls (the whole page was 18,300 px tall at 1280 against the page's 12,500; it is 12,600 now). It also found a defect **of the page**: at 1024 px wide the page scrolls horizontally (the port does not). That is the page's, not touched.

## What regressed (measured; [`cost.md`](cost.md): 15 runs per cell, interleaved, median and [min to max], 4 x Xeon 2.1 GHz, headless Chromium 141, local server, no throttling)

| | page | port |
|---|---|---|
| bytes of a load | 674.6 kB | **910.6 kB** (+35 %) |
| HTML | 48.2 kB | **237.5 kB** |
| script | 62.1 kB (five files) | **276.1 kB** |
| JS heap (Pixel 7 / desktop) | 1.95 / 1.92 MB | **7.3 / 7.2 MB** (3.8 times) |
| DOMContentLoaded (Pixel 7 / desktop) | 291 / 264 ms [190 to 339] | **409 / 427 ms** [355 to 506] |
| load (Pixel 7 / desktop) | 430 / 422 ms [357 to 493] | **509 / 546 ms** [441 to 605] |
| the 24th map rock exists (Pixel 7 / desktop) | 323 / 306 ms [222 to 371] | **377 / 397 ms** [325 to 475] |

**These timings were re-measured on 2026-10-28 against cancho-www `4288356`, on a slower moment of the same machine** (the page's own DOMContentLoaded was 291 ms against 205 ms in the earlier run): compare the ratios and the gaps, not the absolute milliseconds with the earlier documents; the sizes did not move (only the card and the domain changed).

What is **better** in the same measurement: requests 8 against 16; FCP and LCP 88 to 100 ms against 204 to 212 ms (the port's HTML paints before any script); long tasks beyond 50 ms (TBT-ish) 21 to 24 [10 to 59] ms against 86 to 87 [49 to 133]; DOM nodes in the document 1,321 against 2,955 (the drawings are in 15 frames, which are not counted). The causes, where they are known: the 15 sealed frames each carry a document (the page's `rocks.js` is in 12 of them) and cost the bytes and the heap; the `For` over data is unrolled at build time, which is most of the script's growth (the four trail panels alone are about 48 kB) and a design to change, not tried. **These are the numbers of this machine and a local server: no network, no device, no throttling**, so a mobile profile on a real connection would weigh the bytes more than this does.

## Open defects and gaps (not decided, not hidden)

1. **The map's detail pane is not a live region**: a screen reader is not told when a rock is chosen (the language has no `live:` on `Aside`/`Section`); `dropped.md` 13.
2. **The look** (above): not a defect of correctness, a gap of design; the layout rows of the baseline (hero beside its text, a centred column, cards) are what a visitor sees first.
3. **The map is an open island**: plain JavaScript in the page's origin, authority UNBOUNDED; the reach battery in the real island succeeds 29 of 33 times (`scripts/check-www-reach.js`). The one open trust boundary of the port; everything else is sealed or the program's own.
4. **Cairns do not settle when seen, the card drawing uses the system monospace font, the legend and the map are absent without JavaScript** (`dropped.md` 12, 15).
5. **Not measured at all**: Safari, Firefox, a real phone, a screen reader (NVDA, VoiceOver, TalkBack), a real network, a cold cache over HTTP/2, print, forced colours, zoom to 200 %, right-to-left; the contact sheets show nothing about hover or focus rings.
6. **The pictures are AI-generated** (Mistral AI) and supplied by the project owner: **not a permissive licence**; the owner is responsible for the generator's terms permitting this use, in the page and in the port alike ([`THIRD_PARTY.md`](THIRD_PARTY.md); the page says on its face that they are generated). The fonts are SIL OFL 1.1 and were checked.
7. **The port's source** is `site/home.cui` (about 800 lines; edit that file: a hash in it must be updated when an island, data or font file it names changes, and the compiler says which). The compiler changes the port needed (`List`/`Item`, `pressed:`, `data.value` and its use as a prop, `Table` prerender, sealed and open islands) are cancho-ui's now and are in its gate.

## What would be needed before anyone considers replacing the page

None of this is done; this is the list, not a claim.

1. **A decision on the look**: either accept the baseline look (dropped.md 10), or design the handful of roles that carry the page's (a `Card`, a centred column and a two-column hero, a header arrangement) in cancho-ui first, and re-run the pixel diff. The 15 section classes in `pixel-rules.json` are the work list.
2. **A decision on each row of `dropped.md`** by the user (15 rows), and on the regressions above: whether 910 kB, 7 MB of heap and about 120 to 160 ms more to DOMContentLoaded (and 80 to 125 ms more to load). are acceptable, or whether the frames' documents are shared or built when first near the screen (`islands.md` 4.5, designed, not built) and the `For` over data stops being unrolled.
3. **The live region** for the detail pane, and a screen-reader pass on the whole page (the map, the stepper, the pane, the tabs) with at least one real screen reader; the accessibility tree has been compared, not listened to.
4. **Browsers and devices**: Firefox and Safari (the sealed frames, `srcdoc`, CSP `frame-src about:` and the fonts are the risks), one real phone on a real connection, and a throttled run of `scripts/measure-www-port.js`.
5. **The open island's review**: the map's drawing code is the page's own, which nobody has reviewed for this purpose; either accept UNBOUNDED for one named file with its hash, or try the sealed map (filters and choice as props and events; not tried).
6. **Ownership of the generated files**: the page's `data/*.json` and the agents files are generated by the page's own scripts (`scripts/record_demos.py` and others) and are copies here (`THIRD_PARTY.md`); they would have to be generated into the new place, with their hashes updated in `home.cui`, or the program stays in step with the page by hand.
7. **The licence question of the pictures** answered by the owner, in writing, in `THIRD_PARTY.md`.
8. **A rollback**: the port is under `docs/www-port/` and builds with `cancho-ui build docs/www-port/site/home.cui --page`; nothing deploys it, and nothing in this repository touches the page's repository.

## Serving the port beside the live page (a preview at `/next/`; notes of 2026-10-28, cancho-www `4288356`)

* **Build**: `cargo build -p cancho-ui --release`, then `target/release/cancho-ui build docs/www-port/site/home.cui --page -o <dir>`. Measured on this machine (4 cores, cold, `CARGO_TARGET_DIR` empty): **34 s**, 69 MB of `target`, a 4.3 MB binary; the port builds in 0.13 s. `Cargo.lock` names 14 packages from the registry (`serde`, `serde_json`, `thiserror` and what they pull in), so a clean CI job needs crates.io or a cache; this 34 s used the registry cache already on the machine and a cold CI job, which also fetches them, measured 1 m 20 s.
* **The output is relocatable**: measured, no `href`, `src`, `srcset` or `poster` of `index.html` starts with `/`, and `app.js` fetches only relative paths (`agents.md`, `data/ecosystem.json`, `llms.txt`), so the directory can be served at `/next/`.
* **Canonical**: `page { canonical: "https://cancho.dev/"; }`, the live page's own. A preview that named itself canonical would ask search engines to index `/next/` as the home page; naming the live page makes `/next/` a duplicate that points at the original (`og:url` follows it, so a link shared from the preview shows the live address, and `og:image` is `https://cancho.dev/og.png`, which exists). The agent files carry the live page's absolute `cancho.dev` URLs, byte for byte, as the page's do.
* **`noindex`**: `page { }` **cannot express** a robots meta (its keys are `title`, `lang`, `description`, `share_description`, `canonical`, `image`, `icon`, `touch_icon`, `alternate`, `skip_link`; `docs/page.md` section 3). Use `robots.txt` (`Disallow: /next/`) in cancho-www, or an `X-Robots-Tag` header; a `Disallow` stops crawling but not indexing of a URL linked from elsewhere, which the canonical above covers.
