# Routes: `RouteMap`, a diagram of a trip with no tiles and no network

> **Status: BUILT.** Written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1) for a program that holds a list of places in an order (a trip, a delivery round) and
> wants to show it. Section 8 says what was and was not looked at.

## 1. What exists, and why `Map` is not it

`Map` (`maps.md` section 10) is a 3 x 3 picture of tiles from a host the program declares, with pins over it and a list beside it. It has **no route**: the pins are listed nearest-first, not in
the order of a trip, and a picture of tiles needs a tile host (`requires Network("host")`) that sees the reader's IP address and User-Agent. The project's rule is open source and self-hosted only,
with no external server, so "use a tile host" is not an answer for a program that only wants to show a trip. A `RouteMap` is the other half: it draws the program's own positions and nothing else.

## 2. What it is, and what it is not

* **It is** an inline SVG of the program's points: a numbered circle for each located point, a line joining them in list order, a light graticule, and a note of how wide the drawing is ("about 420 km
  across"). The same points are written as an ordered list in a collapsed `<details>`, which is the accessible form.
* **It is not a map of the land.** It draws no coastlines, no borders, no roads, no names of places other than the program's own. Two points a sea apart look as if they were joined by land. It is a
  **diagram of positions**, and its name says so. A program that wants the land under the points uses a `Map` and declares where the tiles come from.
* **It needs no capability**: nothing is fetched, no host is named, no image is loaded, no position is read from the device. A `RouteMap` adds nothing to `cancho-ui diff`'s authority line.

## 3. The syntax

```
RouteMap { label: "Trip" points: rows route: true }
```

* `label:` (required, like a `Chart`): the name of the diagram, a visible `<figcaption>`, and the start of the SVG's accessible name. A literal or an expression of the view.
* `points:` (required): a `List` state field of JSON rows, the row shape `Map`'s `places:` already reads (a text literal list is not accepted; `routemap-binding-invalid`):
  `{"name": "Chamonix", "pos": "45.9237,6.8694", "kind": "lodging"}`.
  * `name`: the text the list shows. A missing or empty name is listed as `(unnamed)`.
  * `pos`: `"lat,lon"` in decimal degrees, the text `location.current()` answers and `distance()` reads: exactly one comma, each part a number, latitude within 90, longitude within 180, no space.
    A row whose `pos` is missing or is anything else is **not drawn and not numbered, and is listed** as `Name, no position`.
  * `kind`: optional, **1 to 12 letters, digits, `-` or `_`** (`lodging`, `activity`, `transport`, or any other word). `lodging`, `activity` and `transport` have a pin colour of their own; any other kind, and
    a `kind` that is longer or has another character (it is not a kind), draws the plain pin. No second row shape: a key other than these three is ignored.
  * The order of the list is the order of the route. At most **200 rows** are drawn and listed (the first 200); the sentence says `, the first 200 of 250 shown`.
* `route:` (optional, default `true`): `true`, `false` or a `Bool` state field. When it is true the located points are joined in order by a polyline; when it is false the pins are drawn alone and the
  sentence says `route not drawn`.
* `RouteMap` takes no other key (`binding-unknown`), and its keys on another element are `binding-unknown`. It is not drawn in a `For` (`routemap-in-loop`); under an `If` it is drawn when the condition holds.

## 4. The drawing

Pure client-side JavaScript in the page (`ROUTEMAP` adapter, emitted only for a program that has a `RouteMap`).

* **Projection.** Equirectangular, with longitude scaled by the cosine of the mean latitude of the located points: `x = lon * cos(mean lat)`, `y = -lat`. The bounding box is fitted into a 600 x 400 `viewBox` with a
  40-unit pad, by one scale for both axes (so the shape is not stretched), and centred. One located point, or several at one position, sits in the middle. The width is `width: 100%; max-width: 40rem; height: auto`.
* **Graticule and scale note.** Three vertical and two horizontal lines and a frame, decorative (`aria-hidden`, `currentColor` at 25 %), and the note `about N km across` (or `all at one position`) at the bottom right.
  `N` is **the diagonal of the bounding box in the drawing's projection**, in kilometres: `hypot((lonmax - lonmin) * 111.195 * cos(mean lat), (latmax - latmin) * 111.195)`, rounded to the nearest 10 km from 100 km,
  to the nearest km from 10 km, to a tenth below that. It is the size of the drawing, not a measured distance: for a trip a continent wide the cosine of the *mean* latitude is a rough scale, and the list's
  distances are the exact ones (below).
* **Pins.** A circle of radius 14 with the number `1..n` over the **located** points (so a row with no position leaves no gap in the numbers), in a class by `kind` (`cui-rm-lodging`, `-activity`,
  `-transport`, `-other`). Pins that coincide are drawn on top of each other; the list is how they are told apart.
* **The route.** One `<polyline>` through the located points in list order, in the chart series colour.
* **Colour.** The fill by kind is a custom property, and the number and the outline are the page's own background colour, so a pin's number has the contrast of its fill against the page. Baseline
  (`--pwa`, `--page`): light `#0550ae` 7.59:1, `#1a7f37` 5.08:1, `#57606a` 6.39:1 against white; dark `#79c0ff` 9.63:1, `#3fb950` 7.37:1, `#9198a1` 6.43:1 against `rgb(18,18,18)`; the plain pin is the text
  colour of the page. With a `theme { .. }` the same properties are `accent`, `good`, `muted` and `fg`, and the number is `bg`: every one is a pair `theme-contrast` already checks at 4.5:1 in both schemes
  (`docs/theme.md` section 4). `warn` is not used: `warn` is checked against `warn_bg`, not against `bg`. **Colour is not the only signal**: the number is, and the kind is written in the list.
