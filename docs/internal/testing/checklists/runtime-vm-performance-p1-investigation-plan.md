# Runtime VM Performance P1 Investigation Plan

Date: 2026-04-12

Purpose: turn the post-`v0.13.0` runtime-speed follow-up into a measured
investigation pass before making another VM optimization change.

Scope: investigation and planning only. Do not change shipped runtime behavior
until the counters/tests below show which hot path actually dominates.

## Current Locked Baseline

Reference checklist:
- `docs/internal/testing/checklists/runtime-vm-performance-follow-up.md`

Locked motion baseline before synthetic isolates:
- `runtime_floor`: `p50 2.074 us`
- `status_only`: `p50 201.612 us`
- `command_idle`: `p50 415.242 us`
- `move_absolute_only`: `p50 245.112 us`
- `full_demo`: `p50 524.020 us`, `p95 859.281 us`
- `vm_profile.register_program_fallbacks == 0` on the refreshed motion demo and
  breakdown workloads

Canonical verification commands:
- `cargo test -p trust-runtime --lib register_`
- `cargo test -p trust-runtime --bin trust-runtime project_bench_json_output_contains_budget_and_watched_globals`
- `./scripts/runtime_motion_example_bench_gate.sh`
- `./scripts/runtime_motion_benchmark_breakdown.sh`

## Ground Truth Confirmed In Code

- [x] `G1` Direct steady-state global access is already offset-based, not
  string-keyed. `VariableStorage::read_by_ref_parts` and
  `write_by_ref_parts` resolve globals by offset via `IndexMap::get_index*`,
  while name lookup only happens when a `ValueRef` is first constructed.
  Evidence:
  - `crates/trust-runtime/src/memory.rs:269`
  - `crates/trust-runtime/src/memory.rs:305`

- [x] `G1a` Instance-field resolution is still string-keyed at runtime.
  `REF_FIELD` on an `Instance` base and FB argument binding both call
  `ref_for_instance_recursive`, which resolves fields through
  `ref_for_map(...).get_index_of(name)` on every lookup.
  Evidence:
  - `crates/trust-runtime/src/memory.rs:278`
  - `crates/trust-runtime/src/memory.rs:442`
  - `crates/trust-runtime/src/runtime/vm/register_ir.rs:2594`
  - `crates/trust-runtime/src/runtime/vm/call.rs:466`

- [x] `G2` The generic ref helpers still clone values on reads and copy ref-path
  data on address/reference construction.
  Evidence:
  - `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs:13`
  - `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs:46`
  - `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs:196`

- [x] `G3` The current tier-1 executor can compile `LoadSelf`, but it still
  rejects the ref-heavy instruction families that dominate real PLC FB bodies:
  `LoadRefAddr`, `RefField`, `RefIndex`, `LoadDynamic`, `StoreDynamic`, and
  `CallNative`.
  Evidence:
  - `crates/trust-runtime/src/runtime/vm/register_ir.rs:2093`
  - `crates/trust-runtime/src/runtime/vm/register_ir.rs:2204`

- [x] `G4` FB call binding still performs repeated recursive instance-field
  lookup and read/write copies for each parameter binding. The multiplier is
  per-parameter, not just per-call, so parameter count must be measured
  separately from bare frame/call overhead.
  Evidence:
  - `crates/trust-runtime/src/runtime/vm/call.rs:463`
  - `crates/trust-runtime/src/runtime/vm/call.rs:537`

- [x] `G5` `Value` is still a cloned enum with boxed composite variants, so
  repeated ref loads/stores can pay clone/copy cost even when fallback count is
  zero.
  Evidence:
  - `crates/trust-runtime/src/value/types.rs:13`
  - `crates/trust-runtime/src/value/types.rs:35`

## Updated Hypotheses

- [x] `H1` Generic ref execution dominates the remaining motion cost.
  Confirmed direction after `M2`..`M4`: the expensive step-up is not direct
  globals, it is instance-field/dynamic-ref traffic and the parameter-heavy
  paths that generate it.

- [ ] `H2` FUNCTION_BLOCK call/binding overhead dominates the remaining motion
  cost.
  Not selected. The low-parameter trivial FB workload stayed close to the
  dynamic-ref workload, while the high-parameter variant climbed materially.
  That points at parameter/ref work, not bare frame push/pop churn, as the next
  target.

- [ ] `H3` Tier-1 specialization coverage is now the next ceiling.
  Still relevant, but not yet the chosen first optimization pass because the new
  synthetic workloads already show a profitable ref/binding path without a new
  tier-1 measurement gap.

- [ ] `H4` Value clone/representation work is only justified if `H1`/`H2`
  remain expensive after the targeted ref/tier-1/call-path work.

## Test-First Investigation Checklist

### T. Behavior Locks Before Instrumentation

- [x] `T1` Add a unit test proving the VM profile snapshot records ref-operation
  counters for a synthetic register-IR program that exercises
  `LoadRef` / `StoreRef`.
- [x] `T2` Add a unit test proving the VM profile snapshot records dynamic-ref
  counters for a program that exercises `LoadRefAddr` + `RefField` +
  `LoadDynamic` / `StoreDynamic`.
