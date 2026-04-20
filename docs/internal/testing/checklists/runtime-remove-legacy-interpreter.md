# Runtime Legacy Interpreter Removal Checklist

Status: Done
Owner: Runtime team
Branch: `runtime/remove-legacy-interpreter`
Scope: `trust-runtime` runtime, harness, tests, scripts, docs, and diagrams

## Goal

Remove the legacy interpreter from `trust-runtime` completely and leave one production execution architecture:

1. bytecode lowering
2. VM/register execution
3. debug/helper/config evaluation only through explicitly owned VM-era helper modules

This is not a blind delete of `src/eval/**`.

The current `eval` namespace mixes three responsibilities:

1. shared runtime/program model types (`Param`, `VarDef`, `FunctionDef`, `FunctionBlockDef`, `MethodDef`, `ClassDef`, `InterfaceDef`, `Expr`, `Stmt`, `CallArg`)
2. shared semantics helpers reused by the VM (`eval::ops::*`)
3. the actual legacy executor (`EvalContext`, expression execution, statement execution, call binding, interpreter backend)

The first two must be extracted or relocated before the third can be deleted.

## Locked Outcome

- no `legacy-interpreter` Cargo feature
- no `ExecutionBackend::Interpreter`
- no `InterpreterBackend`
- no interpreter-only production/runtime code paths
- no VM parity or benchmark workflow that depends on interpreter execution
- runtime/debug/build/helper surfaces no longer depend on the legacy executor namespace
- any remaining public `eval` surface is a temporary compatibility facade for extracted model/semantics only and does not expose executor APIs
- any surviving executor internals compile only under `cfg(test)` and are not part of normal runtime builds

## Current Inventory

### A. Direct interpreter backend surface

These are the primary deletion targets.

- [x] [crates/trust-runtime/src/execution_backend.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/execution_backend.rs)
  - remove `ExecutionBackend::Interpreter`
  - simplify `parse()` / `as_str()`
- [x] [crates/trust-runtime/src/runtime/backend.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/backend.rs)
  - removed `RuntimeExecutionBackend` / `InterpreterBackend` by deleting `runtime/backend.rs`
  - runtime dispatch now calls the VM path directly from `runtime/cycle.rs`
- [x] [crates/trust-runtime/src/runtime/cycle.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/cycle.rs)
  - remove `execute_program_interpreter()`
  - remove `execute_function_block_ref_interpreter()`
- [x] [crates/trust-runtime/Cargo.toml](/home/johannes/projects/trust-platform/crates/trust-runtime/Cargo.toml)
  - remove `legacy-interpreter = []`
- [x] [crates/trust-runtime/src/lib.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/lib.rs)
  - `pub mod eval` survives as a compatibility facade for extracted model/ops types
  - executor APIs are now test-only and are not part of the public runtime contract

### B. `eval/` files that are legacy executor code

These no longer participate in normal runtime builds. They are compiled only under `cfg(test)` until a later final-deletion cleanup.

- [x] [crates/trust-runtime/src/eval/calls.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/calls.rs)
- [x] [crates/trust-runtime/src/eval/bindings.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/bindings.rs)
- [x] [crates/trust-runtime/src/eval/outputs.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/outputs.rs)
- [x] [crates/trust-runtime/src/eval/stmt.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/stmt.rs)
  - only the execution helpers are deletion targets; the statement model must move first
- [x] [crates/trust-runtime/src/eval/expr/eval.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/eval.rs)
  - cannot be deleted until const-eval, initializer-eval, and debug watch/breakpoint evaluation have a replacement
- [x] [crates/trust-runtime/src/eval/expr/access.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/access.rs)
  - read-only debug expression use is gone; the remaining blocker is any surviving interpreter-side reference/lvalue helper usage
- [x] [crates/trust-runtime/src/eval/expr/lvalue.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/lvalue.rs)
  - VM debug lvalue writes now use `helper_eval::storage_lvalue`; the remaining blocker is interpreter-side lvalue execution