* **Nothing from the program is parsed as markup.** Every name, kind and label is set with `textContent` or `setAttribute`; a number is formatted by the page. A name of `</text><script>..` stays text (section 8).

## 5. The accessible form

* The `<svg>` is `role="img"` with `aria-label="<label>: <summary>"`, and everything inside it is presentation only (`aria-hidden`). The summary is one sentence derived from the same data:
  `route map of 5 places, 3 on the route, 2 without a position, about 420 km across`. Its variants: `route map with no places`; `route map of 2 places, none with a position`; `, route not drawn` instead of
  `, N on the route`; `, all at one position`; `, the first 200 of 250 shown`. (The `2 without a position` clause is an addition to the first example: a sentence that says 3 of 5 and not why would be a quiet loss.)
* A `<details>` closed by default, whose summary reads `List of places (5)`, holds an `<ol>` in route order. Each item is `number. name (kind), distance from the previous located point`:
  `2. Chamonix (lodging), 68.3 km from the previous point`; the first located point says `start`; a row with no position reads `Name (kind), no position` with no number. The distance is the great-circle distance
  of `distance()` (6371.0088 km sphere), to a tenth. It is computed whether or not the route is drawn.
* The `<details>` follows the styling of the data table of a `Chart` (a 44 px summary, keyboard-operable).

## 6. Replay

Replay does not draw, so it cannot see geometry. After each step it reports, for each `RouteMap` in the page (an `If` that is false hides it), **`routemaps`**:
`{ label, count, total, located, on_route, km, across_km, summary, list }`, where `count` is the rows drawn (at most 200), `total` the rows given, `km` the length of the route in kilometres to a tenth
(0 when the route is not drawn), `across_km` the rounded width of section 4 (`null` with no located point), and `list` the texts of section 5. `expect` may assert it with the usual array and object
matching: `"expect": { "routemaps": [{ "summary": "route map of 5 places, 4 on the route, 1 without a position, about 83 km across" }] }`.
**The key is `routemaps`, not `routes`**: `routes` is already what a `route Navigated;` event heard in the step (`routing.md`).
The sentence and the list are built by one function in replay and the same function in the page; `tests/routemap_corpus.json` is run against both.

## 7. Guarantee tiers

| claim | tier |
|---|---|
| `RouteMap` takes `label:` and `points:`, a `Bool` or literal `route:`, nothing else; `points:` is a `List` field; not in a `For`; the tags below | **static** (the compiler refuses) |
| the page fetches nothing for it: no host, no capability, no image | **static** for the program (it has no `Network` to declare and the diff shows none); **tested** for the page (the adapter holds no `fetch`, `img`, `innerHTML`) |
| the sentence, the counts, the list and the distances are those of the corpus, in replay and in the page | **tested** (`tests/routemap_corpus.json`, expected values from `scripts/gen-routemap-corpus.py`; `cargo test`, `scripts/check-routemap.js`) |
| the pin geometry is the projection of section 4, inside the box | **tested** (`scripts/check-routemap.js`, against an oracle written again in the script) |
| the pin colours reach 4.5:1 against the page in both schemes | **tested** (`cargo test`: the baseline tokens; the theme's are the checked pairs) |
| the SVG and its list read well in a screen reader | **unknown**: no screen reader was run (section 8) |
| the diagram says where things are on the Earth | **not claimed**: see the limits |

Tags (`spec/diagnostics/README.md`): `routemap-incomplete` (no `label:` or no `points:`), `routemap-binding-invalid` (`points:` not a `List` state field, `route:` not `true`, `false` or a `Bool` field),
`routemap-in-loop`; a key of a `RouteMap` on another element, or another key on a `RouteMap`, is `binding-unknown`.

## 8. Limits, and what was looked at

* **The antimeridian is not handled.** Longitudes are used as given, so points at `179` and `-179` are drawn at the two ends of the box, a diagram the width of the world with a line across it, while the
  list says they are 222 km apart and the note says `about 39810 km across`. A program that crosses the line should shift its longitudes itself. (Case "across the antimeridian" of the corpus.)
* **No geography, no zoom, no panning, no tooltip, no per-pin focus** (the list is the way to read a point). Equirectangular is wrong near the poles and for a continent-wide span.
* Positions beyond the first 200 rows are neither drawn nor listed. Coincident pins hide each other.
* **Looked at in a browser:** the example page built with `--page` and served with `python3 -m http.server`, in the desktop app's built-in browser (Chromium), in both colour schemes, at the desktop width and at
  375 px (no horizontal scroll, the SVG 343 px wide), with the buttons of the example pressed (the route hidden and shown, a stop added, all cleared): the SVG's name, the pin classes and fills read from computed style,
  the list collapsed and opened, no console error. A hostile program (a name of `</text><script>window.__pwned=1</script>`, a kind of `<img onerror>`, a label of `<b>x</b>`) ran nothing and created no `<script>` or
  `<img>`. **Not done:** a screen reader, Safari and Firefox, a real phone, a pixel comparison.
* Node (`scripts/check-routemap.js`) runs the page's own functions without a DOM; the DOM-building function was exercised only in that browser session, not by a script in the repository.
