# Phase 10 Mutation Program Closure Validation

Date: 2026-07-12
Implementation source: `9eacebaa6f272d8a76f038e777279bc69067b5c2`
Report and board checkpoint: `70f7f6215d9148fe28eb97160c2a5d7344746caf`

## Scope

This slice closes `VERIF-P10-001` through `VERIF-P10-006` as a report-only
mutation-program contract. It defines focused mutation adequacy machinery but
does not change runtime or product behavior, execute the five newly planned
shards, collect code coverage, create proof, promote an invariant, close a
specification gap, or enable CI enforcement.

Concurrent reviewer work proposing a later Phase 16 execution slice is outside
this evidence. The modified umbrella checklist, modified implementation-board
addition, and untracked `execution-slice-001.md` are absent from checkpoint
`70f7f6215` and were neither staged nor used as Phase 10 evidence.

## Tests First And Hardening

The slice adds 30 focused regression and adversarial tests across the manifest
contract, report contract, real CLI entrypoints, and legacy bytecode outcome
handling. Initial tests failed because the Phase 10 modules did not exist. A
later real root-script invocation exposed an import-path defect that direct
module tests had not exercised; a failing CLI regression fixture was committed
with the entrypoint fix before report generation.

The fixtures pin:

- six exact ordered shard definitions and seven selected single-file mutants;
- exact source, function, selector, invariant, and live test identities;
- planned shards having empty outcomes and no execution claim;
- focused limits that reject broad regexes, workspace-wide commands, and large
  shards;
- raw exit/timeout-derived outcomes and infrastructure-error precedence;
- exhaustive survivor derivation and resolved durable survivor actions;
- delivered-artifact digest plus direct execution when delivered-build
  confirmation applies;
- zero coverage runs, null percentages, and adequacy-only/non-proof posture;
- canonical JSON, exact Markdown reconstruction, provenance closure, schema
  drift, output collision, and live recomputation;
- hostile TOML, JSON, schema, and legacy report shapes failing without a
  traceback; and
- real root-level report and validator entrypoints.

Independent mutation of the manifest, generic report, schemas, and legacy
report exercised 19,262 hostile variants with zero exceptions; all
honesty-relevant variants were rejected. Platform-label changes remain the
documented format-only provenance class, not a semantic report change.

## Measured Inventory

| Measure | Result |
| --- | ---: |
| Shards defined | 6 |
| Mutants defined | 7 |
| Measured shards | 1 |
| Planned shards | 5 |
| Measured mutants | 2 |
| Caught | 2 |
| Survived / unviable / timeout / error | 0 / 0 / 0 / 0 |
| Coverage runs | 0 |

Only `MUTATION_SHARD_BYTECODE_VALIDATOR_001` is measured. The remote isolated
pilot ran at clean commit
`3d935a063be65fdf0d4dbdf3ceaaa66dc9382ed1` with cargo-mutants 27.0.0. It
finished in 232.63 seconds with two caught mutants and no other outcomes. Its
durable JSON SHA-256 is
`1f979e4da3388a5eec422390f4ba9561faa9c1839c98d012cd065358ce885b83`.

Runtime value conversion, HIR diagnostics, parser recovery, retain/restart,
and connector status projection remain planned with empty result arrays.
Selector discovery and test association are not execution evidence. The
connector-projection shard cannot become measured without a delivered artifact
SHA-256 and direct execution confirmation. Associated test and case IDs remain
traceability labels, never killed-by or executed-test claims.

## Report Reproduction

All fourteen report pairs were generated from clean source
`9eacebaa6f272d8a76f038e777279bc69067b5c2` with timestamp
`2026-07-12T00:36:00+02:00`, using one pristine detached worktree per report.
Every generator and matching at-rest validator exited zero, and every generated
Markdown file was byte-identical to its committed artifact.

