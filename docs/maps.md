# Maps: a nearest-places list first, then a tile map behind a declared host (issue #81)

> **Status: DESIGN, not built.** Written 2026-10-24 before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Numbers measured
> by `scripts/measure-maps.js` (Chromium of the Playwright install, `Pixel 7` profile, 412 px; the tile hosts and the page are local
> servers) on main at 7c16153. Anything not measured says **assumption**. Section 8 lists what is not known.

## 1. What exists

`location.current()` answers the text `"lat,lon"`, rounded to 2 decimals (about 1 km) or, with `requires Location("fine")`, to 5
(`adapters.md` section 11). `Canvas { .. }` and `Image { .. }` are `component-kind-unknown`. `adapters.md` 11.6 says a distance or a map needs
"a typed record or a list index; neither exists". The language has no records. `Network("host")` (section 10) bounds `net.get`,
`net.post` and the other calls the adapter makes, and the adapter checks the host before it sends.

## 2. Measured

| question | measured |
|---|---|
| **Is "the nearest five" expressible today?** | **Yes, and only just.** `scripts/measure-maps/near.cui` parses `"lat,lon"` with `num(join(take(split(p, ","), 1), ""), 0)` and sorts places by a flat squared distance, replayed: `PASS`. The sort key is **367 characters**. It cannot show a distance in km: the language has no square root and no trigonometry (cosine has to be a 4-term series, written out). The index-free parse corrects 11.6: a list index is not needed, `take` and `skip` do it. |
| how wrong is that flat, series-cosine ordering? (200 places, 2,000 random positions, latitude up to 70, any longitude, against haversine) | places within 10 km: same top five **99.90 %**, worst distance error 0.11 %. Within 100 km: 99.95 % (99.65 % in the same order), 0.82 %. Within **1,000 km: 97.90 % (95.10 % same order), error up to 10.96 %**. The series itself is off by at most 2.1e-4 (latitude 0 to 75). So the ordering is right locally and wrong at a continent; the longitude must also be wrapped (`min(d, 360 - d)`), which the probe does. |
| what rounding the position does to the list (200 places within 10 km, the nearest five) | **coarse (2 decimals): the list differs from the exact one in 92.5 % of 2,000 trials**; fine (5 decimals): 0.4 %. A coarse position is good for "which tile" and bad for "which of these five is first". |
| **a tile request** (an `<img>` from a host the page did not come from, page at `http://<origin>/index.html?pos=40.42,-3.70#places`) | carries: `Referer: http://<origin>/` (**origin only**, no path, no query, no fragment: the position in the page's URL does not travel), the full `User-Agent`, `Accept`, `Sec-Fetch-Site: cross-site`, no cookie, no `Origin`. `referrerpolicy="no-referrer"`: no `Referer`. The tile's path (`/14/8023/6177.png`) is the position, to the tile's size. The host also gets the IP address (**assumption**: not observable from the page; true of any HTTP request). |
| **the size of a tile** (north-south x east-west) | latitude 40.4: zoom 13 **3.72 km**, zoom 14 **1.86 km**, zoom 15 0.93 km, zoom 16 0.47 km, zoom 18 0.12 km. Latitude 60: zoom 14 1.22 km. Latitude 75: zoom 13 1.26 km. |
| **the highest zoom whose tile is at least as large as the rounding cell** (so the request says no more than the position the program holds) | coarse (0.01 degree): latitude 0 **15**, 40.4 **14**, 60 **14**, 70 **13**, 75 **13**, 80 **12**. Fine (0.00001 degree): at least 22 everywhere. So **zoom 13 is safe for a coarse position up to latitude 75**, and every zoom the tile sets serve (0 to 19) is safe for a fine one. |
| **which tiles a view needs** (412 x 300, 256 px tiles) | centred on the position: **4 to 9 tiles, 9 different request sets** over the position's offsets inside its tile: the set of tiles asked for says where in the tile the position is. Centred on the middle of the position's tile: **9 tiles, 1 request set**. |
| **is a tile bound by `Network("host")`?** | **No.** With no policy an `<img>` from a host the program never declared loaded (1 x 1 image, the host saw the request). The adapter checks `fetch`, not `img`. |
| can the page bound it? | **Yes, in the browser.** A `<meta http-equiv="Content-Security-Policy" content="img-src 'self' data: <host>">`: an `<img>` from that host loaded; an `<img>` from another host did not load and **the other host saw no request** (0); a CSS `background-image` from the other host was blocked too; the page hears a `securitypolicyviolation` event naming the directive and the URL. |
| **offline** (the installed page of `examples/where`, its worker controlling the page) | The worker returns early for a request of another origin (`pwa.rs`), so **a tile is never cached by the app**. With the host unreachable, a tile the browser's own HTTP cache holds (`max-age=3600`) **loaded** (0 ms); a tile it had never seen **failed** (`error` event, `naturalWidth` 0, 2 to 4 ms), the same with Playwright's emulated offline mode. |
| **keeping tiles for offline** (what `cache.put` of a tile costs) | an opaque (`no-cors`) response of a **70-byte** tile cost **6,448,288**, **9,253,668** and **10,233,408 bytes** of quota per tile in three runs (the browser pads it, and not by a fixed amount); the same tile from a host that sends CORS headers cost **369 bytes**. Nine opaque tiles are about 58 to 92 MB. |
| **what a screen reader gets** (accessibility tree, non-ignored nodes; 9 tiles and a list of 3) | tiles with no `alt`: **24 nodes**, nine of them unnamed `image`s. Tiles with `alt=""`: 15. Tiles in a `role="img"` box with `aria-label`: 15, the box one `image`. With the tiles and an SVG of pins `aria-hidden` inside the box: 15. The list is the content; the picture is one named image. |

## 3. The decision

**Two steps, each its own PR; the map comes second and is not needed by the first.**

### 3.1 Step one: `distance(a, b, default)`, and a "places near me" list

* **A pure builtin `distance(a, b, default)`**: the great-circle distance in **kilometres** (haversine, mean radius 6371.0088 km) between two
  `"lat,lon"` texts, a number. A text that is not `lat,lon` with latitude in [-90, 90] and longitude in [-180, 180] (the grammar of `num`
  for each part) gives `default`, as `num(text, default)` does: no trap, no error value. It is **not rounded**; `fmt.number` shows it.
  Exactly three arguments, else `builtin-shape-mismatch`. Pure, so it runs in a `sort` key.
* **Why a builtin and not a recipe.** The recipe exists (section 2, first row) and is 367 characters, wrong at 1,000 km (up to 11 %), cannot
  say "1.2 km", and every program would copy it. The cost of the builtin is one corpus (`tests/distance_corpus.json`) checked by the replay
  evaluator and by the emitted JS, the way `move` and `num` are.
* **The position stays text.** `adapters.md` 11.1 chose `"lat,lon"` because the language has no records; nothing measured here changes that.
  A place is a JSON text row `{"name": "Sol", "pos": "40.4169,-3.7035"}` read with `json.get`, which is how a list from `net.get` already arrives.
  The page is then:
  `shown = take(sort(places, p => distance(pos, json.get(p, "pos", ""), 20000)), 5);`
* **Done when** (the issue): `examples/near/near.cui` shows the nearest five of a list for a position the trace supplies, with the distance
  in km; its trace covers a position that arrives, a refusal (`Err("permission denied")`), and a place with an unreadable `pos` sorted last;
  `scripts/check-near.js` runs the page in Chromium with the geolocation permission **granted** (a position emulated), **denied** (the
  answer is `permission denied` and the list says so) and **absent** (no API). The ordering is checked against the corpus in both evaluators.

### 3.2 Step two: `Map`, a tile map behind a declared host

A tile layer is **built**, with these conditions, each from section 2. If any of them cannot be kept, the map is not built and this note says which.

```
Map {
    label: "Places near you"
    center: pos                      // a state field holding "lat,lon" (or a literal)
    places: places                   // a List[String] of rows {"name":..,"pos":..}
    zoom: 13
    tiles: "https://tiles.example/{z}/{x}/{y}.png"
    attribution: "Map data: the provider's wording"
}
```

* **The host is declared once, in the place the diff already shows.** `tiles:` is a string literal with `{z}`, `{x}` and `{y}` and a scheme
  of `https` (`http` for `localhost`); its host must be in the component's `requires Network("host", ..)` bound. Not declared: **`map-host-undeclared`**,
  at compile time, naming the line to add. The semantic diff already reports a new `Network` host as a widening; a tile host is reported the same.
* **The browser enforces it, not just the compiler.** A page with a `Map` carries a `<meta http-equiv="Content-Security-Policy">` as the
  first thing in `<head>` with `img-src 'self' data: <the tile hosts of every Map in the page>`. Measured: an undeclared host's image is not
  requested at all. A page without a `Map` gets no policy and nothing else changes.
* **The map is centred on the middle of the position's tile**, not on the position. Measured: the position as the centre makes 9 different
  request sets (the tile host can tell where in the tile the user is); the tile's centre makes 1. The user's own pin is drawn over the
  tiles in an SVG, so the picture is exact and the request is not.
* **The zoom is bounded by the bound.** With a bare `requires Location` (coarse), `zoom:` above **13** is **`map-zoom-invalid`** (13 keeps
  a tile at least as large as the 2-decimal cell up to latitude 75); with `Location("fine")` the limit is 19. `zoom:` is an integer literal.
  *(Beyond latitude 75 zoom 13 is finer than the cell: the limit is 12 at latitude 80. The build lowers the zoom by the centre's latitude at run
  time and the replay report shows the zoom used.)*
* **The referrer is not sent**: every tile `<img>` has `referrerpolicy="no-referrer"` (measured: no `Referer`). The tile host still sees the
  IP address and the User-Agent; the build says so in the page's text for the author (`AGENTS.md`), because no code can prevent it.
* **No tile is kept by the app.** The worker leaves other origins alone (measured) and keeping opaque tiles costs about 6 to 10 MB each in
  quota, which is more than the map is worth. Offline, a tile the browser's HTTP cache holds still shows; the others fail quietly (grey
  tile, `alt=""`), and when **every** tile fails the figure says "The map is not available offline." The list below it is unaffected: it is
  made from the program's data, not from the network.
