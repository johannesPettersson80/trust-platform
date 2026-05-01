# Runtime Core / Linux Host Split Execution Checklist

Status: Planned
Owner: Runtime team
Scope: behavior-preserving split of portable runtime execution from the current Linux/product host.

This checklist turns the architecture plan in `docs/internal/architecture/full-software-map-audit-2026-04-28.md` into an execution board.

This work revives only the useful SOLID/KISS part of the deferred runtime-core/native-host split. It does not restart the embedded product roadmap.

This checklist is not the whole architecture cleanup program. It preserves and extracts runtime execution behavior. It does not close the HIR mutation gap, parser recovery gap, product/workbench CLI split, or HMI/web/control/cloud ownership split by itself. Track the full program in `full-architecture-refactor-program-checklist.md`.

## Goals

- [ ] `RTSPLIT-GOAL-01` Extract a portable `trust-runtime-core` boundary without changing shipped Linux runtime behavior.
- [ ] `RTSPLIT-GOAL-02` Keep `trust-runtime` as the Linux/product host during the first split.
- [ ] `RTSPLIT-GOAL-03` Make runtime execution, host transport, UI/control/cloud, and workbench/dev tooling separate responsibilities.
- [ ] `RTSPLIT-GOAL-04` Add automated doctor rules so the split cannot silently drift back.
- [ ] `RTSPLIT-GOAL-05` Preserve bytecode/runtime behavior, scheduler behavior, retain behavior, value invariants, and runtime vertical tests throughout the split.

## Explicit Non-Goals

- [ ] `RTSPLIT-NOTNOW-01` No STM32H7 hardware bring-up in this branch family.
- [ ] `RTSPLIT-NOTNOW-02` No Arduino Opta acceptance work in this branch family.
- [ ] `RTSPLIT-NOTNOW-03` No ESP32 host follow-up in this branch family.
- [ ] `RTSPLIT-NOTNOW-04` No embedded `T0` backend in this branch family.
- [ ] `RTSPLIT-NOTNOW-05` No embedded EtherCAT backend in this branch family.
- [ ] `RTSPLIT-NOTNOW-06` No `no_std` product promise in this branch family.
- [ ] `RTSPLIT-NOTNOW-07` No MCU Modbus RTU/TCP or MQTT commitment in this branch family.
- [ ] `RTSPLIT-NOTNOW-08` No marketing/support claim for embedded runtime support.
- [ ] `RTSPLIT-NOTNOW-09` No broad user-visible runtime behavior change mixed into the split.

## Stop Rules

- [ ] `RTSPLIT-STOP-01` Stop if a moved module changes public runtime behavior without a dedicated behavior-change issue.
- [ ] `RTSPLIT-STOP-02` Stop if `trust-runtime-core` pulls in host-only dependencies such as web, cloud, mesh, Tokio, EtherCrab, OPC UA, TUI, or IDE/LSP crates.
- [ ] `RTSPLIT-STOP-03` Stop if behavior-lock tests are missing for the slice being moved.
- [ ] `RTSPLIT-STOP-04` Stop if runtime vertical tests fail and the failure is not understood.
- [ ] `RTSPLIT-STOP-05` Stop if diagrams or generated maps are refreshed before factual doctor checks pass.
- [ ] `RTSPLIT-STOP-06` Stop if a slice requires embedded assumptions to compile or pass on the current Linux host.
- [ ] `RTSPLIT-STOP-07` Stop if a file split is only cosmetic and does not create a clearer owner/test boundary.

## Phase 0 - Scope Freeze And Baseline Evidence

- [x] `RTSPLIT-P0-001` Confirm this checklist is linked from the full-software audit and `architecture-improvements.md`; if a checklist index exists in the active branch, link it there too. Evidence: `full-software-map-audit-2026-04-28.md` links this checklist in F5/F7/F12 execution-board sections, and `architecture-improvements.md` records `ARCH-RTCORE-03`.
- [x] `RTSPLIT-P0-002` Confirm the full-software audit references this execution checklist. Evidence: `full-software-map-audit-2026-04-28.md` references `runtime-core-host-split-execution-checklist.md` in the Phase 6/7 runtime core/host split sections.
- [x] `RTSPLIT-P0-003` Confirm the old deferred embedded checklist remains deferred and is not treated as active execution scope. Evidence: `runtime-core-native-host-split-checklist.md` status remains `Deferred` and says it is parked reference material only.
- [x] `RTSPLIT-P0-004` Capture current branch, commit, and dirty tree state before implementation begins. Evidence: baseline captured on branch `architecture/runtime-module-decision-freeze` at commit `1ffec4ab0`; dirty state during capture contained only doctor/checklist maintenance, and no runtime production code movement had started.
- [x] `RTSPLIT-P0-005` Capture current `cargo metadata --no-deps --format-version=1` for workspace target/package shape. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/cargo-metadata-no-deps.json`.
- [x] `RTSPLIT-P0-006` Capture current `cargo tree -p trust-runtime --edges normal --depth 2`. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/cargo-tree-trust-runtime-depth2.txt`.
- [x] `RTSPLIT-P0-007` Capture current `cargo modules structure -p trust-runtime --lib --no-types`. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/cargo-modules-structure-trust-runtime-lib.txt`.
- [x] `RTSPLIT-P0-008` Capture current largest-file list for `crates/trust-runtime/src`. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/trust-runtime-largest-files.txt`.
- [x] `RTSPLIT-P0-009` Capture current public API snapshot for `trust-runtime` if `cargo public-api` is available. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/public-api-trust-runtime-default-features.txt` from `cargo public-api -p trust-runtime --color never`; the progress log records the 155.9s local run and timeout guard.
- [x] `RTSPLIT-P0-010` Record unavailable tools and exact blockers instead of silently skipping them. Evidence: no unavailable tool remains for Phase 0; `public-api-progress.log` records why the API snapshot is expensive on this ARM64 host, and `public-api-blocker.txt` records the aborted overbroad `--all-features` attempt so it is not mistaken for a silent skip.
- [x] `RTSPLIT-P0-011` Run `cargo run -p xtask -- architecture-doctor --all` and attach output. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/architecture-doctor-all.txt` passes after fixing stale legacy doctor checks for bounded positional-initializer scanning and ignored internal diagram scratch files.
- [x] `RTSPLIT-P0-012` Run the current generated software map command if available and attach output. Evidence: `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/architecture-doctor-full-map.txt` and `target/gate-artifacts/full-software-map-1ffec4ab0/software-map.json`.
- [x] `RTSPLIT-P0-013` Add a short implementation note explaining that this branch family is a Linux behavior-preserving extraction. Evidence: Phase 0 note below states the active scope and non-scope.
- [x] `RTSPLIT-P0-014` Hard prerequisite: `architecture-doctor --full-map` MVP exists and implements `FULLMAP-CHECK-01`, `FULLMAP-CHECK-02`, `FULLMAP-CHECK-05`, `FULLMAP-CHECK-06`, and `FULLMAP-CHECK-07`, or this checklist records an explicit owner-approved waiver before code movement. Evidence: `architecture-doctor-full-map.txt` has PASS for `FULLMAP-CHECK-01`, `FULLMAP-CHECK-02`, `FULLMAP-CHECK-05`, `FULLMAP-CHECK-06`, and `FULLMAP-CHECK-07`.
- [x] `RTSPLIT-P0-015` Confirm the HIR mutation hardening and parser recovery boards are tracked separately so this behavior-preserving split is not misreported as "0 silent bugs". Evidence: `full-architecture-refactor-program-checklist.md` keeps HIR mutation, parser recovery, HIR zero-silent-bug, runtime VM mutation, and unsafe/concurrency boards separate from this runtime-core split.

Phase 0 implementation note, 2026-04-30:

- Active scope: Linux behavior-preserving extraction of portable runtime execution into a future `trust-runtime-core` boundary.
- Not active scope: STM32H7, Arduino Opta, ESP32, embedded T0, embedded EtherCAT, `no_std` product support, MCU protocol commitments, or marketing/support claims for embedded runtime support.
- No runtime production code has moved in Phase 0; the only code changes in this slice are doctor/checker fixes needed to make the baseline gates truthful.

### Phase 0 Exit Gate

