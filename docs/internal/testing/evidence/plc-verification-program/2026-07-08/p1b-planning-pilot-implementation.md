# Phase 1B Planning Pilot Implementation

Date: 2026-07-09
Scope: `VERIF-P1B-001`, `VERIF-P1B-002`, `VERIF-P1B-003`, `VERIF-P1B-003A`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata/tooling changes

## Implemented

- Added `verification/matrix.toml` and `verification/schemas/matrix.schema.json` for the bytecode/VM pilot.
- Extended the metadata validator to load and validate the matrix record, bytecode/VM area row, required test classes, required case families, and intent rows.
- Added `scripts/plan_tests.py` and `scripts/verification/planner.py`.
- Planner behavior is default-deny:
  - bytecode/VM changes route to `bytecode_vm`;
  - unmapped changed files exit `4`;
  - non-pilot or uninventoried areas exit `3`;
  - open test-mapping spec gaps exit `3`;
  - expected behavior text is never emitted.
- Added the stable error-code inventory in `p1b-error-code-inventory.md`.
- Updated the Phase 1B board rows through `P1B-003A`.

## Stop Boundary

Not implemented in this slice:

- no `VERIF-P1B-004` behavior rows;
- no case generation;
- no `prove.py`;
- no `verification-cases` crate;
- no product runtime/VM behavior changes;
- no CI enforcement or skill update.

## Validation

- `python3 -m py_compile scripts/plan_tests.py scripts/verification/planner.py scripts/verification/metadata_validator/core.py scripts/verification/metadata_validator/constants.py scripts/validate_verification_metadata.py`
  - Result: pass.
- `python3 scripts/validate_verification_metadata.py`
  - Result after indexing these two evidence records: `verification metadata validated: 70 records`.
- `scripts/verification_metadata_gate.sh`
  - Result after indexing these two evidence records: `verification metadata validated: 70 records`.
- `python3 scripts/plan_tests.py --intent bugfix --changed crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs`
  - Result: `Verdict: spec_gap`, `Exit code: 3`.
  - Blocking gaps: `SPEC_GAP_BYTECODE_VALIDATOR_001`, `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001`, `SPEC_GAP_VM_ERROR_MODEL_001`, `SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001`, `SPEC_GAP_VM_VALUE_SEMANTICS_001`.
- `python3 scripts/plan_tests.py --intent bugfix --changed README.md`
  - Result: `Verdict: unmapped`, `Exit code: 4`.
- `python3 scripts/plan_tests.py --intent refactor --area runtime_safety`
  - Result: `Verdict: spec_gap`, `Exit code: 3`, `Uninventoried areas: runtime_safety`.
- `python3 scripts/plan_tests.py --intent docs --changed verification/invariants/bytecode_vm/VM_SEAM_VALID_001.toml --format json`
  - Result: valid JSON, `verdict: spec_gap`, `exit_code: 3`.
- Verification TOML/JSON parse check:
  - Result: `parsed verification TOML/JSON files`.
- `git diff --check`
  - Result: pass.
- Checklist row-definition duplicate scan:
  - Command:
    ```
    rg -o '^- \[[ x]\] `VERIF-[^`]+`' docs/internal/testing/checklists/plc-verification-program | sed 's/.*`\([^`]*\)`.*/\1/' | sort | uniq -d
    ```
  - Result: no duplicate row definitions.
- Line-count check:
  - These counts were captured before the `VERIF-P1B-003B` review-fix slice.
  - Current line-count evidence is superseded by `p1b-planning-pilot-review-fixes.md`.

## Review Notes

The planner correctly reports bytecode/VM as blocked by spec gaps today. That is intentional: this slice proves the front door and default-deny behavior before behavior rows, generated cases, and red/green proof machinery exist.
