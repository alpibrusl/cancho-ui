# Data: a file the program pins, written out into elements when the page is built

> **Status: BUILT (2026-10-26), with its measurements (section 7).** The decision was taken in [`waffle.md`](waffle.md) 4 (a hashed file read at build time, not `net.get` at
> run time, because the runtime version needs `Network`, shows 0 of 24 names without script, and is invisible to `diff`). Worked example: the cancho-www page's own
> `data/ecosystem.json` (23,489 bytes, commit `0579a80`, read-only) in `examples/ecosystem`. Measured in Chromium (Playwright, desktop Chromium on Linux; the `Pixel 7`
> profile is a viewport and a user agent, not a phone) and with the compiler's own commands: `scripts/check-data.js`, `scripts/measure-ecosystem.js`.

## 1. What it is

```
data Ecosystem { file: "data/ecosystem.json" sha256: ".."; }

For { over: data.items(Ecosystem, "projects") as: p
    Rich { Link { to: json.get(p, "url", "") "{json.get(p, \"name\", \"\")}" } " ({json.get(p, \"stage\", \"\")})" }
}
```

The compiler reads the file when it builds, checks it, and **writes the `For` out**: one `Rich` for each project, with its link, its text and its chip, as if the program had been written with 24 of them. Nothing of the file is in the page: no fetch, no `Network`, no script, and the file is not copied into the build.

## 2. The declaration

