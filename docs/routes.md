# Routes: `RouteMap`, a diagram of a trip, with no tiles and no network unless the program asks for tiles

> **Status: BUILT** (the diagram: sections 1 to 8; the optional tiles: section 9). Written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1) for a program that holds a list of places in an order (a trip, a delivery round) and
> wants to show it. Section 8 says what was and was not looked at; section 9 is the optional `tiles:`.

## 1. What exists, and why `Map` is not it

`Map` (`maps.md` section 10) is a 3 x 3 picture of tiles from a host the program declares, with pins over it and a list beside it. It has **no route**: the pins are listed nearest-first, not in
the order of a trip, and a picture of tiles needs a tile host (`requires Network("host")`) that sees the reader's IP address and User-Agent. The project's rule is open source and self-hosted only,
with no external server, so "use a tile host" is not an answer for a program that only wants to show a trip. A `RouteMap` is the other half: it draws the program's own positions and nothing else.

## 2. What it is, and what it is not

* **It is** an inline SVG of the program's points: a numbered circle for each located point, a line joining them in list order, a light graticule, and a note of how wide the drawing is ("about 420 km
  across"). The same points are written as an ordered list in a collapsed `<details>`, which is the accessible form.
* **It is not a map of the land.** It draws no coastlines, no borders, no roads, no names of places other than the program's own. Two points a sea apart look as if they were joined by land. It is a
  **diagram of positions**, and its name says so. A program that wants the land under the points uses a `Map` and declares where the tiles come from.
* **It needs no capability** (without `tiles:`, section 9): nothing is fetched, no host is named, no image is loaded, no position is read from the device. A `RouteMap` adds nothing to `cancho-ui diff`'s authority line.

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
* `tiles:` and `attribution:` (optional, together; section 9) draw the program's own tile host's tiles under the pins.
* `RouteMap` takes no other key (`binding-unknown`: `zoom:`, `center:`, `places:` are a `Map`'s), and `points:` and `route:` on another element are `binding-unknown`. It is not drawn in a `For` (`routemap-in-loop`); under an `If` it is drawn when the condition holds.

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
`routemap-in-loop`, and, with `tiles:`, `routemap-tiles-invalid`, `routemap-host-undeclared`, and `routemap-incomplete` also for a `tiles:` without `attribution:` (section 9). A key of a `RouteMap` on another element, or another key on a `RouteMap`, is `binding-unknown`.

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

## 9. Tiles: `tiles:` and `attribution:`

```
RouteMap { label: "The trip" points: rows route: true tiles: "https://tile.openstreetmap.org/{z}/{x}/{y}.png" attribution: "© OpenStreetMap contributors" }
```

**What it adds.** The trip is drawn on a real map: the tiles of an address the program writes are laid under the pins and the line. Nothing else changes: the pins (colour by kind, numbers), the route, the
collapsed list, the accessible name, the sentence, the km figure and the report's section 6 fields are the same. **What it costs** is what a `Map` costs: a tile host, which is authority.

**The rules** are the `Map`'s (`maps.md` section 10), with their own tags:

* `tiles:` is a **text literal** with `{z}`, `{x}` and `{y}` once each, `https://` (`http://` for `localhost` and `127.0.0.1`), no user name, no other brace; or a **path on the page's own origin**
  (`./tiles/{z}/{x}/{y}.png`, `/tiles/..`). It is checked by the very function `Map`'s is. Not that: `routemap-tiles-invalid`. A name or a number: the same.
* Its host must be declared: `requires Network("tile.openstreetmap.org");` (port included, compared lower-cased), or a bare `requires Network;` for a path on the page. Not declared, or a bare `Network` for a host
  on the internet: `routemap-host-undeclared`, naming the line to add. A tiled `RouteMap` is the use of `Network` that `capability-unused` looks for (and a `Network` with no `Map` and no tiled `RouteMap` is still unused).
  `cancho-ui diff` reports the new host as it reports a `Map`'s.
* `attribution:` is a text literal of 1 to 200 characters, **required with `tiles:`** (`routemap-incomplete`); with no `tiles:` it is `routemap-binding-invalid` (nothing to credit), as is an empty or longer one.
  It is shown as visible text under the drawing, set with `textContent`, never parsed as markup.
* **There is no default host** and the toolkit names no provider (the test that scans the compiler's sources for provider names covers this element too). The example reads its tiles from a folder beside the page.
* The generated page's `Content-Security-Policy` is the `Map`'s: `img-src 'self' data:` plus the origin (scheme and port) of each declared tile host of any `Map` or tiled `RouteMap`. An image from another host is not
  requested. A page with a `RouteMap` and no tiles gets no policy, as before.

**What the tile host learns.** The reader's **IP address** and **User-Agent**, and the tile addresses asked for, which say the area of the trip to the size of the grid (zoom and tile numbers: the zoom is the fit
of the whole trip, so it also says roughly how far apart the stops are). `referrerpolicy="no-referrer"` is set on every tile, so no `Referer`. No cookie, no `crossorigin`. The page does not store tiles and the
worker (`--pwa`) leaves other origins alone, so a tile is never kept by the app; what the browser's HTTP cache keeps is the browser's. **The public OpenStreetMap tile servers have a usage policy**
(visible attribution, light use, an identifying application; bulk use and heavy traffic are not allowed). It is the operator's to read and to meet; the toolkit has no default host, does not read the policy and cannot
enforce it. A program for many readers wants a provider it has an agreement with, or tiles it serves itself.

**The drawing with tiles** (Web Mercator, the projection of slippy tiles, so that pins and line sit exactly on the picture):

* **World pixel** at zoom `z`, with `s = 256 * 2^z`: `x = (lon + 180) / 360 * s`, `y = (1 - ln(tan(phi) + sec(phi)) / pi) / 2 * s`, the latitude **clamped to +-85.0511287798** first (the tiles stop there: a point at 89.9 is
  drawn and asked for at the northern edge of the world, not where it is).
* **Zoom and grid.** One located point, or all at one position: zoom **13** and the **3 x 3** tiles around its tile. Otherwise the **highest zoom from 18 down to 1** at which the tiles covering the points' bounding box
  plus **32 px** of margin on every side, clipped to the tile range `0 .. 2^z - 1`, are **at most 4 across and 4 down** (at most **16 requests**). (At zoom 2 there are only 4 columns, so a grid always fits there and the
  floor of 1 is never reached; it is the stated bound.) The 32 px is more than a pin and its halo at any size of the drawing, so a pin is never cut by the grid except at the very edge of the world.
* **The picture** is the grid itself: `cols * 256` by `rows * 256` units, tiles in their own pixel space, pins and line in the same space. Pins keep the on-screen size of the plain diagram (14 of 600 across the
  longer side). The box is `width: 100%` and no wider than 40 rem (a narrow grid is narrower, so a tall one does not run down the page); the page's width at 375 px leaves no horizontal scroll.
* **The tiles** are `<img>` elements in that box, positioned in percent, under the SVG: `alt=""` (decorative), `referrerpolicy="no-referrer"`, `loading="eager"` (no lazy loading), no `crossorigin`. The SVG over them
  keeps `role="img"` and the label of section 5; **the summary is unchanged by tiles** (it says nothing of them), and the list is still the accessible form. A tile that fails to load is hidden and the grey ground
  shows; the pins and the list are the program's own data and are unaffected (offline: a tile in the browser's HTTP cache still shows).
