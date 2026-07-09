# P1B prove.py Contract Review Fixes

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008B` only.

## Review Findings Folded

- F-01: bound case artifacts to the command that `prove.py` just ran. The
  contract now requires deleting or quarantining stale artifacts before
  execution, failing when an expected artifact is missing, and validating
  `TRUST_VERIFY_TEST_ID`, `TRUST_VERIFY_RUN_ID`,
  `TRUST_VERIFY_CASE_FILE_DIGEST`, and `TRUST_VERIFY_ARTIFACT_DIR` stamps.
- F-02: tightened green pairing. The paired red/protective-red evidence must be
  allowlist-produced, for the same test and digest, carry
  `assertion_failure` or `expected_rejection`, and have non-empty failed-case
  data.
- F-03: aligned field names to the existing validator-facing names:
  `paired_red_evidence` and `formerly_red_case_ids`.
- F-04: made lock comparison implementable by defining the comparison basis as
  command exit status, case-file digest, case-artifact digest, and per-case
  results rather than raw terminal output.
- F-05 and low items: added `decision_ref` for digest/delta exceptions,
  timeout/infra adversarial fixtures, command/artifact consistency, deterministic
  failure classification signal requirements, a no-retry rule, ignored-test
  refusal, and the accepted risk around forged producer strings.

## Stop Boundary

This slice does not add `scripts/prove.py`, does not execute proof-producing
commands, does not create red/green/lock evidence, does not change runtime or VM
product behavior, does not close spec gaps, does not add CI enforcement, and
does not update Codex skills.

## Validation

Reproduce from the repository root:

```sh
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo fmt --all -- --check
git diff --check
```

Expected local result after this slice: metadata validation reports 84 records.
