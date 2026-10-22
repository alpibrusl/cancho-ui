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

## 5. Not done

`disabled:` on an expression (`disabled: len(q) == 0`: the language has no `&&`, `||` or `!` and no Bool expression binding,
issue #40), `aria-busy` for an in-flight request, a disabled state for the children of a component instance, the refusal
`accessibility-keyboard-missing` for a form with no keyboard path, and `submit:` on anything but a `TextField` (a `Button` already
fires on Enter and Space natively).
