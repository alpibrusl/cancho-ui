# `Button { copy: field }`: copying a text the page holds

> **Status: designed and built (15 Nov 2026), with the checks of section 5; the browser half is `scripts/check-clipboard.js`.** Written before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1).
> It extends the `Clipboard` capability that [`structure.md`](structure.md) section 8 already has for `Code { copy: true }`, and answers a gap found while specifying a table browser in cancho-sheets
> (`docs/design.md` section 23, finding F4): a page that shows rows had no way to copy a selection it computed. **Corrected in place:** that finding said cancho-ui had *no* clipboard capability. It has one,
> for a code box; what was missing is a copy of text that is not a code box's. (The error was mine: I searched for the word with the output cut short and read the short list as the whole one.)
>
> **Corrected in place, the first draft of this document.** It proposed `clipboard.write(text) -> Bool`, a call any handler could make. That would have broken the guarantee `structure.md` section 8 states for the
> capability, *write only, and only from a tap by construction: the compiler writes the one call inside the click of the button it draws, and no other code touches the clipboard*, so the draft was dropped before any code
> was written in its favour (one measurement: the three adapters already in the emitter, `Storage`, `Router` and the copy button, show that guarantee is the one the others do not have).

## 1. The decision

```
Button { label: "Copy range" copy: selection }          // `selection` is a String state field
```

* **`copy:` is a binding of `Button`**, the name of a **`String` state field** of the component (as `value:` and `disabled:` name one). A literal or a field of another type is `copy-not-string`. *Choice.*
* **The tap writes the text the field holds at that moment**, as displayed (the view is rebuilt after every state change, so it is the last text shown), and nothing else writes to the clipboard: the one call is emitted inside the click
  listener of the button it draws. **Write only**: there is no read, and the emitted code never calls `readText`. *Choice, the guarantee already in `structure.md` section 8.*
* **The capability is `Clipboard`**, declared as for a code box: `requires Clipboard;` (`clipboard-not-declared` if it is missing; `capability-unused` if it is declared and no `Code { copy: true }` and no `Button { copy: .. }` is
  reachable). `inspect effects` lists it and `diff` reports it as added or removed, with no code of its own. *Read from `structure.md` section 8; tested here.*
* **`copy:` and `on:` may be given together.** The copy runs first, with the text as it is displayed; then the handler. *Choice, so that a handler that changes the field does not change what was copied.*
* **A text over 1,000,000 UTF-16 units is refused, never cut**: nothing is written and the label says "Too long to copy". *Choice for the bound; open until the browser's own limit is measured.*
* **Feedback is the button's own label**, for 1.5 s, as the code box does: "Copied", or "Not copied" when the browser rejected the write or has no clipboard API (there is no box to select, so no "Press Ctrl+C").
  A live region for a person who cannot see the label is the open gap `structure.md` section 8 already records, and it applies here too. *Choice.*
* **Only in a user gesture** is the browser's rule, not the compiler's: a tap is one by construction, which is the point of tying the write to the button. *Read.*

## 2. What the web backend does

The adapter (`COPY_TEXT_ADAPTER_JS`) is emitted only for a program that has a reachable `Button { copy: .. }`, and is separate from the code box's (`COPY_ADAPTER_JS`), so a program with only a code box carries the same code
as before (its test counts the one `writeText(`). It makes the one `navigator.clipboard.writeText(value)` call, and answers the person through the label, with no throw.

## 3. Replay (`cancho-ui test`)

Replay runs handlers, not the view, so **it does not model the tap** (finding F9 of cancho-sheets `docs/design.md` section 23). What a trace can check is the field the button would copy: after a step, `state` names the text. The
report says nothing about the clipboard, and this document does not claim it does.

## 4. Refusals

`copy-not-string` (new), and existing tags: `clipboard-not-declared`, `capability-unused`, `binding-unknown` (`copy:` on an element that is neither `Code` nor `Button`; its message now says "Code and Button").

## 5. The checks, written before the code

`crates/cancho-ui/tests/clipboard.rs` (the compiler) and `scripts/check-clipboard.js` (Chromium, outside the Rust gate as `check-storage.js` is):
* the adapter is emitted for a program with a copy button and for no other, and a program with only a code box carries none of it; the only `writeText(` is the adapter's, called from the click listener; `readText` appears nowhere;
* a copy button without `requires Clipboard;`, a declaration with no use, a field that is not a `String`, a literal, and `copy:` on a `Text` are refused with their codes;
* the example trace replays and passes; `diff` reports `Clipboard` as added;
* **in Chromium** (permission granted): a tap puts the field's text on the system clipboard (read back), a second tap after the field changed puts the new text, nothing is written at load or at a render, a text of 1,000,001 units is not written
  and the label says so, a rejected write says "Not copied", and `readText` is called 0 times.
* **Mutants** of the compiler: the use of `Clipboard` by a button not counted, the declaration check removed, the type check removed, the adapter emitted for every program, the write at render and not at the tap.

## 6. Not here

Reading the clipboard; copying with `await` and a `Result`; rich content (HTML, images); copying from a handler that is not a tap; a copy button for a `Code` box's selection only.

## 7. Measured (15 Nov 2026, one run each; Chromium from Playwright, desktop profile)

* **The compiler checks fail before the code and pass after it**: the six cases of `crates/cancho-ui/tests/clipboard.rs` failed against the compiler without `copy:` on a `Button` (a parse error at the identifier) and pass with it. `cargo test --release`: 87 test
  binaries, 0 failures. `cargo clippy -D warnings` reports three `collapsible_if` errors in `crates/cancho-ui-ir/src/page.rs`, which are there at the commit this work started from and are not touched.
* **In Chromium** (`scripts/check-clipboard.js`, permission granted): typing writes nothing; a tap writes the field's text and a second tap, after the field changed (with a tab in it), the new text; exactly two writes for two taps; `readText` is called 0 times by
  the page; a text of 1,000,001 units writes nothing and the label says "Too long to copy", one of 1,000,000 is written whole; a rejected write and a browser with no `navigator.clipboard` both say "Not copied" with no script error.
* **9 mutants of the compiler, 9 killed** (`scripts/check-clipboard-mutants.js`; each is killed by the Rust tests, the browser check or both, and the table the script prints says which): a copy button not counted as a use of `Clipboard`; the declaration not checked; the field's type
  not checked; the adapter emitted for every program; the write at render and not at the tap; the limit one unit too high and one too low; the adapter reading the clipboard; a rejected write saying "Copied". The two limit mutants are killed
  by different checks (the high one by both, the low one only by the browser): the Rust test only looks for the number in the emitted text.
* **Not measured:** the browser's own limit on what `writeText` accepts (the 1,000,000 is a choice); a phone browser; a screen reader hearing the label change (the gap `structure.md` section 8 records); the behaviour when the click is not a trusted gesture (the page cannot start one: a script's `click()` on the button
  is not a user's tap, and the check does not claim to cover it).
