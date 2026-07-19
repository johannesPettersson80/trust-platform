# Phase 2A Existing-Test Refactor Assessment Closure Validation

Date: `2026-07-10`

Rows: `VERIF-P2A-001` through `VERIF-P2A-010`

Review-fix commits:
`3e207af163703fda7cbfae3c2bc7cbef9e643f87`,
`47deab16e`

Implementation commits:
`f59b5cfff530c91ac71bca92435c6f22f0197668`,
`d8dc5728828b43a9bf7321fc89e2efcb4b3fbd54`

Report/evidence commit:
`4e9923b0947f78f30bc9a17789f4f6889ff3e819`

This record binds the focused verification-program checks and broad remote
workspace gates to the clean evidence commit above. It is adequacy evidence
with `proof_kind = "none"`; it does not prove a test behavior, close a spec
gap, raise an invariant proof level, authorize a refactor, or enable CI
enforcement.

## Review Fixes

The four Phase 2 report evidence commands now use fail-closed `&&` replay
chains. The verification README states that each clean-source report must be
regenerated from its own pristine checkout, and the small dead timestamp and
symlink-loop branches identified in review were removed. The four reports were
then regenerated from the same final clean source revision as the Phase 2A
assessment.

## Closed Slice

- `VERIF-P2A-001` reports 24 files at or above the inclusive 1,000-line review
  threshold. Mixed-purpose classification uses only reviewed catalog area and
  test-class diversity; it found no reviewed mapping-diversity candidate.
- `VERIF-P2A-002` evaluates all six catalog rows. Each claims one invariant, so
  there is no broad multi-invariant candidate. Catalog v2 has no authorized
  coverage-dimension field, and unknown fields cannot supply one.
- `VERIF-P2A-003` found no exact or whitespace-normalized fact-file duplicate,
  no exact case-input duplicate, six same-table structural case peer groups,
  one shared case-file reference group, and no explicit malformed-class
  overlap. Free-form semantic similarity and helper-level similarity remain
  explicitly unassessed.
- `VERIF-P2A-004` joins all 456 VS Code facts in 38 files to all 38 direct
  registrations. Five registered files meet the large-file threshold.
- `VERIF-P2A-005` records one reviewed scanner duration and five separately
  classified artifact rows. The remaining 3,815 scanner facts are
  unclassified; names, ignore state, hardware flags, and suite names cannot
  infer duration.
- `VERIF-P2A-006` through `VERIF-P2A-009` add closed proposal, identity,
  redirect, behavior-lock, and SOLID/KISS/DRY review contracts. Mechanical
  signals never authorize change. Split, command-changing completion,
  completion without case-file-bound lock evidence, and a second redirect edge
  for one test remain blocked where the current proof model cannot support
  them.
- `VERIF-P2A-010` reviews
  `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` and records
  `no_refactor_needed`: the report found no evidence-based refactor signal for
  that test.

No existing test was moved, split, renamed, or behaviorally changed.

## Report Bindings

All five JSON reports were regenerated from clean source commit
`d8dc5728828b43a9bf7321fc89e2efcb4b3fbd54` with timestamp
`2026-07-10T11:57:39Z`. Each generator used a separate pristine detached
worktree. The artifacts were then revalidated at rest against evidence commit
`4e9923b0947f78f30bc9a17789f4f6889ff3e819`.

| Report | Generated JSON SHA-256 |
| --- | --- |
| Test-class completeness | `023d8af52ffa204d7444a9e1c9beb5a406e96f3225b0d81bf39ccacd785a4b73` |
| Coverage-matrix gaps | `283da61777739fb9300ad39aa5f28a6e73134183c4f9b357f1eab16ee83c50f8` |
| Malformed-input coverage | `6d93f74d843290cec0e8e17684112aa0cf2a9bd5c9b4be23d54b9b7d4ed84c56` |
| Unmapped-test debt | `d9eadcc533347fcdccb85dd8740dc1957ecb0ad169190decc62623dbdd7b5764` |
| Existing-test refactor assessment | `5127e0c590f7925ae44e2bfa20a3ff78fa51da546ed8ff887f4805f5196852c9` |

The at-rest validators recompute the live scanner, catalog, matrix, taxonomy,
proposal, VS Code registration, and case-table joins. They enforce canonical
JSON, exact Markdown reconstruction, actual output paths, clean source
provenance, schema drift pins, and the complete immutable input closure. The
mutable evidence plane is globally validated but excluded from report digests
to avoid a self-reference cycle.

The five generated JSON files are gitignored gate artifacts. Closure replay
therefore consumes the byte-identical files produced by the five registered
report commands above; each generator must run in its own pristine checkout
before the files are staged under `target/gate-artifacts/verification/`.

## Tests First

Initial red tests failed on missing assessment, proposal-contract, and report
modules. Subsequent red fixtures preceded fixes for zero-invariant
classification, production lock-command compatibility, redirect reuse,
transient completion, distinct revisions and run IDs, exact case/invariant
binding, entrypoint digest closure, schema drift, durable structural details,
and the final Markdown section order.

## Local Validation

