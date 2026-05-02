# Runtime VM Mutation Hardening Execution Checklist

Status: Complete; focused runtime VM mutation shards for call dispatch, register IR root/interpreter/lowering, and tier1 compile/execute/state are closed with zero missed/timeout mutants, and full-map doctor output includes the runtime VM mutation evidence.
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

- [x] `RTVMMUT-STOP-01` Do not claim VM behavior is protected because integration tests are broad. Evidence: the closed claim is based on the exact `scripts/runtime_vm_mutation_shards.sh --run <shard>` artifacts, not on broad integration-test presence.
- [x] `RTVMMUT-STOP-02` Do not accept surviving mutants without equivalent-mutant rationale. Evidence: all selected shard `missed.txt` and `timeout.txt` files are empty; the redundant always-true tier1 binary-op guard was removed instead of accepting an equivalent missed mutant.
- [x] `RTVMMUT-STOP-03` Do not mutate-test only unreachable or test-only code and call the VM covered. Evidence: selected shards target production VM call dispatch, register-IR lowering/interpreter, and tier1 compile/execute/state files; test-only helpers are supporting coverage only.
- [x] `RTVMMUT-STOP-04` Do not weaken existing VM parity/differential tests to make mutation pass. Evidence: changes add focused semantic tests and full-map reporting; existing parity/differential assertions remain active and `cargo test -p trust-runtime --lib register_ir::tests -- --nocapture` passes.

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
- [x] `RTVMMUT-P1-002` Run the exact `RTVMMUT-P0-003` and `RTVMMUT-P0-004` commands for register IR root/lowering. Evidence: `register-ir-root` 92 total / 74 caught / 18 unviable / 0 missed / 0 timeout; `register-ir-interpreter` 9 total / 6 caught / 3 unviable / 0 missed / 0 timeout; `register-ir-lower-root` 68 total / 63 caught / 5 unviable / 0 missed / 0 timeout; `register-ir-lower-decode` 138 total / 136 caught / 2 unviable / 0 missed / 0 timeout; `register-ir-lower-fuse` 175 total / 172 caught / 3 unviable / 0 missed / 0 timeout; `register-ir-lower-verify` 11 total / 11 caught / 0 unviable / 0 missed / 0 timeout.
- [x] `RTVMMUT-P1-003` Run the exact `RTVMMUT-P0-005` command for tier1/register execution if the active branch contains the file. Evidence: `register-ir-tier1-root` 32 total / 27 caught / 5 unviable / 0 missed / 0 timeout; `register-ir-tier1-compile` 2 total / 1 caught / 1 unviable / 0 missed / 0 timeout; `register-ir-tier1-execute` 8 total / 6 caught / 2 unviable / 0 missed / 0 timeout; `register-ir-tier1-state` 32 total / 31 caught / 1 unviable / 0 missed / 0 timeout.
- [x] `RTVMMUT-P1-004` Store survivor lists and `--list --json` mutant lists as artifacts. Evidence: Phase 0 list artifacts are present under `target/gate-artifacts/runtime-vm-mutants/lists/`; all selected Phase 1 shard outcome/survivor artifacts are present under `target/gate-artifacts/runtime-vm-mutants/<shard>/mutants.out/`, and their `missed.txt` / `timeout.txt` files are empty.
- [x] `RTVMMUT-P1-005` Classify survivors by semantic area and by test target that should have killed them. Evidence: previous call-dispatch, register-IR root/interpreter, lower-root, and lower-decode survivors remain closed; lower-fuse survivors were closed by direct fusion-window, guard-failure, compare-jump, and `instruction_reads_register` operand tests; lower-verify survivors were closed by undefined-source and move-destination verifier tests; tier1 survivors were closed by DINT guard arithmetic/comparison tests, fused binary and compare compile tests, direct compiled execute branch/null-reference tests, and tier1 state/env/reset tests.

## Phase 2 - Semantic Matrix

