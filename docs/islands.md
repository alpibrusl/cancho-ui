# Islands: the hero, the cairns and the map, declared and measured (addendum to `www.md` section 6)

> **Status: design addendum, nothing built** (2026-10-25). `www.md` section 6 designed islands; section 10 item 3 decided that the map is an **open** island (`authority` prints `UNBOUNDED`) and the hero and the cairns are **sealed**. This note takes the real files of cancho-www (`dbbf7ae`: `rocks.js` 5,524 B, `scene.js` 3,304 B, `map.js` 21,760 B, the rules of `site.css`) as the worked examples, **measures** what a sealed island can and cannot do, and answers what the smallest island API is.
> Measured in **desktop Chromium** (Playwright, the `Pixel 7` profile is a viewport and a user agent, not a device) by [`scripts/measure-islands.js`](../scripts/measure-islands.js), which takes a clone of cancho-www and prints every table below (`node scripts/measure-islands.js <clone> [reach|messages|spin|hero|pointer|cairns|media|a11y|hydrate|race]`). "4x" is Chromium's CPU throttle, a proxy and not a phone. The machine was shared, so timings under throttle vary from run to run; where a number moved between two runs both are given.

## 1. What the measurements changed in `www.md` section 6

Corrected there too, each with the measurement behind it:

1. **The CSP of section 6 does not seal the frame.** With `sandbox="allow-scripts"` and `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'` the island **navigated its own frame to another origin** (`location.href = ...`: **1 request arrived there**) and a `<meta http-equiv="refresh">` it added did the same (1 request). Adding `form-action 'none'; base-uri 'none'; frame-src 'none'` **to the frame's CSP does not close either** (still 1 request each). What closes both is the **page's own CSP: `frame-src about:`** (0 requests). So a page with an island needs a CSP `<meta>` of its own, merged with the one a map already gets (`img-src`).
2. **WebRTC leaks.** An island that opens an `RTCPeerConnection` to a STUN server sent **3 UDP packets** to it in every configuration above; CSP does not govern ICE. A shim written before the island's script, that makes `RTCPeerConnection`, `webkitRTCPeerConnection` and `RTCDataChannel` `undefined` and non-configurable, made it throw `TypeError` and **0 packets arrive**. That closes the way I tried; it is not a proof that no other way exists (see 9).
3. **"Bounded authority" is not bounded time.** A sealed island that loops for 2 s stalled the page: the longest gap between two frames of the *page* went from 17–67 ms to **1,983 ms**. The sandbox bounds what an island can *reach*; it does not bound what it costs. A sealed island can freeze the page it is in, and the design must say so.
4. **`size: Responsive { ratio: 640 / 600 }`** is `width: 640 height: 600`, as for `Image` (16:15 is not in the closed `ratio` set, and the integer pair gives the aspect ratio before the frame loads).
5. **`requires Draw`** is dropped. A sealed island reaches nothing but the page, by messages (section 2): a capability that grants nothing is noise in `authority`. A sealed island is **listed** (section 6), an open one declares **`requires Dom`**.
6. "The pointer is forwarded as a message" has a cost, measured in 3, and a rule: at most 20 a second.

## 2. What a sealed island can reach, measured

An island script runs in `<iframe sandbox="allow-scripts" srcdoc="...">` and tries 30 things; a second HTTP server on another port and a UDP socket record what arrives. Four configurations: (1) the CSP of `www.md` section 6; (2) = (1) + `form-action 'none'; base-uri 'none'; frame-src 'none'` in the frame; (3) = (1) + the page's own CSP `frame-src about:`; (4) = (2) + (3) + the shim of 1.2.

**Nothing arrived, in all four configurations,** for: `parent.document` and `top.location` (`SecurityError`), `localStorage`, `document.cookie`, `indexedDB` (`SecurityError`), `fetch` to another origin and to the page's own (`TypeError`), `XMLHttpRequest`, `new Image().src`, `sendBeacon`, `WebSocket`, `EventSource`, a `<link rel=stylesheet>`, a CSS `url()`, a `<script src>`, `import()`, a nested `<iframe src>`, a `<form>` submitted (the sandbox has no `allow-forms`), `window.open` (`null`), an `<a target=_top>` click, `<link rel=prefetch>`, an `<object data>`; `eval` (`EvalError`), a `Worker` from a blob (error), `serviceWorker.register` (`SecurityError`), `navigator.clipboard.writeText` (`NotAllowedError`). `parent.postMessage` is the one channel that works, by design.

