# Runtime Large-File Split Execution Checklist

Status: In progress; Phase 0 and Phase 1 owner/split inventory complete, `RTLARGE-P2-001` through `RTLARGE-P2-004` complete; next open detail row is `RTLARGE-P3-004`
Owner: Runtime architecture
Scope: address audit F8 and KISS risks from very large runtime files.

## Quantitative Rules

- [x] `RTLARGE-RULE-01` No new Rust source file over 1,000 lines without owner/split note. Evidence: `FULLMAP-CHECK-10` fails runtime `src` and runtime `tests` Rust files over 1,000 lines without a `kiss.large_file_allowlist` owner/split entry; locked by `known_bad_large_runtime_file_without_owner_note_fails` and `known_bad_large_runtime_test_file_without_owner_note_fails`.
- [x] `RTLARGE-RULE-02` Every existing Rust source file over 1,000 lines must have owner, responsibility statement, and split plan or waiver. Evidence: current `FULLMAP-CHECK-10` pass lists all 7 remaining runtime hotspots over 1,000 lines with owner and split plan after `runtime/vm/call.rs`, `web/config_ui_routes.rs`, `runtime/vm/register_ir/tests.rs`, and `memory.rs` were split below the threshold.
- [x] `RTLARGE-RULE-03` Every Rust source file over 1,500 lines must have an approved split branch, completed split, or dated waiver. Evidence: current `FULLMAP-CHECK-10` pass lists all three remaining files over 1,500 lines with `runtime-large-file-split` split plans after `runtime/vm/call.rs`, `web/config_ui_routes.rs`, `runtime/vm/register_ir/tests.rs`, and `memory.rs` were split below the threshold.
- [ ] `RTLARGE-RULE-04` Files over 2,500 lines are release-blocking for unrelated growth until split or waiver.
- [ ] `RTLARGE-RULE-05` Public API growth caused by splits requires review, not automatic acceptance.

## Initial Hotspot Set

- [x] `RTLARGE-HOT-01` `crates/trust-runtime/src/runtime/vm/register_ir/tests.rs`. Completed 2026-05-01: split into a 6-line include root plus domain fragments for support, lowering, profiling, function-block/dynamic refs, Tier-1 execution, and diagnostics; all split files are under 1,000 lines and the stale `FULLMAP-CHECK-10` allowlist entry was removed.
- [x] `RTLARGE-HOT-02` `crates/trust-runtime/src/runtime/vm/call.rs`. Completed 2026-05-01: split into `call.rs` dispatch entry plus `call/bindings.rs`, `call/stdlib.rs`, `call/symbols.rs`, and `call/tests.rs`; all split files are under 1,000 lines and the stale `FULLMAP-CHECK-10` allowlist entry was removed.
- [x] `RTLARGE-HOT-03` `crates/trust-runtime/src/web/config_ui_routes.rs`. Completed 2026-05-01: split into a 67-line route entry point plus request models, response helpers, workspace persistence/services, runtime-cloud projection, live/lifecycle services, and focused route-group modules; all split files are under 1,000 lines and the stale `FULLMAP-CHECK-10` allowlist entry was removed.
- [ ] `RTLARGE-HOT-04` `crates/trust-runtime/tests/agent_command.rs`.
- [ ] `RTLARGE-HOT-05` `crates/trust-runtime/src/runtime/vm/register_ir.rs`.
- [ ] `RTLARGE-HOT-06` `crates/trust-runtime/tests/bytecode_vm_core.rs`.
- [ ] `RTLARGE-HOT-07` `crates/trust-runtime/src/bin/trust-dev/agent.rs` after BOARD-04 moved the implementation out of `trust-runtime`; `trust-runtime/agent.rs` is now a 13-line forwarding alias.
- [ ] `RTLARGE-HOT-08` `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09.rs`.
- [ ] `RTLARGE-HOT-09` `crates/trust-runtime/src/runtime/vm/register_ir/lower.rs`.
- [x] `RTLARGE-HOT-10` `crates/trust-runtime/src/memory.rs`. Completed 2026-05-01: split into a 94-line memory root plus access-map, frame-stack, instance-field/cache, reference read/write, storage/retain, and test modules; all split files are under 1,000 lines and the stale `FULLMAP-CHECK-10` allowlist entry was removed.
- [x] `RTLARGE-HOT-11` Retired: `crates/trust-runtime/src/value/types.rs` no longer exists in `v0.24.12`; stale `FULLMAP-CHECK-10` allowlist entry removed and stale allowlist detection added.
- [ ] `RTLARGE-HOT-12` `crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs`.

## Phase 0 - Full-Map Prerequisite