- [x] `RTSPLIT-P0-GATE-01` Baseline evidence exists.
- [x] `RTSPLIT-P0-GATE-02` Active/non-active scope is unambiguous.
- [x] `RTSPLIT-P0-GATE-03` No code movement has started.
- [x] `RTSPLIT-P0-GATE-04` Full-map doctor MVP prerequisite is met or waiver is recorded.

## Phase 1 - Behavior-Lock Test Matrix

These tests must exist before moving production code.

### Bytecode / VM Equivalence

- [x] `RTSPLIT-P1-VM-001` Add a fixture with a pre-split `program.stbc` artifact or an equivalent stable bytecode fixture. Evidence: `crates/trust-runtime/tests/fixtures/runtime_core_behavior_lock/program.st` is the checked-in stable source fixture used to build the bytecode fixture through the pre-move runtime core path.
- [x] `RTSPLIT-P1-VM-002` Add a test proving the fixture loads after the split path is introduced. Evidence: `stable_bytecode_fixture_loads_on_runtime_core_path` builds fixture bytecode, loads it through `Runtime::apply_bytecode_bytes`, selects `ExecutionBackend::BytecodeVm`, cold-restarts, and asserts no fault.
- [x] `RTSPLIT-P1-VM-003` Add a deterministic VM execution test with fixed inputs and fixed initial memory. Evidence: `vm_fixture_execution_image_status_and_values_are_stable` creates two cold-start runtimes from the same fixture bytes, writes fixed `%IX0.0` input, and executes one VM cycle in each.
- [x] `RTSPLIT-P1-VM-004` Assert bit-identical output image before/after the execution slice moves. Evidence: the same test asserts both runtimes produce identical raw output images and pins the current image to `[0x01, 0x00, 0x34, 0x12]`.
- [x] `RTSPLIT-P1-VM-005` Assert identical runtime status/fault result before/after the execution slice moves. Evidence: the same test asserts cycle counter `1`, `faulted() == false`, and matching `last_fault()` for both fixture runtimes.
- [x] `RTSPLIT-P1-VM-006` Assert identical value identity/equality behavior for enums, structs, arrays, references, FB instances, and retained values used by the fixture. Evidence: the same test asserts `Phase#RUNNING`, `Payload` fields, `ARRAY[0..2]` elements, a live `REF_TO` value, `Bump` FB instance output, and warm/cold restart behavior for `retained_count`.

### Cycle Boundary Semantics

- [x] `RTSPLIT-P1-CYCLE-001` Add a test proving all inputs are latched before user logic runs. Evidence: `cycle_boundary_latches_inputs_once_and_commits_outputs_after_ready_programs` uses a `BoundaryDriver` that writes `%IX0.0` during `read_inputs`; both ready programs observe the latched input.
- [x] `RTSPLIT-P1-CYCLE-002` Add a test proving outputs are committed only after all ready task/program execution completes. Evidence: the same test asserts one driver write after both ready programs run and pins the final raw output image to `[0x03]`.
- [x] `RTSPLIT-P1-CYCLE-003` Add a test proving no mid-cycle input refresh occurs during task execution. Evidence: the same driver would return `0x00` on a second read, but the cycle emits only `driver:read` then `driver:write`, and both outputs remain true.
- [x] `RTSPLIT-P1-CYCLE-004` Add a multi-driver test proving every driver follows the same pre-read/post-write boundary. Evidence: `cycle_boundary_reads_every_driver_before_any_driver_writes_outputs` asserts the exact boundary order `first:read`, `second:read`, `first:write`, `second:write` and matching `[0x03]` output snapshots for both drivers.
- [ ] `RTSPLIT-P1-CYCLE-005` Add a marker-memory `%M` boundary test if marker sync code is touched.

### Scheduler Semantics

- [x] `RTSPLIT-P1-SCHED-001` Add a test for periodic task interval ordering. Evidence: `crates/trust-runtime/tests/tasks.rs::periodic_interval` pins no execution before the 10 ms interval and execution exactly after the interval elapses.
- [x] `RTSPLIT-P1-SCHED-002` Add a test for equal-ready-time FIFO ordering. Evidence: `crates/trust-runtime/tests/tasks.rs::fifo_order_by_due_time_within_priority` pins insertion/FIFO behavior when an event task and periodic task are ready at the same priority/due point.
- [x] `RTSPLIT-P1-SCHED-003` Add a test for priority ordering when multiple tasks become ready in the same cycle. Evidence: `crates/trust-runtime/tests/tasks.rs::priority_order` pins lower numeric priority execution before a second ready task observes the first task's write.
- [x] `RTSPLIT-P1-SCHED-004` Add a test proving overrun accounting does not reorder later execution. Evidence: `crates/trust-runtime/tests/tasks.rs::task_overrun_drops_missed_intervals` pins one execution after a 35 ms jump and records two missed intervals without replaying/reordering extra executions.
- [x] `RTSPLIT-P1-SCHED-005` Add a test for event task edge handling if event scheduling code moves. Evidence: `crates/trust-runtime/tests/tasks.rs::event_single_rise` and `event_edge_coalescing_between_samples` pin rising-edge execution, no repeated high-level execution, re-arm after low, and sample-level coalescing.

### Retain / Restart Semantics

- [x] `RTSPLIT-P1-RETAIN-001` Add a cold-start test proving non-retain state resets. Evidence: `crates/trust-runtime/tests/vars_retain.rs::iec_6_5_6` pins cold restart restoring retain, non-retain, and ordinary variables to declaration defaults.
- [x] `RTSPLIT-P1-RETAIN-002` Add a warm-start test proving retain-backed values are restored before user logic runs. Evidence: `crates/trust-runtime/tests/vars_retain.rs::iec_6_5_6` pins warm restart retaining `VAR RETAIN` state while resetting `VAR NON_RETAIN` and ordinary state.
- [x] `RTSPLIT-P1-RETAIN-003` Add a retain canonicalization test for struct/array/enum values. Evidence: `crates/trust-runtime/tests/retain_store.rs::retain_store_roundtrip` now round-trips scalar, array, struct, and enum values through `FileRetainStore`.
- [ ] `RTSPLIT-P1-RETAIN-004` Add a corrupted/invalid retain snapshot test if retain validation code moves.
- [x] `RTSPLIT-P1-RETAIN-005` Add a test proving retained state priority over defaults remains unchanged. Evidence: `crates/trust-runtime/tests/struct_initializers.rs::retained_struct_value_wins_over_defaults_on_warm_restart` pins warm restart preserving retained struct state over type/declaration defaults and cold restart restoring the default.

### Watchdog / Fault Semantics

- [x] `RTSPLIT-P1-WDOG-001` Add watchdog timeout test. Evidence: `crates/trust-runtime/tests/runtime_reliability.rs::watchdog_faults_resource_on_overrun` pins a real runner watchdog timeout faulting the resource with `RuntimeError::WatchdogTimeout`; `crates/trust-runtime/tests/runtime_core_behavior_lock.rs::watchdog_timeout_preserves_fault_snapshot_and_safe_state_contract` pins direct runtime timeout handling.
- [x] `RTSPLIT-P1-WDOG-002` Add tests for every supported fault policy branch: halt, warn/degrade, restart, or explicit unsupported path. Evidence: `crates/trust-runtime/tests/runtime_core_behavior_lock.rs::watchdog_and_fault_policy_decisions_are_stable` pins `Halt`, `SafeHalt`, and `Restart` decisions for both watchdog actions and fault policies; the same test asserts current warn/degrade spellings are unsupported parse paths.
- [x] `RTSPLIT-P1-WDOG-003` Add a test proving watchdog-triggered faults preserve the expected runtime snapshot/error contract. Evidence: `crates/trust-runtime/tests/runtime_core_behavior_lock.rs::watchdog_timeout_preserves_fault_snapshot_and_safe_state_contract` asserts `WatchdogTimeout`, `last_fault`, `faulted()`, rejected follow-up cycle with `ResourceFaulted`, and safe-state output behavior for `Halt`, `SafeHalt`, and `Restart`.
- [ ] `RTSPLIT-P1-WDOG-004` Add test coverage for watchdog backend no-op/mock behavior if a trait is introduced. No watchdog backend trait exists in the current pre-move implementation, so this remains conditional until `RTSPLIT-P6-008` introduces one.

### Initializer / Value Invariants

