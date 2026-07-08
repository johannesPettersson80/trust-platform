# Fable Review Fold Summary

Date: 2026-07-08

Review verdict: clear-with-edits.

Scope: documentation/checklist changes only. No runtime, compiler, LSP, VS Code,
or test runner behavior changed.

## Blocking Findings Folded

- `V-01`: Added durable-evidence definition, evidence-index schema plan, ignored
  evidence path validation, and `.gitignore` negation for this evidence root.
- `V-02`: Expanded bytecode/VM pilot scope to include
  `crates/trust-runtime-core/src/{bytecode,vm,value}/**` and in-source tests.
- `V-03`: Fixed metadata examples: source status vs authority, common fields,
  coverage matrix on invariants, no empty-string sentinels, real
  `docs/specs/12-bytecode.md` bytecode source, and validator contract as spec
  gap.
- `V-04`: Added cross-field validator preconditions for `test_written`,
  `implemented`, and `validated`, plus matching known-bad fixture requirements.
- `V-05`: Added evidence record/index model and evidence ID references.
- `V-06`: Public claims are committed spec-source records with
  `authority = "public_claim"` and required claim/surface fields.
- `V-07`: Added `trust-debug`/DAP/write-force-release matrix row, ownership, and
  seeds for force lifecycle, debug auth, and pause/watchdog interaction.

## Non-Blocking Findings Folded

- `V-08`: Added Phase 4 import row for confirmed review findings and seeds for
  timers, NaN/Inf ingress, runtime authz, OPC UA lifecycle, online change, and
  related debug/runtime safety.
- `V-09`: Clarified Phase 2 catalog status rules: empty invariants are allowed
  only for unmapped planned/gap records.
- `V-10`: Stale-test validation now requires file path plus test name.
- `V-11`: Added suite inventory row for existing workflows/gate scripts and
  veryquick environment/recipe mapping.
- `V-12`: Extended runtime anomaly taxonomy with clock/time faults and OOM.
- `V-13`: Added committed grace-period configuration row.
- `V-14`: Added `ux_accepted` mapping rule for UI invariant validation.
- `V-15`: Added wrong-result risk, fold-summary row, draft AGENTS pointer,
  owner-alias/suite-composition/coverage-template rows, and PLCopen/trust-dev
  ownership.

## Follow-Up Status

Implementation remains blocked until `VERIF-REVIEW-004` is checked in
`implementation-board.md`. The review edits are folded, but Phase 1 has not
started.
