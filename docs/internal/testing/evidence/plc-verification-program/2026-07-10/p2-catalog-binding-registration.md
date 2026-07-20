# Generated Existing-Test Catalog

Generator: `test-catalog-scanner v2`
Source revision: `3d8a5a79f5fae14fec950c3851323eee5d74915e`
Generated: `2026-07-10T00:00:00Z`
Platform: `linux-x86_64`
Generated JSON SHA-256: `a00e1aeb109d8d058460e49f65c0fa30d4bd5931a321cb6558a7875e50460d83`
Input SHA-256: `sha256:95257ea20be325b9a67f42dcd8b360229d8bbf786fd275a20e2238b535b2e12d`

This is a mechanical source inventory. It does not map tests to claims,
infer expected behavior, or replace hand-owned test catalog metadata.

## Summary

- Records: 3816
- Source files with records: 670
- Ignored records: 85
- Conditional ignore markers: 1
- Visible scan diagnostics: 1
- Scan errors: 0
- Scan warnings: 1

| Source kind | Records |
| --- | ---: |
| `conformance_case` | 21 |
| `fuzz_target` | 2 |
| `gate_script` | 29 |
| `github_workflow_job` | 30 |
| `rust_integration_test` | 1369 |
| `rust_unit_test` | 1652 |
| `structured_text_test` | 257 |
| `vscode_test` | 456 |

## Hand-Owned Intent

The generated JSON explicitly excludes:
- `subject_kind`
- `area`
- `owner`
- `status`
- `test_class`
- `invariants`
- `expected_result`
- `suite_tiers`
- `requires_hardware`
- `requires_network`
- `duration_class`
- `oracle_ref`
- `spec_gap_ref`
- `expected_failure_mode`
- `evidence_destination`
- `command`
- `last_reviewed`

## Limitations

