# Runtime VM Mutation Hardening Execution Checklist

Status: In progress; Phase 1 VM call-dispatch, register-IR root/interpreter, and register-IR lower-root baselines closed with zero missed/timeout mutants; remaining register-IR lowering baselines are next.
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
- [x] `RTVMMUT-P0-008` Local baseline runs may use in-place mutation only with a clean tracked worktree. Evidence: `TRUST_VM_MUTANTS_IN_PLACE=1 scripts/runtime_vm_mutation_shards.sh --run <shard>` adds `--in-place` only after `git status --porcelain --untracked-files=no` is empty and drops copy-only options (`--jobs`, `--gitignore`) that cargo-mutants rejects with in-place mode, allowing local shards to reuse the existing target cache without risking hidden source mutations in a dirty tree.

## Phase 1 - Baseline

- [x] `RTVMMUT-P1-001` Run the exact `RTVMMUT-P0-002` command for VM call dispatch. Evidence: in-place reruns from clean tracked commits closed all call-dispatch shards under `target/gate-artifacts/runtime-vm-mutants/`: `call-root` 25 total / 22 caught / 3 unviable / 0 missed / 0 timeout; `call-bindings` 83 total / 59 caught / 24 unviable / 0 missed / 0 timeout; `call-stdlib` 58 total / 48 caught / 10 unviable / 0 missed / 0 timeout; `call-symbols` 4 total / 2 caught / 2 unviable / 0 missed / 0 timeout.
- [ ] `RTVMMUT-P1-002` Run the exact `RTVMMUT-P0-003` and `RTVMMUT-P0-004` commands for register IR root/lowering. Partial evidence: `register-ir-root` was rerun in-place from clean tracked commit `3471286b2` and closed at 92 total / 74 caught / 18 unviable / 0 missed / 0 timeout under `target/gate-artifacts/runtime-vm-mutants/register-ir-root/mutants.out/`; `register-ir-interpreter` was rerun in-place from clean tracked commit `f3f4727c1` and closed at 9 total / 6 caught / 3 unviable / 0 missed / 0 timeout under `target/gate-artifacts/runtime-vm-mutants/register-ir-interpreter/mutants.out/`; `register-ir-lower-root` was rerun in-place from clean tracked commit `cbd028f6c` and closed at 68 total / 63 caught / 5 unviable / 0 missed / 0 timeout under `target/gate-artifacts/runtime-vm-mutants/register-ir-lower-root/mutants.out/`; `register-ir-lower-decode` was rerun in-place from clean tracked commit `1722687ec` and closed at 138 total / 136 caught / 2 unviable / 0 missed / 0 timeout under `target/gate-artifacts/runtime-vm-mutants/register-ir-lower-decode/mutants.out/`; remaining P1-002 shards are `register-ir-lower-fuse` and `register-ir-lower-verify`.
- [ ] `RTVMMUT-P1-003` Run the exact `RTVMMUT-P0-005` command for tier1/register execution if the active branch contains the file.
- [ ] `RTVMMUT-P1-004` Store survivor lists and `--list --json` mutant lists as artifacts. Partial evidence: Phase 0 list artifacts are present under `target/gate-artifacts/runtime-vm-mutants/lists/`; Phase 1 call-dispatch, `register-ir-root`, `register-ir-interpreter`, `register-ir-lower-root`, and `register-ir-lower-decode` survivor files are present under each shard's `mutants.out/` directory and their `missed.txt` / `timeout.txt` files are empty.
- [ ] `RTVMMUT-P1-005` Classify survivors by semantic area and by test target that should have killed them. Partial evidence: call-dispatch survivors were reduced to zero after adding semantic tests for builtin FB call execution, stdlib fixed/variadic argument binding, split-time output dispatch, VM/native output binding, local reference writes, and integer output conversion; `register-ir-root` survivors were reduced to zero after adding semantic tests for execution-buffer pool return/limits, register-file preparation, env bool parsing, initial-local capacity, linear fallthrough target selection, bool/reference helper errors, loop budget, block-id lookup, debug statement mapping, and deadline boundaries plus a deterministic block-id invariant for corrupted helper resolution; `register-ir-interpreter` survivor was reduced to zero by adding an interpreted `RefField` null-reference-base test; `register-ir-lower-root` survivors were reduced to zero by adding stack-normalization tests for protected registers and independent register cycles, opcode-family lowering tests for NOP/LoadNull/binary operators, call-native/swap stack-depth tests, and a return-termination test, plus a bounded normalization loop that converts non-converging lowering mutations into explicit invalid-bytecode errors; `register-ir-lower-decode` survivors were reduced to zero by adding decode tests for conflicting block-entry stack depths, ROT3/ROT4 underflow and exact-depth acceptance, exit/fallthrough leader exclusion, RETURN termination of entry-depth propagation, conditional fallthrough at `code_end`, and fallback operand preservation.

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
