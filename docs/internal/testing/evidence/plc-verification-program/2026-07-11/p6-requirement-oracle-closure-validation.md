# Phase 6 Requirement And Oracle Audit Closure Validation

Date: 2026-07-11
Implementation commit: `d1cdfc0d98fb1452d7badba67e46aa42219e9f13`
Report and board checkpoint: `1755903f45dfd4d7ed366bef763e23115cca9034`

## Scope

This closure covers the Phase 5 review hardenings and `VERIF-P6-001` through
`VERIF-P6-006`. It adds a report-only audit of explicit requirement and oracle
associations for every committed invariant.

`VERIF-P6-007` remains open until `VERIF-P14-000` defines its grace rule.
`VERIF-P6-008` through `VERIF-P6-010` remain open because the public-claim and
live-test denominators are not exhaustive. The audit creates no proof, closes
no specification gap, and enables no enforcement.

No runtime, VM, compiler, IDE, extension, workflow, skill, agent instruction,
version, changelog, product test, case file, or catalog record changed.

## Review Fixes

- `TRUST_DIT_REQUIRE_HARDWARE=1` is accepted only on the exclusive
  `hardware_lab` entrypoint. Helper and cross-suite records fail validation.
- Path routing now uses one segment-aware matcher. `*` and `?` stay within a
  normalized POSIX segment; only a complete `**` segment recurses.
- The Phase 6 live guard pins the standing stop rows plus the incomplete source,
  public-claim, traceability, mutation, and grace-rule rows as open.
- Phase 6 imports the primary validator's oracle-authority vocabulary. The
  report schema drift-pins oracle states, source authorities, mapping rows, and
  boundary constants.
- A canonical semantic corruption fixture now passes JSON and regenerated
  Markdown through the full at-rest validator. That fixture exposed an
  insertion-order-dependent Markdown bug; boundary rendering now uses the
  contract's fixed order.

## Tests First

The Phase 5 hardening fixtures initially produced three failures: helper and
cross-suite hardware opt-in records were accepted, and a nested path matched a
single-segment performance glob. The Phase 6 test module initially failed to
import before its implementation existed. The later adversarial fixtures first
showed four unpinned schema corruptions and the full at-rest Markdown ordering
failure. Each now passes.

## Measured Audit

The invariant denominator is all 52 committed invariant records. Mappings come
only from explicit invariant spec, oracle, and gap references.

| Board row | Areas | Invariants | Eligible oracle | Spec-gap blocked |
| --- | --- | ---: | ---: | ---: |
| `VERIF-P6-001` | `compiler_iec` | 5 | 2 | 3 |
| `VERIF-P6-002` | `runtime_safety` | 10 | 4 | 6 |
| `VERIF-P6-003` | `protocols` | 7 | 1 | 6 |
| `VERIF-P6-004` | `editor_safety` | 6 | 0 | 6 |
| `VERIF-P6-005` | `control_security`, `supply_chain_platform` | 6 | 0 | 6 |

Across all areas, eight invariants have active, oracle-eligible explicit
sources and 44 remain blocked by open specification gaps. Thirty-four blocked
invariants use a future `VERIF-P6-007` high-risk class. They are reported, not
enforced. Public claims remain context and cannot become oracles.

## Generated Reports

All eight reports were generated independently from a clean
`d1cdfc0d98fb1452d7badba67e46aa42219e9f13` checkout with timestamp
`2026-07-11T01:30:00+02:00`, then validated at rest after the report evidence
row was indexed.

| Report JSON | SHA-256 |
| --- | --- |
| `test-class-completeness.json` | `3f18bb963e642f2a64d611fcd26929f3c100f0e328c16b13e5e5c6fcc28a63ab` |
| `coverage-matrix-gaps.json` | `0bfd4a4e44f93291079f5a6bcc7e1aa7c55526171766a89da142fe80b9e00065` |
| `malformed-input-coverage.json` | `b73f48990384521e4a56a0bdcfd710f55f1391880047774be61798c5fab1ace6` |
| `unmapped-test-debt.json` | `aacfaa6cae651411b7d8208ff601056b157c738e434759025c0f765ddf0e27c0` |
| `test-refactor-assessment.json` | `04ada28471c82c2c1c90596c00177325cc4692e1ed7b5792601a870d58a3ae13` |
| `spec-completeness.json` | `facfb725726c7d6ac833a3478bee35036e537b04f425b79fe00b5d9632a9119d` |
| `phase5-suite-audit.json` | `a71f06b664ccd2b20e5cbf0f048dded967d48810d0c6a7732d75ea9ca54f869e` |
| `requirement-oracle-audit.json` | `ba33665d79b00c65d76b9c0196b55fdbe74afd923095c64f0d8401c860dac5e3` |

The P3 ignored-test inventory and Phase 4 invariant-seed audit did not bind any
changed input and were not regenerated. Case-generator inputs, the test
catalog, case files, and bytecode-validator product sources did not change, so
the cases and mutation report were not regenerated.

## Local Validation

The canonical focused runner discovered 37 modules and passed 434/434 tests.
The focused Phase 5/6 contract command passed 63/63 tests. Both metadata
entrypoints validated 323 records before this closure row was indexed and 324
after indexing.

The following checks passed:

```text
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
`$HOME/projects/trust-platform-p6-validation-1755903f4` was clean at
`1755903f45dfd4d7ed366bef763e23115cca9034`. Rust and Cargo were run on
`x86_64-unknown-linux-gnu` with one isolated warmed target and a home-backed
`TMPDIR`.

```text
just fmt                       2.40 seconds, exit 0
just clippy                  152.57 seconds, exit 0
just verification-veryquick 224.77 seconds, exit 0
just test-all                577.30 seconds, exit 0
```

The clone remained clean. The generated target was deleted after the gates,
restoring 58 GB free under `/home/johannes`; the 215 MB source clone was kept
for review reproduction.

## Filesystem Cleanup

Local `/tmp` moved from 99 percent used with 100 MB free to 60 percent used
with 3.3 GB free. Cleanup removed 601 stale VS Code/LSP harness directories, a
generated docs virtual environment, and old MCP/render probe artifacts. Active
Claude state, source/evidence clones, registered worktrees, system directories,
and recovery bundles were preserved.

On `trust-builder`, generated `sccache`, a stale Scena target, stale mutation
scratch, and stale VS Code harness data were removed before the gates. The
unrelated 13 GB Cardine validation target and every source checkout were left
untouched.

## Preserved Boundaries

- `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P3-006`, `VERIF-P4A-005`,
  `VERIF-P5-000B`, `VERIF-P6-007` through `VERIF-P6-010`,
  `VERIF-P10-001`, `VERIF-P10-003`, and `VERIF-P14-000` remain open.
- All 34 specification gaps remain open.
- All 52 invariants remain unvalidated at proof level `S0`.
- CI remains report-only; no workflow contains the new report command.
- The new evidence rows use `proof_kind = "none"` with empty linked tests,
  invariants, and specification gaps.
