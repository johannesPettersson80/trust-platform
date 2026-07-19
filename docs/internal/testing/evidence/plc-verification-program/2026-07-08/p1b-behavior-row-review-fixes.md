# Phase 1B Behavior-Row Review Fixes

Date: 2026-07-09
Scope: `VERIF-P1B-004A`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata/tooling changes

## Review Findings Folded

- `BR-01`: Added oracle-reference validation before case generation. Behavior
  rows and test catalog rows with `oracle_ref` now resolve the base ID before
  `#` against active non-public-claim spec sources.
- `BR-01`: Added an error-code gate. Behavior rows with `error_code` require an
  active source covering `stable_error_code_model`; until that exists, rows must
  remain blocked by a spec gap.
- `BR-01`: Pinned `partition.equals` as an opaque `UPPER_CASE_LABEL`, so future
  tooling cannot infer product behavior from label text.
- `BR-02`: Added `wrong_type_or_shape` spec-gap coverage cells to
  `VM_SEAM_SUBRANGE_001` and `VM_SEAM_STRING_BOUND_001`.
- `BR-03`: Bumped `SPEC_GAP_VM_VALUE_SEMANTICS_001.last_reviewed` to
  `2026-07-09`.

## Stop Boundary

Not implemented in this slice:

- no `gen_cases.py`;
- no committed case tables;
- no `prove.py`;
- no `verification-cases` crate;
- no bytecode/VM/runtime product behavior changes;
- no CI enforcement or skill update.

## Validation

- `python3 -m py_compile scripts/plan_tests.py scripts/verification/planner.py scripts/verification/metadata_validator/core.py scripts/verification/metadata_validator/constants.py scripts/verification/metadata_validator/taxonomy.py scripts/verification/metadata_validator/integrity.py scripts/verification/metadata_validator/oracle_refs.py scripts/validate_verification_metadata.py`
  - Result: pass.
- `python3 scripts/validate_verification_metadata.py`
  - Result before indexing this evidence record:
    `verification metadata validated: 72 records`.
- `python3 scripts/validate_verification_metadata.py`
  - Result after indexing this evidence record:
    `verification metadata validated: 73 records`.
- `scripts/verification_metadata_gate.sh`
  - Result: `verification metadata validated: 73 records`.
- `python3 scripts/plan_tests.py --intent bugfix --changed crates/trust-runtime-core/src/value/types.rs`
  - Result: `Verdict: spec_gap`, `Exit code: 3`.
- `python3 scripts/plan_tests.py --intent bugfix --changed verification/invariants/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`
  - Result: `Verdict: spec_gap`, `Exit code: 3`.
- `python3 scripts/plan_tests.py --intent docs --changed verification/invariants/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml --format json`
  - Result: `exit_code = 3`, `required_test_classes = ["metadata_validation"]`,
    and no required case families.
- In-memory adversarial validator probes:
  - Fake behavior `oracle_ref = "TOTALLY_FAKE_ORACLE_999"` fails with an
    unknown spec-source error.
  - Behavior `error_code = "MadeUpCode"` fails while no active
    `stable_error_code_model` source exists.
  - `partition = { equals = "bad label" }` fails the opaque-label rule.
- `git diff --check`
  - Result: pass.
- Duplicate `VERIF-*` checklist row scan
  - Result: no duplicates.
- Line-count check
  - `metadata-model.md`: 857 lines.
  - `metadata-evidence-traceability.md`: 155 lines.
  - `core.py`: 938 lines.
  - `oracle_refs.py`: 68 lines.
  - `planner.py`: 415 lines.

## Acceptance Notes

`VERIF-P1B-005` may now trust that an `oracle_ref` in behavior metadata is at
least anchored to a real active oracle source. It still must not generate
expected outcomes from rows that carry `spec_gap_ref`.
