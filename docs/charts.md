# Charts: `Chart`, an SVG drawn from data, with a text alternative (issue #77)

> **Status: BUILT** (design in #125; section 7 says what the build found and corrected). Written 2026-10-24 before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md)
> section 1). Numbers measured in Chromium (Playwright, `Pixel 7` profile, 412 px) on main at 5f61076.

## 1. What exists

`Canvas { label: "Chart" }` and `Image { .. }` are `component-kind-unknown`. The only graphics are text and the tone colours of a `Text` (`presentation.md` section 3). A program that holds a list of numbers
(`state readings: List[Float]`, `List[Int]` are accepted types; `map(json.items(..), x => num(x, 0))` builds one from a reply) cannot show it as anything but text.

## 2. Measured

| question | measured |
|---|---|
| **cost of drawing N points from nothing**, as the page does at every render (median of 15) | line (one `<polyline>`): N=1,000 **1.0 ms**, 10,000 **5.3 ms**, always **2 DOM nodes**. Bars (N `<rect>`): 100 → 0.9 ms, 1,000 → **8.3 ms**, 5,000 → 42 ms, 10,000 → **84 ms**, N+1 nodes. Canvas: 0.1 ms at 1,000, 0.5 ms at 10,000 |
| what a screen reader gets | `<svg role="img" aria-label="..">` is one node, role `image`, named by the label, **whatever is inside** (1,000 `<rect>` children: 3 nodes in the whole page). A canvas has no content at all; its alternative would have to be a second thing |
| a data table as the alternative | 200 rows, **closed** in `<details>`: **5** accessibility nodes; **open**: **1,414** (about 7 per row); 1,000 rows open: 7,014 nodes and 51 ms to build. The summary is a `DisclosureTriangle`, `expanded: false`, keyboard-operable |
| contrast of a series colour against the page | light (`Canvas` `rgb(255,255,255)`): `#0969da` 5.19, **`#0550ae` 7.59**, `#1f6feb` 4.63. Dark (`Canvas` `rgb(18,18,18)`): `#58a6ff` 7.42, **`#79c0ff` 9.63**, `#388bfd` 5.60. WCAG 1.4.11 asks 3:1 for a graphic that carries meaning |
| 412 px (Pixel 7, with the viewport meta the generated page has) | `viewBox="0 0 320 180"` with `width:100%` and 16 px page padding: 364 × 205 px, a 12-unit label appears as **13.7 px**, no horizontal scroll; at 360 px 11.7 px, at 320 px 10.2 px, still no scroll. (Without a viewport meta the same page is laid out at 980 px: the generated pages have one, a bare host page may not) |
| labels made of markup (`</text><script>..`, `<img onerror>`) set with `textContent` and `setAttribute` | stays text: no script ran, no `<img>` was created |

So: SVG, not canvas. A line costs two nodes at any size; bars cost one per bar, which is why the build draws a bounded number. The text alternative is the SVG's own label plus a collapsed data table, which costs five
nodes until someone opens it.

## 3. The decision

* **A view element `Chart { label: "Temperature" kind: "line" values: readings labels: hours unit: "°C" }`**, drawn by the page as an inline SVG inside a `<figure>`. One series per chart. `kind:` is `"line"` or `"bar"`
  (`chart-kind-invalid`). More kinds (a gauge, an area, several series, a scatter) are additions that change nothing here.
* **What the program supplies.**
  * `label:` (required): the chart's name, a visible `<figcaption>` and the start of its accessible name.
  * `values:` (required): the name of a `List[Int]` / `List[Float]` state field, or a list literal of number literals (`chart-values-invalid` otherwise; also a field of another type). A list the program builds
    with `map(.., x => num(x, 0))` is put in a state field first.
  * `labels:` (optional): the name of a `List[String]` field or a list literal of string literals. The i-th label names the i-th value; a missing one is the position (`3`). Extra labels are ignored
    (`chart-labels-invalid` for another type).
  * `unit:` (optional): a string literal of at most 12 characters, written after a number in the text alternative and the table, never interpreted (`chart-unit-invalid`).
