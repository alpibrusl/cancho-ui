# Motion: `Reveal`, `CountUp` and `visible:`, safe by construction

> **Status: BUILT (2026-10-26), with its measurements (section 5); section 7 says what the build changed in this note.** Step 5 of [`www.md`](www.md) (5.4,
> "scroll, viewport, motion"). Measured against the real cancho-www page (`index.html`, `assets/site.js`, `assets/site.css`, commit `0579a80`, read-only) in Chromium
> (Playwright, desktop Chromium on Linux; the `Pixel 7` profile is a viewport and a user agent, not a phone): `scripts/measure-motion.js` for the page, `scripts/check-motion.js`
> for what the compiler writes.

## 1. What the real page does, measured

| | the page | measured by |
|---|---|---|
| reveal | **14** elements with `class="reveal"`; `.js .reveal { opacity: 0; transform: translateY(18px) }` and `.in` shows it; one observer, threshold `0.25` | `index.html`, `site.css`, `site.js` |
| thresholds | **three**: `0.25` (reveal), `0.4` (the proof cards, which also start their count-up), `0.5` (the authority drawing starts to play) | `site.js` |
| count-up | three `<b data-count>`: the HTML says `<span>0</span> / 704`, `0 / 333`, `0 named rules`; the script puts the real number in from the data file and tweens it over 1.3 s | `index.html`, `site.js` |
| with JavaScript off | the 14 blocks are opaque (the rule needs `.js`, which a script sets); the three count-ups **read `0 / 704`, `0 / 333`, `0 named rules`**: the page's own text is wrong without script | Chromium, script off |
| with the page's script blocked but the inline head script run | `<html class="js">` is set by an inline script in `<head>`, and `site.js` is what shows a block: with `site.js` aborted, **14 of 14 blocks have `opacity: 0` and nothing will ever show them** | Chromium, `site.js` aborted |
| reduced motion | `@media (prefers-reduced-motion: reduce)` sets `animation: none !important`, `transition-duration: .01ms !important`, `.js .reveal { opacity: 1 }`, and `site.js` writes a count-up's number at once. Measured: **32 animations running** 1.5 s after load, **0** with `reducedMotion: 'reduce'`; 14 (Pixel 7) and 13 (desktop) blocks not opaque before scrolling, **0** under reduce | Chromium |
| every block is seen | scrolling the whole page leaves **0** `.reveal` without `.in` on the Pixel 7 profile and at 1280 px: no block on this page is taller than four windows, so the observer's fraction is reachable. (A block more than four windows tall can never have a quarter of **itself** inside the window; section 3 builds for it, section 5 measures it) | Chromium |
| layout shift | CLS over a scroll of the whole page: 0 (Pixel 7) and 0.0008 (desktop) with motion; **0.0041 and 0.0046 under reduced motion** | `layout-shift` entries |
| a screen reader and a count-up | half a second into the tween the DOM text is `0 / 704` and the accessibility tree is `"0"`, `" "`, `"/ 704"`; the tween writes each frame into the text, so what is read depends on when | Chromium accessibility tree |

So the page's handling of reduced motion works (it was written by hand and the measurement agrees: `www.md` 5.4 said so), and three things it can get wrong are measured: a script
that fails after `js` was set hides the page for good; a count-up that is `0` in the HTML is read as `0`; a count-up is read as a frame while it runs.

## 2. What `www.md` 5.4 said, and what the page disagrees with

