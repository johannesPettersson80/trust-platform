/// Handle to a running resource thread.
#[derive(Debug)]
pub struct ResourceHandle<C: Clock + Clone> {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    thread_id: thread::ThreadId,
    clock: C,
    join: Option<thread::JoinHandle<()>>,
    cmd_tx: std::sync::mpsc::Sender<ResourceCommand>,
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
    clock: C,
    cmd_tx: std::sync::mpsc::Sender<ResourceCommand>,
    hmi: Arc<crate::hmi::exchange::HmiExchange>,
    simulation_speed: Option<SimulationSpeed>,
}

impl<C: Clock + Clone> ResourceControl<C> {
    /// The exchange between the screens and the program's `Hmi.Display` calls.
    #[must_use]
    pub fn hmi_exchange(&self) -> Arc<crate::hmi::exchange::HmiExchange> {
        self.hmi.clone()
    }

    /// The shared speed of this resource's simulation, when it runs on simulation time (specification 33, §6.1
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
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        (
            Self {
                stop,
                state,
                last_error,
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
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        (
            Self {
                stop,
                state,
                last_error,
                clock,
                cmd_tx,
                hmi: Arc::new(crate::hmi::exchange::HmiExchange::new()),
                simulation_speed: None,
            },
            cmd_rx,
        )
    }
    /// Pause the runtime cycles.
    pub fn pause(&self) -> Result<(), RuntimeError> {
        self.send_command(ResourceCommand::Pause)?;
        self.clock.wake();
        Ok(())
    }

    /// Resume the runtime cycles.
    pub fn resume(&self) -> Result<(), RuntimeError> {
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

    /// Retrieve the last error if the resource faulted.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.last_error
            .lock()
            .expect("resource error poisoned")
            .clone()
    }

    /// Send a command to the running resource.
    pub fn send_command(&self, command: ResourceCommand) -> Result<(), RuntimeError> {
        self.cmd_tx
            .send(command)
            .map_err(|_| RuntimeError::ControlError("command channel closed".into()))
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

    fn sync_into_locked(
        &self,
        globals: &IndexMap<SmolStr, Value>,
        runtime: &mut Runtime,
    ) -> Result<(), RuntimeError> {
        for name in &self.names {
            let value = globals
                .get(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            runtime
                .storage_mut()
                .set_global(name.clone(), value.clone());
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
