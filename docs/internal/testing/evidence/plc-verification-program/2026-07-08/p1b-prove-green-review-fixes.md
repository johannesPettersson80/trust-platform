# P1B prove.py green Review Fixes

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008H` only.

## What Changed

- Pinned case artifact result vocabulary to `passed`, `failed`, `skipped`, and
  `blocked`.
- `prove.py green` now rejects any blocked case in the same-run artifact.
- Unknown case-result strings now fail artifact validation instead of being
  ignored by green proof.
- Paired red evidence must link exactly one test, matching the requested
  `TEST_ID`.
- Committed green evidence validation now anchors the green/red pair to the
  current catalog row's `case_file_digest`.
- Committed green evidence validation now rejects non-passing
  `per_case_summary` entries, including `blocked` and unknown result strings.
- Expanded prover and validator self-tests for unknown pair IDs, bad producers,
  multi-linked red evidence, blocked cases, unknown result strings, missing
  pairing fields, unknown pairs, failure-kind mismatch, digest mismatch,
  missing summaries, non-zero green exits, and catalog digest drift.

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

Expected local result after this slice: metadata validation reports 90 records.
The real catalog-row red and green commands should still fail closed because
current case-table rows are still `planned`.