* **Contrast.** The tiles are a picture and do not change with the colour scheme. Each pin has a white halo under its fill and the page's colour as its outline, and the line has a white casing and a page-coloured
  stroke under the series colour, so the pins and the line read on a light map and on a dark page (the pin fill and its number are the pairs of section 4, checked in both schemes). Colour is still not the only
  signal: the number is, and the list says the kind.
* **Under the picture,** as visible text: `about N km across` (the figure of section 4; the in-SVG note of the plain diagram is moved out of the picture so that it never hides a pin), and the attribution.
* **The figure is kept across renders** while its inputs (label, rows, route, address, attribution) are the same (a page is rebuilt at every state change): a render that changes nothing about the map asks the host
  for no tile. Changing `route:` rebuilds the figure and asks again (the browser's cache decides whether the host sees it).
* **No located point:** nothing is asked for, no attribution is shown (no tile is), and the plain diagram of section 4 stands. Rows with no readable position are listed and not drawn, as before.
* **No tiles:** with no `tiles:` the module the compiler writes is the one it wrote before (the tile code is appended only for a program that has a tiled `RouteMap`; a test compares the shared part, and the
  gallery's `route` demo is byte-for-byte unchanged).

**Replay** cannot fetch a tile, so with `tiles:` (and only then) each `routemaps` entry also has **`zoom`** (`null` with no located point), **`grid`** (`[cols, rows]`, `[0, 0]` with none), **`tiles`** (the exact
addresses in row-major order, which is what the page asks the host for, and no other) and **`attribution`**. `expect` may assert them:
`"expect": { "routemaps": [{ "zoom": 10, "grid": [3, 3], "tiles": ["./tiles/10/529/363.png", "..."] }] }`.

**Guarantee tiers.**

| claim | tier |
|---|---|
| `tiles:` is a valid address, its host is declared, `attribution:` is there, a `Map`'s other keys are not a `RouteMap`'s; the tags above | **static** (`crates/cancho-ui/tests/routemap_tiles.rs`, with the negative controls: no attribution, an undeclared host, no `{z}`/`{x}`/`{y}`, a user name, `http` for a public host, a name instead of a literal) |
| the zoom, the grid, the tile addresses and each pin's pixel are those of the independent corpus, in replay and in the page | **tested** (`tests/routemap_tiles_corpus.json`, expected values from `scripts/gen-routemap-tiles-corpus.py`, which uses the `asinh(tan)` form; `cargo test`, `scripts/check-routemap-tiles.js`) |
| the page lets images in from the declared host and no other | **tested** for the policy text; **measured** for the browser's enforcement of `img-src` (`maps.md` section 2) |
| the tile code sets no markup, asks with no referrer, uses no lazy loading | **tested** (a scan of the adapter) |
| tiles of a real provider line up with the pins | **not claimed**: no real tile server was asked (assumption: they follow the standard formula) |
| the host learns only what the section says | **trust**: nothing here can see the host's logs |

**Limits, and what was looked at.**

* **Longitude is not wrapped.** Points at `179` and `-179` span the world (zoom 2, four columns), as in the plain diagram; the grid is clipped to the tile range, so near `+-180` a single point's 3 x 3 grid loses a
  column. A program that crosses the line shifts its longitudes itself.
* **A latitude beyond 85.05 is clamped** (its pin sits on the edge of the world) and a tile row beyond the Mercator range is clamped to the first or last. Nothing is drawn beyond the poles.
* No panning, zoom control or tooltip. The zoom is the program's data's fit, not a choice.
* **Looked at in a browser** (the desktop app's built-in browser, Chromium; the example built with `--page`, served by `python3 -m http.server`, tiles from a folder beside the page, no real tile host contacted by
  any test or by this look): light and dark at 375 px, no horizontal scroll, 9 tiles then 16 after a stop is added (the host was the page's own origin and nothing else), the SVG's name, the list collapsed,
  the stops cleared (no image, no credit). **Not done:** a real tile provider, a screen reader, Firefox and Safari (the `img-src` policy and `referrerpolicy`), a real phone, a pixel comparison.
