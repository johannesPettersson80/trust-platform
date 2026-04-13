# Runtime VM Performance Follow-Up

Date: 2026-04-12

Purpose: capture the next dedicated runtime-speed work so it does not stay implicit in chat history.

## Current Decision

- PLCopen motion reconciliation is in a good state and should not absorb broad VM architecture work.
- Future runtime-speed work should continue as a separate performance track with benchmark locks and explicit acceptance criteria.
- Do not optimize blindly. Every change must be justified by measured improvement on the locked benchmark set.
- `P1` is now complete enough to choose the next optimization target: start with instance-field/ref binding specialization, not broad frame churn or `Value` representation work.
- The first `P2` step is now in flight as a safe per-instance field-resolution cache in `VariableStorage`; a pure `type_name` cache was rejected because builtin FBs lazily materialize hidden state and same-type instances can temporarily have different layouts.
- Early `P2` ref-cache work stayed mixed, but the later direct-slot/output-target fast path is now accepted after the benchmark script was hardened to rebuild a fresh release binary before measurement.
- Current accepted `P2` state: keep the binder-side declared-parameter direct-offset fast path, the direct-slot `VmWriteTarget` / storage empty-path fast path, the shared direct-read helper for `IN` target arguments, and the borrowed dynamic-ref peek path in `dispatch_refs.rs` for `REF_FIELD`/`REF_INDEX` shape inspection; do not keep the broader self-field `REF_FIELD` fast path on the active runtime path because that wider experiment increased jitter without a reliable median win.
- Current accepted `P3` state: keep the new `trivial_fb_no_params` isolate in the benchmark pack, skip VM FUNCTION_BLOCK field resolution when `OUT`/`IN_OUT` parameters are omitted, keep the ordered named-argument fast path in the hot binder loops, and keep the pop-order native-call payload decode helper in `call.rs` instead of reversing/removing a payload vector on every call.
- Current accepted `P4` state: keep the new VM `value_ops` profiling counters in the bench/reporting path, the narrower borrowed-read follow-up for integer output type inspection in `call.rs`, the fused ref/const DINT guards in `register_ir.rs`, and the targeted `Value::Struct` shared-on-clone / copy-on-write representation. The retained struct-sharing pass removed the remaining `AXIS_REF`-style `VAR_IN_OUT` deep-clone hot path: on the rebuilt motion gate `read_value_clones` fell from `6467` to `227` and `output_value_clones` fell from `4160` to `0`, while end-to-end latency improved to `p50 424.187 us` / `p95 649.225 us`. Do not broaden this branch into `Array`/`Enum`/`Reference` representation churn yet; the next step-change is no longer justified by the current evidence.
- Current bounded `P5` result: keep the new project-bench tier-1 visibility (`vm_profile.tier1_specialized_executor`) and the tier-1 compiler support for `RefField` / `RefIndex` / `LoadDynamic` / `StoreDynamic`, but do not treat this pass as the next accepted speed baseline. The first rebuilt motion rerun with tier-1 enabled in the bench path landed at `p50 471.187 us` / `p95 582.650 us`; a final clean rerun with no leftover cargo/benchmark processes still landed at `p50 469.188 us` / `p95 967.690 us`, and the matching clean rebuilt breakdown reports `full_demo = 468.706 us`. The pass is diagnostically useful, not a clear speed win: tier-1 snapshot evidence shows `compile_attempts = 6967`, `compile_successes = 1705`, `compile_failures = 5262`, `block_executions = 5879`, and `deopt_count = 1657` (`binary_non_dint_guard`). Stop here for this branch unless a later dedicated pass targets the remaining unsupported/deopt-heavy tier-1 surface deliberately.
- Tier-1 misses are now attributable in the standard bench artifacts instead of hiding behind aggregate counts. On the latest rebuilt motion breakdown, `full_demo` compile failures split into `unsupported_instr:call_native = 3283`, `unsupported_instr:load_ref_addr = 1854`, `unsupported_binary_op:and = 97`, and `unsupported_binary_op:or = 28`; deopts remain entirely `binary_non_dint_guard = 1657`. That is the concrete fix queue if a later branch resumes tier-1 work.
- Current bounded `P6` result: the remaining tier-1 fix queue is now closed on the motion benchmark path. After adding tier-1 support for `LoadRefAddr`, widening binary specialization to the generic non-DINT fallback path, and compiling `CallNative`, the latest clean rebuilt breakdown reports `compile_failures = 0` and `deopts = 0` across the locked workloads, with `full_demo = 487.058 us` / `p95 919.059 us`; the matching clean rebuilt gate reports `p50 475.373 us`, `p95 554.503 us`, `p99 587.892 us`, `max 597.391 us`, `0` overruns, and clean semantics. Keep this pass as a completeness/coverage result, not the next accepted speed baseline, because it closes the remaining tier-1 gaps but does not beat the earlier accepted `full_demo` median around `424 us`.
- Current bounded `P7` result: keep the borrowed non-DINT ref-binary fallback rewrite in both the interpreted register executor and the tier-1 executor. The new path keeps the DINT fast guard, but on guard miss it materializes the already-borrowed ref/const values once instead of rereading them through `load_ref()`. The targeted behavior locks now prove `read_value_clones = 0` and `const_load_clones = 0` for BOOL ref/ref and ref/const binaries, and the rebuilt motion breakdown improves versus `P6` on the ref-heavy path (`dynamic_refs = 34.834 us`, `full_demo = 478.836 us` / `p95 931.412 us`) while the matching clean rebuilt gate improves its median to `p50 468.706 us` with `0` overruns. Keep it as a small correctness + throughput cleanup on top of `P6`, not as the next accepted speed baseline, because it still does not beat the retained `P4` median around `424 us`.
- Treat older pre-hardening breakdown-only deltas as provisional. Gate runs were always safe because `runtime_motion_example_bench_gate.sh` rebuilds through `cargo run --release`.
- Next step after restoring the default non-tier1 benchmark path: move from a motion-only benchmark mindset to a supported-syntax/runtime-shape corpus. The planning matrix lives in `docs/internal/testing/checklists/runtime-vm-supported-syntax-performance-matrix.md` and keeps the default Pi suite under a strict size/time budget.