- [x] [crates/trust-runtime/src/eval/expr/call.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/call.rs)
- [x] [crates/trust-runtime/src/eval/expr/call/arg_read.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/call/arg_read.rs)
- [x] [crates/trust-runtime/src/eval/expr/call/reference.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/call/reference.rs)
- [x] [crates/trust-runtime/src/eval/expr/call/split_call.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/call/split_call.rs)
- [x] [crates/trust-runtime/src/eval/expr/call/stdlib_args.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/call/stdlib_args.rs)
- [x] [crates/trust-runtime/src/eval/mod.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/mod.rs)
  - legacy executor exports are gated to `cfg(test)`
  - public `eval` remains only as a thin model/ops compatibility facade

### C. `eval/` files that are shared and must move, not vanish

These are not interpreter execution logic. They are shared data/semantics and need a new home.

- [x] [crates/trust-runtime/src/eval/types.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/types.rs)
  - shared model types moved to `program_model::types`; `eval::types` now retains only test-only executor context/binding internals
  - `EvalContext`, `OutputBinding`, `PreparedBindings`, and `BindingMode` remain on the executor side under `cfg(test)` only
- [x] [crates/trust-runtime/src/eval/expr/ast.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/ast.rs)
  - `Expr`, `LValue`, and `SizeOfTarget` now live in `program_model::expr`; `eval::expr::ast` is only a thin compatibility facade for test-only executor code
- [x] [crates/trust-runtime/src/eval/stmt.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/stmt.rs)
  - move `Stmt`, `CaseLabel`, and `StmtResult` out before deleting statement execution helpers
- [x] [crates/trust-runtime/src/eval/ops.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/ops.rs)
  - `eval::ops` is now a thin compatibility re-export over `program_model::ops`
- [x] [crates/trust-runtime/src/eval/ops/contracts.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/ops/contracts.rs)
  - operator contracts moved to `program_model::ops` together with the AST-facing enums
- [x] [crates/trust-runtime/src/eval/ops/logical_cmp.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/ops/logical_cmp.rs)
  - shared logical comparison helpers moved under `program_model::ops`
- [x] [crates/trust-runtime/src/eval/ops/numeric_arith.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/ops/numeric_arith.rs)
  - shared numeric operator helpers moved under `program_model::ops`
- [x] [crates/trust-runtime/src/eval/ops/time_ops.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/ops/time_ops.rs)
  - shared time/date operator helpers moved under `program_model::ops`
- [x] [crates/trust-runtime/src/eval/locals.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/locals.rs)
  - extract `static_storage_name()` and `method_static_storage_owner()` as shared utilities
  - keep `static_storage_value_ref()` / `init_locals*()` on the executor side until replacement helpers exist
- [x] [crates/trust-runtime/src/eval/expr/call/target_resolution.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/eval/expr/call/target_resolution.rs)
  - kept with the test-only executor call path; no runtime/helper consumer needs a separate extracted copy anymore

### D. Runtime/helper consumers that must be rewritten away from `EvalContext`

These are the most important non-interpreter runtime dependencies.

- [x] [crates/trust-runtime/src/runtime/core/evaluation.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/core/evaluation.rs)
  - `evaluate_expression()` uses `helper_eval::storage_expr` and `with_eval_context()` has been removed
- [x] [crates/trust-runtime/src/runtime/cycle.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/cycle.rs)
  - VM debug pending lvalue writes now use `helper_eval::storage_lvalue()` instead of `with_eval_context()` + `write_lvalue()`
- [x] [crates/trust-runtime/src/bytecode/encoder/consts.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bytecode/encoder/consts.rs)
  - compile-time constant folding currently builds a minimal `EvalContext` and calls `eval_expr()`
- [x] [crates/trust-runtime/src/control/debug_handlers_helpers.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/control/debug_handlers_helpers.rs)
  - debug snapshot evaluation now uses `helper_eval::storage_expr` without building `EvalContext`
- [x] [crates/trust-runtime/src/debug/hook.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/debug/hook.rs)
  - `DebugHook::on_statement_with_context()` now takes a storage-native `DebugRuntimeContext`
