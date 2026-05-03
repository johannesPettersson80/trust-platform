# Architecture Post-Closeout Gap Closure Checklist

Status: Complete
Owner: Architecture/runtime team
Scope: follow-up work after the 12-board architecture program closed. Do not reopen `full-architecture-refactor-program-checklist.md` for these items unless a closed architecture-program claim is proven false.

This board exists because the architecture program now has real source-derived gates, but it did not make every structural risk disappear. The first follow-up must measure what the closeout did not measure, then remove or replace the dated waivers and ambiguous ownership language.

## Verified Baseline - 2026-05-02

- [x] `ARCHPOST-BASE-01` Architecture program closeout commit is pushed: `7ed2d5ee28379a0a57fa723b20fc913ac6a97a8d`.
- [x] `ARCHPOST-BASE-02` Umbrella status is closed for required boards and exit criteria; recurring guard rules remain active.
- [x] `ARCHPOST-BASE-03` Performance, compile-time, binary-size, and memory-footprint deltas were not closed by the final architecture report. Existing benchmark surfaces exist, but no post-refactor comparison table was recorded in `full-architecture-refactor-final-report-2026-05-02.md`.
- [x] `ARCHPOST-BASE-04` `trust-dev` is a separate binary implementation tree under `crates/trust-runtime/src/bin/trust-dev/`, not a separate Cargo package at `crates/trust-dev/`.
- [x] `ARCHPOST-BASE-05` `ARCHPROG-EXIT-11` closed through a dated waiver: the full-map doctor reports current `trust-runtime/src` top-level module count `40` against final cap `18`.
- [x] `ARCHPOST-BASE-06` BOARD-08 did not eliminate every file over 1,000 lines. It completed the measured runtime hotspot set and `FULLMAP-CHECK-10` now requires owner/split metadata for remaining runtime large files. The moved `crates/trust-runtime-core/src/value/types.rs` file is also over 1,000 lines and needs a separate core KISS decision.
- [x] `ARCHPOST-BASE-07` `crates/trust-runtime/src/plcopen/` remains a large host subsystem. It is policy-owned, but its freeze-versus-split decision is not settled by the architecture-program closeout.
- [x] `ARCHPOST-BASE-08` `FULLMAP-RUNTIMEVM-MUT` is only fresh when mutation artifacts exist locally; clean checkouts rely on the BOARD-10 checklist and CI run evidence unless the shards are regenerated.
- [x] `ARCHPOST-BASE-09` Raw `cargo audit` advisories remain policy-owned by `deny.toml`; the upgrade path is not complete.

## Phase 1 - Measurement Baselines

- [x] `ARCHPOST-PERF-01` Lock the benchmark method before changing runtime structure. Record machine, kernel, CPU governor if available, Rust toolchain, git commit, command, samples, warmup, and output artifact paths. Evidence: `docs/internal/architecture/architecture-post-closeout-performance-baseline-2026-05-02.md`.
- [x] `ARCHPOST-PERF-02` Capture current initializer baseline on `main`:
  `cargo run --release -p trust-runtime --bin trust-runtime -- bench init --project crates/trust-runtime/tests/fixtures/init_bench --samples 1000 --output json`.
  Evidence: `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/init.json` and final `target/gate-artifacts/architecture-post-closeout-final-6cfeea572-20260502T231507Z/init.json`.
- [x] `ARCHPOST-PERF-03` Capture current full-project cycle baseline on `main`:
  `cargo run --release -p trust-runtime --bin trust-runtime -- bench project --project examples/plcopen_motion_single_axis_demo --samples 2000 --warmup-cycles 200 --watch g_motion_demo_completed_sequences --output json`.
  Evidence: `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/project.json` and final `target/gate-artifacts/architecture-post-closeout-final-6cfeea572-20260502T231507Z/project.json`.
- [x] `ARCHPOST-PERF-04` Capture current tier-1 project baseline on `main`:
  `cargo run --release -p trust-runtime --bin trust-runtime -- bench project --project examples/plcopen_motion_single_axis_demo --samples 2000 --warmup-cycles 200 --watch g_motion_demo_completed_sequences --tier1 --output json`.
  Evidence: `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/project-tier1.json` and final `target/gate-artifacts/architecture-post-closeout-final-6cfeea572-20260502T231507Z/project-tier1.json`.
- [x] `ARCHPOST-PERF-05` Capture current runtime-cloud dispatch baseline on `main`:
  `cargo run --release -p trust-runtime --bin trust-runtime -- bench dispatch --samples 1000 --fanout 4 --output json`.
  Evidence: `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/dispatch.json` and final `target/gate-artifacts/architecture-post-closeout-final-6cfeea572-20260502T231507Z/dispatch.json`.
- [x] `ARCHPOST-PERF-06` Capture current compile-time baseline:
  `cargo build --release -p trust-runtime --timings`.
  Evidence: current baseline elapsed 202 s; final post-cleanup elapsed 105 s with timing report `target/cargo-timings/cargo-timing-20260502T232651880Z-9a7ca8332d2a4927.html`.
- [x] `ARCHPOST-PERF-07` Capture current binary/crate-size baseline with `cargo bloat --release -p trust-runtime --bin trust-runtime --crates -n 40`; if `cargo bloat` is unavailable, record the exact blocker and install/follow-up decision. Evidence: final `cargo-bloat-crates.txt` reports total `.text` 19.1 MiB and file size 41.9 MiB; `trust_runtime` attribution is 3.9 MiB after extracting PLCopen.
- [x] `ARCHPOST-PERF-08` Try a historical comparison against `bf2f1d6241f12f13d634226aaaff7dd52a80836a` in a separate clean worktree. If that commit cannot build or cannot run the same benchmark commands, record the reason and make `7ed2d5ee2` the forward regression baseline. Evidence: historical runtime benchmarks are under `target/gate-artifacts/architecture-post-closeout-historical-bf2f1d624/`; historical `cargo bloat` was attempted but stopped after a long native TLS/OpenSSL rebuild and is recorded as not comparable.
- [x] `ARCHPOST-PERF-09` Add a short post-closeout performance report under `docs/internal/architecture/` with the measured table, blockers, and future pass/fail budgets. Evidence: `docs/internal/architecture/architecture-post-closeout-performance-baseline-2026-05-02.md`.

