struct LoadedRuntime {
    bundle: Option<RuntimeBundle>,
    /// The program: compiled, when the project holds its image (specification 44 §8.8, NX-15), or on
    /// the bytecode engine.
    runtime: trust_runtime::scheduler::HostProgram,
    sources: SourceRegistry,
    ide_shell_mode: bool,
}

struct SimulationPlan {
    enabled: bool,
    time_scale: u32,
    warning: String,
    controller: Option<trust_runtime::simulation::SimulationController>,
}
