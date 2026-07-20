# P1B-007 Review Fixes

Date: 2026-07-09
Branch: plc-verification-program
Scope: VERIF-P1B-007A only.

## Review Findings Folded

- `H-01`: `RunConfig::new` now requires the expected case-file digest. There is
  no unpinned construction path.
- `H-03`: the default artifact directory is derived from the workspace root:
  `target/gate-artifacts/cases`.
- `H-02`: case files with `schema_version != 1` now fail before execution and
  before artifact write.
- Cleanup: `run_case_file` reads the case file once, hashes those bytes, and
  parses those same bytes.
- Evidence hygiene: fixed the P1B-006A fabricated-expect probe transcription.

## Stop Boundary

This remains helper-contract work only. It does not add `prove.py`, product
runtime or VM behavior changes, spec-gap closure, CI enforcement, or skill
updates.

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
- `python3 scripts/validate_verification_metadata.py`: `verification metadata validated: 82 records`.
- `scripts/verification_metadata_gate.sh`: `verification metadata validated: 82 records`.
- `cargo fmt --all -- --check`: pass.
- `git diff --check`: pass.
