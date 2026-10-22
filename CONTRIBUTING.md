# Contributing to cancho-ui

## 1. The gate

**`docs/` lands before `compiler/`.** Every behavior cancho-ui claims
to enforce is settled in a design document under [`docs/`](docs/)
first — its syntax, its semantics, its guarantee tier, and the
checked fixture that exercises it. The compiler and backend arrive to
implement what is already decided, not to decide it.

The cheapest place for a design claim to be wrong is in a document
that has no tests yet. Every checked claim must end up with an
outstanding test or a measured number; the absence of either is what
gets corrected, in place, in the document that made the claim.

A claim that turns out false is corrected in the document that made
it, not quietly edited and not moved into a commit message.

## 2. The file budget

No source file over **2,000 lines**. This is
[cancho's own rule](https://github.com/alpibrusl/cancho/blob/main/CONTRIBUTING.md)
(`tests/files.rs` enforces it); it survives cancho-ui because the
same forces apply — long files are where unchecked drift hides.

A ceiling is a ceiling; only come down, never up.

## 3. How a slice is done

cancho-ui slices follow the same shape as cancho's:

1. **The design lands first**, in `docs/`, with the guarantee tier
   labelled (`static | tested | runtime | trust | unknown`).
2. **The fixtures land second**, in `examples/`, as the contract the
   compiler or backend must meet — both valid (must compile/run) and
   invalid (must refuse, with the rule that refuses them).
3. **The implementation lands third**, smallest piece first, with the
   gate green at every commit.
4. **The measurement lands fourth**, if the slice makes a performance
   or correctness claim.
5. **The roadmap row lands last**, recording what shipped, what was
   found out, and any corrections to the design document that came
   out of writing it.

A slice that does not change `docs/` first is unlikely to be the
right slice.

## 4. Refusals and rule tags

Every error is a **refusal with a stable rule tag**. There are no
warnings, no hints, no "consider this instead" — a refusal is a
refusal. A diagnostic has:

- `code` — stable, kebab-case (`effect-not-declared`, `unknown-effect`,
  `capability-missing`, `lifecycle-after-dispose`, …).
- `severity` — `error` today; warnings are a refusal tomorrow.
- `span` — source range, optional when not source-located.
- `subject` — what is being refused (`component`, `state`, `effect`,
  `capability`, `lifecycle`, `foreign`).
- `message` — short explanation an agent can read.
- `expected`, `actual` — when a type or value mismatch is the cause.
- `remediation` — suggested fix the compiler can justify; never
  inferred from an LLM.
- `tier` — `static | tested | runtime | trust | unknown`, the same
  five the guarantees table uses.

The format is in [`spec/diagnostics/`](spec/diagnostics/) and will
not drift: the first refusal a tool emits locks the schema.

## 5. The agent-facing CLI

`cancho-ui check`, `cancho-ui build`, `cancho-ui test`,
`cancho-ui inspect`, `cancho-ui diff`, `cancho-ui report` all have
**stable exit codes** and **machine-readable output** for the data
they bear. Human-readable output is parallel, not the primary form.

The pattern is lifted from [`cancho introspect` /
`cancho skill`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md)
and the existing `cancho authority --output json` /
`cancho check --output json`. Exit codes follow cancho's:

- `0` — success.
- `1` — refused, with a located diagnostic.
- `2` — command line wrong.
- `3` — environment failed (missing tool, no such file).
- `4` — (test only) a test failed.
- `5` — (diff only) the two programs differ semantically; `0` is "no
  semantic change". `1` is not used for this because it already means
  "refused" ([`docs/semantic-diff.md`](docs/semantic-diff.md) §5.1).

`--output json` is on the data-bearing commands, not every command.
Single-dash flags where the value follows the flag literally
(`-o <path>`), double-dash where it does — same lesson the cancho
`-o`/`-l`/`-L` bug
[found](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md#2-the-false-familiarity-bug-this-found).

## 6. Refusal tests are first-class

Every rule the checker can produce has at least one **fixture that
must be refused with that rule**, written down in
[`examples/refusal/`](examples/). They are the standing gate for
that rule. A rule whose only fixture is "this should compile" is a
rule that can silently break.

### 6.1 No input reaches a panic (#62)

Two checks stand behind the rule, and one thing is not checked:

- **In the source.** `crates/cancho-ui/tests/panic_sites.rs` fails if a `crates/*/src` file contains `unreachable!`,
  `panic!(`, `todo!`, `unimplemented!`, `.unwrap()` or `.expect("` outside its `#[cfg(test)]` module. The way out is not
  an allow-list entry but a construction that cannot be wrong: a checked match and its extraction in one step
  (`bump_ident`, `<[T; N]>::try_from`), a total `match`, or a tagged refusal. `ALLOWED` in that test is empty; an entry
  needs the invariant in a sentence.
- **By input.** `tests/no_panic.rs` runs every prefix and every one-character deletion of every example through the front
  end, the diff and the replay.
- **Not checked.** Indexing (`v[i]`), slicing, integer overflow and stack depth are not searched for; the mutation test
  is what covers them, and it covers only the inputs it generates.

Measured when #62 was closed: the seven `unreachable!` sites (six in `parser.rs`, one in `machine.rs`) and 31
`.expect`/`.unwrap` sites outside tests were all replaced; the guard fails when one is put back (a `.unwrap()` in
`eval.rs` was reported with its line), and swapping `due < at` for `due <= at` in the replacement for the `machine.rs` arm
fails `a_tick_runs_before_a_sleep_due_at_the_same_instant`.

## 7. Trust boundaries are explicit

Foreign JavaScript, third-party packages, native modules, and platform
APIs are reached through **declared interfaces** (`foreign` blocks
and `effect`-typed adapters), not silently through type erasure. The
evidence report names every trust boundary the program crosses.
A typed wrapper does not prove the wrapped implementation safe —
this is the same hole cancho Core closed with
[`foreign-authority.md`](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md).

## 8. No textual or proc macros

Macros break stable, content-addressable identity. cancho-ui inherits
cancho's prohibition; the syntax tree is canonical, the IR hashes,
and no macro can change a declaration's identity from outside its
text.

## 9. The conventions

- **Naming.** Components `PascalCase`; state, events, effects,
  capabilities, helpers `snake_case`; module paths `lower.case`. The
  same as cancho Core's
  [`canonical-ast.md`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md).
- **Formatting.** `cancho-ui fmt` writes canonical layout; comments
  and blank lines are kept. Refuses a file it cannot reproduce,
  rather than risks an output that does not round-trip.
- **Comments.** They explain *why*, not *what*; the source itself is
  the *what*.
- **Errors in prose.** A wrong number is corrected in place, with the
  date of the correction in the same paragraph (`docs/ROADMAP.md`'s
  convention).

## 10. The suite

Once Phase 1 lands, the standing gate is:

```sh
# (placeholder — lands with Phase 1)
cancho-ui check examples/**/*.cui                 # valid fixtures must compile
cancho-ui check examples/refusal/**/*.cui          # refusal fixtures must be refused
cancho-ui test examples/<name>/<name>.cui examples/<name>/*.trace.json   # replay tests must pass (docs/replay.md)
cancho-ui fmt --check docs/ examples/ spec/       # canonical layout
```

Plus the cancho Core check the integration contract depends on:

```sh
( cd ../cancho && cargo test --workspace )
```

Until Phase 1, the gate is the markdown itself: every claim in
`docs/` is either labelled `unknown` (and acknowledged as such), or
named against the existing cancho Core document that settles it.