## Locked Baseline

Artifacts:
- `target/gate-artifacts/runtime-motion-example-bench/motion-example-bench.json`
- `target/gate-artifacts/runtime-motion-benchmark-breakdown/summary.md`
- `target/gate-artifacts/runtime-motion-benchmark-breakdown/summary.json`
- `target/gate-artifacts/runtime-vm-p3-spotcheck/summary.md`

Current release numbers on the motion demo gate:
- cycle budget: `10_000 us`
- current accepted end-to-end gate: PASS
- current accepted gate latency: `p50 424.187 us`, `p95 649.225 us`, `p99 739.633 us`, `max 1181.098 us`, `0` overruns
- current bounded P5 diagnostic gate (tier-1 enabled in `bench project`): initial rerun `p50 471.187 us`, `p95 582.650 us`, `p99 642.558 us`, `max 661.003 us`; final clean rerun with no leftover cargo/benchmark processes `p50 469.188 us`, `p95 967.690 us`, `p99 1097.543 us`, `max 1105.506 us`, always `0` overruns
- current bounded P6 completeness gate (tier-1 compile/deopt surface closed): clean rerun `p50 475.373 us`, `p95 554.503 us`, `p99 587.892 us`, `max 597.391 us`, `0` overruns
- current bounded P7 borrowed-ref cleanup gate: clean rebuilt rerun `p50 468.706 us`, `p95 985.134 us`, `p99 1062.968 us`, `max 1103.246 us`, `0` overruns
- accepted semantic state: `22` completed sequences, `last_error = 0`, clamp verified, powered, homed
- note: on this host the gate tail metrics are noisier than the breakdown medians, so keep acceptance decisions anchored on fresh rebuilt breakdown medians and use the gate primarily as semantic + budget guard evidence

