# Real-time updates: `subscribe`, a stream of messages the server pushes (issue #78)

> **Status: BUILT** (2026-10-24, issue #78). The design (sections 1 to 6) was written first and merged as #127; sections 7 to 9 say what the
> build made, what it measured and what it corrected in the design. Where a sentence of the design was wrong, it is corrected in place and
> says so. Numbers of section 2 measured in Chromium (Playwright, `Pixel 7` profile, a stand-in HTTP server) on main at 60027ae.

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

So the transport is a `fetch` stream, not `EventSource`: the same host rules, the same `Credential`, a reconnect the page controls, a message size it bounds.

**An assumption, not a measurement (section 8.3 says what the build measured of it, and what stays assumed):** a WebSocket's constructor takes a URL and subprotocols and no
headers either, is bidirectional, and carries binary frames; it is the next transport, not this one.

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
  (Corrected in the build: the bound is on the message; a line of more than 65,542 characters, the longest `data: ` line a legal message can have, also ends the stream, because a line that never ends is a message
  that never ends. A 204 is a status other than 200. A page cannot tell a server that refuses a cross-origin request (no CORS headers, a header not allowed) from a network error: that is a drop, section 8.2.)
* **Hidden pages.** When the page is hidden the connection is closed and the status is `paused`; when it is visible again the page connects at once (`connecting`) with `Last-Event-ID`. The same rule as `every` (`schedules.md` section 11),
  for the same reason: a backgrounded phone should not keep a radio busy for a screen nobody is looking at. A message sent while paused is the server's to keep (the id lets it); a program that cannot rely on that reloads its data when
  `Link` says `open`. A page that mounts while it is hidden shows `paused` and asks for nothing until it is shown (added in the build; replay's traces begin visible, so replay does not play this one).
* **Disposal.** A component that is disposed aborts its stream and receives nothing afterwards.
* **Announcing.** Updates are **not** announced by default: a page that updates itself every second would be unusable with a screen reader. A program says which update matters by putting it in a `Text { role: status }`. Known limit, #106: the page
  draws every status node again at every render, and whether a screen reader announces a re-created live region is not measured here (no screen reader was run). A persistent live region is the fix and is its own issue.
* **The offline page.** With `--pwa` the worker passes the stream through (measured), and a stopped server is a `TypeError` the adapter reads as a drop: `retrying` with the backoff above, then `open` when the server returns.

## 4. Replay

Replay has no network; a trace plays the server. New steps: `{ "stream": "open" }` (the connection the page is waiting for is accepted), `{ "stream": "drop" }` (an open connection closes), `{ "stream": "refuse" }` (the answer is not a stream:
`failed`), `{ "message": "text", "id": "7" }` (a message arrives; `id` is optional). Mount runs the machine: after the first render the status is `connecting` and the `status` event runs with it; `advance` fires a due retry (`retrying` then
`connecting` at the due time on the fake clock); `hide` closes the connection (`paused`); `show` reconnects. The step report gains `stream: { status, attempt, retry_in_ms, last_event_id }`, and `expect` can assert it. A message while the status
is not `open`, an `open` that is not awaited, a `drop` of a connection that is neither being made nor open: `trace-stream-state`. (Corrected in the build: the first draft said "a `drop` of a connection that is not open". A connection
being made can fail, and it is how "the server is down" is played: without it replay could never wait longer than the first second, because only an `open` connection that drops costs the first wait.) A message over 65,536 characters is accepted as a step and fails the stream, because that is what the page does. A retry that falls due at the same instant as a tick or a `sleep` runs after them.

## 5. What the build must have

Tags: `subscribe-event-unknown`, `subscribe-event-params`, `subscribe-url-invalid`, `subscribe-duplicate`, `trace-stream-state` (and the existing `capability-missing`, `capability-bound-violated`). A shared corpus run in replay and in Chromium
against a stand-in server that streams, drops, returns, answers 500 or the wrong type, redirects, sends a 70,000-character message, splits messages across chunks (one byte at a time too) and sends CRLF, CR, BOM, comments, `id`, multi-line
`data`; the backoff schedule measured in the page against replay's; `Last-Event-ID` sent; `Authorization` sent to the declared host and not to another; hide and show closing and reopening; disposal; the installed worker with the server stopped.
Deliberate breaks (no backoff growth, no reset, no hide handling, the id not sent, the size limit removed, a redirect followed, a content type not checked, a message handler skipped while another waits). `examples/stream`. No gallery demo:
a static site has nothing to stream from, and a public echo server is not something the page's `Network` can name without widening it (and this environment's proxy does not reach one).

## 6. Not done (the design's list, kept; section 9 is the list of what the build leaves)

WebSocket (bidirectional, binary); several streams in a component; a stream whose URL the program computes; the event name and `retry:` of the format; a message larger than 64 KiB; jitter; a connection that stays open while hidden; a persistent live
region (#106); a screen reader's speech; iOS and a real phone; a public server.

---

## 7. What was built

* **Language.** `subscribe Event "url" [status Event];` beside `start`, `every` and `route` (`syntax`: `ScheduleKind::Subscribe`, `Schedule::stream`). The IR carries `kind: "subscribe"`, `url`, and `status` on the schedule
  (`ir/src/stream.rs` owns the rules and the numbers: the backoff list, the message bound, the five words, `url_is_legal`). The stream is an effect site of `Network` with the address as its literal, so `capability-missing`,
  `capability-bound-violated` and `capability-unused` (also for a `Credential`, which a stream uses) need no new rule. `cancho-ui diff` prints `subscribe Updated "/api/stream" status Link`.
* **Replay** (`replay/src/stream.rs`): the state machine of section 4, on the fake clock, with the steps `stream` and `message`, the `stream` report and `expect`.
* **Page** (`codegen-web/src/stream.rs`): `__cui_sse`, a parser that reads each chunk once, and `__cui_stream_new`, the connection. The numbers are written into the JavaScript from the IR crate's constants, so the page and replay cannot differ about them.
* **Shared corpus** `tests/stream_corpus.json`: 14 cases (110 steps) read by `crates/cancho-ui/tests/stream.rs` (replay) and `scripts/check-stream.js` (Chromium), and 22 `wire` cases (the same bytes cut in every way) read by the second only.
  The expectations were worked out by hand before either side ran them (every wait of the backoff, the lengths at the bound).
* **Example** `examples/stream` (a live chart with a connection status) and its trace; its stand-in server is `scripts/stream-server.js`, which keeps the last 100 readings and honours `Last-Event-ID`.
* **Tags:** `subscribe-event-unknown`, `subscribe-event-params`, `subscribe-url-invalid`, `subscribe-duplicate`, `trace-stream-state` (in `spec/diagnostics/README.md`; each is reached by a test in `tests/stream.rs`).
* **Not in the gallery:** a static site has nothing to stream from. The "Real-time updates" coming-soon tile is removed (its probe, "WebSocket and EventSource appear nowhere in crates/", is no longer true of EventSource's name).

## 8. Measured at the build

### 8.1 The parser and the page

`scripts/check-stream.js` (106 checks, about 45 s): the parser, extracted from the built module and run in node, reads the 22 `wire` cases whole, cut at every place and one character at a time, and 4,000 random streams
cut at random places, identically to a parser written again in the script from the format (it reads the whole text; it shares no code). Cost: 8 MiB of tiny comment lines in 223 ms against 27 ms for 1 MiB (linear; the note's naive buffer took
2,332 ms for 8 MiB of one message). In Chromium: a line that never ends fails the stream after 196,608 bytes written and the connection is closed; 4 MiB of comment lines and then a message arrive in about 100 ms; a 204 is `failed`; the same bytes written whole and one byte per write
(a BOM, invalid UTF-8, a multi-byte character split across writes among them) give the same messages; the backoff is read off the page: the status word changes at exactly the replay's `retry_in_ms` (999 ms: still `retrying`, 1 ms more: `connecting`).

### 8.2 Authority, and a second origin

* `Authorization` reaches this origin and the host the `Credential` names, and not a host `Network` allows but the `Credential` does not name; not supplied, or not a plain token: `failed` and the server sees no request; a component that sets its own
  token connects without one and sends it on the next connection; an address changed after the build to a host `Network` does not allow, or to `ftp:`, `javascript:` or `http://`, is `failed` at run time and that host is never asked.
* **A second origin must answer a preflight** (measured with a stand-in that logs them). A first request carries only `Accept` and needs none. `Authorization` and `Last-Event-ID` are not CORS-safelisted headers: the browser asks the server first
  (`Access-Control-Request-Headers: authorization`, `last-event-id`). A server that allows both works. **A server that does not allow `Last-Event-ID` makes the reconnect fail in the browser; the page sees a rejected `fetch`, which is a drop, so it
  stays `retrying` with the growing wait and never `failed`. A server with no CORS headers at all is the same.** The page cannot tell the reason; a program that listens to another origin owes that origin's owner a note.
* Chromium re-sends a GET unasked when a socket it had reused is reset before any answer. This hid a reset in the stand-in until the stand-in sent `connection: close`; it concerns the test double, not the page.

### 8.3 WebSocket: what is measured, what is still assumed

`scripts/measure-websocket.js` (a hand-written RFC 6455 server, Chromium, Pixel 7 profile), run once at this commit:

| question | measured |
|---|---|
| constructor | `WebSocket.length` is 1; the prototype has `send`, `close`, `binaryType`, `protocol`, `extensions`, `url`, `readyState`, `bufferedAmount` and four handlers: **no way to choose a header** |
| handshake | carries `Origin`, `Sec-WebSocket-Protocol` (the one asked for), `Sec-WebSocket-Key/Version/Extensions`, `User-Agent`; **no `Authorization`**; `ws://user:secret@host/` is accepted by the constructor and sends **no** `Authorization` either |
| frames | a text frame arrives as a string, a binary frame as a `Blob` (as an `ArrayBuffer` with `binaryType = "arraybuffer"`) |
| direction | the page can send text and binary; the server received both |
| reconnect | none: after the server closed, `close` (code 1000) arrived and **no handshake was made in the next 5 s** |
| service worker | with a worker controlling the page, its `fetch` event saw a plain `fetch` and **did not see the WebSocket** |
| no server | `error` then `close` (code 1006) after 5 ms |

So the sentence in section 2 is now a measurement for: no header from the page, binary frames, bidirectional, and no automatic reconnect (the page would own the policy, as for a stream). Two consequences are new: a bearer token cannot ride the
handshake except in the URL (which is logged by servers and proxies; not measured here), and a service worker cannot pass a WebSocket through or answer for it, so the offline story of section 3 would differ.
**Still an assumption, not measured:** that WebSocket "is the next transport"; its behaviour in a frozen or backgrounded tab on a phone, its battery cost, a message size limit, and iOS Safari.

### 8.4 The deliberate breaks

`scripts/breaks/stream.json`: 45 breaks, each applied to the source, the compiler rebuilt and both checks (`cargo test --test stream`, `check-stream.js`) run. The full run of 46 breaks (20:42 UTC) caught 44; one was missed (below) and one was caught
only because the build failed (replaced by a break that compiles and re-run alone: caught by `cargo test`). Of the 45: 26 are caught by `check-stream.js` only (the page and the parser), 10 by `cargo test` only (replay and the language), 8 by both.
The list covers no growth of the wait, no reset after an open, no pause when hidden, no connect when shown, the id not sent, the size and line limits removed,
a redirect followed, the content type or the status not checked, a drop treated as a failure, `Authorization` to every host, the host not checked at run time or at build time, a missing credential not required, disposal not closing, six
parser rules (CR, a CR before an LF in the next chunk, the space after a colon, an id with NUL, an id without data, an empty id), a BOM read as text, the decoder not told more is coming, a handler skipped while another waits (page and replay), and the
numbers (a 32-second wait, a bound of 65,537).
* **One break was equivalent and is removed:** "a comment line is data" changes nothing, because a line that starts with `:` has the empty field name, which is ignored anyway. The explicit test stays; it is no longer claimed to be checked.
* **A hang, and what it taught.** On the third break (a hidden page keeps its connection) `check-stream.js` waited for something that never arrived and hung for about 55 minutes; I killed it by hand, which proves nothing. The script now ends itself
  after 4 minutes (a watchdog). The break was then run again alone: `check-stream.js` fails by itself in 58 s (exit 1: A11, A12, A13, F) with nothing killed.

## 9. Not done

WebSocket; several streams in a component; an address the program computes; the `event:` name and `retry:` of the format (read and ignored); a message over 65,536 characters; jitter; a connection that stays open while hidden; a persistent live region (#106) and whether a screen reader
announces a status that is drawn again; a real phone and iOS; a public server; a page that mounts hidden is `paused` in the page but not in replay (traces begin visible).