* **Limits.** The page draws **at most the last 1,000 values** (a series is usually time: the recent end matters), and the table lists at most the last 200. Measured above: bars cost 8.3 ms at 1,000, a page that redraws every
  100 ms (`every`) can afford it; 10,000 bars at 84 ms cannot. The alternative says so: `, the last 1000 of 5000 shown`.
* **Drawing.** `viewBox="0 0 320 180"`, `width:100%`, `max-width:40rem`, `height:auto`, `display:block`. Axes and text use `currentColor` (so both themes), three to five "nice" y ticks, x labels for the first, middle and last
  position. A line chart's y range is the data's min and max with 5 % padding; a bar chart's includes 0 and negative bars are drawn below the zero line. No animation.
* **Text alternative, derived from the same data.**
  * the `<svg>` has `role="img"` and `aria-label="<label>: <summary>"`; its contents are presentation only (measured: one node whatever they are);
  * the summary is one sentence: `line chart of 5 values: minimum 1 °C, maximum 5 °C, last 5 °C` (`bar chart`; `with no values` when the list is empty; the truncation clause above), numbers rounded to two decimals;
  * a `<details><summary>Data table</summary><table>` with one row per drawn value up to 200 (position or label, value and unit), collapsed. It is keyboard-operable and its exact values are how a keyboard,
    touch or screen-reader user reads a point.
* **Colour.** One series colour from a CSS custom property, `--cui-chart`: `#0550ae` in light (7.59 : 1) and `#79c0ff` in dark (9.63 : 1), defined by the baseline stylesheet of `--pwa` and `--page` (a host page that
  styles itself defines `--cui-chart`; the fallback in the SVG is the light value). Gridlines are `color-mix` of `currentColor` at 25 %, decorative (the values are in the table and on the axis). Nothing is encoded by
  colour alone: position and length carry the value, and a single series needs no legend.
* **Interaction.** None in this step: no pointer tooltip, no per-point focus (a chart of 1,000 points would be 1,000 tab stops, and 1,000 `<title>` nodes). The chart is not focusable; the table is.
* **No script and no markup from data.** Every string from the program (label, unit, labels) is set with `textContent` or `setAttribute`; every number is formatted by the page (measured: markup stays text).
* **No capability.** A chart draws what the program already holds.

## 4. Replay: what a chart is in a trace

Replay does not draw, so it cannot see geometry, and does not claim to. It can see the data and the text alternative, which is what a trace should assert. After each step the report lists, for each `Chart` that is in the page
(an `If` that is false hides it; a `Chart` is not drawn inside a `For`: `chart-in-loop`), `{ "label", "kind", "count", "summary" }`, read from the state as it is then. A step may assert it:
`"expect": { "charts": [{ "summary": "line chart of 3 values: minimum 1, maximum 3, last 3" }] }` (the existing array and object matching: same length, each entry a subset). The summary is built by one function whose rounding is
`math_builtin`'s `round`; the page builds the same text, and `tests/chart_corpus.json` is run against both.

## 5. What the build must have

Tags: `chart-incomplete` (no `label:`, `kind:` or `values:`), `chart-kind-invalid`, `chart-values-invalid`, `chart-labels-invalid`, `chart-unit-invalid`, `chart-in-loop`; the chart's keys on another element are `binding-unknown`.
A shared corpus run in replay (the summary and the count) and in Chromium (the same text as `aria-label`; the geometry checked against an independent oracle in the script: the `points` of the line, the `x`, `y`, `width`, `height` of
each bar, the ticks); 0, 1, 2, 3, 1,000 and 5,000 values; negatives; a constant series; large and tiny numbers; markup in a label; the table (rows, cap, collapsed by default, keyboard-opened); 412 and 320 px with no overflow;
contrast of the series colour, the axis text and the table text in both schemes read from the page; reduced to the stylesheet. `examples/charts` (a line and a bar chart of a list that a button extends) and a gallery demo. Deliberate breaks
(a wrong scale, the last values not the first, no summary, a label as markup, the series colour in one scheme only, the cap removed, the table open by default, a chart in a hidden `If` reported).

