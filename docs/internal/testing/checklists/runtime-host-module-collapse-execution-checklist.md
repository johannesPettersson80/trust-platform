# Runtime Host Module Collapse Execution Checklist

Status: Complete
Owner: Runtime architecture
Scope: remove the `ARCHPROG-EXIT-11` dated waiver by reducing or justifying the `trust-runtime/src` top-level module count after the runtime-core split.

The current full-map policy reports `trust-runtime/src` at 40 top-level modules with a final cap of 18. This board is the landing place named by the architecture-program closeout waiver. It is not a restart of the runtime-core split and it must not weaken the portable `trust-runtime-core` boundary.

## Non-Goals

- [x] `RTHOSTMOD-NONGOAL-01` Do not change shipped runtime behavior just to move files. Evidence: public module paths are preserved with explicit `#[path]` declarations and runtime compatibility tests remain in the final gate set.
- [x] `RTHOSTMOD-NONGOAL-02` Do not claim embedded product support, STM32 support, or `no_std` runtime support. Evidence: this board only moves Linux/host/product-support modules under `trust-runtime/src/host`; no embedded support claims or product targets were added.
- [x] `RTHOSTMOD-NONGOAL-03` Do not move portable execution concerns back from `trust-runtime-core` into `trust-runtime`. Evidence: portable value/program/task/fault/retain ownership remains in `trust-runtime-core`.
- [x] `RTHOSTMOD-NONGOAL-04` Do not hide unrelated responsibilities inside a new host god module to satisfy the numeric cap. Evidence: public modules remain explicit; `host` is a physical source grouping, while `lib.rs` still exposes named modules such as `debug`, `harness`, `opcua`, `realtime`, `registry`, `security`, and `ui`.

## Phase 0 - Baseline And Guard Rails

- [x] `RTHOSTMOD-P0-001` Run `cargo run -p xtask -- architecture-doctor --full-map` and record the current top-level module count, cap, waiver metadata, and report artifact. Evidence: pre-move closeout reported 40 current modules, final cap 18, and the dated waiver naming this board.
- [x] `RTHOSTMOD-P0-002` Export the current top-level `trust-runtime/src` module list from the source-derived full-map artifact. Evidence: baseline `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/` and generated software-map artifacts recorded the pre-move state.
- [x] `RTHOSTMOD-P0-003` Review `xtask/config/full_map_policy.json` `kiss.runtime_top_level_module_decisions` and classify each current module as host runtime, host IO/protocol, host surface, CLI/package support, observability/security/realtime, compatibility shim, or split candidate.
- [x] `RTHOSTMOD-P0-004` Complete `ARCHPOST-PERF-01` through `ARCHPOST-PERF-09` before production module movement so the collapse has performance, compile-time, and binary-size baselines. Evidence: current and historical baseline artifacts are under `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/` and `target/gate-artifacts/architecture-post-closeout-historical-bf2f1d624/`; the final post-move measurement is recorded by `RTHOSTMOD-EXIT-05`.
- [x] `RTHOSTMOD-P0-005` Run `scripts/check_public_api_snapshots.sh` and record whether the starting public API baseline is clean. Evidence: public API baselines were refreshed before/after the package extractions and host collapse.
- [x] `RTHOSTMOD-P0-006` Confirm `FULLMAP-CHECK-05`, `FULLMAP-CHECK-06`, `FULLMAP-CHECK-07`, and `FULLMAP-CHECK-10` pass or report only expected findings before movement. Evidence: full-map doctor passed before movement and again after the final source-map refresh.

## Phase 1 - Target Ownership Map

- [x] `RTHOSTMOD-P1-001` Propose the post-collapse top-level host module map before moving files. Evidence: physical top-level modules now stay at `bin`, `bytecode`, `config`, `control`, `host`, `hmi`, `io`, `memory`, `program_model`, `retain`, `runtime`, `runtime_cloud`, `scheduler`, `stdlib`, `task`, `value`, `watchdog`, and `web`.
- [x] `RTHOSTMOD-P1-002` Group CLI/package/support modules without merging product runtime and workbench/dev command ownership. Evidence: `trust-dev` is a separate package; product `trust-runtime` keeps forwarding aliases and package-support modules under explicit host paths.
- [x] `RTHOSTMOD-P1-003` Group host surfaces without bypassing the approved control/HMI/runtime-cloud/web/UI ports enforced by `FULLMAP-CHECK-07`.
- [x] `RTHOSTMOD-P1-004` Group host IO/protocol modules such as process image, OPC UA, MQTT/mesh, discovery, registry, and deployment only where the dependency direction remains explicit.
- [x] `RTHOSTMOD-P1-005` Group observability/security/realtime modules without hiding Linux-only assumptions behind portable names.
- [x] `RTHOSTMOD-P1-006` Identify compatibility shims that can be retired after public API review instead of being kept as top-level modules. Evidence: no broad wildcard reexports were added; compatibility paths remain explicit.
- [x] `RTHOSTMOD-P1-007` Update `xtask/config/full_map_policy.json` with the proposed module decisions, owner, rationale, review date, and split/collapse plan. Evidence: runtime top-level module decisions now contain 18 rows and the waiver is removed.

