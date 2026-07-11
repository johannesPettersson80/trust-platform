# Phase 8 Runtime Anomaly Audit Closure Validation

Date: 2026-07-11
Phase 7 predicate fix: `724210c405e21068872c886e42cc028f55b096ec`
Initial implementation commit: `ccdcddc3a24909dba627c757aa8cdca3c62a002b`
Review-hardening commit: `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9`
Final report and board checkpoint: `bbb8e16949ba28c6e3a2b7ed0a2b9c21fe07efec`

## Scope

This closure covers the accepted Phase 7 coverage-gap predicate alignment and
`VERIF-P8-001`, `VERIF-P8-001A`, `VERIF-P8-003`, and `VERIF-P8-004`.

`VERIF-P8-002` remains open. The audit binds 38 explicitly reviewed test
associations, but the 3,021-fact Rust scanner population is provenance context,
not an exhaustive semantic runtime-safety review. The row cannot close until
every fact in a reviewed runtime-safety denominator has an explicit mapped or
reviewed-nonmapping disposition.

`VERIF-P8-005` and `VERIF-P8-006` also remain open. This slice adds no fault
toggle, fault interface, harness hook, or production fault hook. It changes no
runtime, product, workflow, CI, suite command, skill, agent instruction,
specification-gap resolution, invariant status, oracle, proof, or public claim.

## Review Fixes And Tests First

The prior Phase 7 review noted that the producer treated a category as linked
when any case was linked while the at-rest validator required every case to be
linked. A regression fixture was added first, then the producer was aligned to
the stricter all-cases predicate. The focused Phase 7 suite passed 19/19 after
the fix.

Phase 8 contract, mapping, report, and corruption fixtures preceded the final
implementation. Adversarial review then identified three honesty boundaries,
all closed before the implementation commit:

- `VERIF-P8-002` is now guarded open in the generator, at-rest validator,
  report boundary, limitations, documentation, and a checkbox-flip fixture;
- taxonomy IDs and source paths plus every honesty-critical report-schema
  constant, enum, count map, and binding are drift-pinned in both schema and
  Python validation; and
- absolute, traversal, escaping, symlinked, or colliding output paths are
  rejected before live analysis or any file write.

The post-hardening adversarial re-review passed 32/32 focused tests and found no
remaining issue in those scopes. Final review then found and closed three more
defense-in-depth gaps before handoff:

- provenance timestamps now require a real timezone-qualified ISO-8601 value
  in both schema and semantic validation;
- schema-invalid IDs, unhashable leaves, missing leaves, and Markdown renderer
  inputs return validation failures instead of raising exceptions; and
- the taxonomy schema drift check now requires the exact allocation-policy
  phrase list instead of accepting any two strings.

The duplicate hand-maintained taxonomy schema fixture was removed in favor of
the committed schema. Direct and full-validator fixtures cover the static
taxonomy type guard, while table-driven at-rest fixtures cover the report
boundary. A recursive 8,250-mutation semantic and Markdown probe returned zero
exceptions. The final targeted contract run passed 64/64 tests in 62.819
seconds.

## Measured Audit

| Measure | Result |
| --- | ---: |
| Taxonomy classes | 19 |
| Explicit reviewed mappings | 38 |
| Direct / partial / context-only / protective-red | 27 / 8 / 3 / 0 |
| Effectively runnable direct mappings | 27 |
| Ignored or conditional mappings | 5 |
| Runnable / partial-or-non-runnable / unmapped classes | 10 / 4 / 5 |
| Test-gap classes | 9 |
| Live Rust scanner facts | 3,021 |
| Primary PR / nightly / release / hardware-lab classes | 9 / 8 / 2 / 0 |

The nine test-gap rows are `queue_full`, `bad_signal`,
`partial_web_request`, `disk_error`, `clock_step`,
`monotonic_wall_clock_divergence`, `suspend_resume`,
`timer_duration_overflow`, and `allocation_failure_oom`. Four have only
partial, context, ignored, or otherwise non-runnable associations; five have no
explicit association. These are test-gap observations, not new specification
gaps and not claims that no repository test exists.

