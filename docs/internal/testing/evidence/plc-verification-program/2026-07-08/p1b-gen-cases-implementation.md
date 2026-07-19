# Phase 1B Case Generator Implementation

Date: 2026-07-09
Scope: `VERIF-P1B-005`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata/tooling changes

## What Changed

- Added `scripts/gen_cases.py` as the command entrypoint.
- Added `scripts/verification/case_generator.py` for deterministic
  bytecode/VM pilot case generation.
- The generator derives case records from invariant `[[behavior]]` rows:
  - rows with `spec_gap_ref` become `state = "blocked"` cases with the same
    `spec_gap_ref`;
  - rows with `oracle_ref` copy expected outcome fields from the behavior row;
  - no expected outcome is generated from product code, labels, or guesses.
- Added `--check` support to compare generated TOML to a case file. This proves
  deterministic reproduction without committing case tables in this slice.

## Stop Boundary

Not implemented in this slice:

- no committed pilot case tables under `verification/cases/bytecode_vm/**`;
- no test-catalog case digests;
- no `prove.py`;
- no `verification-cases` crate;
- no bytecode/VM/runtime product behavior changes;
- no CI enforcement or skill update.

## Validation

- `python3 -m py_compile scripts/gen_cases.py scripts/verification/case_generator.py`
  - Result: pass.
- `python3 scripts/validate_verification_metadata.py`
  - Result after indexing this evidence record:
    `verification metadata validated: 74 records`.
- `scripts/verification_metadata_gate.sh`
  - Result: `verification metadata validated: 74 records`.
- `python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001`
  - Result: generated TOML with five blocked cases and no `expect` table.
- `python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --format json`
  - Result: generated JSON with four blocked cases and no expected behavior.
- `python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001`
  - Result: generated TOML with five blocked cases and no `expect` table.
  - Boundary values for `STRING[5]` are mechanical string values: `""`,
    `"xxxxx"`, and `"xxxxxx"`.
- `tmp=$(mktemp /tmp/trust-cases-subrange.XXXXXX.toml); python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --out "$tmp" && python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check "$tmp"; rc=$?; rm -f "$tmp"; exit $rc`
  - Result: pass.
- Drift probe: generate the subrange case file, append one blank line, then run
  `--check`.
  - Result: fail closed with `case file drift`.
- `python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --format json --check /tmp/does-not-matter.toml`
  - Result: fail closed with `--check only supports TOML case-file output`.
- No-oracle probe:
  `python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 | rg -n 'expect|outcome|error_code|delta'`
  - Result: no matches.
- TOML parse probe: generate `VM_SEAM_STRING_BOUND_001`, parse it with
  `tomllib`, and assert every case is blocked and no case has `expect`.
  - Result: `parsed 5 blocked cases`.
- `python3 scripts/gen_cases.py --invariant RT_SAFE_STOP_001`
  - Result: fail closed with `P1B case generation is scoped to bytecode_vm only`.
- `python3 scripts/plan_tests.py --intent bugfix --changed crates/trust-runtime-core/src/value/types.rs`
  - Result: still `Verdict: spec_gap`, `Exit code: 3`.
- `git diff --check`
  - Result: pass.

## Acceptance Notes

`VERIF-P1B-005` is complete as a tool slice. It creates the deterministic
front door for case derivation, but does not treat generated cases as proof.
`VERIF-P1B-006` remains responsible for committed pilot case tables and catalog
digests.
