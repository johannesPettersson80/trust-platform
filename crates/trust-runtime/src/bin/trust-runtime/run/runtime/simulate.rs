/// `trust-runtime simulate` (specification 11 §6.8.1, "Simulator for machine tests"; specification
/// 48 RU-58): the project's program and its simulation on a clock that only the test moves, which
/// answers the control protocol on its standard input and output and ends at the end of its input.
///
/// It loads the project as `run --simulation` does, whatever its `simulation.toml`'s `enabled` says,
/// and starts nothing that a test does not drive: no web server, discovery, mesh, Runtime Cloud
/// link, I/O driver, historian or metrics export. The project's watchdog and fault policies and its
/// safe outputs apply; its retained values stay in memory, and nothing is written into the project.
/// The debug surface is on, since a test forces and reads as the debugger does, and `CURRENT_DT` is
/// the logical time from 1 January 1970 UTC. Its log goes to standard error: standard output carries
/// only the responses.
pub fn run_simulator(project: PathBuf) -> anyhow::Result<()> {
    // The program's wall clock, `CURRENT_DT`, is the logical time from 1970, before anything runs.
    let clock = trust_runtime::scheduler::SteppedClock::new();
    trust_runtime::native::read_wall_clock_from(&clock);
    let LoadedRuntime {
        bundle,
        mut runtime,
        sources,
        ..
    } = load_runtime(Some(project), None, None)?;
    let bundle = bundle.ok_or_else(|| anyhow::anyhow!("the project has no runtime.toml"))?;
    let backend_source = execution_backend_source(Some(&bundle));
    let SimulationPlan {
        warning, controller, ..
    } = build_simulation_plan(Some(&bundle), true, 1)?;
    runtime.set_simulation_enabled(true);

    let debug_enabled = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let debug = trust_runtime::debug::DebugControl::new().with_enabled_flag(debug_enabled.clone());
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::new()));
    let io_health = Arc::new(Mutex::new(Vec::new()));
    let identify = trust_runtime::io::IdentifyHandle::new();
    let io_snapshot = Arc::new(Mutex::new(None));
    let io_snapshot_seen_ms = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (io_tx, io_rx) = std::sync::mpsc::channel();
    debug.set_io_sender(io_tx);
    {
        let io_snapshot = io_snapshot.clone();
        std::thread::spawn(move || {
            for snapshot in io_rx {
                if let Ok(mut guard) = io_snapshot.lock() {
                    *guard = Some(snapshot);
                }
            }
        });
    }
    let event_capacity = bundle.runtime.event_store_capacity();
    let events = Arc::new(Mutex::new(VecDeque::with_capacity(event_capacity + 1)));
    {
        let events = events.clone();
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        debug.set_runtime_sender(event_tx);
        std::thread::spawn(move || {
            for event in event_rx {
                if let Ok(mut guard) = events.lock() {
                    store_event(&mut guard, event, event_capacity);
                }
            }
        });
    }

    runtime.set_debug_control(debug.clone());
    runtime.set_metrics_sink(metrics.clone());
    runtime.set_io_health_sink(Some(io_health.clone()));
    runtime.set_identify_handle(identify.clone());
    identify.set(identify_state(Some(&bundle))?);
    runtime.set_watchdog_policy(bundle.runtime.watchdog);
    runtime.set_fault_policy(bundle.runtime.fault_policy);
    // The project's flight recorder, its fault's trace in the slot that the control link reads
    // (specification 48 RU-59; specification 11 §6.8.1).
    let runtime_id = trust_runtime::runtime_id::runtime_identity(bundle.root.as_path())?.id;
    let traces = trust_runtime::control::new_traces();
    runtime.set_flight_recorder(Some(trust_runtime::control::FlightSettings {
        cycles: bundle.runtime.flight_recorder,
        traces: traces.clone(),
        runtime_id: runtime_id.clone(),
    }));
    // The safe outputs of `io.toml` apply; its drivers do not run: the simulation is the process
    // image's only source.
    let mut io = bundle.io.clone();
    io.drivers.clear();
    runtime.add_io_config(&io)?;
    runtime.set_retain_store(None, None)?;
    runtime.restart_with_retain_store(RestartMode::Cold)?;

    let settings = build_runtime_settings(
        Some(&bundle),
        false,
        runtime.watchdog_policy(),
        runtime.fault_policy(),
        &SimulationPlan {
            enabled: true,
            time_scale: 1,
            warning,
            controller: None,
        },
        backend_source,
    );
    let pending_restart = Arc::new(Mutex::new(None));
    let mut runner = ResourceRunner::new(
        runtime,
        StdClock::stepped(clock),
        bundle_cycle_interval(Some(&bundle)),
    )
    .with_restart_signal(pending_restart.clone())
    .with_hang_action(bundle_hang_action(Some(&bundle)));
    if let Some(simulation) = controller {
        runner = runner.with_simulation(simulation);
    }
    let mut handle = runner.spawn("trust-runtime-simulator")?;
    let state = ControlState::local(trust_runtime::control::LocalControl {
        traces,
        debug,
        identify,
        resource: handle.control(),
        sources,
        io_snapshot,
        io_snapshot_seen_ms,
        pending_restart,
        control_mode: trust_runtime::config::ControlMode::Debug,
        metrics,
        events,
        settings,
        project_root: Some(bundle.root.clone()),
        resource_name: bundle.runtime.resource_name.clone(),
        runtime_id,
        io_health,
        debug_enabled,
    });
    let served = trust_runtime::control::serve_lines(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &state,
    );
    handle.stop();
    let _ = handle.join();
    served.map_err(|error| anyhow::anyhow!("the simulator's standard input or output: {error}"))
}
