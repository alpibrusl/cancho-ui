# The look, round two: what was still different, and the small roles that closed part of it

> **Status: built 2026-10-29 (branch `ccr-look2`).** Round one ([`look.md`](look.md)) listed what was still different in its section 6. This note takes that list in the order of expected gain, decides each item, and measures it. *Written alongside the build, not before it*: the pixel diff was the design tool (each item was tried, looked at in Chromium next to the classic page, and kept only if the section's number moved the right way), and the two things the measurement refused are recorded in section 3.

The reference is the classic page (cancho-www `4288356`, now served at `/classic/`), compared by `scripts/pixel-diff-www.js` exactly as in round one (15 sections, 6 widths, light and dark; "differs" is the share of pixels that move by more than 24 of 255, section compared at its top-left corner).

## 1. What was added (each a declared role, the CSS in the compiler, none for a page that does not use it)

| # | role | where | refusal | what the compiler writes |
|---|---|---|---|---|
| 1 | a sticky `Header` has a **mark**, an **inline nav** and a **menu that swap**, and a chip that goes under 40 rem | no new key: an `Image` that is a direct child of a sticky header is its round 1.7 rem mark; a `Nav` that is a direct child is shown from 62 rem and a `Details` of the header is hidden from 62 rem; a `Chip` is hidden under 40 rem | none (a `Nav` and a `Details` are what they were) | rules in the sticky header's own `<style>`; the theme colours the nav links `muted`, takes the border of the chip off, and sets the header's buttons in the mono font |
| 2 | **`card: true` on `Column`, `Row` and `Link`** | a `Link` that is a card is a block with the card's padding, radius and edge | `card-invalid` (not `true`/`false`); `binding-unknown` elsewhere | `cui-card`; `a.cui-card { display: block; color: inherit; text-decoration: none }` |
| 3 | **`overlay: true` on a `Column`** | the first child is the picture; the second is laid over its top-start corner (a label), the third over its bottom (a panel) | `overlay-invalid` | `cui-overlay`: a one-cell grid; the theme gives the laid-over children the `surface` fill, the `fg` text (a checked pair) and the card shadow |
| 4 | **`overlay: "banner"`** on a `Column` | the first child is a picture that fills the block (`object-fit: cover`, 11 rem at least), the others are its text, in white, over it | `overlay-invalid` | `cui-overlay cui-overlay-banner`: a block that grows with its text, the picture absolutely placed under it |
| 5 | **`from: 40 \| 48 \| 60` on a `Grid`** | the cells stack when the **window** is narrower than that many rem, whatever the width of the column (a viewport rule, as the page's own `60rem`) | `grid-from-invalid` | `cui-from-60` and a `@media (max-width: 59.99rem)` rule for each value used on the page |
| 6 | the `scale: "display"` of `page` also sets inline `code` in a 0.38 em padded, 6 px rounded chip; the theme tints it `soft` | `page.md` | none | `:not(pre) > code { .. }` |

The rules of 2 to 5 are written only for a program that uses one (`uses_look_roles`), so every other page, every committed gallery demo and the `--pwa` build stay the bytes they were (the gallery test passes unchanged). The theme adds colours, shadows and a font family only: the allow-list of `theme.md` did not change.

## 2. Decisions

* **The human/agent switch is not added.** It is a script that rewrites the page's text between two renderings of the same content; a role that did that would be a second view of the program, not a style. The port says the same thing for agents in its own pane and in `agents.md`. Recorded in `parity-rules.json` (the page's `human agent` text is an intentional difference) and `dropped.md` 1. The star icon of the GitHub button and its small size (the button is the 44 px baseline link button in the mono font) are not added either.
* **The mark is not inside the brand link.** `Link` holds text and inline elements only (`rich-child-invalid`), and an `Image` there would need `Link` to take blocks. The mark is the header's first child, a sibling of the link: it is not clickable, the one measured difference (a 1.7 rem target the page has and the port does not; the brand text beside it is the link).
* **`from:` is a closed set** (40, 48, 60), not a number: a media query cannot read a custom property, so each value is a rule, and the set is the three breakpoints the page uses.
* **`overlay:` is on a `Column`**, the one element a program already uses to group a picture with text. A `Figure` with a `Caption` was not used: its caption is a `Caption`, which the page's quote is not.

## 3. Tried, measured, and not kept

* **`card: true` on the three `Column`s of `verifies`** (the evidence cards): the section went from 23.4 % to 39.1 % (412 light: 23.3 to 53.0), because the page's three are not cards at 1280 (they have a coloured top edge and no padding): the roles of 2 stay, the use in `home.cui` was removed. The section is as it was.
* **`List { card: true }` on the endpoints of `agents`** (round one) is still not used: with the banner the section is 758 px against the page's 742, and a card list in the half-width cell was measured taller again.

## 4. Measured

Round one is the committed `pixel.md` of `main` at `7964bda`; round two is this branch. Mean of 12 cells (6 widths x 2 schemes) per section; the 1280 and 412 columns are light only.

| section | round 1 (mean of 12) | round 2 | 1280 light | 412 light | height at 1280 light: page / port round 1 -> round 2 |
|---|---|---|---|---|---|
| header | 15.7 % | **9.2 %** | 6.8 -> 6.3 | 15.5 -> 11.1 | 64 / 64 -> 64 |
| top | 47.2 % | **41.8 %** | 37.1 -> 45 | 49.5 -> 43 | 511 / 640 -> 640 |
| try | 27.3 % | **27.5 %** | 36 -> 36.2 | 36.9 -> 37.8 | 1362 / 1491 -> 1491 |
| trust | 21.5 % | **21.8 %** | 15.4 -> 15.7 | 19 -> 19.1 | 1197 / 1124 -> 1124 |
| verifies | 23.4 % | **23.7 %** | 21 -> 21.5 | 25.9 -> 27 | 1130 / 1229 -> 1229 |
| authority | 21.0 % | **21.5 %** | 26.8 -> 27.2 | 32.6 -> 34.4 | 761 / 716 -> 716 |
| ecosystem | 30.2 % | **30.2 %** | 34.2 -> 34.2 | 39.1 -> 39 | 2169 / 1933 -> 1933 |
| proof | 34.7 % | **34.7 %** | 30.9 -> 30.9 | 43.1 -> 43.2 | 650 / 775 -> 775 |
| demo | 19.8 % | **19.8 %** | 14.9 -> 15 | 29.1 -> 29.2 | 703 / 687 -> 687 |
| not-here | 26.7 % | **26.6 %** | 31.9 -> 31.8 | 16.4 -> 16.2 | 309 / 429 -> 429 |
| agents | 60.7 % | **27.8 %** | 76.7 -> 20.4 | 54.8 -> 35.7 | 742 / 1497 -> 758 |
| next | 24.6 % | **21.8 %** | 19.3 -> 21 | 28.8 -> 31 | 361 / 310 -> 416 |
| status | 28.4 % | **28.1 %** | 28.7 -> 28.5 | 32.4 -> 32 | 255 / 207 -> 207 |
| name | 43.5 % | **43.6 %** | 44.4 -> 44.3 | 59.7 -> 59.8 | 650 / 682 -> 682 |
| footer | 21.4 % | **21.4 %** | 21.6 -> 21.6 | 15.6 -> 15.6 | 91 / 76 -> 76 |
| **all 15** | **29.7 %** | **26.6 %** | | | |

Honest reading:

* **Mean over the 15 sections: 29.7 % to 26.6 %.** Better by 3 points or more: `agents` 60.7 to 27.8 (the banner; at 1280 light 82.5 to 20.4, and the section is 758 px against the page's 742, it was 1,497), `header` 15.7 to 9.2 (the five that got worse in round one: this one is now better than it was before round one, 9.7), `top` 47.2 to 41.8, `next` 24.6 to 21.8.
* **Slightly worse, 0.5 points at most:** `try` 27.3 to 27.5, `trust` 21.5 to 21.8, `authority` 21.0 to 21.5, `verifies` 23.4 to 23.7. These are the pages' inline `code`: the chip tint moves a few pixels in every paragraph that has one. Not removed: the chip is in the page.
* **`top` is worse at 1280 light** (37.1 to 45.0) and better at 412 (49.5 to 43.0) and in the mean: the overlaid caption and panel put the hero's drawing where the page has it, and the section is 640 px against the page's 511, which the metric counts as different from there down.
* **Still the five that got worse in round one:** `header` is better than round one and than before it; `agents` is better than both; `not-here` (20.5 -> 26.6) and `proof` (32.7 -> 34.7) and `verifies` (22.6 -> 23.7) are as round one left them, and are in section 5.
* **Full page height at 1280:** the page 12,532 px, the port was 12,955 and is **12,323** (1.7 % shorter); at 360 it was 25,020 and is 24,960 (page 23,782); at 1024 14,152 to 12,809 (page 12,766).
* Looked at, 1280 and 412, `/` against `/classic/`: [`look2/top-1280.webp`](look2/top-1280.webp), [`look2/top-412.webp`](look2/top-412.webp), [`look2/agents-1280.webp`](look2/agents-1280.webp), [`look2/agents-412.webp`](look2/agents-412.webp), [`look2/next-1280.webp`](look2/next-1280.webp) (classic on the left or top, the port on the right or below).

## 5. Still different, and why

* **`not-here`** (429 px against 309): the page's wrapped strip of chips is a list that wraps inline; a `List` is a block list and a card list is a grid of cards. A third list role (`chips`) was not added: its one use is this section, and the section is 4 points of the mean at most.
* **`proof`** (775 against 650): the waffles keep their squares but the page's figure is a card with a smaller type; the `Details` of the data table is visible as a line.
* **`verifies`**: the page's three evidence cards have a coloured top edge, which no role gives (section 3).
* **`next`** (416 against 361): the links are cards of the page's radius, with more padding than the page's.
* **`top`**: the drawing is a plain block with the label and the panel laid over it, and the human/agent switch, the principles strip (`No ambient authority / Content-addressed / ..`, a list with separators) and the page's own proportions are not reproduced.
* **The Copy button** is the 44 px baseline button, not the page's small one: a 44 px target is the measured floor of `presentation.md`, and the small button was judged a regression in reach, not a look. Not changed.
* **The pixel metric** is position-sensitive: a section 129 px taller or shorter than the page's is "different" in every pixel below its first difference. Numbers here are the metric's, not a visual judgement; the screenshots are the other half.
* **Not measured:** Safari, Firefox (`:has()` for the chip rule needs Firefox 121 or later; without it the chip stays visible under 40 rem, which is round one's state), a real phone, forced colours, print.
