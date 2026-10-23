# Diagnostic format

> **Status: Phase 0.** This document locks the shape of every
> diagnostic and report item in cancho-ui. The first refusal the
> tool emits against a real input pins the schema; the schema
> does not drift between releases.

---

## 1. Position

Every refusal is **a structured value**, not a string. A
diagnostic has fields an agent reads; a human-readable
sentence is parallel.

The shape inherits from Cancho Core's
[`agent-errors.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md):
58 named rule tags, each with a stable code, a span, and a
message. cancho-ui extends the shape with the **guarantee tier**
field, which the security model
([`docs/security-model.md`](../../docs/security-model.md))
introduces.

---

## 2. The diagnostic shape

```text
Diagnostic {
    code: String,                       // stable rule tag, kebab-case
    severity: Severity,                 // Error | Warning
    tier: Tier,                         // Static | Tested | Runtime | Trust | Unknown
    span: Option<Span>,
    subject: Subject,
    message: String,                    // short explanation an agent can read
    expected: Option<Value>,            // when relevant
    actual: Option<Value>,              // when relevant
    remediation: Option<Remediation>,
    related: Vec<Related>,              // cross-references
    notes: Vec<String>,                 // anything else, opt-in
}

Severity = Error | Warning

Tier = Static | Tested | Runtime | Trust | Unknown

Subject =
    | Component { name: String }
    | State { component: String, field: String }
    | Event { name: String }
    | Effect { name: String }
    | Capability { name: String }
    | Lifecycle { component: String, phase: String }
    | Foreign { scope: String, symbol: String }
    | Target { name: String }
    | Accessibility { component: String, role: String }
    | Ir { path: String }

Value =
    | Type(...)
    | Expr(...)
    | String(String)
    | Number(Number)
    | Bool(bool)
    | Null

Remediation {
    description: String,                // one sentence
    suggestion: Option<PatchHint>,       // a structured patch when the compiler can justify one
    confidence: Confidence,             // High | Medium | Low
}

Confidence = High | Medium | Low

Related {
    code: String,
    span: Option<Span>,
    message: String,
}

Span {
    file: String,
    start: Position,                    // 1-indexed line, 1-indexed column
    end: Position,
}
```

### 2.1 Required fields

Every diagnostic carries `code`, `severity`, `tier`, `subject`,
`message`. The rest are present only when the diagnostic is
about them — `span` is `None` for diagnostics that do not come
from a source-located check (e.g. an internal compiler error).

### 2.2 No optional `message`

`message` is always present and always a non-empty string. The
message is what an agent reads first; it is the entry point.

### 2.3 No LLM-generated explanations

The `remediation.suggestion` field, when present, is a
**structured patch hint** produced by the compiler's own
analysis, not by an LLM. The agent may apply the hint; the
compiler re-checks the result. The shape is the same one
Cancho Core uses for its rule-tagged diagnostics.

---

## 3. The rule catalog

The full Phase 1 rule catalog. Each entry's `code` is the
**stable rule tag** used by the diagnostic, by the refusal test
in `examples/refusal/`, and by the evidence report.

| `code` | `subject` | `severity` | `tier` | Meaning |
|---|---|---|---|---|
| `state-unused` | `State` | `Error` | `Static` | A `state` declared but never read or updated |
| `state-after-dispose` | `State` | `Error` | `Static + Runtime` | A transition updating state in a disposed component |
| `state-escapes` | `State` | `Error` | `Static` | A component returning its owned state as an owned value |
| `effect-not-declared` | `Effect` | `Error` | `Static` | A transition body performing an effect the component has not declared |
| `state-not-owned` | `State` | `Error` | `Static` | A transition updating a state field not declared on the component |
| `test-asserts-unspecified` | `Effect` | `Error` | `Tested` | A test asserting an ordering the language does not guarantee |
| `match-non-exhaustive` | `Effect` | `Error` | `Static` | A `match` on an effect result missing an `Err` or `Cancelled` arm |
| `call-unknown` | `Component` | `Error` | `Static` | A call to a function the language does not have (`sqrt(x)`, `pow(a, b)`, a misspelt builtin; `max`, `min`, `abs`, `floor`, `ceil` and `round` exist since issue #43). It used to pass `check` and be emitted as `x = /* unsupported call */;`, a JavaScript syntax error that stopped the whole page. *(Built 2026-10-21, found building the ops console.)* |
| `pattern-literal-unsupported` | `Component` | `Error` | `Static` | A `match` arm whose pattern is `true` or `false`: a bare name is a binding that matches everything, so the arm would always win (there are no literal patterns). *(Built 2026-10-21, issue #26.)* |
| `binding-unknown` | `Component` | `Error` | `Static` | A key that is not a binding of the built-in element it is on (`title:` on a `Button`, `disabled:` on a `Column`). It used to be accepted and dropped or written as a stray attribute. *(Built 2026-10-21, issue #35, `docs/controls.md`.)* |
| `disabled-not-bool` | `Component` | `Error` | `Static` | A `disabled:` that is not `true`, `false` or the name of a `Bool` state field of the component. *(Built 2026-10-21, issue #35.)* |
| `submit-event-invalid` | `Component` | `Error` | `Static` | A `submit:` on a `TextField` naming an event the component does not declare, or one that takes parameters (Enter gives none). *(Built 2026-10-21, issue #36, `docs/controls.md` section 3.)* |
| `event-unknown` | `Component` | `Error` | `Static` | An `on:` naming an event the component does not declare (with or without arguments). It used to compile and kill the page on its first render. *(Built 2026-10-21, issue #41, `docs/row-actions.md`.)* |
| `event-arity-mismatch` | `Component` | `Error` | `Static` | An `on:` whose arguments are not the parameters of the handler: the wrong number of arguments, a bare `on:` on a button for a handler with parameters (a click has none; it used to receive the `MouseEvent`), or a field for a handler with two. *(Built 2026-10-21, issue #41.)* |
| `event-args-unsupported` | `Component` | `Error` | `Static` | `on: Event(..)` with arguments on anything but a `Button`. *(Built 2026-10-21, issue #41.)* |
| `if-else-required` | `Component` | `Error` | `Static` | An `if` expression without `else` (`out = if c { 1 };`): an `if` has a value on both sides. *(Built 2026-10-22, issue #40, `docs/conditionals.md`.)* |
| `if-effect-in-expression` | `Component` | `Error` | `Static` | An effect call (`net.*`, `storage.*`, `clock.*`, `location.*`), `sleep` or `await(..)` in the condition or a branch of an `if`: the effects a handler issues would depend on a value the replay trace has to supply. *(Built 2026-10-22, issue #40.)* The condition of an `if` *statement* may not either (`docs/handler-sequences.md`, #93); the branches of a statement may. |
| `condition-not-bool` | `Component` | `Error` | `Static` | The condition of an `if`, or an operand of `&&`, `\|\|`, `!`, that is known not to be a `Bool` (a number, a string, a list, a `Result`/`Option`). A type the pass cannot see (an event parameter, a `For` binding) is not refused here; replay refuses it when it runs (`replay-type-error`). *(Built 2026-10-22, issue #40.)* Also the `when:` of a view `If { when: .. }` (`docs/view-conditional.md`, issue #90). Also the condition of an `if` statement. |
| `operand-type-mismatch` | `Component` | `Error` | `Static` | An operand of `+` that is known to be a list, a `Result`, an `Option` or a `Request`, or a `Bool` added to a number or a `Bool`. Two lists are joined with `concat(a, b)`. A type the pass cannot see is not refused here; replay refuses it when it runs (`replay-type-error`). *(Built 2026-10-22, issue #39.)* |
| `if-branch-type-mismatch` | `Component` | `Error` | `Static` | The two branches of an `if` have different known types (a number and a string, a Bool and a list). *(Built 2026-10-22, issue #40.)* |
| `comparison-type-mismatch` | `Component` | `Error` | `Static` | A `==`, `!=`, `<`, `<=`, `>` or `>=` whose operand types are both known and are not a pair replay defines: `==`/`!=` need two numbers, two strings or two bools (a number against a string is always false, a list or a `Result` is a replay fault and a reference comparison in the page); the orderings need two numbers or two strings. A type the pass cannot see (an event parameter, a `For` binding) is not refused. *(Built 2026-10-23, issue #95, `docs/comparisons.md`.)* |
| `name-unknown` | `Component` | `Error` | `Static` | A name in a handler, a view expression, a label, an event argument or an `over:` that is neither a state field nor a binding in scope (an event parameter, a `For` binding, a lambda parameter, a `match` binding). It used to pass `check` and blank the page with a `ReferenceError`; the message lists what is in scope. *(Built 2026-10-22, issue #55.)* Also in the `when:` of an `If`. Also in a `block`, where a handler's parameters are not in scope. |
| `tone-invalid` | `Component` | `Error` | `Static` | A `tone:` on a `Text` that is not `"ok"`, `"warn"`, `"error"` or the name of a `String` state field. *(Built 2026-10-21, issue #42, `docs/presentation.md` section 3.)* |
| `layout-value-invalid` | `Component` | `Error` | `Static` | A `gap:` outside 0 to 4 or a `max_width:` outside 10 to 200 on a `Column` or `Row`. *(Built 2026-10-21, issue #42, section 2.)* |
| `block-unknown` | `Component` | `Error` | `Static` | A `run Name;` of a block the component does not declare. *(Built 2026-10-22, issue #93, `docs/handler-sequences.md`.)* |
| `block-duplicate` | `Component` | `Error` | `Static` | Two `block`s of one name in a component. *(Built 2026-10-22, issue #93.)* |
| `block-name-conflict` | `Component` | `Error` | `Static` | A `block` named like a state field or an event of the component. *(Built 2026-10-22, issue #93.)* |
| `block-cycle` | `Component` | `Error` | `Static` | A `block` that runs itself, directly or through other blocks: its expansion would never end. *(Built 2026-10-22, issue #93.)* |
| `block-expansion-too-large` | `Component` | `Error` | `Static` | A handler or block that has more than 10,000 statements once its `run`s are replaced (blocks that run blocks several times multiply). *(Built 2026-10-22, issue #93.)* |
| `input-type-invalid` | `Component` | `Error` | `Static` | A `type:` on a `TextField` that is not one of `text`, `password`, `email`, `search`, `tel`, `url` written as a string literal (a state field, `number`, `checkbox`, `PASSWORD`). `type:` on anything but a `TextField` is `binding-unknown`. *(Built 2026-10-22, issue #76, `docs/controls.md` section 5.)* |
| `input-attribute-invalid` | `Component` | `Error` | `Static` | An `autocomplete:` that is not `on`, `off` or one field name of the HTML autofill list; an `inputmode:` that is not one of none, text, decimal, numeric, tel, search, email, url; a `maxlength:` that is not an integer literal from 1 to 100000; a `name:` that is not 1 to 64 letters, digits, `-`, `_`, `.` starting with a letter; a `rows:` of a `TextArea` that is not an integer literal from 1 to 30. These are bindings of a `TextField` and a `TextArea` (`rows:` of a `TextArea` only); on anything else each is `binding-unknown`. *(Built 2026-10-22, issue #91, `docs/controls.md` section 6.)* |
| `error-binding-invalid` | `Component` | `Error` | `Static` | An `error:` on a `TextField` that is not the name of a `String` state field of the component (a literal, a number, an `Int` or `Bool` field, a name that is no field). On anything but a `TextField` it is `binding-unknown`. *(Built 2026-10-23, issue #76, `docs/controls.md` section 7.)* |
| `checked-binding-invalid` | `Component` | `Error` | `Static` | A `checked:` on a `Checkbox` that is not the name of a `Bool` state field of the component (a literal, a `String` or `Int` field, a name that is no field). On anything but a `Checkbox` it is `binding-unknown`. *(Built 2026-10-23, issue #76, `docs/controls.md` section 9.)* |
| `checkbox-incomplete` | `Component` | `Error` | `Static` | A `Checkbox` without `checked:` (the page would forget the box at the next redraw) or without `on:` (it would tell the program nothing). *(Built 2026-10-23, issue #76, `docs/controls.md` section 9.)* |
| `select-incomplete` | `Component` | `Error` | `Static` | A `Select` without `options:` (the list), `value:` (the `String` field holding the choice) or `on:` (the event). *(Built 2026-10-23, issue #76, `docs/controls.md` section 10.)* |
| `select-binding-invalid` | `Component` | `Error` | `Static` | An `options:` that is not a list of string literals or the name of a `List` state field, or a `value:` of a `Select` that is not the name of a `String` state field. `options:` on anything but a `Select` is `binding-unknown`. *(Built 2026-10-23, issue #76, `docs/controls.md` section 10.)* |
| `route-event-unknown` | `Component` | `Error` | `Static` | A `route Event;` naming an event the component does not handle. *(Built 2026-10-23, issue #75, `docs/routing.md` section 4.)* |
| `route-event-params` | `Component` | `Error` | `Static` | A `route Event;` whose event does not take exactly one parameter (the path). *(Built 2026-10-23, issue #75.)* |
| `route-duplicate` | `Component` | `Error` | `Static` | Two `route` lines in a component: one event receives every path. *(Built 2026-10-23, issue #75.)* |
| `navlink-incomplete` | `Component` | `Error` | `Static` | A `NavLink` without `label:` (its text) or without `to:` (the path). *(Built 2026-10-23, issue #75, `docs/routing.md` section 4.)* |
| `navlink-target-invalid` | `Component` | `Error` | `Static` | A literal `to:` of a `NavLink` that is not a path: it must start with `/`, hold no space, `#`, `"`, `<`, `>`, backtick or character outside printable ASCII, and be at most 2,000 characters. `to:` on anything but a `NavLink` is `binding-unknown`. *(Built 2026-10-23, issue #75.)* |
| `file-field-incomplete` | `Component` | `Error` | `Static` | A `FileField` without `label:` or without `on:`. *(Built 2026-10-23, issue #79, `docs/file-upload.md`.)* |
| `file-accept-invalid` | `Component` | `Error` | `Static` | An `accept:` of a `FileField` that is not a string literal of comma-separated tokens (`.ext`, `type/subtype`, `type/*`; no spaces; at most 20). `accept:` on anything but a `FileField` is `binding-unknown`. *(Built 2026-10-23, issue #79.)* |
| `file-limit-invalid` | `Component` | `Error` | `Static` | A `max_bytes:` of a `FileField` that is not an integer literal from 1 to 1,048,576. On anything else it is `binding-unknown`. *(Built 2026-10-23, issue #79.)* |
| `file-field-in-loop` | `Component` | `Error` | `Static` | A `FileField` drawn inside a `For`: the page keeps the chosen file by the place of the field in the source, and every row would share one. *(Built 2026-10-23, issue #79.)* |
| `parse-nesting-too-deep` | `Component` | `Error` | `Static` | `if` statements nested more than 32 deep: a parse error, not a stack overflow. Only statements are limited so far; see alpibrusl/cancho-ui#99 for expressions and views. *(Built 2026-10-22, issue #93.)* |
| `table-child-invalid` | `Component` | `Error` | `Static` | A `Table` child that is not a `Row` or a `For` that makes `Row`s. *(Built 2026-10-21, issue #42, section 4.)* |
| `capability-missing` | `Capability` | `Error` | `Static` | A component using an effect without `requires Capability` |
| `capability-unused` | `Capability` | `Error` | `Static` | A `requires` declaration with no reachable use |
| `capability-insufficient` | `Capability` | `Error` | `Static` | A `requires` narrower than the effect's needs |
| `capability-narrowing-illegal` | `Capability` | `Error` | `Static` | A bound on a capability that takes none, a `Credential` with no name or an illegal name or host, a `Network` bound that is not `name[:port]`, or a `Location` bound that is not `"coarse"` or `"fine"`. *(A bound that is not a string literal is `parse-unexpected-token`; built 2026-10-20, `docs/adapters.md` section 10.)* |
| `credential-ambiguous` | `Capability` | `Error` | `Static` | A component declaring more than one `Credential`: a call cannot choose between them. *(Built 2026-10-21, `docs/adapters.md` section 18.)* |
| `credential-host-not-allowed` | `Capability` | `Error` | `Static` | A host named in `Credential("name", "host")` that the component's `Network` does not allow. *(Built 2026-10-21.)* |
| `credential-needs-mount-options` | `Capability` | `Error` | `Static` | A build with auto-mount (or `--pwa`) of a component declaring `Credential` that never calls `credential.set`: nothing can hand it a token; use `--no-auto-mount` and `mount_X(root, { credentials })`. *(Built 2026-10-21.)* |
| `credential-name-invalid` | `Capability` | `Error` | `Static` | A `credential.set(..)` or `credential.clear(..)` whose first argument is not a string literal naming the component's declared `Credential`, or in a component that declares none. *(Built 2026-10-23, issue #83, `docs/bearer-token.md`.)* |
| `capability-bound-violated` | `Capability` | `Error` | `Static` | A `net.get` of a literal URL whose host the component's `Network` does not allow (the runtime guard answers `Err("blocked: ...")` for the URLs it cannot see) |
| `schedule-event-unknown` | `Schedule` | `Error` | `Static` | A `start` or `every` naming an event the component does not declare |
| `schedule-event-params` | `Schedule` | `Error` | `Static` | A `start` or `every` naming an event that takes parameters |
| `schedule-period-invalid` | `Schedule` | `Error` | `Static` | An `every` period outside 100..86,400,000 ms |
| `schedule-duplicate` | `Schedule` | `Error` | `Static` | The same `start`, or the same `every` period and event, twice in a component |
| `accessibility-role-undefined` | `Accessibility` | `Error` | `Static` | A view primitive without a `role` |
| `accessibility-label-empty` | `Accessibility` | `Error` | `Static` | An interactive primitive with empty `label` |
| `accessibility-keyboard-missing` | `Accessibility` | `Error` | `Static` | An interactive primitive without a keyboard action for Enter |
| `unsupported-on-target` | `Target` | `Error` | `Static` | A `target` block built for a target not in the build |
| `target-not-in-build` | `Target` | `Error` | `Static` | A `target` block for a target not in the build configuration |
| `target-divergence-unproved` | `Target` | `Error` | `Static` | (Phase 4) Two `target` blocks whose semantics differ in a way the build cannot prove equivalent |
| `foreign-scope-missing` | `Foreign` | `Error` | `Static` | A `foreign` function without a `from "scope"` |
| `foreign-interface-missing` | `Foreign` | `Error` | `Static` | A program calling a foreign function without an adapter |
| `name-reserved` | `Ir` | `Error` | `Static` | A binding named `component`, `state`, `view`, etc. |
| `evidence-over-claim` | `Ir` | `Error` | `Tested` | A report claiming a guarantee the implementation does not enforce |
| `ir-version-unsupported` | `Ir` | `Error` | `Runtime` | An IR consumer does not recognize the schema version |
| `internal` | `Ir` | `Error` | `Unknown` | A backend or compiler internal error (mirrors Cancho Core's `internal`) |

### 3.2 Replay rules (`cancho-ui test`)

`cancho-ui test` reports failures in its own JSON shape
([`docs/replay.md`](../../docs/replay.md) §7), tier `tested`, with these stable
tags: `test-state-mismatch`, `test-expect-mismatch`, `test-fault-missing`, `test-fault-mismatch`, `trace-no-assertions`,
`trace-malformed`, `trace-credential-unknown`, `trace-schema-unsupported`, `trace-unknown-key`,
`trace-bad-step`, `trace-bad-value`, `trace-unknown-component`,
`trace-unknown-event`, `trace-params-arity`, `trace-value-too-long`, `trace-value-not-an-option`, `trace-unknown-state-field`,
`trace-unknown-request`, `trace-request-not-pending`,
`trace-request-not-scriptable`, `trace-bad-response`,
`trace-event-after-dispose`, `trace-navigate-invalid`, `trace-history-empty`, `trace-value-not-a-file`, `replay-unsupported`, `replay-tick-limit`, `replay-route-limit`, `replay-type-error`,
`replay-unknown-identifier`, `replay-no-match-arm`, `replay-non-finite`,
`replay-number-format`. (`replay-non-finite` is also what the emitted page throws for the same result: `docs/semantics.md`,
"Arithmetic is finite".) Their meanings are in `docs/replay.md` §7.1. A program that
does not compile is still refused with the §3 rule that refuses it (exit `1`).

### 3.1 What is not in the catalog

- **No warnings.** A diagnostic is `Error`. A warning would
  either be ignored or acted on; if ignored it is not a
  guarantee, if acted on it is a refusal. The same discipline
  Cancho Core applies — "a diagnostic is a refusal."
- **No stylistic hints.** "Consider this instead" messages are
  refused as a category; they go in `remediation.suggestion`
  with `confidence: Low`.
- **No "this may be undefined" hedge.** The Phase 1 contract
  is either the language defines it, or it is `unknown`. A
  refusal that says "may" is refused with `evidence-over-claim`.

---

## 4. The `--output json` shape

The JSON form of a diagnostic is what an agent reads.

```json
{
  "code": "capability-missing",
  "severity": "Error",
  "tier": "Static",
  "subject": {
    "kind": "Capability",
    "name": "Network"
  },
  "span": {
    "file": "examples/agent_tasks/add_capability.cui",
    "start": { "line": 12, "column": 5 },
    "end": { "line": 12, "column": 35 }
  },
  "message": "Component 'SearchForm' performs an effect that requires Network but does not declare it. Add `requires Network` to the component header.",
  "expected": "requires Network",
  "actual": "absent",
  "remediation": {
    "description": "Add `requires Network` to the component header.",
    "suggestion": null,
    "confidence": "High"
  },
  "related": [],
  "notes": []
}
```

### 4.1 Stable field names

Field names do not change between schema versions. A new
field is additive (minor version bump); a renamed field is a
breaking change (major version bump). Consumers ignore unknown
fields.

### 4.2 Stable field order

The serialization order is the declaration order in §2.
Consumers can rely on it for diff stability.

### 4.3 No embedded newlines in `message`

The `message` field is one line. A multi-line explanation goes
in `notes[]`, with one note per entry.

---

## 5. The text form

The human-readable form is parallel. `cancho-ui check` without
`--output json` prints one block per diagnostic:

```
error[capability-missing]: Component 'SearchForm' performs an
effect that requires Network but does not declare it. Add
`requires Network` to the component header.
   --> examples/agent_tasks/add_capability.cui:12:5
   |
12 |     let r = net.request("GET", "/api/items").await()
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
help: Add `requires Network` to the component header.
   |
 2 | component SearchForm {
 3 |     requires Network
   |     ++++++++++++++++
```

The text form is **derived from the JSON form**, not the other
way around. The same discipline Cancho Core's
[`agent-cli.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md)
follows: the binary's output is the source of truth.

---

## 6. The evidence report

`cancho-ui report` produces a JSON document with five sections,
one per tier. Each section is a list of structured items.

```json
{
  "schema": "cancho-ui-report",
  "version": "0.1.0",
  "static": {
    "checks_run": ["capability-missing", "state-not-owned", ...],
    "checks_passed": 42,
    "checks_failed": 1,
    "diagnostics": [
      // structured Diagnostic objects
    ]
  },
  "tested": {
    "tests_run": ["counter_increments", "search_cancels_on_dispose", ...],
    "tests_passed": 17,
    "tests_failed": 0,
    "details": [
      // structured test results
    ]
  },
  "runtime": {
    "checks_performed": ["capability-grant", "dispose-cancellation", ...],
    "checks_passed": 31,
    "checks_failed": 0,
    "details": []
  },
  "trust": {
    "boundaries": [
      { "from": "browser", "symbol": "fetch", "tier": "Trust" },
      { "from": "browser:storage", "symbol": "localStorage.getItem", "tier": "Trust" }
    ],
    "count": 2
  },
  "unknown": {
    "items": [
      { "description": "Cross-component event ordering", "tier": "Unknown" }
    ]
  }
}
```

### 6.1 What the report never contains

- A `trust` boundary labeled as anything other than `Trust`.
- A `static` check that did not actually run (it goes in
  `unknown`).
- A `tested` test that was skipped (it is listed with
  `outcome: "skipped"`, not as `passed`).
- An `evidence-over-claim` item; that is a refusal against the
  report itself, caught by the conformance suite.

---

## 7. The semantic-diff output

> **Corrected in place (Phase 3).** The sketch below was written before
> `cancho-ui diff` existed and is **not** the shape it emits. The built shape
> is specified in [`docs/semantic-diff.md`](../docs/semantic-diff.md) §3:
> `capabilities` / `components` / `authority` (`expanded | reduced |
> unchanged`) replace the flat `added_capabilities` lists;
> `changed_transitions` with prose `from` / `to` is replaced by per-event
> `writes_added`, `writes_removed` and a rendered `body`, because a prose
> description of a transition is not derivable from the IR;
> `added_foreign_dependencies` is not emitted because the IR has no foreign
> nodes yet (the report says so in `not_compared`); `changed_accessibility`
> is folded into the per-component `view` element list. Exit code `5` means
> "differs" (`1` stays "refused"). The sketch is kept below as the
> original intent.

`cancho-ui diff <base> <candidate>` produces a JSON document
listing the semantic changes between two IRs.

```json
{
  "schema": "cancho-ui-diff",
  "version": "0.1.0",
  "added_capabilities": ["Network"],
  "removed_capabilities": [],
  "changed_transitions": [
    {
      "component": "SearchForm",
      "event": "SearchRequested",
      "from": "synchronous local search",
      "to": "async net.search with cancellation",
      "tier_change": null
    }
  ],
  "added_foreign_dependencies": [
    { "from": "browser", "symbol": "AbortController" }
  ],
  "changed_accessibility": [
    { "component": "SearchForm", "from": "role: textbox", "to": "role: search" }
  ],
  "added_components": [],
  "removed_components": ["OldSearchForm"]
}
```

A normal source diff is parallel (line-by-line). The semantic
diff is what an agent uses to decide whether a change is
safe to apply; the source diff is for a human to read.

---

## 8. Versioning

| Artifact | Schema | Version | Bump policy |
|---|---|---|---|
| Diagnostic | `cancho-ui-diagnostic` | `0.1.0` (Phase 1) | Major = breaking field; minor = additive |
| Evidence report | `cancho-ui-report` | `0.1.0` (Phase 1) | Same |
| Semantic diff | `cancho-ui-diff` | `0.1.0` (Phase 2) | Same |

A consumer that does not recognize the schema or version
refuses with `ir-version-unsupported` for IR inputs and
`schema-version-unsupported` for report / diff inputs.

---

## 9. What Phase 1 must add to this sketch

The Phase 1 delta is small and explicit:

- The full rule catalog in §3, each rule with at least one
  refusal fixture in `examples/refusal/`.
- The JSON serializer for `Diagnostic` in §4.
- The text formatter for §5, derived from the JSON form.
- A `cancho-ui report --output json` command that produces
  the shape in §6.
- A `tests/diagnostics.rs` snapshot test that pins every
  fixture's diagnostic output, so the schema does not drift.

Anything else is Phase 2+. Phase 1 does not add a rule that is
not in §3, and does not modify a rule's `code` without a major
schema bump.