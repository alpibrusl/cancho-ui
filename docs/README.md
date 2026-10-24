# Docs

`README.md` is a thin pointer. This is where the depth lives: what
cancho-ui is, why it exists, what is decided, and an index of every
design document.

Design lands here **before** the code that implements it — the
cheapest place for a claim to be wrong. When it turns out wrong
anyway, the document that made the claim is corrected in place
rather than quietly edited. The
[cancho ROADMAP convention](https://github.com/alpibrusl/cancho/blob/main/docs/ROADMAP.md)
applies here too: corrections live where the original claim was
made, with the date of the correction.

---

## Why

AI coding agents can already produce frontend and mobile code. The
problem is that a successful compilation does not establish that an
application behaves correctly, uses only intended permissions,
handles asynchronous work safely, or remains consistent across
platforms. cancho-ui is the hypothesis that a UI language designed
around these properties — with the compiler exposing the evidence
rather than inferring it — can make AI-driven changes both smaller
in surface and more legible in consequence.

The strongest product story is not "AI writes this language
better." It is: **the toolchain gives AI agents a smaller, more
explicit space in which to make changes — and gives humans evidence
about what those changes can do.** That claim is meaningful only
if a benchmark supports it.

---

## Relationship to cancho Core

cancho-ui is an **official Cancho ecosystem extension**, in a
separate repository, depending on a documented and versioned
integration contract rather than unstable compiler internals.

| What cancho Core owns | What cancho-ui owns |
|---|---|
| Core type and effect semantics that are genuinely shared | Declarative UI source syntax |
| Compiler infrastructure and stable integration points | Component, layout, state, event, lifecycle semantics |
| Defined behavior and deterministic representations | UI-specific effect and capability rules |
| Common authority concepts and machine-readable diagnostics | Typed UI intermediate representation (IR) |
| Versioning and compatibility contracts for shared compiler interfaces | Web and mobile backends |
| | UI testing, replay, accessibility metadata, platform adapters |
| | Agent-facing inspection and semantic-diff commands |

Where core capabilities are not yet available, cancho-ui uses a
small adapter or explicitly marks the behavior as provisional; it
does not force premature changes to cancho Core.

---

## What exists

| | | Settled by |
|---|---|---|
| **The proposal** | Position, MVP definition, phase plan, risks | [`proposal.md`](proposal.md) |
| **Language design** | Source syntax sketch, naming, idioms | [`language-design.md`](language-design.md) |
| **Semantics** | The 12 semantic questions, each labelled by guarantee tier | [`semantics.md`](semantics.md) |
| **Security model** | Five-tier guarantee model, capability/effect rules, trust boundaries | [`security-model.md`](security-model.md) |
| **Platform model** | Web-first decision record, mobile-backend candidates | [`platform-model.md`](platform-model.md) |
| **Typed UI IR** | Component, state, event, effect, capability, lifecycle representation | [`../spec/ir-schema/README.md`](../spec/ir-schema/README.md) |
| **Adapters** | `Storage` (with `Storage("ns")`), `Network` (`net.get`, with `Network("host")`) and `Location`, built: the calls, what each guards, replay, what is not built | [`adapters.md`](adapters.md) |
| **Gallery** | The landing page's gallery: seven live demos built from `examples/` and ten "coming soon" tiles that each link the issue tracking the gap. `docs/gallery.json` is the manifest; `node scripts/build-gallery.js` rebuilds the demos and the section; `node scripts/check-gallery.js` checks it (and `--self-test` breaks it on purpose) | [`gallery.json`](gallery.json) |
| **Theme** | A program's own colours (16 tokens, light and dark, contrast checked at 4.5:1) and self-hosted fonts with a licence and a SHA-256; step 1 of the cancho-www programme | [`theme.md`](theme.md), [`www.md`](www.md) |
| **Structure** | Headings, sections, landmarks, `Article`, `Link`, `Details`, `Figure`/`Caption`, the roles of a `Text`; step 2 (B1) of the cancho-www programme | [`structure.md`](structure.md), [`www.md`](www.md) |
| **Mobile** | The mobile decision (an installable web app, fewest dependencies), what `build --pwa` writes (and `--page`, a plain page that is not an app), what is and is not built, and how it is checked | [`mobile-backend.md`](mobile-backend.md) |
| **Routing** | `NavLink`, the URL fragment as state, `route Event;` and `router.go`: the options measured (a path deep link is a 404 on a static host), replay, and the build order (built: language, IR, replay and the page, with the Chromium checks) | [`routing.md`](routing.md) |
| **Screen focus** | Where focus goes and what the page title says when a screen changes (`If` appears, a route changes): measured first, the rule, its limits, what replay cannot claim | [`screen-focus.md`](screen-focus.md) |
| **Bearer token** | A token the program receives from a login and sends: where a secret may live (cookie, memory, `Storage`) measured against a script of the origin, `credential.set`/`clear`, replay, what the build corrected, what is not done | [`bearer-token.md`](bearer-token.md) |
| **File upload** | `FileField`: a native file picker whose chosen file reaches the program as one JSON `String` (text files, bounded), measured against the browser, sent with the existing `net.post`; what the build found; what is not done (binary, multipart) | [`file-upload.md`](file-upload.md) |
| **Charts** | `Chart`: an SVG line or bar chart drawn from a list of numbers, with a text alternative derived from the data and a collapsed data table; why SVG and not canvas, the limits, colour, what replay can assert, what the build found | [`charts.md`](charts.md) |
| **Streams** | `subscribe`: messages a server pushes (Server-Sent Events over a `fetch` stream, not `EventSource`), the reconnect and backoff the page owns, hidden pages, size limit, authority under `Network`, replay, `examples/stream` (built) | [`streams.md`](streams.md) |
| **Internationalization** | message catalogs checked at compile time, formatted numbers and dates from tables (not `Intl`: measured), plural rules, `lang` and `dir`, right-to-left (built) | [`i18n.md`](i18n.md) |
| **Drag and drop** | reordering a list: `SortList`, `move`, a keyboard path that is the element's, pointer events, no rebuild under a drag, focus that follows the row (built) | [`drag-and-drop.md`](drag-and-drop.md) |
| **Maps** | `distance(a, b, default)` and a nearest-places list (built), and `Map`: a static tile map behind a host the program declares (no default host), enforced by a Content-Security-Policy, centred on the tile not the position, with its list as the accessible form, required attribution and replay reporting the exact tile addresses (built); what a tile request carries, offline, the quota cost of keeping tiles | [`maps.md`](maps.md) |
| **Nesting** | one bound (32) on expressions, view nodes and types, and bounds on chains of blocks and of imports: the depth at which each construct overflowed the stack, why a bound and not a bigger stack, the time a deep view took (built) | [`nesting.md`](nesting.md) |
| **Number input** | `TextField { type: "number" }`: a text field with a decimal keyboard that gives the handler the number typed in the language of the component (`1,5` is `1.5` in Spanish), keeps what was typed through the redraw, and why not the browser's `<input type=number>` (built) | [`number-input.md`](number-input.md) |
| **Work orders** | A realistic page (sign-in with validation, list, detail, edit posting with `net.post`) built from what exists, and the six gaps it hit, each a filed issue | [`tickets.md`](tickets.md) |
| **View conditional** | `If { when: cond .. }`: a view node that is in the page only while a condition holds; the options, why replay answers `hidden`, the instance case, and what it cost | [`view-conditional.md`](view-conditional.md) |
| **Handler sequences** | The statement `if` (its branches may hold effects) and `block`/`run`: the options, the revision of the conditionals decision, how replay and the page agree, and what it cost | [`handler-sequences.md`](handler-sequences.md) |
| **Comparisons** | `==`, `!=`, `<`, `<=`, `>`, `>=` across types: the measured table of what replay and the page answer, the rule that refuses the pairs they disagree on, and why `json.get` has no numeric sibling | [`comparisons.md`](comparisons.md) |
| **Semantic diff** | What `cancho-ui diff` compares, its JSON shape, exit codes, and what it does not claim | [`semantic-diff.md`](semantic-diff.md) |
| **Schedules** | `start Event;` and `every N Event;`: when they run, skip-not-queue, disposal, replay and diff | [`schedules.md`](schedules.md) |
| **Replay** | What `cancho-ui test` replays (a trace of events, a scripted network, a fake clock), its rules, and what it does not claim | [`replay.md`](replay.md) |
| **Diagnostic format** | Rule codes, severity, span, subject, expected/actual, remediation, tier | [`../spec/diagnostics/README.md`](../spec/diagnostics/README.md) |
| **Grammar sketch** | EBNF for the syntax in `language-design.md` (Phase 0 only — not implemented) | [`../spec/grammar/README.md`](../spec/grammar/README.md) |
| **Valid and invalid examples** | At least 10 of each, written down, the contract Phase 1 must meet | [`../examples/README.md`](../examples/README.md) |

What is **not** there yet, and why, is
[`ROADMAP.md`](../ROADMAP.md).

---

## The index

| Doc | Purpose | Status |
|---|---|---|
| [`proposal.md`](proposal.md) | Initial proposal and roadmap | written, under review (Phase 0) |
| [`language-design.md`](language-design.md) | Source language syntax and design decisions | Phase 0 |
| [`semantics.md`](semantics.md) | The 12 semantic questions, with guarantee tiers | Phase 0 |
| [`security-model.md`](security-model.md) | Five-tier guarantee model and trust-boundary contract | Phase 0 |
| [`platform-model.md`](platform-model.md) | Web-first decision; mobile-backend candidates | Phase 0 |
| [`semantic-diff.md`](semantic-diff.md) | `cancho-ui diff`: design, JSON shape, exit codes, limits | Phase 3, built |
| [`replay.md`](replay.md) | `cancho-ui test`: trace format, replay semantics, rule tags, limits, measurements | Phase 3, built |
| [`../spec/ir-schema/README.md`](../spec/ir-schema/README.md) | Typed UI IR sketch | Phase 0 |
| [`../spec/diagnostics/README.md`](../spec/diagnostics/README.md) | Machine-readable diagnostic format | Phase 0 |
| [`../spec/grammar/README.md`](../spec/grammar/README.md) | Grammar sketch (EBNF) | Phase 0 |
| [`../examples/README.md`](../examples/README.md) | Valid and invalid fixtures | Phase 0 |

Every claim in every document names the cancho Core document that
settles it (where it is inherited), or labels itself as net-new
with the source it cites. Anything that cannot do one of those two
is `unknown` and is acknowledged as such.