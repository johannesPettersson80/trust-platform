fn run_project_bench(workload: ProjectBenchWorkload) -> anyhow::Result<BenchReport> {
    let runtime_config = RuntimeConfig::load(workload.project.join("runtime.toml"))
        .with_context(|| format!("load runtime.toml for {}", workload.project.display()))?;
    let cycle_budget = runtime_config.cycle_interval;
    let cycle_budget_ns = cycle_budget.as_nanos().try_into().unwrap_or(u64::MAX);
    let compile_sources = collect_project_source_files(&workload.project, None)
        .with_context(|| format!("collect sources for {}", workload.project.display()))?;
    if compile_sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", workload.project.display());
    }

    let session = CompileSession::from_sources(compile_sources);
    let mut runtime = session
        .build_runtime()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    runtime
        .set_execution_backend(runtime_config.execution_backend)
        .map_err(|err| anyhow::anyhow!(err.to_string()))
        .with_context(|| {
            format!(
                "apply execution backend '{}' for {}",
                runtime_config.execution_backend.as_str(),
                workload.project.display()
            )
        })?;
    let capture_vm_profile = runtime.execution_backend()
        == trust_runtime::execution_backend::ExecutionBackend::BytecodeVm;
    if capture_vm_profile {
        runtime.set_vm_register_profile_enabled(true);
        runtime.set_vm_tier1_specialized_executor_enabled(workload.enable_tier1);
        runtime.reset_vm_register_lowering_cache();
        runtime.reset_vm_register_profile();
        runtime.reset_vm_tier1_specialized_executor();
    }

    for warmup_index in 0..workload.warmup_cycles {
        runtime
            .execute_cycle()
            .with_context(|| format!("warmup cycle {} failed", warmup_index + 1))?;
        runtime.advance_time(cycle_budget);
    }

    let mut samples_ns = Vec::with_capacity(workload.samples);
    let mut budget_overruns = 0_u64;
    let measured_started = Instant::now();
    for sample_index in 0..workload.samples {
        let cycle_started = Instant::now();
        runtime
            .execute_cycle()
            .with_context(|| format!("measured cycle {} failed", sample_index + 1))?;
        let elapsed_ns = duration_ns(cycle_started);
        if elapsed_ns > cycle_budget_ns {
            budget_overruns = budget_overruns.saturating_add(1);
        }
        samples_ns.push(elapsed_ns);
        runtime.advance_time(cycle_budget);
    }
    let measured_duration = measured_started.elapsed();
    let measured_secs = measured_duration.as_secs_f64();
    let vm_profile = if capture_vm_profile {
        let register_snapshot = runtime.vm_register_profile_snapshot();
        let lowering_cache_snapshot = runtime.vm_register_lowering_cache_snapshot();
        let tier1_snapshot = runtime.vm_tier1_specialized_executor_snapshot();
        runtime.set_vm_register_profile_enabled(false);
        Some(project_vm_profile_report(
            &runtime,
            &register_snapshot,
            &lowering_cache_snapshot,
            &tier1_snapshot,
        ))
    } else {
        None
    };

    Ok(BenchReport::Project(ProjectBenchReport {
        scenario: "project",
        project: workload.project.display().to_string(),
        resource_name: runtime_config.resource_name.to_string(),
        execution_backend: runtime.execution_backend().as_str().to_string(),
        cycle_budget_us: ns_to_us(cycle_budget_ns),
        samples: workload.samples,
        warmup_cycles: workload.warmup_cycles,
        total_cycles: workload.warmup_cycles.saturating_add(workload.samples),
        measured_duration_ms: measured_secs * 1000.0,
        throughput_cycles_per_sec: if measured_secs > 0.0 {
            workload.samples as f64 / measured_secs
        } else {
            0.0
        },
        cycle_latency: summarize_ns(&samples_ns),
        histogram: histogram_from_ns(&samples_ns),
        budget_overruns,
        watched_globals: capture_watched_globals(&runtime, &workload.watch),
        vm_profile,
    }))
}

fn capture_watched_globals(
    runtime: &trust_runtime::Runtime,
    names: &[String],
) -> BTreeMap<String, serde_json::Value> {
    let mut watched = BTreeMap::new();
    for name in names {
        let value = runtime
            .storage()
            .get_global(name.as_str())
            .map(value_to_json)
            .unwrap_or(serde_json::Value::Null);
        watched.insert(name.clone(), value);
    }
    watched
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::SInt(value) => json!(*value),
        Value::Int(value) => json!(*value),
        Value::DInt(value) => json!(*value),
        Value::LInt(value) => json!(*value),
        Value::USInt(value) => json!(*value),
        Value::UInt(value) => json!(*value),
        Value::UDInt(value) => json!(*value),
        Value::ULInt(value) => json!(*value),
        Value::Real(value) => json!(*value),
        Value::LReal(value) => json!(*value),
        Value::Byte(value) => json!(*value),
        Value::Word(value) => json!(*value),
        Value::DWord(value) => json!(*value),
        Value::LWord(value) => json!(*value),
        Value::Time(value) | Value::LTime(value) => json!(value.as_nanos()),
        Value::Date(value) => json!(value.ticks()),
        Value::LDate(value) => json!(value.nanos()),
        Value::Tod(value) => json!(value.ticks()),
        Value::LTod(value) => json!(value.nanos()),
        Value::Dt(value) => json!(value.ticks()),
        Value::Ldt(value) => json!(value.nanos()),
        Value::String(value) => json!(value.as_str()),
        Value::WString(value) => json!(value),
        Value::Char(value) => json!(format!("CHAR#{}", value)),
        Value::WChar(value) => json!(format!("WCHAR#{}", value)),
        Value::Array(value) => {
            serde_json::Value::Array(value.elements().iter().map(value_to_json).collect())
        }
        Value::Struct(value) => {
            let mut object = serde_json::Map::new();
            for (name, field) in value.fields() {
                object.insert(name.to_string(), value_to_json(field));
            }
            serde_json::Value::Object(object)
        }
        Value::Enum(value) => json!({
            "type": value.type_name().as_str(),
            "variant": value.variant_name().as_str(),
            "value": value.numeric_value(),
        }),
        Value::Reference(_) => serde_json::Value::Null,
        Value::Instance(value) => json!({ "instance": value.0 }),
        Value::Null => serde_json::Value::Null,
    }
}

