# Runtime VM Mutation Hardening Execution Checklist

Status: In progress; Phase 0 mutation command lock captured for the current post-large-file-split VM module layout.
Owner: Runtime VM team
Scope: add mutation-backed semantic tests for high-risk VM execution paths before claiming zero silent bugs for runtime execution.

## Target Files

- [x] `RTVMMUT-TARGET-01` `crates/trust-runtime/src/runtime/vm/call.rs` plus current split child modules `call/bindings.rs`, `call/stdlib.rs`, and `call/symbols.rs`.
- [x] `RTVMMUT-TARGET-02` `crates/trust-runtime/src/runtime/vm/dispatch.rs` plus current split helper modules `dispatch_refs.rs` and `dispatch_sizeof.rs`.
- [x] `RTVMMUT-TARGET-03` `crates/trust-runtime/src/runtime/vm/register_ir.rs` plus current interpreted execution helper `register_ir/interpreter.rs`.
- [x] `RTVMMUT-TARGET-04` `crates/trust-runtime/src/runtime/vm/register_ir/lower.rs` plus current split child modules `lower/decode.rs`, `lower/fuse.rs`, and `lower/verify.rs`.
- [x] `RTVMMUT-TARGET-05` `crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs` plus current split child modules `tier1/compile.rs`, `tier1/execute.rs`, and `tier1/state.rs`.
- [x] `RTVMMUT-TARGET-06` frame/value/reference helper modules used by VM execution. Evidence: Phase 0 includes `runtime/vm/stack.rs`, `memory/references.rs`, and `memory/frames.rs`; `vm-stack` currently produces zero cargo-mutants candidates and is recorded as such.

## Stop Rules

- [ ] `RTVMMUT-STOP-01` Do not claim VM behavior is protected because integration tests are broad.
- [ ] `RTVMMUT-STOP-02` Do not accept surviving mutants without equivalent-mutant rationale.
- [ ] `RTVMMUT-STOP-03` Do not mutate-test only unreachable or test-only code and call the VM covered.
- [ ] `RTVMMUT-STOP-04` Do not weaken existing VM parity/differential tests to make mutation pass.

## Phase 0 - Exact Mutation Command Lock

- [x] `RTVMMUT-P0-001` Confirm `cargo mutants --help` supports `--package`, `--file`, `--output`, and passing focused `cargo test` args after `--`. Evidence: `cargo-mutants 27.0.0`; help lists `-p, --package`, `-f, --file`, `-o, --output`, `--list`, `--json`, and trailing `[CARGO_TEST_ARGS]...` after `--`.
- [x] `RTVMMUT-P0-002` Lock call-dispatch command. Evidence: `scripts/runtime_vm_mutation_shards.sh --run [shard-name]` runs exact `cargo mutants -p trust-runtime --file ... --output target/gate-artifacts/runtime-vm-mutants/<shard> -- <focused cargo test args>` shards for `call-root`, `call-bindings`, `call-stdlib`, and `call-symbols`; `--list --json` artifacts record 25, 100, 62, and 4 mutants respectively.
- [x] `RTVMMUT-P0-003` Lock register-IR root command. Evidence: `scripts/runtime_vm_mutation_shards.sh --run [shard-name]` runs `register-ir-root` and `register-ir-interpreter` against focused `--lib register_ir::tests`; `--list --json` artifacts record 98 and 9 mutants respectively.
- [x] `RTVMMUT-P0-004` Lock register-IR lowering command. Evidence: `scripts/runtime_vm_mutation_shards.sh --run [shard-name]` runs `register-ir-lower-root`, `register-ir-lower-decode`, `register-ir-lower-fuse`, and `register-ir-lower-verify` against focused `--lib register_ir::tests`; `--list --json` artifacts record 74, 138, 179, and 11 mutants respectively.
- [x] `RTVMMUT-P0-005` Lock tier1 command. Evidence: `scripts/runtime_vm_mutation_shards.sh --run [shard-name]` runs `register-ir-tier1-root`, `register-ir-tier1-compile`, `register-ir-tier1-execute`, and `register-ir-tier1-state` against focused `--lib register_ir::tests`; `--list --json` artifacts record 32, 8, 8, and 32 mutants respectively.
- [x] `RTVMMUT-P0-006` If any command is too broad or misses target mutants, replace it only by recording the exact replacement command and the `cargo mutants --list --json` evidence that proves the replacement covers the intended file. Evidence: the pre-split root-only commands would miss current child modules after BOARD-08; `scripts/runtime_vm_mutation_shards.sh --list` wrote 20 shard list artifacts under `target/gate-artifacts/runtime-vm-mutants/lists/` with 1,035 total candidate mutants across current files.
- [x] `RTVMMUT-P0-007` Mutation runs must avoid copying ignored local caches and captured browser artifacts and must constrain the build target itself. Evidence: the first `call-symbols` baseline attempt showed cargo-mutants copying 4.0GB / 134,678 files when `.gitignore` was not honored, then building unrelated package targets that pulled in OpenSSL/aws-lc/Zenoh before the focused test ran; `scripts/runtime_vm_mutation_shards.sh` now passes `--gitignore true` plus per-shard `--cargo-arg --lib` or `--cargo-arg --test --cargo-arg bytecode_vm_core` while keeping exact per-shard `--file` targeting.
- [x] `RTVMMUT-P0-008` Local baseline runs may use in-place mutation only with a clean tracked worktree. Evidence: `TRUST_VM_MUTANTS_IN_PLACE=1 scripts/runtime_vm_mutation_shards.sh --run <shard>` adds `--in-place` only after `git status --porcelain --untracked-files=no` is empty, allowing local shards to reuse the existing target cache without risking hidden source mutations in a dirty tree.

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
