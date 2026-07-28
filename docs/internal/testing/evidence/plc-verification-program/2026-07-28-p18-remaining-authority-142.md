# Phase 18 remaining-authority campaign

Date: 2026-07-28

## Scope and result

This campaign deliberately processes the complete frozen
`no_reviewed_spec_or_invariant_binding` population as one coherent transaction:
142 tests in 92 source paths. It does not split the population into per-file
completion claims.

The reviewed result is:

- 139 tests are catalog-mapped to nine bounded invariants;
- 3 tests remain owned nonmapping facts with reason
  `assertion_oracle_unresolved`;
- all 142 former missing-authority facts leave
  `no_reviewed_spec_or_invariant_binding`;
- the frozen missing-specification-authority equation becomes zero.

The nine invariant authorities are:

- `VERIFICATION_CASE_MODEL_CONFORMANCE_001`;
- `RT_ADS_FOUNDATION_CONFORMANCE_001`;
- `BROWSER_ANALYSIS_FOUNDATION_CONFORMANCE_001`;
- `DEV_WORKBENCH_CONFORMANCE_001`;
- `RT_AGENT_COMMAND_CONFORMANCE_001`;
- `RT_HOST_LIFECYCLE_CONFORMANCE_001`;
- `RT_EXECUTION_LANGUAGE_CONFORMANCE_001`;
- `RT_HOST_SECURITY_WEB_CONFORMANCE_001`;
- `IEC_PROJECT_EXAMPLE_CONFORMANCE_001`.

The review lock covers 142 tests and 1,094 extracted assertions. Its assertion
inventory digest is
`227c299f1126308e3f329485e55e178ba46c4357588d2a4ecc110365f1f3b42c`;
its identity/semantic digest is
`5c1422cb6e177013052c88e0b8e9b04033227c808c25534c262b4b0bfc00541e`.

The three withheld tests use dynamic external observations that the static
assertion extractor intentionally does not pretend to resolve:

| path and test | unresolved oracle |
| --- | --- |
| `crates/trust-hir/tests/iec_table_coverage.rs::iec_table_coverage_report` | recursive repository-source discovery and file reads |
| `crates/trust-syntax/tests/parser_project_examples.rs::test_examples_parse` | dynamically discovered example-corpus file reads |
| `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_05_part_01.rs::representative_corpus_memory_budget_gate` | process RSS observed through `/proc/self/status` |

They were reclassified to `assertion_oracle_unresolved`; they were not falsely
catalog-mapped.

## Test-first and behavior-lock evidence

The campaign extended the frozen nextest adapter only through focused
red-green changes:

1. An unselected source-ignored test initially made normalization fail with
   `unexpected test result`. The new
   `test_normalizer_ignores_unselected_ignored_test_events` reproduced that
   expected failure. The adapter now permits only unselected `started` and
   `ignored` events while continuing to reject every unexpected executed
   terminal result.
2. Evidence-name recovery initially counted an unselected ignored test and
   failed its selected-count assertion. The new
   `test_evidence_selection_excludes_unselected_ignored_tests` reproduced that
   failure. Immutable evidence selection now excludes ignored terminal events.
3. The assignment transaction initially tried to map all 142 tests. The
   focused partition test expected 139 mapped and 3 owned facts and failed
   `139 != 142`. The generator now emits the honest 139-plus-3 partition.
4. Closed-profile validation first rejected the reviewed `stmt_jmp` path.
   Expanding only the applicable frozen profile made the exact profile
   assertion green.
5. Before the assignment generator existed, its focused tests failed on the
   missing module. The completed generator and adapter suite now passes:

   ```text
   python3 -m unittest \
     scripts.verification.remaining_authority_assignments_tests \
     scripts.verification.frozen_rust_nextest_batch_tests

   Ran 49 tests
   OK
   ```

6. The existing zero-assertion
   `crates/trust-runtime/tests/value_types.rs::supports_elementary_types`
   compile smoke passed before editing. It was strengthened to assert the exact
   elementary-value sequence plus default date/time profile epoch and
   resolution, and the same exact test passed afterward on `trust-builder`.

The same 49 Python tests also pass in the isolated builder checkout.

## Focused remote execution

Proof environment:

- evidence host: `trust-builder` (`scena-rust-builder`);
- canonical remote source:
  `/home/johannes/projects/trust-platform`;
- isolated destination:
  `/home/johannes/projects/trust-platform-p18-remaining-142`;
