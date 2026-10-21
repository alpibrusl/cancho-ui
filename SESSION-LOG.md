# Tier 1 — Session log

> What this session built, what works, what's deliberately not
> built, and how to reproduce. Phase 0 (design docs) is on `main`;
> this file is the record of the Phase 1 implementation that
> followed.

---

## 1. What got built

A working Tier 1 canchor-ui compiler end-to-end:

| Step | Crate | What it does |
|---|---|---|
| Lex | `cancho-ui-syntax` | Source → tokens, with span info, line/column tracking |
| Parse | `cancho-ui-syntax` | Tokens → AST (File / Component / ViewBlock / ViewNode) |
| Type-check | `cancho-ui-types` | Reserved-word, PascalCase, duplicate-component checks |
| Lower | `cancho-ui-ir` | AST → typed UI IR, stable per-component ID, span preserved |
| Serialize | `cancho-ui-ir` | IR → canonical JSON (Phase 2 pins the exact shape) |
| Emit | `cancho-ui-codegen-web` | IR → JavaScript + DOM ES module |
| CLI | `cancho-ui` | `check` and `build` subcommands with stable exit codes |

### What compiles today

The full Phase 1 grammar:

```text
file        := [ module_decl ] { component_decl }
module_decl := "module" identifier ";"
component_decl := "component" identifier "{" view_clause "}"
view_clause := "view" "{" view_node "}"
view_node   := element_node | text_node
element_node := identifier "{" [ "role" ":" identifier ] [ "label" ":" string_literal ] { view_node } "}"
text_node   := string_literal
```

Built-in primitive element kinds: `Column`, `Row`, `Text`. Other
PascalCase identifiers are user-defined components.

Refusal rules active today (out of the 22-rule Phase 1 catalog in
`spec/diagnostics/README.md`):

| Code | What it catches |
|---|---|
| `lex-unexpected-char` | Bad character in source |
| `lex-unterminated-string` | String literal never closes |
| `lex-unknown-escape` | Escape other than the six Cancho Core allows |
| `parse-unexpected-token` | Token where the grammar expected another |
| `parse-unexpected-eof` | File ends in the middle of a construct |
| `type-not-pascal-case` | Component name starts with lowercase |
| `type-not-pascal-case-kind` | Element kind starts with lowercase |
| `name-reserved` | Reserved word used as a component name |
| `type-duplicate-component` | Two components with the same name in one file |

The other 13 Phase 1 rules (state, requires, events, capabilities,
match, async, foreign, target, accessibility-mandatory) are
designed in `spec/diagnostics/README.md` §3 and **land in Phase 2**
together with the grammar that triggers them.

---

## 2. Verification

```
$ cargo test --workspace
test result: ok. 9 passed (cancho-ui-syntax)
test result: ok. 4 passed (cancho-ui-types)
test result: ok. 1 passed (cancho-ui-ir)
test result: ok. 5 passed (cancho-ui-codegen-web)
test result: ok. 5 passed (cancho-ui integration)
```

19 tests, all green.

End-to-end DOM smoke test (Node + jsdom):

```
$ cd /tmp && node test-hello.js
banner role: banner
banner aria-label: hello banner
banner class: cui-col
heading role: heading
heading text: hello, world
banner innerHTML: <span role="heading">hello, world</span>
OK: hello world rendered correctly
```

The emitted `examples/hello/hello.js`, when loaded into a real DOM,
produces exactly the structure the cancho-ui source describes.

---

## 3. What's deliberately NOT in Tier 1

These are Phase 1 contracts in `docs/proposal.md` and the Phase 0
specs, deferred to Phase 2+ because each one is a real piece of
work, not a one-line change:

| Not yet | Where it goes |
|---|---|
| `state count: Int = 0` | Phase 2 — reactive state |
| `on Increment { ... }` | Phase 2 — events and event handlers |
| `requires Network` | Phase 2 — capability checking |
| `match Ok(v) => { ... }` | Phase 2 — Result/Option types |
| `await()` on `Request[T]` | Phase 2 — async with cancellation |
| `foreign { fn ... from "browser" }` | Phase 2 — trust boundaries |
| `target web { ... } / target mobile { ... }` | Phase 4 |
| `state-unused`, `state-not-owned`, `state-after-dispose` | Phase 2 — with state |
| `capability-missing`, `capability-unused` | Phase 2 — with capabilities |
| `match-non-exhaustive` | Phase 2 — with `match` |
| `accessibility-role-undefined`, `accessibility-label-empty`, `accessibility-keyboard-missing` | Phase 2 — when interactive primitives land |
| `unsupported-on-target` | Phase 4 |
| `foreign-interface-missing` | Phase 2 — with foreign blocks |
| Source maps | Phase 2 |
| `cancho-ui inspect component` / `inspect effects` | Phase 2 |
| `cancho-ui diff` | Phase 3 |
| `cancho-ui report` | Phase 2 |
| `cancho-ui test` | Phase 2 |
| `cancho-ui skill` / `introspect` | Phase 2 |
| `--output json` on `check` and `build` | Phase 2 |
| Multi-file modules with `import` | Phase 2 |
| Style sheets in the language | Phase 6 |

---

## 4. How to reproduce

From the repo root:

```sh
cargo test --workspace
cargo run -p cancho-ui --bin cancho-ui -- build examples/hello/hello.cui -o examples/hello/hello.js
cargo run -p cancho-ui --bin cancho-ui -- check examples/hello/hello.cui
cargo run -p cancho-ui --bin cancho-ui -- --version
```

To see the hello world in a real browser:

```sh
# (any static server pointing at examples/hello/ works)
cd examples/hello
python3 -m http.server
# then open http://localhost:8000/hello.html
```

To run the DOM smoke test:

```sh
cd /tmp && node test-hello.js
# (requires `jsdom` installed via npm)
```

---

## 5. Open questions / next session

1. **Tier 2 (Counter).** Add `state`, `on`, `requires`, `match`,
   `await`, `foreign`, the capability checker, and the event
   wiring in the JS emitter. Probably 1–2 sessions of focused
   work; lands the full Phase 1 exit criteria (10 valid + 10
   invalid fixtures, search-able list demo, the full rule
   catalog).

2. **`cancho-ui inspect`.** The agent-facing tooling from the
   proposal §7 needs the IR to be inspectable. With the IR
   already shaped, this is mostly a CLI subcommand and a
   serialiser choice (`--output json`).

3. **The cancho-IR integration contract.** cancho Core's
   compiler IR is what cancho-ui eventually consumes; this
   hasn't been carved out yet. Without it, cancho-ui is a
   standalone compiler; with it, cancho-ui becomes a cancho
   package.

4. **Source maps.** Phase 2 deliverable; the IR already carries
   spans so the work is plumbing, not new design.

5. **The `name-reserved` rule's edge.** `view` and `component`
   are hard keywords in the parser; everything else in the
   reserved list (`state`, `on`, `requires`, ...) lexes as
   `Ident` and is caught by the type checker. Phase 2 might
   want to make the parser reject them all at parse time for
   consistency, but that means parser and type checker both
   need to know the full reserved set. The current split
   (parser rejects hard keywords, type checker rejects the
   rest) is fine for Phase 1.

---

## 6. Files in this session

```
cancho-ui/
  Cargo.toml                                      # workspace
  crates/
    cancho-ui-syntax/                             # lexer, parser, AST
      src/lib.rs                                  # crate root
      src/ast.rs                                  # AST types
      src/lexer.rs                                # hand-written lexer
      src/parser.rs                               # hand-written parser
      src/span.rs                                 # Span / Position
    cancho-ui-types/                              # type-check
      src/lib.rs
      src/types.rs                                # Type, PrimitiveType
      src/check.rs                                # Phase 1 validation
    cancho-ui-ir/                                 # typed UI IR
      src/lib.rs
      src/ir.rs                                   # IR types
      src/lower.rs                                # AST → IR
      src/serialize.rs                            # canonical JSON
    cancho-ui-codegen-web/                        # JS + DOM emitter
      src/lib.rs
      src/emit.rs                                 # IR → ES module (module, component, statements)
      src/expr.rs                                 # expressions and builtin calls
      src/view.rs                                 # the view walk and component slots
      src/adapters.rs                             # the JavaScript adapters copied in when used
      src/lists.rs                                # the list and text builtins' helpers
    cancho-ui/                                    # CLI binary
      src/main.rs                                 # cancho-ui check / build
      src/pipeline.rs                             # lex → parse → IR → JS
      tests/integration.rs                        # 5 snapshot / unit tests
  examples/hello/
    hello.cui                                     # the source
    hello.html                                    # browser harness
    hello.js                                      # the generated output
    (generated; re-emitted by `cancho-ui build`)
```