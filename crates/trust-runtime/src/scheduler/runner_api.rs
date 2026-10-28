/// Drives a runtime with a scheduling clock.
pub struct ResourceRunner<C: Clock + Clone> {
    runtime: CompiledRuntime,
    clock: C,
    cycle_interval: Duration,
    timing: CycleTiming,
    /// Where the achieved simulation speed is reported (specification 46, §6.1 item 3), also by a
    /// program loaded again after a stop.
    simulation_metrics: Option<Arc<Mutex<crate::metrics::RuntimeMetrics>>>,
    restart_signal: Option<Arc<Mutex<Option<crate::RestartMode>>>>,
    start_gate: Option<Arc<StartGate>>,
    command_rx: Option<std::sync::mpsc::Receiver<ResourceCommand>>,
    simulation: Option<crate::simulation::SimulationController>,
    thread_init_hook: Option<Arc<dyn Fn() -> Result<(), RuntimeError> + Send + Sync>>,
    hang_action: crate::watchdog::HangAction,
    program_loader: Option<ProgramLoader>,
    /// Whether the next restart loads the installed program instead of restarting the one in the
    /// runtime: the program stopped, or the last load did not end in a started program.
    reload_required: bool,
    /// The scheduler clock's reading when the program started. The program's logical time counts
    /// from it (specification 34, STATE-08), so a program started again begins at zero.
    program_origin: Duration,
    /// The program time at which the tasks' timing last began: the program's start or its last
    /// restart (specification 10 §4.3).
    task_origin: Duration,
    /// Whether the tasks' timing began again since the scan loop last took the grid's origin.
    grid_restarted: bool,
    /// Whether a `MeshCatalogue` request is refused, as a debug session's program publishes no
    /// runtime mesh catalogue (specification 43, "Debug-session boundary").
    refuse_mesh_catalogue: bool,
}

impl<C: Clock + Clone> ResourceRunner<C> {
    #[must_use]
    pub fn new(runtime: CompiledRuntime, clock: C, cycle_interval: Duration) -> Self {
        Self {
            runtime,
            clock,
            cycle_interval,
            timing: CycleTiming::wall_clock(),
            simulation_metrics: None,
            restart_signal: None,
            start_gate: None,
            command_rx: None,
            simulation: None,
            thread_init_hook: None,
            hang_action: crate::watchdog::HangAction::default(),
            program_loader: None,
            reload_required: false,
            program_origin: Duration::ZERO,
            task_origin: Duration::ZERO,
            grid_restarted: true,
            refuse_mesh_catalogue: false,
        }
    }

    /// Attach a restart signal for external control.
    #[must_use]
    pub fn with_restart_signal(mut self, signal: Arc<Mutex<Option<crate::RestartMode>>>) -> Self {
        self.restart_signal = Some(signal);
        self
    }

    /// Attach a start gate that must be opened before the scheduler runs.
    #[must_use]
    pub fn with_start_gate(mut self, gate: Arc<StartGate>) -> Self {
        self.start_gate = Some(gate);
        self
    }

    /// Run on simulation time at the shared `speed` (specification 46, §6.1). Each executed cycle advances
    /// simulation time by one cycle interval, and cycles are paced at the cycle interval divided by the speed.
    /// With `metrics`, the achieved speed is reported to control.status. Without this, process time follows the
    /// wall clock.
    #[must_use]
    pub fn with_simulation_speed(
        mut self,
        speed: SimulationSpeed,
        metrics: Option<Arc<Mutex<crate::metrics::RuntimeMetrics>>>,
    ) -> Self {
        self.timing = CycleTiming::simulated(speed, metrics.clone());
        self.simulation_metrics = metrics;
        self
    }

    /// Attach a simulation controller for coupling/disturbance hooks.
    #[must_use]
    pub fn with_simulation(mut self, simulation: crate::simulation::SimulationController) -> Self {
        self.simulation = Some(simulation);
        self
    }

    /// Run a thread-local startup hook before the scheduler loop begins.
    #[must_use]
    pub fn with_thread_init_hook(
        mut self,
        hook: Arc<dyn Fn() -> Result<(), RuntimeError> + Send + Sync>,
    ) -> Self {
        self.thread_init_hook = Some(hook);
        self
    }

    /// Select what hosted hang supervision does after it records a hung scan thread
    /// (`runtime.watchdog.hang_action`). The default is [`crate::watchdog::HangAction::Fault`].
    #[must_use]
    pub fn with_hang_action(mut self, action: crate::watchdog::HangAction) -> Self {
        self.hang_action = action;
        self
    }

    /// The runner of a debug session's program, which publishes no runtime mesh catalogue
    /// (specification 43, "Debug-session boundary"): it answers every `MeshCatalogue` request with
    /// the error `mesh catalogue unavailable in debug control` instead of the program's catalogue,
    /// while the program runs, is stopped or is faulted.
    #[must_use]
    pub fn refusing_mesh_catalogue(mut self) -> Self {
        self.refuse_mesh_catalogue = true;
        self
    }

