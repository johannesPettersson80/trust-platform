# Runtime VM Performance Follow-Up

Date: 2026-04-12

Purpose: capture the next dedicated runtime-speed work so it does not stay implicit in chat history.

## Current Decision

- PLCopen motion reconciliation is in a good state and should not absorb broad VM architecture work.
- Future runtime-speed work should continue as a separate performance track with benchmark locks and explicit acceptance criteria.
- Do not optimize blindly. Every change must be justified by measured improvement on the locked benchmark set.

## Locked Baseline

Artifacts:
- `target/gate-artifacts/runtime-motion-example-bench/motion-example-bench.json`
- `target/gate-artifacts/runtime-motion-benchmark-breakdown/summary.md`

Current release numbers on the motion demo:
- cycle budget: `10_000 us`
- `p50`: `527.632 us`
- `p95`: `847.725 us`
- `p99`: `1060.337 us`
- `max`: `1099.782 us`
- overruns: `0`
- semantics: `22` completed sequences, `last_error = 0`, clamp verified, powered, homed

Current breakdown highlights:
- `runtime_floor`: `p50 2.074 us`
- `constants_only`: `p50 14.315 us`
- `status_only`: `p50 201.612 us`, `0` fallbacks
- `command_idle`: `p50 415.242 us`, `0` fallbacks
- `move_absolute_only`: `p50 245.112 us`, `0` fallbacks
- `full_demo`: `p50 524.020 us`, `p95 859.281 us`, `0` fallbacks

## Validation Already Completed

### Targeted Runtime Tests

Executed during the CASE/register-IR hotpath fix:

- `cargo test -p trust-runtime --lib register_ir_lowering_handles_case_selector_live_across_branch_blocks -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_runs_case_program_without_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_runs_multi_label_case_program_without_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_runs_case_branch_with_nested_if_without_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_progresses_motion_demo_to_step_40_without_error_by_cycle_three -- --nocapture`
- `cargo test -p trust-runtime --lib register_`
- `cargo test -p trust-runtime --bin trust-runtime project_bench_json_output_contains_budget_and_watched_globals`

### Example And Motion Validation Commands

Executed on the stacked worktree to validate the example/library path and runtime bench path:

- `cargo run -p trust-runtime --bin trust-runtime -- build --project examples/plcopen_motion_single_axis_demo --sources src`
- `cargo run -p trust-runtime --bin trust-runtime -- validate --project examples/plcopen_motion_single_axis_demo`
- `./scripts/runtime_motion_example_bench_gate.sh`
- `./scripts/runtime_motion_benchmark_breakdown.sh`

### Primary Verification Scripts

Use these as the canonical verification entrypoints for the motion/runtime performance work:

- `./scripts/runtime_motion_example_bench_gate.sh`
  - end-to-end semantic + cycle-budget gate for `examples/plcopen_motion_single_axis_demo`
  - writes artifacts under `target/gate-artifacts/runtime-motion-example-bench/`
- `./scripts/runtime_motion_benchmark_breakdown.sh`
  - breakdown benchmark across `runtime_floor`, `constants_only`, `status_only`, `command_idle`, `move_absolute_only`, `full_demo_constants_once`, and `full_demo`
  - writes artifacts under `target/gate-artifacts/runtime-motion-benchmark-breakdown/`

### Key Artifact Files

- `target/gate-artifacts/runtime-motion-example-bench/motion-example-bench.json`
- `target/gate-artifacts/runtime-motion-example-bench/summary.md`
- `target/gate-artifacts/runtime-motion-benchmark-breakdown/summary.md`

### Current Verified State

- release motion gate: PASS
- motion demo semantics: PASS
- `vm_profile.register_program_fallbacks == 0` on the refreshed motion demo and breakdown workloads
- remaining known work is performance follow-up, not an open correctness blocker for the current motion demo

## Rules For The Next Pass

- Tests and benchmarks first for every performance fix.
- Keep the PLCopen ST library shape stable unless a benchmark-driven change is clearly justified.
- Treat correctness regressions as stop-ship issues, even if performance improves.
- Do not mix broad VM optimization with unrelated feature work.

## Priority Order

### P0. Benchmark Locks And Evidence

- [ ] Add a dedicated runtime-performance checklist for each optimization pass with explicit test-first steps.
- [ ] Preserve the motion benchmark breakdown as a locked reference workload.
- [ ] Add a compact benchmark report that compares before/after for `runtime_floor`, `status_only`, `command_idle`, and `full_demo`.
- [ ] Require no fallback regressions in `vm_profile` on the locked motion workloads.

### P1. Global Access Hot Path

Goal: reduce the cost of global reads/writes that dominate realistic ST helper functions.

- [ ] Profile global access in the register VM with focused counters around global load/store/ref operations.
- [ ] Test whether name-based global lookup is still on the hot path during register execution.
- [ ] Prototype index/slot-based global access for hot runtime paths.
- [ ] Re-run the locked motion workloads and record deltas.

### P2. Register-VM Load/Store/Ref Specialization

Goal: speed up the common reference and assignment patterns that dominate PLC workloads.

- [ ] Add focused tests for hot load/store/ref instruction families before optimizing them.
- [ ] Extend the register fast path for the most common global/ref operations seen in the motion profile.
- [ ] Re-measure `status_only`, `command_idle`, and `full_demo` after each specialization step.

### P3. FUNCTION_BLOCK Call/Frame Overhead

Goal: reduce per-scan cost for FB-heavy programs.

- [ ] Add benchmark coverage that isolates FB call overhead from body work.
- [ ] Measure frame setup/teardown cost for hot FB-heavy scans.
- [ ] Prototype a lower-overhead hot-call path only if measurements justify it.

### P4. Runtime Value/Storage Representation

Goal: evaluate whether scalar-heavy workloads are paying avoidable cloning/boxing costs.

- [ ] Measure value-clone/copy cost in the current register executor.
- [ ] Identify whether scalar-heavy code would materially benefit from representation changes.
- [ ] Only start this track if P1-P3 leave clear performance headroom on the table.

## Acceptance Criteria For Any Performance Pass

- [ ] Targeted correctness tests pass.
- [ ] Motion demo bench semantics remain clean.
- [ ] `vm_profile.register_program_fallbacks == 0` on the locked motion workloads.
- [ ] Before/after benchmark evidence is recorded in an artifact or checklist entry.
- [ ] Optimization direction is justified by measured data, not intuition alone.

## Recommended Next Step

Start with `P1`.

Reason:
- it should benefit all ST workloads, not just PLCopen motion
- the current profiling harness can measure it directly
- it is less invasive than changing the whole value representation or call model