| what the island did | (1) CSP of section 6 | (2) + frame-src etc. in the frame | (3) + the page's `frame-src about:` | (4) + the WebRTC shim |
|---|---|---|---|---|
| `location.href = ` another origin | **1 request** | **1 request** | none | none |
| `<meta http-equiv=refresh>` to another origin | **1 request** | **1 request** | none | none |
| `RTCPeerConnection` to a STUN server | **3 packets** | **3 packets** | **3 packets** | `TypeError`, none |

So a sealed island is, with (4): **no network, no storage, no cookies, no access to the page, no navigation, no script of its own source but what the compiler wrote, no `eval`, no clipboard**, and one channel (messages) the page controls.

**Not bounded, and not measured:** CPU and memory (the `while` loop above), timing side channels, DNS prefetch (it sends no HTTP request, so this method cannot see it), anything a future browser adds, and the next hole of the kind WebRTC was. The sandbox is a list of closed doors; the list is only as complete as the tests, and the build should re-run this script (`reach`) as a check.

The user's preferences do reach the frame: with the page emulating dark and reduced motion, the frame's `matchMedia` said dark and reduced, and a stylesheet in the frame with `@media (prefers-color-scheme: dark)` took the dark `--accent` (`#6ea3ee`). So the hero's palette and its `prefers-reduced-motion` rules work unchanged in the frame, **without a prop**.

## 3. What the frame costs, measured

