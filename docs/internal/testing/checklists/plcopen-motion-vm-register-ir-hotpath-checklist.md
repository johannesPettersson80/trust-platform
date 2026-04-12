# PLCopen Motion VM Register-IR Hotpath Checklist

Scope: targeted VM-side reconciliation for the PLCopen motion benchmark hotspot. This pass is limited to register-IR eligibility and diagnostics; it must not reshape the ST library surface.

## Test-First Locks

- [x] `T1` Add a failing unit test proving FUNCTION_BLOCK bodies with self-field access no longer lower to `VmFallback` because of `LOAD_SELF (0x23)`.
- [x] `T2` Add a failing unit test proving `PROGRAM Main` with complex local field access can execute through register IR without the `complex_local_ref_path` fallback.
- [x] `T3` Add a failing unit test proving lowering-error fallbacks preserve the failing POU name and the original lowering message.
- [x] `T4` Add a failing bench JSON test proving `trust-runtime bench project` resolves `vm_profile.hot_blocks[*].pou_id` to readable `pou_name` values.

## Implementation

- [x] `I1` Add register-IR lowering/execution support for `LOAD_SELF (0x23)`.
- [x] `I2` Stop rejecting complex `VmRef::Local` paths that the interpreted register executor already supports.
- [x] `I3` Preserve readable lowering-error details in the register-lowering cache fallback path.
- [x] `I4` Resolve readable POU names in project-bench VM profile output.

## Focused Validation

- [x] `V1` Run the new focused unit tests for register lowering/execution and bench JSON output.
- [x] `V2` Re-run `scripts/runtime_motion_benchmark_breakdown.sh` and confirm the `unsupported_opcode_0x23` and `complex_local_ref_path` fallback buckets collapse materially.
- [x] `V3` Re-run `./scripts/runtime_motion_example_bench_gate.sh` and record the new release-profile timing result.

## Follow-Up

- [x] `F1` Fix the remaining `Main` register-IR lowering fallback by modeling block-entry stack state across CASE-heavy control-flow blocks. Validation on the refreshed release artifacts now shows `full_demo` at `p50 524.020 us`, `p95 859.281 us`, `0` overruns, and `0` VM fallbacks, with the canonical gate artifact reporting `p95 847.725 us`, `0` overruns, and clean motion-demo semantics.