- [x] `RTSPLIT-P1-INIT-001` Keep Issue #51 initializer runtime tests green. Evidence: GitHub issue #51 is "Struct aggregate initializers as VAR initial values fail to parse / typecheck"; `crates/trust-runtime/tests/struct_initializers.rs` pins struct aggregate initializers, type-level aggregate defaults, array-of-struct defaults, `VAR_GLOBAL`/direct-address aggregate initializers, `VAR_CONFIG` aggregate overrides, reference initializers, and FB initializer overrides.
- [x] `RTSPLIT-P1-INIT-002` Keep initializer service funnel doctor checks green. Evidence: `crates/trust-runtime/tests/initializer_architecture.rs::runtime_initializer_service_is_the_source_level_funnel`, `runtime_var_decl_parts_are_structural_not_positional_tuples`, `initializer_service_size_caps_hold`, and `syntax_classifier_helpers_delegate_to_central_api` pass.
- [x] `RTSPLIT-P1-INIT-003` Keep HIR/runtime initializer dependency-boundary tests green. Evidence: `crates/trust-runtime/tests/initializer_architecture.rs::hir_collection_and_import_do_not_drop_member_initializers`, `dependency_boundaries_for_initializer_metadata_hold`, and `runtime_pou_registration_is_hir_catalog_driven` pass.
- [x] `RTSPLIT-P1-INIT-004` Add value movement tests for `StructValue`, `ArrayValue`, enum identity, references, and FB instance IDs before moving value modules. Evidence: `crates/trust-runtime/tests/runtime_core_behavior_lock.rs::vm_fixture_execution_image_status_and_values_are_stable` pins a struct field value, array elements, `Phase#RUNNING` enum identity, live `REF_TO` value, FB instance ID/output, and retained value restart behavior; `crates/trust-runtime/tests/retain_store.rs::retain_store_roundtrip` also pins file-retain serialization for scalar, array, struct, and enum values.

Phase 1 focused test evidence, 2026-04-30:

- `cargo test -p trust-runtime --test tasks -- --nocapture`
- `cargo test -p trust-runtime --test runtime_core_behavior_lock --test retain_store -- --nocapture`
- `cargo test -p trust-runtime --test struct_initializers --test initializer_architecture --test runtime_core_behavior_lock -- --nocapture`
- `cargo test -p trust-runtime --test vars_retain --test retain_store -- --nocapture`
- `cargo test -p trust-runtime --test struct_initializers retained_struct_value_wins_over_defaults_on_warm_restart -- --nocapture`

### Phase 1 Exit Gate

- [x] `RTSPLIT-P1-GATE-01` Behavior-lock tests exist before code movement. Evidence: unconditional Phase 1 VM, cycle, scheduler, retain, watchdog/fault, and initializer/value rows are complete before production runtime code movement; conditional rows `RTSPLIT-P1-CYCLE-005`, `RTSPLIT-P1-RETAIN-004`, and `RTSPLIT-P1-WDOG-004` remain tied to future marker-sync, retain-validation, or watchdog-trait movement.
- [x] `RTSPLIT-P1-GATE-02` Behavior-lock tests fail on an intentionally broken local experiment or are otherwise proven meaningful. Evidence: the added behavior locks assert exact bytecode output image bytes, exact cycle driver order, exact value identities, exact retain round-trip values, exact watchdog safe-state/error outcomes, and exact initializer architecture source contracts rather than only checking that tests execute.
- [x] `RTSPLIT-P1-GATE-03` Behavior-lock tests pass on the pre-move implementation. Evidence: focused Phase 1 commands listed above pass on branch `architecture/runtime-behavior-locks` before any runtime production code movement.
- [x] `RTSPLIT-P1-GATE-04` Test commands are recorded in the checklist or linked evidence. Evidence: Phase 1 focused test evidence is recorded immediately above this gate.

## Phase 2 - Architecture Doctor Rules Before Extraction

- [x] `RTSPLIT-P2-001` Add a doctor rule that recognizes `trust-runtime-core` once introduced. Evidence: `xtask/src/full_map.rs::check_runtime_core_dependency_fence` switches from "crate not present" armed mode to package/import validation when cargo metadata or imports show `trust-runtime-core`; known-bad tests cover forbidden core dependency and host import cases.
- [x] `RTSPLIT-P2-002` Add a dependency fence for `trust-runtime-core`. Evidence: `FULLMAP-CHECK-05` is the dependency/import fence and passed in armed mode on `cargo run -p xtask -- architecture-doctor --full-map` at `62156e7e9`.
- [x] `RTSPLIT-P2-003` Add forbidden dependency checks for `tokio`, `zenoh`, `rumqttc`, `rustls`, `tiny_http`, `tungstenite`, `mdns-sd`, `notify`, `opcua`, `ethercrab`, `ureq`, `ratatui`, `crossterm`, `home`. Evidence: `xtask/config/full_map_policy.json::runtime_core_forbidden_dependencies` includes the full list, `SoftwareMap::direct_dependencies` now records all direct Cargo dependencies from metadata, and `repo_runtime_core_policy_covers_runtime_split_forbidden_sets` fails if any named dependency is removed.
- [x] `RTSPLIT-P2-004` Add forbidden workspace dependency checks from `trust-runtime-core` to `trust-ide`, `trust-lsp`, and `trust-debug`. Evidence: `runtime_core_forbidden_dependencies` includes `trust-ide`, `trust-lsp`, and `trust-debug`; `FULLMAP-CHECK-05` checks source-derived direct dependencies from `trust-runtime-core` against that set.
- [x] `RTSPLIT-P2-005` Add forbidden import checks for host modules: `web`, `hmi`, `control`, `runtime_cloud`, `mesh`, `discovery`, `io`, `opcua`, `debug`, `security`, `setup`, `simulation`, `ui`, `historian`. Evidence: `xtask/config/full_map_policy.json::runtime_core_forbidden_import_modules` includes the full list, `repo_runtime_core_policy_covers_runtime_split_forbidden_sets` locks the list, and `known_bad_runtime_core_forbidden_host_import_fails` proves the rule fails a core import of a host module.
- [x] `RTSPLIT-P2-006` Add a doctor rule requiring each new `trust-runtime` top-level module to have a subsystem decision note. Evidence: `FULLMAP-CHECK-10` validates every source-derived top-level `trust-runtime` module against `kiss.runtime_top_level_module_decisions`; `known_bad_runtime_top_level_module_without_decision_note_fails` locks the failure.
- [x] `RTSPLIT-P2-007` Add a doctor rule requiring runtime diagrams to reference generated/source-derived maps after ownership changes. Evidence: `FULLMAP-P7` checks selected PlantUML diagram aliases and crate dependency claims against source-derived map facts; known-bad unsupported alias and crate-edge tests fail.
- [x] `RTSPLIT-P2-008` Add a public API snapshot rule or documented fallback if the tool is unavailable. Evidence: `FULLMAP-P6-API` reports `cargo-public-api 0.51.0`, and Phase 0 recorded the `trust-runtime` public API snapshot artifact at `target/gate-artifacts/runtime-core-host-split-baseline-1ffec4ab0/public-api-trust-runtime-default-features.txt`.
- [x] `RTSPLIT-P2-009` Add a rule preventing web/HMI/cloud/control modules from bypassing approved runtime value/snapshot ports once those ports exist. Evidence: `FULLMAP-CHECK-07` has active approved-port drift checks and reports zero direct web runtime-state bypasses and zero direct web control-dispatch bypasses; host-surface owner path rules cover `web`, `hmi`, `ui`, `control`, and `runtime_cloud`.
- [x] `RTSPLIT-P2-010` Add these rules in warn/allowlist mode before the first move, then tighten after the relevant slice is complete. Evidence: before `trust-runtime-core` exists, `FULLMAP-CHECK-05` passes in armed mode with "crate not present"; once the crate or imports appear, the same check becomes a failing dependency/import fence. `FULLMAP-CHECK-10` reports the final host cap as inactive until the CLI/host-surface/runtime-core boards close.
- [x] `RTSPLIT-P2-011` Add KISS advisory checks for moved modules: file size, function size, public API growth, and top-level module growth. Evidence: `FULLMAP-CHECK-10` enforces large-file owner/split notes, top-level module decision notes, and a new function-size advisory that fails oversized `trust-runtime-core` functions; `FULLMAP-P6-API` reports public API snapshot tooling status.

### Phase 2 Exit Gate

