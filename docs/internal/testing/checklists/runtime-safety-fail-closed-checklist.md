# Runtime Safety Fail-Closed Checklist

Status: Phase 0 active - tracked board opened from clean `v0.24.14` baseline.
Owner: runtime safety
Contract: `docs/internal/architecture/runtime-safety-fail-closed-contract.md`
Gate: `scripts/runtime_safety_fail_closed_ast_grep_gate.sh`
Full-map check: `FULLMAP-RUNTIMESAFE`

This board covers runtime-internal safety paths that were outside the completed external-boundary board. Ignored local drafts are not source of truth; every checked row below must cite tracked code, a source-derived doctor result, and tests from this branch.

## Stop Rules

- [x] `RTSAFE-STOP-01` Tests or doctor findings come before production runtime fixes.
- [x] `RTSAFE-STOP-02` Phase 0 changes only checklist, contract, policy, doctor, and CI wiring.
- [x] `RTSAFE-STOP-03` Fail-open compatibility paths must be explicit, named, tested, and allowlisted.
- [x] `RTSAFE-STOP-04` The doctor must report live source evidence, not checklist-only claims.
- [ ] `RTSAFE-STOP-05` Do not flip `FULLMAP-RUNTIMESAFE` to fail-class until every finding is fixed or narrowly allowlisted.

## Phase 0 - Doctor Board And Contract

- [x] `RTSAFE-P0-001` Contract path is tracked: `docs/internal/architecture/runtime-safety-fail-closed-contract.md`.
- [x] `RTSAFE-P0-002` Allowlist path is tracked with max 5 entries: `docs/internal/architecture/runtime-safety-fail-closed-allowlist.toml`.
- [x] `RTSAFE-P0-003` Source-derived gate path is tracked: `scripts/runtime_safety_fail_closed_ast_grep_gate.sh`.
- [x] `RTSAFE-P0-004` Policy entry added: `xtask/config/full_map_policy.json` -> `runtime_safety_fail_closed`.
- [x] `RTSAFE-P0-005` Full-map check added as warn-only inventory: `FULLMAP-RUNTIMESAFE`.
- [x] `RTSAFE-P0-006` CI wiring added beside the runtime-boundary gate and uploads runtime-safety artifacts.
- [x] `RTSAFE-P0-007` Xtask unit coverage proves findings are warn-only during Phase 0. Evidence: `cargo test -p xtask runtime_safety_gate -- --nocapture` and `cargo test -p xtask full_map -- --nocapture`.
- [x] `RTSAFE-P0-008` Initial inventory run recorded below from the current tracked source.

Latest inventory command:

```text
./scripts/runtime_safety_fail_closed_ast_grep_gate.sh
```

Latest artifact:

```text
target/gate-artifacts/runtime-safety-fail-closed-<commit>/runtime-safety-summary.txt
```

Latest inventory summary:

```text
commit=0b5e52308
finding_count=82
allowlisted_count=0
phase=warn_only_inventory
```

| Rule | Count | Owner | Planned red test | Planned fix |
| --- | ---: | --- | --- | --- |
| `RUNTIMESAFE-INIT-NULL-FALLBACK` | 21 | runtime/init | `init_fail_closed` | Propagate init/default/materialization errors as typed failures instead of `Value::Null` fallback. |
| `RUNTIMESAFE-COERCE-WARNING-ONLY` | 18 | runtime/HIR | `coercion_proof` | Prove runtime widening behavior or add explicit coercion/rejection. |
| `RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL` | 11 | runtime/eval | `init_fail_closed` | Reject undefined evaluator/debug targets outside explicit setup APIs. |
| `RUNTIMESAFE-AUDIT-EVENT-DROP` | 9 | runtime/audit-event | `audit_durability` | Make audit/event send failures observable with counter/event evidence. |
| `RUNTIMESAFE-DRIVER-FAULT-OK` | 4 | runtime/IO | `io_fail_closed` | Return transport/freshness errors when drivers record fault/degraded state. |
| `RUNTIMESAFE-DISCOVERY-CONFIG-POLICY-OPEN` | 4 | runtime/IO | `io_fail_closed` | Fault discovery/config/image-size failures regardless of warn/ignore policy. |
| `RUNTIMESAFE-RETAIN-COMMIT-ORDER` | 3 | runtime/cycle | `runtime_safety_fail_closed` | Save due retain state before output commit and block outputs on retain failure. |
| `RUNTIMESAFE-RETAIN-NO-CHECKSUM` | 2 | runtime/retain | `retain_integrity` | Add payload length/checksum/trailer validation or named v1 migration path. |
| `RUNTIMESAFE-MESH-TIMEOUT-EMPTY` | 2 | runtime/mesh | `audit_durability` | Return timeout/error separately from successful empty snapshot. |
| `RUNTIMESAFE-IGNORED-FLUSH` | 2 | runtime/IO | `io_fail_closed` | Propagate safety-path flush failures. |
| `RUNTIMESAFE-GPIO-NO-HEALTH` | 2 | runtime/IO | `io_fail_closed` | Track GPIO last read/write failure through `IoDriverHealth`. |
| `RUNTIMESAFE-SAFE-STATE-DISCARD` | 1 | runtime/cycle | `runtime_safety_fail_closed` | Report safe-state write failures while preserving root fault. |
| `RUNTIMESAFE-RETAIN-ORPHAN-SILENT` | 1 | runtime/retain | `retain_integrity` | Emit retain orphan/migration evidence. |
| `RUNTIMESAFE-RETAIN-DIRECT-WRITE` | 1 | runtime/retain | `retain_integrity` | Persist retain through temp write, flush, fsync, atomic rename, and parent dir sync. |
| `RUNTIMESAFE-FEATURE-DISABLED-SILENT` | 1 | runtime/debug-control | `audit_durability` | Return structured `feature_disabled` response/event. |

