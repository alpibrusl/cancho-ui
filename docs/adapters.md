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

1. **A string interpolation cannot hold a string literal.** `"Saved: {storage.get("note", "none")}"` is
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

1. **A string literal with braces is interpolated only inside `Text`.** `status = match r { Err(why) => { "error:
   {why}" } ... }` compiles, and the emitted JavaScript returns the text `error: {why}`, braces included. Replay
   shows it (it was replay that caught it). The workaround is `"error: " + why`. This is the same family as
   section 5 item 1 and a worse one, because nothing is refused. Either interpolation should work in every
   string expression or a `{` in one should be refused; that is a language decision, recorded here and not made.
2. **An awaited `net.get` that returns `Ok(text)` binds a `String`, not a `List[String]`.** The checker does not
   type the payload of `Result[..]` (`state r: Result[String, String]` is accepted for `net.search` too);
   nothing here changes that.

### 9.5 Not done

`Network("host")` narrowing and cross-origin fetches (and with them CORS handling and any credentials), methods
other than GET, request bodies and headers, streaming, aborting the underlying request when it is superseded
(the answer is dropped; the request completes), a retry policy, and the service worker caching API replies. Chromium
only; iOS Safari is not tested.