- [x] `RTSPLIT-P2-GATE-01` Doctor rules exist before extraction. Evidence: Phase 2 rows `RTSPLIT-P2-001` through `RTSPLIT-P2-011` are backed by `architecture-doctor --full-map` before `crates/trust-runtime-core` exists.
- [x] `RTSPLIT-P2-GATE-02` Rules are either passing or explicitly allowlisted with removal dates. Evidence: `cargo run -p xtask -- architecture-doctor --full-map` passed at `62156e7e9`; reported findings are tracked by separate unsafe/concurrency policy, while dependency hygiene allowlists carry owner/rationale/review/removal metadata.
- [x] `RTSPLIT-P2-GATE-03` Known-bad local patterns are caught by at least one doctor rule. Evidence: `cargo test -p xtask full_map -- --nocapture` passed 44 tests, including known-bad runtime-core forbidden dependency/import, stale route handler, direct port bypass, missing top-level module decision, unsupported diagram alias/edge, and oversized runtime-core function fixtures.

## Phase 3 - Core Crate Scaffold

- [x] `RTSPLIT-P3-001` Add `crates/trust-runtime-core`. Evidence: `crates/trust-runtime-core/Cargo.toml`, `src/lib.rs`, and `src/scaffold.rs` exist.
- [x] `RTSPLIT-P3-002` Add it as a workspace member. Evidence: root `Cargo.toml` includes `crates/trust-runtime-core` in `workspace.members` and a workspace dependency entry.
- [x] `RTSPLIT-P3-003` Start with `std` if needed for low-risk extraction. Evidence: `crates/trust-runtime-core/Cargo.toml` defines default feature `std`.
- [x] `RTSPLIT-P3-004` Keep APIs shaped so `no_std + alloc` remains possible later, without claiming product support. Evidence: `src/lib.rs` uses `#![cfg_attr(not(feature = "std"), no_std)]`; `cargo check -p trust-runtime-core --no-default-features` passes; the scaffold has no host dependency and makes no embedded support claim.
- [x] `RTSPLIT-P3-005` Add crate docs describing core ownership and host exclusions. Evidence: `crates/trust-runtime-core/src/lib.rs` documents portable execution ownership and excludes web/HMI/control/cloud, Linux realtime setup, CLI wiring, harness compilation, and external I/O drivers.
- [x] `RTSPLIT-P3-006` Add `lib.rs` module map with only scaffold modules at first. Evidence: `src/lib.rs` only exposes `pub mod scaffold`.
- [x] `RTSPLIT-P3-007` Re-export core APIs from `trust-runtime` only as a compatibility bridge. Evidence: `crates/trust-runtime/src/lib.rs` re-exports `trust_runtime_core` as `runtime_core`; no runtime production code moved.
- [x] `RTSPLIT-P3-008` Add a compile check for `trust-runtime-core`. Evidence: `cargo check -p trust-runtime-core` passes.
- [x] `RTSPLIT-P3-009` Add a minimal unit test proving the crate participates in workspace tests. Evidence: `crates/trust-runtime-core/src/scaffold.rs::scaffold_stage_is_pre_move` passes under `cargo test -p trust-runtime-core -- --nocapture`.
- [x] `RTSPLIT-P3-010` Ensure initial scaffold does not pull forbidden dependencies. Evidence: `cargo run -p xtask -- architecture-doctor --full-map` passes with `FULLMAP-CHECK-05` reporting 18 forbidden dependencies and 17 forbidden import modules checked.

### Phase 3 Exit Gate

- [x] `RTSPLIT-P3-GATE-01` `cargo check -p trust-runtime-core` passes.
- [x] `RTSPLIT-P3-GATE-02` `cargo test -p trust-runtime-core` passes.
- [x] `RTSPLIT-P3-GATE-03` Doctor dependency fence passes for the empty/scaffold core. Evidence: `FULLMAP-CHECK-05` passes after the crate is present.
- [x] `RTSPLIT-P3-GATE-04` No public runtime behavior changed. Evidence: only a new scaffold crate and explicit compatibility re-export were added; no runtime execution modules or behavior paths moved.

## Phase 4 - Move Pure Data And Runtime Models

Move lowest-risk portable pieces first.

- [x] `RTSPLIT-P4-001` Move or re-home `numeric` only if it has no host-only imports.
- [x] `RTSPLIT-P4-002` Move portable `value` model pieces in small commits.
- [x] `RTSPLIT-P4-003` Keep value constructors and validation invariants unchanged.
- [x] `RTSPLIT-P4-004` Move portable `program_model` records used by runtime execution. Evidence: pure utility helpers, shared operator semantics, HIR-backed expression/lvalue/call-argument records, and HIR-backed initializer catalog records now live in `trust-runtime-core`; `stmt.rs` and `types.rs` remain in `trust-runtime` because they still carry host/debug/source-location and IO/retain policy dependencies that require a separate owner split.
- [ ] `RTSPLIT-P4-005` Move bytecode container/decode/format/validation pieces needed after compile. Progress: bytecode metadata/version/process-image records, bytecode error/reader/alignment helpers, and portable task configuration moved to `trust-runtime-core`; full `BytecodeModule` container/decode/validate movement remains open because the public `BytecodeModule::from_runtime*` encoder API is currently an inherent impl in `trust-runtime` and must be split before the type can move without a public API break.
- [ ] `RTSPLIT-P4-006` Keep compile/lowering harnesses in the Linux host unless separately justified.
- [ ] `RTSPLIT-P4-007` Keep web/control/debug formatting helpers host-side.
- [ ] `RTSPLIT-P4-008` Add tests for value serialization, equality, declared type identity, retained canonicalization, and bytecode validation after each moved cluster.
- [ ] `RTSPLIT-P4-009` Avoid moving a giant module wholesale if it mixes host and core responsibilities; split by owner first.

### Phase 4 Progress

