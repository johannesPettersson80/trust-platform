# Full Architecture Refactor Final Report

Status: closeout complete for the architecture-program boards and umbrella cleanup; post-closeout gaps are tracked separately.

Date: 2026-05-02
Owner: architecture/runtime/HIR team

## Fixed

- Full-map architecture doctor is the source-derived acceptance surface for workspace edges, runtime-core fences, command ownership, host-surface ports, dependency hygiene, unsafe/concurrency registers, KISS thresholds, public API baselines, parser recovery, HIR zero-silent-bug checks, VM mutation evidence, and diagram claims.
- Runtime command ownership is split between product runtime and workbench/dev command implementations, with deprecated forwarding aliases recorded. Post-closeout follow-up extracted the workbench/dev implementation into the separate `trust-dev` Cargo package while keeping the shipped `trust-dev` binary name and `trust-runtime` compatibility aliases.
- Runtime host surfaces are owned behind control/HMI/runtime-cloud/web/UI boundaries, and direct web runtime-state/control-dispatch bypasses are blocked.
- `trust-runtime-core` owns portable execution/value/bytecode/task/fault/retain pieces while Linux host, web/HMI/control/cloud, IO, realtime, debug, and product packaging remain in `trust-runtime`.
- Runtime large-file, module-size, function-size, public API, and top-level module growth gates are active through `FULLMAP-CHECK-10` and `FULLMAP-P6-API`; post-closeout follow-up removed the host-module cap waiver and enforces the final cap of 18.
- Runtime VM mutation hardening has selected shard evidence recorded by the BOARD-10 checklist and `FULLMAP-RUNTIMEVM-MUT`.
- Unsafe/concurrency hardening has source-derived unsafe, panic-like, and concurrency-boundary registers plus focused Miri, sanitizer, Valgrind, and deterministic poison-recovery evidence.
- Rust 1.95 modernization follow-up removed the unused direct `trust-runtime` dependency on `thiserror`; `time 0.3.47`, `ratatui 0.30`, and workspace Rust version `1.95` were already in place.

## Remaining Risks

- Performance, compile-time, binary-size, and memory-footprint deltas are now recorded in `architecture-post-closeout-performance-baseline-2026-05-02.md`. Median runtime paths, project throughput, compile time, binary size, and memory footprint are stable or improved; init/project p95 tail movement is documented as non-pinned `ondemand` CPU noise and should move to a dedicated performance board if strict tail budgets are required.
- `trust-runtime/src` now has 18 top-level modules against the final cap of 18. The dated `ARCHPROG-EXIT-11` waiver has been removed by the post-closeout host-module collapse.
- BOARD-08 completed the measured large-file hotspot set and installed `FULLMAP-CHECK-10` owner/split enforcement. Post-closeout follow-up split the remaining oversized runtime/runtime-core files, including `trust-runtime-core/src/value/types.rs`, register-IR root/test files, call tests, and the OSCAT example test corpus.
- PLCopen XML import/export implementation moves to `crates/trust-plcopen/` while `trust_runtime::plcopen::*` remains a compatibility re-export. The extracted crate is no longer a `trust-runtime/src` host subsystem, but it remains a large PLCopen library and needs future internal splits if interchange work resumes.
- Raw `cargo audit` still reports policy-owned transitive advisories through optional/current dependency stacks: OPC UA (`idna`, `derivative`), tiny_http TLS (`rustls 0.20`, `ring 0.16`, `rustls-pemfile`), and Zenoh (`rsa`, `paste`). The enforced repo gate is `cargo deny check` plus `cargo audit --ignore ...` using the documented policy exceptions.
- `cargo geiger` remains advisory-partial because version `0.13.0` does not handle the workspace virtual manifest cleanly here; the full-map unsafe scanner is the enforced first-party gate.
- Local `FULLMAP-RUNTIMEVM-MUT` is partial when `target/gate-artifacts/runtime-vm-mutants/**` has been cleaned. BOARD-10 records the completed shard evidence and CI run; local target artifacts must be regenerated before using that check as fresh local mutation proof.
- Broad Rust syntax/API modernization was intentionally not mixed into the behavior-preserving architecture branch. Candidates such as `array_windows`, `get_disjoint_mut`, `cfg_select!`, Rust 2024 edition, and let chains remain optional future cleanup unless they remove real complexity.

## Gate Evidence

| Claim | Gate |
| --- | --- |
| Source-derived architecture policy passes | `cargo run -p xtask -- architecture-doctor --full-map` |
| KISS module/function/top-level/public API gates are active | `FULLMAP-CHECK-10`, `FULLMAP-P6-API`, `scripts/check_public_api_snapshots.sh` |
| Diagram claims are source-checked | `FULLMAP-P7` plus `python scripts/check_diagram_drift.py` |
| Runtime vertical behavior remains locked | `cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability` |
| Performance deltas are measured post-closeout | `docs/internal/architecture/architecture-post-closeout-performance-baseline-2026-05-02.md` plus artifacts under `target/gate-artifacts/architecture-post-closeout-*` |
| Unsafe/concurrency focused evidence exists | BOARD-11 Miri, sanitizer, Valgrind, poison-recovery, and full-map unsafe/concurrency register gates |
| Dependency policy is enforced | `cargo deny check`, `cargo audit --ignore ...`, `cargo machete --with-metadata crates` |
| Rust 1.95 compatibility holds | `RUSTUP_TOOLCHAIN=1.95 cargo check --all-targets` |
| Workspace gates pass | `just fmt`, `just clippy`, `just test`, `just test-all` |
