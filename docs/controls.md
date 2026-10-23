# Controls: `disabled:`, `submit:` and the keys of a built-in element

> **Status: built (2026-10-21), issues #35 (sections 1, 2) and #36 (section 3).** Written from what `docs/ops-console.md`
> 2.3 and 2.4 measured; the checks are in section 4.

## 1. The decision

```
Button    { label: "Apply" on: Apply disabled: sending }
TextField { label: "Note" value: note on: Edit disabled: locked }
Button    { label: "Never" on: Never disabled: true }
```

* **`disabled:` is a binding of `Button` and `TextField`.** Its value is `true`, `false`, or the name of a `Bool` state field of
  the same component, read on every render. Anything else (a number, a string, a field that is not `Bool`, a name that is no
  field) is `disabled-not-bool`; on `Column`, `Row` or `Text` it is `binding-unknown`.
* **The emitted control is natively disabled** (`el.disabled = !!state`), so the browser itself stops the click, the Enter and
  Space key on a button and the typing in a field, and exposes the state to assistive technology; no `aria-disabled` is added on
  top of it.
* **The pattern for a write** is a flag the handler sets around the request:

  ```
  on Apply { sending = true; r = await(net.post(url, body)); sending = false; ... }
  ```

  A state change made before an `await` is painted before the request is waited for (`AGENTS.md` section 3), so the second click
  of a double click finds the button already disabled. Measured in 4.1 below.
* **An unknown key is refused.** The built-in elements take `label`, `role`, `on`, `value`, `disabled`, `submit` and `placeholder`
  (written as the HTML attribute of that name). Any other key is `binding-unknown`. Before, an unknown key was accepted and
  either dropped or written as a stray attribute: `Button { disabled: true }` compiled and did nothing, which is how a
  double click came to send two writes. Component instances keep their own rule (`property-unknown-key`).

## 2. Replay