    /// Attach the loader that a `restart` of a stopped program uses to load the installed program
    /// again. Without one, a restart restarts the program that stopped, in place (specification 34,
    /// STATE-08).
    #[must_use]
    pub fn with_program_loader(mut self, loader: ProgramLoader) -> Self {
        self.program_loader = Some(loader);
        self
    }

    /// The program the runner runs.
    #[must_use]
    pub fn program(&self) -> &CompiledRuntime {
        &self.runtime
    }

    /// The program the runner runs.
    pub fn program_mut(&mut self) -> &mut CompiledRuntime {
        &mut self.runtime
    }

    /// The program's time on the scheduler clock: the clock's reading `clock_now` less its reading
    /// when the program started.
    fn program_time(&self, clock_now: Duration) -> Duration {
        Duration::from_nanos(
            clock_now
                .as_nanos()
                .saturating_sub(self.program_origin.as_nanos()),
        )
    }

    /// The scheduler clock's reading at `program_time`, a time of the program's (the inverse of
    /// [`Self::program_time`]).
    fn clock_time(&self, program_time: Duration) -> Duration {
        Duration::from_nanos(
            program_time
                .as_nanos()
                .saturating_add(self.program_origin.as_nanos()),
        )
    }

    /// Restart the runtime while preserving the runner's live clock baseline.
    pub fn restart(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        self.runtime.restart(mode)?;
        self.recreate_plants();
        self.continue_after_restart();
        Ok(())
    }

    /// Restart and apply the configured retain store for warm mode.
    pub fn restart_with_retain_store(
        &mut self,
        mode: crate::RestartMode,
    ) -> Result<(), RuntimeError> {
        self.runtime.restart_with_retain_store(mode)?;
        self.recreate_plants();
        self.continue_after_restart();
        Ok(())
    }

    /// A cold and a warm restart recreate every simulated plant from its configuration
    /// (specification 46 §5.3): plant state is not retained through a restart.
    fn recreate_plants(&mut self) {
        if let Some(simulation) = &self.simulation {
            self.simulation = Some(simulation.recreated());
        }
    }

    /// Continues process time after a restart, from the last simulation time in simulation (specification 46,
    /// §6.1 item 4) and from the program's time on the scheduler clock otherwise (specification 34, STATE-08),
    /// and restarts the simulation schedule.
    fn continue_after_restart(&mut self) {
        let now = self
            .timing
            .restart_time(self.program_time(self.clock.now()));
        self.runtime.reset_task_timing(now);
        self.task_origin = now;
        self.grid_restarted = true;
        self.timing.restart_schedule();
    }

    /// The first boundary of the cycle grid on the scheduler clock when the tasks' timing began
    /// again since the last call: the moment it began, so that the grid's boundaries are the tasks'
    /// deadlines and no cycle finds a task of the cycle interval not yet due (specification 10
    /// §4.3). A first scan that starts later, after the start gate or a slow start, takes the last
    /// of those boundaries at or before it.
    fn take_grid_origin(&mut self) -> Option<Duration> {
        std::mem::take(&mut self.grid_restarted).then(|| self.clock_time(self.task_origin))
    }

    /// The simulation snapshot for plant status. Its configured speed is the live speed.
    pub(crate) fn simulation_snapshot(
        &self,
    ) -> Option<crate::simulation::SimulationRuntimeSnapshot> {
        let simulation = self.simulation.as_ref()?;
        let mut snapshot = simulation.runtime_snapshot_for(&self.runtime);
        if let Some(speed) = self.timing.simulation_speed() {
            snapshot.status.time_scale = speed.get();
        }
        Some(snapshot)
    }

    /// Takes the `restart` request `mode`. A program that stopped, or that did not start, is
    /// loaded again through the program loader, when there is one, and started as a new process
    /// starts it: its logical time begins at zero. Any other program restarts in place, as
    /// `restart_with_retain_store` does. A program that starts after a stop can be held by the
    /// debugger again (specification 34, STATE-08).
    fn restart_or_reload(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        let after_stop = self.reload_required;
        // A failed load leaves `reload_required` set, so the next restart loads again.
        if !(after_stop && self.load_installed(mode)?) {
            self.restart_with_retain_store(mode)?;
        }
        if after_stop {
            if let Some(debug) = self.runtime.debug_control() {
                debug.rearm_after_stop();
            }
            self.reload_required = false;
        }
        Ok(())
    }

    /// Loads the installed program anew through the program loader and starts it in `mode`, as a
    /// new process starts it: its logical time begins at zero. False, with nothing changed, for a
    /// runner without a loader.
    fn load_installed(&mut self, mode: crate::RestartMode) -> Result<bool, RuntimeError> {
        let Some(loader) = self.program_loader.as_mut() else {
            return Ok(false);
        };
        let started = loader(&mut self.runtime, mode)?;
        self.cycle_interval = started.cycle_interval;
        self.timing = self.timing_of_loaded(started.simulation.is_some(), started.time_scale);
        self.simulation = started.simulation;
        self.hang_action = started.hang_action;
        self.program_origin = self.clock.now();
        self.runtime.reset_task_timing(Duration::ZERO);
        self.task_origin = Duration::ZERO;
        self.grid_restarted = true;
        Ok(true)
    }

