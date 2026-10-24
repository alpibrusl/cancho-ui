# `theme { .. }`: a program's own colours and fonts, with contrast the compiler checks

> **Status: built (step 1 of [`www.md`](www.md), section 7).** Designed in `www.md` section 5.1 before the code; the user decided on 2026-10-24 that a theme carries **free
> colours with a compiler-checked contrast**, not closed enums, and that fonts are **self-hosted files declared in the program with a licence and a SHA-256**, never remote
> ([`www.md`](www.md) section 10). Measured in Chromium (Playwright, `Pixel 7` profile) and in `cargo test`; numbers say which. #116 is the issue this answers for programs that want a
> look of their own; it asked for the contrast to be checked, and this is the check.

## 1. What a program can say

```
theme {
    light { bg: "#f7f4eb"; fg: "#121417"; muted: "#4f545b"; line: "#e2ddd0"; surface: "#ffffff"; soft: "#efece1"; ink: "#16283b";
            accent: "#185fa5"; accent_fg: "#ffffff"; accent_soft: "#ebf3fc"; good: "#0f7b5f"; warn: "#8a4b0a"; warn_bg: "#f9efd9";
            warn_line: "#d9a441"; bad: "#b91c1c"; bad_bg: "#fdeceb"; }
    dark  { .. the same sixteen tokens .. }
    font body { family: "JetBrains Mono"; file: "fonts/jetbrains-mono-latin-wght-normal.woff2"; sha256: ".."; licence: "OFL-1.1"; weight: "100 800"; }
}
```

* `theme` is a keyword only at the top of a file, followed by `{`; **one per program**, in the entry file (`theme-duplicate`).
* A colour is `"#rrggbb"`, six hex digits, either case, and nothing else: no name, no three digits, no alpha, no function (`theme-colour-invalid`). The value is therefore never CSS the
  author wrote: a value such as `"#fff;}body{display:none"` is a refusal, not a rule (a case of the corpus).
* **Both schemes, all sixteen tokens.** `light` and `dark` are both required and each holds every token (`theme-scheme-missing`): the compiler writes the dark mode from the `dark` block, so there
  is nothing to fall back on and nothing to guess. A token is written once (`theme-token-unknown`). **A token written with a dash is two tokens and a minus**: the names have underscores
  (`accent_fg`), and `accent-fg` is a `parse-unexpected-token`.
* Fonts: roles `body`, `mono`, `hand`; keys `family`, `file`, `sha256`, `licence`, optional `weight`. Section 5.

## 2. The tokens, and what the baseline does with them

| token | what the compiler's stylesheet uses it for |
|---|---|
| `bg`, `fg` | the page's background and text |
| `surface` | the background of buttons, fields, text areas and selects |
| `muted` | the border colour of those controls (a control's edge needs 3:1 against its surroundings, WCAG 1.4.11; `muted` has 4.5:1 against `bg` and `surface` by the checks below) |
| `accent` | links, the focus ring, and the colour of a `Chart` series |
| `accent_fg`, `accent_soft` | not used by the baseline yet: exported as custom properties for a host page, checked against `accent` |
| `good`, `warn`, `warn_bg`, `bad`, `bad_bg` | the colour (and for `warn` and `error`, the background) of a `Text { tone: .. }` |
| `line` | the lines under the cells of a `Table` |
| `soft`, `ink`, `warn_line` | **decoration**: written as custom properties, used by nothing the baseline draws, and in no checked pair except `soft` as the background of `muted` |

Every token is written to the page as a custom property, `--cui-bg`, `--cui-accent-fg`, ..., so a host page or an island ([`www.md`](www.md) section 6) can read the colours. The
**22 colours of the cancho-www page's art** (granite, ridges, sky, the map's paper) are not tokens: a drawing carries its own palette.

## 3. The rules