- [x] `RTSPLIT-P4-DATETIME-001` Move standalone date/time value primitives into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/value/datetime.rs` now owns `Duration`, date/time tick wrappers, long date/time wrappers, and `combine_date_and_tod*`; `trust-runtime::value` re-exports the same API through `trust_runtime_core::value::datetime`.
- [x] `RTSPLIT-P4-DATETIME-002` Keep the first value-model move behavior-preserving and avoid a giant value-module move. Evidence: only the standalone date/time value primitives moved; `Value`, reference paths, partial access, defaults, sizing, memory IDs, and HIR-backed constructors remain in `trust-runtime` until their dependencies are split.
- [x] `RTSPLIT-P4-DATETIME-003` Add focused tests for the moved value cluster. Evidence: `cargo test -p trust-runtime-core -- --nocapture` passes 5 tests, including date/time tick round-trip, duration unit views, timezone rejection, and out-of-range conversion rejection.
- [x] `RTSPLIT-P4-DATETIME-004` Keep the moved value cluster compatible with the future `no_std` shape. Evidence: `cargo check -p trust-runtime-core --no-default-features` passes after the date/time move.
- [x] `RTSPLIT-P4-DATETIME-005` Verify host crate compatibility and dependency fences after the date/time move. Evidence: `cargo test -p trust-runtime value:: --lib -- --nocapture` passes 19 value tests; `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings` passes; `cargo run -p xtask -- architecture-doctor --full-map` passes `FULLMAP-CHECK-05` with 18 forbidden dependencies and 17 forbidden import modules.
- [x] `RTSPLIT-P4-MEMID-001` Move portable memory identity types into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/memory.rs` now owns `MemoryLocation`, `IoArea`, `FrameId`, and `InstanceId`; `trust-runtime::memory` re-exports those types so existing call sites keep the same source path.
- [x] `RTSPLIT-P4-MEMID-002` Verify memory/value compatibility after the identity move. Evidence: `cargo test -p trust-runtime-core -- --nocapture` passes 6 tests; `cargo test -p trust-runtime memory:: --lib -- --nocapture` passes 14 memory tests; `cargo check -p trust-runtime --lib`, `cargo check -p trust-runtime-core --no-default-features`, and `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings` pass.
- [x] `RTSPLIT-P4-MEMID-003` Keep the dependency/import fence green after the identity move. Evidence: `cargo run -p xtask -- architecture-doctor --full-map` passes `FULLMAP-CHECK-05` with 18 forbidden dependencies and 17 forbidden import modules.
- [x] `RTSPLIT-P4-DTCALC-001` Move portable date/time calculation helpers into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/datetime.rs` now owns `NANOS_PER_DAY`, `DateTimeCalcError`, `DivisionMode`, `days_from_civil`, `ticks_per_day`, `days_to_ticks`, and `nanos_to_ticks`; `trust-runtime` keeps a private compatibility module for existing internal `crate::datetime::*` imports.
- [x] `RTSPLIT-P4-DTCALC-002` Verify runtime compatibility and policy after the date/time calculation move. Evidence: `cargo test -p trust-runtime-core -- --nocapture` passes 8 tests; `cargo check -p trust-runtime --lib`, `cargo check -p trust-runtime-core --no-default-features`, and `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings` pass; `cargo run -p xtask -- architecture-doctor --full-map` passes after removing the stale `datetime` physical top-level module decision from the full-map policy.
- [x] `RTSPLIT-P4-ERROR-001` Move the shared runtime error type into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/error.rs` now owns `RuntimeError`; `trust-runtime::error` re-exports the core error API so existing external and internal paths keep compiling.
- [x] `RTSPLIT-P4-ERROR-002` Verify error compatibility after the move. Evidence: `cargo test -p trust-runtime --test errors_policy --test runtime_core_behavior_lock -- --nocapture` passes 7 tests; `cargo test -p trust-runtime-core -- --nocapture`, `cargo check -p trust-runtime --lib`, `cargo check -p trust-runtime-core --no-default-features`, and `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings` pass.
- [x] `RTSPLIT-P4-ERROR-003` Keep the dependency/import fence green after the error move. Evidence: `cargo run -p xtask -- architecture-doctor --full-map` passes after removing the stale `error` physical top-level module decision; `FULLMAP-CHECK-05` still reports 18 forbidden dependencies and 17 forbidden import modules with no findings.
- [x] `RTSPLIT-P4-REF-001` Move portable value reference identities and path helpers into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/value/reference.rs` now owns `RefSegment`, `RefIndices`, `RefPath`, `ValueRef`, `ref_indices_from_iter`, `single_ref_index`, `array_offset_i64`, and `checked_array_offset_i64`; `trust-runtime::value::reference` re-exports the moved API and keeps only the `Value`-dependent path walkers host-side.
- [x] `RTSPLIT-P4-REF-002` Move portable partial-access parsing into `trust-runtime-core` without moving the whole value model. Evidence: `PartialAccess`, `PartialAccessError`, and `parse_partial_access` now live in the core reference module, while string/array/struct mutation still stays in `trust-runtime` until `Value` field accessors and ownership boundaries are split cleanly.
- [x] `RTSPLIT-P4-REF-003` Verify the reference slice with focused tests and no-default compatibility. Evidence: `cargo test -p trust-runtime-core -- --nocapture` passes 12 tests, `cargo test -p trust-runtime value::reference --lib -- --nocapture` passes 3 focused runtime reference tests, `cargo check -p trust-runtime --lib` passes, `cargo check -p trust-runtime-core --no-default-features` passes, and `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings` passes.
- [x] `RTSPLIT-P4-REF-004` Keep the dependency/import fence green after the reference move. Evidence: `cargo run -p xtask -- architecture-doctor --full-map` passes; `FULLMAP-CHECK-05` still reports 18 forbidden dependencies and 17 forbidden import modules with no findings.
- [x] `RTSPLIT-P4-VALUEAPI-001` Split the compound-value accessor boundary before moving the full `Value` model. Evidence: `ArrayValue` now exposes `elements_mut` and validated `set_dimensions`; `StructValue` now exposes `field`, `field_mut`, `contains_field`, and `set_existing_field`; production helper-eval, harness coercion, VM dynamic-reference, and value-reference paths no longer mutate compound value private fields directly.
- [x] `RTSPLIT-P4-VALUEAPI-002` Add behavior locks for the new accessor/mutator boundary. Evidence: `cargo test -p trust-runtime value::types --lib -- --nocapture` passes 7 value-type tests, including `array_value_mutators_preserve_shape_contract` and `struct_value_mutator_updates_existing_fields_only`.
- [x] `RTSPLIT-P4-VALUEAPI-003` Verify touched runtime paths after removing private-field coupling. Evidence: `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime helper_eval::storage_lvalue --lib -- --nocapture`, `cargo test -p trust-runtime helper_eval::const_expr --lib -- --nocapture`, `cargo test -p trust-runtime runtime::vm::dispatch_refs --lib -- --nocapture`, and `cargo clippy -p trust-runtime --lib -- -D warnings` pass.
- [x] `RTSPLIT-P4-VALUEAPI-004` Keep the dependency/import fence green after the accessor split. Evidence: `cargo run -p xtask -- architecture-doctor --full-map` passes; `FULLMAP-CHECK-05` still reports 18 forbidden dependencies and 17 forbidden import modules with no findings.
- [x] `RTSPLIT-P4-VALUE-001` Move the portable runtime value model into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/value/types.rs` now owns `Value`, `ArrayValue`, `StructValue`, `EnumValue`, `ValueConstructionError`, and `normalize_assignment_for_target`; `trust-runtime::value` re-exports the core types through the existing compatibility path.
- [x] `RTSPLIT-P4-VALUE-002` Keep HIR-backed value construction explicit instead of making pure core require HIR. Evidence: `trust-runtime-core` defines optional feature `hir = ["std", "dep:trust-hir"]`; `trust-runtime` enables that feature for existing host-side constructors; `cargo check -p trust-runtime-core --no-default-features` passes and proves the no-default core shape does not require HIR.
- [x] `RTSPLIT-P4-VALUE-003` Replace remaining runtime direct-field coupling exposed by the move. Evidence: eval access/lvalue helpers, eval tests, helper-eval tests, memory tests, VM dynamic-reference tests, VM call/register tests, and array/struct fixtures now use `from_untyped_parts`, `from_canonical_parts`, `elements`, `elements_mut`, `dimensions`, `field`, `contains_field`, or `set_existing_field` instead of private compound-value fields.
- [x] `RTSPLIT-P4-VALUE-004` Verify the moved value model across core and host compatibility gates. Evidence: `cargo test -p trust-runtime-core -- --nocapture` passes 14 default-feature tests; `cargo test -p trust-runtime-core --features hir -- --nocapture` passes 19 tests including HIR-backed constructor tests; `cargo test -p trust-runtime value:: --lib -- --nocapture` passes 14 runtime value tests; `cargo test -p trust-runtime eval::tests::expr_access --lib -- --nocapture`, `cargo test -p trust-runtime eval::tests::pou_fb --lib -- --nocapture`, `cargo test -p trust-runtime runtime::vm::dispatch_refs --lib -- --nocapture`, and `cargo test -p trust-runtime memory:: --lib -- --nocapture` pass.
- [x] `RTSPLIT-P4-VALUE-005` Keep architecture automation green after the ownership move. Evidence: `cargo check -p trust-runtime --lib`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass; `FULLMAP-CHECK-02` classifies the new `trust-runtime-core -> trust-hir` edge as optional host-side HIR construction support and `FULLMAP-CHECK-05` still reports 18 forbidden dependencies and 17 forbidden import modules with no findings.
- [x] `RTSPLIT-P4-NUMERIC-001` Move portable numeric conversion helpers into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/numeric.rs` now owns `NumericKind`, numeric rank selection, integer/float extractors, and signed/unsigned constructors; `trust-runtime` keeps a private compatibility module re-exporting the core API for existing internal paths.
- [x] `RTSPLIT-P4-NUMERIC-002` Add focused numeric behavior locks in core. Evidence: `cargo test -p trust-runtime-core numeric -- --nocapture` passes 2 tests covering numeric-kind/rank behavior, signedness errors, and overflow preservation; `cargo test -p trust-runtime-core -- --nocapture` passes the 16-test core suite after the move.
- [x] `RTSPLIT-P4-NUMERIC-003` Verify host compatibility and architecture policy after the numeric move. Evidence: `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `cargo test -p xtask full_map -- --nocapture`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass; targeted runtime filters for `stdlib::conversions` and `program_model::ops` compile successfully but currently match 0 tests; the stale `numeric` runtime top-level module policy row was removed after the file moved.
- [x] `RTSPLIT-P4-PARTIAL-001` Move partial-access read/write semantics into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/value/partial_access.rs` now owns `read_partial_access` and `write_partial_access`; `trust-runtime::value` re-exports the core module so VM, eval, helper-eval, config init, and runtime accessor paths keep the same call surface.
- [x] `RTSPLIT-P4-PARTIAL-002` Add focused core behavior locks for partial access. Evidence: `cargo test -p trust-runtime-core partial_access -- --nocapture` passes 3 tests covering parser acceptance, read bounds/type behavior, and write bit/byte preservation; `cargo test -p trust-runtime-core -- --nocapture` passes the 18-test core suite after the move.
- [x] `RTSPLIT-P4-PARTIAL-003` Verify host compatibility and architecture policy after the partial-access move. Evidence: `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test types_bit_access -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass; `cargo test -p trust-runtime partial_access --lib -- --nocapture` compiles the runtime test binary but currently matches 0 tests.
- [x] `RTSPLIT-P4-STRING-001` Move portable string element and IEC string helper semantics into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/value/string_semantics.rs` now owns `string_element_count`, indexed string read/write, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`, and `FIND` helper semantics; `trust-runtime::value` imports the core module as the existing crate-internal compatibility surface for stdlib, eval, helper-eval, and value reference paths.
- [x] `RTSPLIT-P4-STRING-002` Keep string semantics compatible with the future `no_std + alloc` core shape. Evidence: the moved module uses `alloc` for `String`/`Vec` and `core::char::from_u32`; `cargo check -p trust-runtime-core --no-default-features` passes after the move.
- [x] `RTSPLIT-P4-STRING-003` Verify host compatibility and architecture policy after the string-semantics move. Evidence: `cargo test -p trust-runtime-core string_semantics -- --nocapture` passes 4 focused string tests; `cargo test -p trust-runtime-core -- --nocapture` passes the 22-test core suite after the move; `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime stdlib::string --lib -- --nocapture`, `cargo test -p trust-runtime value::reference --lib -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass; the `stdlib::string` filter compiles the runtime test binary but currently matches 0 unit tests.
- [x] `RTSPLIT-P4-SIZE-001` Move HIR-backed runtime value/type sizing into `trust-runtime-core` behind the optional HIR feature. Evidence: `crates/trust-runtime-core/src/value/size.rs` now owns `SizeOfError`, `size_of_type`, and `size_of_value`; `trust-runtime::value` re-exports the core sizing API through its existing compatibility surface while enabling `trust-runtime-core/hir`.
- [x] `RTSPLIT-P4-SIZE-002` Preserve the pure core boundary by keeping sizing feature-gated. Evidence: `crates/trust-runtime-core/src/value/mod.rs` gates the `size` module and re-export with `#[cfg(feature = "hir")]`; `cargo check -p trust-runtime-core --no-default-features` passes after the move.
- [x] `RTSPLIT-P4-SIZE-003` Verify host compatibility and architecture policy after the sizing move. Evidence: `cargo test -p trust-runtime-core --features hir size -- --nocapture` passes the focused sizing test; `cargo test -p trust-runtime-core --features hir -- --nocapture` passes the 28-test HIR-feature core suite; `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test sizeof_semantics -- --nocapture`, `cargo test -p trust-runtime --test bytecode_vm_core sizeof -- --nocapture`, `cargo test -p trust-runtime eval::tests::expr_full --lib -- --nocapture`, `cargo test -p trust-runtime helper_eval::const_expr --lib -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-DEFAULTS-001` Move HIR-backed runtime default-value construction into `trust-runtime-core` behind the optional HIR feature. Evidence: `crates/trust-runtime-core/src/value/defaults.rs` now owns `DefaultValueError` and `default_value_for_type_id`; `trust-runtime::value` re-exports the core defaults API through the existing compatibility surface while enabling `trust-runtime-core/hir`.
- [x] `RTSPLIT-P4-DEFAULTS-002` Preserve the pure core boundary by keeping default construction feature-gated. Evidence: `crates/trust-runtime-core/src/value/mod.rs` gates the `defaults` module and re-export with `#[cfg(feature = "hir")]`; `cargo check -p trust-runtime-core --no-default-features` passes after the move.
- [x] `RTSPLIT-P4-DEFAULTS-003` Verify host compatibility and architecture policy after the default-construction move. Evidence: `cargo test -p trust-runtime-core --features hir defaults -- --nocapture` passes 2 focused defaults tests; `cargo test -p trust-runtime-core --features hir -- --nocapture` passes the 30-test HIR-feature core suite; `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test value_defaults -- --nocapture`, `cargo test -p trust-runtime eval::tests::reference --lib -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-VALUE-GATE-001` Close the small-commit value-model movement row. Evidence: portable date/time values, memory identities, reference identities, partial-access parsing/read/write, value types, numeric helpers, string semantics, HIR-backed sizing, and HIR-backed default construction now live in `trust-runtime-core`; `trust-runtime/src/value` now keeps only the runtime compatibility module and `Value`-dependent reference walkers that still belong to the host path.
- [x] `RTSPLIT-P4-VALUE-GATE-002` Close the constructor/invariant preservation row. Evidence: focused core suites, `value_defaults`, `types_bit_access`, `sizeof_semantics`, bytecode `SIZEOF`, eval/reference, helper-eval, and runtime value/reference checks passed across the moved clusters; no public behavior-change row or changelog/version bump is required because this is behavior-preserving architecture movement.
- [x] `RTSPLIT-P4-PROGMODEL-UTIL-001` Move pure program-model static-name helpers into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/program_model/util.rs` now owns `static_storage_name`, `method_static_storage_owner`, and `property_setter_method_name`; `trust-runtime::program_model` re-exports the core helpers through the existing compatibility API.
- [x] `RTSPLIT-P4-PROGMODEL-UTIL-002` Verify host compatibility and architecture policy after the program-model utility move. Evidence: `cargo test -p trust-runtime-core program_model -- --nocapture`, `cargo test -p trust-runtime-core -- --nocapture`, `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test stdlib_split_locals function_local_initializer_runs_in_runtime_and_vm -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-PROGMODEL-OPS-001` Move shared program-model operator semantics into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/program_model/ops.rs` and `src/program_model/ops/*.rs` now own `UnaryOp`, `BinaryOp`, `apply_unary`, `apply_binary`, logical/bitwise comparison, time arithmetic/comparison, and numeric arithmetic; `trust-runtime::program_model::ops` remains as a compatibility re-export for VM, helper-eval, stdlib, bytecode, and eval tests.
- [x] `RTSPLIT-P4-PROGMODEL-OPS-002` Verify host compatibility and architecture policy after the operator move. Evidence: `cargo test -p trust-runtime-core program_model::ops -- --nocapture`, `cargo test -p trust-runtime-core -- --nocapture`, `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime eval::tests::expr_ops --lib -- --nocapture`, `cargo test -p trust-runtime eval::tests::expr_time_ops --lib -- --nocapture`, `cargo test -p trust-runtime eval::tests::errors --lib -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-PROGMODEL-EXPR-001` Move portable expression, lvalue, and call-argument records into `trust-runtime-core` behind the optional HIR feature. Evidence: `crates/trust-runtime-core/src/program_model/expr.rs` now owns `Expr`, `LValue`, `SizeOfTarget`, `ArgValue`, and `CallArg`; `trust-runtime::program_model::expr` and top-level `trust-runtime::program_model` exports remain compatibility re-exports.
- [x] `RTSPLIT-P4-PROGMODEL-EXPR-002` Verify host compatibility and architecture policy after the expression-record move. Evidence: `cargo test -p trust-runtime-core --features hir program_model::expr -- --nocapture`, `cargo test -p trust-runtime-core --features hir -- --nocapture`, `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime eval::tests::expr_access --lib -- --nocapture`, `cargo test -p trust-runtime helper_eval::storage_expr --lib -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-PROGMODEL-INIT-001` Move the HIR-backed runtime initializer catalog into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/program_model/initializers.rs` now owns `InitializerCatalog`, its lowered initializer records, type-default map, and ID allocation; `trust-runtime::program_model::initializers` remains as a compatibility re-export for harness, runtime, VM-local initialization, and tests.
- [x] `RTSPLIT-P4-PROGMODEL-INIT-002` Keep the initializer catalog in the optional HIR feature slice. Evidence: `trust-runtime-core/src/program_model/mod.rs` gates `initializers` with `#[cfg(feature = "hir")]`, `trust-runtime` continues enabling `trust-runtime-core/hir`, and the pure no-default core check remains green.
- [x] `RTSPLIT-P4-PROGMODEL-INIT-003` Verify host compatibility and architecture policy after the initializer-catalog move. Evidence: `cargo test -p trust-runtime-core --features hir initializers -- --nocapture`, `cargo test -p trust-runtime-core --features hir -- --nocapture`, `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test initializer_architecture -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-PROGMODEL-GATE-001` Close the portable program-model record movement row without moving host-bound records wholesale. Evidence: `trust-runtime-core::program_model` now owns utility helpers, operators, expressions/lvalues/call arguments, and initializer catalogs; `trust-runtime::program_model` keeps compatibility modules plus `stmt.rs` and `types.rs` because those files still depend on `debug::SourceLocation`, `io::IoAddress`, and `RetainPolicy`.
- [x] `RTSPLIT-P4-BYTECODE-META-001` Move portable task configuration into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/task.rs` now owns `TaskConfig`; `trust-runtime::task::TaskConfig` remains the public compatibility path, while `ProgramDef` and `TaskState` stay host-side because they still depend on host program statements and scheduler state.
- [x] `RTSPLIT-P4-BYTECODE-META-002` Move bytecode metadata records into `trust-runtime-core` before the full container move. Evidence: `crates/trust-runtime-core/src/bytecode/mod.rs` now owns `BytecodeVersion`, `SUPPORTED_MAJOR_VERSION`, `SUPPORTED_MINOR_VERSION`, `ProcessImageConfig`, `ResourceMetadata`, and `BytecodeMetadata`; `trust-runtime::bytecode` re-exports those names so existing metadata and process-image call sites keep their source paths.
- [x] `RTSPLIT-P4-BYTECODE-META-003` Preserve the full-container move blocker instead of silently changing public API. Evidence: `trust-runtime` still owns `BytecodeModule` and its `decode`/`encode`/`validate` methods because `BytecodeModule::from_runtime`, `from_runtime_with_sources`, and `from_runtime_with_sources_and_paths` are public inherent methods implemented by the host encoder; moving the type first would break those methods.
- [x] `RTSPLIT-P4-BYTECODE-META-004` Verify bytecode metadata compatibility and architecture policy after the metadata move. Evidence: `cargo test -p trust-runtime-core -- --nocapture`, `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test bytecode_metadata -- --nocapture`, `cargo test -p trust-runtime --test process_image -- --nocapture`, `cargo test -p trust-runtime --test bytecode_container -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.
- [x] `RTSPLIT-P4-BYTECODE-CORE-001` Move bytecode error, reader, and alignment helpers into `trust-runtime-core`. Evidence: `crates/trust-runtime-core/src/bytecode/mod.rs` now owns `BytecodeError`, `BytecodeReader`, `align4`, and `pad_to`; `trust-runtime::bytecode::{format, reader, util}` preserve the existing compatibility surfaces for host decode/encode/validation code.
- [x] `RTSPLIT-P4-BYTECODE-CORE-002` Keep the full `BytecodeModule` host-side until the encoder API split is done. Evidence: the `BytecodeModule::from_runtime*` inherent-method blocker remains, so this slice only moved helper and error surfaces that can move without changing the public runtime API.
- [x] `RTSPLIT-P4-BYTECODE-CORE-003` Verify bytecode helper compatibility after the helper move. Evidence: `just fmt`, `cargo test -p trust-runtime-core bytecode -- --nocapture`, `cargo check -p trust-runtime-core --no-default-features`, `cargo check -p trust-runtime --lib`, `cargo test -p trust-runtime --test bytecode_container --test bytecode_sections --test bytecode_validation -- --nocapture`, `cargo clippy -p trust-runtime-core -p trust-runtime --lib -- -D warnings`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, and `python scripts/check_diagram_drift.py` pass.

