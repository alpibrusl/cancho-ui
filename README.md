<p align="center"><img src="docs/assets/cancho-ui-logo-256.png" alt="cancho-ui" width="200"></p>

# cancho-ui

A **declarative UI language and toolchain whose programs compile to
JavaScript + DOM, with capabilities, refusal discipline, and machine-readable
diagnostics modelled on [cancho](https://github.com/alpibrusl/cancho)**.
Components own state, declare the capabilities they need, and have their
authority and reachable effects exposed as data — the same authority model
Cancho uses at the systems level, lifted to the UI level.

## An example

```
component Clock {
    requires Clock;

    state started_at: Int = 0;
    state ticking: Bool = false;

    on Start {
        started_at = clock.now();
        ticking = true;
    }

    on Stop {
        ticking = false;
    }

    view {
        Column {
            Text {
                role: status
                label: "elapsed time"
                "Elapsed: {(clock.now() - started_at) / 1000}s"
            }
            Button { label: "Start" on: Start }
            Button { label: "Stop"  on: Stop  }
        }
    }
}
```

`requires Clock;` is the only way `clock.now()` is reachable. The compiler
checks every reachable effect against the declared capabilities and refuses
the program with `capability-missing` or `capability-unused` — the same
exact-row rule Cancho uses for its effect rows, applied to UI.

Run it:

```sh
cancho-ui build examples/clock/clock.cui -o examples/clock/clock.js
cd examples/clock && python3 -m http.server
# open http://localhost:8000/clock.html, click Start
```

## What's deliberately not here

- **No virtual DOM.** cancho-ui compiles straight to DOM ops
  (`createElement`, `setAttribute`, `createTextNode`, `appendChild`).
  The IR's view tree is data, lowered 1:1 to DOM. Adding a virtual DOM
  would mean two trees for the compiler to reason about, and the
  language model already has finer-grained reactivity available
  (state reads, state writes, view rebuilds) without it.
- **No JSX.** cancho-ui source is block-structured
  (`Text { role: status "hi" }`), not expression-templated. Block-style
  composes more cleanly with `role:` / `label:` accessibility blocks
  and with multi-line children.
- **No CSS-in-language.** cancho-ui emits `cui-col`, `cui-row` and
  semantic tags; styling is the host page's job. Phase 6 may add a
  scoped stylesheet story; for now the design is **the language
  owns behavior and accessibility, the page owns presentation.**
- **No implicit anything.** No implicit conversions, no operator
  overloading, no ambient authority: a component that calls
  `clock.now()` without `requires Clock;` does not compile.
- **No foreign untyped imports.** Every effect call is declared in
  the IR as an `IrReachableEffect`; the evidence report lists every
  trust boundary. A foreign function reached without a `foreign` block
  is a refusal, not a hidden capability.
- **No macros.** Macros break stable content-addressable identity,
  the same reason Cancho Core forbids them.

And honestly, **early**: Tier 1–11 are shipped (the async slice,
`Option[T]`/`List[T]` with `For`, the `Network` capability with
`net.search`, runtime cancellation with stale-result protection,
and pure list operations — `filter`/`map`/`len` with derived
`For` views). The compiler is Rust, the language is a small
declarative subset, and the toolchain is shaped to be one an AI
agent can drive. The seven components shipped so far (`Hello`,
`Counter`, `Clock`, `Search`, `Match`, `Ticker`, `Roster`) all
compile and run; the refusal discipline is locked in
`spec/diagnostics/README.md`; the CLI's JSON output is a contract,
not a string. What is **not** claimed: that an agent using
cancho-ui is more productive than one using a conventional stack.
That needs the Phase 3 benchmark, which has not been run.

The `Search` example is the proposal §9.3 demo, now in its real
form: a `TextField` drives a `query` state field, `Search` kicks
off `net.search(query)` — the `Network` capability — and the
awaited request dispatches across the `Ok(_) | Err(_) | Cancelled`
arms, assigning the matched list into `results`. `For` renders one
row per result, and a query that matches nothing shows the empty
state with zero rows. The Tier 9 `net.search` stub filters a fixed
corpus by substring and its simulated delay grows with the query
length — so Tier 10 cancellation is demonstrably needed: a slow
stale request resolves after a fast newer one, and the runtime
drops the superseded continuation at the await instead of letting
it clobber the fresh results. A real adapter keeps the same
resolve channel.

## Building

```sh
git clone https://github.com/alpibrusl/cancho-ui
cd cancho-ui
cargo test --workspace   # the whole gate: 49 unit + integration tests
```

The browser harness for the end-to-end DOM smoke tests (`/tmp/test-*.js`)
needs [`jsdom`](https://www.npmjs.com/package/jsdom):

```sh
cd /tmp && npm install jsdom
node /tmp/test-counter.js   # expects "OK: counter works"
node /tmp/test-clock.js    # expects "OK: clock works"
node /tmp/test-hello.js     # expects "OK: hello world rendered correctly"
```

Linux-x86_64 and darwin-aarch64 are green in CI; the binary is a single
release-mode artifact (~540 KB).

## The toolchain's whole surface

```sh
cancho-ui check <file.cui>... [--output json]
cancho-ui build <file.cui>... [-o <path>] [--target web] [--output json]
cancho-ui inspect component <file.cui> <name> [--output json]
cancho-ui inspect effects   <file.cui> <name> [--output json]
cancho-ui inspect ir       <file.cui>          [--output json]
cancho-ui report           <file.cui>...       [--output json]
cancho-ui skill [--output json] [<out-file>]
cancho-ui introspect [--output json]
cancho-ui --version
cancho-ui --help
```

Exit codes match cancho Core: `0` success, `1` refused with a
located diagnostic, `2` command line wrong, `3` environment failed.

`--output json` is the agent-facing surface. Consumers match on the
stable `code` field (`type-not-pascal-case`, `capability-missing`,
`lex-unexpected-char`, ...), not on prose. The shape is locked in
[`spec/diagnostics/README.md`](../spec/diagnostics/README.md) §4.

`inspect effects` is the one worth trying on something you did not
write — it lists the capabilities a component declares and the
reachable effects that need them, with the source span for each:

```sh
$ cancho-ui inspect effects examples/clock/clock.cui Clock --output json | jq .
{
  "component": "Clock",
  "reachable_effects": [
    {
      "capability": "Clock",
      "path": ["clock", "now"],
      "span": { "file": "examples/clock/clock.cui", "start": {"line": 11, "column": 24}, "end": {...} }
    }
  ],
  "requires": [
    { "name": "Clock", "span": {...} }
  ]
}
```

A `requires Clock;` with no reachable effect is refused with
`capability-unused`. A `clock.now()` without `requires Clock;` is
refused with `capability-missing`. Both come back as locked-shape
JSON; both carry the source span and a remediation hint with high
confidence.

## Docs

[`docs/README.md`](docs/README.md) — every design document, one per
page, in order. Start with the proposal; the language design, the
semantics, the security model, and the platform model are next; the
IR schema, the diagnostic format, and the grammar are the contracts the
compiler meets.

[`docs/proposal.md`](docs/proposal.md) — the original MVP proposal. The
implementation matches Tier 1–11 of the proposal's roadmap (the async
slice, the `Option`/`List` generics, the `Network` capability,
runtime cancellation, and pure list operations included), plus the
Phase 2 compile-time `match-non-exhaustive` rule. The remaining
Phase 2 items (component composition, `foreign`/`target` blocks)
and beyond are still ahead; the roadmap rows record what each
shipped and what was found.

[`SESSION-LOG.md`](SESSION-LOG.md) — what Tier 1–4 actually built, the
bugs found while building them, and how to reproduce the end-to-end DOM
tests.

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) — the gate, the file budget, the
conventions. [`AGENTS.md`](AGENTS.md) is the one page for writing
cancho-ui **source** (not the compiler); it has the runtime loop and
the rule catalog cross-references a writer needs.

Design lands in `docs/` **before** the code that implements it, and a
claim that turns out wrong is corrected in place there rather than
quietly edited — the roadmap says which.

## Licence

[EUPL-1.2](LICENSE), matching Cancho Core and the rest of the
ecosystem. See `LICENSE` for the notice and where to obtain the full
text in any of the 23 EU languages.