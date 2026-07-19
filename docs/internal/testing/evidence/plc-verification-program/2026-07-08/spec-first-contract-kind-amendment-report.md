# Spec-First Contract-Kind Amendment Report

Date: 2026-07-08
Reviewer target: Claude
Status: docs-only amendment ready for review

## Why This Amendment Exists

The verification program already required specifications before tests, but the
latest design discussion made the next step more precise: a bug fix or feature
should first update the structured invariant/spec record, then tooling should
derive the required tests and hostile inputs from that record.

The amendment folds that model into the reviewed document set without changing
product code or existing tests.

Goal: make shortcuts loud. A tool cannot prove that a human chose the correct
spec, but it can prevent quiet invention of behavior, skipped cases, weakened
case tables, hand-written safety evidence, and unmapped file changes.

## Files Changed

- `docs/internal/testing/checklists/plc-verification-program/README.md`
  - Adds the spec-first planner pilot to the implementation order.
  - Names `plan_tests.py`, `gen_cases.py`, `prove.py`, and the dev-only
    `verification-cases` helper.
  - Keeps runtime fault scenarios out of v1 case generation.

- `docs/internal/testing/checklists/plc-verification-program/policy.md`
  - Expands the code-change discipline from "tests before code" to:
    spec/invariant update, planner requirements, case derivation, red/protective
    proof, then implementation.
  - Adds default-deny rules for unmapped files, uninventoried areas, unknown
    risk, and missing behavior rows.
  - Defines typed outcome semantics for accept and reject cases.
  - Records which bypass is caught by which layer.

- `docs/internal/testing/checklists/plc-verification-program/metadata-model.md`
  - Adds `contract_kind` to invariant records.
  - Defines allowed contract kinds:
    `decision_table`, `state_machine`, `protocol_trace`, `fault_scenario`,
    `ui_journey`, `security_policy`, `perf_budget`, and `release_matrix`.
  - Adds v1 `decision_table` behavior rows for single-input, data-shaped
    contracts.
  - Adds optional test-catalog `case_file` and `case_file_digest` fields.
  - Adds committed case-file and generated case-artifact record shapes.
  - Adds validator fail-closed rules for stale digests, missing behavior rows,
    missing oracles, skipped cases, and manual safety-critical red/green
    evidence.

- `docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md`
  - Declares coverage dimensions as the canonical case-family vocabulary.
  - Separates v1 data-shaped decision-table cases from Phase 8 scenario/fault
    harnesses.
  - Names `verification/matrix.toml` as the future machine-readable source of
    truth, with Markdown drift checking.

- `docs/internal/testing/checklists/plc-verification-program/implementation-board.md`
  - Adds Phase 1 schema rows for `contract_kind`, behavior rows, case files, and
    case artifacts.
  - Adds Phase 1B, a bytecode/VM-only spec-first planning pilot:
    matrix/classifier, planner, pilot decision tables, case generation,
    committed case tables, helper crate, prover, bytecode transform generator,
    report-only CI, adversarial self-tests, and burn-in before skills are
    mandated.
  - Marks the bytecode/VM parts of `VERIF-P5-009` and `VERIF-P5-010` as pulled
    forward by Phase 1B.
  - Adds self-test rows for the new planner/case/prover layer.

## Design Decisions Folded In

- The invariant record is the structured spec unit. No new behavior-record layer
  was added between spec sources and invariants.
- `decision_table` v1 is deliberately narrow: one input, explicit partitions,
  no expression grammar, no multi-input interaction table.
- Tools may generate hostile inputs, but expected outcomes come only from
  behavior rows with oracle refs or blocked cases with spec-gap refs.
- Accept cases require exact delta semantics. Reject cases require zero delta,
  no partial apply, stable error/status, and visible fault surface unless the
  oracle says otherwise.
- Runtime events such as SIGTERM, worker down, slow handshakes, queue-full stop,
  and hardware reconnect are not table cases in v1. They remain Phase 8 fault
  scenarios.
- `prove.py` owns red/green/lock evidence for safety-relevant behavior. Manual
  evidence is not sufficient for safety-critical red/green proof.
- Mutation remains the honest catcher for weak refactor assertions where a
  mutation shard exists. The validator is not claimed to detect assertion
  strength.

## Review Questions

1. Are the `contract_kind` values sufficient without becoming a second planning
   taxonomy?
2. Is the `decision_table` v1 boundary strict enough to prevent scope creep and
   weak generated oracles?
3. Do the `case_file`/digest/artifact/`prove.py` rules make skipped or weakened
   cases loud enough?
4. Is Phase 1B the right place to pull the bytecode/VM parts of matrix and
   changed-file classifier forward?
5. Are the out-of-v1 boundaries correct: no multi-input tables, no expression
   grammar, no scenario/fault case generation, no connector probes, and no
   enforcement outside bytecode/VM during pilot burn-in?
6. Does the bypass-to-catcher mapping avoid overclaiming what static metadata
   can prove?

## What Was Not Done

- No product code changed.
- No existing test was moved, renamed, or rewritten.
- No verification tool was implemented yet.
- No CI enforcement was added yet.
- No skills or repo agent rules were updated to mandate this workflow; that
  remains blocked by `VERIF-STOP-012` until the pilot proves itself.

## Requested Review Verdict

Please review whether the amended document set is precise enough to start Phase
1 implementation and Phase 1B pilot work. If not, identify required document
edits before implementation starts.
