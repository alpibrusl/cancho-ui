# Semantic diff — `cancho-ui diff`

> **Status: designed before built (Phase 3).** This document was written
> before the code, per [CONTRIBUTING.md](../CONTRIBUTING.md) §1. Where
> building disagreed with it, the correction is made in place and listed in
> §10. Guarantee tier of everything below: **static** (read off the typed IR
> by a deterministic function; no execution, no inference).

```sh
cancho-ui diff <base.cui> <candidate.cui> [--output json]
```

The question it answers: *what can this revision do that the last one could
not, and what can it no longer do?* It is the answer to
[proposal §7.4](proposal.md) and the review artefact for an agent-authored
change: the source diff is for a human to read, this is for a gate to act on.

---

## 1. What is compared, and why the IR

Both files go through the **whole** front end (lex, parse, type-check,
lower, including cross-file `import` resolution), exactly as `check` does,
and the two resulting typed IRs are compared. The source text is never
compared. Consequences, all deliberate:

- Comments, blank lines, formatting, declaration order and line numbers do
  not make a diff. Moving a component down ten lines is *no semantic change*.
- A program that does not compile has no IR, so it has no diff. It is a
  refusal (§6), never a guess from text.
- Source spans (`file:line:col`) are **never** part of identity; they are
  only attached to findings so a reader can go and look (§3.4).

Components are matched **by name**. The IR's `id` is an index in file order
and is not stable, so it is ignored.

### 1.1 Categories

| # | Category | Compared as | Why it matters |
|---|---|---|---|
| 1 | Components | set of names | a new component is new behaviour; a removed one is lost behaviour |
| 2 | `requires` (capabilities) | set of capability names per component | **the headline**: this is authority |
| 3 | State fields | by name: added, removed, type changed, initial value changed | state shape is the component's contract with its handlers |
| 4 | Events | by name: added, removed, params changed | the component's input surface |
| 5 | Handler writes | per event, the set of state fields its body assigns | "which events can change which state" ([proposal §7.2](proposal.md)) |
| 6 | Handler body | per event, the body with spans stripped | a handler can do something different while writing the same fields (`min = 12` to `min = 20`) |
| 7 | Reachable effects | multiset of `(capability, call path)` per component | the effects the checker found: what the program actually reaches |
| 8 | View elements | multiset of element descriptors (kind, role, label, event and state bindings) | accessibility metadata and which event a control fires |

Handler body (6) is reported as `body_changed` plus the two rendered bodies,
because (5) alone would call `min = 12` and `min = 20` identical.

### 1.2 What is compared *exactly* (and how)

- **Types** are compared structurally and rendered as source syntax
  (`Result[List[String], String]`).
- **Initial values** and **handler bodies** are compared as a canonical,
  span-free rendering of the IR expression tree (`(a - b)`, `clock.now()`,
  `match r { Ok(_) => "done", ... }`). They are *not* compared by evaluating
  them: `1 + 1` and `2` are different. A syntactic difference with equal
  value is reported; the diff errs toward over-reporting.
- **Reachable effects** are keyed by `(capability, path)`. If a call to
  `clock.now()` goes from 2 sites to 3, the key is `changed` with counts; an
  effect appearing for the first time is `added`.
- **View elements** are a multiset of one-line descriptors such as
  `Button role=button label="Stop" on=Stop`. Multiset, not tree: see §8.

---

## 2. Authority: the headline

Authority is *what a component declares it may reach* (`requires`). Per
component, comparing the set of `requires` names:

| Result | Meaning |
|---|---|
| `expanded` | at least one capability is **new** (including every capability of an added component) |
| `reduced` | no capability is new, and at least one was removed (including every capability of a removed component) |
| `unchanged` | the same set |

The program-level `authority` is `expanded` if **any** component is
`expanded`; otherwise `reduced` if any is `reduced`; otherwise `unchanged`.
So a revision that drops `Clock` from one component and adds `Network` to
another is `expanded`: the safe direction never masks the unsafe one. The
per-component and flat lists say exactly which.

Authority is granted per component, not per program. An added component
that requires `Clock` is `expanded` even if another component already
required `Clock`: the new component is new authority at the unit of review.

`authority` is about `requires` only. A new reachable effect is reported
under `effects` and `changed`, but it can only be added together with a
`requires` (otherwise the revision is refused `capability-missing`) or under
a capability the component already holds; the second case is a behaviour
change inside granted authority, and is *not* `expanded`.

---

## 3. Output

`--output json` (also the shape a tool should rely on); a short human form
otherwise. Both are written to **stdout**. Field order is fixed as shown;
every list is sorted (§4); empty categories are present as empty arrays so a
consumer never has to test for missing keys.

