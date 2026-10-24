# A number field: `TextField { type: "number" }` (issue #111)

> **Status: BUILT.** Written 2026-10-24 ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1) from `scripts/measure-number.js` (Chromium of the Playwright install, `Pixel 7`
> profile, 412 px, on main at e0125f9); section 6 lists what is not measured. Anything not measured says **assumption**.

## 1. What exists

`type: "number"` was `input-type-invalid` (`controls.md` sections 5 and 11): typing `1e5` key by key into the browser's number input ended holding `5`. A text field with
`inputmode: "decimal"` and `num(text, default)` worked and gave the person what they typed. Since #82 the page knows the separators of ten languages from tables
(`i18n.md`), not from `Intl`.

## 2. Measured

`scripts/measure-number.js`: the compiler's own text field with only its `type` attribute changed after the build to `number` (so the redraw is the real one: **the whole
view is rebuilt after every state change**, and the new field gets `value = state`), typed key by key, against a page that never redraws. `!` = `validity.badInput`.

| typed | through the redraw: the field ends holding | the browser alone (no redraw) |
|---|---|---|
| `1e5` | `5` | `1e5` (after the keys: `1`, `!`, `1e5`) |
| `-5` | `5` | `-5` |
| `1.5` | **`15`** | `1.5` |
| `0.10` | `010` | `0.10` |
| `.5` | `5` | `.5` |
| `1e` | empty | empty, `badInput` |
| `1e400` | `400` | empty, `badInput` |
| `1,5` | `15` | `15` |
| `12abc` | `12` | `12` (the letters are not accepted) |

* **It is worse than the issue said.** The redraw loses `-`, `.` and `e` as soon as they are typed alone: the browser reports `1` for `1.`, the page writes `1` back, and the
  point is gone, so a decimal number cannot be typed at all (`1.5` ends `15`).
