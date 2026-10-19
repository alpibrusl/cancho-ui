---
name: cancho-ui
description: Check, build, inspect and compare programs in cancho-ui, a declarative UI language whose compiler refuses a component that uses authority (network, clock) it did not declare. Use when the user wants a cancho-ui component written or changed, wants to know what a component is allowed to do, or wants to know whether a change to a .cui program widens its authority.
---

# cancho-ui

cancho-ui compiles `.cui` programs to JavaScript and DOM. A component declares the authority it needs (`requires Clock;`), and the compiler refuses the program when the code and the declaration disagree. Every command has a JSON form, so you read results as data and match on the stable `code`, not on prose.

## Rules (always)

- Never use `sudo`. Do not install system packages without asking.
- Paths that go into a shell command may hold only letters, digits and `. _ / + : = @ % , -`: no spaces, no `..`.
- Do not retry a failing command in a loop, and do not edit the repository to make a command pass. Report what failed, with the exact error.
- Do not claim more than the tool checked. `diff` reports syntax, not values, and lists what it did not compare in `not_compared`; `test` lists what it did not replay in `not_claimed`.

## 1. Install (once)

Follow `docs/agent-prompt.md` in the cancho-ui repository: clone it, `cargo build --release`, run `cargo test --workspace`, and use `target/release/cancho-ui`.

## 2. Write or change a component

Read `AGENTS.md` for the language, then write the `.cui` file. Check it:

```
cancho-ui check FILE.cui --output json     # exit 0 accepted, 1 refused (a JSON array of diagnostics)
```

On a refusal, read the `code` (`capability-missing`, `capability-unused`, `match-non-exhaustive`, ...), the `span` and the `remediation`. Fix that and check again. Do not regenerate the whole file because of one refusal.

## 3. What may it do?

```
cancho-ui inspect effects FILE.cui NAME --output json   # declared capabilities and every reachable effect with its span
cancho-ui inspect component FILE.cui NAME --output json
cancho-ui report FILE.cui --output json                  # checked vs trusted
```

## 4. Did my change widen authority?

```
cancho-ui diff BASE.cui CANDIDATE.cui --output json
```

Exit codes: `0` no semantic change, `5` the programs differ, `1` a file did not compile (a located refusal), `2` usage, `3` environment. Read `authority`: `expanded` (a new capability), `reduced` or `unchanged`. If it is `expanded`, say so to the user before going on.

## 5. Replay a trace

```
cancho-ui test FILE.cui TRACE.json --output json
```

A trace is a JSON list of steps (`event`, `resolve` a scripted `net.search` answer, `advance` the fake clock, `dispose`), each with the `state` and `expect` it should leave behind. Exit `0` passed, `4` a test failed (an expectation, a malformed trace, or something replay refuses: read each failure's `rule`, `path` and `span`), `1` the program did not compile, `2` usage, `3` environment. To test a stale response, `resolve` the older request after the newer one and expect `"outcome": "dropped"`. Examples: `examples/*/*.trace.json`. A pass means the interpreter and the trace agree; it does not cover the view, timing or the real network (see `not_claimed` and `docs/replay.md`).

## 6. Build

```
cancho-ui build FILE.cui -o out.js      # JavaScript and DOM; the page owns the styling
cancho-ui build FILE.cui --pwa -o DIR   # an installable web app for a phone (docs/mobile-backend.md)
```

## More

`README.md`, `docs/semantic-diff.md`, `docs/replay.md`, `docs/language-design.md`, `docs/security-model.md`, `spec/diagnostics/README.md`.