```json
{
  "schema": "cancho-ui-diff",
  "version": "0.1.0",
  "base": "base.cui",
  "candidate": "candidate.cui",
  "changed": true,
  "authority": "expanded",
  "change_count": 3,
  "capabilities": {
    "added":   [ { "component": "Clock", "capability": "Network" } ],
    "removed": []
  },
  "components": {
    "added":   [ { "name": "Panel", "requires": ["Network"], "state": ["x"], "events": ["Go"] } ],
    "removed": [],
    "changed": [
      {
        "name": "Clock",
        "authority": "expanded",
        "requires": { "added": ["Network"], "removed": [] },
        "schedules": { "added": ["every 100 Tick"], "removed": ["every 5000 Tick"] },
        "state": {
          "added":   [ { "name": "n", "ty": "Int", "initial": "0" } ],
          "removed": [ { "name": "ticking", "ty": "Bool", "initial": "false" } ],
          "changed": [ { "name": "started_at",
                         "ty": null,
                         "initial": { "base": "0", "candidate": "5" } } ]
        },
        "events": {
          "added":   [ { "name": "Reset", "params": [], "writes": ["started_at"] } ],
          "removed": [],
          "changed": [ { "name": "Start",
                         "params": null,
                         "writes_added": ["ticking"],
                         "writes_removed": [],
                         "body": { "base": ["..."], "candidate": ["..."] } } ]
        },
        "effects": {
          "added":   [ { "capability": "Network", "path": "net.search",
                         "spans": [ { "start": {"line": 12, "column": 8},
                                      "end":   {"line": 12, "column": 24} } ] } ],
          "removed": [],
          "changed": [ { "capability": "Clock", "path": "clock.now",
                         "base_count": 2, "candidate_count": 3 } ]
        },
        "view": { "added": ["Button label=\"Reset\" on=Reset"], "removed": [] }
      }
    ]
  },
  "not_compared": [ "foreign_boundaries", "properties", "lifecycle", "target_overrides",
                    "event_ordering_and_cancellation", "view_structure_and_order",
                    "view_text_content", "expression_value_equivalence" ]
}
```

### 3.1 Field meanings

- `changed` — `false` iff every list is empty (and then `authority` is
  `unchanged`). `change_count` is the number of leaf entries (every
  added/removed/changed item counts one), so a gate can threshold on it.
- `capabilities` — the flat, per-component list of `requires` added and
  removed, so "did authority grow, and where" needs no traversal. It repeats
  data in `components`; it is derived from it, never separately computed.
- `components.changed[].state.changed[]` — `ty` and `initial` are each `null`
  when that facet did not change, else `{ "base": ..., "candidate": ... }`.
- `components.changed[].events.changed[]` — `params` is `null` or
  `{base, candidate}` (lists); `writes_added` / `writes_removed` are state
  field names; `body` is `null` unless the span-free body differs, else the
  two lists of rendered statements.
- `effects[].spans` — where the effect is, in the file the entry belongs to:
  `added` entries point into the **candidate**, `removed` entries into the
  **base**. Sorted by position. `changed` entries carry counts only.
- `not_compared` — a fixed list of categories this version does **not**
  inspect (§8). It is in the output so an agent reading `changed: false`
  cannot mistake silence for a claim.

### 3.2 Human form

```
cancho-ui diff base.cui -> candidate.cui
authority: expanded
component Clock  (changed; authority expanded)
  + requires Network
  ~ state started_at: initial 0 -> 5
  + event Reset(): writes started_at
  ~ event Start: writes +ticking
  + effect Network net.search at 12:8
  + view Button label="Reset" on=Reset
```

`+` added, `-` removed, `~` changed. `no semantic change` (one line, exit
0) when nothing differs. The human form is parallel to the JSON, not a
second source of truth.

### 3.3 Diagnostics

Refusals use the existing diagnostic shape
([`spec/diagnostics`](../spec/diagnostics/README.md) §4), so no new rule
tags are introduced by `diff`; it reports the front end's own (§6).

### 3.4 Spans

Spans are positions in the file named by the entry's side (§3.1) and are
informational. They are never compared.

---

## 4. Determinism

The same two files give byte-identical output on every run and every
machine. Sorting: components by name; state, events by name; capabilities,
writes by string; effects by `(capability, path)`; spans by position; view
descriptors by string (a descriptor present twice counts as one entry, with
the multiset difference expanded: a button added a second time is reported
once per added copy). No timestamps, no absolute paths beyond the arguments
as typed, no hash-map iteration order.

---

## 5. Exit codes

| Code | Meaning |
|---|---|
| `0` | **no semantic change** |
| `1` | **refused**: a file did not compile; located diagnostic printed (as for `check`/`build`) |
| `2` | command line wrong (not exactly two files, unknown flag or `--output` value) |
| `3` | environment failed (a file is missing or unreadable) |
| `5` | **the programs differ semantically** |