### Phase 4 Exit Gate

- [ ] `RTSPLIT-P4-GATE-01` Focused value/model tests pass.
- [ ] `RTSPLIT-P4-GATE-02` Runtime initializer/value tests pass.
- [ ] `RTSPLIT-P4-GATE-03` Doctor dependency/import fence passes.
- [ ] `RTSPLIT-P4-GATE-04` Public API snapshot differences are reviewed.

## Phase 5 - Move VM Execution Core

- [ ] `RTSPLIT-P5-001` Identify VM modules that are pure execution versus host assembly/lowering.
- [ ] `RTSPLIT-P5-002` Move VM dispatch/execution pieces that do not require host services.
- [ ] `RTSPLIT-P5-003` Keep host assembly, IO driver invocation, CLI project loading, and runtime config loading in `trust-runtime`.
- [ ] `RTSPLIT-P5-004` Introduce service ports only where the VM truly needs host callbacks.
- [ ] `RTSPLIT-P5-005` Do not let VM code import `web`, `control`, `debug`, `runtime_cloud`, or host IO implementations through the new core.
- [ ] `RTSPLIT-P5-006` Split `runtime/vm/call.rs` only along real boundaries: call dispatch, FB/class method semantics, and error mapping.
- [ ] `RTSPLIT-P5-007` Split register IR code only along real boundaries: lowering, profile, tier1, execution, test fixtures.
- [ ] `RTSPLIT-P5-008` Keep test names stable where possible to avoid losing historical signal.
- [ ] `RTSPLIT-P5-009` Run VM bytecode/core focused tests after every VM movement slice.

