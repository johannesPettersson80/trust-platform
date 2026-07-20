# Non-Catalog Ledger Batch Validation

Date: 2026-07-17

Implementation checkpoint:
`0916ffe590363739dfe4e2488fb990cc231b3a7c`

Report-rebind checkpoint:
`765cdabb7baca57d682b877dcc6667418cdef293`

Final validated checkpoint:
`af4cb3cd7130aac9ba9ee7ee146fd162996f99d5`

## Outcome

This batch closed the executable non-catalog ledgers without bulk-mapping the
remaining scanner catalog debt:

- all 21 conformance cases have explicit reviewed catalog identities and pass
  two consecutive conformance runs;
- the six runtime-anomaly gap classes now have direct, runnable tests backed by
  written runtime requirements;
- all eight required fuzz surfaces have direct cargo-fuzz targets, including
  six new targets checked and exercised for 1,000 bounded runs each;
- the ignored-test register contains 23 reviewed observations, zero conditional
  observations, zero warnings, and zero unknown classifications; and
- the VS Code extension and real code-server command-palette capture pass after
  removal of obsolete skip/capture debt.

`VERIF-P7-002` is closed because every conformance case now has an explicit
catalog link. `VERIF-P8-002` remains open because this batch did not establish
an exhaustive reviewed runtime-safety fact/non-mapping denominator.
`VERIF-P9-005` remains open because crash-to-regression handoff is not yet an
exhaustive governed ledger. No CI enforcement was changed.

## Defects Found And Fixed

The new execution found two stale test/evidence defects:

1. `conformance/expected/timers/cfm_timers_tof_sequence_002.json` still expected
   `ET = 0` after TOF expiry. The written and shipped contract holds `ET = PT`
   until rearm, so the stale expected artifact was corrected to `10_000_000`.
2. The code-server Playwright capture still searched for obsolete command
   titles and expected commands that are intentionally hidden by the current
   package UX contract. The obsolete runtime-panel capture was retired and the
   command-palette capture now uses current public `truST:` commands.

The final comprehensive verification run also caught two stale mechanical
census assertions after the intentional source/catalog additions. They were
updated to the regenerated live values: 4,023 facts, 241 mapped, 3,782
unmapped, and 74 `trust-runtime-core` unit-test facts.

The six new runtime-anomaly tests and six bounded fuzz campaigns found no new
product runtime defect. This is an executed negative result, not a claim that
the remaining runtime denominator is exhaustive.

## Executed Coverage

### Conformance

The full suite ran twice on `trust-builder`:

- 21 manifests passed on both runs;
- normalized ordering and status output was byte-stable; and
- both generated result documents passed their schemas.

The alignment report now records 21 of 21 explicitly linked cases. The ten v2
category rows remain honest about semantic oracle assessment; linkage alone is
not proof.

### Runtime Anomalies

The direct tests cover:

- backward monotonic-clock movement;
- forward monotonic-clock discontinuity/coalescing;
- exact SIGINT graceful-stop mapping;
- exact SIGTERM graceful-stop mapping;
- bounded OPC UA queue/recovery behavior; and
- bytecode resource-bound rejection.

The live audit records 19 classes, 52 explicit associations, 19 runnable
classes, and zero class-gap rows. No association was upgraded by name, path, or
prose inference.

### Fuzzing

The six new cargo-fuzz targets cover HIR lowering, PLCopen XML, bytecode
containers, runtime configuration, LSP incremental edits, and HMI payloads.
`cargo +nightly fuzz check` passed for all six. Each target then completed a
bounded 1,000-run campaign on `trust-builder`; no crash or regression seed was
produced. The audit records 17 executable targets, eight required surfaces, and
zero surface-gap rows.

### Ignored And Browser Tests

The ledger reconciliation removed obsolete ignores from OpenOT, EtherCAT,
Modbus/MQTT, and VS Code coverage, and retired the obsolete runtime-panel
capture. Results:

- ignored-test live join: 23 discovered / 23 registered / 0 unknown;
- VS Code Electron suite under Xvfb: 458 passed;
- real Docker/code-server Playwright command-palette capture: 1 passed;
- the new 1,440 x 900 command-palette screenshot was visually inspected; and
- strict public-doc asset validation passed against the built site.

## Generated Report Binding

