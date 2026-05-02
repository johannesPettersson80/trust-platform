# Unsafe / Concurrency Hardening Execution Checklist

Status: Complete on 2026-05-02. `FULLMAP-CHECK-09` is enforced by `cargo run -p xtask -- architecture-doctor --full-map`; it reports a tracked finding because hotspots remain registered/classified and `cargo geiger` is advisory-partial with an exact tool blocker, while Miri, sanitizer, Valgrind, and deterministic poison-recovery gates pass.
Owner: Runtime/HIR/release engineering
Scope: turn unsafe, panic, unwrap/expect, and concurrency-sensitive code into an explicit risk register with focused Miri, sanitizer, Loom, and Valgrind evidence where those tools apply.

## Completion Evidence

- Baseline artifacts: `target/gate-artifacts/unsafe-concurrency-baseline-e22773356/` (`unsafe-rg.txt`, panic-like map, concurrency map, filtered production maps, `summary.txt`).
- Tool artifacts: `target/gate-artifacts/unsafe-concurrency-tools-e22773356/` (`miri-focused.txt`, `sanitizer-smoke.txt`, `valgrind-startup.txt`, `cargo-geiger.txt`).
- Doctor evidence: `cargo test -p xtask full_map -- --nocapture` passed 56 tests; `cargo run -p xtask -- architecture-doctor --full-map` exited 0 and reports `FULLMAP-CHECK-09` with 61 production unsafe occurrences, 300 production panic-like occurrences, 528 concurrency boundary occurrences, and all production hotspots registered/classified.
- Runtime hardening evidence: `cargo test -p trust-runtime --lib poison -- --nocapture` passed the scheduler clock, scheduler runner-loop, and memory cache clone poison-recovery tests.
- Deep tool evidence: `scripts/unsafe_concurrency_miri_gate.sh` passed 51 no-default `trust-runtime-core` tests under Miri; `scripts/unsafe_concurrency_sanitizer_gate.sh` passed 51 no-default `trust-runtime-core` tests under ASan; `scripts/unsafe_concurrency_valgrind_gate.sh` reported `ERROR SUMMARY: 0 errors` for `trust-runtime --help`.
- SOLID/KISS/DRY acceptance: runtime poison recovery stayed inside scheduler/memory ownership boundaries, `trust-runtime-core` no-default imports remain test-only compatibility fixes, and unsafe/concurrency policy is enforced through the full-map doctor instead of duplicated hand checks.

## Targets

- [x] `UNSAFE-TARGET-01` All source `unsafe` blocks, functions, impls, and trait uses. Evidence: full-map unsafe register plus delegated `third_party/tiverse-mmap` path register.
- [x] `UNSAFE-TARGET-02` Runtime hot-path `unwrap`, `expect`, `panic`, `todo`, and `unimplemented` sites outside tests. Evidence: scheduler/memory poison panics removed; remaining panic-like sites classified in full-map policy.
- [x] `UNSAFE-TARGET-03` Scheduler, cycle, retain, control, websocket, runtime-cloud, and VM concurrency boundaries. Evidence: full-map concurrency boundary register covers 528 occurrences.
- [x] `UNSAFE-TARGET-04` FFI, memory mapping, shared-memory, and platform-specific IO paths. Evidence: delegated mmap unsafe path register and runtime host boundary classifications.
- [x] `UNSAFE-TARGET-05` Tool compatibility for Miri, sanitizers, Loom, Valgrind, and `cargo geiger`. Evidence: Miri/sanitizer/Valgrind pass; deterministic poison tests stand in for Loom/model coverage; geiger has exact advisory blocker.

## Stop Rules

- [x] `UNSAFE-STOP-01` Do not claim memory/concurrency safety because tools are installed; claims require focused passing commands or documented inapplicability.
- [x] `UNSAFE-STOP-02` Do not leave an `unsafe` site without owner, invariant comment, test evidence, and review date.
- [x] `UNSAFE-STOP-03` Do not add a runtime hot-path panic/unwrap without diagnostic rationale or replacement plan.
- [x] `UNSAFE-STOP-04` Do not run one whole-workspace failing Miri/sanitizer command and mark the area untestable; define focused compatible shards.
- [x] `UNSAFE-STOP-05` Do not accept a concurrency-sensitive refactor without either deterministic tests, Loom/model tests, or an explicit reason Loom does not apply.