### Phase 5 Exit Gate

- [ ] `RTSPLIT-P5-GATE-01` Bytecode/VM behavior-lock tests pass.
- [ ] `RTSPLIT-P5-GATE-02` `cargo test -p trust-runtime --test bytecode_vm_core` or the renamed equivalent passes.
- [ ] `RTSPLIT-P5-GATE-03` Any surviving register IR parity/differential suite passes or the rename/removal is explicitly documented.
- [ ] `RTSPLIT-P5-GATE-04` Doctor rules show no host-only imports in moved VM code.

## Phase 6 - Move Scheduler, Cycle, Retain, And Watchdog Core

- [ ] `RTSPLIT-P6-001` Move scheduler model only after scheduler behavior locks pass.
- [ ] `RTSPLIT-P6-002` Move core cycle execution only after I/O boundary behavior locks pass.
- [ ] `RTSPLIT-P6-003` Keep actual IO driver implementations host-owned.
- [ ] `RTSPLIT-P6-004` Keep runtime process image exchange explicit and deterministic.
- [ ] `RTSPLIT-P6-005` Move retain snapshot policy/model into core.
- [ ] `RTSPLIT-P6-006` Keep actual retain persistence backend host-owned.
- [ ] `RTSPLIT-P6-007` Introduce a minimal sync retain storage trait if needed.
- [ ] `RTSPLIT-P6-008` Introduce a minimal sync watchdog trait if needed.
- [ ] `RTSPLIT-P6-009` Keep Linux PREEMPT_RT setup, `mlockall`, CPU affinity, scheduler policy, and systemd deployment host-side.
- [ ] `RTSPLIT-P6-010` Keep realtime/T0 host implementation host-side unless a separate T0 contract split is approved.

### Phase 6 Exit Gate

- [ ] `RTSPLIT-P6-GATE-01` Cycle boundary tests pass.
- [ ] `RTSPLIT-P6-GATE-02` Scheduler tests pass.
- [ ] `RTSPLIT-P6-GATE-03` Retain/restart tests pass.
- [ ] `RTSPLIT-P6-GATE-04` Watchdog/fault tests pass.
- [ ] `RTSPLIT-P6-GATE-05` Runtime vertical tests pass for touched surfaces.

## Phase 7 - Linux Host Rewire

- [ ] `RTSPLIT-P7-001` Rewire `trust-runtime` to consume `trust-runtime-core`.
- [ ] `RTSPLIT-P7-002` Keep public CLI behavior stable.
- [ ] `RTSPLIT-P7-003` Keep existing runtime config behavior stable.
- [ ] `RTSPLIT-P7-004` Keep product commands in the Linux host.
- [ ] `RTSPLIT-P7-005` Do not move workbench/dev command implementation in this phase unless the product/workbench branch is already complete.
- [ ] `RTSPLIT-P7-006` Ensure web/HMI/control/cloud access runtime state through approved ports.
- [ ] `RTSPLIT-P7-007` Ensure benchmarks and conformance harnesses run against the assembled Linux host, not a duplicate mini-runtime.
- [ ] `RTSPLIT-P7-008` Ensure debug/control surfaces do not reach into core internals beyond approved APIs.
- [ ] `RTSPLIT-P7-009` Preserve release artifacts and packaging behavior.

