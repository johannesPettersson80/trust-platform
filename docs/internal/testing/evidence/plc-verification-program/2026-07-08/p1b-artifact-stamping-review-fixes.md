# P1B Artifact Stamping Review Fixes

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008D` only.

## Review Findings Folded

- S-01: fixed the process-global environment race in
  `crates/verification-cases` tests. Every test that calls `run_case_file` now
  holds the same `TRUST_VERIFY_*` environment guard, the guard clears stamp
  variables before and after each test, and mutex poisoning is tolerated.
- S-02: partial and mismatch stamp tests now use runnable case files and assert
  `probe.step == 0`, no runner call, and no artifact write.
- S-03: added explicit mismatch tests for `TRUST_VERIFY_TEST_ID` and
  `TRUST_VERIFY_ARTIFACT_DIR`, in addition to the existing case-file digest
  mismatch path.
- S-04: documented that `TRUST_VERIFY_ARTIFACT_DIR` matching is exact path text,
  `TRUST_VERIFY_RUN_ID` must be unique and non-empty, and cataloged Rust test
  commands must filter to the intended test when stamps are exported.

## Stop Boundary

This slice does not add `scripts/prove.py`, does not execute proof-producing
commands, does not create red/green/lock evidence, does not change runtime or VM
product behavior, does not close spec gaps, does not add CI enforcement, and
does not update Codex skills.

## Validation

Reproduce from the repository root:

```sh
for i in $(seq 1 20); do cargo test -p verification-cases -- --test-threads 8 >/tmp/verification-cases-stress.log || exit 1; done
cargo test -p verification-cases
cargo clippy -p verification-cases --all-targets -- -D warnings
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo fmt --all -- --check
git diff --check
```

Expected local result after this slice: metadata validation reports 86 records.