* **The accessible form is the list.** The `Map` always draws, inside the same `<figure>`, an ordered list of the places by distance from
  `center`: "1. Sol, 0.3 km". The picture is one `role="img"` named by `label:` (tiles `alt=""`, pins `aria-hidden`; measured 15 nodes
  against 24 for unnamed tiles). `places:` is required (**`map-incomplete`**) so a map without its list cannot be written. The list is
  visible text, not a collapsed table: for a map the list is the primary content.
* **No panning, no zoom control, no pins you can press** in this step. A drag on a map fights the page's scroll (it would need
  `touch-action: none` and a keyboard path, the problem of `drag-and-drop.md`) and a zoom control needs one more cost-measured decision. The program
  changes `center:` and `zoom:`; that is the interaction. A press on a list row is a `Button`, which already exists.
* **Attribution is required text**: `attribution:` a literal of at most 200 characters, shown under the map (**`map-incomplete`** when absent).
  What a provider requires, and whether it allows an app to request tiles at all, is the provider's terms, **which this note does not
  state**: the public OpenStreetMap tile policy was not readable from this environment (the host is denied), so it is **unread, not
  summarised**. The compiler names no provider and has no default tile URL.
* **Limits.** At most **200** places are listed and at most **50** pins drawn; the report says `, the first 200 of N`. The view is 3 x 3
  tiles of 256 px, 768 x 768 units scaled to `width:100%` (`max-width:40rem`).
