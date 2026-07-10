# Phase 2 Coverage and Debt Closure Validation

Date: `2026-07-10`

Rows: `VERIF-P2-008`, `VERIF-P2-009`, `VERIF-P2-010`

Implementation commit:
`b63196b4d5196af8a1da6fad5078646784fc8fe1`

Evidence commit:
`b2d2a7fc772f34250e8b17df0af68d62f747c2c5`

This record binds focused verification-program validation and the broad remote
workspace gates to the clean evidence commit above. It is adequacy evidence
with `proof_kind = "none"`; it does not close a spec gap, raise an invariant
proof level, or enable enforcement.

## Closed Slice

- `VERIF-P2-008` reports one mapped area, eight invariants, 80 required
  invariant/family slots, 16 declared slots, 64 structurally missing slots,
  one additional recorded dimension, and 17 declared `spec_gap` cells. Four
  case files expose 21 blocked cases without promoting coverage state.
- `VERIF-P2-009` defines 28 reviewed bytecode/VM malformed-input classes. The
  explicit catalog binding covers only `bad_magic`; `invalid_checksum` and
  `unsupported_version` remain `gap_open`, and 25 classes remain `spec_gap`.
- `VERIF-P2-010` subtracts exact catalog discovery identities from 3,816
  scanner facts. One fact is mapped and all 3,815 unmapped identities remain
  visible debt, including 85 ignored and one conditional fact. Debt exits
  successfully; corrupt inputs fail.

## Report Bindings

All four JSON reports were regenerated from clean source commit
`b63196b4d5196af8a1da6fad5078646784fc8fe1` with timestamp
`2026-07-10T08:20:00Z` and then revalidated at rest against the evidence
commit.

| Report | Generated JSON SHA-256 |
| --- | --- |
| Test-class completeness | `d12fc8da468ea85d885a53cb58e8c30be09dc619ff67b85d47a58eb77de59f53` |
| Coverage-matrix gaps | `21482e519290b99304e13c21ffcfc2da060ddbf68e54de4b51bc3713e60bec6b` |
| Malformed-input coverage | `b89b395745eeb2240355110b58cdf22e3f75b64b6313f3d09e923846944f31c7` |
| Unmapped-test debt | `a4f169194c9ca0a1eda8c7cf7f5dbbfe0969831cc680770c61f8807786c92572` |

The validators recompute live scanner/catalog/matrix/taxonomy joins and source
digests. They reject dirty commit markers, symlinked or escaping inputs,
noncanonical JSON, and Markdown that differs from the JSON-derived rendering.
The mutable evidence index and generated evidence files are intentionally not
part of the report-generator source closure, avoiding a report/evidence digest
cycle; the semantic metadata inputs remain explicitly bound.

## Tests First

- P2-008 began with a missing `coverage_matrix_gaps` module failure.
- P2-009 began with a missing `malformed_input_contract` module failure.
- P2-010 began with missing debt module and CLI failures.
- Additional red fixtures preceded the clean-source commit for dirty P2-007
  provenance, tracked symlink inputs, exact Markdown, canonical JSON, and the
  clean-commit schema pattern.

## Local Validation

Run from `/home/johannes/projects/trust-platform`:

```text
python3 -m unittest \
  scripts.verification.adversarial_selftest_tests \
  scripts.verification.report_gate_tests \
  scripts.verification.prover_tests \
  scripts.verification.metadata_validator.evidence_proof_tests \
  scripts.verification.metadata_validator.schema_contracts_tests \
  scripts.verification.bytecode_transforms_tests \
  scripts.verification.bytecode_validator_mutation_tests \
  scripts.verification.test_catalog_scanner_tests \
  scripts.verification.test_catalog_intent_tests \
  scripts.verification.test_catalog_staleness_tests \
  scripts.verification.test_catalog_vscode_registration_tests \
  scripts.verification.test_class_completeness_tests \
  scripts.verification.report_input_contract_tests \
  scripts.verification.coverage_matrix_gap_tests \
  scripts.verification.malformed_input_coverage_tests \
  scripts.verification.test_catalog_debt_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/check_test_catalog_staleness.py
python3 scripts/check_vscode_test_registration.py
python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_VALID_001 --check
python3 scripts/validate_test_class_completeness_report.py \
  --json target/gate-artifacts/verification/test-class-completeness.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md
python3 scripts/validate_coverage_matrix_gap_report.py \
  --json target/gate-artifacts/verification/coverage-matrix-gaps.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-matrix-gaps.md
python3 scripts/validate_malformed_input_coverage_report.py \
  --json target/gate-artifacts/verification/malformed-input-coverage.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-malformed-input-coverage.md
python3 scripts/validate_unmapped_test_debt_report.py \
  --json target/gate-artifacts/verification/unmapped-test-debt.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md
git diff --check
```

Results before indexing this closure record: 197/197 focused tests passed in
97.522 seconds; metadata and the metadata gate validated 105 records; six
catalog records matched 3,816 live scanner facts; all 456 VS Code facts were
contained in 38 registered test files; all four case tables and all four
at-rest reports validated.

After indexing this record, the exact focused command was repeated: 197/197
tests passed in 135.799 seconds, and both metadata entry points validated 106
records.

## Remote Validation

The clean evidence commit was validated in the temporary clone
`$HOME/projects/trust-platform-p2-phase2-validation` on `trust-builder`. Before
the broad gates, disk usage was inventoried. Cleanup was restricted to
generated cache/target content; no source project or worktree folder was
deleted. The broad gate began with approximately 60 GiB free on
`/home/johannes` and 3.7 GiB free on `/tmp`.

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2-phase2-validation" && python3 -m unittest scripts.verification.adversarial_selftest_tests scripts.verification.report_gate_tests scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests scripts.verification.metadata_validator.schema_contracts_tests scripts.verification.bytecode_transforms_tests scripts.verification.bytecode_validator_mutation_tests scripts.verification.test_catalog_scanner_tests scripts.verification.test_catalog_intent_tests scripts.verification.test_catalog_staleness_tests scripts.verification.test_catalog_vscode_registration_tests scripts.verification.test_class_completeness_tests scripts.verification.report_input_contract_tests scripts.verification.coverage_matrix_gap_tests scripts.verification.malformed_input_coverage_tests scripts.verification.test_catalog_debt_tests'
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2-phase2-validation" && just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2-phase2-validation" && CARGO_TARGET_DIR="$HOME/projects/trust-platform/target" just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2-phase2-validation" && CARGO_TARGET_DIR="$HOME/projects/trust-platform/target" just test-all'
```

The remote focused suite passed 197/197 in 38.764 seconds. Metadata, staleness,
registration, case generation, and all four at-rest report checks also passed.
`just fmt`, `just clippy`, and `just test-all` exited zero. The largest reported
workspace test binary passed 880 tests with 19 ignored; the remaining
integration binaries and doc tests also completed successfully. No aggregate
test count is claimed because `just test-all` reports each binary separately.

## Preserved Boundaries

- `VERIF-P1B-012` and `VERIF-P1B-014` remain open; `VERIF-STOP-012` stays
  closed.
- `VERIF-P2A-001` onward remain open; no test move, split, or rename occurred.
- `VERIF-P10-001` and `VERIF-P10-003` remain open.
- All spec gaps remain open, and `VM_SEAM_VALID_001` remains `spec_gap` at
  proof level `S0`.
- No runtime/product source, workflow, CI-enforcement, skill, or agent-rule
  file changed in this slice. CI remains report-only.
