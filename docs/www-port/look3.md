# The look, round three: the phone

> **Status: built 2026-10-29 (branch `ccr-look3`).** The user opened https://cancho.dev/ on a phone (412 px wide) and said it is "much better, still some things, for example the two pictures". Their screenshot is of the *Run the compiler* card: two dark code panels, the first with a large white **Copy** button laid over its text, and a header with a big **GitHub** button and the word **Menu**.

**What "the two pictures" are: this is not known.** The reading used here is the two dark code panels of that card (the commands and their output), because they are what the screenshot shows. The page also has pictures (the hero drawing, the three card drawings, the dark banner of *For agents*, the three photographs of *The name*); at 412 px they were looked at against `/classic/` and the hero and card drawings are as round two left them (the card drawings, which are iframes, are blank in a full-page screenshot taken before they have loaded and fine in a normal one). If "the two pictures" means two of those, it was not found. The question is open with the user.

## 1. What was wrong (measured at 412 px, DPR 2, against `/classic/`)

| | `/classic/` | the port, round 2 |
|---|---|---|
| Copy | a dark pill (`#1a212b`, 1 px `#2d3644`, mono 0.7 rem, 8 px radius), about 30 px tall, in the panel's top-right corner | the baseline white 44 px button (the theme's `surface` fill), drawn over the text the panel scrolls under |
| card title | `1.35rem`, letter spacing `-0.02em`, no margin | `1.05rem` with a margin: the title was small and the lead far below it |
| card text | `0.93rem`, muted, no margins | the body size, in the text colour, with the paragraph margins |
| header on a phone | the human/agent switch and a hamburger icon; the GitHub button is hidden under 30 rem | a GitHub button and the word *Menu* |

## 2. What was added

* **A `Code { copy: true }` button is a small dark pill with a 44 px target, and the text is kept clear of it.** The pill is the technique of the pill link (round one): a 7 px transparent border is the target and an inset shadow is the drawn edge, so the button is 44 px high and drawn 30 px. The panel's `pre` has 2.5 rem of room above its first line (it had 5.5 rem on its right, which the text scrolled under), so nothing is covered. Written only for a page that has a `Code { copy: true }` (`uses_code_copy`), so every other page, and every committed demo, is the bytes it was.
* **The header on a phone.** Under 30 rem the GitHub link of a header's `Row` is not shown (the menu has it, as the classic page's does) and the `Details` summary of a sticky header is three bars (a 44 px target, the word kept for assistive technology: its text is `font-size: 0`, not removed).
* **Display scale: a card's `Heading { level: 3 }` is 1.35 rem; a paragraph that is a direct child of a card or of a card's `Column` is 0.93 rem with no margins** (`:not([data-style])`: the card's label keeps its own style), and the theme sets it in `muted`, with `Strong` in `fg` (both are checked pairs on `surface`). A card's links are 0.88 rem.

## 3. Measured

Round two is the committed `pixel.md` of `main` at `b974c49`; round three is this branch. Mean of 12 cells (6 widths x 2 schemes) per section.

| section | round 2 (mean of 12) | round 3 | 412 light | height at 1280 light: page / port round 2 -> round 3 |
|---|---|---|---|---|
| header | 9.2 % | **7.8 %** | 11.1 -> 4.3 | 64 / 64 -> 64 |
| top | 41.8 % | **41.7 %** | 43 -> 42.9 | 511 / 640 -> 640 |
| try | 27.5 % | **15.7 %** | 37.8 -> 18.2 | 1362 / 1491 -> 1371 |
| trust | 21.8 % | **21.8 %** | 19.1 -> 19.1 | 1197 / 1124 -> 1124 |
| verifies | 23.7 % | **23.7 %** | 27 -> 26.9 | 1130 / 1229 -> 1229 |
| authority | 21.5 % | **21.4 %** | 34.4 -> 34.1 | 761 / 716 -> 716 |
| ecosystem | 30.2 % | **30.3 %** | 39 -> 38.9 | 2169 / 1933 -> 1911 |
| proof | 34.7 % | **22.3 %** | 43.2 -> 31.1 | 650 / 775 -> 677 |
| demo | 19.8 % | **19.6 %** | 29.2 -> 29 | 703 / 687 -> 687 |
| not-here | 26.6 % | **26.6 %** | 16.2 -> 16 | 309 / 429 -> 429 |
| agents | 27.8 % | **27.7 %** | 35.7 -> 35.7 | 742 / 758 -> 758 |
| next | 21.8 % | **21.7 %** | 31 -> 30.4 | 361 / 416 -> 416 |
| status | 28.1 % | **28.0 %** | 32 -> 31.4 | 255 / 207 -> 207 |
| name | 43.6 % | **43.7 %** | 59.8 -> 59.6 | 650 / 682 -> 682 |
| footer | 21.4 % | **21.5 %** | 15.6 -> 15.6 | 91 / 76 -> 76 |
| **all 15** | **26.6 %** | **24.9 %** | | |

* **Mean over the 15 sections: 26.6 % to 24.9 %.** `try` 27.5 to 15.7 (the card text, titles and the Copy pill), `proof` 34.7 to 22.3 and `header` 9.2 to 7.8 (at 412 light 11.1 to 4.3). No section is worse by more than 0.1 point.
* Full page height at 1280: 12,323 to 12,046 px (page 12,532); at 412 22,272 against 22,604 (it was 23,341).
* Before and after, on a 412 px screen at DPR 2, `/classic/` | round two | round three: [`look3/phone-run-the-compiler-light.webp`](look3/phone-run-the-compiler-light.webp), [`look3/phone-run-the-compiler-dark.webp`](look3/phone-run-the-compiler-dark.webp), [`look3/phone-header-light.webp`](look3/phone-header-light.webp), [`look3/phone-header-dark.webp`](look3/phone-header-dark.webp).

## 4. Still different

* **The human/agent switch** is not in the port (round two, section 2), so the phone header is the brand, a hamburger and nothing between.
* **The panel's top strip.** The pill is above the first line, where the page draws it over the line's end: each copy panel is 1.6 rem taller than the classic one. A panel with no `copy:` is as it was.
* **The pseudo prompt, scrollbar and padding** of the two panels are the same as the classic page's (padding `0.9rem 1rem`, 12 px radius, `0.78rem` mono, the page's dark colours); the scrollbar is the browser's.
* **Safari, Firefox and a real phone's own text size** are not measured. The user's screenshot shows the text larger than the same page at 412 px in Chromium (a phone's text-size setting scales rem), so the layout under a larger text size is the thing to look at next.
