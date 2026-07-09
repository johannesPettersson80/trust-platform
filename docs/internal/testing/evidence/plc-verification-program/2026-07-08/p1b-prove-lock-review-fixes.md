# P1B Prove Lock Review Fixes

Date: 2026-07-09

Review folded:

- `L-01`: lock baselines now require `prove.py vN` or approved proof producer
  at prove time and at rest.
- `L-02`: lock baselines and compares must match the current catalog command at
  prove time and at rest.
- `L-03`: at-rest validation requires lock baseline/compare exit status `0`
  and recomputes `case_result_digest` from command exit status plus
  `per_case_summary`.
- `L-04`: baseline-side refusal tests now cover failed runs, blocked runs,
  exit-zero-with-failed-cases, nonzero clean artifacts, missing lock fields, and
  exit/digest branches.
- `L-05`: `lock --baseline` refuses catalog rows that do not carry a
  `case_file` plus `case_file_digest`.
- DRY cleanup: shared proof-record base writer added so red, green, and lock
  evidence records no longer duplicate the same common fields.

Stop boundary:

- No runtime, VM, compiler, LSP, VS Code, or product behavior changed.
- No spec gap was closed.
- No CI enforcement, skills, or release metadata changed.
- `VERIF-P1B-008` is checked after this fold because red, green, and lock proof
  production now exist and the lock review findings are folded.

Focused validation run:

- `python3 -m unittest scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests`
  - Result: 63 tests passed.
- `python3 scripts/prove.py lock --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 --baseline`
  - Result: refused planned row with exit 7:
    `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 is not runnable proof at status 'planned'`.
- `python3 scripts/prove.py lock --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 --compare EVID_TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001_LOCK_BASELINE`
  - Result: refused planned row with exit 7:
    `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 is not runnable proof at status 'planned'`.
- `python3 -m py_compile scripts/prove.py scripts/verification/prover.py scripts/verification/prover_tests.py scripts/verification/metadata_validator/evidence_proof.py scripts/verification/metadata_validator/evidence_proof_tests.py`
  - Result: passed.
- `python3 scripts/validate_verification_metadata.py`
  - Result: `verification metadata validated: 92 records`.
- `scripts/verification_metadata_gate.sh`
  - Result: `verification metadata validated: 92 records`.
- `cargo test -p verification-cases`
  - Result: 10 tests passed.
- `cargo clippy -p verification-cases --all-targets -- -D warnings`
  - Result: passed.
- `cargo fmt --all -- --check`
  - Result: passed.
- `git diff --check`
  - Result: passed.

Current result after adding this evidence row:

- The metadata validator reports 92 records.
- The real catalog case-table rows are still `status = "planned"`, so live
  lock baseline and compare commands refuse them as non-runnable proof.
