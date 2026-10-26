# Status of the port of the cancho-www home page (B3)

**This is a status, not a recommendation to replace the page, and nothing here says the port is ready to.** Whether the page is replaced is the user's decision, and this document says what the decision would be made on. Every number is measured by a script in `scripts/` against cancho-www `7578537` (see the links); the machine, the number of runs and the spread are in [`cost.md`](cost.md).

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
| bytes of a load | 674.8 kB | **910.7 kB** (+35 %) |
| HTML | 48.2 kB | **237.6 kB** |
| script | 62.1 kB (five files) | **276.1 kB** |
| JS heap (Pixel 7 / desktop) | 1.97 / 1.97 MB | **7.3 / 7.2 MB** (3.7 times) |
| DOMContentLoaded (Pixel 7 / desktop) | 205 / 209 ms [152 to 266] | **302 / 311 ms** [270 to 350] |
| load (Pixel 7 / desktop) | 311 / 316 ms [260 to 420] | **370 / 391 ms** [338 to 435] |
| the 24th map rock exists (Pixel 7 / desktop) | 227 / 232 ms [177 to 314] | **281 / 286 ms** [249 to 328] |

What is **better** in the same measurement: requests 8 against 16; FCP and LCP 72 ms against 164 ms (the port's HTML paints before any script); long tasks beyond 50 ms (TBT-ish) 0 [0 to 15] ms against 41 to 48 [19 to 99]; DOM nodes in the document 1,321 against 2,955 (the drawings are in 15 frames, which are not counted). The causes, where they are known: the 15 sealed frames each carry a document (the page's `rocks.js` is in 12 of them) and cost the bytes and the heap; the `For` over data is unrolled at build time, which is most of the script's growth (the four trail panels alone are about 48 kB) and a design to change, not tried. **These are the numbers of this machine and a local server: no network, no device, no throttling**, so a mobile profile on a real connection would weigh the bytes more than this does.

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
2. **A decision on each row of `dropped.md`** by the user (15 rows), and on the regressions above: whether 910 kB, 7 MB of heap and about 100 ms more to DOMContentLoaded (and 55 to 75 ms more to load) are acceptable, or whether the frames' documents are shared or built when first near the screen (`islands.md` 4.5, designed, not built) and the `For` over data stops being unrolled.
3. **The live region** for the detail pane, and a screen-reader pass on the whole page (the map, the stepper, the pane, the tabs) with at least one real screen reader; the accessibility tree has been compared, not listened to.
4. **Browsers and devices**: Firefox and Safari (the sealed frames, `srcdoc`, CSP `frame-src about:` and the fonts are the risks), one real phone on a real connection, and a throttled run of `scripts/measure-www-port.js`.
5. **The open island's review**: the map's drawing code is the page's own, which nobody has reviewed for this purpose; either accept UNBOUNDED for one named file with its hash, or try the sealed map (filters and choice as props and events; not tried).
6. **Ownership of the generated files**: the page's `data/*.json` and the agents files are generated by the page's own scripts (`scripts/record_demos.py` and others) and are copies here (`THIRD_PARTY.md`); they would have to be generated into the new place, with their hashes updated in `home.cui`, or the program stays in step with the page by hand.
7. **The licence question of the pictures** answered by the owner, in writing, in `THIRD_PARTY.md`.
8. **A rollback**: the port is under `docs/www-port/` and builds with `cancho-ui build docs/www-port/site/home.cui --page`; nothing deploys it, and nothing in this repository touches the page's repository.
