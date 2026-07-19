# P1B-007 Verification Cases Helper

Date: 2026-07-09
Branch: plc-verification-program
Scope: VERIF-P1B-007 only.

## What Changed

- Added `crates/verification-cases`, a small dev-helper crate for tests that
  consume committed verification case files.
- Added `run_case_file!` and `run_case_file(...)`.
- Added the v1 `StateProbe` contract with snapshots containing:
  - process image hash,
  - retain hash,
  - target value,
  - sibling values,
  - diagnostics.
- Added JSON case-artifact emission under the workspace-root
  `target/gate-artifacts/cases/<TEST_ID>.json` or a caller-provided artifact
  directory.
- Added mandatory SHA-256 case-file digest enforcement before execution.

## Tests First

Before implementation, `cargo test -p verification-cases` failed to compile
because the tests referenced the missing API:

- `CaseExecution`
- `CaseResult`
- `RunConfig`
- `StateProbe`
- `StateSnapshot`
- `case_file_digest`
- `run_case_file(...)`

After implementation, the same command passes.

## Locked Behavior

- Blocked cases are recorded as `result = "blocked"` and are not passed to the
  runner closure.
- Runnable cases are wrapped with before/after `StateProbe` snapshots and get a
  state-delta verdict of `changed` or `unchanged`.
- Digest mismatches fail before any case executes and before an artifact is
  written.
- Case files with `schema_version != 1` fail before any case executes and before
  an artifact is written.
- The helper writes observations only. It does not create durable evidence,
  classify red/green proof, or close any spec gap.

## Stop Boundary

This slice does not add `prove.py`, does not add product runtime or VM behavior
changes, does not close the value-semantics spec gap, does not add CI
enforcement, and does not update skills.

## Validation To Reproduce

```sh
cargo test -p verification-cases
cargo clippy -p verification-cases --all-targets -- -D warnings
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo fmt --all -- --check
git diff --check
```

## Observed Local Result

- `cargo test -p verification-cases`: 5 passed; 0 failed.
- `cargo clippy -p verification-cases --all-targets -- -D warnings`: pass.
- `python3 scripts/validate_verification_metadata.py`: `verification metadata validated: 81 records`.
- `scripts/verification_metadata_gate.sh`: `verification metadata validated: 81 records`.
- `cargo fmt --all -- --check`: pass.
- `git diff --check`: pass.
