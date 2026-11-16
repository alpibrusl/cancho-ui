# File budget and `include`: a program too long for one file is split, not refused forever

> **Status: BUILT** (the tests are `crates/cancho-ui/tests/file_budget.rs` and `include.rs`; the mutants are `scripts/breaks/file-budget.json` and
> `include.json`). Written before the compiler ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Anything not measured says **assumption**.

## 1. Why

[`CONTRIBUTING.md`](../CONTRIBUTING.md) section 2 says no source file is over 2,000 lines. It was a rule for this repository's own sources; a program
written in cancho-ui was not held to it, and could not be: `import module.Component;` carries **one component** from a sibling file, there are no
events from a child to its parent and no state they share, so a component that is too big for one file cannot be cut by imports. The console of a
real application (one component, 527 state fields, about 250 handlers, a view of 4,244 lines, 7,405 lines in all) is one file because the language
gave it nowhere else to go.

Two things follow, and they ship together because the first is the way out of the second:

1. **`include "path.part";`** splices the text of another file into this one, before anything reads the program (section 2).
2. **`file-too-long`** refuses a source file over a limit, and says to split it with `import` or `include` (section 3).

## 2. `include`

### 2.1 The rule

An **include line** is a line whose first non-blank text is the word `include`, then a string, then `;`, and nothing else but blanks:

```text
        include "parts/view/home.part";
```

* It is replaced, as text, by the **lines of the named file**. The replacement is made before the lexer runs; the lexer, the parser and everything after
  see one program, the *expanded* text. The expanded text of a file is the file with each include line replaced by the lines of its part (a part
  that does not end in a newline gets one), and parts expand in turn.
* The splice is **not** a macro: it has no parameters, no conditions, no evaluation, and no name; the text it brings is the file's text, unchanged,
  at the line where the directive stands. The identity of a program is its IR, and the IR of the expanded text is the IR of that text written in one
  file (section 2.5), so [`CONTRIBUTING.md`](../CONTRIBUTING.md) section 8 (no textual macros) is not broken: there is no text whose meaning depends
  on where it is used, only text that is somewhere else.
* Because the directive is a whole line replaced by whole lines, it works **wherever a line can stand**: inside a component among its state and
  handlers, inside a `view { }` and at any depth of nested view nodes, between the arms of a `match`, inside a handler. The part is indented as it is
  written; the compiler does not reindent it (nothing in the language depends on indentation).
* An include line is recognised only in code. Inside a string literal, a `//` comment or a `/* */` comment (also when the string or the comment began
  on an earlier line) the same words are just text. A part that opens a comment it does not close leaves the rest of the including file in that
  comment, exactly as if the lines were written in place.
* The word `include` followed by a string anywhere else in code, or an include line with anything after its `;`, is `part-not-standalone-line`. An
  `include` that is not followed by a string (a state field called `include`, say) is an ordinary word and is left alone.

### 2.2 The path

* Relative to the **directory of the entry file** (the file named on the command line), also in a module imported from another file, as the asset
  paths of [`structure.md`](structure.md) section 10.5 are.
