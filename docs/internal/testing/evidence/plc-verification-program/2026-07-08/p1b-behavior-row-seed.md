# Phase 1B Behavior-Row Seed

Date: 2026-07-09
Scope: `VERIF-P1B-004`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata/tooling changes

## What Changed

Seeded decision-table behavior rows for the three bytecode/VM pilot bug
classes:

- `VM_SEAM_SUBRANGE_001`: in-range `INT(0..100)`, below-minimum,
  above-maximum, and wrong-type partitions.
- `VM_SEAM_DECLARED_TYPE_001`: INT-to-REAL, INT-variable-to-REAL,
  INT-expression-to-DINT, and wrong-type conversion partitions.
- `VM_SEAM_STRING_BOUND_001`: in-bound `STRING[5]`, over-bound,
  FB-input-copy-in over-bound, and wrong string-family partitions.

Every row uses `spec_gap_ref = "SPEC_GAP_VM_VALUE_SEMANTICS_001"`.
No row carries `outcome`, `delta`, `error_code`, `no_partial_apply`, or
`fault_surface`. This is intentional: the pilot now has explicit behavior
partitions, but the value-semantics oracle is still unwritten.

## Stop Boundary

Not implemented in this slice:

- no `gen_cases.py`;
- no committed case tables;
- no `prove.py`;
- no `verification-cases` crate;
- no bytecode/VM/runtime product behavior changes;
- no CI enforcement or skill update.

## Validation

- `python3 scripts/validate_verification_metadata.py`
  - Result before indexing this evidence record:
    `verification metadata validated: 71 records`.
- `python3 scripts/validate_verification_metadata.py`
  - Result after indexing this evidence record:
    `verification metadata validated: 72 records`.
- `scripts/verification_metadata_gate.sh`
  - Result: `verification metadata validated: 72 records`.
- `python3 scripts/plan_tests.py --intent bugfix --changed crates/trust-runtime-core/src/value/types.rs`
  - Result: `Verdict: spec_gap`, `Exit code: 3`.
  - The plan still lists `SPEC_GAP_VM_VALUE_SEMANTICS_001` and the other
    bytecode/VM pilot gaps as blockers.
  - The plan emits no expected behavior text.
- `python3 scripts/plan_tests.py --intent bugfix --changed verification/invariants/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`
  - Result: `Verdict: spec_gap`, `Exit code: 3`.
  - The invariant metadata route remains blocked by the same unresolved
    spec-gap set.
- `python3 scripts/plan_tests.py --intent docs --changed verification/invariants/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml --format json`
  - Result: `exit_code = 3`, `required_test_classes = ["metadata_validation"]`,
    and no required case families.

## Acceptance Notes

`VERIF-P1B-004` is complete only as a spec-first metadata slice. It proves that
behavior partitions can be recorded without converting open specification
questions into test oracles. `VERIF-P1B-005` remains blocked until the value
semantics decisions are written or explicitly carried as blocked cases by the
case generator.
