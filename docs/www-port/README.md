# docs/www-port: the home page of cancho-www, written in cancho-ui

**Status (PR B1 of three): the content, head, theme, fonts, pictures, videos, data and the agents pane are ported and measured. The islands (hero, cairns, card drawings, authority animation) and the map are B2; the
byte, request and timing comparison and the pixel diff are B3. This is not a claim that the port can replace the page: that is the user's decision, and B1 is a third of the evidence.**

| file | what |
|---|---|
| [`site/home.cui`](site/home.cui) | the page: `page { }` (13 metas, 2 alternates, skip link), `theme { }` (the page's 18 colours in both schemes, three self-hosted fonts), `AgentFiles` (the pane) and `Home` (everything else); the lists the page's Python generator writes are `For`s over `site/data/ecosystem.json` |
| `site/` | the files the page names: `fonts/` (3 woff2 + 3 OFL texts), `assets/img/` (4 pictures x webp and jpg), `assets/media/` and `demo/` (2 recorded sessions, posters, `demo.cast`), `data/ecosystem.json`, `agents.md`, `llms.txt`, `llms-full.txt`, the logo and the card image. All tracked; copies of cancho-www `7578537`, byte for byte |
| [`THIRD_PARTY.md`](THIRD_PARTY.md) | where each file is from and its licence (fonts: SIL OFL 1.1, checked; pictures: AI-generated, **owner's responsibility**) |
| [`dropped.md`](dropped.md) | **what the port drops or changes on purpose**, row by row, each with how to undo it |
| [`gaps.md`](gaps.md) | PR A: what cancho-ui lacked, and the judgment calls |
| [`parity.md`](parity.md), [`parity-rules.json`](parity-rules.json) | the measured differences against the real page, each classified; written by `scripts/parity-www.js` |

Build: `cancho-ui build docs/www-port/site/home.cui --page -o dist`. Checks: `node scripts/check-www-port.js` (in the gate; needs nothing outside this repository) and
`node scripts/parity-www.js <cancho-www clone> [--write]` (not in the gate: it needs the clone).

## Measured in B1

* **Parity** ([`parity.md`](parity.md)): 124 differences between the page and the port in text, links, headings, landmarks, ids, anchors, accessibility-tree roles and the page with JavaScript off, at 1280 px and on the Pixel 7 profile
  with reduced motion; **every one has a class** (intentional 80, deferred to B2 42, open defects 2) and a reason; the script exits 1 if one has none. The visible text of every section is the same except
  for the rows listed. The 17 claims, 9 path entries, 24 projects, the 3 numbers and the date are the data file's (a check reads the file and compares).
* **Without JavaScript** the port has its lists, claims, projects, final numbers, waffles, pictures, videos and links to files (each answers 200 with the bytes of its file); the page's numbers read 0 there.
* **Authority**: `Home` requires `Clipboard` (the copy buttons) and nothing else; `AgentFiles` requires `Network` and nothing else (its `net.get` can only reach this site); `Ecosystem` data is named by its SHA-256;
  every request of a load is to the page's own origin; the script holds no other address. Trust boundaries of B1: **none** (no island yet); the hosts the page links to are listed by `inspect effects`
  (`alpibru.com`, `alpibrusl.github.io`, `github.com`) and are links, not requests.

## Open defects of the port (not decided, not hidden)

1. ~~**With JavaScript off the comparison table is missing**~~ **Fixed** (the `Table` prerender PR, `prerender.md` section 12): `Table` is written into the page when its cells are literals or what a `For` over data wrote out, and is
   the script's, whole, when anything in it reads state. Measured: with no script the port has the table (1 region, 1 table, 3 column headers, 3 row headers in the accessibility tree), and `parity.md` has no row for it any more
   (the row "human tools, used by agents ..." of the page's JavaScript-off text is gone; the phone-width row is reclassified: the page hides its header row on a phone).
2. **A layout shift of about 0.012 at load** (Pixel 7 profile, measured once; 0.0004 in a 1000 px window): the three fonts are `font-display: swap` and not preloaded, so the first screen's text moves once when they arrive. What the page's
   own number is, is B3's measurement; the fix would be a preload that `page { }` does not yet have.
3. **The page is longer and plainer than the page**: 15,686 px against 12,532 at 1280 px, the baseline's look (dropped.md 10). B3 reports it, width by width.

## Not measured in B1

Safari, Firefox, a phone, a screen reader; bytes, requests, FCP and LCP against the page (B3); pixels (B3); the islands (B2).