Current breakdown highlights on the latest rebuilt release matrix:
- `runtime_floor`: `p50 1.945 us`
- `scalar_globals_only`: `p50 17.944 us`
- `dynamic_refs`: `p50 30.797 us`
- `trivial_fb_no_params`: `p50 25.259 us`
- `trivial_fb_low_params`: `p50 40.666 us`
- `trivial_fb_high_params`: `p50 78.297 us`
- `constants_only`: `p50 11.926 us`
- `status_only`: `p50 152.205 us`, `0` fallbacks
- `command_idle`: `p50 345.038 us`, `0` fallbacks
- `move_absolute_only`: `p50 158.279 us`, `0` fallbacks
- `full_demo_constants_once`: `p50 423.483 us`, `0` fallbacks
- `full_demo`: `p50 424.947 us`, `p95 732.818 us`, `0` fallbacks
- bounded `P5` diagnostic breakdown: initial rerun `full_demo` `p50 453.354 us`, `p95 521.761 us`, `0` fallbacks; final clean rerun `full_demo` `p50 468.706 us`, `p95 802.727 us`, `0` fallbacks. Tier-1 snapshot on the gate reports `cached_blocks = 48`, `compile_successes = 1705`, `compile_failures = 5262`, `block_executions = 5879`, `deopt_count = 1657`; the refreshed breakdown now attributes those misses to concrete buckets (`call_native`, `load_ref_addr`, `binary and/or`, and `binary_non_dint_guard`)
- bounded `P6` completeness breakdown: clean rerun `full_demo` `p50 487.058 us`, `p95 919.059 us`, `0` fallbacks. Tier-1 snapshot now reports `compile_attempts = 100`, `compile_successes = 100`, `compile_failures = 0`, `executions = 12798`, and `deopts = 0` on the same benchmark matrix
- bounded `P7` borrowed-ref cleanup breakdown: clean rebuilt rerun `dynamic_refs` `p50 34.834 us`; `full_demo` `p50 478.836 us`, `p95 931.412 us`, `0` fallbacks; VM `value_ops` now stay at `const_load_clones = 0` / `read_value_clones = 0` on the new non-DINT borrowed-ref behavior locks

Current P1 conclusion:
- Direct globals are cheap.
- One dynamic-ref/self-field-heavy FB already costs about as much as a low-parameter trivial FB pack.
- Keeping the same trivial call count but raising parameter volume materially increases cost.
- The next runtime pass should target instance-field lookup and parameter-driven ref work first.

## Validation Already Completed

### Final Full Gates

Executed on the final retained tree for this pass:

- `just fmt`
- `just clippy`
- `just test-all`
- `cargo test -p trust-runtime --test api_smoke`
- `cargo test -p trust-runtime --test debug_control`
- `cargo test -p trust-runtime --test complete_program`
- `cargo test -p trust-runtime --test runtime_reliability`

### Targeted Runtime Tests

Executed during the CASE/register-IR hotpath fix and the P1 instrumentation pass:

- `cargo test -p trust-runtime --lib register_ir_lowering_handles_case_selector_live_across_branch_blocks -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_runs_case_program_without_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_runs_multi_label_case_program_without_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_runs_case_branch_with_nested_if_without_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_progresses_motion_demo_to_step_40_without_error_by_cycle_three -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_profile_records_ref_op_counters_for_load_ref_store_ref_program -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_profile_records_dynamic_ref_and_instance_lookup_counters -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_profile_records_function_block_call_counters -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_profile_avoids_clone_counters_for_struct_inout_function_block -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_profile_avoids_clone_counters_for_borrowed_ref_ref_non_dint_binary -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_non_dint_binary -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_tier1_specialized_executor_executes_bool_or_without_deopt -- --nocapture`
- `cargo test -p trust-runtime --lib write_by_ref_path_preserves_struct_copy_on_write_isolation -- --nocapture`
- `cargo test -p trust-runtime --lib register_`
- `cargo test -p trust-runtime --lib instance_field_cache_is_scoped_per_instance -- --nocapture`
- `cargo test -p trust-runtime --lib borrowed_value_ref_helpers_match_owned_helpers -- --nocapture`
- `cargo test -p trust-runtime --lib declared_instance_field_offset_reuses_type_layout_for_declared_fields -- --nocapture`
- `cargo test -p trust-runtime --lib declared_instance_field_offset_skips_inherited_fields -- --nocapture`
- `cargo test -p trust-runtime --lib direct_instance_field_offset_reads_and_writes_without_value_ref -- --nocapture`
- `cargo test -p trust-runtime --lib resolved_instance_field_ref_prefers_direct_field_before_parent_fallback -- --nocapture`
- `cargo test -p trust-runtime --lib direct_instance_field_miss_cache_invalidates_on_new_insert -- --nocapture`
- `cargo test -p trust-runtime --lib recursive_instance_field_cache_invalidates_when_child_adds_shadowing_field -- --nocapture`
- `cargo test -p trust-runtime --lib recursive_lookup_does_not_cache_parent_chain_miss -- --nocapture`
- `cargo test -p trust-runtime --bin trust-runtime project_bench_json_output_contains_budget_and_watched_globals`
- `cargo test -p trust-runtime --bin trust-runtime project_bench_table_output_contains_tier1_executor_stats -- --nocapture`
- `cargo test -p trust-runtime --bin trust-runtime project_bench_output_contains_tier1_compile_failure_reasons -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_tier1_specialized_executor_records_compile_failure_reason_for_unsupported_instruction -- --nocapture`
- `cargo test -p trust-runtime --lib tier1_compiler_accepts_function_block_ -- --nocapture`
- `cargo test -p trust-runtime --lib tier1_compiler_accepts_call_native_function_blocks -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_tier1_specialized_executor_executes_array_ref_blocks -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_tier1_specialized_executor_executes_function_call_block -- --nocapture`
- `cargo test -p trust-runtime --lib register_executor_tier1_specialized_executor_executes_function_block_call_block -- --nocapture`
- `cargo test -p trust-runtime --test expr_access -- --nocapture`
- `cargo test -p trust-runtime --test retain_store -- --nocapture`
- `cargo test -p trust-runtime --lib peek_dynamic_ref_borrows_global_storage_value -- --nocapture`
- `cargo test -p trust-runtime --lib peek_dynamic_ref_borrows_local_sentinel_value -- --nocapture`
- `cargo test -p trust-runtime --lib dynamic_ref_field_resolves_instance_field_reference -- --nocapture`
- `cargo test -p trust-runtime --lib dynamic_ref_index_extends_partial_index_against_array_shape -- --nocapture`
- `cargo test -p trust-runtime --lib bind_vm_function_block_arguments_skips_ -- --nocapture`
- `cargo test -p trust-runtime --lib resolve_named_arg_index_ -- --nocapture`
- `cargo test -p trust-runtime --lib unpack_native_call_payload_preserves_receiver_and_argument_order -- --nocapture`