Phase 0 validation:

```text
./scripts/runtime_safety_fail_closed_ast_grep_gate.sh
cargo test -p xtask runtime_safety_gate -- --nocapture
cargo test -p xtask full_map -- --nocapture
cargo run -p xtask -- architecture-doctor --full-map
```

Result: all commands exited 0. `FULLMAP-RUNTIMESAFE` is a warn-only `FINDING` with 82 current source findings and no allowlist entries.

Phase 3 I/O slice inventory:

```text
./scripts/runtime_safety_fail_closed_ast_grep_gate.sh
finding_count=73
allowlisted_count=0
phase=warn_only_inventory
```

Result: the I/O-specific fixed classes are no longer reported by the doctor. Remaining findings are later-phase init/eval/retain/cycle/audit/mesh/debug/HIR classes.

## Phase 1 - Red Tests

- [x] `RTSAFE-P1-001` I/O fail-closed tests cover MQTT freshness/publish/connect, EtherCAT image-size/policy faults, Modbus transport/exception taxonomy, and GPIO health. Red evidence:
  - `cargo test -p trust-runtime --lib fail_closed_ -- --ignored --nocapture` fails 3 MQTT tests because disconnected reads, connect failures, and publish failures return `Ok(())`.
  - `cargo test -p trust-runtime --test ethercat_driver ethercat_ -- --ignored --nocapture` fails 2 EtherCAT tests because warn policy turns write failure and image-size mismatch into `Ok(())`.
  - `cargo test -p trust-runtime --test modbus_driver modbus_ -- --ignored --nocapture` fails 2 Modbus tests because warn policy transport failure returns `Ok(())` and Modbus exception uses the generic I/O driver error.
  - `cargo test -p trust-runtime --lib io::gpio::tests::gpio_ -- --ignored --nocapture` fails 2 GPIO tests because read/write errors leave health as `Ok`.
- [ ] `RTSAFE-P1-002` Cycle ordering tests cover watchdog-before-output, retain-before-output, and safe-state write failure reporting.
- [ ] `RTSAFE-P1-003` Retain integrity tests cover corrupt data, trailing data, orphan globals, scalar widening, and struct add/remove migration.
- [ ] `RTSAFE-P1-004` Init/evaluator/debug tests cover init default failures, unknown assignment rejection, and queued debug write failure.
- [ ] `RTSAFE-P1-005` Audit/event/runtime-cloud/mesh/HMI tests cover event durability, audit drop evidence, corrupt persisted state, mesh timeout, slow WebSocket clients, and structured `feature_disabled`.
- [ ] `RTSAFE-P1-006` Coercion proof tests cover HIR-allowed widening and runtime bytecode results before any coercion refactor.

## Phase 2 - Shared Safety Contracts

- [ ] `RTSAFE-P2-001` Add only the runtime error/event variants proven necessary by Phase 1 tests.
- [ ] `RTSAFE-P2-002` Keep ownership narrow: I/O owns transport health, cycle owns output ordering, retain owns durability, debug/control owns request observability, and doctor owns source-pattern enforcement.
- [ ] `RTSAFE-P2-003` Existing event JSON remains compatible unless a red test proves a breaking change is required.

## Phase 3 - I/O Fail-Closed Fixes