The conditional `VERIF-P8-001A` review created no duplicate gap.
`SPEC_RUNTIME_ENGINE_001` already states both the absence of dynamic allocation
in the hot path and no heap allocation during execution. Restart and time-base
semantics remain blocked by the existing open
`SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001`, which was neither duplicated nor
closed.

## Report Digests

The Phase 8 report was generated independently from clean implementation commit
`bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` with timestamp
`2026-07-11T15:45:29+02:00` on `linux-aarch64`.

- JSON SHA-256:
  `4f8ded3df70ca63bfb9b078da7e25440665ca126e4e9dcec0d10851875bd180b`
- Input SHA-256:
  `sha256:91bdfa1fffd95a3c589a1d0217a88ca3283e457f5e076825c718ceb8c9afd592`
- Durable Markdown SHA-256:
  `11e480f45c531db142d10abc464c6c70856027c1fc624fddf3440d3f46feed10`

The shared validator input closure changed, so eleven historical report pairs
were regenerated from the same clean commit and timestamp, one pristine
detached worktree per report. Every generator and at-rest validator exited
zero, and all temporary worktrees were removed.

| Report JSON | SHA-256 |
| --- | --- |
| `test-class-completeness.json` | `6a9a71d10ca42195e9316d2e193914a07432080aa92b6854a041404eab93be9d` |
| `coverage-matrix-gaps.json` | `ef94cf871b29cd1c15f07e070786bb623e9f51db532a31cc2ed9a505fa8f7ac3` |
| `malformed-input-coverage.json` | `6dc3119877543da5a11cc6c71a40e58e9808095f53e8a0faf9b7cb4ce8b91243` |
| `unmapped-test-debt.json` | `37f25fe027ef0060b9e0189125c52c78ccc6fa4decffc5794672ce08834146d2` |
| `test-refactor-assessment.json` | `e51fd2a6f8a572e37afc3193ed971a49f9783179a0c8a97052e0187799bd5a13` |
| `ignored-test-inventory.json` | `0b1421bf23a054f6e789d54fba07ff2e3532da61277fbadc49803195e3ecd9ce` |
| `invariant-seed-audit.json` | `f561a3928f77cbf26be506f693e8163f423c00fd1b343e18327f468a1bf6614b` |
| `spec-completeness.json` | `d60b708f6df0523ca5cb41360371e1c799d21418b35f72090b951499811b15c2` |
| `phase5-suite-audit.json` | `4d2617a127a87d23ba1002b3697c806532cf7295bf98279e3e085c01c7eaf583` |
| `requirement-oracle-audit.json` | `989e9cb0d7e62048f5949a528ab297473245d779142ce892ba03a54d77e73614` |
| `conformance-alignment.json` | `aa9c5d862cbafd8ef83ab4e54916a5221a34911e509606aa7dcfab9add625418` |

The existing-test catalog, tooling self-test report, four case tables, and
bytecode-validator mutation inputs did not bind the changed report closure and
were not regenerated. The committed mutation report remains byte-identical at
SHA-256 `fd8b7a7ab1f73b639b198678782072dee19dd5c2f0f9b19fb945dedf22069d4a`.

## Local Validation

The canonical focused runner discovered 44 modules and passed 503/503 tests in
315.215 seconds at the final report checkpoint. The verification-tooling
fixture CLI matched 27/27 cases. Both metadata entry points validated 332
records after the Phase 8 audit and closure evidence rows were indexed.

The ignored-test join reported 88 discovered and registered, 63 unknown, and
zero catalog-mapped. Catalog staleness validated six catalog rows against 3,816
facts. VS Code registration validated 456 facts in 38 files and 38 literal
registrations. Refactor proposals validated one proposal, zero redirects, six
catalog records, and 3,816 scanner facts. All four case generators passed
`--check`, and all twelve report pairs passed at-rest validation against the
report checkpoint.

The following local commands passed:

```text
python3 -m unittest scripts.verification.runtime_anomaly_contract_tests scripts.verification.runtime_anomaly_mapping_tests scripts.verification.runtime_anomaly_report_tests scripts.verification.report_input_contract_tests scripts.verification.adversarial_selftest_tests scripts.verification.metadata_validator.schema_contracts_tests
python3 scripts/run_verification_focused_tests.py
python3 scripts/check_verification_tooling_selftests.py --report docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6a-tooling-selftest-fixture-report.md
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
python3 scripts/validate_invariant_seed_audit_report.py --json target/gate-artifacts/verification/invariant-seed-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4-invariant-seed-audit.md
python3 scripts/validate_spec_completeness_report.py --json target/gate-artifacts/verification/spec-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4a-specification-completeness.md
python3 scripts/validate_phase5_suite_audit_report.py --json target/gate-artifacts/verification/phase5-suite-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p5-suite-gate-routing-audit.md
python3 scripts/validate_requirement_oracle_audit_report.py --json target/gate-artifacts/verification/requirement-oracle-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6-requirement-oracle-audit.md
python3 scripts/validate_conformance_alignment_report.py --json target/gate-artifacts/verification/conformance-alignment.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-11/p7-conformance-alignment.md
python3 scripts/validate_runtime_anomaly_audit_report.py --json target/gate-artifacts/verification/runtime-anomaly-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-11/p8-runtime-anomaly-audit.md
git diff --check
```

## Remote Validation

The isolated `trust-builder` clone first ran the broad Rust gates at initial
implementation commit `ccdcddc3a24909dba627c757aa8cdca3c62a002b`.
The hardening delta contains only Phase 8 Python validation, tests, and one
JSON schema. The clone was then advanced to
`bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9`, renamed to
`$HOME/projects/trust-platform-p8-validation-bb82eaf9c`, and the final Python
surface plus exact `verification-veryquick` recipe were rerun there. The host
was `x86_64-unknown-linux-gnu` with
`rustc 1.95.0 (59807616e 2026-04-14)` and
`cargo 1.95.0 (f2d3ce0bd 2026-03-21)`.

```text
just fmt                       2.25 seconds, exit 0
just clippy                  104.33 seconds, exit 0
just test-all                568.97 seconds, exit 0, 880 passed / 19 ignored
python3 scripts/run_verification_focused_tests.py
                              114.68 seconds, exit 0, 503/503 tests
just verification-veryquick  387.28 seconds, exit 0, 503/503 Python tests
```

The final remote metadata gate validated 332 records. The exact veryquick
recipe also passed its HIR, runtime, bytecode-validator, and single-case
conformance commands. Generated Cargo targets and temporary directories were
removed after both gate rounds. The initial cleanup restored 65 GB free under
`/home/johannes`; after the final cleanup the shared builder reported 51 GB
free under `/home/johannes` and 3.8 GB under `/tmp`. The final clean source
clone was retained for review reproduction.

## Preserved Boundaries

- The board is 150/231. `VERIF-P8-002`, `VERIF-P8-005`, and
  `VERIF-P8-006` remain open.
- All 34 specification gaps remain open.
- All 52 invariants remain unvalidated at proof level `S0`.
- Both Phase 8 evidence rows use `proof_kind = "none"` with empty linked tests,
  invariants, and specification gaps.
- CI remains report-only; no workflow references the new audit command.
- The window is empty under product/runtime crates, editors, workflows, xtask,
  skills, agent instructions, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and
  `justfile`.
- `VERIF-STOP-012`, `VERIF-STOP-014`, `VERIF-P1A-002`,
  `VERIF-P1A-003`, `VERIF-P1A-006`, `VERIF-P1A-007`,
  `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P3-006`,
  `VERIF-P4A-005`, `VERIF-P5-000B`, `VERIF-P6-007` through
  `VERIF-P6-010`, `VERIF-P6A-005`, `VERIF-P7-002`, `VERIF-P8-002`,
  `VERIF-P8-005`, `VERIF-P8-006`, `VERIF-P10-001`, `VERIF-P10-003`,
  and `VERIF-P14-000` remain open.