### Example And Motion Validation Commands

Executed to validate the example/library path and the synthetic `P1` workloads:

- `cargo run -p trust-runtime --bin trust-runtime -- build --project examples/plcopen_motion_single_axis_demo --sources src`
- `cargo run -p trust-runtime --bin trust-runtime -- validate --project examples/plcopen_motion_single_axis_demo`
- `target/release/trust-runtime build --project examples/plcopen_motion_single_axis_benchmarks/scalar_globals_only --sources src`
- `target/release/trust-runtime validate --project examples/plcopen_motion_single_axis_benchmarks/scalar_globals_only`
- `target/release/trust-runtime build --project examples/plcopen_motion_single_axis_benchmarks/dynamic_refs --sources src`
- `target/release/trust-runtime validate --project examples/plcopen_motion_single_axis_benchmarks/dynamic_refs`
- `target/release/trust-runtime build --project examples/plcopen_motion_single_axis_benchmarks/trivial_fb_low_params --sources src`
- `target/release/trust-runtime validate --project examples/plcopen_motion_single_axis_benchmarks/trivial_fb_low_params`
- `target/release/trust-runtime build --project examples/plcopen_motion_single_axis_benchmarks/trivial_fb_high_params --sources src`
- `target/release/trust-runtime validate --project examples/plcopen_motion_single_axis_benchmarks/trivial_fb_high_params`
- `target/release/trust-runtime bench project --project examples/plcopen_motion_single_axis_benchmarks/dynamic_refs --samples 8 --warmup-cycles 4 --watch g_motion_bench_cycles --watch g_motion_bench_completed_sequences --watch g_motion_bench_last_error --watch g_motion_bench_power_on --watch g_motion_bench_is_homed --output table`
- `./scripts/runtime_motion_example_bench_gate.sh`
- `./scripts/runtime_motion_benchmark_breakdown.sh`
- Benchmark scripts now honor `TRUST_RUNTIME_HOST_CODEGEN=auto|generic|native`; pin `generic` for portable baseline comparisons and reserve `native` for host-local tuning runs.
  - rerun after the first P1 instrumentation slice so the generated JSON/summary artifacts include `vm_profile.ref_ops` and `vm_profile.call_ops`
  - rerun again after adding `M2`/`M3`/`M4` so the synthetic isolates and motion workloads share the same summary surface

### Primary Verification Scripts

Use these as the canonical verification entrypoints for the motion/runtime performance work:

- `./scripts/runtime_motion_example_bench_gate.sh`
  - end-to-end semantic + cycle-budget gate for `examples/plcopen_motion_single_axis_demo`
  - writes artifacts under `target/gate-artifacts/runtime-motion-example-bench/`
- `./scripts/runtime_motion_benchmark_breakdown.sh`
  - breakdown benchmark across `runtime_floor`, `scalar_globals_only`, `dynamic_refs`, `trivial_fb_low_params`, `trivial_fb_high_params`, `constants_only`, `status_only`, `command_idle`, `move_absolute_only`, `full_demo_constants_once`, and `full_demo`
  - writes artifacts under `target/gate-artifacts/runtime-motion-benchmark-breakdown/`
  - now forces a fresh `cargo build --release -p trust-runtime --bin trust-runtime` before benchmarking so the release binary cannot go stale
  - now also summarizes `tier1_specialized_executor.compile_failure_reasons` and `deopt_reasons`, so unsupported tier-1 buckets are attributable directly from `summary.md`

### Key Artifact Files

- `target/gate-artifacts/runtime-motion-example-bench/motion-example-bench.json`
- `target/gate-artifacts/runtime-motion-example-bench/summary.md`
- `target/gate-artifacts/runtime-motion-benchmark-breakdown/summary.md`
- `target/gate-artifacts/runtime-motion-benchmark-breakdown/summary.json`
- `target/gate-artifacts/runtime-vm-p3-spotcheck/summary.md`

### Current Verified State

- release motion gate: PASS
- motion demo semantics: PASS
- `vm_profile.register_program_fallbacks == 0` on the refreshed motion demo and breakdown workloads
- latest retained motion gate value attribution: `const_load_clones = 227`, `read_value_clones = 227`, `binding_expr_clones = 0`, `output_value_clones = 0`
- `trust-runtime bench project` JSON/table output now includes `vm_profile.ref_ops` and `vm_profile.call_ops` for attribution runs
- `P1` synthetic isolates are now in the benchmark pack and validated on the same release path
- remaining known work is performance follow-up, not an open correctness blocker for the current motion demo
- latest clean motion breakdown and gate now report `vm_profile.tier1_specialized_executor.compile_failures == 0` and `deopt_count == 0` on the locked workloads

## Rules For The Next Pass

- Tests and benchmarks first for every performance fix.
- Keep the PLCopen ST library shape stable unless a benchmark-driven change is clearly justified.
- Treat correctness regressions as stop-ship issues, even if performance improves.
- Do not mix broad VM optimization with unrelated feature work.

## Priority Order

### P0. Benchmark Locks And Evidence

- [x] Add a dedicated runtime-performance checklist for each optimization pass with explicit test-first steps.
- [x] Preserve the motion benchmark breakdown as a locked reference workload.
- [x] Add a compact benchmark report that compares before/after for `runtime_floor`, `status_only`, `command_idle`, and `full_demo`.
- [x] Require no fallback regressions in `vm_profile` on the locked motion workloads.

### P1. Ref Access Hot Path Investigation

Goal: measure the remaining ref/call hot path accurately before choosing the
next optimization pass.

Notes:
- Steady-state globals are already offset-based in `VariableStorage`; the next
  investigation pass should not assume a per-access string lookup bottleneck
  for direct globals. Instance-field resolution via `ref_for_instance_recursive`
  remains string-keyed; see `G1a` in the detailed P1 plan.
- The concrete plan for this pass lives in
  `docs/internal/testing/checklists/runtime-vm-performance-p1-investigation-plan.md`.

- [x] Profile ref and call activity in the register VM with focused counters
  around `load/store/ref` operations, dynamic ref traversal, and FB/frame work.
- [x] Confirm whether the remaining cost is dominated by generic ref helpers,
  FB call/binding overhead, or tier-1 coverage limits.
- [x] Re-run the locked motion workloads and record deltas before selecting the
  next implementation track.

### P2. Instance-Field And Ref-Path Specialization

Goal: speed up the common instance-field and reference patterns that dominate
PLC workloads.

