# eval — evidence for the claims made about cancho-ui

> **Status: Phase 0 scaffold.** The harness, pre-registrations
> and raw data land in Phase 3, when there is something to
> measure. This document is the **plan**, mirroring Cancho Core's
> [`eval/`](https://github.com/alpibrusl/cancho/tree/main/eval)
> directory.

---

## 1. What this directory is

A working record of every experiment that could refute a claim
cancho-ui makes. The shape is the same as Cancho Core's eval:
each claim has a numbered experiment (E1, E2, …), a
pre-registration in a dated file committed before the run, and
a raw-data file produced by the run.

Nothing here is a result until Phase 3 produces a runnable
harness. Until then, this directory is the **claim catalog** and
the protocol, nothing more.

---

## 2. The protocol (inherited from Cancho Core)

The protocol is the same one Cancho Core uses:

- **Pre-register tasks, metrics, and thresholds in a dated file
  before running**; commit it.
- **Publish the raw per-run data** (one JSON line per run: task,
  language, model, context, iterations, tokens, outcome).
- **Tasks are outside what the agent can see.** Nothing in
  `examples/` or in any product repo is a visible task.
- **Oracles are independent.** Expected outputs come from
  Python's standard library or from the language's own test
  suite, never from code written for cancho-ui.
- **At least five runs per task and model; report the spread,
  not only the mean.**
- **Report losses.**
- **Threats to validity, stated up front**: the authors designed
  the language and the tasks; the guide (`AGENTS.md`, when it
  exists) and most of the code are written with help from
  agents.

---

## 3. The Phase 0 claim catalog

The claims cancho-ui intends to make once it has something to
measure. Each is a numbered experiment.

| # | Claim | Phase | Where the experiment runs |
|---|---|---|---|
| E1 | An agent can build a cancho-ui program from the guide | 3 | `eval/e1_agent_writes/` |
| E2 | A model can repair a seeded defect | 3 | `eval/e2_agent_repairs/` |
| E3 | An agent can extend a cancho-ui program without adding authority | 3 | `eval/e3_no_authority_expansion/` |
| E4 | The semantic diff names what changed | 3 | `eval/e4_semantic_diff/` |
| E5 | The evidence report names what is and is not guaranteed | 3 | `eval/e5_evidence_report/` |
| E6 | The capability check rejects the documented cases | 1 | `examples/refusal/capability_*.cui` (Phase 0 deliverable; counted here) |
| E7 | The diagnostic format is stable | 1 | `tests/diagnostics.rs` snapshot suite (Phase 0 deliverable; counted here) |
| E8 | Identity is stable across reformatting | 2 | `eval/e8_identity/` |
| E9 | Cross-platform behavior is equivalent for the supported subset | 4 | `eval/e9_cross_platform/` |
| E10 | The mobile-backend choice is evidence-based | 4 | `docs/platform-model.md` §2 (Phase 0 decision record; measurement in Phase 4) |

E1–E5 are the agent-facing benchmark; E6–E10 are the
implementation's own self-evidence.

---

## 4. The agent-facing benchmark (Phase 3)

The harness runs the proposal §10's example tasks plus the same
shape Cancho Core's `eval/PREREGISTRATION-E2-E3.md` uses:

| Task | What the agent does |
|---|---|
| Add a validation rule | Extend a form with a new validation constraint |
| Fix a stale-response bug | Repair a fixture with a known stale-response bug |
| Add a new state to a search flow | Extend `SearchStatus` with a new variant and handle it |
| Introduce a capability-gated operation | Add a new `requires` capability and gate an effect on it |
| Refactor a component without increasing its authority | Move code around; assert the authority report does not change |

Each task has:

- A **pre-registration** (committed before the run), naming the
  metric and the threshold.
- A **fixed oracle** (Python's `unittest` against the same
  fixture, or the cancho-ui test suite's own pass/fail).
- A **conventional baseline** (React or Svelte — Phase 3 picks
  one based on what an agent would write by default).

### 4.1 Metrics

Same as the proposal §10:

- Task completion and functional correctness.
- Regression rate.
- Number of agent repair iterations.
- Human interventions required.
- Unintended capability expansion (the central safety claim).
- Test coverage of specified behaviors.
- Time to diagnose and fix seeded bugs.

### 4.2 What the benchmark does not measure

- The "AI writes cancho-ui better" claim. The benchmark
  measures the agent's *repair* and *extension* loops on
  existing cancho-ui programs; writing a new program from a
  blank page is E1, which is harder to fix and harder to
  measure.
- A claim that the language is "easy." The benchmark measures
  whether the agent makes correct changes with fewer
  regressions, not whether the language is pleasant to read.

---

## 5. The implementation-self evidence (Phase 1+)

The implementation's own evidence lives in the test suite and
the refusal fixtures.

### 5.1 E6 — capability check rejects the documented cases

Counted by `examples/refusal/`. Each `capability_*` fixture is
a refusal test; the suite's pass criterion is that every named
rule fires exactly when its fixture is checked.

### 5.2 E7 — diagnostic format is stable

`tests/diagnostics.rs` (Phase 1) snapshots the JSON output of
every refusal fixture. A schema change that drifts the
serialization fails the snapshot test.

### 5.3 E8 — identity is stable across reformatting

`eval/e8_identity/` (Phase 2) re-runs `cancho-ui ids` before and
after `cancho-ui fmt` on every fixture. The hash is the
content-addressable identifier, the same way Cancho Core's
[`cancho ids`](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md)
defines identity.

### 5.4 E9 — cross-platform behavior is equivalent

`eval/e9_cross_platform/` (Phase 4) runs the same fixture on
both web and mobile backends. The two outputs are compared
event-by-event; the comparison is a separate test from the
fixture itself.

### 5.5 E10 — the mobile-backend choice is evidence-based

`docs/platform-model.md` §2 records the decision criteria.
Phase 4 measures each candidate against the criteria and
publishes the comparison in `eval/e10_mobile_choice/`.

---

## 6. What Phase 0 must add to this directory

- This document.
- A `data/` subdirectory with `.gitkeep`-equivalent files for
  E1–E10 (each experiment's eventual raw data file).
- A `harness/` subdirectory with the eventual Phase 3 harness
  skeleton (not runnable; just the directory).

Nothing else. The harness, pre-registrations, and raw data
land in Phase 3 with the experiment runs.

---

## 7. What this directory refuses to claim

- That cancho-ui has "an advantage" over conventional UI
  stacks. The benchmark either supports or refutes that claim
  with measurements; a paragraph asserting it without
  measurements is refused.
- That the benchmark is exhaustive. Five tasks is a starting
  point; the harness is open to extension.
- That any individual agent's run is the truth. The spread
  matters; one run is a sample, not a finding.
- That the baseline is the best conventional stack. The
  baseline is what an agent would write by default, with the
  default assumptions; a future run may add a different
  baseline.