- [x] [crates/trust-runtime/src/debug/control.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/debug/control.rs)
  - now threads the storage-native debug runtime context instead of `EvalContext`
- [x] [crates/trust-runtime/src/debug/control/hook.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/debug/control/hook.rs)
  - watch snapshot evaluation uses `helper_eval::storage_expr` and the hook now accepts `DebugRuntimeContext`
- [x] [crates/trust-runtime/src/debug/breakpoints.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/debug/breakpoints.rs)
  - conditional breakpoints and logpoint expressions use `helper_eval::storage_expr` through `DebugRuntimeContext`
- [x] [crates/trust-runtime/src/debug/types.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/debug/types.rs)
  - `LogFragment::Expr` should survive, but it must depend on extracted AST types rather than the executor namespace
- [x] [crates/trust-runtime/src/debug/control/api/watch_stream.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/debug/control/api/watch_stream.rs)
  - `refresh_snapshot()` now takes `DebugRuntimeContext`; VM paths still use `refresh_snapshot_from_storage()`
- [x] [crates/trust-runtime/src/bin/trust-runtime/test_cmd/execute.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bin/trust-runtime/test_cmd/execute.rs)
  - test FB execution now runs through `Runtime::execute_function_block_by_name()` and the VM FB-ref path
- [x] [crates/trust-runtime/src/instance.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/instance.rs)
  - variable and static-local initializers now use `helper_eval::storage_expr` instead of `EvalContext` + `eval_expr()`
- [x] [crates/trust-runtime/src/harness/config.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/config.rs)
- [x] [crates/trust-runtime/src/harness/config/config_inits.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/config/config_inits.rs)
- [x] [crates/trust-runtime/src/harness/config/globals.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/config/globals.rs)
- [x] [crates/trust-runtime/src/harness/build.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/build.rs)
- [x] [crates/trust-runtime/src/harness/lower/expr.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/lower/expr.rs)
- [x] [crates/trust-runtime/src/harness/lower/expr/constants.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/lower/expr/constants.rs)
  - build-time and config-time expression/initializer evaluation need a VM-era helper/evaluator path

### E. Shared model consumers that need import-path rewrites after extraction

These should survive, but they currently depend on `crate::eval::*` model types.