- [x] `RTSAFE-P3-001` MQTT disconnected/stale reads fail with freshness errors by default; publish/connect failures fail with transport errors. Evidence:
  - `cargo test -p trust-runtime --lib fail_closed_ -- --ignored --nocapture`
  - `cargo test -p trust-runtime --test io_multidriver_live -- --nocapture`
- [x] `RTSAFE-P3-002` EtherCAT discovery and image-size mismatches fault under every policy; health uses max-severity semantics. Evidence:
  - `cargo test -p trust-runtime --lib io::ethercat::tests:: -- --nocapture`
  - `cargo test -p trust-runtime --test ethercat_driver -- --nocapture`
  - `cargo test -p trust-runtime --test ethercat_driver ethercat_ -- --ignored --nocapture`
- [x] `RTSAFE-P3-003` Modbus flush and transport errors propagate; Modbus exceptions are distinguishable from transport failures. Evidence:
  - `cargo test -p trust-runtime --test modbus_driver -- --nocapture`
  - `cargo test -p trust-runtime --test modbus_driver modbus_ -- --ignored --nocapture`
- [x] `RTSAFE-P3-004` GPIO exposes last read/write failure through driver health. Evidence:
  - `cargo test -p trust-runtime --lib io::gpio::tests::gpio_ -- --ignored --nocapture`
- [ ] `RTSAFE-P3-005` Runtime/control health remains unhealthy while any driver is faulted and faulted drivers emit structured `IoFault` evidence.

## Phase 4 - Cycle Ordering, Retain Commit, Safe State

- [ ] `RTSAFE-P4-001` Watchdog deadline breach before physical output commit prevents output writes.
- [ ] `RTSAFE-P4-002` Due retain save happens before physical output commit; retain save failure prevents output writes.
- [ ] `RTSAFE-P4-003` Safe-state write failures report `SafeStateFailed` while preserving the root fault.

## Phase 5 - Retain Integrity And Migration

- [ ] `RTSAFE-P5-001` Retain writes use temp file, flush, fsync, atomic rename, and parent directory sync.
- [ ] `RTSAFE-P5-002` Retain codec validates payload length, checksum, and trailer; v1 remains read-only migration input if needed.
- [ ] `RTSAFE-P5-003` Retain migration handles allowed widenings, declared defaults for added fields, named failures for unsafe changes, and orphan evidence.

## Phase 6 - Init, Evaluator, Debug Writes

- [ ] `RTSAFE-P6-001` Runtime init propagates default/materialization errors as `InitFailed`, not `Value::Null`.
- [ ] `RTSAFE-P6-002` Evaluator writes reject undefined targets instead of creating globals outside explicit setup APIs.
- [ ] `RTSAFE-P6-003` Queued debug writes validate/report failures through the cycle/control path.

## Phase 7 - HIR/Runtime Coercion Proof

- [ ] `RTSAFE-P7-001` Prove allowed widening and narrowing rejection across function input, output, InOut, assignment, initializer, return value, and bytecode execution.
- [ ] `RTSAFE-P7-002` If proof shows a gap, add explicit lowering/runtime coercion; otherwise record evidence and do not refactor.

## Phase 8 - Audit, Event, Runtime-Cloud, Mesh, HMI Robustness

- [ ] `RTSAFE-P8-001` Audit/event send/write failures are observable and safety events are durable enough for the covered path.
- [ ] `RTSAFE-P8-002` Runtime-cloud corrupt persisted state returns error/degraded state, not defaults.
- [ ] `RTSAFE-P8-003` Mesh timeout is distinguishable from a successful empty snapshot.
- [ ] `RTSAFE-P8-004` HMI slow WebSocket clients are dropped with event/log evidence.
- [ ] `RTSAFE-P8-005` Debug feature disabled returns structured `feature_disabled`.

## Phase 9 - Flip, Mutation, Docs, Release

- [ ] `RTSAFE-P9-001` `FULLMAP-RUNTIMESAFE` is fail-class only after all findings are fixed or narrowly allowlisted.
- [ ] `RTSAFE-P9-002` Mutation shards or equivalent evidence cover the changed classes.
- [ ] `RTSAFE-P9-003` Diagrams/checklists/changelog/version are updated for release-notable runtime behavior.
- [ ] `RTSAFE-P9-004` Final gates pass: `just fmt`, `just clippy`, `just test-all`, runtime vertical tests, runtime-safety gate, and full-map doctor.
- [ ] `RTSAFE-P9-005` Version bump, tag, release workflow, and GitHub latest release are confirmed if runtime behavior changes ship.
