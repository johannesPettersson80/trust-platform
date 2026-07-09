# P1B prove.py red Implementation

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008E` only.

## What Changed

- Added `scripts/prove.py` as the proof CLI shim.
- Added `scripts/verification/prover.py` with red/protective-red proof
  production only.
- Added `scripts/verification/prover_tests.py` with temporary metadata/case
  fixtures for red proof and fake-proof rejection.
- `prove.py red` validates metadata before proof in normal CLI mode.
- `prove.py red` refuses planned catalog rows and ignored tests.
- `prove.py red` deletes any existing artifact for the target test before
  running the catalog command.
- `prove.py red` exports `TRUST_VERIFY_TEST_ID`, `TRUST_VERIFY_RUN_ID`,
  `TRUST_VERIFY_CASE_FILE_DIGEST`, and `TRUST_VERIFY_ARTIFACT_DIR`.
- `prove.py red` validates the same-run case artifact by test ID, run ID,
  artifact directory, case-file digest, and exact committed case IDs.
- Assertion-failure red proof is generated only when the command exits non-zero
  and the same-run artifact contains failed case IDs.
- Command failures without a case artifact are classified as non-red
  `compile_error`.
- Fake/stale artifact paths, wrong run stamps, planned rows, and ignored rows
  are covered by self-tests.
- `prove.py green` and `prove.py lock` intentionally return unimplemented.

## Tests First

Before implementation, `python3 -m unittest scripts.verification.prover_tests`
failed because `scripts.verification.prover` did not exist. After
implementation, the self-test suite passed.

## Stop Boundary

This slice implements `prove.py red` only. It does not implement green pairing
or lock comparison, does not create durable committed red/green/lock proof for a
product invariant, does not change runtime or VM product behavior, does not
close spec gaps, does not add CI enforcement, and does not update Codex skills.

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

Expected local result at the end of this slice: metadata validation reported 87
records. The follow-up `VERIF-P1B-008F` review-fix slice adds one evidence row,
so the current tree reports 88 records; see
`p1b-prove-red-review-fixes.md`. The real catalog-row red command should fail
closed because current case-table rows are still `planned`.