* **Messages.** Round trip page → frame → page: **1.2 ms** (p50; p95 1.7–2.0 ms) for 16 B and 1 kB, 1.6–1.7 ms for 100 kB; under 4x throttle **8.5–8.8 ms** (p95 12–17 ms). 2,000 messages posted at once took 2.4–2.7 s to be echoed (about 750 a second) at 1x and 16.5 s at 4x. Messages are cheap one at a time and **not** a bulk channel: an island's data goes in as props (state), not as a stream. Posted **right after the iframe is inserted, a message is lost** (0 of 20); posted at the frame's `load` or when the island says it is ready, 20 of 20. The protocol is therefore a handshake: the island posts `ready`, the page answers with the whole current props, and again whenever they change.
* **The hero.** `rocks.js` + `scene.js` unchanged, with the 8,179 bytes of `site.css` rules they need (taken out of `site.css` by the browser's own parser), 4x throttle, 3 s of animation: drawing took **24 ms inline and 25–35 ms in the frame**; 181 frames in 3 s either way, p50 and p95 16.7 ms, **0 frames over 50 ms** in both. The page's own document holds 347 nodes inline and **7** with the frame (the 347 are in the frame's document). The hero is cheap to seal.
* **The pointer.** The page's `hero()` listens to `pointermove` on the window; a frame sees only the pointer inside itself, so the page must forward it. 180 events at 60 Hz for 3 s, 4x throttle: forwarding every event gave a round trip of 58–72 ms (p50) and the *page* 3–11 frames over 50 ms (p95 frame 50–67 ms); at most one per animation frame was no better (7–10 frames over 50 ms); **at most 20 a second** gave 1–3 frames over 50 ms (p95 33–50 ms). The hero's layers already ease `--px` with `transition: transform .25s`, which hides 20 Hz. **Rule: the runtime forwards the pointer at most 20 times a second**, as normalised coordinates of the page's viewport, only while the island is on screen.
* **The cairns.** The page has 11 `.cairn` dividers (3 rocks each). Inline: **15–24 ms** to draw all 11, 15,613 bytes of HTML (5,364 gzipped). As 11 sealed frames at 4x: **875 and 1,237 ms** until all 11 had drawn (and 111 ms at 1x), **168,239 bytes** of HTML (7,173 gzipped: the 11 copies of the runtime compress), 12 documents. **This is the expensive one**: about 50x the time and 11x the bytes of drawing them in the page, for 11 decorations of 3 stones. The reveal on scroll works unchanged, though: an `IntersectionObserver` *inside* a sandboxed frame reports the **page's** viewport (a stone 900 px below the screen has ratio 0; when scrolled to, 1, `rootBounds` null), so the cairns' `.in` class, today set by the page's observer, can be set by the same observer code in the frame (1 of 11 revealed at load, 11 of 11 after scrolling, as inline).
* **Accessibility.** The hero's 347 nodes are shapes with no name: they are **not in the accessibility tree either way** (the page's tree had 6 nodes with `title` only, 6 with `role="img"`, 5 with `aria-hidden="true"`: the iframe itself is the one node). `decorative: true` writes `aria-hidden="true"` on the iframe and removes that node.

## 4. The smallest island API for the real hero and cairns

The fixed decisions: declared, pinned by `sha256`, sealed by default, visible to `authority` and `diff`. The smallest declaration that runs the real files:

```
island Hero {
    script: "assets/rocks.js"   sha256: "ab5fffd8..."      // byte for byte the file cancho-www has
    script: "assets/scene.js"   sha256: "b183de18..."      // byte for byte the file cancho-www has
    script: "assets/island-hero.js" sha256: "..."          // the glue: 15 lines, new
    style:  "assets/island-hero.css" sha256: "..."         // the 8,179 bytes of rules and both palettes
    label: "A stack of granite boulders in front of a blue sun"
    decorative: true
    width: 640 height: 600
    props { pointer: Pointer }
}
island Cairn {
    script: "assets/rocks.js"   sha256: "ab5fffd8..."
    script: "assets/island-cairn.js" sha256: "..."
    style:  "assets/island-cairn.css" sha256: "..."
    decorative: true
    width: 84 height: 58
    props { seed: Int }
}
// in a view:  Hero { pointer: pointer }      Cairn { seed: 3 }
```

* **The pieces, and nothing else:** an **ordered list of hashed scripts**, an optional **hashed style**, `label:` (or `decorative: true` and then none), `width`/`height`, typed **props** (`Bool`, `Int`, `Float`, `String`, and the built-in `Pointer`), typed **events out** (none for these two). No body markup: the glue creates the `<svg>` it draws into. The frame's document is: the policy `<meta>` (section 2), the style, then the scripts in order, each in its own `<script>`.
* **The contract inside the frame** is four names, written by the compiler before the island's scripts: `cancho.props` (the latest props), `cancho.onprops(fn)`, `cancho.emit(name, value)`, `cancho.ready()` (posts `ready`; the page answers with the props). Nothing else is added to the global. The shim also removes the WebRTC constructors (1.2).
* **`rocks.js` and `scene.js` run unchanged.** They need only `window`, `document.body` (for `Rocks.ensureDefs`, which appends the gradients `#gr`, `#shade`, `#halo`, `#speck` that the rocks' `fill: url(#gr)` find in the same document), `document.createElementNS`, and `Scene.draw(svg)`; the same `scripts/make_og.js` use of `Scene.draw` is untouched. Measured: the real files drew the hero in the frame.
* **New code the port needs (not in cancho-www today):** the glue, which is the part of `site.js` `hero()` and `cairns()` that moves: `Scene.draw(svg)`, `--px` and `--py` from `cancho.onprops`, for the cairns `Rocks.rock` for three stones from the `seed` prop and the page's `IntersectionObserver` code for `.in` (measured to work in the frame, 3). About 15 lines each. And `island-hero.css`: the rules of `site.css` for `.rock-*`, `.shrub`, `.scene .layer`, `.sun`, `.ridge*`, `.ground`, `.pine`, `.cloud`, `.bird`, `.drop`, the keyframes `sunrise breathe drift glide settle`, `.cairn` and the two palettes (`:root` and `prefers-color-scheme: dark`). Those rules also style the authority animation and the map on the page, so **they exist twice** (once in the page's style, once in the island's); the duplication is the price of a frame that does not inherit the page's CSS.

### What would NOT work unchanged, so that nobody finds out later

1. **`site.js` `hero()`** (listens to `pointermove` on the *window*, divides by the *page's* `innerWidth`/`innerHeight`, skips drawing when `.scene` is off screen by `getBoundingClientRect`). In a frame the window is the frame's; the rectangle is the frame's. It is replaced by the glue and the `Pointer` prop; the off-screen skip becomes an `IntersectionObserver` in the frame (works, 3).
2. **The page's observer for the cairns** (`io.observe(.cairn)` sets `.in` on a *page* element): the element is now inside a frame. Same code, in the glue.
3. **The `.scene` chrome** (28 px radius, ring, shadow, the sky gradient from `--sky` and `--bg`): it is page CSS around the island. Either a `Card`-like wrapper in the program, or the island's body style; not the island's concern.
4. **The page's CSS variables** (`--accent`, `--g1`, `--ridge0`, `--sky` ...): not inherited. The palette lives in the island's style with a dark variant under `prefers-color-scheme` (measured to work, 2). If the page ever lets the reader choose a scheme independent of the system, the frame will not follow it, because the choice is not a media query; that would need a prop. The cancho-www page has no such switch.
5. **Eleven frames for the cairns** (3): works, but is the cost the decision bought. Options that stay inside the decisions: keep 11 sealed `Cairn` islands (the cost above); or lazily give a frame its `srcdoc` when it nears the screen (a hole filled by the script: saves the 168 kB of HTML, costs the frames' content being absent without script and after a delay). Neither was built. A third (draw the cairns to static SVG at build time) needs a JavaScript engine at build time, which `cancho-ui` does not have.
6. **A `Visible`/pointer prop for the cairns:** not needed (the frame's observer sees the page's viewport).
7. **Anything that reads the page** (`document.querySelector('.reveal')`, `getElementById('top')`): a sealed island cannot, by construction.

## 5. The map as an open island

`map.js` (21,760 B) is not a drawing function: it reads and writes **page elements by id** (`#map-svg`, `#map-chips`, `#map-count`), builds the filter buttons, the detail pane, the demo UI with its buttons and terminal, and sets `window.CanchoMap`; `site.js` (line 259) fetches `ecosystem.json` and `demos.json` and calls it. **It cannot run unchanged as the drawing half of the plan in `www.md` 9** (logic in the program, drawing in the island): by reading, about 140 of its 301 lines (the chips, the detail pane, the demo UI) are the logic that the plan moves to cancho-ui.

* **Run unchanged, as an open island that owns its part of the page:** possible only if the page has elements with those ids (`#map-svg`, `#map-chips`, `#map-count`, `#map-fallback`). The program can give a `Section` or `Article` an `id:`; the id is all `map.js` queries, but the *tag and attributes* of the original (`div.chips role=group aria-label`) are not reproducible with the structure elements, so the DOM would differ from today's page. Not recommended.
* **The decided plan needs a rewritten `map.js`** (by estimate about 150 lines: `SPOTS`, `blob`, `granite`, the trails, the `viewBox` tween) that takes `places`, `trails` and the selection as props and emits `Selected(id)`, with the filters, the count, the detail pane and the repository links written in the program. That is a smaller island with the same API as above plus `sealed: false`: the island gets the **host element** (`cancho.host`) and the page's `document`; it declares **`requires Dom`**; it is what `authority` prints as `UNBOUNDED`.
* **Open island API, additions to 4:** `sealed: false`, `requires Dom;` in the component, a `host` element the compiler draws (a `div` with the island's `id`), keyboard focus order that is the page's (the reason the map is open), and an accessible form that the island cannot supply by itself: the **list under the map stays the text alternative** (`island-text-alternative-missing` if it is absent). The island's scripts still come from a declared, hashed list.
* **Cost.** Not measured as a frame (it is not one). It is in the page's document: today the map is 426 nodes of the page's 2,951.

## 6. How `inspect effects`, `diff`, `report` and `authority` say it (designed, not built)

```
$ cancho-ui inspect effects Page
{ "component": "Page", "requires": [ { "name": "Dom" }, ... ],
  "islands": [
    { "name": "Hero",   "sealed": true,  "decorative": true,  "files": [ {"path": "assets/rocks.js", "sha256": "ab5fffd8..."}, ... ],
      "reaches": "nothing but this page, by messages", "props": ["pointer: Pointer"], "events": [] },
    { "name": "Places", "sealed": false, "authority": "UNBOUNDED (runs plain JavaScript in the page's origin)", ... } ] }
```

* **`inspect effects`** lists every island of the component: sealed or open, each script with its `sha256`, props, events, and for an open one `UNBOUNDED`.
* **`diff`**: a new island, a removed one, a changed `sha256` of any of its files (as an asset change: `~ file assets/scene.js: ab5f... -> c901...`), a changed prop or event list, a change of `sealed`. **`authority: expanded`** for an open island added or a sealed one opened (because `requires Dom` appears); **`authority: unchanged` but a line of its own, ranked first**, for a sealed island added or changed (it reaches nothing). The island's *files* are already covered by the asset pipeline of B3 (size, existence, hash); the scripts are text files, so they get `asset-type-mismatch` for a file that is not text/UTF-8 and a size bound.
* **`report`** (the trust boundaries list that foreign functions use): each island is a boundary; an open one is marked `UNBOUNDED`.
* **`test` (replay)**: an island is **not run**; the report says `island Hero: not replayed`, and the trace records the props sent and the events received. The island's behaviour is checked by a Chromium script the page owns.
* **The page's CSP:** `frame-src about:` is added to the page's `<meta>` whenever it has a sealed island (section 1.1), next to the `img-src` a map gets; a program with an *open* island adds nothing (it runs in the page).

## 7. Islands, prerender and hydration

* **A sealed island is a static element of the skeleton, not a hole.** Its `<iframe srcdoc>` is written by the compiler into `index.html` like an `Image`; the script only *posts* props to it. Without script the visitor gets the frame with its style and no drawing (a blank rectangle with the background of the island's style), and the iframe's `title` (the `label:`) — `aria-hidden` when decorative.
* **Measured: under the P1a script (empties `#root` and rebuilds) an island restarts every time the page renders: 4 starts in one load plus three renders; under P1b (fills only the holes) it starts once and is the same node.** P1b is what makes an animated island workable, and an island is the strongest reason for it.
* **The props handshake** (section 3): the frame posts `ready`; the page's runtime answers with the whole current props; later changes post again. The skeleton hash covers the iframe's `srcdoc`, so a changed island is a changed skeleton and the script of another build leaves the page alone.
* **Bytes.** The hero's `srcdoc` attribute is 18,031 bytes in the HTML (3,854 gzipped) for 5,524 + 3,304 + 718 bytes of script and 8,179 of style; the cairns 168 kB for 11 (7 kB gzipped).
* **An open island is a hole**: it is script in the page's document, drawn into a host element the skeleton holds between markers.

## 8. Rule tags planned (each needs a corpus case when it is built)

`island-undeclared` (a `<script>`, an inline script, a `javascript:` address smuggled through any string), `island-hash-mismatch`, `island-source-missing` (and the asset tags for the files), `island-label-missing`, `island-text-alternative-missing` (an open island that is not decorative has no text alternative element), `island-size-missing`, `island-prop-invalid` / `island-prop-type`, `island-event-undeclared` / `island-event-type` (a message that is not a declared event, or has the wrong type, is dropped and counted: a runtime rule, not a build refusal), `island-open-needs-dom` (`sealed: false` without `requires Dom`), `capability-unused` (the existing tag, for `requires Dom` with no open island), `island-script-order` (a script uses a name no earlier script defines: **not checkable without reading the script, so not offered**).

## 9. What is not measured, and what this design does not claim

* A real phone, Safari, Firefox; screen readers on an iframe; the 4x throttle stands for a slower CPU and nothing more.
* **That the sandbox is complete.** The reach table is 30 attempts; the list of attempts is the thing to extend (the build should run it as a check), and the WebRTC finding is the reason not to trust a short list. The shim that removes the constructors is a defence, not a proof: an island that reaches a pristine `RTCPeerConnection` through some path the 30 attempts do not try (a nested realm is closed by `frame-src 'none'`; no other was found) would not be stopped.
* CPU and memory bounds (the island that loops stalls the page for as long as it loops); timing channels; DNS prefetch.
* Pixel parity of the hero in the frame against the page's inline hero: not compared (the draw is the same files, the stylesheet is a copy; the parity check of the port will compare pixels).
* The glue and the island stylesheet were written for the measurement, not for the port; their size (718 bytes and 8,179 bytes) is what was measured, not what the port will write.

## 10. Decisions for the coordinator

1. **Cairns**: 11 sealed frames (about 1 s at 4x throttle and 168 kB of HTML for 11 decorations), or the lazy `srcdoc` variant in 4.5? Recommendation: build the 11 sealed frames first, measure them on the real page in the port (Q11), and change only if the page's budget says so.
2. **The page's CSP** gains `frame-src about:` when there is a sealed island: this makes any other frame on that page (an embedded video player, say) impossible without listing it. cancho-www has none.
3. **Order of the build**: islands (sealed, then open) as one PR or two? Recommendation: two — the sealed island with the hero and the cairns first (runtime shim, CSP, reach check, the `inspect`/`diff`/`report` lines, the glue and the corpus), the open island with the map after.