| rule tag | refused |
|---|---|
| `theme-token-unknown` | a scheme other than `light`/`dark`; a token not among the sixteen; a font role other than `body`/`mono`/`hand`; a font key other than the five; any of these written twice |
| `theme-colour-invalid` | a value that is not `"#rrggbb"` |
| `theme-scheme-missing` | no `light` or no `dark`; or a scheme that lacks a token (the message lists them) |
| `theme-contrast` | a checked pair under 4.5:1 in either scheme; the message names **every** failing pair as `scheme: text on background is R:1` |
| `theme-duplicate` | two themes in one file, or one in the entry file and one in an imported module |
| `font-remote` | a `file:` that is an address (`http`, `https`, `//host`, `data:`, anything with `://`) |
| `font-licence-missing` | no `licence:`, or one that is not an identifier |
| `font-hash-mismatch` | `sha256:` not 64 hex digits, or not the hash of the file (the message gives the file's) |
| `font-file-missing` | the file cannot be read |
| `font-incomplete` | no `family`, `file` or `sha256`; an unusable family, file or weight |

`font-file-missing`, `font-incomplete` and `theme-duplicate` are **additions to the list the user gave**: the requirements that the compiler checks the file exists, that the
family and path cannot carry CSS, and that a theme is one per program each needed a tag, and a refusal without one is not allowed. All ten are in
[`spec/diagnostics/README.md`](../spec/diagnostics/README.md), each reached by a test (`crates/cancho-ui/tests/theme.rs`).

## 4. The contrast check

**Thirteen pairs per scheme**, text on background, each at least **4.5:1** (WCAG 2.x 1.4.3): `fg` on `bg` and on `surface`; `muted` on `bg`, `surface` and `soft`; `accent` on `bg`,
`surface` and `accent_soft`; `accent_fg` on `accent`; `good` on `bg`; `warn` on `warn_bg`; `bad` on `bad_bg` and on `bg`. (`www.md` 2.4 said "15 pairs"; that counted two pairs of the
site's art palette, `term-fg` on `term-bg` and `map-ink` on `map-paper`, which are not tokens. Corrected there.) The ratio is the WCAG relative-luminance formula; it is **cut, not
rounded**, to two decimals when shown, so a colour that reaches 4.4776 is shown as 4.47 and never as the limit it did not reach. A pair is never rounded up to pass: the comparison is on
the unrounded number.

**Measured on the palette of the cancho-www home page** (commit `0579a80`, vendored in `tests/theme_corpus.json`): all 26 pairs (13 x 2) pass, the lowest being light `good` on
`bg`, 4.75:1. **Independently computed:** `scripts/theme-oracle.js` (Node, its own implementation: the 0.03928 knee and the 0.2126, 0.7152, 0.0722 weights as WCAG writes them; the compiler
uses the 0.04045 knee, and no 8-bit channel lies between the two, so they agree) writes the ratios and, for each failing case, the failing pairs into the corpus; `cargo test` compares
the compiler's arithmetic with those to 0.001 and the refusal messages pair for pair. The boundary is in the corpus: `#767676` on white (4.54:1) passes, `#777777` (4.4776:1, shown as
4.47) is refused. **Moving `muted` of the light scheme to `#9a9a9a` is `theme-contrast`**: `light: muted on bg is 2.55:1; muted on surface is 2.81:1; muted on soft is 2.37:1`.

What a pair does *not* promise: the contrast of a colour on a background it was not paired with (`ink`, `line`, `warn_line` have none), or of text over an image.

## 5. Fonts

A font is **a file of the program**, in the directory of the entry file, declared with its SHA-256 and its licence:

* the compiler (in `check` as well as `build`, because it reads the file) checks that the file exists and that its SHA-256 is the declared one. The hash is computed by the compiler's own SHA-256
  (no dependency was approved for one; it is checked against the published vectors for the empty string, `abc`, the 448-bit message, 55, 56, 64 and a million bytes, those last four also by the
  system's `sha256sum`). A file that changes by one byte is `font-hash-mismatch` and the message gives the hash it now has.
* `build --page` and `build --pwa` **copy the file** into the output at the path written, and write
  `@font-face { font-family: ".."; font-style: normal; font-display: swap; [font-weight: ..;] src: url("<path>") format("woff2"); }` with the **relative** address. `--pwa` precaches it.
  `font-display: swap` is not a choice.
* **No request to another host.** Measured in Chromium (`scripts/check-theme.js`, both colour schemes): every request of the themed page is counted; none left `localhost`; the server saw
  `/`, `app.js` and the font, and nothing else; the font response was 200 and its bytes equal the file in the program's directory.
* The roles: `body` becomes the page's `font-family` (`--cui-font`), `mono` that of `code, pre, kbd, samp` (`--cui-mono`), `hand` only the custom property `--cui-hand`.
* Only `.woff2`; the path is relative, without `..`, of letters, digits, `.`, `_`, `-`, `/`. A family of letters, digits, spaces, `-`, `_`: no quote or brace can reach the stylesheet (a case of the corpus).
* The example's font, JetBrains Mono (40,404 bytes, SIL OFL 1.1, from the cancho-www page's `assets/fonts/`), is vendored in `examples/theme/fonts/` with its licence text, which the OFL asks to
  travel with the font.

## 6. What the stylesheet is, and what no token can do

A themed page has the baseline stylesheet, unchanged, and **after it** a second `<style id="cui-theme">`. A program with no theme has no second block and its page is the same bytes as before
(`a_program_without_a_theme_is_emitted_as_it_was`; and a program's script is the same bytes with and without a theme, so replay and the emitted module never read it).

The theme block sets: the custom properties; `@font-face`; `background`, `color` on the body; `color` on links; `background`, `color`, `border-color` on controls; the border colour of table
cells; the tone colours; the font family; and **`:focus-visible { outline: 3px solid var(--cui-accent); outline-offset: 2px; }`**. It contains no rule about size, position, display, visibility
or the outline's width or style. So:

* **The 44 px targets** (`button, input, textarea { min-height: 44px; min-width: 44px; .. }`) are the baseline's, and the theme block comes after it and does not mention them.
* **The focus ring** is the compiler's: 3 px, solid, in the accent. A token can change its *colour*, and the accent has 4.5:1 against the page by the checks above; nothing can remove it.
* Proved, not asserted, two ways. `no_token_can_remove_the_44_px_targets_or_the_focus_ring` (Rust): every one of the 16 tokens of both schemes at four extreme values (`#000000`, `#ffffff`,
  `#ff00ff`, `#808080`) = 128 programs; 71 are refused, all by `theme-contrast`; the 57 that compile each have the baseline rule once, a theme block whose properties are all in a short allow list
  and none of `min-height`, `min-width`, `outline-width`, `outline-style`, `outline: none`, and the exact ring rule; each of the 16 tokens built at least once. `scripts/check-theme.js` runs the same
  128 in Chromium: for the 57 that compile, every `button` and `input` is at least 44 x 44 CSS px and a keyboard-focused control has `outline: solid 3px` in the accent colour.
* **Dark mode:** the compiler writes `@media (prefers-color-scheme: dark) { :root { .. } }` from the `dark` block. There is **no `data-theme` switch**: no need for one was measured (the page
  follows the system, which is what the cancho-www page does in practice: its manual `data-theme="light"` hook is set by nothing in its HTML or JavaScript, counted in `www.md` 2.4). Measured:
  both schemes drawn in Chromium with `colorScheme` emulated; the body, a button, a field and the three tones are the colours of the scheme the browser is in.

## 7. `diff`, `authority`, replay

* **`diff` reports a theme change as its own line**: `~ theme light.accent: #185fa5 -> #0f3d6e`, `+ theme dark.bg = #0b1016` (a theme added), `- theme ..` (removed), `~ theme font.body.sha256: ..`.
  In JSON, a `theme` array of `{ setting, base, candidate }` that is **absent when the themes are equal**, so no existing report changes. A theme change makes `changed` true (exit 5), counts in
  `change_count`, and does **not** touch `authority`: `authority: unchanged`.
* **`authority` is unchanged**: a theme has no authority and `requires` does not mention it. `inspect effects` prints the same bytes with and without a theme (a test).
* **Replay is unaffected, and this is shown**: `cancho-ui test examples/theme/theme.cui examples/theme/theme.trace.json` passes (3 steps, 7 checks); the IR of the components is the same with and
  without a theme apart from where they sit in the file (a test compares them with spans removed); and the whole existing corpus passes unchanged (the gate, below). Replay does not draw, and
  reads no part of a theme.
* `inspect ir` has the theme under `theme` (and has no such key for a program without one).

## 8. Checked

* `cargo test` (`crates/cancho-ui/tests/theme.rs`, 13 tests; the unit tests of `cancho_ui_ir::theme` and of the SHA-256): the palette passes and its ratios are the oracle's; every case of
  `tests/theme_corpus.json` (22) is refused by its tag with the oracle's pairs, or accepted; the boundary and the cut; `theme-duplicate` in one file and across an import; 16 font cases and two keys;
  the tampered font; the themed page, its font copied and no address; no theme, same bytes; no authority and replay; `diff`; every token cannot remove the targets or the ring; a malformed theme is a
  parse refusal.
* `node scripts/theme-oracle.js`: the corpus has the numbers an independent implementation computes. `node scripts/check-theme.js` (5-minute watchdog): section 6, and no request to another host.
* `scripts/breaks/theme.json`: **24 deliberate breaks, each caught by a check failing on its own** (none `HUNG`, none `BUILD`): the limit 4.0, a wrong luminance weight, a wrong knee, a pair swapped,
  the dark scheme left unchecked, a ratio rounded and not cut, a missing token unnoticed, an alpha accepted, an address not remote, a quote allowed in a family, no licence needed, a second theme
  unnoticed, the font hash not compared, a wrong SHA-256 constant, the dark block repeating the light, a height set by the theme, the focus ring removed, the focus ring in the text colour, the font
  not swapped, a remote `@import`, the font not copied into a page, not precached by an app, a theme change not counted by `diff`, a theme change that widens authority. The first run left one uncaught
  (the quote case's family had other forbidden characters too, so allowing the quote changed nothing); a family that is only a quote in the middle was added and the break re-run.

## 9. Not measured, not done

* **Not measured:** Safari and Firefox; a real phone; the *look* of a themed page (no pixel comparison with the cancho-www page was made: the colours are checked, the design is not); how a screen
  reader reads the tone prefix and the focus ring; forced-colours mode (`forced-colors: active` replaces author colours, so a theme is unused there by the browser's design, not measured here);
  a font that fails to load (the page falls back to the system stack, by `font-family` order, not measured).
* **Not done, on purpose:** free CSS (a `theme.css`); a `data-theme` switch; `line`, `ink`, `soft`, `warn_line` in any rule that sets text; the art tokens of the cancho-www page; a theme in an
  imported module (refused when the entry file has one too, `theme-duplicate`, tested; if only the module has one the code uses it with font paths relative to the entry file, **not tested**); more than two schemes; a high-contrast
  scheme. A **non-woff2** font is refused.
* **Proposed, not done:** the gallery could be given a theme so its 13 demos look like the landing page. It is not in this PR: it changes 13 generated pages and the mobile and overflow checks that
  measure them, and it earns its place only when a theme is wanted for the gallery itself.
