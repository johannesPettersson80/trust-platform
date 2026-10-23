/// Handle to a running resource thread.
#[derive(Debug)]
pub struct ResourceHandle<C: Clock + Clone> {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    supervision: Arc<supervisor::ScanSupervision>,
    thread_id: thread::ThreadId,
    clock: C,
    join: Option<thread::JoinHandle<()>>,
    cmd_tx: std::sync::mpsc::SyncSender<ResourceCommand>,
    /// The exchange the resource's `Hmi.Display` calls serve (specification 39, §18 Phase 5).
    pub(crate) hmi: Arc<crate::hmi::exchange::HmiExchange>,
    /// The shared speed of the resource's simulation, when it runs on simulation time.
    simulation_speed: Option<SimulationSpeed>,
}

impl<C: Clock + Clone> ResourceHandle<C> {
    /// Cloneable control handle for external management.
    #[must_use]
    pub fn control(&self) -> ResourceControl<C> {
        ResourceControl {
            stop: self.stop.clone(),
            state: self.state.clone(),
            last_error: self.last_error.clone(),
            supervision: self.supervision.clone(),
            clock: self.clock.clone(),
            cmd_tx: self.cmd_tx.clone(),
            hmi: self.hmi.clone(),
            simulation_speed: self.simulation_speed.clone(),
        }
    }
    /// Signal the resource thread to stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.clock.wake();
    }

    /// Retrieve the last error if the resource faulted.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.last_error
            .lock()
            .expect("resource error poisoned")
            .clone()
    }

    /// Current resource state.
    #[must_use]
    pub fn state(&self) -> ResourceState {
        *self.state.lock().expect("resource state poisoned")
    }

    /// Whether hosted hang supervision found the scan thread inside a call that did not return.
    #[must_use]
    pub fn is_hung(&self) -> bool {
        self.supervision.is_hung()
    }

    /// Thread id for the running resource.
    #[must_use]
    pub fn thread_id(&self) -> thread::ThreadId {
        self.thread_id
    }

    /// Join the resource thread.
    pub fn join(&mut self) -> thread::Result<()> {
        if let Some(join) = self.join.take() {
            return join.join();
        }
        Ok(())
    }
}

/// Lightweight control handle for a running resource.
#[derive(Debug, Clone)]
pub struct ResourceControl<C: Clock + Clone> {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    supervision: Arc<supervisor::ScanSupervision>,
    clock: C,
    cmd_tx: std::sync::mpsc::SyncSender<ResourceCommand>,
    hmi: Arc<crate::hmi::exchange::HmiExchange>,
    simulation_speed: Option<SimulationSpeed>,
}

impl<C: Clock + Clone> ResourceControl<C> {
    /// The exchange between the screens and the program's `Hmi.Display` calls.
    #[must_use]
    pub fn hmi_exchange(&self) -> Arc<crate::hmi::exchange::HmiExchange> {
        self.hmi.clone()
    }

    /// The shared speed of this resource's simulation, when it runs on simulation time (specification 46, §6.1
    /// item 5). `config.set` changes the speed through it.
    #[must_use]
    pub fn simulation_speed(&self) -> Option<SimulationSpeed> {
        self.simulation_speed.clone()
    }

    /// Attaches the speed of a resource that runs on simulation time.
    #[must_use]
    pub fn with_simulation_speed(mut self, speed: SimulationSpeed) -> Self {
        self.simulation_speed = Some(speed);
        self
    }

