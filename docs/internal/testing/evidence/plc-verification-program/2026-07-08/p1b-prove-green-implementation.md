# P1B prove.py green Implementation

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008G` only.

## What Changed

- Added `prove.py green --test <TEST_ID> --red-evidence <EVIDENCE_ID>`.
- Green proof pairs only to red/protective-red evidence for the same catalog
  test.
- The paired red record must use an allowlisted `prove.py vN` producer, link
  the same test, carry the same `case_file_digest`, have failure kind
  `assertion_failure` or `expected_rejection`, and name non-empty
  `red_case_ids` plus `per_case_summary`.
- Green reruns the cataloged command with fresh `TRUST_VERIFY_*` stamps and the
  same artifact binding rules used by red.
- Green evidence is written only when the command exits 0, no case fails or is
  skipped, and every formerly-red case is recorded as `passed`.
- Added committed-evidence pairing validation for green records so a hand-edited
  green evidence record cannot pair to non-red evidence, an unrelated test, a
  different case-table digest, or an empty formerly-red set.
- Added self-tests for successful green pairing, wrong pair kind, missing red
  case data, wrong test, digest drift, still-red cases, and non-zero green runs.

## Stop Boundary

This slice does not implement `prove.py lock`, expected-rejection proof
generation, CI enforcement, product runtime/VM behavior, spec-gap closure, or
skill updates.

## Validation

Reproduce from the repository root:

```sh
python3 -m unittest scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests
python3 scripts/prove.py red --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001
python3 scripts/prove.py green --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 --red-evidence EVID_TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001_RED
python3 scripts/prove.py lock
python3 -m py_compile scripts/prove.py scripts/verification/prover.py scripts/verification/prover_tests.py scripts/verification/metadata_validator/evidence_proof.py scripts/verification/metadata_validator/evidence_proof_tests.py
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo test -p verification-cases
cargo clippy -p verification-cases --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Expected local result at the end of this slice: metadata validation reported 89
records. The follow-up `VERIF-P1B-008H` review-fix slice adds one evidence row,
so the current tree reports 90 records; see
`p1b-prove-green-review-fixes.md`. The real catalog-row red and green commands
should still fail closed because current case-table rows are still `planned`.
