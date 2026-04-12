#[cfg(feature = "legacy-interpreter")]
use trust_runtime::execution_backend::{
    ExecutionBackend, VmRegisterLoweringCacheSnapshot, VmRegisterProfileSnapshot,
    VmTier1SpecializedExecutorSnapshot,
};
#[cfg(feature = "legacy-interpreter")]
use trust_runtime::harness::{bytecode_bytes_from_source, TestHarness};
#[cfg(feature = "legacy-interpreter")]
use trust_runtime::RestartMode;

#[cfg(feature = "legacy-interpreter")]
struct ExecutionBackendFixture {
    name: &'static str,
    source: &'static str,
}

#[cfg(feature = "legacy-interpreter")]
const EXECUTION_BACKEND_CORPUS: &[ExecutionBackendFixture] = &[
    ExecutionBackendFixture {
        name: "call-binding",
        source: r#"
            FUNCTION Add : INT
            VAR_INPUT
                a : INT;
                b : INT := INT#2;
            END_VAR
            Add := a + b;
            END_FUNCTION

            FUNCTION Bump : INT
            VAR_IN_OUT
                x : INT;
            END_VAR
            VAR_INPUT
                inc : INT := INT#1;
            END_VAR
            x := x + inc;
            Bump := x;
            END_FUNCTION

            PROGRAM Main
            VAR
                v : INT := INT#10;
                out_named : INT := INT#0;
                out_default : INT := INT#0;
                out_inout : INT := INT#0;
            END_VAR

            out_named := Add(b := INT#4, a := INT#3);
            out_default := Add(a := INT#3);
            out_inout := Bump(v, INT#5);
            END_PROGRAM
        "#,
    },
    ExecutionBackendFixture {
        name: "string-stdlib",
        source: r#"
            PROGRAM Main
            VAR
                out_left : STRING := '';
                out_mid : STRING := '';
                out_find_found : INT := INT#0;
                out_find_missing : INT := INT#0;
                out_w_replace : WSTRING := "";
                out_w_insert : WSTRING := "";
            END_VAR

            out_left := LEFT(IN := 'ABCDE', L := INT#3);
            out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
            out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
            out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
            out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
            out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
            END_PROGRAM
        "#,
    },
    ExecutionBackendFixture {
        name: "refs-sizeof",
        source: r#"
            TYPE
                Inner : STRUCT
                    arr : ARRAY[0..2] OF INT;
                END_STRUCT;
                Outer : STRUCT
                    inner : Inner;
                END_STRUCT;
            END_TYPE

            PROGRAM Main
            VAR
                o : Outer;
                idx : INT := INT#1;
                value_cell : INT := INT#4;
                r_value : REF_TO INT;
                r_outer : REF_TO Outer;
                out_ref : INT := INT#0;
                out_after_write : INT := INT#0;
                out_nested_chain : INT := INT#0;
                out_size_type_int : DINT := DINT#0;
            END_VAR

            r_value := REF(value_cell);
            r_outer := REF(o);
            out_ref := r_value^;
            r_value^ := r_value^ + INT#3;
            out_after_write := r_value^;
            out_nested_chain := r_outer^.inner.arr[idx];
            out_size_type_int := SIZEOF(INT);
            END_PROGRAM
        "#,
    },
    ExecutionBackendFixture {
        name: "loop-arith",
        source: r#"
            PROGRAM Main
            VAR
                i : DINT := DINT#0;
                acc : DINT := DINT#0;
            END_VAR

            i := DINT#0;
            acc := DINT#0;
            WHILE i < DINT#1000 DO
                acc := acc + i;
                i := i + DINT#1;
            END_WHILE;
            END_PROGRAM
        "#,
    },
    ExecutionBackendFixture {
        name: "call-heavy-callee-arith",
        source: r#"
            FUNCTION HotLoopFn : INT
            VAR_INPUT
                a : INT;
                b : INT := INT#2;
            END_VAR

            HotLoopFn := a + b;
            HotLoopFn := HotLoopFn + INT#1;
            HotLoopFn := HotLoopFn + INT#2;
            HotLoopFn := HotLoopFn + INT#3;
            HotLoopFn := HotLoopFn + INT#4;
            HotLoopFn := HotLoopFn + INT#5;
            HotLoopFn := HotLoopFn + INT#6;
            HotLoopFn := HotLoopFn + INT#7;
            HotLoopFn := HotLoopFn + INT#8;
            HotLoopFn := HotLoopFn + INT#9;
            HotLoopFn := HotLoopFn + INT#10;
            HotLoopFn := HotLoopFn + INT#11;
            HotLoopFn := HotLoopFn + INT#12;
            HotLoopFn := HotLoopFn + INT#13;
            HotLoopFn := HotLoopFn + INT#14;
            HotLoopFn := HotLoopFn + INT#15;
            HotLoopFn := HotLoopFn + INT#16;
            HotLoopFn := HotLoopFn + INT#17;
            HotLoopFn := HotLoopFn + INT#18;
            HotLoopFn := HotLoopFn + INT#19;
            HotLoopFn := HotLoopFn + INT#20;
            HotLoopFn := HotLoopFn - INT#1;
            HotLoopFn := HotLoopFn - INT#2;
            HotLoopFn := HotLoopFn - INT#3;
            HotLoopFn := HotLoopFn - INT#4;
            HotLoopFn := HotLoopFn - INT#5;
            HotLoopFn := HotLoopFn - INT#6;
            HotLoopFn := HotLoopFn - INT#7;
            HotLoopFn := HotLoopFn - INT#8;
            HotLoopFn := HotLoopFn - INT#9;
            HotLoopFn := HotLoopFn - INT#10;
            END_FUNCTION

            PROGRAM Main
            VAR
                out_named : INT := INT#0;
                out_default : INT := INT#0;
                out_positional : INT := INT#0;
            END_VAR

            out_named := HotLoopFn(b := INT#4, a := INT#3);
            out_default := HotLoopFn(a := INT#3);
            out_positional := HotLoopFn(INT#5, INT#6);
            END_PROGRAM
        "#,
    },
];

#[cfg(feature = "legacy-interpreter")]
struct VmProfileCapture {
    samples_ns: Vec<u64>,
    register_snapshot: VmRegisterProfileSnapshot,
    lowering_cache_snapshot: VmRegisterLoweringCacheSnapshot,
    tier1_specialized_executor_snapshot: VmTier1SpecializedExecutorSnapshot,
}

#[cfg(feature = "legacy-interpreter")]
fn run_execution_backend_bench(
    workload: ExecutionBackendBenchWorkload,
) -> anyhow::Result<BenchReport> {
    let mut fixture_reports = Vec::with_capacity(EXECUTION_BACKEND_CORPUS.len());
    let mut aggregate_interpreter_ns = Vec::new();
    let mut aggregate_vm_ns = Vec::new();

    for fixture in EXECUTION_BACKEND_CORPUS {
        let interpreter_samples = collect_backend_samples(
            fixture,
            ExecutionBackend::Interpreter,
            workload.warmup_cycles,
            workload.samples,
        )?;
        let vm_samples = collect_backend_samples(
            fixture,
            ExecutionBackend::BytecodeVm,
            workload.warmup_cycles,
            workload.samples,
        )?;
        let vm_profile_capture =
            collect_vm_profile_capture(fixture, workload.warmup_cycles, workload.samples)?;
        let vm_profile = build_vm_profile_report(
            &vm_profile_capture.register_snapshot,
            &vm_profile_capture.lowering_cache_snapshot,
            &vm_profile_capture.tier1_specialized_executor_snapshot,
            vm_samples.as_slice(),
            vm_profile_capture.samples_ns.as_slice(),
        );

        aggregate_interpreter_ns.extend(interpreter_samples.iter().copied());
        aggregate_vm_ns.extend(vm_samples.iter().copied());

        fixture_reports.push(build_comparison_summary(
            fixture.name,
            &interpreter_samples,
            &vm_samples,
            Some(vm_profile),
        ));
    }

    let mut aggregate = build_comparison_summary(
        "aggregate",
        &aggregate_interpreter_ns,
        &aggregate_vm_ns,
        None,
    );
    aggregate.median_latency_ratio = median_of_fixture_ratios(
        &fixture_reports
            .iter()
            .map(|fixture| fixture.median_latency_ratio)
            .collect::<Vec<_>>(),
    );
    Ok(BenchReport::ExecutionBackend(ExecutionBackendBenchReport {
        scenario: "execution-backend",
        corpus: "mp-060-corpus-v4",
        cycles_per_fixture: workload.samples,
        warmup_cycles: workload.warmup_cycles,
        fixtures: fixture_reports,
        aggregate,
    }))
}

#[cfg(not(feature = "legacy-interpreter"))]
fn run_execution_backend_bench(
    _workload: ExecutionBackendBenchWorkload,
) -> anyhow::Result<BenchReport> {
    anyhow::bail!(
        "bench execution-backend requires --features legacy-interpreter for interpreter-vs-vm comparison"
    );
}

#[cfg(feature = "legacy-interpreter")]
fn collect_backend_samples(
    fixture: &ExecutionBackendFixture,
    backend: ExecutionBackend,
    warmup_cycles: usize,
    samples: usize,
) -> anyhow::Result<Vec<u64>> {
    let mut harness = harness_for_backend(fixture.source, backend)?;
    run_cycles_checked(&mut harness, warmup_cycles, fixture.name, backend, false)?;
    run_cycles_checked(&mut harness, samples, fixture.name, backend, true)
}

#[cfg(feature = "legacy-interpreter")]
fn collect_vm_profile_capture(
    fixture: &ExecutionBackendFixture,
    warmup_cycles: usize,
    samples: usize,
) -> anyhow::Result<VmProfileCapture> {
    let mut harness = harness_for_backend(fixture.source, ExecutionBackend::BytecodeVm)?;
    harness.runtime_mut().set_vm_register_profile_enabled(true);
    harness.runtime_mut().reset_vm_register_lowering_cache();
    harness.runtime_mut().reset_vm_register_profile();
    harness.runtime_mut().reset_vm_tier1_specialized_executor();

    run_cycles_checked(
        &mut harness,
        warmup_cycles,
        fixture.name,
        ExecutionBackend::BytecodeVm,
        false,
    )?;
    let samples_ns = run_cycles_checked(
        &mut harness,
        samples,
        fixture.name,
        ExecutionBackend::BytecodeVm,
        true,
    )?;
    let register_snapshot = harness.runtime().vm_register_profile_snapshot();
    let lowering_cache_snapshot = harness.runtime().vm_register_lowering_cache_snapshot();
    let tier1_specialized_executor_snapshot =
        harness.runtime().vm_tier1_specialized_executor_snapshot();
    verify_vm_fixture_profile_requirements(fixture.name, &register_snapshot)?;
    harness.runtime_mut().set_vm_register_profile_enabled(false);
    Ok(VmProfileCapture {
        samples_ns,
        register_snapshot,
        lowering_cache_snapshot,
        tier1_specialized_executor_snapshot,
    })
}

#[cfg(feature = "legacy-interpreter")]
fn verify_vm_fixture_profile_requirements(
    fixture_name: &str,
    register_snapshot: &VmRegisterProfileSnapshot,
) -> anyhow::Result<()> {
    if fixture_name == "call-heavy-callee-arith" || fixture_name == "loop-arith" {
        require_register_ir_execution(fixture_name, register_snapshot)?;
    }
    if fixture_name == "loop-arith" {
        let body_hits = register_snapshot
            .hot_blocks
            .iter()
            .filter(|block| block.block_id != 0)
            .map(|block| block.hits)
            .sum::<u64>();
        if body_hits <= register_snapshot.register_programs_executed {
            anyhow::bail!(
                "fixture '{fixture_name}' must execute non-entry loop body during benchmark sampling (body_hits={}, executed={})",
                body_hits,
                register_snapshot.register_programs_executed
            );
        }
    }
    Ok(())
}

#[cfg(feature = "legacy-interpreter")]
fn require_register_ir_execution(
    fixture_name: &str,
    register_snapshot: &VmRegisterProfileSnapshot,
) -> anyhow::Result<()> {
    if register_snapshot.register_programs_executed == 0
        || register_snapshot.register_program_fallbacks != 0
    {
        anyhow::bail!(
            "fixture '{fixture_name}' must execute through register IR (executed={}, fallbacks={}, reasons={:?})",
            register_snapshot.register_programs_executed,
            register_snapshot.register_program_fallbacks,
            register_snapshot.fallback_reasons
        );
    }
    Ok(())
}

#[cfg(feature = "legacy-interpreter")]
fn harness_for_backend(source: &str, backend: ExecutionBackend) -> anyhow::Result<TestHarness> {
    let mut harness = TestHarness::from_source(source).map_err(|err| anyhow::anyhow!("{err}"))?;
    match backend {
        ExecutionBackend::BytecodeVm => {
            let bytes =
                bytecode_bytes_from_source(source).map_err(|err| anyhow::anyhow!("{err}"))?;
            harness
                .runtime_mut()
                .apply_bytecode_bytes(&bytes, None)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            harness
                .runtime_mut()
                .set_execution_backend(ExecutionBackend::BytecodeVm)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            harness
                .runtime_mut()
                .restart(RestartMode::Cold)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
        }
        ExecutionBackend::Interpreter => {
            harness
                .runtime_mut()
                .set_execution_backend(ExecutionBackend::Interpreter)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            harness
                .runtime_mut()
                .restart(RestartMode::Cold)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
        }
    }

    let effective_backend = harness.runtime().execution_backend();
    if effective_backend != backend {
        anyhow::bail!(
            "execution-backend harness backend mismatch: requested={} effective={}",
            backend_label(backend),
            backend_label(effective_backend),
        );
    }
    Ok(harness)
}

#[cfg(feature = "legacy-interpreter")]
fn run_cycles_checked(
    harness: &mut TestHarness,
    count: usize,
    fixture_name: &str,
    backend: ExecutionBackend,
    measure: bool,
) -> anyhow::Result<Vec<u64>> {
    let mut samples = if measure {
        Vec::with_capacity(count)
    } else {
        Vec::new()
    };
    for cycle in 0..count {
        let started = Instant::now();
        let result = harness.cycle();
        if !result.errors.is_empty() {
            anyhow::bail!(
                "execution-backend benchmark error (fixture={fixture_name} backend={} cycle={cycle}): {:?}",
                backend_label(backend),
                result.errors
            );
        }
        if measure {
            samples.push(duration_ns(started));
        }
    }
    Ok(samples)
}

#[cfg(feature = "legacy-interpreter")]
fn build_comparison_summary(
    fixture: &'static str,
    interpreter_samples_ns: &[u64],
    vm_samples_ns: &[u64],
    vm_profile: Option<VmProfileReport>,
) -> ExecutionBackendFixtureReport {
    let interpreter_latency = summarize_ns(interpreter_samples_ns);
    let vm_latency = summarize_ns(vm_samples_ns);
    let interpreter_throughput = throughput_cycles_per_sec(interpreter_samples_ns);
    let vm_throughput = throughput_cycles_per_sec(vm_samples_ns);

    ExecutionBackendFixtureReport {
        fixture,
        interpreter: BackendComparisonSummary {
            latency: interpreter_latency.clone(),
            throughput_cycles_per_sec: interpreter_throughput,
        },
        vm: BackendComparisonSummary {
            latency: vm_latency.clone(),
            throughput_cycles_per_sec: vm_throughput,
        },
        median_latency_ratio: safe_ratio(vm_latency.p50_us, interpreter_latency.p50_us),
        p99_latency_ratio: safe_ratio(vm_latency.p99_us, interpreter_latency.p99_us),
        throughput_ratio: safe_ratio(vm_throughput, interpreter_throughput),
        vm_profile,
    }
}

#[cfg(feature = "legacy-interpreter")]
fn build_vm_profile_report(
    register_snapshot: &VmRegisterProfileSnapshot,
    lowering_cache_snapshot: &VmRegisterLoweringCacheSnapshot,
    tier1_specialized_executor_snapshot: &VmTier1SpecializedExecutorSnapshot,
    vm_samples_ns: &[u64],
    vm_profile_samples_ns: &[u64],
) -> VmProfileReport {
    let mut hot_blocks = register_snapshot.hot_blocks.to_vec();
    hot_blocks.sort_by(|left, right| {
        right
            .hits
            .cmp(&left.hits)
            .then_with(|| left.pou_id.cmp(&right.pou_id))
            .then_with(|| left.block_id.cmp(&right.block_id))
    });
    let hot_blocks = hot_blocks
        .into_iter()
        .take(16)
        .map(|entry| VmProfileHotBlockReport {
            pou_id: entry.pou_id,
            pou_name: None,
            block_id: entry.block_id,
            start_pc: entry.start_pc,
            hits: entry.hits,
        })
        .collect::<Vec<_>>();
    let fallback_reasons = register_snapshot
        .fallback_reasons
        .iter()
        .map(|entry| VmProfileFallbackReasonReport {
            reason: entry.reason.clone(),
            count: entry.count,
        })
        .collect();
    let vm_latency = summarize_ns(vm_samples_ns);
    let profile_latency = summarize_ns(vm_profile_samples_ns);
    let profiling_overhead_ratio = safe_ratio(profile_latency.p50_us, vm_latency.p50_us) - 1.0;
    let total_lookups = lowering_cache_snapshot
        .hits
        .saturating_add(lowering_cache_snapshot.misses);
    let lowering_cache_hit_ratio = if total_lookups == 0 {
        0.0
    } else {
        (lowering_cache_snapshot.hits as f64) / (total_lookups as f64)
    };
    let register_lowering_cache = VmRegisterLoweringCacheReport {
        enabled: lowering_cache_snapshot.enabled,
        cache_capacity: lowering_cache_snapshot.cache_capacity,
        cached_entries: lowering_cache_snapshot.cached_entries,
        hits: lowering_cache_snapshot.hits,
        misses: lowering_cache_snapshot.misses,
        hit_ratio: lowering_cache_hit_ratio,
        build_errors: lowering_cache_snapshot.build_errors,
        cache_evictions: lowering_cache_snapshot.cache_evictions,
        invalidations: lowering_cache_snapshot.invalidations,
    };
    let tier1_specialized_executor = if tier1_specialized_executor_snapshot.enabled {
        Some(VmTier1SpecializedExecutorReport {
            enabled: tier1_specialized_executor_snapshot.enabled,
            hot_block_threshold: tier1_specialized_executor_snapshot.hot_block_threshold,
            cache_capacity: tier1_specialized_executor_snapshot.cache_capacity,
            cached_blocks: tier1_specialized_executor_snapshot.cached_blocks,
            compile_attempts: tier1_specialized_executor_snapshot.compile_attempts,
            compile_successes: tier1_specialized_executor_snapshot.compile_successes,
            compile_failures: tier1_specialized_executor_snapshot.compile_failures,
            cache_evictions: tier1_specialized_executor_snapshot.cache_evictions,
            block_executions: tier1_specialized_executor_snapshot.block_executions,
            deopt_count: tier1_specialized_executor_snapshot.deopt_count,
            deopt_reasons: tier1_specialized_executor_snapshot
                .deopt_reasons
                .iter()
                .map(|entry| VmTier1SpecializedExecutorDeoptReasonReport {
                    reason: entry.reason.clone(),
                    count: entry.count,
                })
                .collect(),
        })
    } else {
        None
    };

    VmProfileReport {
        register_programs_executed: register_snapshot.register_programs_executed,
        register_program_fallbacks: register_snapshot.register_program_fallbacks,
        fallback_reasons,
        hot_blocks,
        profiling_overhead_ratio: Some(profiling_overhead_ratio),
        register_lowering_cache,
        tier1_specialized_executor,
    }
}

#[cfg(feature = "legacy-interpreter")]
fn throughput_cycles_per_sec(samples_ns: &[u64]) -> f64 {
    if samples_ns.is_empty() {
        return 0.0;
    }
    let total_ns: u128 = samples_ns.iter().copied().map(u128::from).sum();
    if total_ns == 0 {
        return 0.0;
    }
    (samples_ns.len() as f64) * 1_000_000_000.0 / (total_ns as f64)
}

#[cfg(feature = "legacy-interpreter")]
fn safe_ratio(lhs: f64, rhs: f64) -> f64 {
    if rhs.abs() <= f64::EPSILON {
        if lhs.abs() <= f64::EPSILON {
            return 1.0;
        }
        return f64::INFINITY;
    }
    lhs / rhs
}

#[cfg(feature = "legacy-interpreter")]
fn median_of_fixture_ratios(ratios: &[f64]) -> f64 {
    if ratios.is_empty() {
        return 0.0;
    }
    let mut sorted = ratios.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    sorted[sorted.len() / 2]
}

#[cfg(feature = "legacy-interpreter")]
fn backend_label(backend: ExecutionBackend) -> &'static str {
    match backend {
        ExecutionBackend::Interpreter => "interpreter",
        ExecutionBackend::BytecodeVm => "vm",
    }
}
