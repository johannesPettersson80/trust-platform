# Runtime Legacy Interpreter Removal: Claude Review Brief

Status: Draft
Owner: Runtime team
Branch: `runtime/remove-legacy-interpreter`
Primary artifacts:
- [runtime-remove-legacy-interpreter.md](/home/johannes/projects/trust-platform/docs/internal/testing/checklists/runtime-remove-legacy-interpreter.md)
- [runtime-interpreter-removal-map.puml](/home/johannes/projects/trust-platform/docs/diagrams/architecture/runtime-interpreter-removal-map.puml)
- [runtime-bytecode-vm-execution.puml](/home/johannes/projects/trust-platform/docs/diagrams/architecture/runtime-bytecode-vm-execution.puml)

## Review Goal

We are preparing to remove the legacy interpreter from `trust-runtime` completely.

This review is not asking for a superficial thumbs-up on the existing checklist.
Claude must independently verify the architecture and produce an independent removal map so we can catch anything the first-pass inventory missed.

## What We Already Believe

### High-level conclusion

The legacy interpreter cannot be removed by simply deleting `crates/trust-runtime/src/eval/**`.

The current `eval` namespace appears to mix three different concerns:

1. shared runtime/program model types
2. shared semantics/operator helpers still used by the VM
3. the actual legacy interpreter executor

### Current proposed removal order

1. Extract shared model types out of `eval`
2. Extract shared semantics helpers out of `eval`
3. Replace `EvalContext` helper consumers
4. Delete interpreter backend and executor code
5. Remove interpreter-only tests, benchmark surfaces, CI scripts, and docs/specs

### Current proposed first implementation slice

Do **not** start by deleting `execute_program_interpreter()` / `execute_function_block_ref_interpreter()`.

Start by splitting the shared model out of:
- `crates/trust-runtime/src/eval/types.rs`
- `crates/trust-runtime/src/eval/expr/ast.rs`
- `crates/trust-runtime/src/eval/stmt.rs` (or splitting model from execution inside it)

Then retarget the consumers in:
- `task.rs`
- `runtime/core.rs`
- `runtime/metadata.rs`
- `instance.rs`
- `harness/**`
- `bytecode/encoder/**`

## Evidence We Already Collected

### Direct interpreter backend surface

- `crates/trust-runtime/src/execution_backend.rs`
- `crates/trust-runtime/src/runtime/backend.rs`
- `crates/trust-runtime/src/runtime/cycle.rs`
- `crates/trust-runtime/Cargo.toml` (`legacy-interpreter` feature)

### Important non-interpreter `EvalContext` / evaluator users

- `crates/trust-runtime/src/runtime/core/evaluation.rs`
- `crates/trust-runtime/src/control/debug_handlers_helpers.rs`
- `crates/trust-runtime/src/bin/trust-runtime/test_cmd/execute.rs`
- `crates/trust-runtime/src/instance.rs`
- `crates/trust-runtime/src/harness/config.rs`
- `crates/trust-runtime/src/harness/config/config_inits.rs`
- `crates/trust-runtime/src/harness/config/globals.rs`
- `crates/trust-runtime/src/harness/build.rs`
- `crates/trust-runtime/src/harness/lower/expr.rs`
- `crates/trust-runtime/src/harness/lower/expr/constants.rs`

### Shared model / semantics consumers still importing through `crate::eval`

- `crates/trust-runtime/src/task.rs`
- `crates/trust-runtime/src/runtime/core.rs`
- `crates/trust-runtime/src/runtime/metadata.rs`
- `crates/trust-runtime/src/bytecode/encoder/**`
- `crates/trust-runtime/src/harness/compiler/**`
- `crates/trust-runtime/src/harness/lower/**`
- `crates/trust-runtime/src/harness/parse.rs`
- `crates/trust-runtime/src/harness/io.rs`
- `crates/trust-runtime/src/runtime/vm/dispatch_ops.rs`
- `crates/trust-runtime/src/runtime/vm/register_ir.rs`
- `crates/trust-runtime/src/stdlib/numeric.rs`
- `crates/trust-runtime/src/stdlib/time.rs`

### Builtin FB note

`crates/trust-runtime/src/stdlib/fbs/mod.rs` already has a storage-native execution path:
- `execute_builtin_in_storage(...)`

The thin `execute_builtin(ctx: &mut EvalContext, ...)` wrapper looks removable after the evaluator is gone.