- [x] `RTLARGE-P0-001` Hard prerequisite before Phase 3 and before claiming `ARCHPROG-EXIT-07`: `architecture-doctor --full-map` MVP implements `FULLMAP-CHECK-10`. Evidence: `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed on 2026-05-01 and reported `PASS: FULLMAP-CHECK-10`.
- [x] `RTLARGE-P0-002` If `FULLMAP-CHECK-10` is unavailable, record an owner-approved waiver with local large-file scan command, threshold policy, owner, and expiration date. Evidence: not required; `FULLMAP-CHECK-10` is available and passing.
- [x] `RTLARGE-P0-GATE-01` Do not treat owner/split notes as sufficient unless the full-map doctor or waiver proves new regressions are blocked. Evidence: `FULLMAP-CHECK-10` now blocks missing owner/split notes for runtime `src` and runtime `tests`, blocks stale large-file allowlist entries, and passes with current policy.

## Phase 1 - Owner/Split Notes

- [x] `RTLARGE-P1-001` Generate current >1,000-line file list. Evidence command: `rg --files crates/trust-runtime/src crates/trust-runtime/tests -g '*.rs' | xargs wc -l | sort -nr`.
- [x] `RTLARGE-P1-002` Add owner/split note for each file. Evidence: `xtask/config/full_map_policy.json` `kiss.large_file_allowlist` covers all current files over 1,000 lines; `FULLMAP-CHECK-10` reports each owner/split plan.
- [x] `RTLARGE-P1-003` Classify each file as test-only, runtime-hot-path, route/controller, model/value, or CLI/workbench. Evidence: see Phase 1 inventory below.
- [x] `RTLARGE-P1-004` Identify behavior-lock tests needed before each split. Evidence: see Phase 1 inventory below.
- [x] `RTLARGE-P1-005` Prioritize files by risk, churn, and runtime criticality. Evidence: see Phase 1 inventory below.

### Phase 1 Inventory - 2026-05-01

This baseline records the start-of-board hotspot inventory. Completed Phase 2 splits can remove rows from the current `FULLMAP-CHECK-10` output; the hotspot rows above record those completions.

Files currently over 1,000 lines:

| Priority | File | Lines | Classification | Owner | Split / behavior-lock notes |
| --- | --- | ---: | --- | --- | --- |
| P1 | `crates/trust-runtime/src/runtime/vm/call.rs` | 2,809 | runtime-hot-path | runtime/VM | Split by dispatch, FB/class method semantics, and error mapping. Behavior locks: focused VM call tests plus `cargo test -p trust-runtime --test bytecode_vm_core`. |
| P2 | `crates/trust-runtime/src/web/config_ui_routes.rs` | 2,540 | route/controller | runtime/web | Split routes, persistence, request/response models, and domain services. Behavior locks: focused config UI/web route tests. |
| P3 | `crates/trust-runtime/src/runtime/vm/register_ir/tests.rs` | 3,202 | test-only runtime VM | runtime/VM | Split tests by VM feature/domain without renaming stable tests unnecessarily. Behavior locks: same test suite must pass before and after split. |
| P4 | `crates/trust-runtime/tests/bytecode_vm_core.rs` | 1,920 | test-only runtime VM | runtime/VM | Split by bytecode execution domain after preserving current test names where practical. Behavior lock: the split itself is the test suite. |
| P5 | `crates/trust-runtime/tests/agent_command.rs` | 1,566 | test-only CLI/workbench | dev tooling | Split agent command integration tests by server, config, and workflow behavior. Behavior locks: focused `agent_command` tests. |
| P6 | `crates/trust-runtime/src/runtime/vm/register_ir.rs` | 1,565 | runtime-hot-path | runtime/VM | Split interpreter loop helpers only after VM/register-IR behavior locks are focused. |
| P7 | `crates/trust-runtime/src/bin/trust-dev/agent.rs` | 1,371 | CLI/workbench | dev tooling | Split agent serve workflow from command parsing and output; keep `trust-runtime agent` forwarding alias behavior locked. |
| P8 | `crates/trust-runtime/src/runtime/vm/register_ir/lower.rs` | 1,303 | runtime-hot-path | runtime/VM | Split lowering by expression/statement/value areas after register-IR lowering tests are focused. |
| P9 | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09.rs` | 1,279 | test-only web IDE | runtime/web IDE | Split integration fixture tests by scenario while preserving browser/IDE behavior locks. |
| P10 | `crates/trust-runtime/src/memory.rs` | 1,186 | model/value | runtime/core | Split by layout, access, retain interaction, and tests if ownership analysis confirms mixed responsibilities. |
| P11 | `crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs` | 1,130 | runtime-hot-path | runtime/VM | Split Tier-1 compiled execution helpers after register-IR execution behavior locks are focused. |

Non-hotspot note: `crates/trust-runtime/tests/oscat_oop_examples.rs` is exactly 1,000 lines, so it is below the "over 1,000" rule and is not part of the current owner/split required set.

## Phase 2 - First High-Risk Splits