- [x] Add focused tests for hot instance-field lookup and parameter-binding ref paths before optimizing them.
- [x] Add a safe per-instance field-resolution cache in `VariableStorage` with invalidation on new instance-field insert; do not use a pure `type_name` cache because builtin FBs can lazily add hidden state and temporarily diverge same-type layouts.
- [x] Prototype borrowed `ValueRef` helpers in the hot read/write path to remove clone churn where the current access layer allows it.
- [x] Add a binder-side declared-parameter direct-offset fast path with recursive fallback for inherited or unusual fields.
- [x] Re-measure `dynamic_refs`, `trivial_fb_low_params`, `trivial_fb_high_params`, `status_only`, `command_idle`, and `full_demo` after each step.
  - Accepted result: keep the binder-side declared-parameter fast path, the direct-slot/output-target fast path, and the shared direct-read helper for `IN` target refs. On the latest fresh rebuilt release gate the current accepted state reports `p50 = 505.595 us`, `p95 = 618.484 us`, `p99 = 696.911 us`, `0` overruns; the matching rebuilt breakdown reports `dynamic_refs = 38.427 us`, `trivial_fb_low_params = 37.723 us`, `trivial_fb_high_params = 72.926 us`, and `full_demo = 517.428 us` with `0` fallbacks. This is a small keep, not a step-change.
  - Rejected result: the follow-on self-field `REF_FIELD` fast path was reverted because it raised end-to-end gate jitter and did not show a reliable additional win on safe gate runs.
  - Rejected result: a follow-on experiment that stored pre-resolved `VmWriteTarget` values directly inside `VmNativeArgValue::Target` regressed the rebuilt end-to-end gate on two consecutive runs (`p50 582.984 us / p95 1112.190 us` and `p50 590.096 us / p95 1021.505 us`) and was reverted. Post-revert confirmation returned `p50 516.817 us`, `p95 793.874 us`, `0` overruns.
- [x] Reduce clone-heavy generic ref reads/writes further only if the next targeted pass shows a measurable win.
  - Accepted result: `dispatch_refs.rs` now uses a borrowed `peek_dynamic_ref()` path for `dynamic_ref_field()` / `dynamic_ref_index()` shape inspection, so those paths stop cloning `Value`s just to inspect struct/array/instance shape. Fresh rebuilt breakdown medians improved materially versus the earlier accepted state: `dynamic_refs 38.427 -> 29.612 us`, `status_only 195.205 -> 158.908 us`, `command_idle 452.187 -> 351.428 us`, and `full_demo 517.428 -> 433.984 us`, all with `0` fallbacks and clean motion semantics.
  - Rejected result: a follow-on `call.rs` named-argument matcher that replaced the per-parameter `args.iter().position(...)` scan regressed the rebuilt breakdown on the hot high-parameter / full-demo paths (`trivial_fb_high_params 73.297 -> 86.686 us`, `full_demo 433.984 -> 457.224 us`) and was reverted.

### P3. FUNCTION_BLOCK Call/Frame Overhead

Goal: reduce per-scan cost for FB-heavy programs if ref-path work is no longer the clear limiter.

- [x] Add benchmark coverage that isolates FB call overhead from body work.
  - Accepted result: `trivial_fb_no_params` is now part of `examples/plcopen_motion_single_axis_benchmarks/` and the breakdown runner, so pure FB call/frame cost plus one output copy-back can be compared directly against the parameter-heavy variants.
- [x] Measure frame setup/teardown cost for hot FB-heavy scans after the ref-path pass.
  - Accepted result: fresh rebuilt breakdown runs put `trivial_fb_no_params` at about `24.3 us`, `trivial_fb_low_params` at about `37.8 us`, and repeated direct spot checks keep `trivial_fb_high_params` in the low-`70 us` range after warm runs. That confirms the remaining cost is not just bare frame churn; argument handling still matters materially.
- [x] Prototype a lower-overhead hot-call path only if measurements justify it.
  - Accepted result: keep the VM FUNCTION_BLOCK binder skip for omitted `OUT`/`IN_OUT` fields, the ordered named-argument fast path, and the pop-order native-call payload decode helper. On the latest rebuilt release matrix the hot call-heavy isolates land at `trivial_fb_no_params = 28.482 us`, `trivial_fb_low_params = 46.278 us`, `trivial_fb_high_params = 79.353 us`, and the end-to-end gate still passes at `p50 = 445.521 us`, `p95 = 662.688 us`, `0` overruns, with clean motion semantics. Evidence: `target/gate-artifacts/runtime-vm-p3-spotcheck/summary.md` plus the refreshed motion gate artifact.

### P5. Tier-1 Ref-Op Coverage (bounded pass)

Goal: expose tier-1 executor evidence in `bench project`, extend the tier-1 compiler to the remaining ref-heavy instruction family requested for this pass, rerun the motion gate/breakdown once, and stop regardless of outcome.

