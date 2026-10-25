// The program of a launch, and of a `restart` that finds the resource Stopped: loaded, attached to
// the process's control surfaces and started the same way (specification 34, STATE-08).

/// The handles through which a running program reports to the control surfaces.
struct ProgramWiring {
    debug: trust_runtime::debug::DebugControl,
    metrics: Arc<Mutex<RuntimeMetrics>>,
    io_health: Arc<Mutex<Vec<trust_runtime_services::status::IoDriverStatus>>>,
}

/// The cycle interval of a bundle's program, and the computer's default for a launch without one.
fn bundle_cycle_interval(bundle: Option<&RuntimeBundle>) -> Duration {
    bundle
        .map(|bundle| bundle.runtime.cycle_interval)
        .unwrap_or_else(|| Duration::from_millis(10))
}

/// What hosted hang supervision does for a bundle's program.
fn bundle_hang_action(bundle: Option<&RuntimeBundle>) -> trust_runtime::watchdog::HangAction {
    bundle
        .map(|bundle| bundle.runtime.watchdog_hang_action)
        .unwrap_or_default()
}

/// Attaches a freshly loaded `program` to the control surfaces, applies the runtime-level services
/// of its bundle, and starts it in `restart_mode`, as a new process starts its program
/// (specification 44 §8.8).
fn start_program(
    program: &mut trust_runtime::CompiledRuntime,
    bundle: Option<&RuntimeBundle>,
    wiring: &ProgramWiring,
    restart_mode: RestartMode,
) -> anyhow::Result<()> {
    program.set_debug_control(wiring.debug.clone());
    program.set_metrics_sink(wiring.metrics.clone());
    program.set_io_health_sink(Some(wiring.io_health.clone()));
    if let Some(bundle) = bundle {
        apply_bundle_services(program, bundle)?;
    }
    // A warm start takes the retained values of the retain file (specification 44 §11).
    program.restart_with_retain_store(restart_mode)?;
    Ok(())
}

/// What a `restart` that finds the resource Stopped needs to load the installed program again: where
/// the program comes from, the launch options that shape it, and the records of the process that
/// start empty with the new program (specification 34, STATE-08).
struct ProgramReload {
    /// The project folder, configuration file and runtime root of the launch.
    source: (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>),
    simulation: bool,
    time_scale: u32,
    wiring: ProgramWiring,
    events: Arc<Mutex<VecDeque<trust_runtime::debug::RuntimeEvent>>>,
    settings: Arc<Mutex<RuntimeSettings>>,
}

impl ProgramReload {
    /// The loader the scheduler calls on a restart of a stopped program.
    fn into_loader(self) -> trust_runtime::scheduler::ProgramLoader {
        Box::new(move |slot, restart_mode| {
            self.load(slot, restart_mode).map_err(|error| {
                trust_runtime::error::RuntimeError::InvalidBundle(format!("{error:#}").into())
            })
        })
    }

    /// Loads the program that the launch's project folder names now into `slot` and starts it.
    ///
    /// The program is read first, so a bundle that cannot be read changes nothing. The program that
    /// stopped is dropped, with its I/O drivers, before the drivers of the new one are built, so a
    /// driver that holds a device alone is released first (specification 34, STATE-08). The
    /// metrics, the event store and the I/O health start empty with the new program, and the
    /// settings that the program's configuration gives the scan are read again.
    fn load(
        &self,
        slot: &mut trust_runtime::CompiledRuntime,
        restart_mode: RestartMode,
    ) -> anyhow::Result<trust_runtime::scheduler::StartedProgram> {
        let (project, config, runtime_root) = self.source.clone();
        let LoadedRuntime {
            bundle,
            runtime,
            ide_shell_mode,
            ..
        } = load_runtime(project, config, runtime_root)?;
        let plan = build_simulation_plan(bundle.as_ref(), self.simulation, self.time_scale)?;

        *slot = runtime;
        self.restart_records();
        start_program(slot, bundle.as_ref(), &self.wiring, restart_mode)?;

        if let Ok(mut settings) = self.settings.lock() {
            let fresh = build_runtime_settings(
                bundle.as_ref(),
                ide_shell_mode,
                slot.watchdog_policy(),
                slot.fault_policy(),
                &SimulationPlan {
                    enabled: plan.enabled,
                    time_scale: plan.time_scale,
                    warning: plan.warning.clone(),
                    controller: None,
                },
                execution_backend_source(bundle.as_ref()),
            );
            refresh_resource_settings(&mut settings, fresh);
        }
        Ok(trust_runtime::scheduler::StartedProgram {
            cycle_interval: bundle_cycle_interval(bundle.as_ref()),
            time_scale: plan.time_scale,
            simulation: plan.controller,
            hang_action: bundle_hang_action(bundle.as_ref()),
        })
    }

    /// Starts the records of the process again, as a new process starts them: empty metrics whose
    /// uptime counts from now, an empty event store and no I/O driver health.
    fn restart_records(&self) {
        if let Ok(mut metrics) = self.wiring.metrics.lock() {
            metrics.restart(trust_runtime::metrics::HostMonotonicClock::default());
        }
        if let Ok(mut events) = self.events.lock() {
            events.clear();
        }
        if let Ok(mut io_health) = self.wiring.io_health.lock() {
            io_health.clear();
        }
    }
}

/// Takes the settings that the program's own configuration gives the scan from `fresh`: the cycle
/// interval, the watchdog and fault policy, the retain settings and the simulation. The settings of
/// the process's services keep the values the process started with (specification 34, STATE-08).
fn refresh_resource_settings(settings: &mut RuntimeSettings, fresh: RuntimeSettings) {
    settings.cycle_interval = fresh.cycle_interval;
    settings.watchdog = fresh.watchdog;
    settings.watchdog_hang_action = fresh.watchdog_hang_action;
    settings.fault_policy = fresh.fault_policy;
    settings.retain_mode = fresh.retain_mode;
    settings.retain_save_interval = fresh.retain_save_interval;
    settings.simulation = fresh.simulation;
}
