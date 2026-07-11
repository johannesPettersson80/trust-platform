# Phase 6A Verification Tooling Self-Tests Closure Validation

Date: 2026-07-11
Implementation commit: `fa228977fee66537ea22a727555aee297bb28abe`
Fixture evidence checkpoint: `b3de7c33e8517db9f62fd9a276c0a9ebaab07afa`

## Scope

This closure covers `VERIF-P6A-001` through `VERIF-P6A-004` and
`VERIF-P6A-006` through `VERIF-P6A-010`. It adds closed, machine-readable
known-good and known-bad fixtures for the metadata validator, catalog
staleness, changed-file routing, report rendering, planner, case, artifact,
evidence-pairing, and proof-producer boundaries.

`VERIF-P6A-005` remains open. The specification-source scanner is not yet
defined or implemented because `VERIF-P1A-002`, `VERIF-P1A-003`, and
`VERIF-P1A-006` remain open. This slice does not invent that scanner contract.

The Phase 6 review reported no findings. Its exact-string hardware reservation
and remote replay notes were informational and did not authorize a contract
change. This slice creates no product test, proof, specification-gap closure,
CI enforcement, runtime behavior, skill update, or public claim.

## Tests First

The first targeted run failed because
`scripts.verification.tooling_selftest_contract` did not exist. The independent
renderer test then failed because its committed golden did not exist. Both
failures preceded implementation.

Adversarial review exposed and then locked these additional boundaries before
the implementation commit:

- registered public claims now traverse top-level, behavior-row, and coverage
  gap references before validated green/lock evidence is required;
- each fixture's executor, assigned layer, expected disposition, signal, and
  full-validator wiring label is contract-pinned, preventing relabeling from
  evading its assigned catcher;
- the public-claim fixture removes every valid gap/proof route and produces one
  intended failure through both its direct catcher and full
  `Validator.validate()`;
- risk-downgrade findings remain report-only unless an optional `decision_ref`
  resolves to an active, oracle-eligible reviewed decision or deviation; and
- covered decision-table dimensions cannot omit all behavior rows.

The post-fix adversarial re-review was clear.

## Fixture Results

The closed manifest
`verification/selftests/bypass-fixtures.toml` contains 27 canonical cases:

| Board scope | Cases | Result contract |
| --- | ---: | --- |
| `VERIF-P6A-001` | 1 | known-good committed graph accepted |
| `VERIF-P6A-002` | 7 | metadata lies rejected |
| `VERIF-P6A-002A` | 8 | stale, invalid, or overstated metadata rejected |
| `VERIF-P6A-009` | 10 | planner, case, artifact, pairing, and producer bypasses caught |
| `VERIF-P6A-010` | 1 | assertion-strength overclaim rejected by its owning producer layer |

The measured partition is one accept, 25 rejects, and one report-only risk
finding. All 27 signals matched their assigned production layers. Metadata is
explicitly recorded as not proving assertion strength.

Fixture manifest SHA-256:
`f3cad5498064c06d34c704053011fe68fff71953ce72ccafa13c1913340b446e`.
Independent renderer golden SHA-256:
`00c714a2098493d402cbd5e261113dcaf5e5fc4f02ccae1af18f59b055836e17`.
Durable result SHA-256:
`f1ad3e4de83fb8bd1ef8930f6dec9e0cf21adb4cd2da8b92d58cc9d8d29fdf2e`.

## Generated Reports

Eight input-bound reports were stale after the new verification modules were
added. Each was regenerated independently from a pristine detached worktree at
`fa228977fee66537ea22a727555aee297bb28abe` with timestamp
`2026-07-11T08:00:00+02:00`, then validated at rest against the evidence
checkpoint.

| Report JSON | SHA-256 |
| --- | --- |
| `test-class-completeness.json` | `c01bd689129153b1fdfc51dccfc9a7c91892e1a8f0611d09300021d965323d6c` |
| `coverage-matrix-gaps.json` | `63db14b2eef9e68ce35a7cf2bea5d5c40e217287e56a48999c4730da6d239879` |
| `malformed-input-coverage.json` | `847b2ae077c075af3010958b6425266f75d777e9c7058d23139fe835819a9697` |
| `unmapped-test-debt.json` | `2b531a141192b564352965a248d43f0a9b9e3a39e64096fe345e8097ca2e8d87` |
| `test-refactor-assessment.json` | `18e785e652e6ffe024c863a2286264cf0e0658b2868262f6c2aa7c66d5cadcbb` |
| `spec-completeness.json` | `90575c36a91878c75407b7bff416bb82516973d92a4fe43dc019be8112fe7dff` |
| `phase5-suite-audit.json` | `b44159e073e60db004e9bad8bcb7d8e0d2a7a5bb0808185730c20af7479a6ae8` |
| `requirement-oracle-audit.json` | `4f32851a78ae805d7b489c94dc6105c7bcd8245a39daf5f216f2c0c61486e2a5` |

