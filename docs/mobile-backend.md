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

Read from the code generator (`crates/cancho-ui-codegen-web/src/`, `emit.rs` and the modules beside it) and
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

(For a page that must *not* be installable, such as a gallery demo, use `--page`: `app.js` and `index.html` only, §5.2.)

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

### 5.1 What an update looks like, measured (issue #67)

Issue #67 asked that the cache name change with the content so a stale cache is never served for a new build. **It already does**
(the bullet above, and `pwa.rs` test `the_worker_version_follows_every_byte_it_caches`); what the issue did not have was a measurement of
what a user *sees*. `scripts/check-update.js` (Chromium, outside the gate) serves two builds of one program from the same URL, one after
the other:

| Step | What happens |
|---|---|
| Version one installed, reload | version one shown; one cache, `cancho-ui-<16 hex>` |
| The site is replaced by version two; the browser checks for an update | the new worker installs and **waits** (`registration.waiting`); **two** caches exist (the old one is not deleted until the new worker activates) |
| The same tab reloads | **still version one**; the new worker is still waiting |
| A second tab is opened while the first is open | **version one** too |
| Every old tab is closed, then a fresh tab is opened | **version two**; the old cache is gone (`activate` deleted it) |

**How the script waits (issue #97).** It waits for `registration.waiting`, a condition, with a 60 s ceiling that only turns a hang into a failure (the whole script has a 3-minute watchdog as well). It used to fail in 4 runs of 10 with the machine loaded (6 busy loops on 4 cores) and in none of 10 unloaded.
The cause, measured by logging the worker's `statechange` times against the wait: the predicate was `async () => { await getRegistration(); return !!r.waiting }`, which returns a Promise, and Playwright counts a Promise as true at once, so the "8 second wait" returned after 67 to 129 ms; the worker needs about 90 ms to reach `installed` (88, 104, 107, 118 ms in the
runs where it did), so the check passed or failed on that race. The registration is now kept in the page and the predicate is a plain function. After the change 20 runs of 20 pass at a load average of 18, and the three breaks (a `skipWaiting()` in the worker, a constant cache name, an `activate` that keeps the old cache; `scripts/breaks/update.json`)
are each caught by the script on its own. No timeout was widened.