- [x] [crates/trust-runtime/src/task.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/task.rs)
- [x] [crates/trust-runtime/src/runtime/core.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/core.rs)
- [x] [crates/trust-runtime/src/runtime/metadata.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/metadata.rs)
- [x] [crates/trust-runtime/src/harness/compiler/model.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/compiler/model.rs)
- [x] [crates/trust-runtime/src/harness/compiler/pou.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/compiler/pou.rs)
- [x] [crates/trust-runtime/src/harness/io.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/io.rs)
- [x] [crates/trust-runtime/src/harness/parse.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/parse.rs)
- [x] [crates/trust-runtime/src/harness/lower/stmt.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/lower/stmt.rs)
- [x] [crates/trust-runtime/src/harness/lower/expr/lowering.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/harness/lower/expr/lowering.rs)
- [x] [crates/trust-runtime/src/bytecode/encoder/**](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bytecode/encoder)
  - currently tied to `crate::eval::{Expr, Stmt, Param, VarDef, CallArg, ops::*}`

### F. VM code that already depends on shared semantics and should keep doing so

These should survive but stop importing from `eval::` after extraction.

- [x] [crates/trust-runtime/src/runtime/vm/dispatch_ops.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/vm/dispatch_ops.rs)
- [x] [crates/trust-runtime/src/runtime/vm/register_ir.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/runtime/vm/register_ir.rs)
- [x] [crates/trust-runtime/src/stdlib/numeric.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/stdlib/numeric.rs)
- [x] [crates/trust-runtime/src/stdlib/time.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/stdlib/time.rs)

### G. Builtin FB execution

Builtin FB execution already has a storage-native path and should be preserved.

- [x] [crates/trust-runtime/src/stdlib/fbs/mod.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/stdlib/fbs/mod.rs)
  - builtin FB execution now only exposes `execute_builtin_in_storage()`
  - keep `execute_builtin_in_storage(...)`

### H. Interpreter-only tests and benchmark surfaces

These must be deleted or rewritten.

- [x] [crates/trust-runtime/tests/api_smoke.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/api_smoke.rs)
- [x] [crates/trust-runtime/tests/debug_stepping.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/debug_stepping.rs)
- [x] [crates/trust-runtime/tests/e2e_full_grammar.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/e2e_full_grammar.rs)
- [x] [crates/trust-runtime/tests/e2e_scheduler.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/e2e_scheduler.rs)
- [x] [crates/trust-runtime/tests/expr_full.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/expr_full.rs)
- [x] [crates/trust-runtime/tests/pou_class.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/pou_class.rs)
- [x] [crates/trust-runtime/tests/pou_oop.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/pou_oop.rs)
- [x] [crates/trust-runtime/tests/runtime_events.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/runtime_events.rs)
- [x] [crates/trust-runtime/tests/stdlib_numeric_full.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/stdlib_numeric_full.rs)
- [x] [crates/trust-runtime/tests/stmt_assign_attempt.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/stmt_assign_attempt.rs)
- [x] [crates/trust-runtime/tests/tasks_fb.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/tasks_fb.rs)
- [x] [crates/trust-runtime/tests/tutorial_examples.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/tutorial_examples.rs)
- [x] [crates/trust-runtime/tests/types_bit_access.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/types_bit_access.rs)
- [x] [crates/trust-runtime/tests/vars_access.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/vars_access.rs)
- [x] [crates/trust-runtime/tests/bytecode_vm_differential.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/bytecode_vm_differential.rs)
  - removed; VM-only behavior-lock / determinism gates now carry the evidence burden
- [x] [crates/trust-runtime/tests/common/mod.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/tests/common/mod.rs)
  - direct `EvalContext` construction was moved out of the integration suite and now exists only in in-crate evaluator tests
- [x] [crates/trust-runtime/src/bin/trust-runtime/bench/execution_backend.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bin/trust-runtime/bench/execution_backend.rs)
- [x] [crates/trust-runtime/src/bin/trust-runtime/bench/models.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bin/trust-runtime/bench/models.rs)
- [x] [crates/trust-runtime/src/bin/trust-runtime/bench/output.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bin/trust-runtime/bench/output.rs)
- [x] [crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs](/home/johannes/projects/trust-platform/crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs)

### I. CI and script surfaces

These still encode interpreter-oracle policy and must change with the architecture.

- [x] [scripts/runtime_vm_differential_gate.sh](/home/johannes/projects/trust-platform/scripts/runtime_vm_differential_gate.sh)
- [x] [scripts/runtime_vm_determinism_reliability_gate.sh](/home/johannes/projects/trust-platform/scripts/runtime_vm_determinism_reliability_gate.sh)
- [x] [scripts/runtime_vm_bench_gate.sh](/home/johannes/projects/trust-platform/scripts/runtime_vm_bench_gate.sh)
- [x] [scripts/runtime_vm_production_backend_guard.sh](/home/johannes/projects/trust-platform/scripts/runtime_vm_production_backend_guard.sh)
- [x] [.github/workflows/ci.yml](/home/johannes/projects/trust-platform/.github/workflows/ci.yml)
- [x] [.github/workflows/release.yml](/home/johannes/projects/trust-platform/.github/workflows/release.yml)
- [x] [.github/workflows/nightly-reliability.yml](/home/johannes/projects/trust-platform/.github/workflows/nightly-reliability.yml)

### J. Docs, specs, diagrams, and reports

These are currently written for a “VM + interpreter oracle” architecture.

- [x] [docs/guides/RUNTIME_EXECUTION_BACKEND_MIGRATION.md](/home/johannes/projects/trust-platform/docs/guides/RUNTIME_EXECUTION_BACKEND_MIGRATION.md)
- [x] [docs/specs/10-runtime-semantics.md](/home/johannes/projects/trust-platform/docs/specs/10-runtime-semantics.md)
- [x] [docs/specs/README.md](/home/johannes/projects/trust-platform/docs/specs/README.md)
- [x] [docs/internal/runtime-bytecode-vm-spec.md](/home/johannes/projects/trust-platform/docs/internal/runtime-bytecode-vm-spec.md)
- [x] [docs/internal/masterPlan.md](/home/johannes/projects/trust-platform/docs/internal/masterPlan.md)
- [x] [docs/internal/reports/mp-060-runtime-vm-benchmark-corpus.md](/home/johannes/projects/trust-platform/docs/internal/reports/mp-060-runtime-vm-benchmark-corpus.md)
- [x] [docs/diagrams/architecture/runtime-bytecode-vm-execution.puml](/home/johannes/projects/trust-platform/docs/diagrams/architecture/runtime-bytecode-vm-execution.puml)
- [x] [docs/diagrams/architecture/runtime-execution.puml](/home/johannes/projects/trust-platform/docs/diagrams/architecture/runtime-execution.puml)
- [x] [docs/diagrams/debug/debug-architecture.puml](/home/johannes/projects/trust-platform/docs/diagrams/debug/debug-architecture.puml)

## Recommended Removal Sequence

### Phase 1: Split shared model from the executor

- [x] Create a new backend-agnostic model namespace (`program_model`, `model`, or equivalent)
- [x] Move shared POU/program model types out of `eval::types`:
  - `Param`
  - `VarDef`
  - `FunctionDef`
  - `FunctionBlockBase`
  - `FunctionBlockDef`
  - `MethodDef`
  - `ClassDef`
  - `InterfaceDef`
  - `ArgValue`
  - `CallArg`
- [x] Move AST types out of `eval::expr::ast`:
  - `Expr`
  - `LValue`
  - `SizeOfTarget`
- [x] Move statement model out of `eval::stmt` or split model from execution code:
  - `Stmt`
  - `CaseLabel`
  - `StmtResult`
- [x] Move operator contracts together with the AST:
  - `BinaryOp`
  - `UnaryOp`
- [x] Extract shared static-storage naming helpers from `eval::locals`:
  - `static_storage_name()`
  - `method_static_storage_owner()`
- [x] Keep `eval::mod` as a temporary re-export facade only if it reduces risk during the import-path migration
- [x] Retarget `task.rs`, `runtime/core.rs`, `runtime/metadata.rs`, `harness/**`, `bytecode/encoder/**`, and the debug AST/model consumers to the new model namespace

### Phase 2: Build the replacement helper evaluator surface

- [x] Land a bounded helper-evaluator foothold with `helper_eval::const_expr` and use it in `bytecode/encoder/consts.rs`
- [x] Design a small VM-era evaluator/helper API that does not construct `EvalContext`
  - landed as `helper_eval::{const_expr, storage_expr, storage_lvalue}` with bounded storage-native helpers for const-folding, read-only evaluation, and debug writes
- [x] Cover the concrete surviving use cases explicitly:
  - [x] compile-time constant folding in `bytecode/encoder/consts.rs` and `harness/lower/expr/constants.rs`
  - [x] runtime/config/build initializer evaluation in `instance.rs` and `harness/**`
  - [x] debug watch / breakpoint / logpoint expression evaluation
  - [x] debug lvalue writes from `runtime/cycle.rs`
- [x] Decide whether pure lookup helpers such as `target_resolution.rs` move into this new helper layer or are inlined at the call sites
  - kept under the `cfg(test)` executor call path; no production/helper consumer needs it moved

### Phase 3: Rewrite `EvalContext` helper consumers

- [x] Rewrite `runtime/core/evaluation.rs` and remove or replace public `with_eval_context()`
  - `evaluate_expression()` remains on `helper_eval::storage_expr` and `with_eval_context()` has been removed
- [x] Rewrite debug surfaces:
  - read-only watch / breakpoint / logpoint expression evaluation is on `helper_eval::storage_expr`
  - debug lvalue writes in `runtime/cycle.rs` are on `helper_eval::storage_lvalue`
  - `debug/hook.rs`, `debug/control.rs`, `debug/control/hook.rs`, `debug/breakpoints.rs`, and `debug/control/api/watch_stream.rs` now use `DebugRuntimeContext` instead of `EvalContext`
- [x] Rewrite `runtime/cycle.rs` pending debug lvalue writes away from `write_lvalue()`
- [x] Rewrite `bytecode/encoder/consts.rs` away from `eval_expr()`
- [x] Rewrite `instance.rs`, `harness/config*.rs`, and `harness/build.rs` to the new helper evaluator
- [x] Rewrite `test_cmd/execute.rs` so FB tests execute without `with_eval_context()` / `call_function_block()`
- [x] Ensure builtin FB execution only depends on storage-native helpers

### Phase 4: Delete the interpreter backend and collapse the seam

- [x] Remove `ExecutionBackend::Interpreter`
- [x] Remove `InterpreterBackend`
- [x] Remove interpreter functions from `runtime/cycle.rs`
- [x] Remove `legacy-interpreter` feature from `Cargo.toml`
- [x] Demote the remaining executor-specific `eval/` files to `cfg(test)` only
- [x] Collapse `RuntimeExecutionBackend` / `resolve_backend()` if a VM-only runtime no longer needs the dispatch seam

### Phase 5: Rewrite or retire interpreter-oracle evidence

- [x] Replace interpreter-vs-VM differential gates with VM-only behavior-lock and deterministic corpus gates
- [x] Remove or rewrite `bench execution-backend`
- [x] Remove interpreter-only tests or convert them into VM-only behavioral tests
- [x] Rewrite `tests/common/mod.rs` away from direct `EvalContext` construction
  - interpreter-expression tests still use it; debug helper tests now use `DebugRuntimeContext`

### Phase 6: Final doc/spec cleanup

- [x] Remove interpreter-oracle language from runtime docs/specs
- [x] Update diagrams to show one runtime execution architecture
- [x] Decide the final public API fate of `pub mod eval`
- [x] Update master-plan/checklists to reflect completed deletion

## Acceptance Criteria

- [x] `rg -n "legacy-interpreter|ExecutionBackend::Interpreter|InterpreterBackend|execute_program_interpreter|execute_function_block_ref_interpreter" crates/trust-runtime scripts .github docs -g '!target'` returns no live-runtime references outside archived historical MP-060 planning/evidence documents
- [x] `rg -n "crate::eval::|use crate::eval" crates/trust-runtime/src -g '!target'` only matches the compatibility facade or test-only evaluator internals
- [x] no production runtime path constructs `EvalContext`
- [x] no debug helper, config/build helper, or bytecode const-folding path constructs `EvalContext`
- [x] no benchmark, CI, or nightly workflow requires interpreter execution
- [x] diagrams/specs/docs describe a VM-only runtime architecture or explicitly mark archived interpreter-comparison material as historical
- [x] full runtime validation gates pass after deletion
  - validated with `just fmt`, `just clippy`, `just test-all`, targeted `trust-debug` debugger evaluation tests, and a fresh portable syntax-corpus rerun

## Notes

- The current diagram [runtime-bytecode-vm-execution.puml](/home/johannes/projects/trust-platform/docs/diagrams/architecture/runtime-bytecode-vm-execution.puml) is still useful as a top-level architecture view, but it is not sufficient as the removal inventory.
- The hardest part of this effort is not deleting `runtime/cycle.rs` interpreter entry points; it is replacing the surviving evaluator use cases: const-folding, initializer evaluation, debug watch/breakpoint evaluation, and debug lvalue writes.
- The debug subsystem is a first-class consumer in this plan, not an afterthought. The VM already uses storage snapshots for paused-state scopes, but several debug contracts still depend on `EvalContext`.
