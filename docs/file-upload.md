# Choosing a file and sending it: `FileField` (issue #79)

> **Status: DESIGN** (this PR is the note alone; the build follows). Written 2026-10-23 before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md)
> section 1). Numbers measured in Chromium (Playwright, `Pixel 7` profile, files set through the browser's file chooser) on main at 30df5cc.

## 1. What exists

* `FileField { label: "Upload" }` is `component-kind-unknown`; the emitter writes no `<input type="file">`.
* `net.post(url, body)` (`adapters.md` section 17) sends a **String** as `application/json` to this origin, needs `requires NetworkWrite`, and the program builds the JSON text
  (`"[" + json.quote(a) + "," + json.quote(b) + "]"`, as `examples/tickets` does). The language has no byte values.

## 2. Measured

| question | measured |
|---|---|
| what the browser gives a page for a chosen file | `files[0]` has `name` (no path), `size` (bytes) and `type` (`text/plain`, `image/png`, `application/octet-stream`); `value` reads `C:\fakepath\a.txt` |
| **the page redraws from nothing at every state change** | a chosen file is **lost**: the new `<input type="file">` has `files.length` 0 (`root.textContent=''` is what every render does) |
| putting the old `FileList` back: `newInput.files = oldInput.files` | **works**: `files.length` 1, name `a.txt`, and the control shows the name again |
| `accept=".txt"` and a PNG set through the chooser | `change` fires and the file is the PNG: `accept` filters the OS dialog and is not a guarantee; the page must check |
| reading a file as text, strictly (`TextDecoder('utf-8', {fatal: true})` on the bytes) | `a.txt` (13 bytes) ok; 3,000 random bytes **rejected**; a PNG header **rejected**; a UTF-8 file with a BOM ok, BOM removed (`café`, 4 characters from 8 bytes); 1 MiB + 1 byte ok, **39 ms** |
| the lenient way, `FileReader.readAsText` | the same binary files come back with `U+FFFD` replacement characters and no error: a program would send garbage |
| accessibility tree of a labelled input | role **button**, name = the label, value `No file chosen`; focusable; Enter or Space opens the chooser (native) |

## 3. The decision

* **A native control.** `FileField { label: "Report" accept: ".txt,.csv" max_bytes: 100000 on: Picked }` is a `<input type="file">` inside the same `<label class="cui-field">` as a `TextField`
  (the label is visible and is its accessible name), so the keyboard, the screen reader and the Android and iOS pickers are the platform's. One file (no `multiple`).
* **The page keeps the choice across redraws** by putting the `FileList` back into the rebuilt input (measured above), keyed by the place of the `FileField` in the source. So a `FileField` is refused inside a `For`
  (`file-field-in-loop`): every row would share one key.
* **What the program receives: one `String`**, the JSON text of
  `{"name": "report.txt", "size": 13, "type": "text/plain", "text": "..."}`, or, when the file cannot be read as the page promises,
  `{"name": .., "size": .., "type": .., "error": "<one of four words>"}` with no text. The words: `too large` (the size is over `max_bytes`; the file is not read), `not text` (the bytes are not UTF-8: no lenient
  decoding, a program never gets replacement characters), `not an accepted type` (see `accept`), `unreadable` (the read failed). The program takes it apart with `json.get`, as it does a reply. The event fires
  once per choice; cancelling the chooser fires nothing. The path, the last-modified time and the bytes of anything else are never given.
* **Limits.** `max_bytes:` is an integer literal from 1 to 1,048,576 (1 MiB, the same cap as a reply), default 1,048,576 (`file-limit-invalid`). `accept:` is a string literal of comma-separated tokens: `.ext` (letters and digits),
  `type/subtype` or `type/*` (`file-accept-invalid`); the page applies it to the name's extension (case-insensitive) or the file's type, because the dialog's filter is not a guarantee (measured).
* **No capability.** The program cannot open a chooser, and receives nothing the user did not hand it in the gesture; there is no standing authority to declare, which is the difference from `Location` and `Network`.
  The new element is in `inspect`'s tree and in `cancho-ui diff` (`FileField` added), so a reviewer sees "this page takes files". Where the text goes is declared as before: sending it is `net.post`, `requires NetworkWrite`.
* **Sending: the existing `net.post`.** The program builds the JSON body it wants, usually `"[" + json.quote(json.get(f, "name", "")) + "," + json.quote(json.get(f, "text", "")) + "]"`. No multipart and no binary.
  Reason: there are no byte values in the language, and a body of text is the whole of what a text file needs. Option B, recorded and not built: keep the `File` in the page and give the program an id, then
  `net.upload(url, id)` sends the bytes as the body (binary-safe, no copy into a JavaScript string, any size) without the program ever reading them. It needs a handle type that is not a String, a new capability line,
  a cap and a progress story; it is the right next step for images and large files.
* **Replay.** A trace gives the file as the event: `{ "event": "Picked", "params": ["{\"name\":\"a.txt\",\"size\":13,\"type\":\"text/plain\",\"text\":\"hello, world\\n\"}"] }`. When every control that fires an event is
  a `FileField`, replay refuses a parameter the page could not produce (`trace-value-not-a-file`, as `trace-value-not-an-option` does for a `Select`): not a JSON object with exactly `name`, `size`, `type` and one of
  `text` or `error`; a name with `/` or `\`; a size over `max_bytes` with a `text`; a `text` longer in bytes than `size`; an `error` that is not one of the four words; no error for a file that `accept` would refuse.
* **Privacy.** The page sees the chosen file's name, size, type and text, never its path (Chromium shows `C:\fakepath\`). The name is the user's text: drawn only as text, never as markup.

## 4. Accessibility

Role `button` named by the label, value "No file chosen" or the file name, keyboard-operable by the platform. After a choice the program should say what it got (a status `Text`: name, size); the native control also shows the name.
`disabled:` is the native attribute. Not measured: a screen reader's speech; focus after the chooser closes (the browser's).

## 5. What the build must have

Rule tags: `file-field-incomplete` (no `label:` or no `on:`), `file-accept-invalid`, `file-limit-invalid`, `file-field-in-loop`, `trace-value-not-a-file`; `accept:`, `max_bytes:` and `on:` with a payload on other
elements are `binding-unknown`. A shared corpus run in replay and in Chromium (files set through the chooser: text, empty, UTF-8 with a BOM, invalid UTF-8, a PNG, exactly `max_bytes` and one over, `accept` by extension and
by type, a name with spaces and quotes), the selection surviving a redraw, an upload to a stand-in endpoint with the body checked byte for byte, the accessibility tree, deliberate breaks (lenient decoding, no size
check, no `accept` check, the selection lost at a redraw, the path given, the name as markup). `examples/upload`.

## 6. Not done

Binary files and multipart (option B above); several files; drag and drop (#80); camera capture; progress and cancellation of a big upload; files over 1 MiB; reading a file as anything but UTF-8 text; a `FileField` in
a `For`; re-choosing the same file by a real chooser (`change` does not fire when the selection is unchanged; the harness's chooser always fires it, so this is not measured); a screen reader; iOS and a real phone.
