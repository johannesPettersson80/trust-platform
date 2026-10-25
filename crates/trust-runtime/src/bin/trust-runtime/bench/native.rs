/// The project compiled for this computer and run by the kernel (specification 44 §14.3): cycle
/// after cycle at the project's cycle interval in logical time, without waiting between cycles. The
/// project's `[build] debug` chooses a debug or a release image (§13.2), as `trust-runtime build`
/// does. Reports the time of each cycle, the time of each task's activations and the image's kind,
/// code and memory, which the RuSTy comparison reads.
fn run_project_bench(workload: ProjectBenchWorkload) -> anyhow::Result<BenchReport> {
    use trust_runtime::native::{self, NoHooks, Target, Toolchain};

    let runtime_config = RuntimeConfig::load(workload.project.join("runtime.toml"))
        .with_context(|| format!("load runtime.toml for {}", workload.project.display()))?;
    let cycle_budget = runtime_config.cycle_interval;
    let cycle_budget_ns = u64::try_from(cycle_budget.as_nanos()).unwrap_or(u64::MAX);
    let sources = collect_project_source_files(&workload.project, None)
        .with_context(|| format!("collect sources for {}", workload.project.display()))?;
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the native backend needs its toolchain: {error}"))?;
    let target = Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU has no native target"))?;
    let debug = native::debug_hooks(&workload.project)?;
    let work = std::env::temp_dir().join(format!("trust-bench-native-{}", std::process::id()));
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
    let mut program =
        native::load(&built.image, &device).map_err(|error| anyhow::anyhow!("{error}"))?;
    program
        .cold_start(trust_runtime::value::Duration::ZERO)
        .map_err(|error| anyhow::anyhow!("the cold start failed: {error}"))?;
    let task_names: Vec<String> = program.tasks().map(|(name, _)| name.to_string()).collect();

    // Cycle k runs at k cycle intervals, as the scheduler's grid puts it (specification 10 §4.3).
    let at = |cycle: usize| {
        let nanos = i64::try_from(cycle)
            .ok()
            .and_then(|cycle| cycle.checked_mul(cycle_budget.as_nanos()))
            .unwrap_or(i64::MAX);
        trust_runtime::value::Duration::from_nanos(nanos)
    };
    for cycle in 1..=workload.warmup_cycles {
        program
            .scan(at(cycle), &mut NoHooks)
            .with_context(|| format!("warmup cycle {cycle} failed"))?;
    }
    let mut timer = TaskTimer {
        started: None,
        samples: vec![Vec::new(); task_names.len()],
    };
    let mut samples_ns = Vec::with_capacity(workload.samples);
    let mut budget_overruns = 0_u64;
    let measured_started = Instant::now();
    for sample in 1..=workload.samples {
        let cycle_started = Instant::now();
        program
            .scan(at(workload.warmup_cycles + sample), &mut timer)
            .with_context(|| format!("measured cycle {sample} failed"))?;
        let elapsed_ns = duration_ns(cycle_started);
        if elapsed_ns > cycle_budget_ns {
            budget_overruns = budget_overruns.saturating_add(1);
        }
        samples_ns.push(elapsed_ns);
    }
    let measured_secs = measured_started.elapsed().as_secs_f64();
    let watched_globals = workload
        .watch
        .iter()
        .map(|name| {
            let value = program
                .value(&name.to_ascii_uppercase())
                .as_ref()
                .map_or(serde_json::Value::Null, value_to_json);
            (name.clone(), value)
        })
        .collect();
    let figure = built.figure;
    Ok(BenchReport::Project(ProjectBenchReport {
        scenario: "project",
        project: workload.project.display().to_string(),
        resource_name: runtime_config.resource_name.to_string(),
        execution_backend: "native".to_string(),
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
        watched_globals,
        tasks: Some(
            task_names
                .into_iter()
                .zip(timer.samples)
                .map(|(name, samples)| TaskLatencyReport {
                    name,
                    activations: samples.len(),
                    latency: summarize_ns(&samples),
                })
                .collect(),
        ),
        native_image: Some(NativeImageReport {
            debug,
            bytes: built.image.len(),
            code: figure.code,
            rodata: figure.rodata,
            init: figure.init,
            memory_figure: figure.total,
        }),
    }))
}

/// The time of each task's activations within the scans, by the task's number in the image.
struct TaskTimer {
    started: Option<Instant>,
    samples: Vec<Vec<u64>>,
}

impl trust_runtime::native::ScanHooks for TaskTimer {
    fn task_started(&mut self, _task: usize) {
        self.started = Some(Instant::now());
    }

    fn task_finished(&mut self, task: usize) {
        if let (Some(started), Some(samples)) = (self.started.take(), self.samples.get_mut(task)) {
            samples.push(duration_ns(started));
        }
    }
}

/// The JSON of a watched global's value.
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
