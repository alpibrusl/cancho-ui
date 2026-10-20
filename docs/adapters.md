# Platform adapters: `Storage` and `Network`

> **Status: designed, and two adapters built: `Storage` (sections 1 to 8) and `Network` as `net.get`
> (section 9).** Written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1); where building
> disagreed with it, the correction is made in place and listed in section 8. `Location` is **not built**.
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

* **Keys are prefixed `cui:`.** `storage.get("note", ...)` reads `localStorage["cui:note"]`. The program can
  neither read nor overwrite a key that something else on the origin wrote, and nothing it writes collides with
  one. This is the first form of `Storage("namespace")` narrowing of `security-model.md` section 2.3, fixed
  rather than chosen by the program; the declared narrowing (`requires Storage("user-prefs")`) is still Phase 2.
* **It never throws.** A private window, blocked site data or a full quota make `localStorage` throw. The
  adapter then keeps the value in memory for the life of the page, `get` finds it there, and `set` answers
  `false`, so a program can tell the user "not saved". After a reload the value is gone, which is the honest
  answer.
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

Replay does not model `localStorage` failing; that is the browser script's (section 6).

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

`Network("host")` narrowing and cross-origin fetches (and with them CORS handling and any credentials), methods
other than GET, request bodies and headers, streaming, aborting the underlying request when it is superseded
(the answer is dropped; the request completes), a retry policy, and the service worker caching API replies. Chromium
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
* **Only `Network` takes a bound for now.** `Clock("x")`, `Storage("ns")` and an unknown capability with a bound
  are `capability-narrowing-illegal`, with the reason (`Storage("ns")` is designed in section 2.3 of the security
  model and not built). So is a host that is not `name[:port]`.
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

* `Storage("ns")` and `Location` bounds (the plumbing exists; the meaning does not), wildcards and subdomains, a
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
language question the adapters raise, not an adapter one.

---

## 12. Where the adapters stand

| Capability | Calls | Bound | Real in the browser |
|---|---|---|---|
| `Clock` | `clock.now()`, `sleep(ms)` | none | `Date.now`, `setTimeout` |
| `Storage` | `storage.get(k, d)`, `storage.set(k, v)` | none (prefix `cui:` fixed) | `localStorage`, with a page-lifetime fallback |
| `Network` | `net.get(url)`; `net.search(q)` is the simulated demo | `Network("host", ..)` adds hosts | `fetch`, same-origin plus the allow-list |
| `Location` | `location.current()` | `Location("coarse" \| "fine")` | Geolocation, rounded in the adapter |

Not built anywhere: `Storage("ns")`, wildcard or per-path `Network` bounds, `watchPosition`, a typed position,
IndexedDB, and any of it on iOS Safari (nothing here was run there).

