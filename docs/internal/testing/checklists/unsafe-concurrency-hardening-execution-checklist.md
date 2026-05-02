# Unsafe / Concurrency Hardening Execution Checklist

Status: Active; Phase 1 baseline map is next after BOARD-10 completion.
Owner: Runtime/HIR/release engineering
Scope: turn unsafe, panic, unwrap/expect, and concurrency-sensitive code into an explicit risk register with focused Miri, sanitizer, Loom, and Valgrind evidence where those tools apply.

## Targets

- [ ] `UNSAFE-TARGET-01` All source `unsafe` blocks, functions, impls, and trait uses.
- [ ] `UNSAFE-TARGET-02` Runtime hot-path `unwrap`, `expect`, `panic`, `todo`, and `unimplemented` sites outside tests.
- [ ] `UNSAFE-TARGET-03` Scheduler, cycle, retain, control, websocket, runtime-cloud, and VM concurrency boundaries.
- [ ] `UNSAFE-TARGET-04` FFI, memory mapping, shared-memory, and platform-specific IO paths.
- [ ] `UNSAFE-TARGET-05` Tool compatibility for Miri, sanitizers, Loom, Valgrind, and `cargo geiger`.

## Stop Rules

- [ ] `UNSAFE-STOP-01` Do not claim memory/concurrency safety because tools are installed; claims require focused passing commands or documented inapplicability.
- [ ] `UNSAFE-STOP-02` Do not leave an `unsafe` site without owner, invariant comment, test evidence, and review date.
- [ ] `UNSAFE-STOP-03` Do not add a runtime hot-path panic/unwrap without diagnostic rationale or replacement plan.
- [ ] `UNSAFE-STOP-04` Do not run one whole-workspace failing Miri/sanitizer command and mark the area untestable; define focused compatible shards.
- [ ] `UNSAFE-STOP-05` Do not accept a concurrency-sensitive refactor without either deterministic tests, Loom/model tests, or an explicit reason Loom does not apply.

## Phase 1 - Baseline Map

- [ ] `UNSAFE-P1-001` Generate exact `rg -n "\bunsafe\b" crates third_party` artifact and classify comments/tests separately from production code.
- [ ] `UNSAFE-P1-002` Generate exact `rg -n "unwrap\(|expect\(|panic!|todo!|unimplemented!" crates/trust-runtime/src crates/trust-hir/src crates/trust-lsp/src crates/trust-ide/src` artifact.
- [ ] `UNSAFE-P1-003` Generate concurrency map for thread spawn, async task spawn, channels, locks, atomics, shared-memory, websocket, and runtime-control boundaries.
- [ ] `UNSAFE-P1-004` Record which tests can run under Miri today.
- [ ] `UNSAFE-P1-005` Record which runtime shards can run under ASan/TSan/LSan/MSan on the active platform/toolchain.
- [ ] `UNSAFE-P1-006` Record which binaries/tests are viable under Valgrind or rr and which are not.

## Phase 2 - Policy

- [ ] `UNSAFE-P2-001` Add unsafe-site register with file, line, owner, invariant, test evidence, and review date.
- [ ] `UNSAFE-P2-002` Add panic/unwrap policy separating tests, build-time tooling, startup validation, and runtime hot path.
- [ ] `UNSAFE-P2-003` Add concurrency-boundary register with owner, shared state, synchronization primitive, and invariant.
- [ ] `UNSAFE-P2-004` Add full-map doctor summary for unsafe/concurrency hotspot counts and unowned entries.
- [ ] `UNSAFE-P2-005` Add `cargo geiger` policy: reliable gate if compatible, advisory-only with exact failure if not.

## Phase 3 - Tool Gates

- [ ] `UNSAFE-P3-001` Add focused Miri command for HIR/type/value pure logic tests that do not require unsupported OS APIs.
- [ ] `UNSAFE-P3-002` Add focused Miri command for runtime value/reference/struct tests if compatible.
- [ ] `UNSAFE-P3-003` Add sanitizer smoke command for runtime VM/control shards on nightly Linux when supported.
- [ ] `UNSAFE-P3-004` Add Valgrind or rr smoke command for the runtime binary startup/one-cycle path when available.
- [ ] `UNSAFE-P3-005` Add Loom/model tests for at least one scheduler/control concurrency primitive, or record why the active primitive is not modelable.
- [ ] `UNSAFE-P3-006` Add failing fixture/unit test proving an unowned unsafe site or unclassified runtime panic fails the doctor.

## Phase 4 - Fixes

- [ ] `UNSAFE-P4-001` Replace avoidable runtime hot-path unwrap/expect/panic sites with typed diagnostics or explicit startup validation.
- [ ] `UNSAFE-P4-002` Add missing invariant comments and tests for retained unsafe sites.
- [ ] `UNSAFE-P4-003` Reduce broad locks or shared mutable state where the concurrency map shows unclear ownership.
- [ ] `UNSAFE-P4-004` Move tool-incompatible tests into documented shards rather than skipping the whole safety gate.

## Exit Criteria

- [ ] `UNSAFE-EXIT-01` Every production unsafe site has owner, invariant, evidence, and review date.
- [ ] `UNSAFE-EXIT-02` Every runtime hot-path panic/unwrap is removed, converted to typed error/diagnostic, or justified with owner and review date.
- [ ] `UNSAFE-EXIT-03` Focused Miri/sanitizer/Valgrind/Loom gates exist or each unavailable tool has exact blocker and follow-up.
- [ ] `UNSAFE-EXIT-04` Full-map doctor reports unsafe/concurrency status and fails unowned unsafe sites or unclassified runtime panics.
- [ ] `UNSAFE-EXIT-05` No zero-silent-bug claim includes memory/concurrency safety without this board's evidence.