- [x] `T3` Add a unit test proving the VM profile snapshot records FB/frame
  counters for a trivial FUNCTION_BLOCK call path.
- [x] `T4` Add a bench JSON test proving the new counters are emitted from
  `trust-runtime bench project`.

### I. Instrumentation To Add

- [x] `I1` Extend `VmRegisterProfileSnapshot` / bench `VmProfileReport` with
  explicit counters for:
  - ref ops: `load_ref`, `store_ref`, `load_ref_addr`, `ref_field`,
    `ref_index`, `load_dynamic`, `store_dynamic`, `instance_field_lookups`
  - call/frame ops: frame pushes, frame pops, FB call entries, parameter
    bindings, output copy-backs
  - tier-1 utilization: compiled block executions, compile attempts/successes,
    deopts on the benchmark path

- [x] `I2` Increment those counters in the real hot paths:
  - `crates/trust-runtime/src/runtime/vm/dispatch.rs`
  - `crates/trust-runtime/src/runtime/vm/call.rs`
  - `crates/trust-runtime/src/runtime/vm/register_ir.rs`
  - note: on the locked motion workloads (`0` fallbacks), register-path ref
    counters are recorded at the register-dispatch/call sites rather than
    inside `dispatch_refs.rs`, so the measured counts still reflect the active
    hot path.

- [x] `I3` Surface the counters in:
  - `trust-runtime bench project` JSON
  - bench table output
  - `scripts/runtime_motion_benchmark_breakdown.sh` summary output

### M. Workloads To Measure

- [x] `M1` Re-run the locked motion workloads unchanged:
  - `runtime_floor`
  - `status_only`
  - `command_idle`
  - `move_absolute_only`
  - `full_demo`

- [x] `M2` Add one synthetic scalar-ref benchmark project that performs many
  global scalar reads/writes without FB call overhead.
  Result:
  - `scalar_globals_only`: `p50 18.408 us`, `instance_field_lookups = 0`,
    `parameter_bindings = 0`

- [x] `M3` Add one synthetic dynamic-ref benchmark project that performs
  `LoadRefAddr` / field / dynamic load-store traffic without real motion logic.
  Include at least one FUNCTION_BLOCK-style `LOAD_SELF + REF_FIELD` instance
  access path so the `ref_for_instance_recursive` / `get_index_of(name)` cost
  is measured separately from plain struct-field path traversal.
  Result:
  - `dynamic_refs`: `p50 42.537 us`, `instance_field_lookups = 6092`,
    `parameter_bindings = 480`

- [x] `M4` Add one trivial-FB benchmark project that isolates frame setup,
  parameter bind/copy, and FB output propagation. Include variants with
  different parameter counts so per-call cost can be separated from
  per-parameter binding cost.
  Result:
  - `trivial_fb_low_params`: `p50 40.741 us`, `parameter_bindings = 3840`
  - `trivial_fb_high_params`: `p50 74.889 us`, `parameter_bindings = 12800`

### D. Decision Gates After Measurement

- [x] `D1` If ref counters dominate and trivial-FB overhead is small, proceed to
  a ref-path specialization pass.
  Go decision:
  - direct globals are cheap: `scalar_globals_only p50 18.408 us`
  - dynamic refs with one FB body already cost `42.537 us`
  - low-parameter trivial FB calls cost `40.741 us`
  - holding call count constant while increasing parameter volume raises cost to
    `74.889 us`
  - the next pass should target instance-field lookup and parameter-driven ref
    work first, not generic frame churn

- [ ] `D2` If trivial-FB overhead dominates, proceed to an FB call/frame pass.
  Not selected by the measured data.

- [ ] `D3` Only escalate to `Value` representation work if both ref-path and
  FB/frame work leave clear headroom on the table.

## Expected Deliverables From This Pass

Status after the synthetic workload slice:
- `T1`..`T4` are locked and green.
- `I1`..`I3` are implemented for the active VM/register benchmark path.
- `M1`..`M4` were rerun through `./scripts/runtime_motion_benchmark_breakdown.sh`.
- The investigation no longer needs another synthetic workload round before the
  next implementation pass.

- [x] Updated motion benchmark artifacts with ref/call/tier-1 counters
- [x] A short before/after benchmark summary for:
  - `runtime_floor`
  - `status_only`
  - `command_idle`
  - `full_demo`
- [x] A written go/no-go recommendation for the next implementation pass:
  - ref specialization
  - FB/frame optimization
  - or deeper value/storage work

## Measurement Note

- [ ] Treat instrumentation runs as attribution evidence, not final absolute
  latency evidence. Re-run the locked timing workloads after removing or
  disabling the extra counters before making a performance claim.

## Acceptance Criteria

- [x] All new profiling tests pass.
- [x] Existing targeted runtime tests still pass.
- [x] Motion demo semantics remain clean.
- [x] `vm_profile.register_program_fallbacks == 0` remains true on the locked
  motion workloads.
- [x] The chosen next optimization target is justified by measured counters, not
  by assumption.

Follow-on implementation note: the first P2 cache should be per-instance, not keyed only by `type_name`, because builtin FBs such as timers/triggers/counters can lazily materialize hidden state and temporarily give same-type instances different field layouts.