| 5.4 said | measured | decision |
|---|---|---|
| `Reveal` "gets `data-in` once, when 25 % of it has been seen"; "the page uses 0.25, 0.4, 0.5 in three places" | true, and the three have three meanings: show a block, start a count, start a drawing | `Reveal` is 25 % and has no key; `CountUp` starts at 40 %; `visible:` is 50 %. None can be set by the author |
| "with no `IntersectionObserver`, or under `prefers-reduced-motion`, it is shown immediately" | the page does the first (`site.js` adds `.in` to every block) and the second in CSS | both, and a third: **a block is hidden only while the page's script has armed it** (section 3), so a script that never runs, or throws, hides nothing |
| `CountUp { value: n }` on a number `Text`; "under reduced motion, or with no JS, the final number is written at once. The final value is always in the DOM text" | the page's HTML says `0`; its tween writes frames into the text | `CountUp { to: n }`, an **inline** element (like `Strong`) inside a `Rich`, a `Heading` ..: the text of the element **is** the final number in the HTML, and the frames are drawn over it by CSS with an empty alternative text. The tag is `countup-not-number` (as planned) plus `countup-out-of-range` and `countup-has-children` |
| `on Visible` / `on Hidden` on a `Section`; "the replay trace carries the event `{"event":"Visible"}`" | a handler named `Visible` would need a new event kind in replay for nothing | **`visible: Seen` and `hidden: Gone`** are bindings that name the program's own events (like `submit:`), on `Section`, `Article`, `Aside` and `Figure`; replay needs nothing new, its trace carries `{"event":"Seen"}` like any event |
| `visible-threshold-invalid` | there is no threshold to write | **dropped**: nothing to refuse |
| "`Reveal`/`CountUp`/`on Visible` reach nothing outside the page" | true | no capability, no new network access |

## 3. How each is made safe