### 5.1 Why `5`, and not `1` as in `diff(1)`

`diff(1)` uses 1 for "differ". In cancho-ui `1` already means *refused with
a located diagnostic* for `check` and `build`, and every wrapper that
treats `1` as "the program is wrong" would read "the candidate differs"
as a failure to compile, or the reverse, and an agent gate that reacts to
a refusal by fixing the source would try to *fix a diff*. So `1` keeps its
one meaning across the CLI and `diff` takes its own code.

`4` was rejected because [CONTRIBUTING](../CONTRIBUTING.md) §5 reserves it
for `test` ("a test failed"); a code a wrapper can read as a command-independent
failure class should not be overloaded. `5` is the next free value. The
consequence for scripts: `diff` is not `set -e`-friendly on a real change.
That is by design; the change is data, not an error, and the caller decides
what a `5` means.

A refusal wins: if either file does not compile the exit is `1` and no diff
is attempted (there is no IR to compare).

---

## 6. Bad input

| Input | Result |
|---|---|
| A file that does not lex, parse, type-check or lower | exit `1`; the same located diagnostic `check` prints; **both** files are checked, so both refusals are reported. Human: stderr. `--output json`: stdout, a JSON array of diagnostics (the `check` shape; the `file` in each span says which file) |
| A file that does not exist or cannot be read | exit `3`, a one-line message on stderr |
| Fewer or more than two files, an unknown flag, `--output` without a valid value | exit `2` with usage |
| `base` and `candidate` the same path | exit `0` |

No input reaches a panic (CONTRIBUTING: refusals are values).
`tests/no_panic.rs` runs the diff over every prefix and every
one-character deletion of the examples against the original.

On exit `1` with `--output json`, stdout is the diagnostic array, not the
diff object; the exit code says which one to expect.

---

## 7. What the guarantee is

**Tier: static.** If the diff says `authority: unchanged` the set of
`requires` of every matched component is identical, and every component
present in one program is present in the other. That is a statement about
the *declared* authority in the typed IR.

---

## 8. What it deliberately does not claim

- **Not a proof of equivalence.** `changed: false` means *none of the
  categories above differ*. The category list is the claim. It is not
  "the two programs behave the same".
