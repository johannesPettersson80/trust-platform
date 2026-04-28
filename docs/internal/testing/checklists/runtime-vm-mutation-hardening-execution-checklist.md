# Runtime VM Mutation Hardening Execution Checklist

Status: Planned
Owner: Runtime VM team
Scope: add mutation-backed semantic tests for high-risk VM execution paths before claiming zero silent bugs for runtime execution.

## Target Files

- [ ] `RTVMMUT-TARGET-01` `crates/trust-runtime/src/runtime/vm/call.rs`.
- [ ] `RTVMMUT-TARGET-02` `crates/trust-runtime/src/runtime/vm/dispatch.rs` if present in the active branch.
- [ ] `RTVMMUT-TARGET-03` `crates/trust-runtime/src/runtime/vm/register_ir.rs`.
- [ ] `RTVMMUT-TARGET-04` `crates/trust-runtime/src/runtime/vm/register_ir/lower.rs`.
- [ ] `RTVMMUT-TARGET-05` `crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs`.
- [ ] `RTVMMUT-TARGET-06` frame/value/reference helper modules used by VM execution.

## Stop Rules

- [ ] `RTVMMUT-STOP-01` Do not claim VM behavior is protected because integration tests are broad.
- [ ] `RTVMMUT-STOP-02` Do not accept surviving mutants without equivalent-mutant rationale.
- [ ] `RTVMMUT-STOP-03` Do not mutate-test only unreachable or test-only code and call the VM covered.
- [ ] `RTVMMUT-STOP-04` Do not weaken existing VM parity/differential tests to make mutation pass.

## Phase 0 - Exact Mutation Command Lock

- [ ] `RTVMMUT-P0-001` Confirm `cargo mutants --help` supports `--package`, `--file`, `--output`, and passing focused `cargo test` args after `--`.
- [ ] `RTVMMUT-P0-002` Lock call-dispatch command:
  `cargo mutants -p trust-runtime --file crates/trust-runtime/src/runtime/vm/call.rs --output target/gate-artifacts/runtime-vm-mutants/call -- --test bytecode_vm_core`
- [ ] `RTVMMUT-P0-003` Lock register-IR root command:
  `cargo mutants -p trust-runtime --file crates/trust-runtime/src/runtime/vm/register_ir.rs --output target/gate-artifacts/runtime-vm-mutants/register-ir -- --test bytecode_vm_core`
- [ ] `RTVMMUT-P0-004` Lock register-IR lowering command:
  `cargo mutants -p trust-runtime --file crates/trust-runtime/src/runtime/vm/register_ir/lower.rs --output target/gate-artifacts/runtime-vm-mutants/register-ir-lower -- --test bytecode_vm_core`
- [ ] `RTVMMUT-P0-005` Lock tier1 command:
  `cargo mutants -p trust-runtime --file crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs --output target/gate-artifacts/runtime-vm-mutants/tier1 -- --test bytecode_vm_core`
- [ ] `RTVMMUT-P0-006` If any command is too broad or misses target mutants, replace it only by recording the exact replacement command and the `cargo mutants --list --json` evidence that proves the replacement covers the intended file.

## Phase 1 - Baseline

- [ ] `RTVMMUT-P1-001` Run the exact `RTVMMUT-P0-002` command for VM call dispatch.
- [ ] `RTVMMUT-P1-002` Run the exact `RTVMMUT-P0-003` and `RTVMMUT-P0-004` commands for register IR root/lowering.
- [ ] `RTVMMUT-P1-003` Run the exact `RTVMMUT-P0-005` command for tier1/register execution if the active branch contains the file.
- [ ] `RTVMMUT-P1-004` Store survivor lists and `--list --json` mutant lists as artifacts.
- [ ] `RTVMMUT-P1-005` Classify survivors by semantic area and by test target that should have killed them.

## Phase 2 - Semantic Matrix

- [ ] `RTVMMUT-P2-001` Arithmetic and comparison opcode behavior.
- [ ] `RTVMMUT-P2-002` Branch/jump/control-flow behavior.
- [ ] `RTVMMUT-P2-003` FB/class method call behavior.
- [ ] `RTVMMUT-P2-004` String/array/struct access behavior.
- [ ] `RTVMMUT-P2-005` Reference and pointer behavior.
- [ ] `RTVMMUT-P2-006` Error mapping behavior.
- [ ] `RTVMMUT-P2-007` Register IR lowering behavior for supported instruction families.
- [ ] `RTVMMUT-P2-008` Tier1 fallback/deopt behavior where applicable.

## Phase 3 - Mutation Gate

- [ ] `RTVMMUT-P3-001` Rerun focused VM mutation shards.
- [ ] `RTVMMUT-P3-002` Reduce unexplained survivors to zero for selected shards.
- [ ] `RTVMMUT-P3-003` Document equivalent mutants.
- [ ] `RTVMMUT-P3-004` Add scheduled or manual gate command.

## Exit Criteria

- [ ] `RTVMMUT-EXIT-01` Focused VM semantic tests pass.
- [ ] `RTVMMUT-EXIT-02` Focused VM mutation shard has zero unexplained survivors.
- [ ] `RTVMMUT-EXIT-03` VM mutation evidence is included in full-map doctor/report output.