* **Replay can assert the privacy contract with no browser.** `cancho-ui test` reports `maps: [{label, zoom, tiles: [..the nine URLs..],
  list: [{name, km}]}]`, and a trace may expect it, so "this page asks the host for exactly these nine tiles, and no other URL" is a trace
  line (the `charts` and `sortlists` pattern). The Chromium check compares the requests the host really saw with that list.
* **Rule tags (new, each documented in `spec/diagnostics/README.md` with a test that reaches it)**: `map-incomplete` (label, center, places,
  zoom, tiles, attribution), `map-tiles-invalid` (not a literal, no `{z}`/`{x}`/`{y}`, a scheme other than `https`/`localhost` `http`, a userinfo or a
  query that is not a placeholder), `map-host-undeclared`, `map-zoom-invalid`, `map-binding-invalid` (`places:` or `center:` not a field of the right type),
  and `binding-unknown` for a key `Map` does not take. `distance` has `builtin-shape-mismatch` for its arity.

## 4. What the build must have

1. Step one: the builtin in `lower.rs`, `conditions.rs` (kind Float), the replay evaluator and the emitted JS; `tests/distance_corpus.json` (the
   fixed points: Madrid to Barcelona 505 km, the equator and the poles, the antimeridian, an unreadable text, `NaN`, a latitude of 91);
   `examples/near`; `scripts/check-near.js` with the permission granted, denied and absent; `scripts/breaks/near.json`; the gallery tile
   (a live page, with the position scripted by the demo).