| Report JSON | SHA-256 |
| --- | --- |
| `test-class-completeness.json` | `8f18466debde147b436026f9543eb8753fec5e27c19ad2d34aed0ce28fcebde7` |
| `coverage-matrix-gaps.json` | `fd2162ab1cb6536be4b9517965d734e4520a5f1853e9c6de7b0b83677b686777` |
| `malformed-input-coverage.json` | `6a6f27edba191b1fb6aa474936b88dc33a696d19b99c160bc4115e0af2e1cbe0` |
| `unmapped-test-debt.json` | `20a711ad9bf735b72c7898b7852d55a146e8e4b0922ab4eabde20afccd3d7278` |
| `test-refactor-assessment.json` | `ae8c0cd50a074880a7f76c653b890278674deb3d38e8dc26bb9ac1f34afd2cbb` |
| `ignored-test-inventory.json` | `6eb80656e12ceaf8c295f09c2ff9e531fea2902391a62698b4b485d73d4bb358` |
| `invariant-seed-audit.json` | `62eb081193af8bbb2d40d4935a7d1aaf0936cd39f8604c5455573bfbc30d30ab` |
| `spec-completeness.json` | `acca505ce57a7ade1258783d7f2c659d91995b1c120b2da155ac39165355f7dc` |
| `phase5-suite-audit.json` | `598a45481d85bf7a0fd52d8910881aa1ea77643639f7bd1c327e2ab376536369` |
| `requirement-oracle-audit.json` | `a130315b10d4e92fe7269869df9f3d89826571b9b97306d4c505d1f95d7100ed` |
| `conformance-alignment.json` | `bc5148a1bf4211b2c9fd9cd58e23c6396fd47aa7d8ac9602acb01e8f9bd15bbf` |
| `runtime-anomaly-audit.json` | `f21ab36b9f0cc7d165eab5b4ab7e6b16041581c1c99bb4e9685a1d5ec89b879d` |
| `fuzz-program-audit.json` | `a353405f11f8dffa43e288f36466d5219f5348941e45efe11e460e5eff9241b3` |
| `p10-mutation-survivor-report.json` | `cd41cb7c7b3536510df4dfee3004863ba8f199acc59d6f609d368b0bda95bc52` |

The Phase 10 Markdown SHA-256 is
`12587c6aef6f4bad3f98abfec2782820343cc824f8358cf0f3d9eaec0dd69beb`;
its input digest is
`sha256:e9d16f83e7d75c68d70918ad794471d1d049c382d461665babfaa6c5832288fe`.

## Local Validation

The canonical focused runner discovered 50 modules and passed 579/579 tests in
690.293 seconds. Both metadata entrypoints validated 337 records at the report
checkpoint. All fourteen report pairs passed at-rest validation.

Additional local results:

- verification-tooling self-tests: 27/27;
- ignored-test join: 88 discovered, 88 registered, 63 unknown, 0
  catalog-mapped;
- catalog staleness: 6 records against 3,816 scanner facts;
- VS Code registration: 456 facts, 38 files, 38 registrations;
- refactor proposals: 1 proposal, 0 redirects, 6 catalog records, 3,816 facts;
- all four `gen_cases.py --check` invocations passed; and
- `git diff --check 4efeb793d..70f7f6215` passed.

## Remote Validation

The retained builder worktree
`$HOME/projects/trust-platform-p10-validation-3d935a063` was clean at exact
checkpoint `70f7f6215d9148fe28eb97160c2a5d7344746caf`. The host was
`x86_64-unknown-linux-gnu` with rustc 1.95.0 and cargo 1.95.0. Broad gates used
the warmed `$HOME/.cache/codex-targets/trust-platform-gate` target.

```text
just fmt                       2.41 seconds, exit 0
just clippy                   67.88 seconds, exit 0
just verification-veryquick 414.09 seconds, exit 0, 579/579 Python tests
just test-all                391.02 seconds, exit 0
```

Both metadata entrypoints reported 337 records, and all fourteen staged report
pairs passed at-rest validation on the builder. The final worktree remained
clean. Before the broad rerun, only stale generated Cardine/diagram targets and
`sccache` were removed; every source checkout and the active warmed target were
preserved.

## Preserved Boundaries

- The committed board is 161/231; the six flips are exactly
  `VERIF-P10-001` through `VERIF-P10-006`.
- All 20 standing guarded rows remain open, including `VERIF-P1B-012`,
  `VERIF-P1B-014`, `VERIF-P7-002`, `VERIF-P8-002`, `VERIF-P9-005`, and
  `VERIF-P14-000`.
- `VERIF-STOP-012` and `VERIF-STOP-014` remain open.
- All 34 specification gaps remain `spec_gap/open`.
- All 52 invariants remain at `S0`: 44 `spec_gap`, 8 `gap_open`, none
  validated.
- Phase 10 evidence uses `proof_kind = "none"` with empty invariant, test, and
  gap links.
- CI remains report-only; no workflow or `justfile` wiring was added.
- Workspace version remains 0.24.29.
- The committed window is empty under runtime/product code, product tests,
  editors, `xtask`, workflows, skills, agent instructions, public/spec docs,
  conformance, fuzz sources, Cargo manifests/lockfile, changelog, and
  `justfile`.

## SOLID, KISS, And DRY

Phase 10 is split by responsibility: CLI orchestration is 125 lines, manifest
contract 796, live/provenance state 518, report rendering 216, report contract
736, and validation wrapper 206. The metadata core remains 972 lines and
contains only delegation and wiring. Existing scanner, report-input, schema,
and bytecode-report helpers are reused; the narrow P1B pilot contract remains
separate from the generic Phase 10 format. No god file or mixed
product/verification ownership was introduced.