* **Reveal** is `<div class="cui-reveal">` in the page's HTML (static: its text is prerendered, `prerender.md`). The page's inline script, which runs before `app.js` is fetched, adds `cui-armed` to the blocks **that are in the HTML now**, and only when the browser has `IntersectionObserver` and the visitor has not asked for reduced motion. The stylesheet hides `.cui-reveal.cui-armed:not(.cui-in)` and nothing else. So: no script, a script that fails, a browser without the API, a block that script builds later (it is never armed): all show the block. Reduced motion and print are also in the stylesheet (`@media (prefers-reduced-motion: reduce), print`), so an armed block is shown even if a script armed it by mistake. A block is **seen** when it intersects the window and either a quarter of it, or a quarter of the window's height, is inside: the second half is for a block taller than four windows, which no fraction of itself can ever satisfy. Only `opacity` and `transform` change, so the layout does not shift.
* **CountUp** is `<span class="cui-countup" data-to="704">704</span>`. The text is the final number from the first byte; when the element is 40 % seen, the script sets `data-frame` and CSS draws the frame in `::after` with `content: attr(data-frame) / ""` (the empty alternative text keeps a frame out of the accessibility tree) while the real text is made transparent for 1.3 s; then the attribute goes. The script does this only if the browser supports an alternative text in `content` (`CSS.supports`); otherwise nothing counts. The numbers are integers from 0 to 1,000,000,000 written in the program; the width of the number does not change (`tabular-nums`; the real text keeps the box).
* **`visible:` / `hidden:`** bind an event of the component without parameters to a `Section`, `Article`, `Aside` or `Figure`; the element is written `data-cui-visible="Seen"`. The page's script observes it (50 %, or half the window's height) and tells `app.js` the name when it is seen and when it is left, by a queue, so an element on the screen before `app.js` has run is told once when it has. The element must be **in the page's HTML and never rebuilt by script** (`visible-dynamic`): an element script builds is built again at every render, and the event it fires would draw the page again. A `Reveal` is not subject to this, because one that is not armed is only shown.
* **Not offered**: pointer parallax (`www.md` 5.4: an island's), a threshold, a delay, an easing, an `animation` binding, a way to opt out of reduced motion.
* **Authority**: none. No capability, no host, no `requires`; `diff` shows a changed `to:` or event in the view as any change, and `authority: unchanged`.

## 4. Refusals

| tag | refused |
|---|---|
| `countup-not-number` | no `to:`, or one that is not a whole number written in the program |
| `countup-out-of-range` | `to:` outside 0 to 1,000,000,000 |
| `countup-has-children` | text or an element inside a `CountUp` |
| `visible-event-invalid` | `visible:` or `hidden:` naming an event the component does not handle, or one with parameters |
| `visible-dynamic` | `visible:` or `hidden:` on an element that is not in the page's HTML: in a `For`, an `If`, a component that is not the mounted one, or a page that is built by script and not hydrated |
| `inline-outside-rich`, `rich-child-invalid`, `binding-unknown` | existing tags: a `CountUp` outside a `Rich`, a `Heading`, a `Caption`, a `Link` or another inline element; a `Reveal` in a `Rich`; `visible:` on any other element, `label:` on a `Reveal` |

## 5. Measured in the build

`scripts/check-motion.js` (Chromium, `Pixel 7` profile and desktop 1280, `examples/motion`; 36 checks, all pass), `cargo test --test motion` (8: 8 valid views, 23 refusals, every tag documented and reached, a program cut at every prefix and with every character deleted never a panic, the HTML, the script only where used, the module's table, authority):

* **JavaScript off**: the served HTML has the 3 blocks, the final numbers `704`, `333`, `62` and the watched section, and none of the script's own state (`cui-armed`, `cui-in`, `data-frame`); with the script off every block is opaque with a height, and the text reads `Conformance: 704 of 704, refusals 333.` (the real page reads `0 / 704`).
* **No `IntersectionObserver`** (deleted before the page loads): nothing armed, nothing hidden, the numbers final, a `visible:` section told once.
* **Normal motion**: 3 blocks armed, 2 of them hidden below the window at load on the Pixel 7 profile; scrolling through the page shows all 3, **the tall one included: it is 4,064 px against a window of 839 (a quarter of it, 1,016 px, is more than the window)**, so the fraction alone could never have shown it; **layout shift 0** from load to the end on both profiles.
* **Reduced motion** (both profiles): nothing armed, nothing hidden, 0 running animations, 0 frames drawn in 2 s with the first count-up on the screen, the numbers final; arming every block by hand still leaves it opaque (the stylesheet alone); print shows every block.
* **A count-up**: the DOM text is `704` at 6 samples across the tween; frames (`499`, `584`, `650` ..) are in `::after` with `content: "499" / ""` while the text is transparent; the accessibility tree of the paragraph is `Conformance: 704 of 704, refusals 333.` while it runs; at the end the attribute, the colour and the style are gone. With `CSS.supports` returning false nothing counts. (The real page, same measurement: DOM `0 / 704` and tree `"0"`, `" "`, `"/ 704"` half a second in.)
* **`visible:` and `hidden:`**: `[0,0]` below the window, `[1,0]` at half, `[1,1]` when left, `[2,1]` on the second visit, still `[2,1]` 1.5 s later (no loop); a section on the screen at load with `app.js` held back 1.5 s is told once when it arrives; a section 3,624 px tall in a window of 839 is told once half a window of it is in.
* **A block built by script** (an `If` that shows a `Reveal`) is shown at once and never armed.
* **Nothing is touched**: the blocks, numbers and sections are the same nodes after `app.js` ran; **0 requests to another host**; no console error.
* `scripts/breaks/motion.json`: 28 breaks, each caught by a check failing on its own (none `HUNG`, `BUILD`, `BAD`), the first time.

## 7. What the build changed in this note

* `CountUp { value: n }` is `CountUp { to: n }`, an inline element; `on Visible` is `visible: Event`; `visible-threshold-invalid` is dropped; `reveal-dynamic` was planned and is not needed (an unarmed block is shown); `visible-event-invalid` and `visible-dynamic` were added (section 4).
* A dead check was not written: `visible:` given a string or a number is a **parse** refusal, not `visible-event-invalid`, and a `Section` cannot read state in a binding, so `visible-dynamic` is reached by a `For`, an `If`, another component and a page that is not hydrated.
* The 13 gallery pages embed the baseline stylesheet, which gained the motion rules, so `docs/demo/*/index.html` were rebuilt.
* `examples/motion`: 33 programs and 28 replaying a trace now (`docs/index.html`, `check-gallery.js`).

## 6. Not measured

Safari and Firefox (`IntersectionObserver` thresholds, `content` with an alternative text, `CSS.supports`), a real phone, a screen reader reading a page while a count-up runs (the accessibility tree is Chromium's: it reads the final number, a screen reader was not run); a flash of the visible block before the inline script arms it (the script runs before the first paint on this machine's loads, not measured as a paint); a page hundreds of reveals long (one observer; not timed); the page being scrolled by find-in-page or an anchor into a block that is not yet seen (the observer sees it when the browser scrolls there; one anchor was measured in `page.md`, not for a reveal).