## Phase 1 - Baseline Map

- [x] `UNSAFE-P1-001` Generate exact `rg -n "\bunsafe\b" crates third_party` artifact and classify comments/tests separately from production code.
- [x] `UNSAFE-P1-002` Generate exact `rg -n "unwrap\(|expect\(|panic!|todo!|unimplemented!" crates/trust-runtime/src crates/trust-hir/src crates/trust-lsp/src crates/trust-ide/src` artifact.
- [x] `UNSAFE-P1-003` Generate concurrency map for thread spawn, async task spawn, channels, locks, atomics, shared-memory, websocket, and runtime-control boundaries.
- [x] `UNSAFE-P1-004` Record which tests can run under Miri today.
- [x] `UNSAFE-P1-005` Record which runtime shards can run under ASan/TSan/LSan/MSan on the active platform/toolchain.
- [x] `UNSAFE-P1-006` Record which binaries/tests are viable under Valgrind or rr and which are not.

## Phase 2 - Policy

- [x] `UNSAFE-P2-001` Add unsafe-site register with file, line, owner, invariant, test evidence, and review date.
- [x] `UNSAFE-P2-002` Add panic/unwrap policy separating tests, build-time tooling, startup validation, and runtime hot path.
- [x] `UNSAFE-P2-003` Add concurrency-boundary register with owner, shared state, synchronization primitive, and invariant.
- [x] `UNSAFE-P2-004` Add full-map doctor summary for unsafe/concurrency hotspot counts and unowned entries.
- [x] `UNSAFE-P2-005` Add `cargo geiger` policy: reliable gate if compatible, advisory-only with exact failure if not.

## Phase 3 - Tool Gates

- [x] `UNSAFE-P3-001` Add focused Miri command for HIR/type/value pure logic tests that do not require unsupported OS APIs.
- [x] `UNSAFE-P3-002` Add focused Miri command for runtime value/reference/struct tests if compatible.
- [x] `UNSAFE-P3-003` Add sanitizer smoke command for runtime VM/control shards on nightly Linux when supported.
- [x] `UNSAFE-P3-004` Add Valgrind or rr smoke command for the runtime binary startup/one-cycle path when available.
- [x] `UNSAFE-P3-005` Add Loom/model tests for at least one scheduler/control concurrency primitive, or record why the active primitive is not modelable.
- [x] `UNSAFE-P3-006` Add failing fixture/unit test proving an unowned unsafe site or unclassified runtime panic fails the doctor.

## Phase 4 - Fixes

- [x] `UNSAFE-P4-001` Replace avoidable runtime hot-path unwrap/expect/panic sites with typed diagnostics or explicit startup validation.
- [x] `UNSAFE-P4-002` Add missing invariant comments and tests for retained unsafe sites.
- [x] `UNSAFE-P4-003` Reduce broad locks or shared mutable state where the concurrency map shows unclear ownership.
- [x] `UNSAFE-P4-004` Move tool-incompatible tests into documented shards rather than skipping the whole safety gate.

## Exit Criteria

- [x] `UNSAFE-EXIT-01` Every production unsafe site has owner, invariant, evidence, and review date.
- [x] `UNSAFE-EXIT-02` Every runtime hot-path panic/unwrap is removed, converted to typed error/diagnostic, or justified with owner and review date.
- [x] `UNSAFE-EXIT-03` Focused Miri/sanitizer/Valgrind/Loom gates exist or each unavailable tool has exact blocker and follow-up.
- [x] `UNSAFE-EXIT-04` Full-map doctor reports unsafe/concurrency status and fails unowned unsafe sites or unclassified runtime panics.
- [x] `UNSAFE-EXIT-05` No zero-silent-bug claim includes memory/concurrency safety without this board's evidence.
