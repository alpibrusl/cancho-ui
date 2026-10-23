# Real-time updates: `subscribe`, a stream of messages the server pushes (issue #78)

> **Status: DESIGN** (this PR is the note alone; the build follows). Written 2026-10-24 before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md)
> section 1). Numbers measured in Chromium (Playwright, `Pixel 7` profile, a stand-in HTTP server) on main at 60027ae.

## 1. What exists

A page can refresh by polling (`every 5000 Refresh;`, `schedules.md`) but receives nothing the server pushes. `grep -rn "WebSocket\|EventSource" crates` finds nothing; `security-model.md` calls `Network` "Outbound HTTP /
WebSocket requests", which describes the intent and nothing built. The schedules already have what a stream needs from the page: an event that runs without the user, a `hide` / `show` that pauses it, a fake clock in replay.

## 2. Measured

| question | measured |
|---|---|
| **`EventSource`: can the page set a header?** | **No.** The constructor takes `(url, { withCredentials })`. So `Authorization: Bearer` (`Credential`, `adapters.md` 18, `bearer-token.md`) cannot ride on an `EventSource` |
| `EventSource` after the server closes the stream | reconnects by itself after **3.3 s** (one request, then another 3,303 ms later); the page sees `open`, `error` with `readyState` 0, `open`. The delay is the browser's (the server may send `retry:`); there is no growing backoff and no cap the page can set |
| `EventSource` on a `500` or on `text/plain` | `readyState` 2 (closed) at once, **no reconnect** |
| a **`fetch` stream** read with `getReader()` and a small event-stream parser | sends `Authorization: Bearer tok` and `Last-Event-ID: 41` as the page asked; **the first message arrives while the response is still open**; the page owns the reconnect policy |
| parse cost | one 64 KiB message 0.5 ms, 1 MiB 34 ms, **8 MiB 2,332 ms** (a naive buffer is quadratic: the build bounds a message instead of trusting the parser); 10,000 small messages 46 ms |
| a **frozen** page (a background tab Chrome froze, `Page.setWebLifecycleState`) | a message sent while frozen is received after the page is active again (it was buffered); nothing is lost while the connection survives |
| **through the installed service worker** (`--pwa`) | a chunk written after the headers arrives before the response ends (the worker's pass-through does not buffer); when the server ends the stream the page sees `done`; **with the server stopped, `fetch` rejects with `TypeError`** (the worker answers `Response.error()` for a non-navigation), so the page can tell "down" from "ended" |

So the transport is a `fetch` stream, not `EventSource`: the same host rules, the same `Credential`, a reconnect the page controls, a message size it bounds. (A WebSocket's constructor takes a URL and subprotocols and no headers
either, is bidirectional, and carries binary frames; it is the next transport, not this one. Not measured here.)

## 3. The decision

* **A component member `subscribe`, beside `start`, `every` and `route`:**

  ```
  subscribe Updated "/api/stream";
  subscribe Updated "/api/stream" status Link;
  ```

  `on Updated(text)` runs for **each message**, with its `data` text (several `data:` lines joined with `\n`, per the Server-Sent Events format; comments, `event:` and `retry:` lines are read and ignored). `on Link(state)` (optional)
  runs when the connection changes, with one of five words: `connecting`, `open`, `retrying`, `paused`, `failed`. Both events take exactly one parameter (`subscribe-event-unknown`, `subscribe-event-params`; the same two for `status`).
  A message is data, so a message that arrives while an earlier handler is still waiting on a request runs anyway (unlike an `every` tick, which is skipped).
* **The URL is a string literal**, a path (`/api/stream`, `api/stream`) or an `http(s)` URL (`subscribe-url-invalid` otherwise: no fragment, no credentials in it, at most 2,000 characters), so a review reads where the page listens.
  One `subscribe` per component (`subscribe-duplicate`).
* **Authority: `Network`, no new capability.** A stream is a long-lived GET of the kind `net.get` already is, to this origin or a host in `Network("host")`. It is an effect site of `Network` (`capability-missing` without it), the literal URL is
  checked against the allowed hosts like a `net.get` literal (`capability-bound-violated`), and the adapter checks the resolved URL again and answers `failed` before connecting. `cancho-ui diff` shows `+ subscribe Updated "/api/stream"`
  next to the capability line, so "this release now listens to this address" is a line. A component that declares a `Credential` sends it on the stream (`Authorization: Bearer`, to the hosts it names), with the rules of `bearer-token.md`.
* **Reconnect and backoff, owned by the page.** After a drop: `retrying`, then wait 1 s, 2 s, 4 s, 8 s, 16 s, then 30 s for every later try, then `connecting`; a connection that reached `open` resets the next wait to 1 s. No jitter, so
  replay and the page have one schedule. The page sends `Last-Event-ID` with the id of the last message it had, so a server that keeps a log can resume.
* **What ends the stream for good** (`failed`, no retry): an HTTP status other than 200, a content type other than `text/event-stream`, a redirect, a message over **65,536 characters** (measured: bounded so a hostile or broken server cannot
  make the page build an 8 MiB string), or an unlisted host. Network errors and the server closing the stream are drops, not failures.
* **Hidden pages.** When the page is hidden the connection is closed and the status is `paused`; when it is visible again the page connects at once (`connecting`) with `Last-Event-ID`. The same rule as `every` (`schedules.md` section 11),
  for the same reason: a backgrounded phone should not keep a radio busy for a screen nobody is looking at. A message sent while paused is the server's to keep (the id lets it); a program that cannot rely on that reloads its data when
  `Link` says `open`.
* **Disposal.** A component that is disposed aborts its stream and receives nothing afterwards.
* **Announcing.** Updates are **not** announced by default: a page that updates itself every second would be unusable with a screen reader. A program says which update matters by putting it in a `Text { role: status }`. Known limit, #106: the page
  draws every status node again at every render, and whether a screen reader announces a re-created live region is not measured here (no screen reader was run). A persistent live region is the fix and is its own issue.
* **The offline page.** With `--pwa` the worker passes the stream through (measured), and a stopped server is a `TypeError` the adapter reads as a drop: `retrying` with the backoff above, then `open` when the server returns.

## 4. Replay

Replay has no network; a trace plays the server. New steps: `{ "stream": "open" }` (the connection the page is waiting for is accepted), `{ "stream": "drop" }` (an open connection closes), `{ "stream": "refuse" }` (the answer is not a stream:
`failed`), `{ "message": "text", "id": "7" }` (a message arrives; `id` is optional). Mount runs the machine: after the first render the status is `connecting` and the `status` event runs with it; `advance` fires a due retry (`retrying` then
`connecting` at the due time on the fake clock); `hide` closes the connection (`paused`); `show` reconnects. The step report gains `stream: { status, attempt, retry_in_ms, last_event_id }`, and `expect` can assert it. A message while the status
is not `open`, an `open` that is not awaited, a `drop` of a connection that is not open: `trace-stream-state`. A message over 65,536 characters is accepted as a step and fails the stream, because that is what the page does.

## 5. What the build must have

Tags: `subscribe-event-unknown`, `subscribe-event-params`, `subscribe-url-invalid`, `subscribe-duplicate`, `trace-stream-state` (and the existing `capability-missing`, `capability-bound-violated`). A shared corpus run in replay and in Chromium
against a stand-in server that streams, drops, returns, answers 500 or the wrong type, redirects, sends a 70,000-character message, splits messages across chunks (one byte at a time too) and sends CRLF, CR, BOM, comments, `id`, multi-line
`data`; the backoff schedule measured in the page against replay's; `Last-Event-ID` sent; `Authorization` sent to the declared host and not to another; hide and show closing and reopening; disposal; the installed worker with the server stopped.
Deliberate breaks (no backoff growth, no reset, no hide handling, the id not sent, the size limit removed, a redirect followed, a content type not checked, a message handler skipped while another waits). `examples/stream`. No gallery demo:
a static site has nothing to stream from, and a public echo server is not something the page's `Network` can name without widening it (and this environment's proxy does not reach one).

## 6. Not done

WebSocket (bidirectional, binary); several streams in a component; a stream whose URL the program computes; the event name and `retry:` of the format; a message larger than 64 KiB; jitter; a connection that stays open while hidden; a persistent live
region (#106); a screen reader's speech; iOS and a real phone; a public server.
