# Mobile: a web app the phone installs, and nothing else it depends on

> **Status: design, with the first slice (`build --pwa`) implemented.**
> This answers the Phase 4 question of [`ROADMAP.md`](../ROADMAP.md) in the
> direction that keeps outside technology smallest. It does **not** make
> cancho-ui a native mobile framework, and it does not close Phase 4: the
> React Native comparison in [`platform-model.md`](platform-model.md) is not run.
> A claim that turns out false is corrected here, in place.

## 1. The decision

The mobile target is **the web backend, installed from the phone's browser**: a
Progressive Web App (PWA). The compiler already emits plain JavaScript and DOM
calls; a PWA adds four generated text files and one image, and the program
depends on nothing the user must install except the browser already on the
phone.

The options, by what each one makes you depend on:

| Level | Approach | You depend on | Cost |
|---|---|---|---|
| 0 | **PWA**: the compiled `.js` plus a page, a manifest and a service worker | The phone's browser; HTTPS hosting (a local server on `localhost` works for testing) | Generated files only; no library, no SDK |
| 1 | A small native shell around the system WebView | Android SDK or Xcode to build and sign; store accounts | A shell per OS to keep |
| 2 | Native UI with no WebView | A native toolchain per OS and our own text and widget rendering | Very large |
| – | React Native | Node, Metro, the framework, and both native SDKs | The most dependencies of all |

Level 0 matches the language's own rules ([`platform-model.md`](platform-model.md)
§1: no JSX, no virtual DOM, no framework). Levels 1 and 2 stay available as
later work; nothing here prevents them.

## 2. What the web backend does on a phone today (measured, not hoped)

Read from the code generator (`crates/cancho-ui-codegen-web/src/emit.rs`) and
from compiled output (`examples/search`):

- The emitted JavaScript contains no `import`, no `fetch`, no `localStorage`,
  no geolocation call and no `visibilitychange` or `pagehide` handler.
- **The platform adapters are stubs.** `Network` is `net.search(q)`: a promise
  that resolves after a simulated delay (60 ms plus 45 ms per query character)
  against a fixed six-name corpus, so the demo runs offline. `Clock` and the
  `sleep` adapter use `setTimeout`. There is **no real network adapter**, no
  `Storage` adapter and no `Location` adapter in the web backend. *(Corrected 2026-10-19: `Storage` is built since;
  the paragraph describes the web backend before that change. [`adapters.md`](adapters.md).)* The design
  documents name `Network`, `Storage`, `Clock` and `Location`
  ([`platform-model.md`](platform-model.md) §1); two of the four are not
  written, and the one called `Network` is not a network.
- A component keeps running when the page is hidden. A browser may throttle or
  freeze timers of a background tab; the runtime does not react to that.

So an app built today is, honestly, a **self-contained interactive page**. Making
it a PWA gives it install, offline start and a phone-sized layout. It does not
give it real network, storage or location access; those are adapter work (§6).

## 3. The slice built here: `cancho-ui build <file.cui> --pwa -o <dir>`

```
cancho-ui build counter.cui --pwa -o out/ [--name "Counter"]
```

`out/` gets, and only gets:

| File | What it is | Rule |
|---|---|---|
| `app.js` | The compiled program | Byte-identical to `build` without `--pwa` |
| `index.html` | The page that mounts it | `viewport` with `viewport-fit=cover`, `theme-color`, links to the manifest and icons, the module script, the service-worker registration, and a small baseline stylesheet (§4) |
| `manifest.webmanifest` | The install description | `name`, `short_name`, `start_url` and `scope` `"./"`, `display: standalone`, colours, two PNG icons |
| `sw.js` | The service worker | Precaches every file above; serves them cache-first; refuses nothing and caches nothing else (§5) |
| `icon-192.png`, `icon-512.png` | Placeholder icons | Flat colour with a centred square, written by the compiler with no image library; to be replaced by a real icon later |

Rules of the command:

1. **Same refusals as `build`.** The program is compiled first. A refused program
   exits `1` with the same diagnostic and writes **nothing**: no directory and no
   file.
2. **Exactly one input file, and `-o` is required** and names a directory
   (created if missing). `--pwa` without `-o`, or with two files, is a usage
   error, exit `2`.
3. **Deterministic.** The same source, given by the same path, gives the same bytes (`app.js`
   embeds the input path as `build` always has). There is no clock, no random value and no network
   in the command.
4. **No dependency added.** The compiler writes the PNG itself (stored-deflate
   zlib stream, CRC-32, Adler-32) and hashes with FNV-1a 64. The hash is a cache
   version, **not** a security measure, and the manifest and worker say so.
5. **The name.** `--name` sets `name` and `short_name` and the page title; the
   default is the input file's stem.

## 4. The baseline stylesheet (the page owns presentation, still)