* May name subfolders: `parts/view/home.part`. Must end in `.part` and have a name before it.
* No `..` or `.` segment, no empty segment (`a//b.part`), no leading `/`, no `\`, no `:` and no control characters. `part-path-invalid`.
* A symbolic link is followed and its **real path** must be inside the real directory of the entry file; a link out of it is `part-path-invalid`
  (the same test as an asset's, which says `asset-missing` because there the question is whether the file is there).
* A file that is not there, is a directory, cannot be read or is not UTF-8 is `part-missing`.

### 2.3 Limits

| rule | limit | refusal |
|---|---|---|
| nesting of includes (a part that includes a part ...) | 8 deep: the entry's own include is depth 1 | `part-nested-too-deep` |
| a file reached while it is being expanded (`a.part` includes `b.part` includes `a.part`, or a part includes itself) | none: it is a cycle | `part-cycle` |
| the expanded text of one source file | 100,000 lines, or the line limit of section 3 if that is higher | `part-expansion-too-large` |

The last bound is not in the owner's list of rules; it is there because without it a part that includes the next one a hundred times at each of eight
levels is 10^16 lines. The same part may be included twice: it is the text twice, with whatever the language does with that text written twice in place.

### 2.4 What a diagnostic says

A diagnostic about text that came from a part cites **the part**: `span.file` is the part's path and `span.start.line` is the line in the part, the
column is the column in the part (the splice does not move a column). The compiler keeps, for each expanded file, a table from each line of the
expanded text to the file and line it came from; every diagnostic is mapped through it once, at the end of the compile, so no check has to know.
A refusal of the directive itself (`part-missing`, ...) cites the file the include line is in, which may be a part, at the include line.

The table is lines, not spans, because a directive is a whole line replaced by whole lines. The IR and the emitted script are the IR and the script
of the expanded text, so **their positions are lines of the expanded text** (this is what makes section 2.5 true); only diagnostics are mapped back.

### 2.5 Same IR as written inline

For a program and the same program with some of its text moved into parts, `inspect ir`, the built `app.js` and the built `index.html` are
**byte for byte the same** (test `include.rs`: a component with state, handlers and a nested view, split three ways, built with `--page`).
Source paths that the output records are the entry's, never a part's.

### 2.6 Reading the text

`cancho-ui inspect sources <entry.cui>` prints the **expanded text of the entry file**, exactly what the lexer is given, to stdout. There is no flag:
the concatenation is the default, so `cancho-ui inspect sources app.cui | grep -n 'on Save'` finds a handler wherever the text is kept. It does not
need the program to type-check (only that its includes can be expanded). `--output json` gives `{ "entry", "files": [{ "path", "role", "lines", "bytes",
"included_by" }], "text" }` with `role` one of `entry`, `module` (a file reached by `import`, which has its own includes expanded) and `part`.

### 2.7 Not parts, not modules

A part is not parsed alone, declares nothing and has no name in the program: it is not a component and not a module, and `import` cannot name it.
Each part counts for the file budget on its own (section 3). `include` is not for packaging: there is no search path, no version, no registry.

## 3. `file-too-long`

Each source file the compile reads (the entry, each imported module, each part) is refused when it has more than the **limit**:

* **2,000 physical lines** by default. Lines are counted as `str::lines()` counts them: a line ends at `\n` (a `\r\n` is one line), a last line without
  a newline counts, an empty file has 0, comments and blank lines count.
* **and 160 KiB** (163,840 bytes) by default, the second bound being the one a file of 2,000 very long lines meets first. It scales with the line limit
  (`bytes = 163,840 x limit / 2,000`, so a limit of 7,405 lines is 606,630 bytes) so that one setting is all a consumer sets.

The refusal is tag `file-too-long`, `Error`, tier `Static`, subject `File { path }`. Its span is the first line over the limit (line 1 for the byte
bound), and its message says the file, the count, the limit and the **unit** (`lines` or `bytes`). Remediation: split the file with `import` (one
component per module file) or `include` (parts of one component); it does not suggest raising the limit.

Where the limit comes from, first that is set:

1. `--max-file-lines N` on any command that compiles (`check`, `build`, `inspect`, `diff`, `test`, `report`);
2. the environment variable `CANCHO_UI_MAX_FILE_LINES=N`;
3. 2,000.

`N` is a whole number of at least 1; `0`, a negative number, a number with a sign or a unit, an empty value and text are **not** a limit: the flag is
a usage error (exit 2) and the variable an environment error (exit 3), each saying what it got. A number too large for 64 bits means "no limit"
(it is clamped; nothing overflows). The command `skill` and `introspect` compile nothing and neither read the variable nor take the flag.

**A ratchet, by convention.** The default is the ceiling the project holds itself to (`CONTRIBUTING.md` section 2: "only come down, never up"). A
consumer that already has a long file sets the limit at that file's length and lowers it as the file is split; raising it above 2,000 is what the
console of section 5 needs on the first day and is meant to be temporary. **The compiler does not enforce the direction**: it cannot know what the
last value was. A consumer that wants the direction enforced keeps the number in its repository and fails its own CI if it grows.

## 4. Diagnostics

All refusals, `Error`, tier `Static` (no warnings: [`spec/diagnostics/README.md`](../spec/diagnostics/README.md) section 3.1):

| tag | subject | when |
|---|---|---|
| `file-too-long` | `File` | a file over the limit of section 3 |
| `part-missing` | `File` | the named part is not a readable UTF-8 file |
| `part-cycle` | `File` | a part reachable from itself |
| `part-nested-too-deep` | `File` | more than 8 levels of include |
| `part-path-invalid` | `File` | the path breaks a rule of section 2.2 (or links out of the directory) |
| `part-not-standalone-line` | `File` | `include "..."` where it is not the whole of a line |
| `part-expansion-too-large` | `File` | the expanded text of one file is over its bound |

## 5. What the console sees (measured, 2026-11-16)

The one-file console of the application this was written for (`cancho-ota/ui/trip.cui`, read only) is 7,405 lines and 482,826 bytes. Built with this
compiler:

| run | result |
|---|---|
| `cancho-ui check trip.cui` | `error[file-too-long] at trip.cui:2001:0: ... has 7405 lines, over the limit of 2000 lines`, exit 1 |
| `CANCHO_UI_MAX_FILE_LINES=7404` | the same, at line 7405 |
| `CANCHO_UI_MAX_FILE_LINES=7405` | passes; `build` writes a 3,914,943-byte module; `inspect sources trip.cui` prints the file unchanged (no include in it) |

The byte bound at a limit of 7,405 is 606,630, so the file is under it (79.6 %); with an unscaled 160 KiB the same file would have been refused for bytes
at any line limit, which is why the bound scales. Its view alone is 4,244 lines, so more than two parts of the view are needed to bring each file under
2,000: that is the next piece of work for the console, not for the compiler.

## 6. Not claimed

* **Editors and line numbers outside the compiler.** A tool that reads `span.file` and `span.start.line` finds the right line. A tool that takes
  the line of the *entry* (the `Diagnostic.related` of an unrelated rule, a source map built from IR positions) sees lines of the expanded text.
* **No fmt, no per-part check.** A part is not checked alone; an unbalanced brace in a part is reported where the compile meets it.
* **The ratchet is not enforced** (section 3).
* **`diff` compares programs, not files**: moving text between a file and a part changes no IR, so `diff` reports no change (the point), and
  nothing reports that a file moved.

## 7. Interaction with the nesting bound

[`nesting.md`](nesting.md) measures a program 100,000 levels deep; as text that is 200 to 500 KB, over the byte bound, so with the default limit the
answer is `file-too-long` and not `parse-nesting-too-deep`. `tests/nesting.rs` runs the compiler with `CANCHO_UI_MAX_FILE_LINES=1000000`, because it measures the stack and not
the budget. Both refusals are right; which one a program meets first is the one that is cheaper to check.
