# Runtime Large-File Split Execution Checklist

Status: Planned
Owner: Runtime architecture
Scope: address audit F8 and KISS risks from very large runtime files.

## Quantitative Rules

- [ ] `RTLARGE-RULE-01` No new Rust source file over 1,000 lines without owner/split note.
- [ ] `RTLARGE-RULE-02` Every existing Rust source file over 1,000 lines must have owner, responsibility statement, and split plan or waiver.
- [ ] `RTLARGE-RULE-03` Every Rust source file over 1,500 lines must have an approved split branch, completed split, or dated waiver.
- [ ] `RTLARGE-RULE-04` Files over 2,500 lines are release-blocking for unrelated growth until split or waiver.
- [ ] `RTLARGE-RULE-05` Public API growth caused by splits requires review, not automatic acceptance.

## Initial Hotspot Set

- [ ] `RTLARGE-HOT-01` `crates/trust-runtime/src/runtime/vm/register_ir/tests.rs`.
- [ ] `RTLARGE-HOT-02` `crates/trust-runtime/src/runtime/vm/call.rs`.
- [ ] `RTLARGE-HOT-03` `crates/trust-runtime/src/web/config_ui_routes.rs`.
- [ ] `RTLARGE-HOT-04` `crates/trust-runtime/tests/agent_command.rs`.
- [ ] `RTLARGE-HOT-05` `crates/trust-runtime/src/runtime/vm/register_ir.rs`.
- [ ] `RTLARGE-HOT-06` `crates/trust-runtime/tests/bytecode_vm_core.rs`.
- [ ] `RTLARGE-HOT-07` `crates/trust-runtime/src/bin/trust-runtime/agent.rs`.
- [ ] `RTLARGE-HOT-08` `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09.rs`.
- [ ] `RTLARGE-HOT-09` `crates/trust-runtime/src/runtime/vm/register_ir/lower.rs`.
- [ ] `RTLARGE-HOT-10` `crates/trust-runtime/src/memory.rs`.
- [ ] `RTLARGE-HOT-11` `crates/trust-runtime/src/value/types.rs`.
- [ ] `RTLARGE-HOT-12` `crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs`.

## Phase 0 - Full-Map Prerequisite

- [ ] `RTLARGE-P0-001` Hard prerequisite before Phase 3 and before claiming `ARCHPROG-EXIT-07`: `architecture-doctor --full-map` MVP implements `FULLMAP-CHECK-10`.
- [ ] `RTLARGE-P0-002` If `FULLMAP-CHECK-10` is unavailable, record an owner-approved waiver with local large-file scan command, threshold policy, owner, and expiration date.
- [ ] `RTLARGE-P0-GATE-01` Do not treat owner/split notes as sufficient unless the full-map doctor or waiver proves new regressions are blocked.

## Phase 1 - Owner/Split Notes

- [ ] `RTLARGE-P1-001` Generate current >1,000-line file list.
- [ ] `RTLARGE-P1-002` Add owner/split note for each file.
- [ ] `RTLARGE-P1-003` Classify each file as test-only, runtime-hot-path, route/controller, model/value, or CLI/workbench.
- [ ] `RTLARGE-P1-004` Identify behavior-lock tests needed before each split.
- [ ] `RTLARGE-P1-005` Prioritize files by risk, churn, and runtime criticality.

## Phase 2 - First High-Risk Splits

- [ ] `RTLARGE-P2-001` Split `runtime/vm/call.rs` by dispatch, FB/class method semantics, and error mapping.
- [ ] `RTLARGE-P2-002` Split `web/config_ui_routes.rs` by routing, persistence, request models, response models, and domain services.
- [ ] `RTLARGE-P2-003` Split `runtime/vm/register_ir/tests.rs` by feature/domain without losing test names unnecessarily.
- [ ] `RTLARGE-P2-004` Split `memory.rs` by layout, access, retain interaction, and tests if ownership analysis confirms mixed responsibilities.

## Phase 3 - Doctor Gates

- [ ] `RTLARGE-P3-001` Add full-map doctor report for files >1,000 lines.
- [ ] `RTLARGE-P3-002` Fail new files >1,000 lines without note.
- [ ] `RTLARGE-P3-003` Fail files >1,500 lines without split plan or waiver.
- [ ] `RTLARGE-P3-004` Report public API growth from splits.

## Exit Criteria

- [ ] `RTLARGE-EXIT-01` Every >1,000-line runtime file has owner/split note.
- [ ] `RTLARGE-EXIT-02` Every >1,500-line runtime file has split plan, completed split, or dated waiver.
- [ ] `RTLARGE-EXIT-03` At least the top two risk-ranked files have concrete split branches or completed splits.
- [ ] `RTLARGE-EXIT-04` Doctor blocks new large-file regressions.