`data Name { file: "<path>" sha256: "<hex>"; }`, a top-level declaration in the **entry file** (`data` is a keyword only there, followed by a name). The file is relative to the program, inside its directory (a link out of it counts as not there), `.json`, at most **1 MB**, UTF-8 that is one JSON value (as `JSON.parse` reads it: no comment, no trailing comma, `NaN`, byte order mark or raw line break in a string; **duplicate keys: the last one wins**, as in `JSON.parse`, measured), nesting at most **32** (a document of 200 levels is refused by the parser's own recursion limit before that). Its SHA-256 is written in the program: **a changed file is a changed program**, as for an island.

## 3. The expansion

* **What is written out.** A `For` whose `over:` reads a data file (`data.items(Name, "path")`, the compact JSON text of each item of the array at `path`, at most 1000) **or a name an enclosing expansion bound** (`json.items(p, "tags")` inside the loop over `p`, `filter(data.items(..), q => ..)` for a join). Its children are written once for each item, with the name bound to the item's JSON text. A `For` over a state field, or over a literal list, **is left as it is** (a hole, as before).
* **What is worked out in the copies.** Every text part, `label:`, `to:` of a `Link` or `CountUp`, `of:` and `counted:` of a `Waffle`, and event argument that reads a bound name is evaluated by the evaluator replay uses (`cancho_ui_replay::eval_closed`: one evaluator, so `json.get` is the function of the page's script and of replay) and written as a literal; an `If` whose `when:` is then `true` is replaced by its children and one that is `false` by nothing. What is not closed (it reads state) stays an expression and is a hole, as before. Only expressions that read a bound name or a data file are touched; the rest of the program is as it was written. Bindings that take a literal accept an expression for exactly these keys (`to:` of `Link` and `CountUp`, `of:` and `counted:` of `Waffle`; `label:` already did); a `Chip`'s `tone:` takes a literal, so a tone that depends on the data is a chain of `If`s, as in the example.
* **Before everything else.** The expansion runs on the syntax tree before the program is type-checked, lowered, replayed, diffed or built, so **there is one program after it**: every refusal applies to the expanded values (a `javascript:` address in the file is `link-scheme-refused`, a size of 0 is `waffle-of-invalid`, a number over a billion is `countup-out-of-range`), and the nesting bound of 32 still holds (a `For` is replaced by its children at the same depth).
* **Not offered**: data in an imported module (the file is declared and used in the entry file), a data file as a value in a handler, a text of the file that is markup (the page's generator turns `` `code` `` in a claim into `<code>`: not reproduced, section 6).
* **Bounds**: 1,000 items for one `data.items`, 20,000 nodes written by the program's expansions between them (`data-expansion-too-large`).

## 4. What each tool sees (measured, `cargo test --test data`)

| tool | sees |
|---|---|
| `check` | the program after the expansion; the refusals above and every ordinary refusal on the expanded elements |
| `inspect component` | the elements: no `For`, no `data.items`, the texts as literals |
| `inspect effects` | `requires: []`, `reachable_effects: []`, the hosts the data links to in `links` (as for any `Link`), and `data`: the name, file, **sha256**, bytes, items read and the components that read it |
| `inspect data` | the same list for the program |
| `inspect prerender` | **0 holes** where the program had a `For` over data: the elements are static |
| `test` (replay) | the expanded program: its handlers run as written, and the elements are in the view it holds |
| `build --page`/`--pwa` | the elements in the page's HTML before any script, hydrated (not rebuilt); **no** `net.get`, `fetch`, `data.items` or file name in `app.js` or the page, the file not in the build |
| `build` (module only) | the same elements, drawn by the module with no data in it |
| `diff` | `~ data X.sha256: a -> b`, `.bytes`, `.rows` (the items read) and `.file` as lines of their own, **counted in `change_count`** (exit 5); `authority` does not move; **plus the changes of the expanded elements the diff compares**: an added project is `+ view Link to=".."`, `+ view Chip tone=".."`, `+ view Rich` and `+ link <host>` when the host is new; a stage that changed from `built` to `alpha` is `+ view Chip tone="warn"` and `- view Chip tone="accent"`. **The text of an element is not compared by `diff`** (`view_text_content` is in its `not_compared`), so a project renamed is the data lines alone: the file's hash and size are the review handle, and `git diff` of the file is the review |

A file edited without its pin changed is not a build: `data-hash-mismatch`, exit 1 (a test).

## 5. Refusals

| tag | refused |
|---|---|
| `data-duplicate`, `data-path-invalid`, `data-hash-invalid` | a name declared twice; a path with `..`, absolute, an address, not `.json`; a hash that is not 64 lower-case hex digits |
| `data-file-missing`, `data-hash-mismatch`, `data-too-large` | not a file inside the directory of the program (a link out counts as not there); the SHA-256 is not the pin (the message gives the one the file has); over 1 MB |
| `data-malformed`, `data-too-deep` | not UTF-8, not JSON; more than 32 levels (or the parser's recursion limit) |
| `data-unknown`, `data-use-invalid` | `data.items(Name, ..)` for a name not declared (or no declaration at all); not `data.items(Name, "path")` |
| `data-shape-invalid`, `data-too-many-rows`, `data-expansion-too-large` | no array at the path (missing, an object, a string); more than 1000 items; more than 20,000 nodes written |
| `link-scheme-refused`, `link-to-invalid`, `waffle-of-invalid`, `chip-tone-invalid`, `countup-out-of-range`, `countup-not-number` | existing tags, reached by the **expanded** values |

## 6. The worked example against the page's own lists (`scripts/measure-ecosystem.js`)

`examples/ecosystem` is written from the page's `data/ecosystem.json` and read with JavaScript off against the lists the page's Python generator writes into `index.html` (Chromium; the page's waffles with script on):

| | the page (`index.html`, generated) | `examples/ecosystem` (compiled) |
|---|---|---|
| the list under the map | 24 items | **24, identical text and links in all 24** |
| the three paths | 3 articles; per path 9 links (project, its page, the next step) and 3 stage chips | **3 articles, the links in order and the chips identical in all 3** |
| the claims | 17 items, 23 links, **5 `<code>` elements** | 17 paragraphs, 23 links, **0 `<code>` and 10 backticks left in the text**: the generator's markup of `` `code` `` is not reproduced |
| structure | `ul` 10, `li` 68 | **`ul` 0, `li` 0**: cancho-ui has no list element, so a list is a row of paragraphs and a screen reader is not told it is one |
| the waffles | **0 drawings and counts `0` with script off**; 3 drawings, 1,015 counted squares with script on | 3 drawings and 1,015 counted squares **with script off**; counts `697`, `256`, `62` in the HTML |
| bytes | `index.html` 49,368 | `index.html` 48,148 (the page's stylesheet and the whole page, with no other file), `app.js` 7,308 |

What it does not reproduce, and so what the port needs: **inline code in data text** (a markup for the text of a claim), **list elements** (`List`/`Item`), and the page's `reveal`, its sticky header and its map, which are other steps. The numbers of the proof section are in the program as `num(json.get(f, "passed", "0"), 0)` over the file's own figures; the waffle's third figure is hard-coded in the page's script (16 columns, 62 of 62) and is `value` and not `passed` in the file, which the example reads.

## 7. Measured in the build, and what it found

`scripts/check-data.js` (Chromium, `Pixel 7` profile, with JavaScript off for the data; 15 checks), `cargo test --test data` (13 tests: 17 valid programs, 38 refusals, every tag documented and reached, a program cut at every prefix and with every character deleted, every prefix of five data files, a hostile file, effects, diff, replay, a link out of the directory):

* **A. The data** (the oracle is the file, read in the check by `JSON.parse`): the 24 projects (names, stages, summaries, addresses, in order), the 3 paths with the join of each entry to its project (the stage and the tone of each of the 9 chips, 27 addresses in order), the 17 claims and the 23 evidence links, the three final counts, the three waffles (697 counted and 7 outlined as known, 256 and 77 hollow, 62 and none).
* **B. It asks for nothing**: the page asks for `/` and `/app.js` and for nothing else; no data file in the build; layout shift 0; no console error.
* **C. Hostile data**: names holding `<img onerror>`, `<script>`, `</title>`, and an address with a quote, are text and one `href`, and nothing ran; `javascript:` in the data is `link-scheme-refused` at build. The hostile files of the corpus are refusals and never a panic: 9 kinds of malformed JSON, 33 and 200 levels, 1 MB and 1000 and 1001 items, a name of 600,000 characters (valid, written), duplicate keys (the last wins), numbers (a 16-digit integer reads as the double `JSON.parse` gives, `1e300` is not an integer and `json.get` answers its default, `-0` is `0`).
* **D. Nothing is touched**: the 39 elements the data wrote are the same nodes after `app.js` ran.
* `scripts/breaks/data.json`: 26 breaks, each caught by a check failing on its own.
* **Found, and fixed here: a program named by a bare file name did not find its files.** `cancho-ui check page.cui` run inside the program's directory gave `asset-missing` (a picture, a page file) and `island-source-missing` (an island): the directory of `page.cui` is the empty path, which cannot be looked at. It was in `main` since the files of a view and the islands were read from disk (measured: `examples/page` and `examples/islands` failed from their own directory). Every directory is now `program_dir(file)` (`.` for a bare name), with a test over four examples.

## 8. Not measured

Safari, Firefox and a real phone; a data file near 1 MB expanded into 20,000 nodes (the bound is tested, the time it takes is not); a data file that is edited while the compiler runs; the page's other generated regions (`demo-text`, from `data/demos.json`); that `agents.md` and `llms.txt` made by the Python script and the page made by this agree: they read the same file, and nothing yet checks that their words are the same (`www.md` 10 item 4 is still open).
