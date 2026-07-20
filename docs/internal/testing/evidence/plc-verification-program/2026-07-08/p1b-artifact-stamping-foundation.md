# P1B Artifact Stamping Foundation

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008C` only.

## What Changed

- Added `TRUST_VERIFY_*` stamp fields to `verification-cases` artifacts:
  `trust_verify_test_id`, `trust_verify_run_id`,
  `trust_verify_case_file_digest`, and `trust_verify_artifact_dir`.
- The helper stamps those fields when all four environment variables are
  present.
- Partial stamp environments fail before case execution and before artifact
  write.
- Mismatched test ID, case-file digest, or artifact directory stamps fail before
  case execution and before artifact write.
- Updated `verification/schemas/case-artifact.schema.json` and
  `metadata-model.md` to describe the new artifact fields.
- Added `VERIF-P1B-008C` to the board.

## Tests First

Before implementation, `cargo test -p verification-cases trust_verify --
--nocapture` failed because `CaseRunArtifact` did not expose the stamp fields.
After implementation, the same focused test filter passed.

## Stop Boundary

This slice does not add `scripts/prove.py`, does not execute proof-producing
commands, does not create red/green/lock evidence, does not change runtime or VM
product behavior, does not close spec gaps, does not add CI enforcement, and
does not update Codex skills.

## Validation

Reproduce from the repository root:

```sh
cargo test -p verification-cases
cargo clippy -p verification-cases --all-targets -- -D warnings
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo fmt --all -- --check
git diff --check
```

Expected local result after this slice: metadata validation reports 85 records.