Their substantive counts did not change: 1/3,816 catalog facts mapped, 64/80
coverage cells missing, 1/28 malformed classes mapped, 3,815/3,816 test facts
unmapped, 24 large refactor candidates, 44/52 incomplete invariants, 62 gate
inventory records across six suites, and eight eligible versus 44 missing
invariant oracles.

The Phase 3 ignored-test report, Phase 4 invariant-seed report, case files, and
bytecode-validator mutation report bind no changed input and were not
regenerated.

## Local Validation

The canonical focused runner discovered 40 modules and passed 449/449 tests in
122.398 seconds. A targeted post-review run passed 105/105 relevant tests, and
the final review-fix subset passed 36/36. The fixture CLI matched 27/27 cases.

Before this closure row was indexed, both metadata entrypoints validated 325
records. The ignored-test join reported 88 discovered and registered, including
63 unknown and zero catalog-mapped records. Catalog staleness validated six
catalog rows against 3,816 facts. VS Code registration validated 456 facts in
38 registered files, and refactor proposals validated one proposal with zero
redirects.

The following local commands passed:

```text
python3 scripts/check_verification_tooling_selftests.py --report docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6a-tooling-selftest-fixture-report.md
python3 scripts/run_verification_focused_tests.py
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
python3 scripts/validate_spec_completeness_report.py --json target/gate-artifacts/verification/spec-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4a-specification-completeness.md
python3 scripts/validate_phase5_suite_audit_report.py --json target/gate-artifacts/verification/phase5-suite-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p5-suite-gate-routing-audit.md
python3 scripts/validate_requirement_oracle_audit_report.py --json target/gate-artifacts/verification/requirement-oracle-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6-requirement-oracle-audit.md
git diff --check
```

## Remote Gates

The isolated `trust-builder` clone
`$HOME/projects/trust-platform-p6a-validation-b3de7c33e` was clean at
`b3de7c33e8517db9f62fd9a276c0a9ebaab07afa`. Rust and Cargo ran on
`x86_64-unknown-linux-gnu` with a single isolated target and a home-backed
`TMPDIR`.

```text
just fmt                        2 seconds, exit 0
just clippy                   176 seconds, exit 0
just verification-veryquick  238 seconds, exit 0
just test-all                 581 seconds, exit 0
```

The remote Python replay passed 449/449 focused tests in 67.018 seconds and
validated 325 metadata records before the broad gates. The broad
`verification-veryquick` replay passed the same 449 tests in 67.834 seconds,
the focused runtime and validator tests, and one conformance case. The final
clone clean-tree assertion passed.

The 49 GB generated target and temporary bundle were removed after the gates,
restoring 67 GB free under `/home/johannes`. The clean 220 MB source clone was
retained for review reproduction. Local `/tmp` has 3.3 GB free; only the
temporary Phase 6A bundle was removed locally.

## Preserved Boundaries

- `VERIF-P1A-002`, `VERIF-P1A-003`, `VERIF-P1A-006`, `VERIF-P1B-012`,
  `VERIF-P1B-014`, `VERIF-P3-006`, `VERIF-P4A-005`, `VERIF-P5-000B`,
  `VERIF-P6-007` through `VERIF-P6-010`, `VERIF-P6A-005`,
  `VERIF-P10-001`, `VERIF-P10-003`, and `VERIF-P14-000` remain open.
- All 34 specification gaps remain open.
- All 52 invariants remain unvalidated at proof level `S0`.
- CI remains report-only; no workflow references the Phase 6A command.
- The new evidence rows use `proof_kind = "none"` with empty linked tests,
  invariants, and specification gaps.
- The window is empty under product/runtime crates, editors, workflows, xtask,
  skills, agent instructions, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and
  `justfile`.
