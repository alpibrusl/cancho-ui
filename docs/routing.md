# Routing: `NavLink`, the URL as state, and the back button

> **Status: BUILT (issue #75): language, IR and replay in step 1 (section 12), the page, the Chromium checks and the gallery demo in step 2 (section 13).** Written 2026-10-23 before any code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). The numbers in
> section 2 were measured in Chromium (Playwright, `Pixel 7` profile, a stand-in for a static host that answers an unknown path with 404
> as GitHub Pages does) on main at 02f6763. A claim found false while building is corrected here, in place, and listed in section 9.
> Guarantee tier of the replay semantics: **tested** once built; of the browser behaviour: **observed in Chromium**, not guaranteed.

## 1. The decision, in one paragraph

A route is the **fragment of the URL** (`#/items/3`). Three small pieces, no router:

| piece | what it is | authority |
|---|---|---|
| `NavLink { label: "Item 3" to: "/items/3" }` | a view element: a real `<a href="#/items/3">` | none: it can only point at this document's own fragment |
| `route Navigated;` | a component member, next to `start` and `every`: runs the event `on Navigated(path)` once after the first render with the current path, and again at every change of the fragment (a link, Back, Forward, a typed URL, `router.go`) | `requires Router;` |
| `router.go(path)` / `router.replace(path)` | builtins that push or replace the URL entry; `Bool`, synchronous, like `storage.set` | `requires Router;` |

The program decides what a path means: it keeps the path (or what it makes of it) in state and draws screens with
`If { when: .. }` ([`view-conditional.md`](view-conditional.md)), parsing with the text builtins that already exist
(`split(path, "/")`, `take`, `skip`, `join`: [`lists.md`](lists.md) section 7). The language gets no route table, no pattern syntax and
no history object.

## 2. What exists today, measured

1. **No navigation.** `NavLink { .. }` is `component-kind-unknown`; no crate touches `location.hash`, `history.*` or `hashchange`
   (`grep` finds nothing, as #75 recorded).
2. **A screen is state, so Back leaves the app.** A two-screen page (`If { when: screen == "list" }` / `"detail"`), Open pressed:
   the text changes to "the detail", `history.length` is 2 before and 2 after, the URL is unchanged, and the browser's Back goes to
   `about:blank`. A reload returns to the list, and typing `#/detail` into the URL shows the list (nothing reads the fragment).
3. **A deep link by path does not work on a static host.** The app served under `/app/` with a 404 for any unknown path:
   `/app/items/3` is **404** on a cold visit, and **404 again with the service worker installed and controlling the page** (its fetch
   handler passes an uncached request through; it falls back to `index.html` only when the network fails). Offline, the fallback
   answers 200 with the right `index.html`, but the page is **blank**: the relative `./app.js` resolves to `/app/items/app.js`, which
   is in no cache. Path routing would need `<base>` or absolute asset URLs, a `404.html` copy of the page, and a path prefix per
   host; GitHub Pages (the gallery) is exactly such a host.
4. **A deep link by fragment works, online and offline.** `/app/#/items/3`: status 200, the app mounted, `location.hash` kept;
   with the server stopped and the service worker installed, the same URL mounts (two buttons). The fragment is never sent to the
   server, so the precache (`ignoreSearch`) matches `index.html` and nothing else is needed.
5. **The browser already does the history.** Two `<a href="#/b">` / `<a href="#/c">` taps: `history.length` grows by 2; Back returns to
   `#/b`. Events are `popstate` then `hashchange` for each; assigning `location.hash` to the value it has fires **nothing** and adds
   no entry; `history.replaceState` fires **nothing**; `location.hash` is updated at once while the events come after the current task.
6. **Focus and the title do not follow.** After the screen changed, `document.activeElement` is `BODY` and `document.title` is still the
   program's name (#92 measured the same after sign-in). That is not routing's to fix (section 7).

## 3. Options

| | Option | For | Against |
|---|---|---|---|
| A | **Fragment routes, an event with the path, `NavLink` as an anchor** (chosen) | works on any static host and offline (4); Back, Forward, middle-click, "copy link" are the browser's (5); the program owns the meaning, no new matching language; one new element, one member, two builtins | URLs carry `#/`; the server cannot see the route; a fragment that is not a route needs a rule (section 4) |
| B | History API paths (`/items/3`) | clean URLs | fails measure 3 on the gallery's own host; needs a server rule or a `404.html` trick, a base path, and a service worker change; each host differs |
| C | A route table in the language (`route "/items/:id" -> Detail`) | declarative; replay could name the matched route | a pattern language and a parameter type system for one feature; the text builtins already parse; a route per component multiplies the IR |
| D | State bound to the URL (`state page: String in url`) | tiny | one field cannot hold a path and its parts; no event, so a change of URL cannot run a handler or a request; hides the authority |
| E | Do nothing | | measure 2: Back leaves the app |

**Chosen: A.** B is rejected by a measurement, C and D by what they add for what the builtins already give. Server-side
routes, path URLs and nested routers stay possible later without changing what A means (section 8).

## 4. The form and its rules

```
component Shell {
    requires Router;
    state path: String = "/";
    route Navigated;
    on Navigated(p) { path = p; }
    view {
        Column {
            NavLink { label: "Home" to: "/" }
            If { when: path == "/"
                NavLink { label: "Item 3" to: "/items/3" }
            }
            If { when: starts_with(path, "/items/")
                Text { "item " + join(skip(split(path, "/"), 2), "/") }
            }
        }
    }
}
```

* **The path** is the fragment without its `#`, verbatim (no decoding, query and all): `#/items/3?x=1` is `"/items/3?x=1"`. An empty
  fragment (or a bare `#`) is `"/"`.
* **A fragment that does not start with `/` is not a route.** `#main` (a skip link, an in-page anchor) does not run the event and does
  not change the program's screen; the browser scrolls as it always did. Otherwise a skip link would send the program home.
* **When the event runs.** Once after the first render (in the order the `start` and `route` lines are written), then whenever the
  fragment changes to a route, whatever caused it. Setting the fragment to the value it has fires nothing (measure 5), and replay agrees.
  The event takes exactly one `String` parameter. Its run is an ordinary handler: `await`, supersede, `Cancelled` and replay all
  apply (the same rule as a schedule, [`schedules.md`](schedules.md) section 1).
* **`NavLink`** takes `label:` (drawn as the text and the accessible name) and `to:`, a `String` expression that is the path. The page
  writes `href="#" + to`, always: a `NavLink` cannot name another origin or a `javascript:` URL, which is why it needs no capability.
  A literal `to:` that does not start with `/`, or holds a space, `#`, `"`, `<`, `>` or a backtick, is refused. It has the baseline's 44 px target
  ([`mobile-backend.md`](mobile-backend.md) section 4). An external link is **not** this (section 8).
* **`router.go(path)`** pushes an entry, **`router.replace(path)`** replaces the current one. Both answer `true` when the URL is now
  `path`, `false` when the path is refused (it does not start with `/`, is over 2,000 characters, or holds a character outside printable
  ASCII): a bad path is a value, never a throw. The route event then runs after the handler returns (measure 5: the browser's events come
  later), and not at all if the path was already the current one.
* **`requires Router;`** is needed by `route`, `router.go` and `router.replace` (`capability-missing` as for any effect). The fragment is
  input from whoever made the link, and may carry a token; the capability is what puts that on the report (`inspect effects`, `diff`:
  adding it is a widened authority). `Router` takes no bound.
* **Back and Forward are the browser's.** The program has no `router.back()`: a Back control in the page is a `NavLink` to the parent.

## 5. Replay

Replay does not draw; it needs the URL and the history the page would have. New, in the trace:

* `"url": "/items/3"` at the top: the fragment at load (default `"/"`). The route event runs after the first render with it.
* `{ "navigate": "/items/4" }`: the user followed a link or typed a URL; the path becomes current, a new entry is pushed, the event
  runs. The same path as the current one runs nothing (`"outcome": "unchanged"`).
* `{ "back": true }` and `{ "forward": true }`: move one entry along the history replay keeps; the event runs with the path reached
  (when it differs from the one left). Nothing to go back or forward to is a refused step (`trace-history-empty`).
* `router.go` / `router.replace` are effects like `storage.set`: journalled, updating the same history, queuing the event.
* The report prints, for a component that `requires Router;`, `url`, `history` (`entries`, `position`) and `routes` (the paths the route event
  received in the step, in order), and `expect` may assert any of them. The human report prints `route <path>` and `url=`.

What replay cannot say: that the fragment of a *real* page is a route (the page decides in the browser), so a trace's `navigate` of a
non-route is refused (`trace-navigate-invalid`), and the page-side rule of section 4 is checked by the Chromium corpus only.

## 6. The checks the build must have

1. **A shared corpus** (`tests/routing_corpus.json`), read by `cargo test` (replay) and `scripts/check-routing.js` (the emitted page in
   Chromium, mobile profile): initial URL, link, `router.go`, `router.replace`, Back, Forward, a typed fragment, a fragment that is not a
   route, the same path again, a bad path. Each step says the path the route event received and the screen text; both evaluators must agree.
2. **A reload on a detail URL, online and with the server stopped** (measure 4 as a check, not a hope), and the gallery's embedded and
   standalone load (`check-mobile-demos.js` covers every gallery demo).
3. **Deliberate breaks:** the event not run at mount, run for a non-route fragment, `href` without the `#`, `go` replacing and not pushing,
   Back ignored, the same hash running the event twice, `to:` accepted without a leading `/`, the capability not required.
4. The gate, and the diff: `requires Router;` shows as authority; `NavLink` and `route` are in `inspect` and in `diff`.

## 7. #92 (focus and the title on a screen change) is separate

It does not fall out of this. Routing changes state; the screen is replaced by the `If`s, and the focus is lost to `BODY` for **any** state
change that removes the focused node, with or without a route (measure 6, and the sign-in in `examples/tickets`). The repair is a rule
about re-rendering ("where does focus go when its node is removed, and is a change of screen announced"), and the page title is a second
binding. Both would be needed by a routed app and by a sign-in alike, so #92 follows this work as its own design and PR, and the routing
example will use it when it lands. Until then the routing example says, in its text, that focus returns to the top of the page.

## 8. Not done, on purpose

* **Path URLs** (option B), a `<base>` and a `404.html`: a host-by-host decision (measure 3), reopened when a host needs it.
* **Route patterns, parameters, nested routes, guards**: the program parses and decides (`If` on the path, a sign-in screen as state).
* **Query-string parsing**, percent-decoding, a typed path: text only, as for `net.get` URLs.
* **`router.back()`**, scroll restoration (not measured), transitions.
* **External links** (`Link { to: "https://.." }`): leaving the document is navigation with its own authority, a separate decision; `Link`
  stays refused.
* **Page title and focus**: #92, section 7.
* **iOS Safari and a real phone**: not measured; the hash events are standard, but this note rests on Chromium emulation.

## 9. Corrections

* **2026-10-23, found building step 1.** The sample in section 4 wrote `on Navigated(p: String)`. An event's parameters have no types in this language
  (`on Go(p)`), so the event is `on Navigated(p)` and `route-event-params` can check the count only; that the argument is a `String` is by
  construction (the page and replay both pass one). Sections 1 and 4 are corrected above.
* **2026-10-23.** Section 5 wrote `{ "back": 1 }`. Like `hide` and `show` the steps are `{ "back": true }` and `{ "forward": true }`, one entry each;
  corrected above.
* **2026-10-23.** Section 4 says `router.replace` fires the route event when the path changes. Measure 5 found that `history.replaceState` fires **nothing**,
  so the page cannot use it for `replace`. Measured in step 2 before building it: `location.replace("#/r")` fires `hashchange` (`/q` to `/r`), adds no entry
  (`history.length` unchanged) and fires nothing when the fragment is already `#/r`. The page uses it; replay's single entry and queued event agree
  (the corpus checks both).
* **2026-10-23, found building step 2: five more characters are not legal in a path.** Sections 4 and 5 and the step-1 code allowed every printable ASCII
  character except space and `#`. Measured: assigning `location.hash` a path with `"`, `<`, `>` or a backtick makes `location.href` and `location.hash` read it back
  **percent-encoded** (`/a%22b`), so the route event would hear a path other than the one `router.go` was given. Every other printable ASCII character,
  `%` included, reads back as written (the other 89 of the 93 tried, one at a time; `#` was not tried). The legal set is now printable ASCII other than space, `#`, `"`, `<`, `>` and the
  backtick (`crates/cancho-ui-ir/src/routing.rs`, the adapter, the corpus, a deliberate break each). A path with a quote or an angle bracket is answered
  `false` by `router.go`, refused by a literal `to:` (`navlink-target-invalid`) and by a trace (`trace-navigate-invalid`).

## 10. Rule tags this adds (each to be documented in `spec/diagnostics/README.md` with its test)

`route-event-unknown`, `route-event-params` (not exactly one `String` parameter), `route-duplicate` (two `route` lines in a component),
`navlink-incomplete` (no `label:` or no `to:`), `navlink-target-invalid` (a literal that is not a path), `binding-unknown` (`to:` off a
`NavLink`), `capability-missing` (`Router` not required), `trace-navigate-invalid` (a `navigate` step or the trace's `url` that is not a path), `trace-history-empty`, `replay-route-limit` (a step that runs
more than 1,000 route events, because a handler navigates to a new path every time), and `builtin-shape-mismatch` for a wrong `router.go` call.
`routing-not-emitted` existed in step 1 only (`build` refused a routing program until the page was written); it is gone.

## 11. Order of the build, in separate PRs

1. Language, IR, diagnostics and replay (`NavLink`, `route`, the builtins, the trace steps), with tests.
2. The emitted page (the adapter, the anchor, the event), the corpus in Chromium, the breaks, `examples/routes` (a list and a detail, with a
   deep link), a gallery demo that shows it on a static host, and this note's section 2 re-measured on the built page.
3. #92 as its own design.

## 12. Step 1, built: language, IR and replay (2026-10-23)

* **Syntax and IR.** `route Event;` is a schedule of kind `route` (`inspect ir` says `"kind": "route"`); `NavLink` is an element (`IrElementKind::NavLink`, JSON
  name `NavLink`); `to:` is parsed as an expression and lowered like the other constant-expression bindings. The checks live in `crates/cancho-ui-ir/src/routing.rs`
  (and `controls.rs` for the bindings), which keeps `lower.rs` under the 2,000-line ceiling. A `route` is also recorded as a reachable effect of capability `Router`,
  so a component without `requires Router;` is `capability-missing` and `inspect effects` and the semantic diff list it with no new code; the diff shows
  `+ schedule route Event` and an `expanded` authority (tested).
* **Builtins.** `router.go(path)` and `router.replace(path)` are `Bool`, synchronous, one argument (`builtin-shape-mismatch` otherwise); a path that is not
  legal (section 4) answers `false` and moves nothing; the current path answers `true` and fires nothing.
* **Replay.** The machine keeps the history (`router.rs`) and runs the route event after the handler that queued it returned (and after a `navigate`, `back`,
  `forward`); a run may queue more, up to 1,000 per step. The first step also reports the mount's route event, so a trace's first step asserts it.
* **What the example showed.** `examples/routes` replays 12 steps and 41 checks. The first draft of its trace pressed `Jump` while the page was at a detail
  path; replay answered `hidden` (the button is under an `If` that is false), which is the existing outcome doing its job.
* **Checked.** `crates/cancho-ui/tests/routing.rs` (17 tests: every refusal above reached, the replay semantics, the diff) in the gate; seventeen deliberate
  breaks (`scripts/breaks/routing.json`), each caught.
* **Not in this step.** The page: nothing is emitted for a routing program (`build` says `routing-not-emitted`), so the Chromium corpus, the `replace`
  measurement above, the reload and offline checks and the gallery demo are step 2. The gallery tile now says so and its probe is a `build` that must
  still be refused (`check-gallery` R5).

## 13. Step 2, built: the page (2026-10-23)

* **The page.** `NavLink` is an `<a href="#" + to>` with its label as text (the `#` is written by the page, so a link cannot name another origin or a `javascript:`
  URL). A component with `route E;` registers one `hashchange` listener, reads the path from the event's `newURL` (two changes in one task are heard as two paths,
  in order), calls `E` once after the first render with the fragment it loaded at (`/` when there is none or it is not a route), and removes the listener when the
  component is disposed. `router.go` assigns `location.hash` (a new entry), `router.replace` calls `location.replace('#' + path)` (measured above); both answer
  `false` for an illegal path and `true` without touching the URL when the fragment is already the path. The adapter (`ROUTER_ADAPTER_JS`) is in the module only
  when something uses `Router`; a link alone needs none. `routing-not-emitted` and its test skip are gone.
* **Checked in Chromium** (`scripts/check-routing.js`, Pixel 7 profile, a stand-in static host that serves under `/app/` and answers 404 to anything else):
  the shared corpus `tests/routing_corpus.json` (34 steps; `cargo test` replays the same file), checking per step the paths the route event heard, the state, the
  fragment and how many history entries the step added (a link: 1, Back/Forward/replace: 0, a repeated link: 0); then the visitor's path (a deep link mounts the detail, a
  reload stays on it, a link, Back and Forward, Enter on a focused link, a path with no page); then the `--pwa` build with its service worker: a deep link and a reload on
  the detail work **with the server stopped**, and `/app/items/gauge` is a 404 online. 49 checks. The corpus covers ten illegal paths (empty, no slash, space, `#`,
  `"`, `<`, `>`, backtick, non-ASCII, 2,001 characters), five legal odd ones (`%20`, a query, `//`, every symbol that is legal, 2,000 characters), a redirect
  (the event for `/old` replaces the entry with `/new` and hears it), two changes in one handler, a typed URL and a fragment that is not a route.
* **Deliberate breaks.** `scripts/breaks/routing-page.json`: href without `#`, no listener, no mount event, location read instead of `newURL`, `replace` that
  pushes, a non-route heard as `/`, a quote allowed, no length limit, `go` always `false`, the listener never removed; each caught (two by `cargo test`, the rest in
  the browser).
* **Gallery.** `examples/routes` is the eighth live demo ("Screens at their own URLs"), built with `--page`, and the "Multi-page routing" tile is gone from the
  coming-soon list (its probe, `build` refused as `routing-not-emitted`, is false now: `check-gallery` R5 said so). R8 compares the sentence "N of them run live" with the
  manifest by number word. `check-mobile-demos.js` loads it embedded and on its own in the mobile profile.
* **Not verified.** A real phone and iOS Safari (the events are standard, the evidence is Chromium emulation). Back inside the gallery's iframe: the iframe's fragment
  changes join the page's session history, which this check does not exercise. Focus and the page title after a route change are still #92 (focus goes to the top of
  the page when the link that was followed disappears with its screen). The service worker's cache is keyed without the fragment, so one precached `index.html`
  serves every route; the 404 of the path form is the host's, not the worker's.