- [x] `RTLARGE-P2-001` Split `runtime/vm/call.rs` by dispatch, FB/class method semantics, and error mapping. Evidence: `wc -l crates/trust-runtime/src/runtime/vm/call.rs crates/trust-runtime/src/runtime/vm/call/*.rs` reports `call.rs` 465, `bindings.rs` 917, `stdlib.rs` 452, `symbols.rs` 70, `tests.rs` 957; `cargo test -p trust-runtime --lib runtime::vm::call::tests -- --nocapture` passed with 27 tests; `cargo test -p trust-runtime --test bytecode_vm_core call_native -- --nocapture` passed with 7 tests; `cargo test -p trust-runtime --lib function_block_call -- --nocapture` passed with 2 tests; `cargo clippy -p trust-runtime --lib -- -D warnings` passed; `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed with the stale `call.rs` allowlist row removed.
- [x] `RTLARGE-P2-002` Split `web/config_ui_routes.rs` by routing, persistence, request models, response models, and domain services. Evidence: `wc -l crates/trust-runtime/src/web/config_ui_routes.rs crates/trust-runtime/src/web/config_ui_routes/*.rs crates/trust-runtime/src/web/config_ui_routes/routes/*.rs` reports every split file under 1,000 lines; `cargo check -p trust-runtime --lib` passed; `cargo test -p trust-runtime --test web_io_config_integration config_ui -- --nocapture` passed with 6 tests; `cargo clippy -p trust-runtime --lib -- -D warnings` passed; `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed with the stale `config_ui_routes.rs` allowlist row removed.
- [x] `RTLARGE-P2-003` Split `runtime/vm/register_ir/tests.rs` by feature/domain without losing test names unnecessarily. Evidence: `wc -l crates/trust-runtime/src/runtime/vm/register_ir/tests.rs crates/trust-runtime/src/runtime/vm/register_ir/tests/*.rs` reports every split file under 1,000 lines; `cargo test -p trust-runtime --lib register_ir::tests -- --nocapture` passed with 55 tests and preserved the `runtime::vm::register_ir::tests::*` module path; `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed with the stale `register_ir/tests.rs` allowlist row removed.
- [x] `RTLARGE-P2-004` Split `memory.rs` by layout, access, retain interaction, and tests if ownership analysis confirms mixed responsibilities. Evidence: `wc -l crates/trust-runtime/src/memory.rs crates/trust-runtime/src/memory/*.rs` reports every split file under 1,000 lines; `cargo test -p trust-runtime --lib memory::tests -- --nocapture` passed with 14 tests; `cargo test -p trust-runtime --test instances -- --nocapture` passed; `cargo test -p trust-runtime --test memory_lifetime -- --nocapture` passed; `cargo test -p trust-runtime --test vars_access var_config_memory_binding_syncs_with_program_storage -- --nocapture` passed; `cargo clippy -p trust-runtime --lib -- -D warnings` passed; `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed with the stale `memory.rs` allowlist row removed.

## Phase 3 - Doctor Gates

- [x] `RTLARGE-P3-001` Add full-map doctor report for files >1,000 lines. Evidence: `FULLMAP-CHECK-10` report lists all current runtime `src` and runtime `tests` large files with line counts, owners, and split plans.
- [x] `RTLARGE-P3-002` Fail new files >1,000 lines without note. Evidence: `known_bad_large_runtime_file_without_owner_note_fails` and `known_bad_large_runtime_test_file_without_owner_note_fails`.
- [x] `RTLARGE-P3-003` Fail files >1,500 lines without split plan or waiver. Evidence: `FULLMAP-CHECK-10` enforces split-plan metadata for files over the configured split-plan line limit.
- [ ] `RTLARGE-P3-004` Report public API growth from splits.

## Exit Criteria

- [x] `RTLARGE-EXIT-01` Every >1,000-line runtime file has owner/split note. Evidence: `FULLMAP-CHECK-10` passed on 2026-05-01 with all 7 remaining runtime hotspots listed after the `call.rs`, `config_ui_routes.rs`, `register_ir/tests.rs`, and `memory.rs` splits.
- [x] `RTLARGE-EXIT-02` Every >1,500-line runtime file has split plan, completed split, or dated waiver. Evidence: `FULLMAP-CHECK-10` passed on 2026-05-01 with split plans for the three remaining files over 1,500 lines after the `call.rs`, `config_ui_routes.rs`, `register_ir/tests.rs`, and `memory.rs` splits.
- [x] `RTLARGE-EXIT-03` At least the top two risk-ranked files have concrete split branches or completed splits. Evidence: `RTLARGE-P2-001` split `runtime/vm/call.rs` and `RTLARGE-P2-002` split `web/config_ui_routes.rs`; both stale large-file allowlist rows were removed and `FULLMAP-CHECK-10` passed.
- [x] `RTLARGE-EXIT-04` Doctor blocks new large-file regressions. Evidence: `FULLMAP-CHECK-10` blocks missing notes for runtime `src` and runtime `tests`, blocks stale allowlist paths, and passed after policy cleanup.