Run from `/home/johannes/projects/trust-platform` at
`4e9923b0947f78f30bc9a17789f4f6889ff3e819`:

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
  scripts.verification.test_refactor_report_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
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
python3 -m py_compile scripts/report_test_refactor_assessment.py scripts/validate_test_refactor_assessment_report.py scripts/validate_test_refactor_proposals.py scripts/verification/test_refactor*.py
git diff --check
```

Results before indexing this closure record: 244/244 focused tests passed in
137.885 seconds; both metadata entry points validated 108 records; six catalog
records matched 3,816 live scanner facts; all 456 VS Code facts were contained
in 38 registered files; one proposal and zero redirects validated; all four
case tables and all five at-rest reports validated; Python compilation and
`git diff --check` passed.

After indexing this closure record, the full focused command passed 244/244 in
111.644 seconds, both metadata entry points validated 109 records, and proposal
validation again reported one proposal and zero redirects.

## Remote Validation

The exact evidence commit was transferred by Git bundle to the standalone,
detached clone
`$HOME/projects/trust-platform-p2a-validation-4e9923b09` on `trust-builder`.
The canonical remote checkout was not used because it was stale and dirty.
The five locally hashed JSON gate artifacts were copied byte-for-byte into the
clone's gitignored target path before the remote at-rest validators ran.

Before the broad gates, generated `.vscode-test` caches in inactive validation
checkouts were removed; no source checkout, user work, or active process state
was deleted. The gate began with 62 GiB free on `/home/johannes` and 3.3 GiB
free on `/tmp`. A dedicated cold target and temp directory were used:
`$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09`.

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2a-validation-4e9923b09" && python3 -m unittest scripts.verification.adversarial_selftest_tests scripts.verification.report_gate_tests scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests scripts.verification.metadata_validator.schema_contracts_tests scripts.verification.bytecode_transforms_tests scripts.verification.bytecode_validator_mutation_tests scripts.verification.test_catalog_scanner_tests scripts.verification.test_catalog_intent_tests scripts.verification.test_catalog_staleness_tests scripts.verification.test_catalog_vscode_registration_tests scripts.verification.test_class_completeness_tests scripts.verification.report_input_contract_tests scripts.verification.coverage_matrix_gap_tests scripts.verification.malformed_input_coverage_tests scripts.verification.test_catalog_debt_tests scripts.verification.test_refactor_assessment_tests scripts.verification.test_refactor_contract_tests scripts.verification.test_refactor_report_tests'
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2a-validation-4e9923b09" && python3 scripts/validate_verification_metadata.py && scripts/verification_metadata_gate.sh && python3 scripts/check_test_catalog_staleness.py && python3 scripts/check_vscode_test_registration.py && python3 scripts/validate_test_refactor_proposals.py && python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check && python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --check && python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001 --check && python3 scripts/gen_cases.py --invariant VM_SEAM_VALID_001 --check && python3 scripts/validate_test_class_completeness_report.py --json target/gate-artifacts/verification/test-class-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md && python3 scripts/validate_coverage_matrix_gap_report.py --json target/gate-artifacts/verification/coverage-matrix-gaps.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-matrix-gaps.md && python3 scripts/validate_malformed_input_coverage_report.py --json target/gate-artifacts/verification/malformed-input-coverage.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-malformed-input-coverage.md && python3 scripts/validate_unmapped_test_debt_report.py --json target/gate-artifacts/verification/unmapped-test-debt.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md && python3 scripts/validate_test_refactor_assessment_report.py --json target/gate-artifacts/verification/test-refactor-assessment.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-test-refactor-assessment.md && python3 -m py_compile scripts/report_test_refactor_assessment.py scripts/validate_test_refactor_assessment_report.py scripts/validate_test_refactor_proposals.py scripts/verification/test_refactor*.py && git diff --check && test -z "$(git status --short)"'
ssh trust-builder 'mkdir -p "$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09/tmp" && cd "$HOME/projects/trust-platform-p2a-validation-4e9923b09" && TMPDIR="$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09/tmp" CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09" just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2a-validation-4e9923b09" && TMPDIR="$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09/tmp" CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09" just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform-p2a-validation-4e9923b09" && TMPDIR="$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09/tmp" CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-p2a-4e9923b09" just test-all'
```

The remote focused suite passed 244/244 in 40.237 seconds. Metadata, proposal,
staleness, registration, case generation, Python compilation, and all five
at-rest report checks passed. `just fmt`, `just clippy`, and `just test-all`
exited zero; clippy completed in 2 minutes 44 seconds. The largest reported
workspace test binary ran 899 tests, with 880 passed and 19 explicitly ignored;
the remaining integration binaries and doc tests also completed successfully.
No aggregate workspace count is claimed because Cargo reports each binary
separately. The remote checkout was clean before and after validation.

The dedicated 44 GiB generated target was deleted after the gate, restoring
61 GiB free on `/home/johannes`; the exact source clone was retained for review.

## Preserved Boundaries

- `VERIF-P1B-012` and `VERIF-P1B-014` remain open; `VERIF-STOP-012` stays
  closed.
- Phase 3 and later implementation rows remain open.
- `VERIF-P10-001` and `VERIF-P10-003` remain open.
- All ten spec gaps remain open. `VM_SEAM_VALID_001` remains `spec_gap` at
  proof level `S0`.
- No runtime/product source or existing product-test source changed; only
  verification-program tooling and its tests were added. No workflow,
  CI-enforcement, skill, or agent-rule file changed. CI remains report-only.
