# Platform adapters: `Storage` and `Network`

> **Status: designed, and two adapters built: `Storage` (sections 1 to 8) and `Network` as `net.get`
> (section 9).** Written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1); where building
> disagreed with it, the correction is made in place and listed in section 8. *(Corrected: `Location` (section 11) and the `Storage("ns")` narrowing (section 13) are built since.)*
> This answers the first item of [`mobile-backend.md`](mobile-backend.md) section 6 ("real adapters").

## 1. What exists, and what this adds

[`platform-model.md`](platform-model.md) section 1 names four adapters. Measured on the emitted code
(2026-10-19): only `Clock` and `Network` exist, both stubs (`Network` is `net.search`, a simulated search over a
fixed corpus); `Storage` and `Location` were not written. A program therefore could not keep anything between
visits, which is the first thing a phone app needs.

This change adds **`Storage`**: a persistent string-to-string store, read and written synchronously.

## 2. The decision

```
storage.get(key: String, default: String) -> String
storage.set(key: String, value: String)   -> Bool      // true when the value is persisted
```

* **Builtin calls, not a `foreign` block.** `platform-model.md` section 5.1 sketched adapters as `foreign`
  functions from `"browser:storage"`. The `foreign` interface is designed and unimplemented
  ([`language-design.md`](language-design.md)); building it for one adapter would be the larger change and would
  teach nothing. `storage.get` and `storage.set` are builtins exactly as `clock.now` and `net.search` are, and
  move to `foreign` when it exists. The capability story does not change: the call needs `requires Storage;`.
* **The exact-row rule applies unchanged.** A call without `requires Storage;` is `capability-missing`; a
  `requires Storage;` with no reachable call is `capability-unused`. Both come from the existing check
  ([`security-model.md`](security-model.md) section 2.2); nothing was added for them. The semantic diff
  reports `Storage` as an added or removed capability for free (tested).
* **Two arguments, always.** A call with another count is `builtin-shape-mismatch`: a missing argument would
  reach the browser as `undefined` and print as the word "undefined".
* **`get` takes the default** rather than returning an `Option`: the language has `Result` and `List` but no
  way to branch on an optional string in a view, and a default is what a view wants.
* **Strings only.** A program that wants a number or a list encodes it (the language has no JSON).
* **Synchronous.** `localStorage` is. An asynchronous store (IndexedDB) would be a different adapter with
  `await`, and a different design.

## 3. What the web backend does

The adapter is emitted only for a program that has a reachable `Storage` use, about 1 KB:

* **Keys are prefixed `cui:`** *(and `cui:ns:` for a component that declares `Storage("ns")`, section 13)*. `storage.get("note", ...)` reads `localStorage["cui:note"]`. The program can
  neither read nor overwrite a key that something else on the origin wrote, and nothing it writes collides with
  one. This is the first form of `Storage("namespace")` narrowing of `security-model.md` section 2.3, fixed
  rather than chosen by the program; the declared narrowing (`requires Storage("user-prefs")`) is still Phase 2.
* **It never throws.** A private window, blocked site data or a full quota make `localStorage` throw. The
  adapter then keeps the value in memory for the life of the page, `get` finds it there, and `set` answers
  `false`, so a program can tell the user "not saved". After a reload the value is gone, which is the honest
  answer. *(Corrected, #66, section 20: when the store was merely full and already held an older value under the key, `get`
  returned the older value, not the one the page had just written; measured in Chromium. `get` now reads the page's memory
  first.)*
* **Reads are live.** A view that calls `storage.get` shows what is stored now. The web backend rebuilds the
  whole view after every handler, so a `set` in a handler is on screen at once. Nothing re-renders when
  *another tab* writes: no `storage` event is wired.

## 4. Replay (`cancho-ui test`)

Replay models the store as a map ([`replay.md`](replay.md)):

* The trace header may seed it: `"storage": {"note": "from last time"}` (an object of strings; anything else
  is `trace-malformed`).
* `storage.set` writes the map, `storage.get` reads it with the same default rule. Both are journalled like
  `clock.now`, so a statement that is re-evaluated after an `await` sees the answer it saw the first time.
* Every step report carries the store after the step (`storage`, left out when it is empty), and a step may
  `expect` it: `"expect": {"storage": {"note": "buy milk"}}` is a subset check, like the other expectations.

Replay does not model `localStorage` failing; that is the browser script's (section 6). *(Corrected, #66: a trace may set
`storage_quota`, section 20, and replay then models a full store.)*

## 5. Found while building it

1. **A string interpolation cannot hold a string literal** (still true; the lexer ends the string at the first inner quote). `"Saved: {storage.get("note", "none")}"` is
   `parse-unexpected-token`: the lexer ends the string at the first inner quote. The workaround is the general
   expression form of `Text`: `Text { storage.get("note", "none") }`. `examples/notes` uses it. A lexer fix is a
   language change and is not made here.
2. **There is no mount event.** A component cannot copy a stored value into state when it starts. Reading in
   the view (live) or on a user event (`on Restore { draft = storage.get(...) }`) are the two shapes; the second
   is what `examples/notes` shows. A `Start`-like lifecycle event is [`semantics.md`](semantics.md)'s open
   lifecycle question.
3. **No builtin's argument count was checked.** `clock.now(1)` and `sleep()` compile. Only the new calls are
   checked here; the others are listed in section 7.

## 6. Checked

In the gate (`cargo test`, `crates/cancho-ui/tests/storage.rs`, 9 tests): the adapter is emitted for a program
that requires `Storage` and for no other; both calls prefix the key and both are inside a `try`; the two
capability refusals; the two arity refusals; both example traces pass; a wrong expectation fails with the store
in the report; a store written before an `await` is there after the request resolves; three malformed stores;
the diff reports the capability.

Outside the gate (`scripts/check-storage.js`, Chromium, needs Node and `playwright`): the default is shown
before any save; after Save the note is shown at once; exactly one key, `cui:note`, is written; the note survives
a reload; a page's own unprefixed `note` is not read; with `localStorage` replaced by an object whose methods
throw, Save still shows the note for the life of the page, a reload forgets it, and there is no script error.

**The checks can fail.** Eight deliberate breaks, each run against the storage tests: the prefix dropped from one
of the two functions, `set` that does not write the replay store, `get` that ignores it, the arity check removed,
the capability mapping removed, the trace seed ignored (all six killed by the gate), `get`'s memory fallback
returning `undefined` (killed by the browser script, 3 of its checks, and by nothing in the gate), and `set` not
journalled (**survives, and cannot be observed**: a re-evaluated `set` writes the same value again). The last is
kept for symmetry with `clock.now` and is stated here rather than hidden.

## 7. Not done

* **`Location`** (`location.current`), the declared narrowings `Storage("ns")` and `Network("host")`, an event for another tab's write, an
  asynchronous store, quotas beyond "it threw", and values that are not strings.
* **Argument counts of the other builtins** (`clock.now`, `sleep`, `net.search`) are not checked statically.
* **iOS Safari** is not tested; Chromium only. Safari's private mode is known to make `localStorage` throw on
  older versions; the fallback exists for it, but this document does not claim it was observed.

## 8. Corrections

*(None yet.)*

---

## 9. `Network`: `net.get(url)`

### 9.1 The decision

```
await(net.get(url: String))  ->  Ok(text: String) | Err(reason: String) | Cancelled
```

It is the second effect `await` accepts that is a request ([`semantics.md`](semantics.md) Q2): starting a call
cancels the component's still-pending `net.get` (stale-result protection), the cancellation group is `net_get`
and is not shared with `net.search`, and a disposed component's pending `net.get` resolves to `Cancelled`. A
`match` on the awaited field needs all three arms, exactly as for `net.search`.

