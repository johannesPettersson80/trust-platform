# Runtime Host Module Collapse Execution Checklist

Status: Planned
Owner: Runtime architecture
Scope: remove the `ARCHPROG-EXIT-11` dated waiver by reducing or justifying the `trust-runtime/src` top-level module count after the runtime-core split.

The current full-map policy reports `trust-runtime/src` at 40 top-level modules with a final cap of 18. This board is the landing place named by the architecture-program closeout waiver. It is not a restart of the runtime-core split and it must not weaken the portable `trust-runtime-core` boundary.

## Non-Goals

- [ ] `RTHOSTMOD-NONGOAL-01` Do not change shipped runtime behavior just to move files.
- [ ] `RTHOSTMOD-NONGOAL-02` Do not claim embedded product support, STM32 support, or `no_std` runtime support.
- [ ] `RTHOSTMOD-NONGOAL-03` Do not move portable execution concerns back from `trust-runtime-core` into `trust-runtime`.
- [ ] `RTHOSTMOD-NONGOAL-04` Do not hide unrelated responsibilities inside a new host god module to satisfy the numeric cap.

## Phase 0 - Baseline And Guard Rails

- [ ] `RTHOSTMOD-P0-001` Run `cargo run -p xtask -- architecture-doctor --full-map` and record the current top-level module count, cap, waiver metadata, and report artifact.
- [ ] `RTHOSTMOD-P0-002` Export the current top-level `trust-runtime/src` module list from the source-derived full-map artifact.
- [ ] `RTHOSTMOD-P0-003` Review `xtask/config/full_map_policy.json` `kiss.runtime_top_level_module_decisions` and classify each current module as host runtime, host IO/protocol, host surface, CLI/package support, observability/security/realtime, compatibility shim, or split candidate.
- [ ] `RTHOSTMOD-P0-004` Complete `ARCHPOST-PERF-01` through `ARCHPOST-PERF-09` before production module movement so the collapse has performance, compile-time, and binary-size baselines.
- [ ] `RTHOSTMOD-P0-005` Run `scripts/check_public_api_snapshots.sh` and record whether the starting public API baseline is clean.
- [ ] `RTHOSTMOD-P0-006` Confirm `FULLMAP-CHECK-05`, `FULLMAP-CHECK-06`, `FULLMAP-CHECK-07`, and `FULLMAP-CHECK-10` pass or report only expected findings before movement.

## Phase 1 - Target Ownership Map

- [ ] `RTHOSTMOD-P1-001` Propose the post-collapse top-level host module map before moving files.
- [ ] `RTHOSTMOD-P1-002` Group CLI/package/support modules without merging product runtime and workbench/dev command ownership.
- [ ] `RTHOSTMOD-P1-003` Group host surfaces without bypassing the approved control/HMI/runtime-cloud/web/UI ports enforced by `FULLMAP-CHECK-07`.
- [ ] `RTHOSTMOD-P1-004` Group host IO/protocol modules such as process image, OPC UA, MQTT/mesh, discovery, registry, and deployment only where the dependency direction remains explicit.
- [ ] `RTHOSTMOD-P1-005` Group observability/security/realtime modules without hiding Linux-only assumptions behind portable names.
- [ ] `RTHOSTMOD-P1-006` Identify compatibility shims that can be retired after public API review instead of being kept as top-level modules.
- [ ] `RTHOSTMOD-P1-007` Update `xtask/config/full_map_policy.json` with the proposed module decisions, owner, rationale, review date, and split/collapse plan.

## Phase 2 - Behavior And API Locks

- [ ] `RTHOSTMOD-P2-001` Run runtime vertical tests before the first move:
  `cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability`.
- [ ] `RTHOSTMOD-P2-002` Run focused CLI/product-workbench compatibility tests if any `src/bin` or command module path moves.
- [ ] `RTHOSTMOD-P2-003` Run focused host-surface tests if any web/HMI/control/runtime-cloud path moves; use browser verification for browser-visible changes.
- [ ] `RTHOSTMOD-P2-004` Run focused IO/protocol tests if any registry, process-image, OPC UA, MQTT, mesh, or deployment path moves.
- [ ] `RTHOSTMOD-P2-005` Capture public API snapshot diff after each slice that changes exports, reexports, or module paths.

## Phase 3 - Collapse Slices

- [ ] `RTHOSTMOD-P3-001` Move one host-family slice at a time and keep each slice reviewable.
- [ ] `RTHOSTMOD-P3-002` After each slice, run focused tests for the moved family plus `cargo run -p xtask -- architecture-doctor --full-map`.
- [ ] `RTHOSTMOD-P3-003` Remove stale top-level module decision rows when physical modules are removed.
- [ ] `RTHOSTMOD-P3-004` Keep deprecated public compatibility paths explicit; do not add broad wildcard reexports to hide churn.
- [ ] `RTHOSTMOD-P3-005` Update architecture diagrams only when ownership, data flow, or execution flow changes.

## Exit Criteria

- [ ] `RTHOSTMOD-EXIT-01` `FULLMAP-CHECK-10` reports `trust-runtime/src` top-level modules at or below the accepted cap, or a revised cap is approved with source-derived rationale.
- [ ] `RTHOSTMOD-EXIT-02` The `ARCHPROG-EXIT-11` waiver in full-map policy is removed or replaced by the revised-cap evidence.
- [ ] `RTHOSTMOD-EXIT-03` Public API snapshot diff is reviewed and committed.
- [ ] `RTHOSTMOD-EXIT-04` Runtime vertical behavior gates pass after the final slice.
- [ ] `RTHOSTMOD-EXIT-05` Performance, compile-time, and binary-size deltas from `ARCHPOST-PERF-*` are recorded after the final slice.
- [ ] `RTHOSTMOD-EXIT-06` `architecture-workboard-index.md`, `architecture-improvements.md`, and the post-closeout gap checklist point to the final result rather than the old waiver.