Replay does not render the view, but it can answer one question about it: *would the browser have delivered this event?* A step
`{"event": "Apply"}` is refused with the outcome **`disabled`** when the view has at least one control with `on: Apply` and
**every** such control is disabled in the current state (`true`, or a `Bool` field that is `true`). The handler does not run
and nothing changes: no state, no request. *(This is the rule for `on:` controls; section 3 states it over the ways to fire an event once `submit:` exists: Enter is a second way, open only while the button is.)* An event that no control fires (a schedule's `Tick`, an event only a script calls)
is never refused, and an event with an enabled control among several is delivered. `expect: {"outcome": "disabled"}` asserts
it; a trace that fires a disabled control and expects the handler to have run fails at that step.

A control inside an `If { when: .. }` whose condition is false is not in the page (`docs/view-conditional.md`): if that is every control that fires the event, the outcome is **`hidden`**, a different fact from `disabled` (a control that is there and cannot be used), and the handler does not run. A hidden disabled button does not block Enter in a field either.

What replay does not model: which of two controls bound to the same event the user pressed (it only knows whether any
could have been), and the timing of the paint (the browser check in 4.1 covers that).

## 3. `submit:`, Enter in a field (issue #36)

```
TextField { label: "Setpoint" value: setpoint on: SetSetpoint submit: Apply }
```

* **`submit: Event` is a binding of `TextField`.** Pressing Enter in the field fires `Event`. The event must be one of the
  component's and must take **no parameters** (Enter has none to give); otherwise `submit-event-invalid`. On anything but a
  `TextField` it is `binding-unknown`.
* **Emitted** as a `keydown` listener that fires only for `Enter`, not while an IME is composing (`isComposing`: the Enter
  that confirms a candidate must not send a form) and not for the auto-repeat of a held key (`repeat`), and that calls
  `preventDefault` so the browser does not also act on it. A disabled field never gets the key.
* **Enter is the same act as the button.** A field with `submit: Save` next to `Button { on: Save disabled: sending }` must not
  get a second write through while the first is in flight, and the field itself is not disabled (that would take its focus
  and its text away mid-edit). So the page carries one table, `__submit_blocked`, event to a function that answers "is any
  control with `on: Event` disabled right now?", and the listener fires only when it answers no. Measured in 4.1: without
  it a second Enter during the write sent a second write.
* **Replay.** A trace fires an event by name, not a key, so the rule of section 2 is stated over the *ways to fire it*:
  a control with `on: E` is a way that is open when it is enabled, and a field with `submit: E` is a way that is open when it
  is enabled **and** every `on: E` control is enabled. The event is `disabled` when no way is open. With one button and one
  field that is exactly "the button is enabled".
* `accessibility-keyboard-missing` (AGENTS.md section 7) stays a contract: with `submit:` a form can have a keyboard path, but
  nothing yet refuses a form without one (section 5).

## 4. Checked

### 4.1 In the browser

`scripts/check-controls.js` (Chromium, a real server that holds the answer): a double click on `disabled: sending` sends **one**
write (the same page without the binding sent 2, `docs/ops-console.md` 2.3), the button is disabled while it is in flight, a forced
click on it sends nothing more, the answer re-enables it and a new click works, and a fast triple click sends one write, not
three; `disabled: true` is a disabled button whose event never fires; a `TextField` with `disabled: locked` takes no typing.

For `submit:`: Enter in a field sends **one** write; a second Enter while it is in flight sends nothing (the guard); a held Enter
(three `keydown` with `repeat`) sends one write; an Enter that confirms an IME composition (`isComposing`) sends nothing and typing
still works; on an event nothing disables, a held Enter fires it once and a new press fires it again; a letter or Tab fires nothing.

### 4.2 In the gate

`crates/cancho-ui/tests/controls.rs` (9 tests): what is emitted for a field, a literal and a `TextField`; the four ways to get
`disabled-not-bool`; `binding-unknown` for a stray key and for `disabled:` on a `Column`, with `placeholder` still legal;
a trace in which the second `Save` is `disabled` and the third, after the answer, is delivered; an event no control fires is
never disabled; `submit:` is a guarded `keydown` listener, is refused on a `Button` (`binding-unknown`) and for an unknown event or one
with parameters (`submit-event-invalid`); Enter is closed while the button of the event is disabled; an event only a disabled
field fires is `disabled`.

### 4.3 The checks can fail

Browser: the state-field form no longer emitted (killed by 7 checks, including the 2 writes), the literal form no longer
emitted (killed by 2). Gate: replay refusing an event no control fires (1), replay treating any `Bool` as disabled (1), `title`
allowed as a key (1), any state field allowed for `disabled` (1), `disabled` allowed on a `Column` (1). For `submit:`, browser: the
IME guard removed (1), the `__submit_blocked` guard removed (2), the table ignoring the disabled state (2), any key but Tab
submitting (3), the auto-repeat guard removed (2; **the first version of the checks missed it**, because the guard of a
disabled button already stops the repeats; the extra event with nothing disabling it kills it, and so does the `Tab`/letter check
for "any key").

## 5. `type:`, the kind of text a field holds (issue #76, first part)

> **Status: BUILT** for `type:`. The other parts of #76 (a checkbox, a choice, a text area, an invalid state) are not built; see
> section 7.

**Measured before** (`cancho-ui check` on a one-element program): `TextField { .. type: "password" }` was
`error[binding-unknown]: type: is not a binding of TextField`, so the password of a sign-in form (`examples/tickets`) was typed
in the clear, with `type=text` in the built page (`docs/tickets.md` section 3).

**The form.** `TextField { label: "Password" value: pass on: Pass type: "password" }`. `type:` is a binding of a `TextField`
only (`binding-unknown` on anything else) and takes one of `text` (the default), `password`, `email`, `search`, `tel`, `url`, as
a string literal; anything else, including a state field, is `input-type-invalid`. The emitted page sets that `type` attribute
and nothing more: the value the handler receives is the string the browser reports, as for `text`.

**Why not `number`, a date, a checkbox.** Those give the handler something other than what is on the screen, or are another
control. Measured in Chromium (`scripts/check-input-types.js`, the compiler's own text field with only its type attribute
changed after the build): typing `1e5` key by key into a number field ends holding `5`. A number field answers `""` while what is
typed is not yet a number, and the page draws the field again from the state at every key (`docs/semantics.md`, the view is a
function of the state), so the half-typed text is wiped. The six types offered are all one line of text.

**What each type does to the value, and what replay does.** Replay does not draw the field, and a trace gives the event its
parameter directly, so the trace of a field gives the value *the browser would report*. Measured
(`tests/input_types_corpus.json`, typed into a field of each type in Chromium and replayed by `cargo test` with the `received`
value): `text`, `password`, `search` and `tel` keep what is typed, spaces included; `email` and `url` drop the whitespace around it
(`"  ada@example.org  "` arrives as `"ada@example.org"`), and keep text that is not an address. A trace for an `email` field that
gives `"  ada@example.org  "` is a trace of a page that does not exist.

**What the browser does with it** (measured, same script): the attribute is on the element; the field is masked
(`-webkit-text-security: disc`); typed key by key through the re-render every key causes, a password field keeps its type, the
focus and every character. The password is in the program's state like any other string: `type: "password"` hides it on the
screen and tells a browser and a password manager what the field is, and does not change how the program may use it (the
`cancho-ui test` report prints state values, so a trace should not hold a real password).

**Rule tag.** `input-type-invalid` (`spec/diagnostics/README.md`). **Deliberate breaks** (`scripts/breaks/input-types.json`): see
the file; each is caught.

**`examples/tickets`** sets `type: "password"` on its password field; `scripts/check-tickets.js` asserts the attribute (the note
that recorded the gap is a check now). The gallery's "Sign-in flow" tile (#83) named three things: a password input (this), a
screen to move to after signing in (`docs/view-conditional.md`), and a place for what a login returns (a cookie the service sets,
as in `examples/tickets`, or a state field). Its probe now passes, so `check-gallery.js` R5 failed and the tile was removed;
there is no tickets demo in the gallery, because the static host has no login route and the demo could only show a refused
sign-in.

## 6. `autocomplete:`, `inputmode:`, `maxlength:` and `name:` (issue #91)

> **Status: BUILT.**

**Measured before.** `TextField { label: "User" value: s on: Go autocomplete: "username" }` was
`error[binding-unknown]: autocomplete: is not a binding of TextField`, and so were `inputmode:`, `maxlength:` and `name:`.
Without `autocomplete="username"` and `current-password` a browser or a password manager cannot reliably fill a sign-in form;
without `inputmode` a phone shows the wrong keyboard; and the 200-character limit of the note in `examples/tickets` was a message
after the fact and a disabled Save, not a limit.

**The bindings**, all on a `TextField` only (`binding-unknown` on anything else) and all with a checked value
(`input-attribute-invalid`, one tag with a message per binding):

| Binding | Value | Written as |
|---|---|---|
| `autocomplete:` | a string literal: `on`, `off`, or one field name of the HTML autofill list (`username`, `current-password`, `new-password`, `one-time-code`, `email`, `tel`, `given-name`, ...: 53 in all, `AUTOCOMPLETE` in `crates/cancho-ui-ir/src/controls.rs`). A section or a hint (`shipping given-name`) is not offered. | the attribute |
| `inputmode:` | one of `none`, `text`, `decimal`, `numeric`, `tel`, `search`, `email`, `url` | the attribute |
| `maxlength:` | an integer literal from 1 to 100,000 | the attribute |
| `name:` | a string literal of 1 to 64 letters, digits, `-`, `_`, `.`, starting with a letter | the attribute |

**What the browser does with `maxlength`** (measured, `scripts/check-input-attrs.js`, `tests/input_attrs_corpus.json`): typing,
setting a whole value (Playwright `fill`) and pasting all stop at the limit, so the handler never receives a longer value; the limit
counts UTF-16 code units (with `maxlength: 3`, two emoji of two units each leave one emoji, and a following letter still fits);
and a value the *program* sets (`v = "..."`, drawn back into the field) is not cut: the limit is on a person's typing, not on the
state.

**What replay does.** A trace gives an event its parameter directly, so a step that gives a field's handler a string longer than
the field allows describes a page that does not exist. Replay stops with `trace-value-too-long` (`docs/replay.md`), counting UTF-16
code units. It stops only when every control in the page that fires the event is a `TextField` with a `maxlength:` (the longest
limit counts), when no button can give the event a value of any length, and a control inside a false `If` is not in the page. The
other three attributes have nothing to assert beyond being valid. Replay does not cut a value: the browser would, but a trace that
needs cutting was written wrong, and a cut value would hide that.

**Rule tags.** `input-attribute-invalid`, `trace-value-too-long` (`spec/diagnostics/README.md`).
**Deliberate breaks** (`scripts/breaks/input-attrs.json`): twelve, each caught. The first run missed one (replay applying a limit although a button could hand the event a long value); a test was added.

**`examples/tickets`** sets `autocomplete` and `name` on both fields of the sign-in form, and `maxlength: 200` on the note. The
note's validation message and its state are gone (the field stops at 200), and so is the trace step that typed 201 characters,
which replay would now refuse. `check-tickets.js` fills 201 characters and finds 200.

**Not built:** `minlength`, `pattern`, `required` (validation the program already does with `if`), `spellcheck`, `autocapitalize`,
`enterkeyhint`, and `autocomplete` with a section or several tokens.

## 7. `error:`, a message tied to its field (issue #76, second part)

> **Status: BUILT** for `TextField`. What is left of #76 is in the last section ("Not done").

**Measured before** (Chromium, accessibility tree through the DevTools protocol; a `TextField` with a `Text { role: status
tone: "error" "{msg}" }` after it, the way `examples/tickets` did): the textbox is `invalid: false` whatever the field holds,
with no description. The message is a live region on its own and nothing says it is about that field; a screen reader user who
moves to the field is not told it is wrong or why.

**Options.**

| | Option | Against |
|---|---|---|
| A | `invalid: flag` on the field, and an `id:` on the `Text` plus `describedby: id` on the field | three new bindings; an id must be unique in the page, across the rows of a `For`, and the page draws everything again at every render, so a name that does not match, or that two rows share, breaks silently |
| B | **`error: msg` on the field**: the field draws its own message next to itself and points at it | one binding; the pair cannot be written wrong |
| C | Do nothing; author writes `aria-*` by hand | there is no way to write them |

**Chosen: B.** `TextField { label: "User name" value: user on: User error: user_msg }`. `error:` names a `String` state field of
the component (anything else is `error-binding-invalid`; on anything but a `TextField`, `binding-unknown`). The field is invalid
while that field holds text and valid while it is empty: there is no separate `invalid:` to disagree with the message.

**What the page draws.** A `<span id="cui-err-N" role="status" data-tone="error">` right after the field's `<label>` holding the
message; while the message has visible characters the field has `aria-invalid="true"` and `aria-describedby="cui-err-N"`; while
it has none the field has neither and the span has no tone and no mark (the rule of section 3 of `docs/presentation.md`, #94).
The id comes from a counter on `window` (`__cui_uid`), not from the node number, because a `For` runs the same code once per row.

**Measured after** (`tests/field_error_corpus.json`, `scripts/check-field-error.js`): while invalid, `aria-invalid="true"`, the
target of `aria-describedby` is the status span holding the message, and the accessibility tree has the textbox `invalid: true`
with the message as its description; while valid, neither attribute, `invalid` is not true, the description is empty and the span
is 0 px tall with no mark; typing keeps the focus in the field; two fields in a `For` get two ids and each points at the message
right after it; the ids are unique. **A finding:** the accessible description is `✗ At least 3 characters.`, because Chromium
includes the generated content of `::before` (the tone's mark) in the text it computes. The mark is a deliberate second signal
for sighted users (section 3 of `docs/presentation.md`) and is the only thing that tells a screen reader the tone of a status text
that is not tied to a field, so it is left as it is; the stylesheet's alternative-text form (`content: ".." / ""`) would hide it
from assistive technology and is not used.

**Replay** does not draw: it models `error:` as it models `placeholder:`, not at all. The message is a state field, so a trace
asserts it (`"state": {"msg": "..."}`) and the corpus does. **Not measured:** the announcement itself with a real screen reader
(alpibrusl/cancho-ui#106).

**Rule tag.** `error-binding-invalid` (`spec/diagnostics/README.md`). **Deliberate breaks:** `scripts/breaks/field-error.json`, eight (never invalid, no describedby, invalid when empty, one shared id, not a status element, an empty message keeping its tone, any field accepted, any element accepted), each caught.
**`examples/tickets`** uses `error:` on both sign-in fields and on the status field of the edit form; `check-tickets.js` asserts
`aria-invalid` and `aria-describedby` come and go with the message.

## 8. `TextArea`, several lines of text (issue #76, third part)

> **Status: BUILT.**

**Measured before.** `TextArea { label: "Note" value: n on: Note }` was `error[component-kind-unknown]`. A note in
`examples/tickets` was a one-line field with `submit: Save`, so a service note could not hold a line break.

**The form.** `TextArea { label: "Note" value: note on: Note rows: 4 maxlength: 200 error: msg }`. It is a `TextField` that holds
several lines, written as a `<textarea>` inside the same `<label class="cui-field">`, with the same `label:` (required), `value:`,
`on:` (the handler takes the text), `disabled:`, `placeholder:`, `autocomplete:`, `inputmode:`, `maxlength:`, `name:` and `error:`.
Differences, all refused with a tag: **no `type:`** (a textarea has none; `binding-unknown`) and **no `submit:`** (Enter inserts a
line break there; `binding-unknown`, and the message says so). One binding of its own, `rows:`, an integer literal from 1 to 30
(`input-attribute-invalid`), because a textarea is two lines tall without it; on any other element `rows:` is `binding-unknown`.

**What the browser gives the handler** (measured, `tests/textarea_corpus.json`, `scripts/check-textarea.js`): the text with its
line breaks as LF; Enter inserts one; pasted CRLF and bare CR arrive as LF (so a trace gives LF, as for `email` in section 5); a
leading and a trailing line break are kept; spaces are kept; **a line break counts as one character of `maxlength`** (`maxlength: 3`
leaves `a\nb` of `a\nb\nc`, typed or pasted). Replay counts the same (UTF-16 code units, a line feed is one), so the
`trace-value-too-long` of section 6 holds for a `TextArea`.

**A finding, fixed:** the page draws a field again from nothing at each key, and restored the focus and the caret but not the
scroll position. Measured in a 40-line note scrolled to line 12 with the caret there: after one more key the first draw of the new
element starts at the top and the browser's scroll-to-caret leaves it 40 px off the old position. The focus helper
(`__cui_focus_save` and `__cui_focus_restore`) now records `scrollTop` and puts it back; it applies to every field, and a field
that does not scroll saves 0 and restores nothing. Measured after: caret, scroll position and focus are all as before the key.

**Accessibility** (Chromium's tree): a `textbox` with `multiline: true`, its label as the name; with `error:` it is `invalid` and
described by its message, as section 7.

**Replay** treats it as it treats a `TextField` (the value is the event's parameter; `error:` and `rows:` are not modelled).
**Rule tags:** none new (`binding-unknown`, `input-attribute-invalid`, `error-binding-invalid`). **Deliberate breaks:**
`scripts/breaks/textarea.json`, ten, each caught. **`examples/tickets`** takes its note in a `TextArea` with `rows: 4`; the Save
button is its only way to send (no Enter), and the check script is unchanged.

**Not built:** `cols`, `wrap`, `resize`, `readonly`, `spellcheck`, a rich-text editor.

## 9. `Checkbox`, a box bound to a `Bool` (issue #76, fourth part)

> **Status: BUILT.**

**Measured before.** `Checkbox { label: "Agree" }` was `error[component-kind-unknown]`. The program could only use a button that
flips a field, which a screen reader announces as a button and not as a state.

**The form.** `Checkbox { label: "I agree" checked: agreed on: Agree disabled: locked error: msg name: "agree" }`.
`checked:` names a `Bool` state field (`checked-binding-invalid` otherwise) and `on:` names the event the box reports to; both are
required (`checkbox-incomplete`: without `checked:` the page would forget the box at the next redraw, and without `on:` it would
tell the program nothing). The handler takes the new state: `on Agree(v) { agreed = v; }` gets `true` or `false` (it may also
take none, `on Flip { on_ = !on_; }`, as a field's handler may; two parameters are `event-arity-mismatch`). A Checkbox takes
only `label`, `role`, `on`, `checked`, `disabled`, `error` and `name`; any other key (`value`, `placeholder`, `type`, `submit`,
`maxlength`, `rows`, `tone`, ...) is `binding-unknown`, and `checked:` on another element is too.

**What the page draws.** `<label class="cui-check"><input type="checkbox" aria-label=".."> text</label>`: the box first, then the
text, in one label, so a click on the text toggles the box. The box is `.checked = !!state` at every redraw and reports
`e.target.checked` on `change`. With `error:` the message and `aria-invalid` / `aria-describedby` work as in section 7.

**Measured** (`tests/checkbox_corpus.json`, `scripts/check-checkbox.js`, 420 px wide): a click on the box, a click on the text of
the label and Space each turn it and report the new state; **the focus survives the redraw** (Space twice in a row, no refocus
between); a disabled box reports nothing (replay answers `disabled` for that step); `error:` is invalid and described while
the box is off in this program, and valid and empty while it is on; the accessibility tree has a `checkbox` named by its label with
a `checked` property of `false` or `true`; the label is 388 by 44 px, so the touch target is the whole line, at least 44 by 44 px
(a bare checkbox is 13 px, and the baseline stylesheet's `min-height: 44px` for inputs would have made the box itself huge, so
`.cui-check input` opts out and the label is the target). The stylesheet is part of `build --pwa`; a page built without it has an
ordinary checkbox in a label.

**Replay** answers the handler's parameter from the trace (`"params": [true]`), checks `disabled:` as for any control, and does
not draw. A trace for a checkbox gives a `Bool`; giving it text is a mistake the language does not see (event parameters are untyped:
#103).

**Rule tags.** `checked-binding-invalid`, `checkbox-incomplete` (`spec/diagnostics/README.md`). **Deliberate breaks:**
`scripts/breaks/checkbox.json`, ten (not drawn from the `Bool`, the state inverted, not wired, the text outside the label, no label target in the stylesheet, not focusable, `on:` or `checked:` unchecked, any key accepted, replay not refusing a disabled box), each caught. **`examples/tickets`** has a "Hide finished orders" checkbox over the list
(`filter(orders, x => !hide_done || ..)`); `check-tickets.js` checks the rows come and go.

**Not built:** an indeterminate state, a group of radio buttons (`Select` is the next part), a switch.

## 10. `Select`, a choice of one string from a list (issue #76, fifth part)

> **Status: BUILT.** With this, every control and binding of #76 except `number` exists (section 11).

**Measured before.** `Select { label: "Status" ... }` was `error[component-kind-unknown]`. `examples/tickets` took the status of an
order in a text field and refused three words' worth of mistakes with a message after the fact.

**The form.** `Select { label: "Status" options: ["open", "doing", "done"] value: status on: Pick error: msg disabled: busy name: "status" }`.
`options:` is a list of string literals or the name of a `List` state field (`select-binding-invalid` otherwise, also for a list
holding anything but string literals); `value:` names the `String` state field that holds the choice; `on:` the event, whose
handler takes the chosen string. All three are required (`select-incomplete`). A Select takes only `label`, `role`, `on`,
`value`, `options`, `disabled`, `error`, `name`; any other key is `binding-unknown`, and `options:` on another element is too.
Only the string is shown and reported: there is no separate display text (the list is the values).

**What the page draws.** `<label class="cui-field">text <select aria-label=".."><option value="..">..</option> ...</select></label>`,
the options rebuilt from the list at every redraw (so an option added to the state's list appears), `.value = state` after them,
and `change` reports `e.target.value`.

**Measured** (`tests/select_corpus.json`, `scripts/check-select.js`):
* **A value that is not in the list selects nothing**: `selectedIndex` is -1 and the box is blank, not the first option. The initial
  state `""` shows an empty select, and the screen never says what the state does not.
* Choosing with the mouse API, and **ArrowUp on the focused closed select**, both report the option's string; the focus survives the
  redraw (ArrowUp twice in a row, no refocus), so a keyboard user can keep going.
* An option with spaces around it reports exactly itself; with two options of one string the handler gets that string and the
  redraw selects the first of them (the state holds a string, not a position).
* `error:` ties its message as in section 7; the accessibility tree has the select named by its label, with the chosen value.

**Replay.** The trace gives the string; replay reads the options from the state as they are when the step runs and **stops with
`trace-value-not-an-option`** when every control that fires the event is a Select and the string is none of their options (the
union): `"nope"`, `""`, `"Open"` for `open`, and `"blocked"` before the step that adds it, all in the corpus. A button that fires
the same event with an argument, or a text field, lifts the check, as for `maxlength` (section 6).

**Rule tags.** `select-incomplete`, `select-binding-invalid`, `trace-value-not-an-option` (`spec/diagnostics/README.md`,
`docs/replay.md`). **Deliberate breaks:** `scripts/breaks/select.json`, twelve, each caught (the first run missed one: replay reading the options of only the first of two Selects for one event; a test was added). **`examples/tickets`** chooses the status in a Select: three
words, no message, no way to send a status the service does not know; its invalid-status trace steps are gone (replay would stop on
them). The gallery's "Rich forms" tile, which probed `Select`, is removed: R5 reported its probe no longer fails.

**Not built:** option groups, a separate display text, multiple selection, a radio group.

## 11. Closing #76, and `number`

The done-when of #76 is a form with a password field, a checkbox, a select and a text area, with a validation message tied to a
field, that builds, replays a trace and is checked in Chromium (values, focus, `aria-invalid`, the message announced by role), each
refusal with a rule tag and the design with its measurements here. Sections 5 to 10 are those, one concern each, and
`examples/tickets` has all of them in one program. **`number` is not offered** (section 5: `1e5` typed key by key ends as `5`);
that is its own issue (#111), because making a number field safe is a change to how the page redraws a field being typed in.

## 12. Not done

`disabled:` on an expression (`disabled: len(q) == 0`: the language has no `&&`, `||` or `!` and no Bool expression binding,
issue #40), `aria-busy` for an in-flight request, a disabled state for the children of a component instance, the refusal
`accessibility-keyboard-missing` for a form with no keyboard path, and `submit:` on anything but a `TextField` (a `Button` already
fires on Enter and Space natively). From #76: `number`, date and the other input types (section 5). (`invalid:` and the message tied to its field are section 7.) From #91: `autocomplete`,
`maxlength`, `inputmode`, `name` (section 6, done).