So the cost of "a page never runs against two versions" is that an installed app stays on the old build **until every window of it
is closed**, however long that takes; a reload does not help. This is the policy chosen in section 5 (no `skipWaiting`) and it is kept.
Reasons, with what each alternative would cost: `skipWaiting()` plus a reload on `controllerchange` would put the new build in the
open tab at once, at the price of losing the state of a page the user is using (a half-typed form) with no way for the program to
save it; a visible "new version, reload" message needs the program to be told an update is waiting, and there is no lifecycle event
in the language (section 6). Neither is built. If it matters for a product, the smallest honest step is that event (issue #67
item 4), not a change to the worker. Deliberate breaks of the check: `skipWaiting()` in the worker (fails "waiting" and "a second tab
shows version one"), a constant cache name (fails five checks, the user would be served the old build for ever).

### 5.2 A page that is not an app: `build --page` (issue #65, first real-device report)

**What was reported.** On Chrome for Android, the landing page's `#gallery` showed a large empty bordered box
where the Stopwatch demo should be, and "Open on its own" made the browser offer to install the stopwatch.

**What was measured** (Playwright `Pixel 7` profile: touch, 412 px, Android UA; `scripts/check-mobile-demos.js`;
the tree served is `git ls-files --cached --others --exclude-standard docs`, that is, what a push publishes):

| Hypothesis | Result |
|---|---|
| The lazy iframe never intersects | **Refuted.** All 7 iframes carry `loading="lazy"` and all load once scrolled into view. |
| The demo's service worker breaks the embed | **A consequence, not the cause.** `sw.js` was never committed (404), so registration failed; the cause is below. |
| The demo script is missing | **Confirmed.** `.gitignore` has `*.js`, so `docs/demo/<id>/app.js` was never in git: the pages were published without their program. All 7 embedded demos were blank, with a 404 for `app.js`, on the committed tree. |
| The install prompt | `--pwa` is *meant* to write a manifest, icons and `display: standalone`; that is what makes a page installable. Intended for apps, wrong for demos. |

Side effect, measured on the working tree (where the files existed): viewing the landing page registered **7 service
workers**, one per demo scope, from iframes of a page nobody asked to install.

**Why `check-gallery` R2 and R3 passed.** Both read the *working tree*, where the ignored `app.js` files exist, so
the demos load clean there. CI runs only `cargo fmt`, `clippy` and `test`; no `scripts/check-*.js` runs in CI. The
check and the thing it checks did not see the same files. `check-mobile-demos.js` exports the tree git would publish.
Desktop vs. mobile played no part: a desktop load of the committed tree is blank too.

**Decision.** Gallery demos are *pages*, not apps. `build --page -o <dir>` writes `app.js` and `index.html` only: the
same `index.html` as `--pwa` minus the `theme-color`, manifest, icon links and service-worker registration
(`--pwa` output is byte-identical to before). `--page` and `--pwa` together, and `--page` with `--no-auto-mount`, are usage
errors. The "Open on its own" link now opens a plain page. `build-gallery.js` uses `--page`; `.gitignore` excepts
`docs/demo/*/app.js`; `crates/cancho-ui/tests/gallery_demos.rs` (in the cargo gate, so CI runs it) asserts that each
committed demo equals a fresh `--page` build, that the file set is `app.js`, `index.html` and `api/`, that no file is
git-ignored, and that the landing page links every demo it embeds.

**The check.** `node scripts/check-mobile-demos.js`: each demo, embedded and on its own, in the mobile profile: renders,
`loading="lazy"`, no 4xx / failed request / console error, no manifest link, an empty `Page.getAppManifest().url`, no
service-worker registration. `--self-test` applies six breaks (script not published; manifest link re-added; worker
registered; iframe not lazy; demo throws; demo blank) and requires each to be caught. Against the old committed tree it
fails as the phone did.

**Not verified.** The live GitHub Pages URL (the proxy refuses `github.io`); a real Android phone; iOS Safari; the real install
prompt. `Page.getInstallabilityErrors` returns `[]` in headless Chromium *with no manifest at all*, so install
eligibility cannot be measured headlessly; the check asserts the cause (no manifest, no worker) instead. Whether a
real phone's lazy loading matches the emulation is untested; the cause found fully explains the blank box, so it is
not needed to explain it.

## 6. Not done, on purpose

- **Real adapters.** *(`Storage` over `localStorage` and a same-origin `net.get` are built: [`adapters.md`](adapters.md). The rest of
  this bullet is still open.)* A `fetch`-based `Network`, a `Storage` over `localStorage`
  or IndexedDB, and `Location` over the Geolocation API are the work that makes
  the capabilities real on every web target, phones included. Each needs its
  own design (what the capability allows, what the trace replays). They are the
  next slice after this one.
- **Lifecycle.** *(Built for schedules, 2026-10-20: `schedules.md` section 11. The page's visibility pauses `every`
  timers; `pagehide` pauses and `pageshow` with `persisted` resumes. This bullet used to say `pagehide` maps to disposal;
  it does not, because a page restored from the back-forward cache cannot be restored from disposal.)* Beyond schedules, a
  component does not know the page was hidden, and there is no `on Pause` or `on Resume` event.
- **Icons.** The generated icons are placeholders and there is no `--icon` option. *(Corrected, #67: the manifest now also declares
  each icon as `"purpose": "maskable"`, a separate entry from `"any"` as the specification recommends. The same file serves both: it is
  full-bleed (the background fills the image, so a circular or rounded mask shows no transparent corner) and its square mark has its
  farthest corner at 28.3% of the image from the centre, inside the maskable safe zone, a circle of radius 40%. Checked by
  `pwa.rs` test `the_maskable_icon_keeps_its_mark_inside_the_safe_zone` and in Chromium by `scripts/check-pwa.js`: the parsed manifest
  has both maskable sizes, no installability errors, and in the decoded 512 px icon no mark pixel lies outside the safe zone and
  all four corners are opaque. A break that makes the mark 80% wide fails both. What a real launcher draws, on Android or
  elsewhere, is not measured: #65.)*
- **Install prompt (#67 item 3, a decision note).** In Playwright's headless Chromium, with the page installable and the worker
  active, `beforeinstallprompt` did **not** fire (zero events after install and a reload). So nothing here can show what a page would
  do with it, and the question of an install button is left open: it would need a capability (the event is a user-gesture grant,
  as in `docs/adapters.md` for other grants) and a place in the language for the button's event, and it should be designed once
  the real-device run (#65) shows the event on Android Chrome. Browsers also offer install from their own menu, which needs nothing.
- **Lifecycle beyond schedules (#67 item 4, a decision note).** Schedules pause when the page is hidden. Nothing else is told:
  there is no `on Resume` and no `on Pause`, so a program cannot refresh its list the moment the app is brought back, nor save a
  draft when it is hidden. A page restored from the back-forward cache keeps its state; one that was discarded starts again, and only
  what the program kept in `Storage` survives (docs/adapters.md section 20). The smallest step would be one event, `on Show`, run
  when the page becomes visible after having been hidden, through the same path as `every`'s resume, with a replay step
  (`{"show": true}` beside the existing hide/show steps of `docs/schedules.md` section 11). That is a language change; it is
  proposed here, not built, and the update question above is the reason to want it.
- **Native shells (levels 1 and 2), a React Native comparison, push
  notifications, background work, app-store distribution.** Not attempted. Browsers
  restrict push and background work for installed web apps, more on iOS; this
  note does not claim what iOS permits.
- **iOS Safari.** Nothing here is tested on iOS. The checks of §7 run in
  Chromium (Playwright device emulation) only.

## 7. How it is checked

*(Added, #67: `scripts/check-update.js` measures the update sequence of section 5.1 in Chromium, and `scripts/check-pwa.js` checks the
maskable icon. Both are outside the gate.)*

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
- **Chromium reports no installability errors** (`Page.getInstallabilityErrors` is empty; *corrected, §5.2:
  this is empty in headless Chromium with no manifest at all, so it shows nothing about installability*) and the
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
- **Not measured:** iOS Safari, Lighthouse (a real phone was first reported on in §5.2: Chrome for Android, by screenshot description, not measured), update behaviour (a new version
  taking over after the old tabs close), and what any browser does with the install prompt.

---

## 9. The emitted module: embedding it, and what it guarantees

> **Status: built (written before the code; measured in 9.4).** Asked for by `alpibrusl/cancho-ui#20`, found when
> `alpibrusl/cancho-industrial#9` (`docs/ui-spike.md` there) compiled the emitted JavaScript into a program that has no file
> access and had to embed it as a string literal in a language with a small escape set.

### 9.1 Mounting is a choice: `--no-auto-mount`

`cancho-ui build` ends the module with a side effect: it looks up `#root` and mounts the last component into it. That
stays the default (nothing existing changes). `cancho-ui build <file.cui> --no-auto-mount` omits those two lines and keeps
the export, so a page that imports the module mounts it where it wants, and as many times as it wants:

```js
import { mount_Readings } from "./readings.js";   // imports without mounting
mount_Readings(document.getElementById("left"));
mount_Readings(document.getElementById("right"));
```

The flag changes **exactly** the mount lines: with it, the output is the default output with `const root = ...` and
`if (root) { mount_X(root); }` removed (checked byte for byte, 9.4). `--no-auto-mount` with `--pwa` is a usage error:
the installed page's `index.html` relies on the module mounting itself.

### 9.2 The module is ASCII

Every byte of the emitted module is printable ASCII (32 to 126) or a newline or tab. This is a guarantee, not a habit:

* the emitter's own comments and adapters contain no other byte (four comments had a U+2014 dash; they use `-` now);
* a string from the program (text, a label, a state default) reaches the module only through `js_string`, which writes every
  character above 126 and every control character as a `\uXXXX` escape (UTF-16 units, so an astral character is a surrogate pair,
  which JavaScript reads back as the same string);
* the `// Source:` comment shows the source path with every character outside 32 to 126 replaced by `?`.

The value of a string is unchanged: `"caf\u00e9"` in the module is the same JavaScript string as `"café"` was. The guarantee covers
the `.js` module; the PWA's `index.html`, manifest and service worker are separate files and carry the app name as given.

### 9.3 What it touches

`EmitOptions { auto_mount }` and `emit_module_with` in `codegen-web` (`emit_module` is the default-options wrapper, so every
existing caller is unchanged), `js_string`, the `// Source:` line, the four adapter comments, `BuildOpts` and `compile_to_js_with`
in the CLI (and the usage text and `introspect` listing).

### 9.4 Checked

* `cargo test` (`crates/cancho-ui/tests/build_output.rs`, 5 tests): **every example** (14 programs) builds to ASCII; a program
  with non-ASCII text, a label, a state default and an astral character, in a **non-ASCII directory**, builds to ASCII
  with the expected `\u` escapes (a surrogate pair for the astral character) and a `// Source:` line with `?`; the
  `--no-auto-mount` output equals the default output with exactly the two mount lines removed, and keeps the export;
  `--no-auto-mount --pwa` is a usage error (exit 2) and writes nothing; a non-ASCII identifier is refused and nothing is written.
* `scripts/check-embed.js` (Chromium, outside the gate): the default build mounts into `#root` on import; a non-ASCII
  string written as escapes is **the same string on the page** (`café 😀 —`, and a button label `Über`); the
  `--no-auto-mount` build mounts nothing on import, can be mounted twice into two elements, leaves `#root` empty, and its two
  instances are independent (a click in one does not change the other).

**The checks can fail.** Seven deliberate breaks, all killed: `js_string` passing everything through (2 tests, 2 browser
checks), escapes without zero padding (2 tests; the module no longer parses), an astral character losing its low surrogate
(2 tests, 1 browser check: the page shows a different string), the `Source` comment not sanitized (2), the option ignored
(2 tests, 2 browser checks), the export dropped under the flag (2), and an em dash put back in an adapter comment (2).

### 9.5 Not done

A guarantee for the PWA's `index.html`, manifest and service worker (they carry the app name as given); a way to ask for
the mount point by name (`--no-auto-mount` leaves it to the page); a build that emits an ES module without the `export` line;
and non-ASCII identifiers (a component or state name with a non-ASCII letter is `lex-unexpected-char`, so no identifier in the module is non-ASCII; `tests/build_output.rs` pins it).


