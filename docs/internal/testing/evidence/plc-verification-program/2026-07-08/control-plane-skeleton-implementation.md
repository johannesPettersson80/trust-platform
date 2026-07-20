# PLC Verification Control-Plane Skeleton Implementation

Date: 2026-07-08
Branch: `plc-verification-program`
Base commit at implementation start: `1d9b3ec6a`

## Scope Completed

Closed rows:

- `VERIF-P1-001` through `VERIF-P1-024`
- `VERIF-P1A-010`

Implemented metadata-only skeleton:

- `verification/README.md`
- `verification/schemas/*.schema.json`
- `verification/spec-sources.toml`
- `verification/spec-gaps.toml`
- `verification/spec-matrix.toml`
- `verification/evidence-index.toml`
- `verification/test-catalog.toml`
- `verification/ignored-tests.toml`
- `verification/risk-register.toml`
- `verification/suites/*.toml`
- `verification/invariants/**`
- `scripts/validate_verification_metadata.py`
- `scripts/verification_metadata_gate.sh`

## Validator Proof

Command:

```sh
python3 scripts/validate_verification_metadata.py
```

Result:

```text
verification metadata validated: 66 records
```

Gate command:

```sh
scripts/verification_metadata_gate.sh
```

Result:

```text
verification metadata validated: 66 records
```

Post-review note: review fixes added after this skeleton report are recorded in
`control-plane-review-fixes.md`, including the current post-fix validator count.

The validator is intentionally dependency-free and does not run Rust, Node,
browser, network, or hardware tests.

## Enforced Checks

Current Phase 1 validator checks include:

- JSON schema files exist and parse.
- TOML metadata parses.
- `schema_version = 1`.
- canonical area/status/risk/contract-kind vocabularies.
- plural wrapper convention for flat registries.
- one-invariant-per-file convention.
- no empty-string sentinels.
- spec-source path existence and public-claim fields.
- spec-gap source/evidence/invariant references.
- suite include references.
- invariant spec/oracle/coverage references.
- validated/test-written/implemented status preconditions.
- high-risk validated records cannot retain open coverage cells.
- required-spec matrix source/gap resolution.
- required-spec `covers` and authority matching.
- committed-file evidence path existence and git-ignore durability.
- high-risk red/green proof producer allowlist.
- green proof red-pairing fields.
- catalog case-file digest checks when case files are referenced.
- `not_applicable` coverage cells require an active reviewed decision/deviation.

## Stop Boundary

Not implemented in this slice:

- `plan_tests.py`
- `gen_cases.py`
- `prove.py`
- `verification/matrix.toml`
- changed-file classifier
- case tables beyond the directory placeholder
- `crates/verification-cases`
- mutation shard
- product/runtime/compiler behavior changes

The next logical review point is after `VERIF-P1B-001` through
`VERIF-P1B-003A`, before any decision-table behavior rows or case/proof tools
are added.