    /// The cycle timing of a program loaded anew, from its configuration (specification 46, §6.1): on simulation
    /// time when it `simulates`, at its speed `time_scale`, which this resource's shared speed takes so that
    /// `config.set` still changes it (item 5), with simulation time beginning again at zero with the program, as
    /// in a new process (specification 34, STATE-08); otherwise on the wall clock.
    fn timing_of_loaded(&self, simulates: bool, time_scale: u32) -> CycleTiming {
        if !simulates {
            return CycleTiming::wall_clock();
        }
        let speed = self
            .timing
            .simulation_speed()
            .unwrap_or_else(|| SimulationSpeed::new(time_scale));
        speed.set(time_scale);
        CycleTiming::simulated(speed, self.simulation_metrics.clone())
    }

    /// Execute one cycle using the current clock time.
    pub fn tick(&mut self) -> Result<(), RuntimeError> {
        let now = self.clock.now();
        self.runtime.set_current_time(now);
        self.runtime.execute_cycle()
    }

    /// Spawn the runner in a dedicated OS thread.
    pub fn spawn(self, name: impl Into<String>) -> Result<ResourceHandle<C>, RuntimeError> {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let supervision = Arc::new(supervisor::ScanSupervision::new(
            state.clone(),
            last_error.clone(),
        ));
        let clock = self.clock.clone();
        let simulation_speed = self.timing.simulation_speed();
        let (cmd_tx, cmd_rx) = command_channel();
        let mut runner = self;
        runner.command_rx = Some(cmd_rx);
        // The exchange the program's `Hmi.Display` calls serve (specification 39, §18 Phase 5): the
        // runtime's own, or a new one that the runtime and the handle share.
        let hmi = runner.runtime.hmi_exchange().unwrap_or_else(|| {
            let exchange = Arc::new(crate::hmi::exchange::HmiExchange::new());
            runner.runtime.set_hmi_exchange(exchange.clone());
            exchange
        });
        let debug = debug_for_new_thread(&runner.runtime);

        let stop_thread = stop.clone();
        let state_thread = state.clone();
        let last_error_thread = last_error.clone();
        let supervision_thread = supervision.clone();

        let (id_tx, id_rx) = std::sync::mpsc::channel();
        let builder = thread::Builder::new()
            .name(name.into())
            .stack_size(SCAN_THREAD_STACK_BYTES);
        let thread_init_hook = runner.thread_init_hook.clone();
        let join = builder
            .spawn(move || {
                run_guarded_resource_thread(
                    thread_init_hook,
                    id_tx,
                    state_thread.clone(),
                    last_error_thread.clone(),
                    move || {
                        run_resource_loop(
                            runner,
                            stop_thread,
                            state_thread,
                            last_error_thread,
                            supervision_thread,
                        );
                    },
                );
            })
            .map_err(|err| RuntimeError::ThreadSpawn(err.to_string().into()))?;

        let thread_id = match id_rx.recv() {
            Ok(Ok(thread_id)) => thread_id,
            Ok(Err(err)) => {
                let _ = join.join();
                return Err(err);
            }
            Err(_) => join.thread().id(),
        };

        Ok(ResourceHandle {
            stop,
            state,
            last_error,
            supervision,
            thread_id,
            clock,
            join: Some(join),
            cmd_tx: cmd_tx.clone(),
            hmi,
            simulation_speed,
            debug,
        })
    }
}

/// The debugger of `runtime`, ready for a resource thread that is about to start. A stop that
/// ended an earlier thread no longer keeps the debugger from holding the scan, so a control that
/// outlives its resource is not left inert. This runs on the spawning thread, before any stop
/// request can reach the new thread (specification 11 section 6.9, "Operations while the debugger
/// holds the scan").
fn debug_for_new_thread(runtime: &CompiledRuntime) -> Option<DebugControl> {
    let debug = runtime.debug_control();
    if let Some(debug) = &debug {
        debug.rearm_after_stop();
    }
    debug
}

fn run_guarded_resource_thread(
    thread_init_hook: Option<Arc<dyn Fn() -> Result<(), RuntimeError> + Send + Sync>>,
    id_tx: std::sync::mpsc::Sender<Result<thread::ThreadId, RuntimeError>>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    body: impl FnOnce(),
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(hook) = thread_init_hook.as_ref()
            && let Err(err) = hook()
        {
            set_last_error(&last_error, err.clone());
            set_resource_state(&state, ResourceState::Faulted);
            let _ = id_tx.send(Err(err));
            return;
        }
        let _ = id_tx.send(Ok(thread::current().id()));
        body();
    }));

    if let Err(payload) = result {
        let err = RuntimeError::ResourcePanic(panic_payload_message(payload));
        set_last_error(&last_error, err.clone());
        set_resource_state(&state, ResourceState::Faulted);
        let _ = id_tx.send(Err(err));
    }
}