### Phase 7 Exit Gate

- [ ] `RTSPLIT-P7-GATE-01` Runtime product command smoke tests pass.
- [ ] `RTSPLIT-P7-GATE-02` Runtime vertical tests pass.
- [ ] `RTSPLIT-P7-GATE-03` Public API snapshot differences are reviewed.
- [ ] `RTSPLIT-P7-GATE-04` Doctor rules prevent host/core boundary regressions.

## Phase 8 - Product / Workbench Coordination

This phase coordinates with the separate runtime CLI product/workbench split. It does not have to move commands itself.

- [ ] `RTSPLIT-P8-001` Confirm subcommand ownership policy exists.
- [ ] `RTSPLIT-P8-002` Confirm product runtime commands do not import workbench-only modules.
- [ ] `RTSPLIT-P8-003` Confirm workbench/dev commands do not become the reason for core dependencies.
- [ ] `RTSPLIT-P8-004` Confirm `bundle_builder` ownership is decided before any core import is allowed.
- [ ] `RTSPLIT-P8-005` Confirm `agent`, `commit`, `git`, `docs`, `prompt`, `workflow`, and `style` remain outside core.
- [ ] `RTSPLIT-P8-006` Add compatibility-wrapper plan if commands move to `xtask` or `trust-dev`.

### Phase 8 Exit Gate

- [ ] `RTSPLIT-P8-GATE-01` Product/workbench boundary is compatible with the core/host split.
- [ ] `RTSPLIT-P8-GATE-02` No workbench command pulls host-only dependencies into core.

## Phase 9 - Maps, Diagrams, And Documentation

- [ ] `RTSPLIT-P9-001` Regenerate source-derived software map after each major ownership change.
- [ ] `RTSPLIT-P9-002` Update PlantUML sources for changed ownership/data/execution flow.
- [ ] `RTSPLIT-P9-003` Regenerate diagrams with `scripts/render_diagrams.sh`.
- [ ] `RTSPLIT-P9-004` Verify diagram drift with `python scripts/check_diagram_drift.py`.
- [ ] `RTSPLIT-P9-005` Add or update runtime-core/host split diagram.
- [ ] `RTSPLIT-P9-006` Update `docs/specs/11-runtime-engine.md` if the architectural contract changes.
- [ ] `RTSPLIT-P9-007` Update public/operator docs only if behavior or support claims change.
- [ ] `RTSPLIT-P9-008` Update `docs/internal/testing/checklists/architecture-improvements.md`.
- [ ] `RTSPLIT-P9-009` Record any unavoidable SOLID deviation in `docs/notes/runtime-refactor-notes.md` or the current equivalent.

### Phase 9 Exit Gate

- [ ] `RTSPLIT-P9-GATE-01` Diagrams are fresh.
- [ ] `RTSPLIT-P9-GATE-02` Diagram claims are source-derived or explicitly documented.
- [ ] `RTSPLIT-P9-GATE-03` Docs do not claim embedded support.

## Phase 10 - Validation Gates

### Focused Gates

- [ ] `RTSPLIT-P10-FOCUS-001` `cargo check -p trust-runtime-core`.
- [ ] `RTSPLIT-P10-FOCUS-002` `cargo test -p trust-runtime-core`.
- [ ] `RTSPLIT-P10-FOCUS-003` Focused VM/bytecode tests.
- [ ] `RTSPLIT-P10-FOCUS-004` Focused scheduler/cycle tests.
- [ ] `RTSPLIT-P10-FOCUS-005` Focused retain/watchdog tests.
- [ ] `RTSPLIT-P10-FOCUS-006` Focused initializer/value tests.
- [ ] `RTSPLIT-P10-FOCUS-007` `cargo run -p xtask -- architecture-doctor --all`.
- [ ] `RTSPLIT-P10-FOCUS-008` `cargo xtask architecture-doctor --full-map`.

### Runtime Vertical Gates

- [ ] `RTSPLIT-P10-RUNTIME-001` `cargo test -p trust-runtime --test api_smoke`.
- [ ] `RTSPLIT-P10-RUNTIME-002` `cargo test -p trust-runtime --test debug_control`.
- [ ] `RTSPLIT-P10-RUNTIME-003` `cargo test -p trust-runtime --test complete_program`.
- [ ] `RTSPLIT-P10-RUNTIME-004` `cargo test -p trust-runtime --test runtime_reliability`.
- [ ] `RTSPLIT-P10-RUNTIME-005` `cargo test -p trust-runtime --test realtime_t0_integration` if realtime/T0 ownership is touched.
- [ ] `RTSPLIT-P10-RUNTIME-006` Run relevant runtime benchmark smoke if startup/runtime assembly code moves.

### Workspace Gates

- [ ] `RTSPLIT-P10-WS-001` `just fmt`.
- [ ] `RTSPLIT-P10-WS-002` `just clippy`.
- [ ] `RTSPLIT-P10-WS-003` `just test`.
- [ ] `RTSPLIT-P10-WS-004` `just test-all` before declaring the split complete.

### Optional Deep Gates

- [ ] `RTSPLIT-P10-DEEP-001` Run mutation tests for moved core semantics where feasible.
- [ ] `RTSPLIT-P10-DEEP-002` Run Miri on core tests where feasible.
- [ ] `RTSPLIT-P10-DEEP-003` Run sanitizer or Valgrind/rr only where the moved code uses unsafe, FFI, threads, or memory-sensitive paths.
- [ ] `RTSPLIT-P10-DEEP-004` Run benchmark sweeps only after behavior is stable enough that perf numbers mean something.

### Phase 10 Exit Gate

- [ ] `RTSPLIT-P10-GATE-01` Final focused gates pass.
- [ ] `RTSPLIT-P10-GATE-02` Final runtime vertical gates pass.
- [ ] `RTSPLIT-P10-GATE-03` Final workspace gates pass.
- [ ] `RTSPLIT-P10-GATE-04` Any skipped optional deep gate has a written blocker or rationale.

## Phase 11 - Release / Merge Readiness

- [ ] `RTSPLIT-P11-001` Confirm whether the split is release-notable.
- [ ] `RTSPLIT-P11-002` Update `CHANGELOG.md` if release-notable.
- [ ] `RTSPLIT-P11-003` Bump workspace version only if release policy requires it for this branch.
- [ ] `RTSPLIT-P11-004` Confirm no embedded support claim was added to public docs.
- [ ] `RTSPLIT-P11-005` Confirm no user-visible CLI behavior changed unless separately documented.
- [ ] `RTSPLIT-P11-006` Confirm all generated files are intentionally included.
- [ ] `RTSPLIT-P11-007` Confirm branch diff contains no unrelated user changes.
- [ ] `RTSPLIT-P11-008` Prepare PR/merge summary with:
  - behavior-lock evidence,
  - doctor evidence,
  - runtime vertical evidence,
  - public API snapshot summary,
  - generated map/diagram evidence,
  - explicit statement that embedded support remains deferred.

## Final Exit Criteria

- [ ] `RTSPLIT-EXIT-001` `trust-runtime-core` exists and owns portable execution concerns.
- [ ] `RTSPLIT-EXIT-002` `trust-runtime` remains the Linux host and does not duplicate core execution logic.
- [ ] `RTSPLIT-EXIT-003` Host-only dependency leakage into the core is blocked by automation.
- [ ] `RTSPLIT-EXIT-004` Runtime behavior locks pass after the split.
- [ ] `RTSPLIT-EXIT-005` Runtime vertical tests pass after the split.
- [ ] `RTSPLIT-EXIT-006` Full workspace validation passes before merge.
- [ ] `RTSPLIT-EXIT-007` Diagrams and source-derived maps reflect the new ownership.
- [ ] `RTSPLIT-EXIT-008` No embedded runtime support is claimed.
- [ ] `RTSPLIT-EXIT-009` Any remaining host/core compromise has a written follow-up with owner and reason.
- [ ] `RTSPLIT-EXIT-010` Final summary explicitly states this split does not by itself close F2, F3, F10, or F11.
