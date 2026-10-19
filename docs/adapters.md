# Platform adapters: `Storage` first

> **Status: designed, and the first adapter built: `Storage`.** Written before the code
> ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1); where building disagreed with it, the correction is made in
> place and listed in section 8. `Network` as `fetch`, and `Location`, are designed here and **not built**.
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

* **`Network` as `fetch`** (`net.get(url)` awaited, with a `Network("host")` bound), **`Location`**
  (`location.current`), the declared narrowing `Storage("ns")`, an event for another tab's write, an
  asynchronous store, quotas beyond "it threw", and values that are not strings.
* **Argument counts of the other builtins** (`clock.now`, `sleep`, `net.search`) are not checked statically.
* **iOS Safari** is not tested; Chromium only. Safari's private mode is known to make `localStorage` throw on
  older versions; the fallback exists for it, but this document does not claim it was observed.

## 8. Corrections

*(None yet.)*