- [x] Expose `vm_profile.tier1_specialized_executor` in `trust-runtime bench project` JSON/table output and enable the experimental tier-1 executor for VM-profiled project benchmark runs.
- [x] Add behavior-lock tests for tier-1 compilation/execution of `RefField` / `RefIndex` / `LoadDynamic` / `StoreDynamic`.
- [x] Add tier-1 compiler/executor support for `RefField` / `RefIndex` / `LoadDynamic` / `StoreDynamic`.
- [x] Rebuild and rerun the same motion gate + breakdown, then stop there.
  - Result: diagnostic success, not a new speed win. Initial rebuilt gate with tier-1 enabled in the project bench path: `p50 471.187 us`, `p95 582.650 us`, `p99 642.558 us`, `max 661.003 us`, `0` overruns, clean motion semantics. Final clean rerun with no leftover cargo/benchmark processes: gate `p50 469.188 us`, `p95 967.690 us`, `p99 1097.543 us`, `max 1105.506 us`, `0` overruns; rebuilt breakdown `dynamic_refs 38.445 us`, `status_only 160.853 us`, `command_idle 370.317 us`, `move_absolute_only 179.057 us`, `full_demo 468.706 us`, all with `0` fallbacks. Tier-1 evidence from the gate stayed stable: `cached_blocks 48`, `compile_attempts 6967`, `compile_successes 1705`, `compile_failures 5262`, `block_executions 5879`, `deopt_count 1657`, `deopt_reasons = [binary_non_dint_guard]`. Keep the visibility/tests/support from this pass, but do not treat it as the next accepted performance baseline.

### P4. Runtime Value/Storage Representation

Goal: evaluate whether scalar-heavy workloads are paying avoidable cloning/boxing costs.

- [x] Measure value-clone/copy cost in the current register executor after the ref-path pass.
  - Accepted result: `trust-runtime bench project` now exposes `vm_profile.value_ops` so the locked motion workloads report clone/move attribution for const loads, register reads, read-side value clones, binding expr clones, and output copy-back clones.
- [x] Identify whether scalar-heavy code would materially benefit from representation changes.
  - Accepted result: the borrowed-read pass showed the register file was already avoiding most clone churn (`register_read_moves = 225143` vs `register_read_clones = 1407`) but left composite `VAR_IN_OUT` transfers as the dominant remaining cost. The retained follow-up changed `Value::Struct` to shared-on-clone with copy-on-write mutation, which removed the remaining `AXIS_REF` deep-clone path without widening the change to arrays/enums/references. Fresh rebuilt evidence on the retained tree: motion gate `p50 424.187 us`, `p95 649.225 us`, `0` overruns; `full_demo` breakdown `p50 424.947 us`; `status_only 152.205 us`; `command_idle 345.038 us`; `move_absolute_only 158.279 us`; `read_value_clones 6467 -> 227`; `output_value_clones 4160 -> 0`. Keep the struct-sharing change, but do not broaden this branch into a full `Value` representation rewrite yet.
- [x] Only start this track if P2-P3 leave clear performance headroom on the table.
  - Decision: complete the measurement pass, but do not start a `Value` representation rewrite from this branch. The runtime is already comfortably inside the `10 ms` budget, and the counters say the remaining cost surface is narrower than a full representation change.

## Acceptance Criteria For Any Performance Pass

- [x] Targeted correctness tests pass.
- [x] Motion demo bench semantics remain clean.
- [x] `vm_profile.register_program_fallbacks == 0` on the locked motion workloads.
- [x] Before/after benchmark evidence is recorded in an artifact or checklist entry.
- [x] Optimization direction is justified by measured data, not intuition alone.

## Recommended Next Step

If runtime-speed work continues after this branch, start with a narrower read-side clone reduction pass rather than a `Value` representation rewrite.

Reason:
- `P4` showed the register executor already consumes most values by move (`register_read_moves` dwarf `register_read_clones` on the locked workloads)
- the dominant remaining clone pressure is read-side value materialization (`read_value_clones`) plus constant cloning (`const_load_clones`)
- the current motion gate is already comfortably inside budget (`p50 447.428 us`, `p95 810.077 us`, `0` overruns)
- the next meaningful optimization, if you want more speed, is to reduce read-side clone churn and repeated constant materialization, not to start a broad `Value` storage/representation rewrite blind
