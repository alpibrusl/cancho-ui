# Controls: `disabled:` and the keys of a built-in element

> **Status: built (2026-10-21), issue #35.** Written from what `docs/ops-console.md` 2.3 measured; the checks are in
> section 4. Section 3 (Enter, issue #36) is added by the next change.

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
  of a double click finds the button already disabled. Measured in 3.1 below.
* **An unknown key is refused.** The built-in elements take `label`, `role`, `on`, `value`, `disabled` and `placeholder`
  (written as the HTML attribute of that name). Any other key is `binding-unknown`. Before, an unknown key was accepted and
  either dropped or written as a stray attribute: `Button { disabled: true }` compiled and did nothing, which is how a
  double click came to send two writes. Component instances keep their own rule (`property-unknown-key`).

## 2. Replay

Replay does not render the view, but it can answer one question about it: *would the browser have delivered this event?* A step
`{"event": "Apply"}` is refused with the outcome **`disabled`** when the view has at least one control with `on: Apply` and
**every** such control is disabled in the current state (`true`, or a `Bool` field that is `true`). The handler does not run
and nothing changes: no state, no request. An event that no control fires (a schedule's `Tick`, an event only a script calls)
is never refused, and an event with an enabled control among several is delivered. `expect: {"outcome": "disabled"}` asserts
it; a trace that fires a disabled control and expects the handler to have run fails at that step.

What replay does not model: which of two controls bound to the same event the user pressed (it only knows whether any
could have been), and the timing of the paint (the browser check in 3.1 covers that).

## 3. Checked

### 3.1 In the browser

`scripts/check-controls.js` (Chromium, a real server that holds the answer): a double click on `disabled: sending` sends **one**
write (the same page without the binding sent 2, `docs/ops-console.md` 2.3), the button is disabled while it is in flight, a forced
click on it sends nothing more, the answer re-enables it and a new click works, and a fast triple click sends one write, not
three; `disabled: true` is a disabled button whose event never fires; a `TextField` with `disabled: locked` takes no typing.

### 3.2 In the gate

`crates/cancho-ui/tests/controls.rs` (5 tests): what is emitted for a field, a literal and a `TextField`; the four ways to get
`disabled-not-bool`; `binding-unknown` for a stray key and for `disabled:` on a `Column`, with `placeholder` still legal;
a trace in which the second `Save` is `disabled` and the third, after the answer, is delivered; an event no control fires is
never disabled.

### 3.3 The checks can fail

Browser: the state-field form no longer emitted (killed by 7 checks, including the 2 writes), the literal form no longer
emitted (killed by 2). Gate: replay refusing an event no control fires (1), replay treating any `Bool` as disabled (1), `title`
allowed as a key (1), any state field allowed for `disabled` (1), `disabled` allowed on a `Column` (1).

## 4. Not done

`disabled:` on an expression (`disabled: len(q) == 0`: the language has no `&&`, `||` or `!` and no Bool expression binding,
issue #40), `aria-busy` for an in-flight request, and a disabled state for the children of a component instance.