### Interpreter-only test / benchmark / CI blast radius

Representative surfaces:
- `crates/trust-runtime/tests/bytecode_vm_differential.rs`
- `crates/trust-runtime/src/bin/trust-runtime/bench/execution_backend.rs`
- `scripts/runtime_vm_differential_gate.sh`
- `scripts/runtime_vm_determinism_reliability_gate.sh`
- `scripts/runtime_vm_bench_gate.sh`
- `.github/workflows/ci.yml`
- runtime migration/spec docs and MP-060 docs/checklists

## What Claude Must Do Independently

Claude should not rely only on our checklist or diagram. Claude should build an independent map from the current codebase.

### Required independent mapping tasks

1. Re-scan the runtime codebase for all references to:
- `legacy-interpreter`
- `ExecutionBackend::Interpreter`
- `InterpreterBackend`
- `EvalContext`
- `use crate::eval`
- `crate::eval::`
- `execute_program_interpreter`
- `execute_function_block_ref_interpreter`

2. Classify every hit into one of four buckets:
- `delete`
- `extract and keep`
- `rewrite to VM-only helper`
- `test/docs/CI only`

3. Build a separate architecture view of the removal problem:
- what is the actual interpreter executor
- what is actually shared runtime model
- what is shared semantics the VM depends on
- what helper surfaces still use `EvalContext`

4. Challenge the proposed first slice:
- is “extract model first” really the right first move?
- or is there an even smaller safer split?

5. Identify anything likely missing from our first-pass map, especially:
- hidden consumers of shared model types
- hidden `EvalContext` creation sites
- debug/step/watch semantics that become risky after removal
- CI or benchmark surfaces we forgot

## Deliverables Expected From Claude

Claude’s review should produce:

1. Findings first, ordered by severity
- missing dependencies
- wrong assumptions
- sequencing risks
- correctness hazards

2. An independent file/category map
- delete
- extract
- rewrite
- doc/test/CI

3. A recommended removal sequence
- phase-by-phase
- with a clearly identified first implementation slice

4. A verdict on our current plan
- correct
- mostly correct with changes
- wrong in key ways

## Specific Questions For Claude

1. Is our claim correct that `eval/` currently mixes:
- shared model
- shared semantics
- legacy executor
?

2. Is extracting shared model types before executor deletion the right first implementation slice?

3. Which file is the most dangerous hidden dependency in this removal?

4. Which current test or CI surface becomes invalid immediately once the interpreter is removed?

5. Should `RuntimeExecutionBackend` survive in a VM-only runtime, or should it be collapsed?

6. What is the smallest safe milestone that genuinely reduces interpreter footprint without starting a half-refactor?

## Suggested Commands For Claude

These are suggested starting points, not a complete workflow:

```bash
rg -n "legacy-interpreter|ExecutionBackend::Interpreter|InterpreterBackend|EvalContext|use crate::eval|crate::eval::|execute_program_interpreter|execute_function_block_ref_interpreter" crates/trust-runtime/src crates/trust-runtime/tests scripts .github docs -g '!target'
```

```bash
rg -l "use crate::eval|crate::eval::|EvalContext|eval_expr\(" crates/trust-runtime/src -g '!target' | sort
```

```bash
find crates/trust-runtime/src/eval -type f | sort
```

```bash
sed -n '1,260p' crates/trust-runtime/src/eval/types.rs
sed -n '1,260p' crates/trust-runtime/src/eval/expr/ast.rs
sed -n '120,420p' crates/trust-runtime/src/runtime/cycle.rs
sed -n '1,260p' crates/trust-runtime/src/instance.rs
```

## Current Artifacts Produced On This Branch

- checklist: [runtime-remove-legacy-interpreter.md](/home/johannes/projects/trust-platform/docs/internal/testing/checklists/runtime-remove-legacy-interpreter.md)
- focused removal map: [runtime-interpreter-removal-map.puml](/home/johannes/projects/trust-platform/docs/diagrams/architecture/runtime-interpreter-removal-map.puml)
- rendered diagram: [runtime-interpreter-removal-map.svg](/home/johannes/projects/trust-platform/docs/diagrams/generated/runtime-interpreter-removal-map.svg)

## Review Standard

A good review should do more than repeat our checklist.
It should prove that the plan survives an independent codebase walk and should explicitly call out anything we missed or sequenced badly.
