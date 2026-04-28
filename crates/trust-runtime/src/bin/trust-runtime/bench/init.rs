fn run_init_bench(workload: InitBenchWorkload) -> anyhow::Result<BenchReport> {
    let runtime_config = RuntimeConfig::load(workload.project.join("runtime.toml"))
        .with_context(|| format!("load runtime.toml for {}", workload.project.display()))?;
    let cycle_budget = runtime_config.cycle_interval;
    let compile_sources = collect_project_source_files(&workload.project, None)
        .with_context(|| format!("collect sources for {}", workload.project.display()))?;
    if compile_sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", workload.project.display());
    }

    let mut init_only_ns = Vec::with_capacity(workload.samples);
    let mut init_plus_first_cycle_ns = Vec::with_capacity(workload.samples);
    let mut first_cycle_ns = Vec::with_capacity(workload.samples);
    let mut first_mutation_ns = Vec::with_capacity(workload.samples);
    let mut retain_restart_ns = Vec::with_capacity(workload.samples);
    let mut struct_value_new_ns = Vec::with_capacity(workload.samples);
    let mut struct_value_untyped_ns = Vec::with_capacity(workload.samples);
    let mut steady_cycle_ns = Vec::with_capacity(workload.samples);
    let mut execution_backend = runtime_config.execution_backend.as_str().to_string();

    for sample_index in 0..workload.samples {
        let (new_ns, untyped_ns) = measure_struct_value_constructors()?;
        struct_value_new_ns.push(new_ns);
        struct_value_untyped_ns.push(untyped_ns);

        let startup_started = Instant::now();
        let session = CompileSession::from_sources(compile_sources.clone());
        let init_started = Instant::now();
        let mut runtime = session
            .build_runtime()
            .map_err(|err| anyhow::anyhow!(err.to_string()))
            .with_context(|| format!("initialization sample {} failed", sample_index + 1))?;
        init_only_ns.push(duration_ns(init_started));
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
        execution_backend = runtime.execution_backend().as_str().to_string();

        let first_cycle_started = Instant::now();
        runtime
            .execute_cycle()
            .with_context(|| format!("first cycle sample {} failed", sample_index + 1))?;
        let first_cycle_elapsed = duration_ns(first_cycle_started);
        first_cycle_ns.push(first_cycle_elapsed);
        first_mutation_ns.push(first_cycle_elapsed);
        init_plus_first_cycle_ns.push(duration_ns(startup_started));
        runtime.advance_time(cycle_budget);

        let restart_started = Instant::now();
        runtime
            .restart(trust_runtime::RestartMode::Warm)
            .with_context(|| format!("warm restart sample {} failed", sample_index + 1))?;
        retain_restart_ns.push(duration_ns(restart_started));

        for _ in 0..workload.warmup_cycles {
            runtime.execute_cycle().context("init bench warmup cycle failed")?;
            runtime.advance_time(cycle_budget);
        }

        let steady_cycle_started = Instant::now();
        runtime
            .execute_cycle()
            .with_context(|| format!("steady cycle sample {} failed", sample_index + 1))?;
        steady_cycle_ns.push(duration_ns(steady_cycle_started));
    }

    let mut histogram_samples = init_only_ns.clone();
    histogram_samples.extend(first_cycle_ns.iter().copied());
    Ok(BenchReport::Init(InitBenchReport {
        scenario: "init",
        project: workload.project.display().to_string(),
        resource_name: runtime_config.resource_name.to_string(),
        execution_backend,
        samples: workload.samples,
        warmup_cycles: workload.warmup_cycles,
        init_only_latency: summarize_ns(&init_only_ns),
        init_plus_first_cycle_latency: summarize_ns(&init_plus_first_cycle_ns),
        first_cycle_latency: summarize_ns(&first_cycle_ns),
        first_mutation_latency: summarize_ns(&first_mutation_ns),
        retain_restart_latency: summarize_ns(&retain_restart_ns),
        struct_value_new_latency: summarize_ns(&struct_value_new_ns),
        struct_value_untyped_latency: summarize_ns(&struct_value_untyped_ns),
        steady_cycle_latency: summarize_ns(&steady_cycle_ns),
        histogram: histogram_from_ns(&histogram_samples),
    }))
}

fn measure_struct_value_constructors() -> anyhow::Result<(u64, u64)> {
    let mut registry = trust_hir::types::TypeRegistry::new();
    let fields = (0..50)
        .map(|idx| trust_hir::types::StructField {
            name: smol_str::SmolStr::new(format!("f{idx}")),
            type_id: trust_hir::TypeId::INT,
            address: None,
            default_initializer: None,
        })
        .collect::<Vec<_>>();
    let type_id = registry.register_struct("BenchStruct50", fields);

    let new_started = Instant::now();
    for _ in 0..1_000 {
        let value = trust_runtime::value::StructValue::new(
            &registry,
            type_id,
            struct_value_fields(),
        )
        .context("construct StructValue through validating constructor")?;
        std::hint::black_box(value);
    }
    let new_ns = duration_ns(new_started);

    let untyped_started = Instant::now();
    for _ in 0..1_000 {
        let value = trust_runtime::value::StructValue::from_untyped_parts(
            "BenchStruct50".into(),
            struct_value_fields(),
        );
        std::hint::black_box(value);
    }
    let untyped_ns = duration_ns(untyped_started);

    Ok((new_ns, untyped_ns))
}

fn struct_value_fields() -> indexmap::IndexMap<smol_str::SmolStr, Value> {
    (0..50)
        .map(|idx| {
            (
                smol_str::SmolStr::new(format!("f{idx}")),
                Value::Int(idx as i16),
            )
        })
        .collect()
}