## 6. Not done

Several series and a legend; a gauge, area, scatter, pie; axes with dates; a tooltip, a crosshair, point focus and arrow-key reading; zoom; logarithmic scales; animation; exporting the image; a canvas escape hatch (an
`Image`/`Canvas` for a picture the program cannot describe is a separate decision: it would need its own, explicit text alternative); a screen reader's speech; iOS and a real phone.

## 7. Built (2026-10-24), and what the build found

* **Built.** `Chart` in the language, IR (`IrElementKind::Chart`, JSON name `Chart`), replay (`crates/cancho-ui-replay/src/chart.rs`: the one sentence builder; the report's `charts`) and the page (`CHART_ADAPTER_JS`, emitted only
  for a program with a `Chart`; the stylesheet's `--cui-chart` for both schemes). `examples/charts` (+ trace) and a live gallery demo; `tests/chart_corpus.json` (15 cases, including 1,000 and 1,500 values and markup
  in labels); `crates/cancho-ui/tests/chart.rs` (9 tests) and 2 unit tests; `scripts/check-chart.js` (116 checks in Chromium); eighteen deliberate breaks (`scripts/breaks/chart.json`), each caught. The tags of section 5 are
  in `spec/diagnostics/README.md`.
* **Checked in Chromium** (Pixel 7, 412 px): for every case the SVG's accessible name is `<label>: <the sentence replay reports>`; the geometry (the polyline's points, every bar's `x`, `y`, `width` and `height`, the y ticks, the x
  labels) agrees with an **oracle written again in the check script from the formulas of section 3**, sharing no code with the page, and for `[1, 2, 3]` equals the numbers worked out by hand before the page existed
  (`40,145.55 175,81 310,16.45`; bars `53.5,104.67,63,47.33` and so on), which anchors the oracle too; 1,000 values draw 1,000 points and 1,000 bars, 1,500 draw the last 1,000, the table lists the last 200 (first row position 1,301, last
  1,500) and says so; the table is collapsed, opens with Enter on its summary and closes with Space, and the summary is 44 px high; markup in labels ran nothing and created nothing; contrast read from the page in both
  schemes: series colour 7.59 : 1 (light) and 9.63 : 1 (dark) against `Canvas`, axis and table text over 4.5 : 1; 412, 360 and 320 px wide: no horizontal scroll, a label at 13.7, 11.7 and 10.2 px; 1280 px: the chart stops at 640 px.
* **Found while building.**
  * **The text of the axis is in the accessibility tree.** §2 measured that 1,000 `<rect>` children of `role="img"` cost nothing; `<text>` children are different: Chromium lists them (seven numeric `StaticText` nodes
    for one chart), although ARIA makes the children of an image presentational. The axis text is now `aria-hidden`, and a check asserts that no number of the axis is a node.
  * **Rounding to two decimals is Math.round's, not the usual one.** `2.005` reads `2.01` (JavaScript's `2.005 * 100` is `200.5`), `1.005` reads `1`; replay's `math_builtin("round")` agrees with the page on every
    corpus value, which is the point of using it. My first test of replay asserted `2` for `2.005`; the check against JavaScript is why the assertion was corrected, not the code.
  * **A report with no chart has no `charts`**, so `expect: {"charts": []}` is the empty list, as for `ticks` and `routes`.
* **The gallery tile.** The "Charts" coming-soon tile is removed and the page shows a live demo. `check-gallery` R5 did **not** force it (its probe, `Canvas` is still refused, is still true), so this is a judgment: the tile
  promised charts, charts exist, and what is still missing (`Canvas`, `Image`) is already named by the Maps tile ("a map or image primitive").
* **Still not done:** section 6. In addition, numbers of 1e21 or more are written `?` in the alternative on both sides.