    /// Create a lightweight stub control with a command receiver.
    ///
    /// Intended for debug/control IPC where no scheduler thread is running.
    pub fn stub(clock: C) -> (Self, std::sync::mpsc::Receiver<ResourceCommand>) {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Ready));
        let last_error = Arc::new(Mutex::new(None));
        let supervision = Arc::new(supervisor::ScanSupervision::new(
            state.clone(),
            last_error.clone(),
        ));
        let (cmd_tx, cmd_rx) = command_channel();
        (
            Self {
                stop,
                state,
                last_error,
                supervision,
                clock,
                cmd_tx,
                hmi: Arc::new(crate::hmi::exchange::HmiExchange::new()),
                simulation_speed: None,
            },
            cmd_rx,
        )
    }

    /// Create a test-only stub control with a specific state/error.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn stub_with_state(
        clock: C,
        initial_state: ResourceState,
        initial_error: Option<RuntimeError>,
    ) -> (Self, std::sync::mpsc::Receiver<ResourceCommand>) {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(initial_state));
        let last_error = Arc::new(Mutex::new(initial_error));
        let supervision = Arc::new(supervisor::ScanSupervision::new(
            state.clone(),
            last_error.clone(),
        ));
        let (cmd_tx, cmd_rx) = command_channel();
        (
            Self {
                stop,
                state,
                last_error,
                supervision,
                clock,
                cmd_tx,
                hmi: Arc::new(crate::hmi::exchange::HmiExchange::new()),
                simulation_speed: None,
            },
            cmd_rx,
        )
    }

    /// Mark the stub as hung, as hosted hang supervision does when the scan thread does not return.
    #[cfg(test)]
    pub(crate) fn mark_hung_for_test(&self) {
        self.supervision.set_hung_for_test(true);
    }

    /// Pause the runtime cycles.
    pub fn pause(&self) -> Result<(), CommandSendError> {
        self.send_command(ResourceCommand::Pause)?;
        self.clock.wake();
        Ok(())
    }

    /// Resume the runtime cycles.
    pub fn resume(&self) -> Result<(), CommandSendError> {
        self.send_command(ResourceCommand::Resume)?;
        self.clock.wake();
        Ok(())
    }

    /// Signal the resource thread to stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.clock.wake();
    }

    #[cfg(test)]
    pub(crate) fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Current resource state.
    #[must_use]
    pub fn state(&self) -> ResourceState {
        *self.state.lock().expect("resource state poisoned")
    }

    /// Whether hosted hang supervision found the scan thread inside a call that did not return.
    #[must_use]
    pub fn is_hung(&self) -> bool {
        self.supervision.is_hung()
    }

    /// Retrieve the last error if the resource faulted.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.last_error
            .lock()
            .expect("resource error poisoned")
            .clone()
    }

    /// Queue a command for the running resource without blocking. A full queue refuses the
    /// command with [`CommandSendError::Full`] and queues nothing (specification 11 section 6.9).
    pub fn send_command(&self, command: ResourceCommand) -> Result<(), CommandSendError> {
        try_queue(&self.cmd_tx, command)
    }
}

/// Shared global variables synchronized across multiple resources.
#[derive(Debug, Clone)]
pub struct SharedGlobals {
    names: Vec<SmolStr>,
    inner: Arc<Mutex<IndexMap<SmolStr, Value>>>,
}

impl SharedGlobals {
    /// Create a shared global set from a runtime snapshot.
    pub fn from_runtime(names: Vec<SmolStr>, runtime: &Runtime) -> Result<Self, RuntimeError> {
        let mut values = IndexMap::new();
        for name in &names {
            let value = runtime
                .storage()
                .get_global(name.as_ref())
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            values.insert(name.clone(), value.clone());
        }
        Ok(Self {
            names,
            inner: Arc::new(Mutex::new(values)),
        })
    }

    fn with_lock<T>(&self, f: impl FnOnce(&mut IndexMap<SmolStr, Value>) -> T) -> T {
        let mut guard = self.inner.lock().expect("shared globals poisoned");
        f(&mut guard)
    }

    /// Read a shared global value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Value> {
        self.with_lock(|globals| globals.get(name).cloned())
    }

    fn validate_destination(&self, runtime: &mut Runtime) -> Result<(), RuntimeError> {
        self.with_lock(|globals| {
            for name in &self.names {
                let value = globals
                    .get(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
                runtime.validate_global_write(name, value.clone())?;
            }
            Ok(())
        })
    }

    fn execute_cycle(&self, runtime: &mut Runtime) -> Result<(), RuntimeError> {
        if runtime.faulted() {
            return Err(RuntimeError::ResourceFaulted);
        }
        let result = self.with_lock(|globals| {
            self.sync_into_locked(globals, runtime)?;
            let cycle_result = runtime.execute_cycle();
            let publication_result = self.sync_from_locked(globals, runtime);
            // Keep the owning PLC error when publication fails as well.
            cycle_result.and(publication_result)
        });
        result.map_err(|error| runtime.scheduler_fault(error))
    }

    fn sync_into_locked(
        &self,
        globals: &IndexMap<SmolStr, Value>,
        runtime: &mut Runtime,
    ) -> Result<(), RuntimeError> {
        for name in &self.names {
            let value = globals
                .get(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            runtime.write_global(name, value.clone())?;
        }
        Ok(())
    }

    fn sync_from_locked(
        &self,
        globals: &mut IndexMap<SmolStr, Value>,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        for name in &self.names {
            let value = runtime
                .storage()
                .get_global(name.as_ref())
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            globals.insert(name.clone(), value.clone());
        }
        Ok(())
    }
}
