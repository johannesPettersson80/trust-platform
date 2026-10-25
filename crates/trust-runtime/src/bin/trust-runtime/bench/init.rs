/// The initialization of the project compiled for this computer and run by the kernel
/// (specification 44 §8.8): the image is built once, and each sample loads it and starts it cold,
/// runs its first cycle, restarts it warm, and runs one steady cycle after the warmup cycles, at the
/// project's cycle interval in logical time.
fn run_init_bench(workload: InitBenchWorkload) -> anyhow::Result<BenchReport> {
    use trust_runtime::native::{self, NoHooks, Target, Toolchain};

    let runtime_config = RuntimeConfig::load(workload.project.join("runtime.toml"))
        .with_context(|| format!("load runtime.toml for {}", workload.project.display()))?;
    let cycle_budget = runtime_config.cycle_interval;
    let sources = collect_project_source_files(&workload.project, None)
        .with_context(|| format!("collect sources for {}", workload.project.display()))?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", workload.project.display());
    }
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the native backend needs its toolchain: {error}"))?;
    let target = Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU has no native target"))?;
    let debug = native::debug_hooks(&workload.project)?;
    let work = std::env::temp_dir().join(format!("trust-bench-init-{}", std::process::id()));
    let built = native::build(
        &sources,
        &native::Options {
            target,
            hooks: debug,
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    let built =
        built.map_err(|error| anyhow::anyhow!("the project does not compile natively: {error}"))?;
    let device = native::this_computer()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU runs no program image"))?;
    // Cycle k runs at k cycle intervals, as the scheduler's grid puts it (specification 10 §4.3).
    let at = |cycle: usize| {
        let nanos = i64::try_from(cycle)
            .ok()
            .and_then(|cycle| cycle.checked_mul(cycle_budget.as_nanos()))
            .unwrap_or(i64::MAX);
        trust_runtime::value::Duration::from_nanos(nanos)
    };

    let mut init_only_ns = Vec::with_capacity(workload.samples);
    let mut init_plus_first_cycle_ns = Vec::with_capacity(workload.samples);
    let mut first_cycle_ns = Vec::with_capacity(workload.samples);
    let mut first_mutation_ns = Vec::with_capacity(workload.samples);
    let mut retain_restart_ns = Vec::with_capacity(workload.samples);
    let mut struct_value_new_ns = Vec::with_capacity(workload.samples);
    let mut struct_value_untyped_ns = Vec::with_capacity(workload.samples);
    let mut steady_cycle_ns = Vec::with_capacity(workload.samples);

    for sample_index in 0..workload.samples {
        let sample = sample_index + 1;
        let (new_ns, untyped_ns) = measure_struct_value_constructors()?;
        struct_value_new_ns.push(new_ns);
        struct_value_untyped_ns.push(untyped_ns);

        let startup_started = Instant::now();
        let mut program = native::load(&built.image, &device)
            .map_err(|error| anyhow::anyhow!("initialization sample {sample} failed: {error}"))?;
        program
            .cold_start(trust_runtime::value::Duration::ZERO)
            .map_err(|error| anyhow::anyhow!("initialization sample {sample} failed: {error}"))?;
        init_only_ns.push(duration_ns(startup_started));

        let first_cycle_started = Instant::now();
        program
            .scan(at(1), &mut NoHooks)
            .with_context(|| format!("first cycle sample {sample} failed"))?;
        let first_cycle_elapsed = duration_ns(first_cycle_started);
        first_cycle_ns.push(first_cycle_elapsed);
        first_mutation_ns.push(first_cycle_elapsed);
        init_plus_first_cycle_ns.push(duration_ns(startup_started));

        let restart_started = Instant::now();
        program
            .warm_start(at(1))
            .map_err(|error| anyhow::anyhow!("warm restart sample {sample} failed: {error}"))?;
        retain_restart_ns.push(duration_ns(restart_started));

        for cycle in 0..workload.warmup_cycles {
            program
                .scan(at(2 + cycle), &mut NoHooks)
                .context("init bench warmup cycle failed")?;
        }

        let steady_cycle_started = Instant::now();
        program
            .scan(at(2 + workload.warmup_cycles), &mut NoHooks)
            .with_context(|| format!("steady cycle sample {sample} failed"))?;
        steady_cycle_ns.push(duration_ns(steady_cycle_started));
    }

    let mut histogram_samples = init_only_ns.clone();
    histogram_samples.extend(first_cycle_ns.iter().copied());
    Ok(BenchReport::Init(InitBenchReport {
        scenario: "init",
        project: workload.project.display().to_string(),
        resource_name: runtime_config.resource_name.to_string(),
        execution_backend: trust_runtime::execution_backend::EXECUTION_BACKEND.to_string(),
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
        let value =
            trust_runtime::value::StructValue::new(&registry, type_id, struct_value_fields())
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
