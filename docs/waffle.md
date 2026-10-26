# Waffle, Chip, and the data a page is made from

> **Status: `Waffle` and `Chip` BUILT (2026-10-26) with their measurements (section 6); data-driven sections are DECIDED, NOT BUILT (section 4).** Step 6 of [`www.md`](www.md) (5.5 and the
> data-driven lists of 5.1b). Measured against the real cancho-www page (`index.html`, `assets/site.js`, `assets/site.css`, `data/ecosystem.json`, commit `0579a80`, read-only) in
> Chromium (Playwright, desktop Chromium on Linux; the `Pixel 7` profile is a viewport and a user agent, not a phone): `scripts/measure-waffle.js`, `scripts/measure-data.js`
> and `scripts/check-waffle.js`.

## 1. What the real page does, measured

| | the page | measured by |
|---|---|---|
| the waffles | 3 SVGs of `<rect>`s from `data.measured.figures` by `site.js` `waffle()`: **704** squares in 44 columns, **333** in 37, **62** in 16: **1,099 `<rect>`s** (`www.md` says 1,099 of 2,951 nodes) | `index.html`, `site.js` |
| with JavaScript off | `<div id="w-conformance"></div>`, `w-refusals`, `w-rules`: **empty**: the drawings are made by script from a fetched file | `index.html` |
| what is told to a screen reader | `aria-label="697 of 704"` on the SVG, nothing else; the meaning of the amber outlines ("known differences") is a sentence in a paragraph below | `site.js`, `index.html` |
| three kinds of square | counted (filled `--accent`), not counted (`--soft` fill, `--line` stroke), and the 7 "known differences" (no fill, `--warn-line` stroke 1.4) | `site.css` |
| their contrast against `--bg` (light / dark) | counted **5.93 / 7.40**; known outline **2.05 / 3.30**; empty stroke **1.23 / 1.33**; empty fill **1.08 / 1.16**. WCAG 1.4.11 asks 3:1 of a graphic that carries meaning: **the known outline fails in light and the empty squares fail in both**; the squares that say "what is not checked yet" are close to invisible | `theme-oracle.js` on the page's palette |
| the same with text tokens of the same palette | `muted` 6.94 / 7.65, `warn` 6.18 / 11.81 | same |
| a staggered fill | `transition-delay: calc(var(--i) * 1.6ms)` on each `rect` (the squares light one after another when the card is seen) | `site.css` |
| the chips | `<span class="chip built">` and three more stages in the generated lists: **14** (built 9, early 2, alpha 2, design 1) among the 42 inline spans of text blocks that `structure.md` 6 counted; the other classes are `evs`, `us`, `ln`, `n`, `prever`, `tag` (parts of the page's drawings and panes, not of prose) | regular expressions over `index.html` |
| the data | `data/ecosystem.json`: **23,489 bytes**, 24 projects (stages: built 8, early 7, alpha 6, design 3), 3 measured figures, 3 lists of claims, 3 paths. Four regions of `index.html` between `<!-- name:start -->` markers are written into it by `scripts/gen_agent_files.py`; the map, the waffles and the count-ups read the file at run time with `fetch` | `index.html`, the script |

## 2. The waffle: what the squares cost, measured

The same 1,099 squares written three ways (`scripts/measure-waffle.js`, Chromium, 412 px, median of 15 parse-and-layouts; the accessibility tree of the page is **6 nodes in every case**):

| | bytes | gzipped | elements | parse + layout |
|---|---:|---:|---:|---:|
| one `<rect rx="3">` per square (the page's) | 69,789 | 3,611 | 1,103 | 7.5 ms (20.5 ms on the first run) |
| one `<path>` per kind, rounded corners | 86,295 | 3,701 | 9 | 1.2 ms |
| **one `<path>` per kind, square corners** | **20,355** | **2,164** | **9** | **0.5 ms** |

So the squares are `<path>`s with square corners: 29 % of the bytes and 1 % of the elements of the page's version, and the same text alternative. The price is the corner (`rx: 3`) and the staggered fill, which are cosmetic and are not offered (a motion that is a function of a square's index is the kind of thing `motion.md` keeps out).

## 3. The decisions

* **`Waffle` is its own element, not a kind of `Chart`** *(corrected 2026-10-26; `www.md` 5.5 said `Chart { kind: "waffle" .. }`)*. A `Chart` draws numbers that are **state** (`values: readings`), by script, into a figure: it is a hole in the prerendered page, bounded at 1,000 values, with a replay report. A waffle is drawn from **three numbers written in the program**, which makes it static: it is in the page's HTML with its caption, its sentence and its table, and the real page's empty `<div>` is exactly what a crawler and a reader without script see today. The two share the look of a figure (`class="cui-chart"`) and nothing else. A waffle of state is not offered: it would be a hole again.
* **`Waffle { label: ".." of: 704 counted: 697 columns: 44 rest: "known" }`** (the plan's `value:` is `counted:`: a key `value:` is parsed as a state field name). `label:` is the caption and the start of the name; `of:` 1 to 5,000 squares (20 bytes each: 100 kB at the most); `counted:` 0 to `of`; `columns:` 1 to 100; `rest:` is what the squares that are not counted are: `"known"` (outlined, thick: a difference somebody listed) or `"open"` (hollow, thin, the default: not yet). The plan's `1472`/`1380` were wrong numbers; the page's are 704/697, 333/256 and 62/62.
* **Colours are tokens the theme already checks.** Counted squares are filled `accent` (the series colour `--cui-chart`: 7.59:1 on white and 9.63:1 on dark with no theme, 5.93:1 and 7.40:1 with the example theme); not-counted squares are outlined, `known` in `accent` 2 wide and `open` in `muted` 1 wide (`--cui-chart-open`: 6.39:1 and 6.43:1 with no theme, 6.94:1 and 7.65:1 themed). **No new token and no new pair**: `accent` and `muted` each have a pair with `bg` in the theme's 14. This replaces the page's `warn_line` outline (2.05:1) and `soft`/`line` empties (1.23:1).
* **No meaning in colour alone**: counted is filled, the others are hollow; known is 2 wide and open 1 wide; the sentence says the numbers (`697 of 704, 7 known differences`); the table says them again. All measured (`check-waffle.js` C).
* **Text alternative.** The `<svg>` is `role="img"` named `"<label>: <counted> of <of>[, <rest> known differences | not yet]"` and the figure has a `<figcaption>` and a closed `<details>` with a three-row table (counted, known differences or not yet, squares): the `Chart` contract, with the words fixed in English as `Chart`'s are.
* **`Chip { tone: "good" "working" }`** *(decided from measurement: the page has 14 chips in prose and 4 stages; `structure.md` 6 left it to the port)*: an inline element, in a `Rich`, a `Heading` or a `Link`, holding **text and nothing else**. `tone:` is a closed set: `good`, `warn`, `accent`, `quiet`, each a colour the theme already pairs (`good` on `bg`, `warn` on `warn_bg`, `accent` on `bg`, `muted` on `bg`: 4.5:1). Its words are the message; the quiet tone also has a dashed border. A `Span { class }` is still not offered: the 14 chips are all the styled inline text of prose the page has.

## 4. Data-driven sections: how the page's data gets in (DECIDED, not built)

What is measured about the choices (`scripts/measure-data.js`, on the page's real `ecosystem.json`):

| | a list **fetched at run time** (`requires Network; net.get("data/ecosystem.json")`, `json.items`, a `For`) | the same 24 names **written in the program** (what a build-time expansion would write) |
|---|---|---|
| `inspect effects` `requires` | **`Network`** | `[]` |
| project names in the page with script off / on | **0 / 24** | 24 / 24 |
| `index.html`, `app.js` | 9,508 and 14,069 bytes | 10,006 and 1,641 bytes |
| requests the page makes (script on) | 3 (the page, the script, the data file) | 2 |
| `diff` when `ecosystem.json` changes a stage and the program does not change | **exit 0, "no semantic change"**: the data is invisible to review | (a changed literal is a changed program: exit 5) |

The runtime version is correct for what it does and costs three things the page cares about: nothing for a crawler, a reader without script or `agents.md` to read; a `Network` capability for a file of the page's own; and a data file that `diff` cannot see. The literal version has none of them, and is what the page's Python generator writes today into `index.html`; what it lacks is the file as the one source.

**Decision: a hashed data file declared in the program, read at build time**, like the files of an island or an `Image`:

```
data Ecosystem { file: "data/ecosystem.json" sha256: ".." }
```

* **Read at build, not fetched at run.** The compiler reads the file once (inside the program's directory, at most 1 MB, valid JSON, its SHA-256 equal to the declared one: a changed file is a changed program, as for an island), and the page's lists and numbers are **expanded at build time into static elements** (prerender, no hole), so a crawler, a reader without script and `agents.md` see the same content; **no `Network` capability and no fetch**. `authority` is unchanged (nothing is reached); `inspect effects` lists the data file with its hash and size; `diff` prints `~ data Ecosystem.sha256 ..` and the changes of the expanded view as any change, and counts them (exit 5). This is exactly what the runtime `net.get` version cannot do: it hides the file from `diff`, needs `Network`, and draws nothing before the script.
* **What it needs, not built here**: a way to say "for each item of this list in the file" and "this field of it" that is evaluated by the compiler, not by the page (`For { over: data.items(Ecosystem, "projects") as: p .. }` and `json.get(p, "name", "")` run once, at build, over a literal list), and a replay that sees the same expanded view. It is a feature of its own (the expansion changes what `check`, `replay` and `diff` look at), so **it is a pull request of its own**, after this one.
* **What stays as it is**: `net.get("data/ecosystem.json")` for what a program wants to refresh while it runs (the map's filter, a live count), which is an own-origin read declared as `Network`, as today.
* **Open, for the user**: whether `agents.md`, `llms.txt` and the page may then come from one file by two routes (the Python script and the compiler) without disagreeing: `www.md` 10 item 4 left it to be revisited after the page is built, and it still is.

## 5. Refusals

| tag | refused |
|---|---|
| `waffle-incomplete` | no `label:` (or a blank or non-literal one), `of:`, `counted:` or `columns:` |
| `waffle-of-invalid`, `waffle-counted-invalid`, `waffle-columns-invalid`, `waffle-rest-invalid` | a number that is not written in the program or is outside its range (`of` 1 to 5,000, `counted` 0 to `of`, `columns` 1 to 100), a `rest:` that is not `"known"` or `"open"` |
| `waffle-has-children` | text or an element inside a `Waffle` |
| `chip-tone-invalid`, `chip-empty` | no `tone:` or one outside the four; a chip with no text |
| `binding-unknown`, `inline-outside-rich`, `rich-child-invalid` | existing tags: `of`, `counted`, `rest` or `columns` on another element, `gap:` or `tone:` on a `Waffle`, `label:` on a `Chip`; a `Chip` outside a `Rich`, a `Heading` or another inline element; an element inside a `Chip` |

## 6. Measured in the build

`scripts/check-waffle.js` (Chromium, `Pixel 7` profile and desktop, `examples/waffle`; 31 checks, all pass) and `cargo test --test waffle` (8: 10 valid views, 33 refusals, every tag documented and reached, a program cut at every prefix and with every character deleted never a panic, the HTML, the module's drawing, the largest waffle, authority):

* **With JavaScript off**: three waffles in the page's HTML; **every square is where an oracle written in the check from the note's formulas puts it** (697 counted of 704 in 44 columns, 256 of 333 in 37, 62 of 62 in 16; the drawings are `0 0 745 269`, `0 0 626 150` and `0 0 269 65`); the sentence (`Conformance tests that pass: 697 of 704, 7 known differences`), the caption and a **closed** three-row table; **17 elements** in a figure of 704 squares (the page's is 705: the drawing and a `<rect>` each); the accessibility tree has the drawing as **one image** named by the sentence.
* **Drawn by script** (inside an `If`): the figure is **byte for byte** the one the page was written with, in the SVG namespace.
* **Colour**: counted, known and open squares are **7.59, 7.59 and 6.39:1** of the page with no theme in light and **9.63, 9.63 and 6.43** in dark, **5.93, 5.93, 6.94** and **7.40, 7.40, 7.65** with the example theme (the squares are its `accent` and `muted`); the four chips are 4.5:1 or more of what is behind them in all four settings; counted is filled and the others hollow, known is 2 wide and open 1 wide, the quiet chip has a dashed border and every chip says its message in words.
* **The page around them**: no horizontal scroll at 320 px; layout shift 0 and no animation; the figures, drawings, paths and chips are the same nodes after `app.js` ran; no request to another host; no console error.
* `scripts/breaks/waffle.json`: 28 breaks, each caught by a check failing on its own (none `HUNG`, `BUILD` or `MISS`; the first run reported one `BAD`: a `from` that `cargo fmt` had rewritten, corrected).
* `examples/waffle`: the proof section of the page in miniature (34 programs now: `docs/index.html`, `check-gallery.js`; the 13 gallery pages were rebuilt because the baseline stylesheet gained the waffle and chip rules).

## 7. Not measured

Safari, Firefox and a real phone; Windows high-contrast mode (forced colours: the squares' fills and strokes may be replaced by the system's, and counted versus hollow would then be the only difference); a screen reader reading the figure (the accessibility tree is Chromium's: one image named by the sentence); the look of a waffle beside the page's (no pixel comparison: square corners and no staggered fill are different on purpose); a waffle of more than 5,000 squares; a chip whose text is long (it does not wrap: `white-space: nowrap`).