- **Schedules are compared** (`start E`, `every N E`, as strings; a rate is not authority, so `authority` is unchanged by them).
- **The IR has no foreign/trust boundaries, component properties, lifecycle
  hooks, target overrides or async-ordering structures yet.** There is
  nothing to diff, so `not_compared` names them. When the IR grows them, the
  diff grows with it and the schema version minors. (Proposal §7.4's "new
  foreign-code dependencies", "changed event ordering or cancellation
  behavior" and "platform-specific behavior" are therefore **not built**.)
- **Component instances.** A parent composing a child appears as a
  `Named` view element (`Panel`), and so shows up in the `view` multiset;
  the *property values* it passes (`Const` bindings) are rendered into the
  descriptor, so changing one is a `view` change. There is no separate
  `properties` category.
- **View structure and order.** The view is compared as a multiset of
  element descriptors. Reordering siblings, or moving a button into another
  container, is *not* reported. Text content (`Text { "..." }`) is not
  compared, except through the effects it reaches. A tree diff is future
  work.
- **Value equivalence.** Initial values and bodies are compared
  syntactically (§1.2), never evaluated.
- **No runtime behaviour.** A handler's *dynamic* writes are not tracked
  beyond its assignments; no event ordering, no cancellation timing.
- **Cross-file edits** are covered only through the merged IR: a changed
  imported module changes the importer's diff if and only if the merged IR
  differs.
- It does not say whether the change is *good*.

---

## 9. Surfaces

`cancho-ui diff` appears in `--help`, `introspect` (with its exit codes,
including `5`), the `skill` text, the README command list,
AGENTS.md and [CONTRIBUTING](../CONTRIBUTING.md) §5. The exit-code table in
`introspect` gains the per-command override `diff: {"5": ...}` rather than
changing the global table, because `5` means nothing to the other commands.

---

## 10. Corrections made while building

- **Spans of view-originating effects are coarse.** The design (§3.4)
  assumed every effect span points at the call. Effects found inside a
  `Text` interpolation in the view carry the lowering's placeholder span
  (`1:1`-`1:12` for `clock.now()` in `examples/clock/clock.cui`), not the
  call site. The diff passes the IR's span through unchanged; it does not
  repair it. Effects in event handlers have exact spans. (A lowering fix is
  outside this slice; it will make the diff's spans exact with no change to
  the diff.)
- **`introspect`'s per-command `exit_codes`.** §9 said the global table
  would not change; in fact the global `exit_codes` object gained a `5`
  entry labelled "diff only", and `diff` carries its own full table. The
  global entry is there so a consumer that reads only the global table
  does not meet an undocumented `5`.
- **Tests run the engine in-process, not imports.** The in-process front
  end used by the tests does not resolve cross-file `import`s, so the
  multi-file example (`dashboard`) is covered through the binary only
  (`diff x x` for every example exits 0), and the mutation test covers the
  single-file examples. An imported module change is therefore tested only
  as "the merged IR is what is compared", not by a dedicated cross-file
  fixture.
- **`introspect` and `skill` take their first argument as an output path**
  (pre-existing: `introspect --output json` writes a file named
  `--output`). `diff` does not share that parser and is strict about flags.
  Not changed here.
- Measured, not designed: the diff was run over 14,698 compiling
  one-character mutants of the single-file examples against their
  originals, in both directions, without a panic
  (`tests/no_panic.rs`).

---

## 11. The theme and the links of a view (added 2026-10-24)

Two things beside components are compared, and neither is authority.

* **A theme** (`theme.md`): `theme` in the JSON is a list of `{ setting, base, candidate }`, one per colour token, font key or scheme added, removed or changed (`light.accent`, `dark.bg`,
  `font.body.sha256`), absent when the themes are equal; the human form is `~ theme light.accent: #185fa5 -> #0f3d6e`. It counts in `change_count`; `authority` does not move.
* **The hosts a `Link` names** (`structure.md` 3): a changed component has a `links` delta (`added`, `removed`: `github.com`, `mailto:example.org`), absent when equal; the human form is `  + link evil.example`.
  A new host is a review item: it is where a reader can be sent. It is not an effect, so `authority` does not move.

---

## 12. Sealed islands (added 2026-10-25)

A sealed island (`islands.md`) is code the program did not write, so the diff says it first. `islands` in the JSON is a list of `{ setting, base, candidate }`, one per setting added, removed or changed: the island itself
(`Hero`), each file with its SHA-256 (`Hero.script assets/scene.js`, `Hero.style assets/hero.css`), the order the scripts run in (`Hero.script-order`), `Hero.label`, `Hero.decorative`, `Hero.size`, each prop
(`Hero.prop seed`: `Int`) and each event (`Hero.event Picked`: `Int`). Absent when the islands are equal. The human form is `~ island Hero.script assets/scene.js: ab5fffd8c1d2 -> 7c0e2e91aa40`, and these lines are printed **before** the
components. It counts in `change_count`. **`authority` does not move**: a sealed island reaches nothing but the page, by messages, so adding one, or changing a file of one, is `authority: unchanged` and a line of its own. An **open** island (`sealed: false;`, `islands.md` 12) runs in the page's origin: one that is added, or one that was sealed and is opened, makes `authority: expanded` (`reduced` for the reverse), whether or not `requires Dom` changed; the island line says `sealed -> open`. The props an *instance* is given (`Stones { seed: 3 }`) are properties, which this version does not compare (`not_compared`).

### The head of the page

`page { .. }` ([`page.md`](page.md)) is compared as `theme` is. `page` in the JSON is a list of `{ setting, base, candidate }`: `title`, `lang`, `description`, `share_description`, `canonical`, `host` (the one host `canonical:` names), `image`, `image.alt`, `image.size`, `image.sha256`, `icon`, `icon.sha256`, `touch_icon`, `touch_icon.sha256`, `alternate agents.md` (its title), `alternate agents.md.sha256` and `skip_link`; absent when the pages are equal. The human form is `~ page title: A -> B` and `+ page host example.org = ..`. It counts in `change_count` (exit 5). **`authority` does not move**: nothing in the head is fetched by the page from another host, so a new canonical host is a line of its own and not a capability. A sticky `Header` is a binding of the view and is compared with the rest of the view.

### The data a page is written from

`data Name { file sha256 }` ([`data.md`](data.md)) is compared as `theme` and `page` are: `data` in the JSON is a list of `{ setting, base, candidate }` for `Name.file`, `Name.sha256`, `Name.bytes` and `Name.rows` (the items read), absent when equal; the human form is `~ data Ecosystem.sha256: a1b2c3d4e5f6 -> 0f1e2d3c4b5a`, and they count in `change_count` (exit 5). **`authority` does not move**: the file is read when the page is built and nothing is reached when it runs. The `For` over the data is written out before the program is compared, so the elements it stands for are compared like any: a project added is a `+ view` line for each element and a `+ link <host>` for a new host, a chip that changed tone is a `+`/`-` pair. **The text of an element is not compared** (`view_text_content` in `not_compared`), so a changed name is the data lines alone.