* **Same origin only.** Phase 1 has no way to say which host a component may reach (`Network("host")` is
  Phase 2, `security-model.md` section 2.3), and an unbounded `fetch` of a program-built string is an exfiltration
  channel. So the adapter fetches **only this page's own origin**. A URL on another origin is answered with
  `Err("blocked: only this site's own origin may be fetched")` **before any request is made**. That also fits the
  installed web app of [`mobile-backend.md`](mobile-backend.md): a phone app talks to the server that served it.
  Cross-origin access is the declared `Network("host")` narrowing, not built.
* **GET only**, no headers, no body, `credentials: "same-origin"`.
* **Redirects are refused** (`redirect: "manual"`; the answer is `Err("blocked: redirect")`): a redirect could
  leave the origin after the check.
* **At most 1 MiB of text**, else `Err("too large")`; **10 s** to the reply, else `Err("timeout")`.
* **Every failure is a value.** `Err("HTTP 404")` for a non-2xx status, `Err("network error")` when the request
  fails (offline, refused, reset; including the installed app's service worker having nothing cached for it),
  `Err("blocked: not a URL")` for a string `new URL` rejects. Nothing throws into the program.
* **`net.search` stays** as the simulated search the demos use; a program that only uses `net.get` no longer
  carries its fixed corpus. The capability is the same, `Network`; the row does not say which call.

### 9.2 Replay

`net.get` is an effect like `net.search`: the step report shows `issued #n net.get("data.txt")`, the trace
scripts the answer with `{"resolve": n, "with": {"Ok": "text"}}` or `{"Err": "HTTP 404"}`, and a newer request
drops an older one's answer. A URL that is not a string is `replay-type-error`. Replay does not model the five
guards of 9.1; they are the browser's (9.3).

### 9.3 Checked

In the gate (`crates/cancho-ui/tests/network.rs`, 6 tests): only the adapter a program uses is emitted (a
fetch-only program carries no corpus; the search example carries no `net_get`; the counter carries no `fetch(`);
the four guards are present and there is exactly one `fetch(`; the two refusals (`capability-missing`, and
`builtin-shape-mismatch` for zero and for two arguments); the example trace passes and a wrong answer fails; a
non-string URL is `replay-type-error`; a `net.search` in flight is not superseded by a `net.get`.

Outside the gate (`scripts/check-network.js`, Chromium, a real HTTP server for the page and a second server for
"another origin"): a same-origin file is fetched and shown; a 404 is `Err("HTTP 404")` and the old body is
cleared; **another origin is blocked and that server receives 0 requests**; `http://` is blocked as not a URL; a
redirect is refused; a 2 MiB body is `too large`; a reply that never comes is a `timeout` after 10.07 s; with the
server stopped the answer is `network error`; no script errors.

**The checks can fail.** Five deliberate breaks of the adapter, each run against the browser script: the origin
check removed (killed by 2 checks, including the request count), redirects followed, the size bound removed, the
status not checked (each killed by 1). `credentials: "include"` in place of `"same-origin"` **survives, and
cannot be observed**: with the origin check in front, every request is same-origin, so the two are equal.

### 9.4 Found while building it

1. **A string literal with braces was interpolated only inside `Text`.** `status = match r { Err(why) => {
   "error: {why}" } ... }` compiled, and the emitted JavaScript returned the text `error: {why}`, braces
   included; replay showed it (it was replay that caught it). **Fixed 2026-10-20**, in the change that follows `net.get`: `{expr}`
   interpolates in every string expression, and a `{` in static text (a label, an accessibility label, a
   `value:` string, an instance property) is `parse-unexpected-token` instead of a silent brace. The same bug
   was in `Button { label: "Item {n}" }`, found while fixing this one. Decision 12 of
   [`language-design.md`](language-design.md); tests in `crates/cancho-ui/tests/interpolation.rs`.
2. **An awaited `net.get` that returns `Ok(text)` binds a `String`, not a `List[String]`.** The checker does not
   type the payload of `Result[..]` (`state r: Result[String, String]` is accepted for `net.search` too);
   nothing here changes that.

### 9.5 Not done

*(Aborting a superseded or disposed request: built 2026-10-20, section 16.)*

`Network("host")` narrowing and cross-origin fetches (and with them CORS handling and any credentials), methods
other than GET (a POST exists since section 17, with its own capability), request bodies and headers for a read, streaming, a retry policy, and the service worker caching API replies (section 19: not done on purpose). Chromium
only; iOS Safari is not tested.

---

## 10. Narrowing: `requires Network("host")`

> **Status: designed, then built (written before the code; measured in 10.3 to 10.6).** Answers
> [`security-model.md`](security-model.md) section 2.3, for `Network` only.

### 10.1 The decision

```
requires Network;                          // this page's own origin (section 9)
requires Network("api.example.com");       // and that host
requires Network("a.example", "b.example:8443");
```

* **A bound adds hosts; it never replaces the page's origin.** `security-model.md` section 2.3 says a narrowed
  capability *restricts* ("`Network("api.example.com")`: restrict outbound requests to one host"). In this
  backend the bare `Network` is already the narrowest useful thing (the page's own origin), so a bound cannot
  restrict further without making a component unable to reach the server that served it, which no deployment
  wants. A bound is therefore the **allow-list of other hosts** a component may fetch. The honest cost is that
  the bound is a widening of the bare form, not a narrowing of it, and the semantic diff reports it as one (10.5).
* **A host is `name` or `name:port`**: letters, digits, `-` and `.`; compared lower-cased and exactly (no
  wildcards, no subdomains, no paths, no scheme). Both `http` and `https` are accepted for an allowed host (the
  browser refuses an insecure request from a secure page on its own, and `localhost` development needs `http`).
* **Only `Network` took a bound here** *(corrected: `Location` and `Storage` take one now, sections 11 and 13)*. `Clock("x")` and an unknown capability with a bound
  are `capability-narrowing-illegal`, with the reason (`Storage("ns")` was designed in section 2.3 of the security
  model and is built in section 13). So is a host that is not `name[:port]`.
* **Two tiers.** Static: a `net.get` whose URL is a *string literal* naming a host that is neither allowed nor
  the page's own is `capability-bound-violated` at compile time (a relative literal is always fine); the
  remediation names the `requires` line to add. Runtime: the adapter checks the host of the resolved URL for
  every call, literal or not, and answers `Err("blocked: ...")` before any request is made. A computed URL is
  therefore never unchecked, only checked later.
* **Cross-origin means CORS.** The server has to allow the page's origin. A refused CORS preflight or response
  is `Err("network error")`, which the page cannot tell from being offline: the browser withholds the reason
  from scripts. Credentials are never sent to another origin (`credentials: "same-origin"`).

### 10.2 What it touches

Parser (`requires Name("..", ..)`), AST and IR (`bounds` on a capability reference, left out of the IR JSON when
empty so existing IR is byte-identical), the two refusals, the web adapter (`__cui_net_get(url, hosts)`, with the
component's `hosts` as a constant of its instance), the semantic diff and `inspect` (the bound is part of the
declared authority), replay (the bound is not modelled: the trace scripts the answer; the static check still
runs on the source).

### 10.3 What was built

`requires Network("a.example", "b.example:8443");` parses (a bound must be a string literal: anything else is
`parse-unexpected-token`, "a capability bound is written in the source, not computed"). The bounds are on
`IrCapabilityRef` and in the IR JSON only when present. `capability-narrowing-illegal` and
`capability-bound-violated` are in `spec/diagnostics/README.md`. The adapter is `__cui_net_get(url, hosts)`; each
component instance declares `const __cui_net_hosts = [...]` (lower-cased) and a program without a bound gets `[]`.
The adapter also refuses a URL that is not `http:` or `https:` (a `blob:` URL of this origin, for one, is not a
fetch the program should make). The semantic diff lists a declaration with its bounds, so adding a host shows
as `Network` removed and `Network("a.example")` added: a widening, visibly.

### 10.4 Checked

In the gate (`crates/cancho-ui/tests/narrowing.rs`, 6 tests): the accepted literals (relative, an allowed host,
an uppercase host, a default port, a port, userinfo before an allowed host); seven literals that are
`capability-bound-violated` (no bound, another host, protocol-relative, a different port, a suffix of the allowed
host, the allowed host as userinfo of another, an empty host) and that the remediation names the line to add;
ten bounds that are `capability-narrowing-illegal` (on `Clock` and `Storage`, a URL, a path, a wildcard, empty, a
space, a bare colon, a bad port, a leading dot); a computed bound is a parse error; the emitted hosts; the IR has
no `bounds` key for a bare capability; the diff.

In the browser (`scripts/check-network.js`, section B): the same program built with the second server's host in
its bound fetches that server (which sees exactly one request); a third server is blocked and sees none; the
scheme and host compare case-insensitively; this site still works.

**The checks can fail.** Nine deliberate breaks: eight of the compiler (the literal check inverted, host legality
always true, default ports not stripped, userinfo not stripped, a bound allowed on any capability,
protocol-relative URLs not read, bounds not lower-cased, the diff ignoring a bound) all killed by the gate, and the
runtime host check removed, killed by 4 browser checks. The userinfo break survived the first version of the
tests, because the case I had written is a violation either way; the accepted case `https://user@api.example.com/`
was added and kills it.

### 10.5 Not done, and the limits

* *(`Storage("ns")` and `Location` bounds: built, sections 11 and 13.)* Wildcards and subdomains, a
  bound per path, and CORS diagnostics (the browser withholds the reason: a refused request is `network error`).
* A **computed** URL on a disallowed host is caught at run time only (`Err("blocked: ...")`), never at compile
  time; a literal is caught at both.
* The bound widens, it does not narrow (10.1). Anyone reading `cancho-ui diff` for authority has the new
  `Network("host")` line in front of them; there is no "this program can reach nothing" form below the bare
  `Network`, because the bare `Network` already cannot leave the origin.


---

## 11. `Location`: `location.current()`

> **Status: designed here before the code; built and measured in 11.4 to 11.6.** (Section 12 is the status of all three adapters.)

### 11.1 The decision

```
await(location.current())  ->  Ok(position: String) | Err(reason: String) | Cancelled
```

* **A position is the text `"lat,lon"`**, decimal degrees, so it can be shown, or sent to this site with
  `net.get("/near?pos=" + pos)`. The language has no records and no way to index a list or split a string, so a
  structured position would be a value nothing could read; a string is what a program can use today. (A typed
  position is a language change, not made here.)
* **Precision is the bound, and the default is the coarse one.** Bare `requires Location;` rounds to **2
  decimals** (about 1 km). `requires Location("fine");` gives **5 decimals** (about 1 m). `Location("coarse")` is
  the bare form spelled out. This is `security-model.md` section 2.3 ("`Location` is narrowed by precision"), with
  the same direction as `Network("host")`: the bare capability is the least, a bound asks for more, and the
  semantic diff shows `Location("fine")` as a change of declared authority. Any other bound is
  `capability-narrowing-illegal`. The rounding is done in the adapter, so a coarse component never holds a fine
  position.
* **Failures are values**, never a throw: `Err("permission denied")`, `Err("position unavailable")`,
  `Err("timeout")` (10 s), `Err("unavailable")` (no Geolocation API, or an insecure context).
* **The browser asks the user.** The page does not decide whether a position is shared; the first call shows the
  browser's prompt, and a refusal is `Err("permission denied")`. `maximumAge` is 60 s: a position up to a minute
  old is accepted so a repeated call does not wake the radio.
* Its own cancellation group (`location_current`); a newer call supersedes an older one; `Cancelled` on
  disposal. Zero arguments, else `builtin-shape-mismatch`.

### 11.2 Replay

`location.current` is an effect like the others: `issued #n location.current()`, scripted with
`{"resolve": n, "with": {"Ok": "40.42,-3.70"}}`. Replay does not round: the trace says what the adapter would
have delivered.

### 11.3 Not done

Watching a position (`watchPosition`), altitude, accuracy, heading and speed, a typed position, asking for
permission ahead of the call, and iOS Safari.

### 11.4 What was built

`location.current()` is a builtin of the `Location` capability; the adapter (`__cui_location_current(digits)`) is
emitted only for a program that uses it, and each component instance declares `const __cui_location_digits`
(2, or 5 when any of its `requires Location(..)` says `"fine"`). `capability-narrowing-illegal` covers a
`Location` bound that is not exactly one of `"coarse"` and `"fine"` (matched exactly: `"Fine"` is refused).
Replay: `Effect::LocationCurrent`, scriptable, its own cancellation group.

### 11.5 Checked

In the gate (`crates/cancho-ui/tests/location.rs`, 5 tests): the adapter appears only where used and has one
guard and one call of `navigator.geolocation`; the default is 2 digits, `"coarse"` is 2, `"fine"` is 5, and two
components in one file keep their own; four illegal bounds; missing declaration and an argument; the example
trace passes and `location.current` is not superseded by a `net.get`.

In Chromium (`scripts/check-location.js`, the position and the permission emulated by Playwright): a bare
`requires Location` shows `40.42,-3.70` for a position of 40.416775, -3.70379; `Location("fine")` shows
`40.41678,-3.70379`; the signs of a southern, eastern position are kept (`-33.87,151.21`); without the user's
permission the answer is `permission denied` and the position is not shown; with no Geolocation API
`unavailable`; an error code 2 `position unavailable`; code 3 `timeout`; an adapter-side throw `unavailable`
and no script error.

**The checks can fail.** Eight deliberate breaks: the default digits swapped, `"fine"` never detected, any bound
legal, the arity check removed, the capability unmapped (all killed by the gate), the rounding removed (killed by 3
browser checks), a refusal mapped to the wrong reason (1), and the adapter's `catch` removed (the script ends in
a timeout of the page, not in a failed check, so the break is counted by the script's exit status). One
**harness mistake of mine** is worth stating: the first run of the browser script said `"fine"` gave 2 decimals;
the cause was the script replacing the first `requires Location;` of the example, which is inside its header
comment, so the "fine" build was not fine. The replacement now targets the declaration line.

### 11.6 Found while building it

The language has no way to read a position's parts, so `"lat,lon"` is only displayable or forwardable (11.1). A
real consumer (distance, a map) needs a typed record or a list index; neither exists, and that is the next
language question the adapters raise, not an adapter one. *(Corrected 2026-10-24, `maps.md` section 2: the parse is possible today with `take`, `skip`, `join` and `num`; what is missing is a square root and trigonometry, so a distance in km, not a position.)*

---

## 13. Narrowing: `requires Storage("ns")`

### 13.1 The decision

`Storage` had one fixed prefix, `cui:`, so every component on an origin could read every other component's keys,
and a component that only needs its own preferences held the whole store. `requires Storage("notes")` narrows it:

* the component's keys live under `cui:notes:` (`storage.get("note", ..)` reads `localStorage["cui:notes:note"]`),
  and it cannot spell a key outside that prefix, because the adapter appends the key to a fixed prefix;
* a bare `requires Storage;` is unchanged, holds the whole `cui:` store and is therefore the wider grant (it can
  read `notes:note` by asking for the key `notes:note`). That is the narrowing rule working as everywhere: a bound
  makes the declaration smaller, and the semantic diff shows `Storage` becoming `Storage(notes)`;
* a namespace is exactly one string: 1 to 32 lower-case letters, digits and `-`, starting with a letter or a digit.
  No `:` (so one namespace cannot spell another's prefix), no upper case (so two spellings are never two
  namespaces on a case-insensitive reading), nothing that needs escaping. Anything else, an empty string or a
  second bound is `capability-narrowing-illegal` with the reason.

### 13.2 What it touches

* **Emit:** each component carries `const __cui_storage_ns = "notes";` (or `""`) beside its other per-instance
  constants, and the two adapter functions take it as a last argument, so two components in one program keep their
  own namespaces.
* **Replay:** the trace's `storage` keys are the full `ns:key` for a namespaced component (the adapter's `cui:` is
  still left off). A trace that seeds the bare `note` for a component in namespace `notes` finds nothing.

### 13.3 Checked

* `cargo test`: the namespace and the two-component case in the emitted JavaScript; seven illegal bounds
  (empty, `a:b`, upper case, leading `-`, a space, two bounds, 33 characters); a namespaced trace that writes
  `notes:note` and one seeded under the bare key; the existing narrowing test now uses `a:b` as its illegal
  `Storage` bound (it used `prefs`, which is legal now).
* `scripts/check-storage.js` in Chromium (outside the gate): with a bare program's `cui:note` already in
  `localStorage`, the namespaced program does not see it, Save writes `cui:notes:note` and leaves `cui:note` alone,
  and a reload shows the note.

### 13.4 Not done

Nothing stops a bare `requires Storage;` component from reading a namespaced one's keys: the bare form is the whole
store by definition. A program that wants isolation gives every component a namespace. Keys are not encrypted, and
another script on the same origin can read all of them; the namespace is a boundary between *components of this
program*, not between this program and the page.

---

## 14. `json.get` and `json.items`, and an operator console for the plant-floor stack

Written 2026-10-20, after `alpibrusl/cancho-industrial` made the question concrete: its gateway answers `GET /metrics`
as JSON, and a page that can fetch (section 9) could only show the document whole.

### 14.1 The decision

Two pure builtins (`docs/language-design.md` decision 13). They need no capability: the program already holds the text.

* `json.get(text, path, default)`: the String at `path` of the JSON `text`, or `default`. A path is dot-separated: an
  object key, or, in an array, a decimal index (`0`, `12`; no sign, no leading zero, at most 9 digits); the empty path is
  the root. A String is returned as it is, a Boolean as `true` or `false`, a Number when it is an integer of
  magnitude 2^53 or less, as decimal digits (`1.0` and `1e3` are `1` and `1000`), or a decimal with 1e-6 <= |x| < 1e15,
  as the shortest digits that read back as the same number (`1.5`, `2.50` is `2.5`, `2.5e-3` is `0.0025`); any other
  number is the default. `null`, an object,
  an array, a missing member and a path through a scalar are the default.
* `json.items(text, path)`: a `List[String]` of the elements of the array at `path`, at most 1000, each the compact JSON text
  of the element. They are **opaque**: for `json.get` and `json.items` only (the key order of an element is the runtime's:
  insertion order in the browser, sorted in replay).

Both are total. A text that is not JSON, that is nested past 127 levels, that has a number out of range or a lone
surrogate escape (the whole document is refused, as `serde_json` refuses it) is the default or the empty list.

What they do not do, on purpose: a key that holds a dot, wildcards, filters, building JSON, a number outside those ranges
(an exponent form would print differently in Rust and JavaScript, so it is the default rather than a disagreement).

Decimals were added the same day (decision 13). Rust's `{}` and JavaScript's `String` print the same shortest round-trip digits
exactly while neither uses an exponent, which is 1e-6 <= |x| < 1e15; that range is the contract. *(Found by the corpus:
`serde_json` parses a float with a fast path that is not always correctly rounded, so `999999999999999.5` read as
`...9.6`; the workspace now enables its `float_roundtrip` feature, which JavaScript's `JSON.parse` already matches.)*

### 14.2 Checked on both runtimes

`tests/json_get_corpus.json` has 87 cases (every shape above, duplicate keys, `__proto__`, escapes, surrogates, the depth
limit, lists of 3, 1000 and 1500). `cargo test` runs it through the replay model (`cancho_ui_replay::json_get` and
`json_items`, over `serde_json`); `scripts/check-json-get.js` runs the same file through the emitted JavaScript, plus five
hostile texts that must not throw. Both agree on all 87. *(Found by the corpus: the depth limit is 127, not the 128 the
first version of the JavaScript used; `serde_json` fails the 128th level.)*

### 14.3 The console

`examples/plant/plant.cui` reads `admin/metrics` and `admin/readyz` with `net.get` and shows readiness, uptime, sessions,
each route's 2xx and 5xx counts and each upstream's circuit. Its trace (`plant.trace.json`, 9 steps, 17 checks) replays the
gateway's `GET /metrics` shape, a gateway that answers `503`, and a transport error. `scripts/check-plant.js` runs the built
page in Chromium against a server that plays the gateway: the values appear, a `503` shows `ready: false` with its
reasons (section 15), a dropped connection shows `unknown (network error)`, and a document that is not JSON gives the
defaults. *(Corrected the same day: the first version showed `not ready (HTTP 503)` and no reasons, which is what
`net.get` allows; section 15 is the change.)*

### 14.4 What this showed about the stack (for `alpibrusl/cancho-industrial`)

* **The admin listener is another origin.** The gateway's admin port is a different port, so the browser will not let a
  page served from the proxy's port read it, and it sends no CORS headers. The console works from a path of its own origin:
  the deployment forwards `admin/…` to the admin listener (a route whose upstream is the gateway's own admin port), or serves
  the page from the admin origin. Neither was run against the real gateway here; the check uses a stand-in.
* **A non-2xx response comes back with no body** from `net.get` (section 9: `Err("HTTP 503")`). `GET /readyz` explains
  *why* it is not ready only in the body of that `503`. *(Corrected the same day: `net.fetch`, section 15, keeps the
  body, and the console reads the reasons.)*
* **The page asks for `Network` only.** Nothing else is declared, so the console's authority report is the one line
  `Network` (`cancho-ui inspect effects examples/plant/plant.cui Plant`: one reachable effect, `net.get`).

### 14.5 Not done

The real gateway in the loop (`alpibrusl/cancho-industrial`),
a `Text` that mixes a literal and a call with a string argument (`"{json.get(t, "a", "-")}"` still ends the string at the
inner quote; the concatenation form is the way round it), sorting or filtering the routes.

---

## 15. `net.fetch(url)`: the answer of an HTTP error status

### 15.1 The decision

`net.get` answers `Err("HTTP 503")` for a status outside 200 to 299 and drops the body (section 9). That is right for
a page that wants a document, and wrong for one that wants to know *why* a service refused: the gateway's `/readyz`
puts its reasons in the body of the `503`. `net.fetch(url)` is the same request (same origin, same `Network("host")`
allow-list, same size and time bounds, same refusals, same capability) with one difference:

* it resolves to `Ok(text)` for **any** HTTP status, where `text` is the JSON text `{"status":503,"body":"..."}` (the
  body as a JSON string, escaped by `JSON.stringify`). `Err` is left for what produced no HTTP answer: `blocked: ...`,
  `network error`, `timeout`, `too large`.

The envelope is a String so that nothing new is needed in the language: `json.get(reply, "status", "0")` and
`json.get(reply, "body", "")` take it apart, and the body is itself a JSON text for `json.get` and `json.items`.
Nothing is decided for the program: a `200` and a `404` are both `Ok`, and the program looks at the status. That is
the cost of the shape, and why `net.get` stays: most pages should not have to check.

### 15.2 What it touches

* **Capability and bounds:** exactly `net.get`'s. A literal URL on another host is `capability-bound-violated` without
  `Network("host")`; a missing declaration is `capability-missing`. One argument, or `builtin-shape-mismatch`.
* **Emit:** `__cui_net_get` and `__cui_net_fetch` are two entry points of one `__cui_net_request(url, hosts, envelope)`, so
  the guards are one piece of code. The redirect refusal comes before the status is looked at.
* **Replay:** an effect `net.fetch` with its own cancellation group (`net_fetch`), scripted by a trace like `net.get`; the
  trace writes the envelope as the `Ok` text. Replay does not make the envelope: it plays what the trace says the
  adapter answered.

### 15.3 Checked

* `cargo test`: the emitted adapter, the arity and capability refusals, the bound on a literal URL, and a trace in which
  `net.get` and `net.fetch` are two pending requests that resolve independently.
* `scripts/check-plant.js` (Chromium, outside the gate): the stand-in gateway answers `503` with
  `{"ready":false,"why":[...]}` and the page shows both reasons; with the connection dropped it shows
  `ready: unknown (network error)`.

### 15.4 Not done

The response headers, a method other than GET (`net.post`: section 17), headers and credentials (section 18, a decision is requested), a body for a read, and a status that is a redirect (still `blocked: redirect`). The
envelope is built in the browser; replay's trace has to write it by hand, and a trace that writes it wrongly tests
nothing about the adapter (the Chromium check does).

---

## 16. A request that is superseded or disposed is aborted

### 16.1 The decision

Until now a superseded request (`semantics.md` Q4) or one whose component was disposed (Q5) had its **answer** dropped
and its **request** left to finish: the browser kept the connection, the server did the work, and a page that
superseded a slow request every tick (the plant console, a search box) piled them up. The adapter now stops it. The
semantics do not change: the answer is dropped or `Cancelled` exactly as before; what changes is that the browser is
told to abandon the fetch (`AbortController.abort()`), so the connection closes and the server sees it.

### 16.2 How

`__cui_call` makes a token for the request, publishes it in `__cui_owner` for the duration of the synchronous `start()`
call, and `__cui_net_request` (the one function behind `net.get` and `net.fetch`) reads it as it starts and sets
`token.abort`. Superseding a group aborts the previous token; a component's `__cui_dispose` aborts the tokens it
disposes. A call that is not a network request (`sleep`, `location.current`) sets no `abort` and is as before. The
adapter answers the aborted fetch `Err("aborted")`, which nobody reads (the token is `cancelled` or `disposed`).

### 16.3 Checked

* `scripts/check-network.js` (Chromium): a slow request that never answers, then a newer one: the newer is shown and the
  server sees the slow request's connection close at once, not after the 10 s timeout.
* `scripts/check-schedules.js`: a request held by the server, the component removed: the server sees it close.
* `cargo test`: the emitted runtime carries the wiring.

### 16.4 Not done

An abort the server may not notice (a proxy that keeps the upstream). A write (`net.post`, section 17) is deliberately not
aborted: a write cut off half way may or may not have been applied.

---

## 17. `net.post(url, body)`: a write, with its own capability

### 17.1 The decision

```
requires NetworkWrite;
await(net.post(url: String, body: String))  ->  Ok(envelope: String) | Err(reason: String) | Cancelled
```

* **Its own capability.** A write is a stronger authority than a read: a component that may read the server's
  state must not, by that, be able to change it. `Network` stays read-only (`net.get`, `net.fetch`, `net.search`);
  `net.post` needs `NetworkWrite`, and a program that declares only `Network` is `capability-missing`, one that
  declares `NetworkWrite` and never writes is `capability-unused`. `cancho-ui diff` shows the capability added, so
  the review of "this release can now change things" is a line, not an inference.
* **This page's origin only, with no bound.** `NetworkWrite("host")` is `capability-narrowing-illegal`, and a
  literal URL naming a host is `capability-bound-violated`; at run time any other origin is
  `Err("blocked: a write reaches only this site's own origin")` before a request is made. A cross-origin write is an
  exfiltration channel and a CSRF target; if a deployment needs one, it is a design of its own, not a widening of
  this call.
* **The answer is `net.fetch`'s envelope**: `Ok` of the JSON text `{"status":N,"body":"..."}` for any HTTP status,
  because for a write the status is the answer (`201` made it, `409` already exists, `422` refused). `Err` is left
  for what produced no HTTP answer: `blocked: ...`, `network error`, `timeout`, `too large`. `json.get(reply,
  "status", "0")` takes it apart.
* **The body is a String sent as `application/json`** with `credentials: "same-origin"` (this origin's cookies);
  no other header, no method but POST *(corrected: `net.put` and `net.delete` add PUT and DELETE in section 21)*. The program builds the JSON text; a body that is not JSON is the server's
  to refuse (a `4xx` is an answer). Redirects are refused (`blocked: redirect`), a reply over 1 MiB is `too large`,
  and a reply later than 10 s is `timeout`.
* **A write is not aborted when it is superseded or disposed.** Section 16 aborts a read that nobody will look at.
  For a write the same abort would leave the server in a state the page cannot know (applied, or not); letting it
  finish leaves it known and the answer is still dropped (superseded) or `Cancelled` (disposed), exactly as for a
  read. The cost is that a superseded write still happens; the program that must not issue two should not offer the
  button twice (a state flag), and the server should make the write idempotent.
* **Never retried.** There is no retry policy for any request, and for a write there never will be one in the adapter:
  a retry of a write that may have applied is the program's decision, made with the server's help.
* The cancellation group is `net_post`, shared with no other call: a newer `net.post` supersedes the answer of the
  previous one only.

### 17.2 What it touches

* **Lowering:** `net.post` has exactly two arguments (`builtin-shape-mismatch` otherwise) and performs `NetworkWrite`.
  The literal-URL host check applies with an empty allow-list.
* **Emit:** `__cui_net_post(url, body)` is emitted only into a program that uses it (a read-only program carries no
  `method: "POST"`), and is a separate function from `__cui_net_request` on purpose: the guards of a read and of a write
  are not one piece of code that a change to one can loosen in the other.
* **Replay:** an effect `net.post` with its own group, scripted by a trace like `net.fetch`; the step report shows
  `issued #n net.post("/api/notes", "hello")`. A URL or a body that is not a String is `replay-type-error`.

### 17.3 Checked

* `cargo test` (`crates/cancho-ui/tests/net_post.rs`, 6 tests): the adapter is emitted only for a program that writes and
  carries no abort wiring; `capability-missing` for none and for `Network`, `capability-unused`; arity 0, 1 and 3;
  a bound, and three literals on other hosts; a trace with a `201` and a `409` where the newer write's answer wins;
  a non-String body.
* `scripts/check-post.js` (Chromium, real servers, outside the gate): the POST arrives as a POST with the body, the JSON
  content type and this origin's cookie; `201`, `409` and `503` are answers; another origin is blocked **and that server
  receives 0 requests**; a redirect is refused and not followed; a 2 MiB reply is `too large`; two writes in a row both
  reach the server (neither is cut off) and the newer answer is shown; with the server stopped the answer is
  `network error`.

**The checks can fail.** Five deliberate breaks of the adapter, each run against the browser script: the origin check
removed (killed by 2 checks, including the request count of the other server), redirects followed (2), the size bound
removed (1), `credentials: "omit"` (1, the cookie) and the method changed (1). Unlike `net.get` (9.3) the credentials
setting is observable here, because the check looks at the cookie the server received.

### 17.4 Not done

Cross-origin writes, other methods (`PUT` and `DELETE` are built since, section 21; `PATCH` is not), headers (a CSRF token, an `Authorization`), a body that
is not text, a timeout the program chooses, an idempotency key, and a way to say "this write must not be superseded"
(today: a state flag). A server that relies on cookies alone for authority is open to a cross-site form post whatever
this adapter does; the `Content-Type: application/json` is not a defence, only a convention a server can check. iOS
Safari is not tested.

---

## 18. Credentials: how a component sends one and still declares it

> **Status: BUILT (2026-10-21), option A, with the four decisions below.** Asked for by `alpibrusl/cancho-ui#17`, found by the
> plant-floor spike in `alpibrusl/cancho-industrial#9` (`docs/ui-spike.md` there). 18.1 to 18.3 are kept as written when it was
> design only; where the build differs, 18.4 and 18.5 say so.

### 18.1 What exists today (read from the code, and measured where marked)

* `net.get`, `net.fetch` and `net.post` send **no header** the program chooses; `net.post` adds only `Content-Type:
  application/json` (section 17). Requests use `credentials: "same-origin"`, so this origin's **cookies** go with them (measured
  for `net.post`: `scripts/check-post.js` asserts the server received the cookie; `net.get` uses the same setting in the
  adapter and is not separately measured).
* So the cookie pattern the spike used works today: the page's script sets a cookie from the URL fragment and the server accepts
  it on `GET`. Its limits, stated plainly: the cookie is not `HttpOnly` (script set it, so any script of the origin can read it),
  and the credential **never appears in the component's `requires`**, so `cancho-ui diff` and `report` cannot show that the page
  now carries an admin credential. The capability story stops at the glue.

### 18.2 Options

| | Option | The credential is declared | The value lives in | Replay can assert | Cost |
|---|---|---|---|---|---|
| A | **Named credential** `requires Credential("api");` | yes, in `requires`, so a diff shows it | the page's glue, handed to the module at mount | which credential a request carried (never the value) | a new capability and a `mount_X(root, options)` argument |
| B | **Header allow-list on `Network`**, e.g. `requires Network("host", header "Authorization")` | yes | `Storage`, read by the component | the header and its value | the secret is program state, so it is in traces, reports and the diff of an initial value; a header allow-list is a new bound shape |
| C | **Say plainly it is the page's business**, keep the cookie pattern | no | a cookie | no | nothing built; the gap stays open and is documented |

### 18.3 Recommendation: A

* **Declaration.** `requires Credential("api");` is a capability with a name as its bound, like `Storage("ns")`. A name is 1 to 32
  lower-case letters, digits and `-`. The diff shows `Credential("api")` added, so "this release now sends a credential" is one line.
* **Use.** A component with exactly one declared `Credential` sends it as `Authorization: Bearer <value>` on its `net.get`,
  `net.fetch` and `net.post`, **only to hosts those calls may reach** (this origin, and the hosts of `Network("host")` or, for a
  write, this origin): the credential is never attached to a request the allow-list would have blocked, so it cannot be sent to a
  host the component did not declare. Two declared credentials are `credential-ambiguous` (a call cannot choose between them without a
  language change this section does not propose). A `Credential` that no call can use is `capability-unused`, as for any capability.
* **Supply.** The value is given by the page when it mounts: `mount_Readings(root, { credentials: { api: token } })` (the
  new argument is optional; `--no-auto-mount`, section 9 of `mobile-backend.md`, is what makes this possible, because the auto-mounted
  default has no place to pass it). A component that declares a credential and is mounted without it gets `Err("blocked: no credential
  api")` from every request, before the request is made, never a request without it.
* **Replay and reports.** A trace may list `"credentials": ["api"]`; the step report shows `issued #1 net.get("/x") [credential api]`;
  a request issued without the credential available is the `Err` above. The value is never in a trace, a report or the IR.
* **What it does not fix, said first.** The token is readable by any script of the page's origin (an XSS reads it), exactly as in
  the cookie pattern; the custody of the value is the page's. The gain is that the **use** of a credential becomes declared,
  reviewable authority, and that it cannot be sent to an undeclared host.

### 18.4 Decisions (taken by the maintainer, 2026-10-21) and what was built

1. **A.** `requires Credential("name", "host", ..)`.
2. **`Authorization: Bearer` only.** A declarable scheme can be added later without changing any program that exists (a second
   bound would be additive).
3. **Host-scoped, stricter than 18.3 proposed.** The credential goes to this origin (GET and POST) and **only** to the hosts the
   declaration names after the name; every named host must also be in the component's `Network` (`credential-host-not-allowed`
   otherwise). A `Network` host the credential does not name never receives it. 18.3's "every host the call may reach" is
   superseded.
4. **Both.** A module built with auto-mount cannot supply a token, so it is refused at build time
   (`credential-needs-mount-options`, naming `--no-auto-mount`; `--pwa` cannot carry one either). A module mounted without the
   token (or with a name it does not declare, or only an inherited property) answers `Err("blocked: no credential <name>")` before
   any request. A token that is not 1 to 4096 printable ASCII characters without a space is
   `Err("blocked: credential <name> is not a plain token")`.

How it works: `mount_<Last>(root, { credentials: { "api": token } })` copies the own string-valued entries into a private record
for the duration of the mount; each component captures it (children inherit it through the instance mount) and builds its
`{ name, hosts, token }`; the net adapters check it first and attach the header only when the target is this origin or in `hosts`.
More than one `Credential` in a component is `credential-ambiguous`. A `Credential` with no reachable `net.get`, `net.fetch` or
`net.post` is `capability-unused`. Replay: a trace may list `"credentials": ["api"]` (an unknown name is
`trace-credential-unknown`); the report shows `[credential api]` or `[credential api (not supplied)]` on an issued request, and the
value is in no trace, report or IR.

**Checked.** `crates/cancho-ui/tests/credentials.rs` (acceptance, each refusal with its tag, the auto-mount refusal, emission,
replay, diff). `scripts/check-credential.js` runs Chromium against three real servers (this origin, a named host, a `Network`
host the credential does not name): the token arrives at this origin (GET and POST) and at the named host, and is **not** sent to
the third; unsupplied, wrong-name, inherited-only and badly shaped tokens are refused before the server sees a request. Deliberate
breaks, each caught: attach to every host, no host test on GET, a missing token not refused, a write that drops the token, no
token-shape check, the wrapper copying inherited properties. One guard is not observable by design: the component-side own-property
test sits behind the wrapper's, so removing only it changes nothing a page can see.

**What it does not fix.** The token is readable by any script of the page's origin (an XSS reads it); custody of the value is the
page's. The gain is that the **use** of a credential is declared, reviewable authority (`cancho-ui diff` shows `Credential("api")`
added) and cannot reach an undeclared host.

### 18.5 Not done

A scheme other than Bearer; a credential for `net.search`; a credential attached to a path rather than a host; refreshing or
expiring a token (the page passes a new one by remounting).

### 18.6 A token the program receives (#83)

A program can also take the token from a login response: `credential.set("api", token)` and `credential.clear("api")` (`docs/bearer-token.md`). Such a component is anonymous until it holds a token
(the "blocked until supplied" rule of 18.4 applies to a component that cannot set its own), can be auto-mounted and built with `--pwa`/`--page`, and holds the token in memory only.

## 19. The installed page, started with the service down (issue #44)

**Measured** (`scripts/check-offline.js`, Chromium, the server stopped because `setOffline` does not reach a service worker): the
page opens from the service worker's precache and, before the change below, showed `status: error: network error` and `0 of 0
machines`. The worker answers `GET`s from its cache only for the precached files; a data `GET` goes to the network and fails.
Inside one session the list was never lost (an `Err` keeps `rows`); only a **cold start** was empty.

**Decision: do not cache API answers in the service worker; keep the last good answer in the program, with `Storage`.** The
issue proposed stale-while-revalidate in the worker with the staleness reported to the program. Reasons for not doing it:

* The worker sits below the program. A cached reply reaching `net.get` as `Ok(text)` is **indistinguishable from a fresh one**, and
  telling the program needs a new arm (`Stale(text)` or an age) in the `Ok | Err | Cancelled` answer of every `net.*` call: a
  language change that makes every existing `match r` non-exhaustive (`match-non-exhaustive`) until edited. Not proposed here.
* A worker cache keyed by URL has no bound on size or age unless one is designed, and it would cover every same-origin `GET`,
  including ones whose answer must never be shown late.
* The program already has the right tool. `storage.get`/`storage.set` are declared (`Storage("ns")`), seeded and asserted in
  replay, and the program knows what it is showing, so it can say so.

**What the console does** (`examples/ops-console`): a `last: String` state holds the last good answer; `Refresh` starts with
`last = storage.get("machines", last)`, writes `last` back after every read, and builds `rows` from `last` whatever the answer was.
An `Err` keeps `last`, and the status says `error: <why> (showing the last saved list, if any)` *(section 20 makes that text say
how old the list is, or that nothing is saved)*, so an old list is never presented as fresh. No new language feature, no conditional (`docs/conditionals.md` is not needed for it).

**Checked.** `examples/ops-console/ops-console.offline.trace.json` (replay: storage seeded, the first read fails, the saved list is
shown, the next read replaces and saves it); the existing 37-step trace still passes with the one changed status text;
`scripts/check-offline.js` and the former "KNOWN GAP #4" of `scripts/check-ops-console.js`. Against the previous program the new
check fails three of its five checks (`0 of 0 machines`), so it measures the change.

**Not done, and the limits.** *(Corrected, #66, section 20: the first-ever start now says "nothing is saved yet", the saved list
has an age, and a full store is refused visibly.)* The saved list is the whole text of the last answer in `localStorage` (1,697
bytes for 23 machines, measured), which has a per-origin quota of a few MB: a program with a large answer must still choose
what to keep. Other pages need the same few lines; there is
no builtin for it. iOS Safari is not tested.

---

## 20. A page offline for the first time, its saved list's age, and a full store (issue #66)

Three follow-ups to section 19, measured and decided.

**1. The very first start, no network, nothing saved.** Measured before: the status read `error: network error (showing the
last saved list, if any)` and the page showed `0 of 0 machines`: it promised a saved list there was none of. Decision: no
language change; an app says what it knows. With `if` (docs/conditionals.md) the console's status is `error: <why> (nothing is
saved yet: try again when online)` when `last` is empty and `error: <why> (showing the list saved <age>)` otherwise
(`examples/ops-console/ops-console.firststart.trace.json`; `scripts/check-offline.js` starts a fresh profile with the page served
and the machine service down and reads that status).

**2. The age of the saved list.** Decision: store the time with the list, from the `Clock`: the console `requires Clock;`,
writes `machines_at` (milliseconds, as text) next to `machines` after a successful read, reads it back with
`num(storage.get("machines_at", ""), 0)`, and shows `less than a minute ago`, `N min ago` or, when no time was saved (a list
written before this change), `at an unknown time`. This widens the console's authority by `Clock` (the `diff` shows it). Both
traces that depend on the age start the fake clock at a real-looking epoch (`"clock": 1700000000000`), because `0` means "no
time saved". Not done: hours and days (the unit stays minutes), and the age is computed at the read that failed, not live.

**3. A full `localStorage`.** Measured in Chromium (`scripts/check-storage-quota.js`, the store filled until a one-unit write
throws): `storage.set` answered `false` (the adapter's `try`), nothing threw, the older value stayed in the store, and the page
kept working. **One defect found and fixed:** `storage.get` then returned the **older persisted** value, not the one the page had
just written (`got: the older note` after saving a newer one), because the adapter read `localStorage` before its page-lifetime
memory. It now reads the memory first, so the page reads back what it last wrote; after a reload only the persisted value is left
(the honest answer, and what `set`'s `false` told the program). A later `set` that fits persists and clears the memory copy.

*Replay now models it.* A trace header `"storage_quota": N` is the most UTF-16 units the persisted store may hold, counting each key
with the adapter's `cui:` prefix plus its value, over all keys (a write that replaces a key counts the new size, not old plus
new); anything but a non-negative integer is `trace-bad-value`. A write that does not fit answers `false`, keeps the persisted
value, and keeps the new one in the memory that `get` reads first. The step report's `storage` is the persisted store only.
`examples/ops-console/ops-console.quota.trace.json`: the first read succeeds, cannot be saved, the status says
`updated (not saved: this device's storage is full)`, and a later failed read keeps showing the list the page holds.

**Checked.** `tests/storage_quota_corpus.json` (12 cases: exactly at the quota, one over, a refused write readable in the same page,
the older value kept in the store, replacing a key, a later write that fits, other keys counting, an astral character counting two
units, an empty value, a key never written): replayed through the real replay in `cargo test` (`tests/storage.rs`) and run against
the emitted adapter over a fake `localStorage` with the same rule in `scripts/check-storage-quota.js`; the same script fills the
real Chromium store, saves, and reads back, and reloads. The four traces of the console and the browser scripts `check-offline.js`
and `check-ops-console.js`.

**Deliberate breaks, each caught:** replay ignoring the quota, not clearing the memory copy on a write that fits, counting the
replaced key twice, and reading the store before the memory (all in the corpus test); the old read order in the emitted adapter (4
checks of the real-browser script, the failure above); the program without its first-start branch, without its not-saved note, and
with the age in seconds (each fails a console trace).

**Not done.** The real quota of Safari, Firefox or a phone is not measured (Chromium's was filled in a desktop profile); quota
accounting by other engines may differ from "UTF-16 units of key plus value". An event for another tab's write is still not wired.

---

## 21. `net.put(url, body)` and `net.delete(url)`: the other two writes a console needs

> **Status: designed first (this section), then built; 21.4 records what was measured.** Asked for by the plant-floor
> console in `alpibrusl/cancho-industrial` (`docs/console.md`): a REST API sets a quota with `PUT` and revokes a device,
> an agent or a token with `DELETE`, and a page that can only `POST` cannot offer either button.

### 21.1 The decision

```
requires NetworkWrite;
await(net.put(url: String, body: String))  ->  Ok(envelope: String) | Err(reason: String) | Cancelled
await(net.delete(url: String))             ->  Ok(envelope: String) | Err(reason: String) | Cancelled
```

* **No new capability.** Both are writes and perform `NetworkWrite`, the capability of section 17. A program that
  declares `NetworkWrite` may now `POST`, `PUT` and `DELETE` to its own origin; `cancho-ui diff` shows the capability
  and not the method, so the review line "this release can now change things" is unchanged by this section. (A reviewer
  who needs the method reads the builtin in the source or the trace; a per-method capability is rejected here because
  it would make every existing `NetworkWrite` program a breaking change for no reduction in what a same-origin write
  can do.) A program that declares only `Network` and calls either is `capability-missing`; one that declares
  `NetworkWrite` and calls no write is `capability-unused`.
* **Same origin only, no bound.** `NetworkWrite("host")` stays `capability-narrowing-illegal`, a literal URL naming a
  host is `capability-bound-violated` for `net.put` and `net.delete` as for `net.post`, and at run time any other
  origin is `Err("blocked: a write reaches only this site's own origin")` before a request is made.
* **The answer is `net.fetch`'s envelope**, exactly as for `net.post` (17.1): `Ok` of `{"status":N,"body":"..."}` for
  any HTTP status, `Err` for `blocked: ...`, `network error`, `timeout`, `too large`. A `204` is `Ok` with status 204
  and an empty body.
* **Wire form.** `net.put` sends the body as `application/json` like `net.post`. `net.delete` has no body and sends no
  `Content-Type`. Both send this origin's cookies, the `Authorization: Bearer` header of a declared `Credential`
  (section 18) and no other header; redirects are refused, a reply over 1 MiB is `too large`, a reply later than 10 s is
  `timeout`.
* **As for `net.post`**: never retried, not aborted when superseded or disposed (a cut-off write is in an unknown
  state), and each has its own cancellation group (`net_put`, `net_delete`) so a newer `net.put` supersedes only the
  answer of the previous `net.put`.
* **Arity.** `net.put` has exactly two arguments and `net.delete` exactly one; anything else is
  `builtin-shape-mismatch`.

### 21.2 What it touches

* **Lowering:** `net.put`/`net.delete` perform `NetworkWrite`; the literal-URL host check applies with an empty
  allow-list; a `Credential` is used by them (so `capability-unused` does not fire for a credential whose only user is a
  `PUT`); both are known calls (`call-unknown` otherwise).
* **Emit:** `__cui_net_put(url, body, cred)` and `__cui_net_delete(url, cred)`, each emitted only into a program that
  calls it and each a **separate function** from `__cui_net_post` and from each other, for the reason in 17.2: the
  guards of one write are not shared code that a change to another can loosen. A program that only posts carries no
  `method: "PUT"` or `method: "DELETE"`.
* **Replay:** effects `net.put` and `net.delete` with their own groups, scripted by a trace like `net.post`; the step
  report shows `issued #n net.put("/api/q", "{}")` and `issued #n net.delete("/api/d/1")`. A URL or a body that is not a
  String is `replay-type-error`.

### 21.3 Claims to measure (each gets a check that can fail)

1. The adapters are emitted only for a program that uses them, and carry the same-origin guard, `redirect: "manual"`,
   `credentials: "same-origin"`, and no abort wiring.
2. `net.delete` sends no body and no `Content-Type`; `net.put` sends the JSON content type.
3. A program without `NetworkWrite` is `capability-missing` (also with only `Network`); an unused `NetworkWrite` is
   `capability-unused`; a bound is `capability-narrowing-illegal`; a literal on another host is
   `capability-bound-violated`; wrong arity is `builtin-shape-mismatch`.
4. A credential is sent by both and counts as used by both.
5. A trace scripts both, with the newer answer of the same call winning; a non-String argument is `replay-type-error`.
6. In Chromium against real servers: the method, the body, the content type, the cookie and the `Authorization` header
   arrive as stated; `204`, `404` and `409` are answers; another origin is blocked and that server sees 0 requests; a
   redirect is refused; two writes in a row both reach the server.
7. No input reaches a panic (`panic_sites.rs`, `no_panic.rs` run over the examples; the new tests add the cases).
8. Each claim fails against a deliberate break (21.4).

### 21.4 Measured

* `cargo test` (`crates/cancho-ui/tests/net_write.rs`, 10 tests) covers claims 1 to 5 and 7: each adapter is emitted only for
  a program that uses it; both carry the guards of `net.post` and no abort wiring; `net.delete` carries no `Content-Type`
  and no body, `net.put` the JSON one; `capability-missing` (none, and `Network` only), `capability-unused`,
  `capability-narrowing-illegal`, `capability-bound-violated` (three literals each), `builtin-shape-mismatch` (arity 0, 1,
  3 for `put`; 0 and 2 for `delete`); a credential alone with a `PUT` or a `DELETE` is accepted and handed to the
  adapter; a trace scripts both and a put and a delete do not supersede each other; a non-String argument is
  `replay-type-error`. The workspace gate (`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo
  test --workspace`, which includes `panic_sites.rs` and `no_panic.rs`) passes. One existing test, `call_unknown.rs`, used
  `net.delete` as its example of a call the language does not have; it now uses `net.patch` (correct in place: `PATCH` is
  still unknown).
* `scripts/check-write.js` (Chromium, real servers, outside the gate; 24 checks) covers claim 6: the PUT arrives as a PUT
  with the body, `application/json`, the cookie and `Authorization: Bearer`; the DELETE arrives with no body and no content
  type, with the cookie and the bearer; `204`, `404`, `409`, `503` are answers; another origin is blocked **and that server
  receives 0 requests** for both; a redirect is refused and not followed; a 2 MiB reply is `too large`; two PUTs in a row
  both reach the server; without the credential both are `blocked: no credential api` and the server sees 0 requests; with
  the server stopped the answer is `network error`. `check-post.js` and `check-credential.js` still pass.

**The checks can fail.** `scripts/breaks/net-write.json`, run with `node scripts/deliberate-breaks.js` (a green baseline of
three checks first): 19 deliberate breaks, **all caught**. Browser-visible: the put adapter sending a POST, the delete
adapter sending a POST, redirects followed, the cookie omitted, a body and content type on a DELETE, the content type
dropped from a PUT, the origin check removed from each, the credential check removed from each. Compiler-side: `delete`
performing no capability, `put` accepting a missing body, `delete` accepting a body, a write to another host allowed
unless it is a `post`, a credential not counted as used by a `delete`, `delete` an unknown call, a `delete` sharing the
`put` cancellation group, no adapter emitted for a `delete`, and replay accepting a non-String `delete` URL. The first run
reported one real miss, **a delete sharing the put group**: the trace only checked the pending list, which does not change
when a group is shared. The trace now asserts the state after each answer (`a` is `200` while `b` is still `-`) and the
break is caught. Two of the breaks had anchors that occurred twice or not at all and were corrected; they are not
"caught" until the runner reports them.

### 21.5 Not done

`PATCH`, headers other than `Authorization`, a body for `DELETE`, cross-origin writes, a per-method capability, and the
other limits of 17.4. iOS Safari is not tested.

---

## 12. Where the adapters stand

| Capability | Calls | Bound | Real in the browser |
|---|---|---|---|
| `Clock` | `clock.now()`, `sleep(ms)` | none | `Date.now`, `setTimeout` |
| `Storage` | `storage.get(k, d)`, `storage.set(k, v)` | `Storage("ns")` keeps keys under `cui:ns:` | `localStorage`, with a page-lifetime fallback |
| `Network` | `net.get(url)`, `net.fetch(url)` (keeps an error status); `subscribe Event "url"` (a stream of messages the server pushes, `streams.md`); `net.search(q)` is the simulated demo | `Network("host", ..)` adds hosts | `fetch`, same-origin plus the allow-list; a stream is a `fetch` read as it arrives |
| `Credential` | none of its own: `Authorization: Bearer` on `net.get`, `net.fetch`, `net.post`, `net.put`, `net.delete` and on the stream of `subscribe` | `Credential("name", "host"..)` | header added in the adapter, this origin and the named hosts only |
| `NetworkWrite` | `net.post(url, body)`, `net.put(url, body)`, `net.delete(url)` (the envelope of `net.fetch`) | none: this origin only | `fetch` POST, PUT, DELETE, same-origin, not aborted |
| `Locale` | `t(key, ..)`, `fmt.number`, `fmt.percent`, `fmt.date`, `fmt.time`, `locale.current()`, `locale.set(lang)` (`i18n.md`) | none | tables the compiler holds (no `Intl`); `<html lang dir>` |
| `Location` | `location.current()` | `Location("coarse" \| "fine")` | Geolocation, rounded in the adapter |

Not built anywhere: wildcard or per-path `Network` bounds, WebSocket, `watchPosition`, a typed position,
IndexedDB, and any of it on iOS Safari (nothing here was run there).
