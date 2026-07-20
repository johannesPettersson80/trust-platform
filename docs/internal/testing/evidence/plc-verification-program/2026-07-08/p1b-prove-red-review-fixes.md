# P1B prove.py red Review Fixes

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008F` only.

## What Changed

- Removed the undocumented `expected_red_failure_kind` red path from
  `prove.py red`.
- `prove.py red` now rejects `expected_red_failure_kind` as a metadata error
  until expected-rejection proof has a validator-backed catalog contract.
- Added command-timeout classification: a timed-out catalog command raises
  non-red `timeout` instead of producing a raw traceback.
- Increased the red command timeout default to 1800 seconds so cold cargo runs
  are not classified as timeout by default.
- Expanded red prover self-tests for:
  - reserved expected-rejection catalog knob rejection,
  - timeout classification,
  - exit 0 with failed cases,
  - non-zero exit with an artifact but no failed cases,
  - exit 0 with no failed cases,
  - unknown, duplicate, missing, and skipped case IDs.
- Updated the proof contract to state that expected-rejection red remains
  reserved until the catalog field, rejection signal, and validator rules are
  implemented together.

## Stop Boundary

At the end of this slice, `prove.py green` and `prove.py lock` were still
unimplemented. The follow-up `VERIF-P1B-008G` slice implements green, so the
current tree requires `prove.py green --test <TEST_ID> --red-evidence
<EVIDENCE_ID>`. This slice still does not implement `prove.py lock`, expected
rejection proof, CI enforcement, product runtime/VM behavior, spec-gap closure,
or skill updates.

## Validation

Reproduce from the repository root:

```sh
python3 -m unittest scripts.verification.prover_tests
python3 scripts/prove.py red --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001
python3 scripts/prove.py green
python3 scripts/prove.py lock
python3 -m py_compile scripts/prove.py scripts/verification/prover.py scripts/verification/prover_tests.py
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo test -p verification-cases
cargo clippy -p verification-cases --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Expected local result after this slice: metadata validation reports 88 records.
The real catalog-row red command should still fail closed because current
case-table rows are still `planned`.