## Phase 2 - Host Module Collapse

- [x] `ARCHPOST-HOST-01` Execute `runtime-host-module-collapse-execution-checklist.md` after Phase 1 baselines exist. Evidence: host-only modules moved under `crates/trust-runtime/src/host/`; public module paths are preserved through explicit `#[path]` declarations.
- [x] `ARCHPOST-HOST-02` Remove or replace the `ARCHPROG-EXIT-11` dated waiver only when `FULLMAP-CHECK-10` reports the host top-level count at or below the accepted cap, or when an owner-approved revised cap is justified by source-map evidence. Evidence: `xtask/config/full_map_policy.json` removes the waiver, sets the current cap to 18, and `FULLMAP-CHECK-10` reports `trust-runtime top-level modules: 18 (current cap 18, final host cap 18)`.
- [x] `ARCHPOST-HOST-03` Keep `trust-runtime-core` portable ownership stable during host collapse. Do not move portable execution back into the host crate to make the host module count look better. Evidence: host collapse only moved Linux/host/product-support modules inside `trust-runtime`; portable value/program/task/fault/retain ownership remains in `trust-runtime-core`.

## Phase 3 - Remaining Structural Gaps

- [x] `ARCHPOST-LARGE-01` Split or renew the remaining `kiss.large_file_allowlist` rows after BOARD-10 mutation evidence is durable enough to move the VM test/root files again. Evidence: `runtime/vm/call/tests.rs`, `runtime/vm/register_ir/tests/lowering.rs`, and `runtime/vm/register_ir/tests/tier1.rs` are now include roots with focused submodules under 1,000 lines.
- [x] `ARCHPOST-LARGE-02` Decide whether `FULLMAP-CHECK-10` should enforce runtime-core file-size owner/split metadata, then split or register `crates/trust-runtime-core/src/value/types.rs`. Evidence: `crates/trust-runtime-core/src/value/types.rs` moved its tests to `value/types/tests.rs`, adds focused value identity tests, and is now under 1,000 lines.
- [x] `ARCHPOST-DEV-01` Decide whether `trust-dev` should become a separate Cargo package. If not, keep checklist/report wording precise: it is a separate binary and implementation tree inside the `trust-runtime` package. Evidence: `trust-dev` is now a workspace package at `crates/trust-dev/`; release/CI workflows package/build it directly.
- [x] `ARCHPOST-PLCOPEN-01` Decide PLCopen status: either split `crates/trust-runtime/src/plcopen/` into smaller owned subsystems, or mark it as an inactive/frozen subsystem with an explicit dated review trigger. Evidence: PLCopen XML import/export moved to `crates/trust-plcopen/`; `trust_runtime::plcopen` remains a compatibility re-export, and future internal PLCopen splits are tracked as library follow-up if interchange work resumes.
- [x] `ARCHPOST-SAFE-01` Resolve the `cargo geiger` advisory-partial blocker or add another third-party unsafe cross-check. Keep the first-party full-map unsafe register as the enforced gate until the external tool is reliable. Evidence: unresolved external-tool work is carried by `architecture-external-safety-dependency-follow-up-checklist.md`; `FULLMAP-CHECK-09` remains the enforced gate.
- [x] `ARCHPOST-DEPS-01` Open dependency-upgrade work for raw audit advisory paths still covered by policy exceptions: OPC UA, tiny_http TLS, and Zenoh. Evidence: `architecture-external-safety-dependency-follow-up-checklist.md` owns OPC UA, tiny_http TLS, and Zenoh upgrade/removal work with exit criteria.
- [x] `ARCHPOST-CLAIMS-01` Keep final-report and checklist wording aligned with source facts. Do not say "zero large files", "separate trust-dev crate", "performance preserved", or "unsafe eliminated" unless a source-derived check proves that exact claim. Evidence: reports/checklists now distinguish measured performance, package boundaries, completed large-file splits, and remaining external safety/dependency risks.

## Exit Criteria

- [x] `ARCHPOST-EXIT-01` Post-closeout performance, compile-time, and binary-size baselines are recorded with exact commands and artifacts. Evidence: `docs/internal/architecture/architecture-post-closeout-performance-baseline-2026-05-02.md`.
- [x] `ARCHPOST-EXIT-02` Host module cap waiver is replaced by either a passing count or an approved revised cap with source-derived rationale. Evidence: waiver removed; `FULLMAP-CHECK-10` reports 18/18.
- [x] `ARCHPOST-EXIT-03` Remaining large-file and PLCopen decisions are either executed or explicitly frozen with dated owner review. Evidence: oversized runtime/runtime-core files split; PLCopen extracted to `trust-plcopen`.
- [x] `ARCHPOST-EXIT-04` `trust-dev` ownership language is unambiguous at binary/package/crate level. Evidence: `crates/trust-dev` is a workspace package; `trust-runtime` keeps forwarding aliases.
- [x] `ARCHPOST-EXIT-05` External unsafe/dependency cross-check gaps are resolved or carried by dedicated follow-up boards, not buried in the closed architecture umbrella. Evidence: `architecture-external-safety-dependency-follow-up-checklist.md`.
