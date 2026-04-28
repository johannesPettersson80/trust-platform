#[derive(Debug, Clone, Serialize)]
struct HistogramBucket {
    upper_us: Option<u64>,
    count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct LatencySummary {
    samples: usize,
    min_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
}

#[derive(Debug, Clone, Serialize)]
struct T0ShmBenchReport {
    scenario: &'static str,
    one_way_latency: LatencySummary,
    round_trip_latency: LatencySummary,
    jitter: LatencySummary,
    histogram: Vec<HistogramBucket>,
    overruns: u64,
    stale_reads: u64,
    spin_exhausted: u64,
    fallback_denied: u64,
}

#[derive(Debug, Clone, Serialize)]
struct MeshZenohBenchReport {
    scenario: &'static str,
    pub_sub_latency: LatencySummary,
    pub_sub_jitter: LatencySummary,
    query_reply_latency: LatencySummary,
    histogram: Vec<HistogramBucket>,
    loss_count: u64,
    reorder_count: u64,
    configured_loss_rate: f64,
    configured_reorder_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
struct DispatchBenchReport {
    scenario: &'static str,
    fanout: usize,
    preflight_latency: LatencySummary,
    dispatch_latency: LatencySummary,
    end_to_end_latency: LatencySummary,
    audit_correlation_latency: LatencySummary,
    histogram: Vec<HistogramBucket>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectBenchReport {
    scenario: &'static str,
    project: String,
    resource_name: String,
    execution_backend: String,
    cycle_budget_us: f64,
    samples: usize,
    warmup_cycles: usize,
    total_cycles: usize,
    measured_duration_ms: f64,
    throughput_cycles_per_sec: f64,
    cycle_latency: LatencySummary,
    histogram: Vec<HistogramBucket>,
    budget_overruns: u64,
    watched_globals: BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vm_profile: Option<VmProfileReport>,
}

#[derive(Debug, Clone, Serialize)]
struct InitBenchReport {
    scenario: &'static str,
    project: String,
    resource_name: String,
    execution_backend: String,
    samples: usize,
    warmup_cycles: usize,
    init_only_latency: LatencySummary,
    init_plus_first_cycle_latency: LatencySummary,
    first_cycle_latency: LatencySummary,
    first_mutation_latency: LatencySummary,
    retain_restart_latency: LatencySummary,
    struct_value_new_latency: LatencySummary,
    struct_value_untyped_latency: LatencySummary,
    steady_cycle_latency: LatencySummary,
    histogram: Vec<HistogramBucket>,
}


#[derive(Debug, Clone, Serialize)]
struct VmProfileFallbackReasonReport {
    reason: String,
    count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmProfileHotBlockReport {
    pou_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pou_name: Option<String>,
    block_id: u32,
    start_pc: u32,
    hits: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmProfileRefOpReport {
    load_ref: u64,
    store_ref: u64,
    load_ref_addr: u64,
    ref_field: u64,
    ref_index: u64,
    load_dynamic: u64,
    store_dynamic: u64,
    instance_field_lookups: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmProfileCallOpReport {
    frame_pushes: u64,
    frame_pops: u64,
    function_block_call_entries: u64,
    parameter_bindings: u64,
    output_copy_backs: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmProfileValueOpReport {
    const_load_clones: u64,
    register_read_clones: u64,
    register_read_moves: u64,
    read_value_clones: u64,
    binding_expr_clones: u64,
    output_value_clones: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmTier1SpecializedExecutorCompileFailureReasonReport {
    reason: String,
    count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmTier1SpecializedExecutorDeoptReasonReport {
    reason: String,
    count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmTier1SpecializedExecutorReport {
    enabled: bool,
    hot_block_threshold: u64,
    cache_capacity: usize,
    cached_blocks: usize,
    compile_attempts: u64,
    compile_successes: u64,
    compile_failures: u64,
    compile_failure_reasons: Vec<VmTier1SpecializedExecutorCompileFailureReasonReport>,
    cache_evictions: u64,
    block_executions: u64,
    deopt_count: u64,
    deopt_reasons: Vec<VmTier1SpecializedExecutorDeoptReasonReport>,
}

#[derive(Debug, Clone, Serialize)]
struct VmRegisterLoweringCacheReport {
    enabled: bool,
    cache_capacity: usize,
    cached_entries: usize,
    hits: u64,
    misses: u64,
    hit_ratio: f64,
    build_errors: u64,
    cache_evictions: u64,
    invalidations: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VmProfileReport {
    register_programs_executed: u64,
    register_program_fallbacks: u64,
    fallback_reasons: Vec<VmProfileFallbackReasonReport>,
    hot_blocks: Vec<VmProfileHotBlockReport>,
    ref_ops: VmProfileRefOpReport,
    call_ops: VmProfileCallOpReport,
    value_ops: VmProfileValueOpReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    profiling_overhead_ratio: Option<f64>,
    register_lowering_cache: VmRegisterLoweringCacheReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier1_specialized_executor: Option<VmTier1SpecializedExecutorReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "benchmark", content = "report")]
#[allow(clippy::large_enum_variant)]
enum BenchReport {
    #[serde(rename = "t0-shm")]
    T0Shm(T0ShmBenchReport),
    #[serde(rename = "project")]
    Project(ProjectBenchReport),
    #[serde(rename = "init")]
    Init(InitBenchReport),
    #[serde(rename = "mesh-zenoh")]
    MeshZenoh(MeshZenohBenchReport),
    #[serde(rename = "dispatch")]
    Dispatch(DispatchBenchReport),
}

#[derive(Debug, Clone)]
struct BenchWorkload {
    samples: usize,
    payload_bytes: usize,
}

impl BenchWorkload {
    fn normalize(samples: usize, payload_bytes: usize) -> anyhow::Result<Self> {
        if samples == 0 {
            anyhow::bail!("--samples must be greater than zero");
        }
        if payload_bytes == 0 {
            anyhow::bail!("--payload-bytes must be greater than zero");
        }
        Ok(Self {
            samples,
            payload_bytes,
        })
    }
}

#[derive(Debug, Clone)]
struct ProjectBenchWorkload {
    project: std::path::PathBuf,
    samples: usize,
    warmup_cycles: usize,
    watch: Vec<String>,
    enable_tier1: bool,
}

impl ProjectBenchWorkload {
    fn normalize(
        project: std::path::PathBuf,
        samples: usize,
        warmup_cycles: usize,
        watch: Vec<String>,
        enable_tier1: bool,
    ) -> anyhow::Result<Self> {
        if samples == 0 {
            anyhow::bail!("--samples must be greater than zero");
        }
        if !project.is_dir() {
            anyhow::bail!("--project must point to an existing project folder");
        }
        Ok(Self {
            project,
            samples,
            warmup_cycles,
            watch,
            enable_tier1,
        })
    }
}

#[derive(Debug, Clone)]
struct InitBenchWorkload {
    project: std::path::PathBuf,
    samples: usize,
    warmup_cycles: usize,
}

impl InitBenchWorkload {
    fn normalize(
        project: std::path::PathBuf,
        samples: usize,
        warmup_cycles: usize,
    ) -> anyhow::Result<Self> {
        if samples == 0 {
            anyhow::bail!("--samples must be greater than zero");
        }
        if !project.is_dir() {
            anyhow::bail!("--project must point to an existing project folder");
        }
        Ok(Self {
            project,
            samples,
            warmup_cycles,
        })
    }
}

#[derive(Debug, Clone)]
struct MeshBenchWorkload {
    base: BenchWorkload,
    loss_rate: f64,
    reorder_rate: f64,
}

impl MeshBenchWorkload {
    fn normalize(
        samples: usize,
        payload_bytes: usize,
        loss_rate: f64,
        reorder_rate: f64,
    ) -> anyhow::Result<Self> {
        if !(0.0..=1.0).contains(&loss_rate) {
            anyhow::bail!("--loss-rate must be between 0.0 and 1.0");
        }
        if !(0.0..=1.0).contains(&reorder_rate) {
            anyhow::bail!("--reorder-rate must be between 0.0 and 1.0");
        }
        Ok(Self {
            base: BenchWorkload::normalize(samples, payload_bytes)?,
            loss_rate,
            reorder_rate,
        })
    }
}

#[derive(Debug, Clone)]
struct DispatchBenchWorkload {
    base: BenchWorkload,
    fanout: usize,
}

impl DispatchBenchWorkload {
    fn normalize(samples: usize, payload_bytes: usize, fanout: usize) -> anyhow::Result<Self> {
        if fanout == 0 {
            anyhow::bail!("--fanout must be greater than zero");
        }
        Ok(Self {
            base: BenchWorkload::normalize(samples, payload_bytes)?,
            fanout,
        })
    }
}