Thirteen affected report pairs were generated sequentially from a pristine
worktree at `af4cb3cd7130aac9ba9ee7ee146fd162996f99d5` with timestamp
`2026-07-17T23:15:00+02:00`. The worktree was restored to pristine before each
generation. The ignored-test inventory and Phase 5 suite audit remain bound to
`0916ffe590363739dfe4e2488fb990cc231b3a7c` at
`2026-07-17T20:50:00+02:00` because their declared input closures exclude the
two census-test files changed at the final checkpoint. Every production
validator passed at generation and at rest.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `44e76c04b8e55ffc7573e755e858b0bdb960b4bdb248f8b3fc599d64b12edb11` |
| Coverage-matrix gaps | `64ac85b17d3cf7e1421ab905e19f8688a174a0457834a8c65dae9d7e7030c443` |
| Malformed-input coverage | `81dd7da704a80a82d93ec247b43101ba76149900fe8c5672a574e42a120b9d65` |
| Unmapped-test debt | `5875aad181cc538b9787c0809191bba8c80215053a78eaf08ac2331a8c8d0bcc` |
| Test-refactor assessment | `6ac7badad32122e0d95c84653774f89451ef9e34d80d793ab579a286bafdee7f` |
| Ignored-test inventory | `a64f21ca30e44fb9dcba367d6b3add8ec0811a8ec0e897ef1b0672c3525a19ef` |
| Phase 5 suite audit | `13ca1221f17059f56c9b9a4ae8ab60d32c1760994c3e32244203efcaa666dd9b` |
| Invariant-seed audit | `c9b024d5126ef5f0a6232f745f0ae8ae13e7bddbba83ba05ed3e88191cf33ddd` |
| Specification completeness | `e55bea08a8e2117e2e3db0a6666f906209eb22bbc67b0a1d8a1ad5a0e08e3f6c` |
| Requirement/oracle audit | `96932632f0cae96d0aa833aaa2ed056efda770e10e2b720b9f4dbfeac191ee45` |
| Conformance alignment | `5127a14e5f28e52f318e672c28b528b6dba86f2620bfa983388ef807f7990a2d` |
| Runtime-anomaly audit | `113fea8cbd6c5d63f57eed2ba3da0de1102b9d5d7fd36d0de08b727e7d24bf9b` |
| Fuzz-program audit | `fd49daa9c6eaf99ed43b7ef0c59bc019e7704b8dcc224188d623a385e5e0b8e5` |
| Mutation program | `75950d772b4d3a3b5e9b44f5933bb30a3d12847d248ad8c7fa796b50b1675d8d` |
| Specification-source audit | `e789bf2418dfc94427f93ea6bbf43ea346c81f3cdd5630ab0b7fb55cd4dd0812` |

Key report results are 4,023 scanner facts with 241 mapped and 3,782 unmapped;
21 of 21 conformance cases linked; 19 of 19 runtime-anomaly classes runnable;
17 fuzz targets covering all eight required surfaces; six mutation shards
measured with zero survivors; 54 of 54 invariants with eligible oracles; and
zero unknown ignored-test observations.

## Final Validation

Final validation ran on the isolated `trust-builder` worktree
`$HOME/projects/trust-platform-noncatalog-report-regen`:

- `just fmt`: passed in 2.61 seconds at the report checkpoint;
- `just clippy`: passed in 108.75 seconds at the report checkpoint;
- `just verification-veryquick`: 815 focused Python tests plus metadata,
  generated-case, fast HIR/runtime, bytecode, and conformance checks passed in
  1,262.20 seconds at `af4cb3cd`;
- `CARGO_INCREMENTAL=0 just test-all`: passed with zero failures in 784.43
  seconds at `af4cb3cd`;
- explicit `api_smoke`, `debug_control`, `complete_program`, and
  `runtime_reliability`: 3 + 20 + 1 + 4 tests passed in 211.87 seconds;
- `./scripts/prepush_ci_gate.sh`: passed in 891.49 seconds, including path and
  IEC-log hygiene, warning-deny checks, 167 passing LSP tests with 10 intentional
  performance ignores, Windows compilation, and mesh TLS stability 8/8;
- strict MkDocs build: passed in 3.95 seconds;
- public-doc asset validation: passed;
- diagram source/manifest drift validation: passed; and
- metadata validation: 725 records before this evidence row was indexed.

The first two `test-all` attempts failed during linking with `No space left on
device`; neither reached test execution. Work stopped after each failure, no
compiler/linker process was left running, only generated cache/target output
was removed, disk was rechecked, and the successful retry started with the
required 60 GiB available. The successful target grew to 53 GiB. These are
infrastructure failures and are not counted as product test failures.

## Honest Remaining Debt

- The hand-owned catalog maps 241 of 4,023 scanner facts; the remaining 3,782
  facts are catalog debt, not automatically missing product tests.
- `VERIF-P8-002` still requires an exhaustive runtime-safety denominator.
- `VERIF-P9-005` still requires governed crash-to-regression handoff.
- Ten conformance v2 categories remain `not_assessed` for semantic-oracle
  alignment even though all cases are explicitly linked and runnable.
- The specification-source audit reports 14,482 unreviewed prose blocks and
  127 warnings; this remains report-only review debt.
- No new proof row or invariant promotion was created by this association and
  adequacy batch.