- Discovery is static and recognizes only the declaration forms documented for this slice.
- Command hints are navigation aids; non-exact hints are not runnable proof commands.
- Reference candidates are lexical observations and never create proof or invariant mappings.
- Nested Rust integration support files use package-level hints when no Cargo target is evident.
- Dynamic VS Code titles and runtime skips remain visible diagnostics rather than inferred facts.
- Structured Text command hints use project-level substring filters and remain conservative.
- The scripts/*gate* surface means root-level files under scripts whose names contain gate.
- The P2 board scope excludes Rust tests under xtask/**; they are not included in report totals.
- Only the root fuzz/Cargo.toml is scanned; crate-local fuzz workspaces are excluded.
- Reviewed live census counts are an evidence tripwire and require an intentional refresh on drift.
- Hand-owned catalog intent and enforcement remain outside VERIF-P2-001 through VERIF-P2-003.

## Diagnostics

- `editors/vscode/src/test/suite/new-project.test.ts:339` `warning/conditional_runtime_skip`: runtime this.skip() cannot be represented as a declared ignore attribute

## Hand-Owned Catalog Audit

Implemented rows:

- `VERIF-P2-004`: catalog schema v2 and the first scanner-backed, hand-owned
  mapping.
- `VERIF-P2-005`: live stale-path validation for every committed catalog row.
- `VERIF-P2-005A`: exact discovery ID, source kind, path, and test-name binding
  for native tests.
- `VERIF-P2-006`: explicit VS Code extension-test registration validation.

The committed catalog has six records:

- one `generated_test`, `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`, bound to
  `DISC_88F921D24D3708CEF3E1` / `header_validation`;
- four `case_table_artifact` records whose paths equal their committed case
  files;
- one closed `mutation_shard_runner` record for the bytecode-validator shard.

Only the `bytecode_vm` area is meaningfully mapped. The generated row maps the
written requirement that bytecode magic must be `STBC` to
`SPEC_BYTECODE_FORMAT_001`; `SPEC_GAP_VM_ERROR_MODEL_001` remains open so the
test's current `InvalidMagic` variant is not promoted into a stable public error
contract. Its suite list is empty because the current `veryquick` and `pr`
suites do not execute this Rust integration test. The other 3,815 discovered
facts remain unmapped debt for `VERIF-P2-010`; this slice makes no completeness
claim.

Live staleness result:

```text
test catalog staleness validated: 6 committed records against 3816 scanner facts
```

Line-only movement is accepted. Fixtures reject a deleted test, a renamed test
inside a surviving file, a moved test, changed source kind, duplicate discovery
identity, missing/escaping paths, and scanner fields on either non-native
artifact kind. A full `Validator.validate()` fixture rejects changing the
mutation runner into a generated-test bypass without its discovery binding.

## VS Code Registration Audit

```text
VS Code test registration validated: 38 files, 38 registrations
```

All 456 generated `vscode_test` facts are in those 38 registered files. The
checker accepts only direct same-line literal `require("./...test")` statements
between the unique Mocha pre-require and run boundaries. Fixtures reject
orphans, missing or duplicate targets, case mismatch, dynamic or conditional
loads, path traversal, and symlink escape. The one runtime `this.skip()` remains
the visible scanner warning above; it is not misclassified as registration.

## Mutation Evidence Refresh

Catalog v2 changed the case-generator provenance digest and the
`VM_SEAM_VALID_001` case-file digest without changing its seven case IDs. The
bytecode-validator mutation shard was therefore rerun against clean source
commit `3d8a5a79f5fae14fec950c3851323eee5d74915e` in a dedicated target on
`trust-builder`. Result: 2 caught, 0 survived, 0 unviable, 0 timeout, 0 error.
The refreshed machine-report SHA-256 is
`bc6f8a3ad2f2ebcd28611083cd21c69518afea7814497b998e0c0e2c97100d79`.

## Tests First

- The first unit run failed with three missing-module import errors before the
  intent, staleness, and registration implementations existed.
- The full-validator subject-bypass fixture failed before the intent validator
  was wired through `Validator.validate()`.
- The out-of-workspace symlink fixture first raised `ValueError`; lexical path
  capture plus explicit containment now returns deterministic missing/unregistered
  diagnostics instead.

## Validation

Local focused validation:

```text
python3 -m unittest \
  scripts.verification.adversarial_selftest_tests \
  scripts.verification.report_gate_tests \
  scripts.verification.prover_tests \
  scripts.verification.metadata_validator.evidence_proof_tests \
  scripts.verification.bytecode_transforms_tests \
  scripts.verification.bytecode_validator_mutation_tests \
  scripts.verification.test_catalog_scanner_tests \
  scripts.verification.test_catalog_intent_tests \
  scripts.verification.test_catalog_staleness_tests \
  scripts.verification.test_catalog_vscode_registration_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/check_test_catalog_staleness.py
python3 scripts/check_vscode_test_registration.py
python3 scripts/validate_generated_test_catalog.py \
  --json target/gate-artifacts/verification/existing-test-catalog.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-catalog-binding-registration.md
```

Results before indexing this evidence row: 138 tests passed; metadata and the
metadata gate validated 100 records; all four generated case tables matched;
six committed catalog records matched 3,816 live facts; 38 VS Code test files
matched 38 registrations; the generated JSON and this Markdown binding
validated at rest.

Remote focused behavior and mutation validation on `trust-builder`:

```text
CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-p2-004-006-mutation" \
  cargo test -p trust-runtime --test bytecode_container header_validation -- --exact
python3 scripts/bytecode_validator_mutation.py \
  --target-dir "$HOME/.cache/codex-targets/trust-platform-p2-004-006-mutation" \
  --output-json /tmp/p2-bytecode-validator-mutation-report.json \
  --output-markdown /tmp/p2-bytecode-validator-mutation-report.md
```

The cataloged Rust test passed 1/1. The isolated mutation result is recorded
above. No files under `crates/**`, `editors/**`, `.github/**`, skills, or agent
instructions changed. CI remains report-only; every spec gap remains open;
`VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P2-007` onward, `VERIF-P10-001`, and
`VERIF-P10-003` remain open.
