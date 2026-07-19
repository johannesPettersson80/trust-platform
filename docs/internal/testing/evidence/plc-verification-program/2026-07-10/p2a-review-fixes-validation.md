# Phase 2A Review Fixes Validation

Date: `2026-07-10`

Implementation commit:
`e5c0d9d194649bd36ae54e699d685fa6e2b996d2`

This record closes the actionable findings from the review of
`VERIF-P2A-001` through `VERIF-P2A-010`. It is adequacy evidence with
`proof_kind = "none"`; it changes no PLC behavior, test behavior, proof level,
spec-gap status, or CI enforcement.

## Fixes

- Behavior-lock revisions now require a clean full 40-hex Git SHA. Dirty and
  abbreviated revisions fail before ancestry checks. Positive fixtures use
  actual full revisions.
- `verification/README.md` now states the intentional validation boundary:
  the primary metadata validator does not run the source scan required for
  proposal and redirect validation. The standalone proposal/staleness commands
  and all Phase 2A report generation and at-rest paths run the live contract.

The global historical evidence-marker vocabulary was not tightened; existing
evidence rows may still carry older dirty or abbreviated provenance. The
stricter rule applies specifically to future behavior-lock evidence used to
authorize a completed test refactor.

## Tests First

The two new fixtures failed before implementation:

- dirty full and clean abbreviated behavior-lock commits produced no failure;
- the README lacked the primary-vs-live validator boundary statement.

After implementation, the complete proposal contract suite passed 30/30.

## Refreshed Reports

All five reports were regenerated from the clean implementation commit in
separate pristine worktrees with timestamp `2026-07-10T13:51:37Z`.

| Report | Generated JSON SHA-256 |
| --- | --- |
| Test-class completeness | `400469d2042688fb700df7c4401ca50a7ce72d7c1a70602931d31b16b98ac2ec` |
| Coverage-matrix gaps | `362287b6651409208ca94695ad8e3507ff68460c111fa8cc335c3f70e788be68` |
| Malformed-input coverage | `b01e13ba91c2f7c7bf9e1c385185328c13a5c9666c9badc0ca5bc8a30a670325` |
| Unmapped-test debt | `f2b207ba35d5a3fc5fce10f7afa32b622d38fad303404ceb3761367ab8b5461f` |
| Existing-test refactor assessment | `9708e1419fe38eef6964326865c30ca25c32ddd7a77f9454b69608e9f0df5add` |

All five at-rest validators recomputed their live inputs and passed against the
refreshed artifacts.

## Validation

Run locally from `/home/johannes/projects/trust-platform`:

```text
python3 -m unittest scripts.verification.adversarial_selftest_tests scripts.verification.report_gate_tests scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests scripts.verification.metadata_validator.schema_contracts_tests scripts.verification.bytecode_transforms_tests scripts.verification.bytecode_validator_mutation_tests scripts.verification.test_catalog_scanner_tests scripts.verification.test_catalog_intent_tests scripts.verification.test_catalog_staleness_tests scripts.verification.test_catalog_vscode_registration_tests scripts.verification.test_class_completeness_tests scripts.verification.report_input_contract_tests scripts.verification.coverage_matrix_gap_tests scripts.verification.malformed_input_coverage_tests scripts.verification.test_catalog_debt_tests scripts.verification.test_refactor_assessment_tests scripts.verification.test_refactor_contract_tests scripts.verification.test_refactor_report_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/validate_test_refactor_proposals.py
python3 scripts/check_test_catalog_staleness.py
python3 scripts/validate_test_class_completeness_report.py --json target/gate-artifacts/verification/test-class-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md
python3 scripts/validate_coverage_matrix_gap_report.py --json target/gate-artifacts/verification/coverage-matrix-gaps.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-matrix-gaps.md
python3 scripts/validate_malformed_input_coverage_report.py --json target/gate-artifacts/verification/malformed-input-coverage.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-malformed-input-coverage.md
python3 scripts/validate_unmapped_test_debt_report.py --json target/gate-artifacts/verification/unmapped-test-debt.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md
python3 scripts/validate_test_refactor_assessment_report.py --json target/gate-artifacts/verification/test-refactor-assessment.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-test-refactor-assessment.md
python3 -m py_compile scripts/verification/test_refactor_behavior_lock.py scripts/verification/test_refactor_contract_tests.py
git diff --check
```

Results before indexing this record: 246/246 focused tests passed in 82.819
seconds; metadata and the metadata gate validated 109 records; proposal
validation reported one proposal and zero redirects; staleness reconciled six
catalog rows with 3,816 facts; all five at-rest reports and Python compilation
passed.

After indexing this record, the same focused suite passed 246/246 in 102.132
seconds, both metadata entry points validated 110 records, and all five at-rest
report validators passed again.

Broad Rust gates were not rerun for this Python/docs-only review fix. The
accepted Phase 2A closure record already binds the unchanged Rust workspace at
`4e9923b0947f78f30bc9a17789f4f6889ff3e819` to clean remote `fmt`, `clippy`,
and `test-all` results.

## Preserved Boundaries

- `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P10-001`, and `VERIF-P10-003`
  remain open; `VERIF-STOP-012` stays closed.
- All ten spec gaps remain open, and `VM_SEAM_VALID_001` remains `spec_gap` at
  proof level `S0`.
- No product/runtime source, existing product test, workflow, skill, or agent
  instruction changed. CI remains report-only.