2. Step two: `Map` through the usual pipeline (syntax, IR with the tags, replay with `maps`, the page adapter, the shared corpus, an example
   with a trace, `scripts/check-map.js` with a local tile host, `scripts/breaks/map.json`, docs). The Chromium check must show: the nine
   requests and no other; no `Referer`; a request to an undeclared host never made; the offline message; the same page in a 412 px
   profile with no horizontal scroll; the list in the accessibility tree.
3. Deliberate breaks for each condition in 3.2 (host check removed, centre on the position, zoom bound removed, referrer sent, list
   removed, policy absent). A break counts as caught only if a check fails on its own.

## 5. Not decided here

* Whether `distance` should also be given a bearing or a bounding-box filter. Neither is needed for "the nearest five".
* A tile cache the author opts into (`Storage`-like). It needs a host that sends CORS headers (measured: 369 bytes against 6 to 10 MB per tile
  otherwise), which the author knows and the compiler cannot check.

## 6. Found while writing it

* **Correction to `adapters.md` 11.6** ("a real consumer needs a typed record or a list index"): the parse is possible with `take`, `skip`,
  `join` and `num` today (section 2). What is missing is a square root and trigonometry, so a distance in km, not a position.
* `Network("host")` is a statement about `fetch`, not about the page. Until a `Map` (or any element that loads a resource) states its host
  through the same declaration and the browser enforces it, an `<img>` or a stylesheet can reach any host from any page. The policy in 3.2 is
  the first place the declaration reaches the browser; it is worth doing for `Image` and for fonts later (no new element is designed here).
* A harness mistake of mine: the first CSP measurement showed the declared host blocked too. The cause was the page, which I served with
  `page.route` and `fulfill`; Chromium treats such a document as an unknown address space and blocks a request to `localhost` as a private-network
  request. Served from a real local server the declared host loads. That block is the harness's, not the policy's.

## 7. Checked

Section 2 is `node scripts/measure-maps.js` (about a minute, a 4-minute watchdog, outside the gate). Nothing is built, so nothing here is in the
gate. The probe `scripts/measure-maps/near.cui` and its trace pass under `cancho-ui test` (and the script runs them).

## 8. Not measured: assumptions

* **A real device.** The geolocation granted and denied flows are emulated by Playwright (as in `check-location.js`); a phone's prompt, its accuracy
  and its battery are not measured (`device tests are #65`).
* **Firefox and Safari.** Whether a `<meta>` Content-Security-Policy bounds images in them as in Chromium, and what they send as a referrer for
  `referrerpolicy="no-referrer"`, is **assumption** (both are specified; neither was measured).
* **Tile providers.** Their terms, their rate limits, whether they send CORS headers, and whether they log or sell requests: **not measured**, and the
  compiler does not decide them. The IP address a tile host receives is **assumption** (not visible to the page).
* **Large places lists.** The 200 and 50 limits are chosen from the charts measurements (`charts.md`), not measured for pins.
* **Screen reader speech.** The accessibility tree is measured; what NVDA, VoiceOver or TalkBack say for the figure is **assumption**.
* **A coarse position near the poles** (beyond latitude 75) and the antimeridian in the Chromium page: the corpus covers the maths, the page is not measured there.
