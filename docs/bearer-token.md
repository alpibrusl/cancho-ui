# A bearer token the program receives and sends (issue #83)

> **Status: DESIGN** (this PR is the note alone; the build follows in the next PR). Written 2026-10-23 before the code
> ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). The numbers in section 2 were measured in Chromium (Playwright, a real HTTP server) on main at 61df10c.
> Builds on `docs/adapters.md` section 18 (`requires Credential(..)`), which this note does not change.

## 1. The gap, and what already exists

* A login that answers `{"token": ".."}` in its body cannot be used by the program today. `Credential("api")` sends `Authorization: Bearer <token>` on `net.get`,
  `net.fetch` and `net.post`, to this origin and to the hosts it names (`adapters.md` 18.4), but the value comes only from the page's glue at mount
  (`mount_X(root, { credentials: { api: token } })`). A program cannot take the token out of a response. The auto-mounted page and `build --pwa` cannot carry one at all
  (`credential-needs-mount-options`).
* What works today is the cookie pattern: the service sets an `HttpOnly; SameSite=Strict` cookie and `net.*` send same-origin cookies (`examples/tickets`). The program never
  holds a secret. That is also the safer arrangement (section 2), and it stays the recommendation.
* A service that returns a bearer token and expects `Authorization: Bearer` (most token APIs, anything a mobile client also uses) is unreachable from a program.

## 2. Where a token may live, measured

A test host page mounts a program with `requires Credential("api")` and a service that records what it receives; a script that stands for an injected one (XSS) then does
what such a script can do. (Chromium; a script of the page's origin.)

| where the secret is | read passively by a script of the origin | used by a script of the origin | persisted |
|---|---|---|---|
| `HttpOnly` cookie | **no** (`document.cookie` is empty) | **yes**: `fetch('/api/me')` from the script carries it (the server saw the cookie) | until it expires |
| a `Credential` held by the module (supplied at mount) | **no**: enumerating every string property of `window`, `localStorage`, `sessionStorage` and `document.cookie` finds nothing | **yes**, and it can be *read*: a script that wraps `window.fetch` after the page mounted receives `{"Authorization": "Bearer tok-.."}` on the program's next request | no (gone at reload) |
| `Storage` (`storage.set("token", t)`, i.e. `localStorage`) | **yes**: `localStorage.getItem(..)` | yes | until cleared |

So no choice makes an XSS harmless. The cookie is the only one a script cannot *read*; memory-only is the next best (nothing to find without running before a request, nothing to
steal from a later disk or storage dump, never in a trace or a report); `Storage` is the worst and is the program's explicit choice, with `requires Storage` on the report next to the credential.
The bearer header, unlike a cookie, is not attached by the browser on its own, so a forged request from another site carries none (no CSRF); that is its real advantage over a cookie.

## 3. The decision

Build it, small, and keep the cookie as the recommended default.

* **`credential.set("api", value)`** stores the token in the component's `Credential`, in memory, and answers a `Bool`: `true` when it was accepted, `false` when it is not a plain token (1 to 4096 printable
  ASCII characters, no space: the same test the adapter applies when it sends one, `adapters.md` 18.4), in which case nothing changes. **`credential.clear("api")`** forgets it and
  answers `true`. Both are synchronous.
* **The name is a string literal that is the component's declared `Credential`** (`credential-name-invalid` otherwise: not a literal, or not the name declared; a call cannot choose between
  credentials, as `credential-ambiguous` already says). So the capability a reviewer reads in `requires Credential("api", "host")` is exactly the one the program may set, and the host
  scoping of 18.4 is unchanged: setting a token does not widen where it is sent.
* **No new capability.** Setting and sending are one authority; `cancho-ui diff` already shows `Credential("api")` added. The calls are effect sites under it (`capability-missing` without it).
* **Memory only.** The token lives in the component instance's record and is gone at reload (the user signs in again). A program that wants it to survive writes it to `Storage` itself, which puts
  `requires Storage` on the report and makes the exposure in the table above its own, visible choice. No `persist` option is offered.
* **The value does not go through state if the program does not put it there.** `credential.set("api", json.get(reply, "token", ""))` passes it from the response to the credential in one expression.
  A program that assigns it to a state field first has it in every report's final state; the note says so, and cannot prevent it (a lint for "a state field that receives a response's `token`"
  would be a heuristic; not built).
* **Sign-out** is `credential.clear("api")`, usually after a `net.post` that tells the service to revoke. The next request answers `Err("blocked: no credential api")` before any request is made
  (the existing behaviour for a missing token).
* **Auto-mount.** `credential-needs-mount-options` exists because an auto-mounted page has nothing to give a component its token. A component that calls `credential.set` does not need one, so the
  refusal applies only to a component that declares a `Credential` and never sets it; `build --pwa` and `--page` can then carry a program that signs in by itself.
* **Children.** A component's children receive the credentials it was mounted with, copied when they mount. A token set later is not seen by a child already mounted. The program that holds the
  `Credential` makes the requests (documented, not changed).

## 4. Replay

* A trace may still list `"credentials": ["api"]` for a token the page supplied at mount. `credential.set` adds the name to the supplied set when the value is a plain token; `credential.clear` removes it.
  The value is never stored in the machine, the report or the trace: replay needs only whether a token is present.
* The step report gets `credentials` (the names present after the step, only for a component that declares one), so `expect: { "credentials": ["api"] }` asserts "signed in" and `[]` asserts "signed out".
  The `issued` line already says `[credential api]` or `[credential api (not supplied)]`.
* A scripted login response carries a token like any scripted body (`{"Ok": "{\"token\":\"t-1\"}"}`): that is authored test data, not a secret.

## 5. What the build must have

A Chromium check against a service that returns a token and rejects a request without `Authorization: Bearer <that token>`: sign-in stores it, the next `net.get` and `net.post` carry it, the token
arrives at the declared host and not at one the credential does not name, a malformed token is `false` and sends nothing, sign-out clears it and the next request is blocked before it is made, a reload
forgets it, and the token is in no `localStorage`, `sessionStorage`, cookie, window property or serialized DOM. A shared corpus run in replay and in the page; deliberate breaks (set accepted for any
string, clear that keeps the token, set that widens the hosts, the auto-mount refusal kept, the token written to storage, the value in the report). Tags: `credential-name-invalid` (new) and the existing
`builtin-shape-mismatch`, `capability-missing`, `credential-ambiguous`; the `credential-needs-mount-options` text is corrected.

## 6. Not done

* A scheme other than Bearer; a refresh or expiry (the program signs in again; `credential.set` replaces the token).
* A token that survives a reload other than through `Storage` (and the cookie pattern, which needs no program code).
* Hiding the token from a script that wraps `fetch` before or after the page: the adapter calls the global `fetch` at request time. Capturing it at module load would shrink the window (a wrapper installed
  *after* load would no longer see the header) but not close it (a wrapper installed before load still would), so it is recorded, not built.
* A lint for a token stored in state; a credential for `net.search`; setting a credential of a child component from its parent.
* A real service, a real phone, iOS: the evidence is a stand-in service in Chromium.
