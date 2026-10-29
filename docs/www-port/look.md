# The look of the port: the roles that carry it (design note, before the code)

> **Status: designed 2026-10-29, measured before any code; built in the same branch (`ccr-look`), and each number below that the build changes is corrected in place at the end (section 6).** This answers
> item 1 of "What would be needed" in [`status.md`](status.md) and the work list in [`pixel-rules.json`](pixel-rules.json). The user's request was "match the styles". It is **not** a request to copy the page's
> CSS into the program: the page has 106 classes and 19 viewport rules, and [`presentation.md`](../presentation.md) decides that a program says what a thing *is* and the stylesheet says how it looks. So the
> question is the smallest set of declared, checked roles that closes the most pixels, with the CSS in the compiler.

## 1. Measured first (Chromium 141, Playwright, reduced motion; the live page at cancho-www `8128105`, whose `index.html` and `assets/` are those of `4288356`; the port as built from `main` at `17f0cf1`)

Computed styles at 1280 px, then what the screenshots (1280 and 412, light and dark, top 2,400 px, read by eye) show:

| | the page | the port | what it is |
|---|---|---|---|
| content column | `main` 1152 px wide with 20 px padding: content 1112 px from x = 84 | x = 16, 1248 px (the window less a 16 px gutter) | **a centred measure** |
| hero | two columns from 60 rem (text left, a 28 px card with the drawing right), stacked below | drawing under the text at every width, 1,170 px tall against 511 | **a two-column row that stacks** (the compiler's `Grid` already does this) |
| the three principles, the three steps | cards: `var(--surface)`, radius 16 px, `var(--shadow)` + 1 px `--ring`, padding 20.8 / 22.4 / 24 px | no fill, radius 0, no shadow, padding 0 | **card chrome** |
| the steps' text and code | aligned to the centre of the card | the code box stretched to the height of the text column (an empty dark area under 7 lines) | **`Grid` cross-axis alignment** |
| h1 | 48.8 px, weight 800, line-height 1.02, letter-spacing -0.035 em, the second line (`em`) blue and upright | 32 px, 700, 1.5, italic, the text colour | **display type** |
| h2 | 32 px, 700, line-height 1.18, -0.025 em | 24 px | display type |
| section spacing | `section { padding-top: 5rem }` (80 px) | 0 (the sections touch: the code of step 1 touches the label of step 2) | **rhythm** |
| eyebrow, label | the mono font, 12 px, 0.12 em | the body font, 12 px, 0.1 em | the theme's `mono` |
| lead | 18 px, line 1.6, 34 rem | 17.6 px, line 1.5, 40 rem | rhythm |
| body line height | 1.6 | 1.5 | rhythm |
| header | sticky, `bg` at 86 % with a 10 px blur, a 1 px ring and a soft shadow; the brand in bold mono with the logo, a **chip** "Pre-1.0 · alpha", eight inline links from 62 rem, the human/agent switch, a GitHub button; under 62 rem the brand, the switch and a menu | a solid `bg`, a 1 px line; the brand and GitHub as underlined links, "Pre-1.0 · alpha" as plain text, "Menu" | **a header arrangement** |
| the pill above the h1 | a `warn` pill with a dot | an underlined link | an appearance of a `Link` |

What dominates, in the order a visitor sees it: (1) the cards and the shadows (the page's whole middle is white rounded boxes on the cream), (2) the narrow centred column against a full-width one, (3) the hero beside its text, (4) the heading sizes and the section
gaps (the page is about 1.7 times as airy between sections), (5) the header. The colours, fonts and buttons were already the page's.

## 2. What the language already had, and is enough (no new surface)

* **A two-column row that stacks**: `Grid { min: 26 .. }` is "as many columns as fit at `min` rem, equal" and one column when two do not fit (`structure.md` section 10). The hero is `Grid { min: 26 Column { text } Hero { } }`: two columns from about 54 rem
  of column, one under it. The page's 60 rem is a media query on the window; this is a rule on the column, so the two switch at slightly different widths (said in section 6). **Not new.**
* **A header with a chip**: `Chip { tone: "warn" }` is the page's badge, and `Header { sticky: true }` the sticky bar. **Not new.**
* **Unequal columns**: `Grid { weights: [1, 1.75] }` is the steps' 1 : 1.75 columns. **Not new.**

## 3. What is added (five small things; each has a rule tag, a test and a line in the document that owns it)

| # | what | where it is written | the CSS it is | refused |
|---|---|---|---|---|
| 1 | **`card: true`** on a `Section`, `Article`, `Aside` or `Figure` | the program | class `cui-card`. Baseline: padding 1.1 / 1.2 / 1.3 rem, radius 16 px, a 1 px edge. Theme: the `surface` fill and `--cui-shadow` (the one the link buttons use; a 1 px `line` ring in the dark scheme) | `card-invalid` (not `true` or `false`); `binding-unknown` on any other element |
| 2 | **`align: "center"`** (or `"start"`) on a `Grid` | the program | `align-items` on the grid (`cui-align-center`). Default unchanged (stretch) | `grid-align-invalid` |
| 3 | **`page { measure: "72"; }`**: the page's column, in rem (30 to 100) | the page declaration | the `header`, `main` and `footer` of the page are centred in a column that wide with a 20 px gutter; the sticky header stays full width with its content in the column | `page-measure-invalid` |
| 4 | **`page { scale: "display"; }`**: the display type and rhythm | the page declaration | h1 `clamp(2.4rem, 6.4vw, 3.5rem)` at 800 and -0.035 em, h2 `clamp(1.6rem, 3.4vw, 2rem)`, h3 1.05 rem, the `Em` of a heading upright and in the accent, line height 1.6, `main > section` 5 rem above it, the eyebrow letter-spacing 0.12 em, the lead at 1.125 rem and 34 rem. **The theme's `mono` font is the eyebrow's and label's.** | `page-scale-invalid` (the only value is `"display"`) |
| 5 | **A header's `Row` is its end group**, and **`appearance: "pill"`** on a `Link` | the program | in a sticky `Header` a `Row` child has `margin-inline-start: auto` (brand and chip at the start, GitHub and the menu at the end); a pill is a small rounded `warn` link. Theme: the header is `bg` at 86 % with `backdrop-filter: blur(10px)` and the page's shadow | `link-appearance-invalid` (the set grows by one) |

Why these and not others, each by the number it closes:

* **Cards (1) are the biggest** single difference of the page and have no stand-in in the language. They are a boolean on the elements the page puts them on (`Article` for the three principles and the three steps, `Aside` for the status, `Figure`
  for the pictures), not a new element: `Article` is already the page's "a self-contained piece", so a card is how an `Article` looks, which the program says once.
* **The centred measure (3) is a page-level fact**, so it belongs where the page's other facts are (`page { }`), not as a width on each section (`Column { max_width }` counts characters and would not centre a full-bleed header).
* **The scale (4) is one switch, not sixteen sizes.** The alternative, `Heading { size: "display" }` and `Section { space: .. }`, is more surface and invites a size on every element; a page is either set in the display scale or it is not.
  A **free size or a free colour is not offered anywhere**: that is `theme.css`, which `theme.md` section 9 refuses on purpose.
* **Alignment (2) is a key of `Grid`**, the one element whose cross axis a program cannot otherwise say; the page uses it for the hero and for each step.

Considered and **not** done, for the record (each is a decision the user can overturn):

* **The inline links of the header from 62 rem.** The page has two lists of links and shows one by width. A `Details { summary: "Menu" }` that is open and has no summary at a width needs `::details-content` (Chromium 131 and later; Firefox and Safari not
  measured) or a script. `dropped.md` 3 stays: the menu is the only navigation. **The human/agent switch** (`dropped.md` 1) and **the logo in the link** (`dropped.md` 2) likewise.
* **A card with no padding** (the page's hero card holds a picture to its edge): the hero drawing stays in a plain block; a `flush` would be a sixth thing for one use.
* **A blurred header is not a checked pair.** Text on a translucent `bg` is checked as `fg` on `bg` (the contrast check of `theme.md` section 4); what shows through the 14 % is not, and is said so in `theme.md` section 6.
* **Hover lift of the cards, the zoom of the pictures** (`dropped.md` 9).

## 4. What stays the compiler's

The theme block stays a list of colours, fonts, shadows and the translucent header; it gains **no** rule about size, position or display. `card`, `measure`, `scale`, the grid alignment and the header's end group are **baseline**
rules (`pwa.rs`), written once, and written only when the program uses them (a `.cui-card` rule is a rule about a class no other program draws; the `measure` and `scale` rules are written only for a page that declares them). So the 44 px targets, the focus ring
and the contrast check are untouched, `a_program_without_a_theme_is_emitted_as_it_was` holds, and the property allow-list of `no_token_can_remove_the_44_px_targets_or_the_focus_ring` is extended by exactly the properties above
(`background`, `box-shadow`, `backdrop-filter`, `border-color` on a card), each named in the test.

## 5. How it is checked

`cargo test`: each of the five accepted and refused with its tag; the class and the attribute in the DOM recipe (`structure_corpus.json`, so the JavaScript emitter and the prerenderer cannot disagree); the baseline rule and the theme rule present
and absent; a program using none of it emits the same bytes as before; the diff reports a changed `measure` or `scale`. In Chromium (`scripts/check-look.js`): the column is 1152 px wide and 64 px from the left at 1280 and the gutter is 20 px at 412 with no
horizontal scroll; a card has the computed radius, fill and shadow; the hero is two columns at 1280 and one at 412; the header is sticky with a blur; every control is still 44 px and the focus ring is still 3 px. Then `scripts/pixel-diff-www.js` again.

## 6. Result

(Filled in at the end of the work, in place: the pixel numbers before and after, per section, and what is still different.)
