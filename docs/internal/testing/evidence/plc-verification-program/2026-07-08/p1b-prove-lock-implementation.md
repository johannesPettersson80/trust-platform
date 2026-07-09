# P1B Prove Lock Implementation

Date: 2026-07-09

Scope:

- Implemented `prove.py lock --baseline` and `prove.py lock --compare`.
- Added self-tests before implementation for baseline writing, successful
  compare, missing/wrong baseline refusal, wrong-test baseline refusal,
  case-file digest drift, command-exit drift, case-result drift, failed current
  run refusal, and blocked current run refusal.
- Added committed-evidence lock-pair validation for `lock_compare` records.
- Updated the evidence contract to use deterministic `case_result_digest` for
  lock comparison. Raw `case_artifact_digest` remains provenance only because
  same-run freshness stamps include a unique run id.

Stop boundary:

- No runtime, VM, compiler, LSP, VS Code, or product behavior changed.
- No spec gap was closed.
- No CI enforcement, skills, or release metadata changed.
- The umbrella `VERIF-P1B-008` remains open pending review.

Focused validation run:

- `python3 -m unittest scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests`
  - Result: 55 tests passed.
- `python3 scripts/prove.py lock --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 --baseline`
  - Result: refused planned row with exit 7:
    `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 is not runnable proof at status 'planned'`.
- `python3 scripts/prove.py lock --test TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 --compare EVID_TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001_LOCK_BASELINE`
  - Result: refused planned row with exit 7:
    `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001 is not runnable proof at status 'planned'`.
- `python3 -m py_compile scripts/prove.py scripts/verification/prover.py scripts/verification/prover_tests.py scripts/verification/metadata_validator/evidence_proof.py scripts/verification/metadata_validator/evidence_proof_tests.py`
  - Result: passed.
- `python3 scripts/validate_verification_metadata.py`
  - Result: `verification metadata validated: 91 records`.
- `scripts/verification_metadata_gate.sh`
  - Result: `verification metadata validated: 91 records`.
- `cargo test -p verification-cases`
  - Result: 10 tests passed.
- `cargo clippy -p verification-cases --all-targets -- -D warnings`
  - Result: passed.
- `cargo fmt --all -- --check`
  - Result: passed.
- `git diff --check`
  - Result: passed.

Current result after adding this evidence row:

- The metadata validator reports 91 records.
- The real catalog case-table rows are still `status = "planned"`, so lock
  baseline and compare both refuse them as non-runnable proof with exit 7.
- The generated proof target directory is not treated as durable evidence.