The language has no styling ([`platform-model.md`](platform-model.md) §1: "CSS is
external in Phase 1"), so the generated page carries a small default so that a
program is usable on a phone, and an author replaces it freely:

- `.cui-col` is a flex column with a gap.
- `button`, `input` and `textarea` have `min-height: 44px` (a touch target of
  at least 44 CSS pixels) and `font: inherit` at a 16 px base; 16 px inputs are
  what stops iOS Safari from zooming a page on focus (**not verified here**: no
  iOS device).
- `padding` uses `env(safe-area-inset-*)` so content clears notches.
- `color-scheme: light dark`.

## 5. The service worker

- **Precache, cache-first.** On `install` it caches `./` and every file of §3.
  A `fetch` for a same-origin GET is answered from that cache; a navigation
  that is not cached falls back to the cached `./index.html`; a request that is
  not in the cache goes to the network and is **not** stored.
- **Versioned by content.** The cache name is `cancho-ui-` plus the FNV-1a hash of
  the file names and bytes of everything precached except `sw.js`. Any change to
  the program changes the name; `activate` deletes every other `cancho-ui-*` cache.
- **No `skipWaiting`.** A new version takes over when every tab of the app is
  closed, so one page never runs against two versions. (The browser still checks
  `sw.js` on navigation; it sees the new bytes because the hash is inside it.)

## 6. Not done, on purpose

- **Real adapters.** *(`Storage` over `localStorage` and a same-origin `net.get` are built: [`adapters.md`](adapters.md). The rest of
  this bullet is still open.)* A `fetch`-based `Network`, a `Storage` over `localStorage`
  or IndexedDB, and `Location` over the Geolocation API are the work that makes
  the capabilities real on every web target, phones included. Each needs its
  own design (what the capability allows, what the trace replays). They are the
  next slice after this one.
- **Lifecycle.** Mapping `pagehide` to disposal (`Cancelled`) and
  `visibilitychange` to inactive/active is designed in
  [`semantics.md`](semantics.md) and not wired. Until it is, a component does
  not know the page was hidden.
- **Icons.** The generated icons are placeholders. A `--icon` option, and a
  maskable variant, are not built.
- **Native shells (levels 1 and 2), a React Native comparison, push
  notifications, background work, app-store distribution.** Not attempted. Browsers
  restrict push and background work for installed web apps, more on iOS; this
  note does not claim what iOS permits.
- **iOS Safari.** Nothing here is tested on iOS. The checks of §7 run in
  Chromium (Playwright device emulation) only.

## 7. How it is checked

Two layers, kept apart:

1. **In the gate** (`cargo test`): the files exist and only they; `app.js` equals
   the plain build; the manifest is valid JSON with the required members and its
   icons exist; each PNG has a valid signature, `IHDR` size and chunk CRCs; every
   file the worker precaches exists and is listed; two builds are byte-identical;
   a refused program writes nothing; `--pwa` without `-o` and with two files exit
   `2`.
2. **Outside the gate** (`scripts/check-pwa.js`, needs Node, `playwright` and
   Chromium, like `scripts/record-demo.js`): served over `http://localhost`,
   Chromium reports **no installability errors**, the page works **offline after
   the first load**, and at a phone viewport every interactive element is at
   least 44 CSS pixels in both directions.

## 8. Measured

Run with `node scripts/check-pwa.js` (Chromium, Pixel 5 emulation, `http://localhost`) on
`examples/counter` and `examples/search`, 2026-10-19, Linux x86-64. The generated app, in bytes:

| | `app.js` | `index.html` | `manifest.webmanifest` | `sw.js` | `icon-192.png` | `icon-512.png` | total |
|---|---:|---:|---:|---:|---:|---:|---:|
| counter | 2,170 | 1,259 | 430 | 1,448 | 4,886 | 33,366 | 43,559 |
| search | 8,582 | 1,258 | 428 | 1,448 | 4,886 | 33,366 | 49,968 |

Built from a relative path, as above. The icons are palette images in stored (uncompressed) deflate
blocks: 77 % of the size is the 512 px placeholder icon. `app.js` embeds the input path as given
(an existing property of `build`), so building from an absolute path makes it longer by the
path's length; the script's own totals (43,580 and 49,989) are that, 21 bytes more.

- **Runtime dependencies: none.** `app.js` has 0 occurrences of `import`, `fetch(` or `require(` in
  both programs; nothing is fetched at run time.
- **Chromium reports no installability errors** (`Page.getInstallabilityErrors` is empty) and the
  manifest has a URL, the name, and no parse errors, for both programs. (An absent manifest is
  reported as an empty one with no errors, so the check requires the URL and the name too.)
- **One cache**, named `cancho-ui-` and the content hash, after two loads.
- **Every interactive element is at least 44 x 44 CSS px** (2 in the counter, 4 in the search page).
- **Offline start works** with the server really stopped (Playwright's `setOffline` does not reach a
  service worker's own requests, so it is not used): the page mounts from the cache, and a tap
  changes the counter (`Count: 0` to `Count: 1`).
- **The checks can fail.** Three mutants of the generator, each caught: the manifest link removed
  (the manifest check fails), the 44 px rule removed (buttons measure 30 px high, the size check fails),
  and `app.js` left out of the precache list (the offline start fails). The first run of the script
  passed two of these mutants: it trusted an empty manifest and used `setOffline`; both were fixed
  and the mutants re-run.
- **Not measured:** iOS Safari, a real phone, Lighthouse, update behaviour (a new version
  taking over after the old tabs close), and what any browser does with the install prompt.
