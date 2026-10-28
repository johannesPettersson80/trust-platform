struct LoadedRuntime {
    bundle: Option<RuntimeBundle>,
    /// The program, compiled for this computer (specification 44 §8.8, NX-15).
    runtime: trust_runtime::CompiledRuntime,
    sources: SourceRegistry,
    ide_shell_mode: bool,
}

struct SimulationPlan {
    enabled: bool,
    time_scale: u32,
    warning: String,
    controller: Option<trust_runtime::simulation::SimulationController>,
}
