# docs/www-port: the home page of cancho-www, written in cancho-ui

**Status: live at `cancho.dev/` since the cancho-www pull request `ccr-go-live` (the user's decision; the previous page is at `/classic/`, [`status.md`](status.md) "Live"). B1, the `Table` prerender, B2a, B2b and B3 are in: the content, head, theme, fonts, pictures, videos, data, the agents pane, the hero, the ten cairns, the three card drawings, the authority animation and the ecosystem map are ported and measured, against the real page: text, links, outline, accessibility tree, JavaScript off ([`parity.md`](parity.md)), bytes, requests, FCP, LCP, CLS, long tasks and heap over 15 runs ([`cost.md`](cost.md)), and pixels at six widths in light and dark ([`pixel.md`](pixel.md)). [`status.md`](status.md) says what is equal, what differs on purpose, what regressed, what is open, and what would be needed before anyone considers replacing the page. The replacement was the user's decision, not a claim of these documents that the port is ready: what is not checked (Firefox, Safari, a real phone, a screen reader) is listed there.**

| file | what |
|---|---|
| [`site/home.cui`](site/home.cui) | the page: `page { }` (13 metas, 2 alternates, skip link), `theme { }` (the page's 18 colours in both schemes, three self-hosted fonts), `AgentFiles` (the pane) and `Home` (everything else); the lists the page's Python generator writes are `For`s over `site/data/ecosystem.json` |
| `site/` | the files the page names: `fonts/` (3 woff2 + 3 OFL texts), `assets/img/` (4 pictures x webp and jpg), `assets/media/` and `demo/` (2 recorded sessions, posters, `demo.cast`), `data/ecosystem.json`, `data/demos.json`, `islands/` (the island files, the map's among them), `agents.md`, `llms.txt`, `llms-full.txt`, the logo and the card image. All tracked; copies of cancho-www `4288356`, byte for byte |
| [`THIRD_PARTY.md`](THIRD_PARTY.md) | where each file is from and its licence (fonts: SIL OFL 1.1, checked; pictures: AI-generated, **owner's responsibility**) |
| [`dropped.md`](dropped.md) | **what the port drops or changes on purpose**, row by row, each with how to undo it |
| [`gaps.md`](gaps.md) | PR A: what cancho-ui lacked, and the judgment calls |
| [`status.md`](status.md) | **start here**: equal, different on purpose, regressed, open, and what a replacement would need |
| [`look2.md`](look2.md) | round two of the look: the header, the cards the language could not name, the hero's overlay, the `agents` banner; mean pixel difference 29.7 % to 26.6 % |
| [`look.md`](look.md) | the roles of the look (`card:`, `align:`, a pill link, a header end group, `page { measure; scale }`): designed from a measurement, built, and measured again | 
| [`pixel.md`](pixel.md), [`pixel-rules.json`](pixel-rules.json), [`pixel/`](pixel/) | the pixel diff at 360, 412, 768, 1024, 1280 and 1536 px, light and dark, and the contact sheets (`scripts/pixel-diff-www.js`) |
| [`cost.md`](cost.md) | bytes, requests, timings and heap against the page, 15 runs, median and spread (`scripts/measure-www-port.js`) |
| [`parity.md`](parity.md), [`parity-rules.json`](parity-rules.json) | the measured differences against the real page, each classified; written by `scripts/parity-www.js` |

Build: `cancho-ui build docs/www-port/site/home.cui --page -o dist`. Checks: `node scripts/check-www-port.js` (in the gate; needs nothing outside this repository) and
`node scripts/parity-www.js <cancho-www clone> [--write]` (not in the gate: it needs the clone).

## Measured (B1 to B2b)

* **Parity** ([`parity.md`](parity.md)): 126 differences between the page and the port in text, links, headings, landmarks, ids, anchors, accessibility-tree roles and the page with JavaScript off, at 1280 px and on the Pixel 7 profile
  with reduced motion; **every one has a class** (intentional 126, open defects 0; B2b resolved or reclassified the 18 rows that had been deferred to it: see the rows of `dropped.md` 13 to 15) and a reason; the script exits 1 if one has none. The visible text of every section is the same except
  for the rows listed. The 17 claims, 9 path entries, 24 projects, the 3 numbers and the date are the data file's (a check reads the file and compares).
* **Without JavaScript** the port has its lists, claims, projects, final numbers, waffles, pictures, videos and links to files (each answers 200 with the bytes of its file); the page's numbers read 0 there.
* **Authority**: `Home` requires `Clipboard` (the copy buttons) and `Dom` (the map) and nothing else; `AgentFiles` requires `Network` and nothing else (its `net.get` can only reach this site); the data is named by its SHA-256;
  every request of a load is to the page's own origin; the script holds no address of a host it contacts. **Trust boundaries**: the 15 sealed frames (the hero, ten cairns, three card drawings and the authority canvas) reach nothing; the
  **map is an open island: plain JavaScript in the page's origin, `inspect effects` prints its authority as UNBOUNDED, and `scripts/check-www-reach.js` ran a battery of 33 attempts in the real map island: 29 succeeded**
  (it read `parent.document`, `localStorage`, cookies and IndexedDB, sent 12 requests to another origin, one to the page's own, and four STUN packets). The hosts the page links to are listed by `inspect effects`
  (`alpibru.com`, `alpibrusl.github.io`, `github.com`) and are links, not requests.

## Open defects of the port (not decided, not hidden)

1. ~~**With JavaScript off the comparison table is missing**~~ **Fixed** (the `Table` prerender PR, `prerender.md` section 12): `Table` is written into the page when its cells are literals or what a `For` over data wrote out, and is
   the script's, whole, when anything in it reads state. Measured: with no script the port has the table (1 region, 1 table, 3 column headers, 3 row headers in the accessibility tree), and `parity.md` has no row for it any more
   (the row "human tools, used by agents ..." of the page's JavaScript-off text is gone; the phone-width row is reclassified: the page hides its header row on a phone).
2. **Layout shift**: the three fonts are `font-display: swap` and not preloaded, so the text can move once when they arrive. Measured over 5 loads ([`cost.md`](cost.md)): median CLS **0.0000** (Pixel 7) and **0.0001** (desktop) for the port, 0.0000 and 0.0007 for the page; a single earlier load of the port measured 0.012 at 135 ms, so the number moves with the order in which the fonts and the first paint arrive. Not a defect at these values; named because it is not zero in every load.
3. **The cost of the frames** (numbers: [`cost.md`](cost.md), 15 runs per cell, median and spread; summary in [`status.md`](status.md)): the port's HTML, script, JS heap and DOMContentLoaded are larger than the page's, and its first paint and long tasks are better. 15 frames carry their document in `srcdoc` (the page's `rocks.js` is in 12 of them). **This is a regression in bytes, memory and time to load, and it is the price of sealing the drawings.** The fallback that was designed and not built (the document of a frame written when it is first near the screen, `islands.md` 4.5) would take the cairns' and the cards' share out of the first load.
4. **The cost of the map** ([`cost.md`](cost.md), "The map alone"): the 24th rock exists later than on the page (about 55 to 90 ms in the 15-run measurement; the cause is not isolated: the island's code is inside `app.js`, which is three times larger), the section has 664 elements against 639, a click on a rock shows its detail in 1 to 2 ms on both. The port's script grew by about 190 kB to carry a map the page does with 25.1 + 5.4 kB of script and two data files: the island's own files are 20 kB of that; the four trail panels are about 48 kB; the rest is by subtraction the 24 project panels, because a `For` over `data.items` is **unrolled at build time** into node-building code (`data.md` section 3). **This is a regression in script bytes caused by the unrolling, not by the map.** A `For` over data that stays a loop would probably remove most of it (not tried); that is a design.
5. **The detail pane is not a live region** (`dropped.md` 13): a screen reader is not told when a rock is chosen.
6. **The page is longer than the page**: 12,323 px against 12,532 at 1280 px (round two, [`look2.md`](look2.md); it was 12,955 after round one; 24,960 against 23,782 at 360). It was 17,453 px when this was written, 12,592 after the agents pane was bounded (B3) and is 12,955 since the roles of the look ([`look.md`](look.md)) put the page's section spacing back. [`pixel.md`](pixel.md) reports it, width by width.

## Not measured yet

Safari, Firefox, a phone, a screen reader; bytes, requests, FCP and LCP against the page over many runs and several widths (B3); pixels (B3). The open island under a hole (`islands.md` 13.1).