fn project_vm_profile_report(
    runtime: &trust_runtime::Runtime,
    register_snapshot: &trust_runtime::execution_backend::VmRegisterProfileSnapshot,
    lowering_cache_snapshot: &trust_runtime::execution_backend::VmRegisterLoweringCacheSnapshot,
    tier1_snapshot: &trust_runtime::execution_backend::VmTier1SpecializedExecutorSnapshot,
) -> VmProfileReport {
    let mut fallback_reasons = register_snapshot
        .fallback_reasons
        .iter()
        .map(|entry| VmProfileFallbackReasonReport {
            reason: entry.reason.clone(),
            count: entry.count,
        })
        .collect::<Vec<_>>();
    fallback_reasons.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.reason.cmp(&right.reason))
    });

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
            pou_name: runtime.vm_pou_name(entry.pou_id),
            block_id: entry.block_id,
            start_pc: entry.start_pc,
            hits: entry.hits,
        })
        .collect::<Vec<_>>();

    let total_lookups = lowering_cache_snapshot
        .hits
        .saturating_add(lowering_cache_snapshot.misses);
    let hit_ratio = if total_lookups == 0 {
        0.0
    } else {
        lowering_cache_snapshot.hits as f64 / total_lookups as f64
    };

    let tier1_specialized_executor = if tier1_snapshot.enabled {
        Some(VmTier1SpecializedExecutorReport {
            enabled: tier1_snapshot.enabled,
            hot_block_threshold: tier1_snapshot.hot_block_threshold,
            cache_capacity: tier1_snapshot.cache_capacity,
            cached_blocks: tier1_snapshot.cached_blocks,
            compile_attempts: tier1_snapshot.compile_attempts,
            compile_successes: tier1_snapshot.compile_successes,
            compile_failures: tier1_snapshot.compile_failures,
            compile_failure_reasons: tier1_snapshot
                .compile_failure_reasons
                .iter()
                .map(|entry| VmTier1SpecializedExecutorCompileFailureReasonReport {
                    reason: entry.reason.clone(),
                    count: entry.count,
                })
                .collect(),
            cache_evictions: tier1_snapshot.cache_evictions,
            block_executions: tier1_snapshot.block_executions,
            deopt_count: tier1_snapshot.deopt_count,
            deopt_reasons: tier1_snapshot
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
        ref_ops: VmProfileRefOpReport {
            load_ref: register_snapshot.ref_ops.load_ref,
            store_ref: register_snapshot.ref_ops.store_ref,
            load_ref_addr: register_snapshot.ref_ops.load_ref_addr,
            ref_field: register_snapshot.ref_ops.ref_field,
            ref_index: register_snapshot.ref_ops.ref_index,
            load_dynamic: register_snapshot.ref_ops.load_dynamic,
            store_dynamic: register_snapshot.ref_ops.store_dynamic,
            instance_field_lookups: register_snapshot.ref_ops.instance_field_lookups,
        },
        call_ops: VmProfileCallOpReport {
            frame_pushes: register_snapshot.call_ops.frame_pushes,
            frame_pops: register_snapshot.call_ops.frame_pops,
            function_block_call_entries: register_snapshot.call_ops.function_block_call_entries,
            parameter_bindings: register_snapshot.call_ops.parameter_bindings,
            output_copy_backs: register_snapshot.call_ops.output_copy_backs,
        },
        value_ops: VmProfileValueOpReport {
            const_load_clones: register_snapshot.value_ops.const_load_clones,
            register_read_clones: register_snapshot.value_ops.register_read_clones,
            register_read_moves: register_snapshot.value_ops.register_read_moves,
            read_value_clones: register_snapshot.value_ops.read_value_clones,
            binding_expr_clones: register_snapshot.value_ops.binding_expr_clones,
            output_value_clones: register_snapshot.value_ops.output_value_clones,
        },
        profiling_overhead_ratio: None,
        register_lowering_cache: VmRegisterLoweringCacheReport {
            enabled: lowering_cache_snapshot.enabled,
            cache_capacity: lowering_cache_snapshot.cache_capacity,
            cached_entries: lowering_cache_snapshot.cached_entries,
            hits: lowering_cache_snapshot.hits,
            misses: lowering_cache_snapshot.misses,
            hit_ratio,
            build_errors: lowering_cache_snapshot.build_errors,
            cache_evictions: lowering_cache_snapshot.cache_evictions,
            invalidations: lowering_cache_snapshot.invalidations,
        },
        tier1_specialized_executor,
    }
}
