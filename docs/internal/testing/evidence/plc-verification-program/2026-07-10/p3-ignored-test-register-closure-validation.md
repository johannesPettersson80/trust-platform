# Phase 3 Ignored-Test Register Closure Validation

Date: 2026-07-10

Implemented rows: `VERIF-P3-001` through `VERIF-P3-005`.
`VERIF-P3-006` remains open because `VERIF-P14-000` has not defined its grace
period.

## Revisions And Reports

- Implementation checkpoint: `7d041edb9192ec65646ca231d0d9b0c5abffa781`.
- Mutation-evidence checkpoint: `a2d8bb7b50d0ec5c2fad33d348ed41e46b705158`.
- Report and board checkpoint validated here:
  `cf513c5d6f8c54a27fb4c7b71b0cf2d1944c9b96`.
- The validator mutation shard was rerun against the clean implementation
  checkpoint: 2 caught, 0 survived, 0 unviable, 0 timeout, 0 error. Report
  SHA-256: `d813727096bb415f0de5105add8b588edbc31290e8f814a809e031a1d71b57ec`.
- All six generated reports used timestamp `2026-07-10T17:05:47+02:00` and
  source commit `a2d8bb7b50d0ec5c2fad33d348ed41e46b705158` in separate pristine
  detached worktrees.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `ca1bc053d3cf2ffd93c0f86c0309a6691a1bc9dfdca5d515f0341bab201f1b64` |
| Coverage-matrix gaps | `c7e67119c30a22c1ca7de5413fdabcc5a5a716743aff258fc174cf94e7d74da3` |
| Malformed-input coverage | `5dd28c6ebfdea7a406730aeac53d277305e139d04680c8011a46bd85bc820878` |
| Unmapped-test debt | `86818961248e7316721eb5a27cbb87b711ca6d25305903fd3704003625112faa` |
| Test-refactor assessment | `3db89930962dec7791f97ae770434bbd2044d7c453807465afd2a24f5679e941` |
| Ignored-test inventory | `21dd101354aa7ec138aab6056c8613ccc71cef5c92d0b33188664b7882020a43` |

## Result

- Mechanical inventory: 88 records, 86 ignored, 2 conditional, zero
  diagnostics. The source split is 57 Rust integration, 29 Rust unit, one VS
  Code, and one Playwright record.
- Reviewed registry: 88 discovered, 88 registered, 63 `unknown`, 15
  `perf_soak`, five `lab_required`, five `manual`, and zero catalog-mapped.
- The 63 historical red or unstable-looking observations remain `unknown`
  unless current failing behavior and durable evidence are established. Source
  prose did not create `red_protective` or `flaky_quarantined` claims.
- The five `lab_required` records carry non-secret environment variables,
  reviewed topology text, tracked topology evidence, and explicit public-claim
  impact. The impact field is not a public-claim mapping.
- No ignored source marker or existing product test was changed. This slice
  added 47 verification-tool regression tests.

## Tests First And Review Fixes

- Initial tests failed because the inventory/report modules and closed registry
  validator did not exist, and because `prove.py` indexed ignored record IDs
  instead of optional catalog test IDs.
- Adversarial review then demonstrated unsupported and multiline Node skip
  forms that could escape discovery, a deleted modeled source that could shrink
  an at-rest input closure, and unpinned report-schema vocabularies. Dedicated
  fixtures failed before each fix and pass now.
- VS Code, Playwright, and excluded-Node whole-file lexical sentinels reject
  unsupported skip forms without treating comments, strings, templates, or
  regular-expression literals as tests.
- At-rest validation derives modeled paths from the claimed commit tree, so a
  later source deletion cannot silently remove a fact from the report closure.
- The prior Phase 2A clean-full-SHA behavior-lock fix remains in force, and the
  README states that proposal/redirect source joins are live standalone checks,
  not part of the primary static metadata validator.

## Local Validation

The focused Python command ran these suites:

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
  scripts.verification.test_catalog_debt_tests \
  scripts.verification.test_refactor_assessment_tests \
  scripts.verification.test_refactor_contract_tests \
  scripts.verification.test_refactor_report_tests \
  scripts.verification.ignored_test_inventory_tests \
  scripts.verification.ignored_test_vscode_skip_tests \
  scripts.verification.ignored_test_provenance_tests \
  scripts.verification.ignored_test_schema_drift_tests \
  scripts.verification.ignored_test_staleness_tests \
  scripts.verification.metadata_validator.ignored_tests_tests
```

Result: 293 tests passed in 93.318 seconds.

The following commands also passed locally at the report checkpoint:

```text
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/check_ignored_test_staleness.py
python3 scripts/check_test_catalog_staleness.py
python3 scripts/check_vscode_test_registration.py
python3 scripts/validate_test_refactor_proposals.py
python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_VALID_001 --check
python3 scripts/validate_test_class_completeness_report.py --json target/gate-artifacts/verification/test-class-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md
python3 scripts/validate_coverage_matrix_gap_report.py --json target/gate-artifacts/verification/coverage-matrix-gaps.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-matrix-gaps.md
python3 scripts/validate_malformed_input_coverage_report.py --json target/gate-artifacts/verification/malformed-input-coverage.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-malformed-input-coverage.md
python3 scripts/validate_unmapped_test_debt_report.py --json target/gate-artifacts/verification/unmapped-test-debt.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md
python3 scripts/validate_test_refactor_assessment_report.py --json target/gate-artifacts/verification/test-refactor-assessment.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-test-refactor-assessment.md
python3 scripts/validate_ignored_test_inventory_report.py --json target/gate-artifacts/verification/ignored-test-inventory.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p3-ignored-test-inventory.md
python3 -m py_compile scripts/check_ignored_test_staleness.py scripts/report_ignored_test_inventory.py scripts/validate_ignored_test_inventory_report.py scripts/verification/ignored_test*.py scripts/verification/metadata_validator/ignored_tests.py
git diff --check
```

Both metadata entry points validated 199 records before this closure row was
indexed. The ignored-test join reported 88/88 with 63 unknown and zero catalog
mappings; catalog staleness reported 6/3,816; VS Code registration reported
456 facts in 38 registered files; proposals reported 1 proposal and 0 redirects.

## Remote Validation

Remote host: `trust-builder`, Linux x86-64. Clean detached clone:
`$HOME/projects/trust-platform-p3-validation-7d041edb9` at
`cf513c5d6f8c54a27fb4c7b71b0cf2d1944c9b96`.

- The same focused 293-test command passed in 40.987 seconds.
- Both metadata entry points validated 199 records, and the live ignored-test
  join reported 88/88 with 63 unknown and zero catalog mappings.
- `just fmt` passed at 2026-07-10T15:24:51Z.
- `just clippy` (`cargo clippy --all-targets --all-features`) passed in 1 minute
  26 seconds at 2026-07-10T15:26:18Z.
- `just test-all` passed at 2026-07-10T15:35:47Z.
- The clone remained clean after all gates. The dedicated generated target was
  45 GiB after validation and was removed after recording the result; the clean
  source clone was retained for review.

## Preserved Boundaries

- `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P3-006`, `VERIF-P10-001`, and
  `VERIF-P10-003` remain open. `VERIF-STOP-012` remains closed to skills and
  agent-instruction updates.
- CI remains report-only. No workflow, product/runtime source, existing product
  test, skill, agent instruction, version, or release metadata changed.
- All spec gaps remain open. `VM_SEAM_VALID_001` remains `spec_gap` at S0.
- The new evidence rows use `proof_kind = "none"`; no proof, invariant mapping,
  coverage-cell upgrade, or public-claim mapping was created.