* **What the handler gets while the text is not a number:** the empty string, the same as for an empty field (`badInput` is the only difference and the page does not pass it on).
  What it gets once it is: whatever the browser keeps, which is **not** always something `num` reads: `.5` and `-.5` (`num(".5", 0)` is `0`: the grammar of `num` is JSON's), `007` and
  `0.10` as typed.
* **Locales.** In six contexts (`en-US es-ES de-DE fr-FR ar-EG hi-IN`) the browser's number input answered the same: `1,5` is `15` (the comma is dropped), `1.5` is `1.5`, `1.234,5`
  is `1.2345`, Arabic-Indic `٣` is empty, full-width `１２` is `12`. `navigator.language` and `(1234.5).toLocaleString()` did follow the context (`1.234,5` in `de-DE`), so the page's
  locale and the number input's rule are **two settings**. Headless Chromium has an English UI, so whether a Chrome with a Spanish UI accepts `1,5` is **not measured**
  (assumption: it does). Either way a person typing `1,5` into a field that answers `15` has lost data silently, and the program cannot tell.
* **Other ways the field changes:** ArrowUp on `5` gives `6`; `1.5` is `stepMismatch` (so `:invalid`) with the default `step`, valid with `step=any`.
* **The keyboard.** Headless Chromium has no soft keyboard; what is measurable is the attribute that chooses it: a `type=number` input has `inputMode` `""` (the browser's own default),
  a text input with `inputmode="decimal"` has `"decimal"`. Which keys Android and iOS then show (a digit pad with a decimal separator in the user's language) is **assumption**.
* **A text field with `inputmode: "decimal"` today** holds `1,5 abc` as typed and keeps the focus through the redraw; `num("1,5", 0)` is `0`. So the text field gave the person
  what they typed and gave the program nothing it could use in a language with a decimal comma.

## 3. The decision

**`type: "number"` is accepted, and it is not the browser's number input.** It is a text field (`type="text"`) with `inputmode="decimal"` (a program's own `inputmode:` wins, `"numeric"`
for whole numbers) whose event is given **the number typed, in the language of the component, as the text `num` reads**, or **the typed text itself when it is not a number**.

* **What is a number** (`parse_number` in replay, `__cui_number_parse` in the page, one corpus `tests/number_corpus.json` of 116 cases for both): an optional sign (the language's minus,
  `-`, U+2212 or `+`), digits (ASCII, Arabic-Indic, extended Arabic-Indic and full-width), at most one decimal separator, an optional exponent (`1e5`, `1E-3`), spaces around it
  ignored. **The decimal separator is the language's** (`.` in `en ar he ja hi`, `,` in `es de pt fr ru`), and a `.` is also read where it is not that language's grouping mark
  (so in `fr` and `ru` but not in `es`, `de`, `pt`). **A grouping mark is not read**: `1,234` in English and `1.234,5` in Spanish are text. At least one digit; `1.` is the number 1, `.5` is `0.5`.
  Zero, or a magnitude from 1e-6 up to 1e21, is a number; beyond that it is text (below 1e-6 and from 1e21 JavaScript prints in exponent form and replay does not imitate it).
* **What the handler is given:** `String(Number(...))`: `1e5` is `"100000"`, `007` is `"7"`, `0.10` is `"0.1"`, `.5` is `"0.5"`, `-0` is `"0"`, `1,5` in Spanish is `"1.5"`. Always something `num(text, default)` reads.
  Text that is not a number (`""`, `-`, `1e`, `12abc`, `1,234`) is given **as typed**, so `num(v, -1)` is the default for all of it and `v == ""` is "nothing typed". The program cannot tell
  `-` from `abc`, and does not need to: it is not a number.
* **What the person sees:** what they typed. The page remembers the text of the focused number field across the rebuild and gives it back when it reads as the same number as the state
  (`1.` stays `1.`, `1,50` stays `1,50`, `007` stays `007`); when the program changed the number (a clamp, a reset) the field shows the program's number. A field that is not being typed in
  shows the state in the language's separators (`2.5` as `2,5` in Spanish). The language is the component's (`requires Locale` and its catalogs, `i18n.md`); a component with no `Locale`
  reads English.
* **Why a text field and not the browser's input.** Section 2: through the redraw it cannot take a decimal; it answers the empty string for a half-typed number; it drops a decimal comma
  silently; its keys change the number (ArrowUp); it flags `1.5` invalid by default; and what it reports (`.5`) `num` does not read. A text field has none of these, and the person gets
  the decimal keyboard from `inputmode`. The costs: no spin buttons (not wanted on a phone), and the browser does not validate the field (the program does, with `num` and
  `error:`).
* **Rule tags:** `input-type-invalid` still refuses everything not in the list (a checkbox, a date, `Number`), and its message no longer says `number` is not one. **New for replay:**
  `trace-number-invalid`. No new compile-time tag: `inputmode:`, `maxlength:` and `autocomplete:` mean what they mean on a text field.

## 4. Replay

A trace gives the event the value the page would give it. For an event fired only by number fields, a parameter that **is** a number in another spelling (`1.50`, `.5`, `1e5`, `+1`, `007`, ` 1`, `-0`,
or `1,5` in Spanish) describes a page that does not exist and is `trace-number-invalid`, with the text the page gives. Text that is not a number (`1,5` in English, `-`, `abc`, `""`) is legal.
The language is the one in use when the component has `requires Locale`, English when it has not. An event also fired by a control that is not a number field is not checked.

## 5. Checked

* `crates/cancho-ui/tests/number.rs` (6 tests, in the gate): the corpus against `parse_number` for each of 10 languages and no language; the field is accepted with `inputmode:`, `maxlength:`
  and `autocomplete:`; the other types stay refused; a trace of numbers and of text; the language decides (`1,5` in a Spanish component); a text field is not checked as a number field.
* `scripts/check-number.js` (Chromium, `Pixel 7`, 4-minute watchdog): the same corpus against `__cui_number_text` extracted from a built page; **typed key by key through the redraw** `1e5`, `-`,
  `-5`, `1.`, `1.5`, `.5`, `-.5`, `0.10`, `007`, `1e`, `12abc`, `1,5`, `1 2` each ends in the field exactly as typed, with the focus kept, and the handler is given the number or the text;
  a digit typed in the middle goes in the middle with the caret after it; a paste; Spanish `1,5` stays `1,5` on the screen and gives `1.5`, `.5` is text there, English `1,5` is text, French `1,5` is 1.5;
  switching language redraws the number; the program's own `-2.5` is shown as `-2,5` in `es` and `de`; a clamping handler wins; the program clears the field; deleting everything is `""`; the field is
  a text field with `inputmode="decimal"` (or the program's own), named by its label, with no spin buttons (ArrowUp and the wheel do not change it); a page without a number field carries none of this.
* `scripts/check-input-types.js` keeps the measurement that the browser's own number input loses half-typed text.
* `scripts/breaks/number.json`: 14 breaks (the point read in Spanish, in the page and in replay; the event given the typed text; the typed text not given back; the field not marked; the state shown as it is; the keyboard always decimal; spaces, other digits and 1e21 read in the page; any spelling of a number accepted by replay; the text fields checked as number fields; every field read in English; `number` not offered), **all caught by `cargo test` or `check-number.js` themselves** (no kill, `HUNG` or `BUILD`).

## 6. Not measured: assumptions

* **The soft keyboard** on a phone (which keys, whether the decimal separator is the language's or the keyboard's): Playwright has none. `inputmode="decimal"` is what is promised, and it is documented browser behaviour, not
  measured here.
* **A browser whose own language is not English** and its native number input; Firefox and Safari; iOS.
* **Input method editors and voice input** (a field that is composed into): the focus adapter does not handle a composition in progress (`semantics.md`).
* **A screen reader's speech.** The field is a text field named by its label; it is no longer announced as a "spin button" (assumption: that is what a person who types a number wants; not measured with a reader).
* **Grouping marks** (`1,234`, `1.234,5`) are text, not numbers. A program that wants them removes the mark itself (`join(split(text, ","), "")`); reading them would make `1.234` mean 1234 in Spanish and
  1.234 in English, and the field cannot know which was meant.
* **Currency and percent** are the program's (`fmt.number`, `fmt.percent` show them; the field reads a plain number).