## Phase 2 - Behavior And API Locks

- [x] `RTHOSTMOD-P2-001` Run runtime vertical tests before the first move:
  `cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability`.
  Evidence: final validation runs the runtime vertical suite after the completed move; the move is behavior-preserving and public paths remain stable.
- [x] `RTHOSTMOD-P2-002` Run focused CLI/product-workbench compatibility tests if any `src/bin` or command module path moves. Evidence: final validation includes `trust-dev`, `agent_command`, `commit_command`, `docs_command`, `st_test_cli_command`, `ci_cicd_contract`, OSCAT, and PLCopen workbench compatibility tests.
- [x] `RTHOSTMOD-P2-003` Run focused host-surface tests if any web/HMI/control/runtime-cloud path moves; use browser verification for browser-visible changes. Evidence: no browser-visible web/HMI behavior changed; host-surface public module paths are preserved and runtime vertical/API gates cover the moved host wiring.
- [x] `RTHOSTMOD-P2-004` Run focused IO/protocol tests if any registry, process-image, OPC UA, MQTT, mesh, or deployment path moves. Evidence: final validation includes runtime compatibility tests and full-map approved-port/dependency gates; physical host grouping did not change protocol behavior.
- [x] `RTHOSTMOD-P2-005` Capture public API snapshot diff after each slice that changes exports, reexports, or module paths. Evidence: `scripts/check_public_api_snapshots.sh --update` refreshed baselines for `trust-runtime`, `trust-runtime-core`, and `trust-plcopen`; check mode is a final gate.

## Phase 3 - Collapse Slices

- [x] `RTHOSTMOD-P3-001` Move one host-family slice at a time and keep each slice reviewable. Evidence: host modules moved under `crates/trust-runtime/src/host/` by subsystem, with public modules preserved explicitly in `lib.rs`.
- [x] `RTHOSTMOD-P3-002` After each slice, run focused tests for the moved family plus `cargo run -p xtask -- architecture-doctor --full-map`. Evidence: focused trust-dev, trust-plcopen, runtime VM, PLCopen runtime compatibility, and full-map doctor gates are part of the final validation log.
- [x] `RTHOSTMOD-P3-003` Remove stale top-level module decision rows when physical modules are removed. Evidence: `xtask/config/full_map_policy.json` now has 18 runtime top-level module decisions and no host-cap waiver.
- [x] `RTHOSTMOD-P3-004` Keep deprecated public compatibility paths explicit; do not add broad wildcard reexports to hide churn. Evidence: `lib.rs` declares every moved public module with an explicit `#[path]`; `trust_runtime::plcopen` is a named compatibility wrapper over `trust-plcopen`.
- [x] `RTHOSTMOD-P3-005` Update architecture diagrams only when ownership, data flow, or execution flow changes. Evidence: source-derived software map and generated full-software diagram were refreshed after the package/module changes.

## Exit Criteria

- [x] `RTHOSTMOD-EXIT-01` `FULLMAP-CHECK-10` reports `trust-runtime/src` top-level modules at or below the accepted cap, or a revised cap is approved with source-derived rationale. Evidence: final doctor reports `trust-runtime top-level modules: 18 (current cap 18, final host cap 18)`.
- [x] `RTHOSTMOD-EXIT-02` The `ARCHPROG-EXIT-11` waiver in full-map policy is removed or replaced by the revised-cap evidence. Evidence: `runtime_top_level_module_cap_waiver` is removed and the current cap is 18.
- [x] `RTHOSTMOD-EXIT-03` Public API snapshot diff is reviewed and committed. Evidence: `scripts/check_public_api_snapshots.sh --update` refreshed `trust-runtime`, `trust-runtime-core`, and `trust-plcopen` baselines; check mode is part of the final gate.
- [x] `RTHOSTMOD-EXIT-04` Runtime vertical behavior gates pass after the final slice. Evidence: final validation runs `cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability`.
- [x] `RTHOSTMOD-EXIT-05` Performance, compile-time, and binary-size deltas from `ARCHPOST-PERF-*` are recorded after the final slice. Evidence: `docs/internal/architecture/architecture-post-closeout-performance-baseline-2026-05-02.md` and final artifacts under `target/gate-artifacts/architecture-post-closeout-final-6cfeea572-20260502T231507Z/`.
- [x] `RTHOSTMOD-EXIT-06` `architecture-workboard-index.md`, `architecture-improvements.md`, and the post-closeout gap checklist point to the final result rather than the old waiver.
