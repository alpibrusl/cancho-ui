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
| `capability-missing` | `Capability` | `Error` | `Static` | A component using an effect without `requires Capability` |
| `capability-unused` | `Capability` | `Error` | `Static` | A `requires` declaration with no reachable use |
| `capability-insufficient` | `Capability` | `Error` | `Static` | A `requires` narrower than the effect's needs |
| `capability-narrowing-illegal` | `Capability` | `Error` | `Static` | (Phase 2+) A narrowed literal that is not a string literal |
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