- destination detached base:
  `3479807f26f56b8347d588d658cd9301d6735050`;
- local source branch: `plc-verification-program`;
- local starting HEAD:
  `15775632ecef79a1c9e031a06c1abfffd0bf5df9`;
- exact local source snapshot synchronized to the destination;
- destination `AGENTS.md` and complete `.codex/skills/**` manually copied and
  verified byte-for-byte against the canonical source before execution;
- initial disk preflight: 41 GiB available under `/home/johannes` and
  3.3 GiB under `/tmp`, sufficient for this focused campaign but not used as a
  final broad-gate claim.

All evidence-producing build and test commands ran through
`ssh trust-builder`. The frozen campaign selected the 142 reviewed tests and
79 already-mapped companions required by their exact binaries. Across 92
source paths it produced:

| result | count |
| --- | ---: |
| exact test results | 221 |
| passed | 221 |
| failed | 0 |
| pending | 0 |

After the source-ignored adapter fix, the 46 already completed path artifacts
were normalized from their immutable raw evidence and the remaining 46 paths
were executed. No completed test path was rerun merely to repair the
normalizer. The one mappable test colocated with the withheld syntax corpus
test was then executed through an exact mapping-only selection and passed 1/1.

Raw and normalized artifacts are retained under:

```text
target/gate-artifacts/verification/p18-remaining-authority-142/execution/
target/gate-artifacts/verification/p18-remaining-authority-142/mapping-execution/
```

## Atomic catalog transaction

Ninety path plans cover the 139 mapped tests. Eighty-nine plans passed their
first remote dry run; regenerated plans were proved semantically identical for
those paths, and the syntax override passed its separate dry run. The three
classification changes also passed dry run.

The isolated builder then applied all 90 mapping plans and the 3 nonmapping
classifications without a failure. The resulting 13 metadata files were
copied back and applied locally as one recoverable transaction with exact
original-file checks and postcondition validation:

- `verification/test-catalog.toml`;
- `verification/test-catalog-denominator.toml`;
- `verification/test-catalog-proposals.toml`;
- `verification/test-catalog-mapping-decisions.toml`;
- the nine invariant files named above.

All nine new invariants pass scoped metadata validation locally and on
`trust-builder`. The aggregate metadata validator is not green: it reports
1,291 pre-existing findings locally and 1,331 in the older detached remote
base. The required planner likewise exited 6 on that repository-wide baseline
after accepting the intent and complete changed-path set. These harness
baselines are recorded as blocking debt; neither is misreported as product
expected-red evidence. The post-transaction
`python3 scripts/check_test_catalog_staleness.py` check also exits nonzero
through the same aggregate validator and existing stale-evidence/proposal
population; it does not yield a clean repository-wide staleness claim.

## Frozen equations

The metadata-only report at
`target/gate-artifacts/verification/p18-remaining-authority-142/frozen-gaps-after-local.json`
records:

| population | before | after |
| --- | ---: | ---: |
| frozen live tests | 4,393 | 4,393 |
| `catalog_mapped` | 3,744 | 3,883 |
| `reviewed_nonmapping` | 649 | 510 |
| missing specification authority | 142 | 0 |
| owned nonmapping | 507 | 510 |
| specified gap-open cells | 207 | 216 |
| confirmed missing tests | blocked at 0 | 422 |

The 510 owned nonmapping facts are:

| reason | count |
| --- | ---: |
| `assertion_oracle_unresolved` | 290 |
| `fuzz_program_owned` | 8 |
| `gate_inventory_owned` | 59 |
| `ignored_test_registry_owned` | 23 |
| `not_catalogable_product_oracle` | 130 |

The 422 confirmed missing tests are not a regression. Their calculation was
fail-closed while missing authority was nonzero; reaching zero made the
specified behavior-demand ledger runnable for the first time. They are the
next large `P18-006` proof campaign. Thirty-one invariants also remain in the
exact-behavior-mapping debt set.

## Deferred proof

This transaction closes the frozen missing-authority population, not
`VERIF-P18-003`, `VERIF-P18-005`, or `VERIF-P18-006` globally. Code
specification gaps, all 510 nonmapping terminal facts, 422 now-confirmed
missing behavior tests, 31 exact-mapping debts, evidence registration,
reachability, mutation sensitivity, and later adequacy rows remain open.

No broad workspace `just fmt`, `just clippy`, `just test`, or `just test-all`
was run. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