- [x] `RTVMMUT-P2-001` Arithmetic and comparison opcode behavior. Evidence: `register_ir::tests` covers DINT tier1 guard exact arithmetic/comparison results, binary opcode-family lowering, ref/const fused binary variants, and comparison jump guards.
- [x] `RTVMMUT-P2-002` Branch/jump/control-flow behavior. Evidence: decode tests cover branch leaders, fallthrough, return termination, invalid targets, and block-entry depths; tier1 direct execution tests cover compare-jump and `JumpIf` branch conditions.
- [x] `RTVMMUT-P2-003` FB/class method call behavior. Evidence: register-IR and tier1 tests cover function calls, function block calls, self-field dynamic ops, load-super dynamic blocks, and call-native function-block paths without fallback.
- [x] `RTVMMUT-P2-004` String/array/struct access behavior. Evidence: register-IR corpus diagnostics execute string stdlib/case fixtures without fallback; tier1 tests cover array reference blocks and struct/function-block IN_OUT clone behavior.
- [x] `RTVMMUT-P2-005` Reference and pointer behavior. Evidence: tests cover load-ref-address, ref-field/ref-index, dynamic load/store, null-reference read helpers, borrowed ref/ref and ref/const binary guards, and fused ref-to-ref operations.
- [x] `RTVMMUT-P2-006` Error mapping behavior. Evidence: tests cover verifier invalid bytecode, invalid jump targets, null reference, condition-not-bool, division/modulo by zero, unsupported opcode fallback reasons, and lowering-cache error caching.
- [x] `RTVMMUT-P2-007` Register IR lowering behavior for supported instruction families. Evidence: lower-root/decode/fuse/verify shards are closed with zero missed/timeouts, and `register_ir::tests` covers NOP/null/full binary opcode families, stack normalization, call-native/swap depth, return termination, fallback operand preservation, and fuse windows.
- [x] `RTVMMUT-P2-008` Tier1 fallback/deopt behavior where applicable. Evidence: tier1 tests cover cold/hot thresholds, compile failure reasons, bool/non-DINT execution without deopt, cache hits/evictions, reset/env state, and full tier1 shards are closed with zero missed/timeouts.

## Phase 3 - Mutation Gate

- [x] `RTVMMUT-P3-001` Rerun focused VM mutation shards. Evidence: all 14 selected shards are represented in `FULLMAP-RUNTIMEVM-MUT` from `target/gate-artifacts/full-software-map-14b2200b7/full-map-report.json`.
- [x] `RTVMMUT-P3-002` Reduce unexplained survivors to zero for selected shards. Evidence: `FULLMAP-RUNTIMEVM-MUT` reports 0 missed and 0 timeout mutants for every selected shard.
- [x] `RTVMMUT-P3-003` Document equivalent mutants. Evidence: no selected shard has a remaining missed/equivalent mutant; the redundant tier1 all-`BinaryOp` support guard was removed rather than documented as an accepted equivalent.
- [x] `RTVMMUT-P3-004` Add scheduled or manual gate command. Evidence: manual shard gate remains `TRUST_VM_MUTANTS_IN_PLACE=1 scripts/runtime_vm_mutation_shards.sh --run <shard>` from a clean tracked tree; `cargo xtask architecture-doctor --full-map` now reports selected shard evidence through `FULLMAP-RUNTIMEVM-MUT`.

## Exit Criteria

- [x] `RTVMMUT-EXIT-01` Focused VM semantic tests pass. Evidence: `cargo test -p trust-runtime --lib register_ir::tests -- --nocapture` passed 94 tests.
- [x] `RTVMMUT-EXIT-02` Focused VM mutation shard has zero unexplained survivors. Evidence: `FULLMAP-RUNTIMEVM-MUT` reports all 14 selected shards with 0 missed and 0 timeout mutants.
- [x] `RTVMMUT-EXIT-03` VM mutation evidence is included in full-map doctor/report output. Evidence: `cargo xtask architecture-doctor --full-map` passed and wrote `target/gate-artifacts/full-software-map-14b2200b7/full-map-report.json` / `.md` with `PASS: FULLMAP-RUNTIMEVM-MUT`.
