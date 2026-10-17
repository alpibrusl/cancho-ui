# cancho-ui

A declarative UI language and toolchain for applications whose
behavior, capabilities, and trust boundaries an AI agent can read and
verify. Compiles one source model to a web backend first and a mobile
backend later, on top of an existing browser, runtime, and platform
ecosystem — not as a replacement for them.

## Position

> A UI language and toolchain for applications that AI agents can
> change with explicit behavioral contracts and verifiable evidence.

This is a **research-backed compiler and agent-workflow project**, not
initially a broad UI framework. The first proof is narrow and
falsifiable — an agent modifies a small application, the compiler
checks state and authority constraints, tests replay the relevant
behavior, and the evidence report clearly identifies what is and is
not guaranteed.

The proposal itself is honest about what this is not:

- Not a replacement for JavaScript, TypeScript, Swift, or Kotlin.
- Not a system that proves third-party or native code safe.
- Not a cross-platform solution that is equivalent merely because it
  shares source code.

The strongest product story is not "AI writes this language better."
It is: **the toolchain gives AI agents a smaller, more explicit
space in which to make changes — and gives humans evidence about
what those changes can do.** That claim is meaningful only if a
benchmark supports it.

## Where this sits

```
                  cancho (systems language)
                  ─────────────────────────
                  capabilities, effect rows, IR,
                  authority reports, rule-tagged refusals
                            │
                            │  documented, versioned
                            │  integration contract
                            │
                  cancho-ui (UI language)
                  ─────────────────────────
                  components, state, events, effects,
                  capabilities, lifecycle, typed UI IR
                            │
                  ┌─────────┴─────────┐
                  │                   │
            web backend          mobile backend
         (DOM + browser)      (cross-platform runtime
                               or explicit native codegen)
```

Cancho-UI depends on [cancho](https://github.com/alpibrusl/cancho)'s
type/effect semantics, capability model, and machine-readable
diagnostic format, through a versioned contract rather than unstable
compiler internals.

## Status

**Phase 0 — design contract.** This repository is currently
proposal- and design-stage. No compiler, runtime, or backend exists
yet. The exit gate is the four documents under
[`docs/`](docs/README.md), a written-down set of valid and invalid
examples under [`examples/`](examples/), the typed UI IR sketch under
[`spec/ir-schema/`](spec/ir-schema/), the machine-readable diagnostic
format under [`spec/diagnostics/`](spec/diagnostics/), and an explicit
guarantees/non-guarantees table in
[`docs/semantics.md`](docs/semantics.md).

Phase 1 (parser + minimal type checker + typed UI IR + Counter-style
demo on the web backend) does not start until those exist and the
cancho integration contract is real. See
[`ROADMAP.md`](ROADMAP.md) for the phasing, exit criteria, and
stop/rethink conditions.

## Building

**There is nothing to build yet.** The repository is docs only. Once
Phase 1 lands:

```sh
# (placeholder — will land with Phase 1)
cancho-ui check <file.cui>...                 # refuse, or say nothing
cancho-ui build --target web <file.cui>       # produce JS+DOM output
cancho-ui test <file.cui>...                  # deterministic replay
cancho-ui inspect component <id>              # machine-readable
cancho-ui inspect effects <id>                # machine-readable
cancho-ui diff <base> <candidate>             # semantic diff
cancho-ui report                              # evidence report
```

## Docs

[`docs/README.md`](docs/README.md) is the index. Start there.

Every design document cites the cancho Core documents it depends on,
so a reader can see which guarantees are inherited, which are
re-stated, and which are net-new for cancho-ui.

## Licence

[EUPL-1.2](LICENSE), matching the rest of the ecosystem.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) — the gate, the file budget,
and the conventions. A claim that turns out false is corrected in
place in the document that made it, rather than quietly edited.