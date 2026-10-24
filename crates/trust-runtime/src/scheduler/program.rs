/// The program a resource's scan thread runs (specification 44 §8.8, §15): an interpreted program on
/// the bytecode engine until that engine is deleted, or a compiled program on the kernel. A restart
/// that loads the installed program may change the one for the other.
#[derive(Debug)]
pub enum HostProgram {
    /// A program on the bytecode engine.
    Engine(Box<Runtime>),
    /// A compiled program.
    Compiled(Box<CompiledRuntime>),
}

impl From<Runtime> for HostProgram {
    fn from(runtime: Runtime) -> Self {
        Self::Engine(Box::new(runtime))
    }
}

impl From<CompiledRuntime> for HostProgram {
    fn from(program: CompiledRuntime) -> Self {
        Self::Compiled(Box::new(program))
    }
}

impl HostProgram {
    /// The program on the bytecode engine, when it runs there.
    #[must_use]
    pub fn engine(&self) -> Option<&Runtime> {
        match self {
            Self::Engine(runtime) => Some(runtime.as_ref()),
            Self::Compiled(_) => None,
        }
    }

    /// The program on the bytecode engine, when it runs there.
    pub fn engine_mut(&mut self) -> Option<&mut Runtime> {
        match self {
            Self::Engine(runtime) => Some(runtime.as_mut()),
            Self::Compiled(_) => None,
        }
    }

    /// The compiled program, when the program is one.
    #[must_use]
    pub fn compiled(&self) -> Option<&CompiledRuntime> {
        match self {
            Self::Engine(_) => None,
            Self::Compiled(program) => Some(program.as_ref()),
        }
    }

    /// The compiled program, when the program is one.
    pub fn compiled_mut(&mut self) -> Option<&mut CompiledRuntime> {
        match self {
            Self::Engine(_) => None,
            Self::Compiled(program) => Some(program.as_mut()),
        }
    }

    /// The program's logical time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        match self {
            Self::Engine(runtime) => runtime.current_time(),
            Self::Compiled(program) => program.current_time(),
        }
    }

    /// Sets the logical time of the next scan.
    pub fn set_current_time(&mut self, now: Duration) {
        match self {
            Self::Engine(runtime) => runtime.set_current_time(now),
            Self::Compiled(program) => program.set_current_time(now),
        }
    }

    /// Runs one scan.
    ///
    /// # Errors
    ///
    /// The error of the scan, after its fault policy acted.
    pub fn execute_cycle(&mut self) -> Result<(), RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.execute_cycle(),
            Self::Compiled(program) => program.execute_cycle(),
        }
    }

    /// Restarts the program in place.
    ///
    /// # Errors
    ///
    /// The error of the restart.
    pub fn restart(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.restart(mode),
            Self::Compiled(program) => program.restart(mode),
        }
    }

    /// Restarts the program, warm with the values of its retain store.
    ///
    /// # Errors
    ///
    /// The error of the restart.
    pub fn restart_with_retain_store(
        &mut self,
        mode: crate::RestartMode,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.restart_with_retain_store(mode),
            Self::Compiled(program) => program.restart_with_retain_store(mode),
        }
    }

    /// Every task counts its next activation from `now`.
    pub fn reset_task_timing(&mut self, now: Duration) {
        match self {
            Self::Engine(runtime) => runtime.reset_task_timing(now),
            Self::Compiled(program) => program.reset_task_timing(now),
        }
    }

    /// The debugger's control, when one is set.
    #[must_use]
    pub fn debug_control(&self) -> Option<DebugControl> {
        match self {
            Self::Engine(runtime) => runtime.debug_control(),
            Self::Compiled(program) => program.debug_control(),
        }
    }

    /// The watchdog policy.
    #[must_use]
    pub fn watchdog_policy(&self) -> crate::watchdog::WatchdogPolicy {
        match self {
            Self::Engine(runtime) => runtime.watchdog_policy(),
            Self::Compiled(program) => program.watchdog_policy(),
        }
    }

    /// Takes the watchdog policy.
    pub fn set_watchdog_policy(&mut self, policy: crate::watchdog::WatchdogPolicy) {
        match self {
            Self::Engine(runtime) => runtime.set_watchdog_policy(policy),
            Self::Compiled(program) => program.set_watchdog_policy(policy),
        }
    }

    /// The fault policy.
    #[must_use]
    pub fn fault_policy(&self) -> crate::watchdog::FaultPolicy {
        match self {
            Self::Engine(runtime) => runtime.fault_policy(),
            Self::Compiled(program) => program.fault_policy(),
        }
    }

    /// Takes the fault policy.
    pub fn set_fault_policy(&mut self, policy: crate::watchdog::FaultPolicy) {
        match self {
            Self::Engine(runtime) => runtime.set_fault_policy(policy),
            Self::Compiled(program) => program.set_fault_policy(policy),
        }
    }

    pub(crate) fn effective_execution_deadline(&self) -> Option<std::time::Instant> {
        match self {
            Self::Engine(runtime) => runtime.effective_execution_deadline(),
            Self::Compiled(program) => program.effective_execution_deadline(),
        }
    }

    pub(crate) fn effective_output_commit_deadline(&self) -> Option<std::time::Instant> {
        match self {
            Self::Engine(runtime) => runtime.effective_output_commit_deadline(),
            Self::Compiled(program) => program.effective_output_commit_deadline(),
        }
    }

    pub(crate) fn set_execution_deadline(&mut self, deadline: Option<std::time::Instant>) {
        match self {
            Self::Engine(runtime) => runtime.set_execution_deadline(deadline),
            Self::Compiled(program) => program.set_execution_deadline(deadline),
        }
    }

    pub(crate) fn set_output_commit_deadline(&mut self, deadline: Option<std::time::Instant>) {
        match self {
            Self::Engine(runtime) => runtime.set_output_commit_deadline(deadline),
            Self::Compiled(program) => program.set_output_commit_deadline(deadline),
        }
    }

    #[cfg(test)]
    pub(crate) fn execution_deadline(&self) -> Option<std::time::Instant> {
        match self {
            Self::Engine(runtime) => runtime.execution_deadline(),
            Self::Compiled(program) => program.execution_deadline(),
        }
    }

    /// The time the debugger held scans in total.
    #[must_use]
    pub fn debug_watchdog_pause_elapsed(&self) -> std::time::Duration {
        match self {
            Self::Engine(runtime) => runtime.debug_watchdog_pause_elapsed(),
            Self::Compiled(program) => program.debug_watchdog_pause_elapsed(),
        }
    }

    /// Applies the watchdog's action for `error`.
    pub(crate) fn watchdog_fault(&mut self, error: RuntimeError) -> RuntimeError {
        match self {
            Self::Engine(runtime) => runtime.watchdog_fault(error),
            Self::Compiled(program) => program.watchdog_fault(error),
        }
    }

    /// Reports a physical watchdog expiration.
    pub fn watchdog_timeout(&mut self) -> RuntimeError {
        match self {
            Self::Engine(runtime) => runtime.watchdog_timeout(),
            Self::Compiled(program) => program.watchdog_timeout(),
        }
    }

    /// Reports a panic of the scan thread.
    pub fn resource_panic(&mut self, message: impl Into<SmolStr>) -> RuntimeError {
        match self {
            Self::Engine(runtime) => runtime.resource_panic(message),
            Self::Compiled(program) => program.resource_panic(message),
        }
    }

    /// Reports a simulation fault.
    pub fn simulation_fault(&mut self, message: impl Into<SmolStr>) -> RuntimeError {
        match self {
            Self::Engine(runtime) => runtime.simulation_fault(message),
            Self::Compiled(program) => program.simulation_fault(message),
        }
    }

    /// Writes `value` at the process-image `address` between scans.
    ///
    /// # Errors
    ///
    /// The refusal of an address or a value the image does not take.
    pub fn write_io(
        &mut self,
        address: &crate::io::IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.write_io(address, value),
            Self::Compiled(program) => program.write_io(address, value),
        }
    }

    /// The value at the process-image `address`.
    ///
    /// # Errors
    ///
    /// The refusal of an address the image does not hold.
    pub fn read_io(&self, address: &crate::io::IoAddress) -> Result<Value, RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.io().read(address),
            Self::Compiled(program) => program.read_io(address),
        }
    }

    /// Drives the safe outputs.
    ///
    /// # Errors
    ///
    /// The first failure among the drivers.
    pub fn apply_io_safe_state(&mut self) -> Result<(), RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.apply_io_safe_state(),
            Self::Compiled(program) => program.apply_io_safe_state(),
        }
    }

    /// Saves the retained values.
    ///
    /// # Errors
    ///
    /// The error of a save that did not complete.
    pub fn save_retain_store(&mut self) -> Result<(), RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.save_retain_store(),
            Self::Compiled(program) => program.save_retain_store(),
        }
    }

    /// Takes the safe outputs of `[[io.safe_state]]`.
    pub fn set_io_safe_state(&mut self, state: crate::io::IoSafeState) {
        match self {
            Self::Engine(runtime) => runtime.set_io_safe_state(state),
            Self::Compiled(program) => program.set_io_safe_state(state),
        }
    }

    /// The retained values' save interval.
    #[must_use]
    pub fn retain_save_interval(&self) -> Option<Duration> {
        match self {
            Self::Engine(runtime) => runtime.retain_save_interval(),
            Self::Compiled(program) => program.retain_save_interval(),
        }
    }

    #[cfg(test)]
    pub(crate) fn output_commit_deadline(&self) -> Option<std::time::Instant> {
        match self {
            Self::Engine(runtime) => runtime.output_commit_deadline(),
            Self::Compiled(program) => program.output_commit_deadline(),
        }
    }

    /// Saves the retained values at most every `interval`.
    pub fn set_retain_save_interval(&mut self, interval: Option<Duration>) {
        match self {
            Self::Engine(runtime) => runtime.set_retain_save_interval(interval),
            Self::Compiled(program) => program.set_retain_save_interval(interval),
        }
    }

    /// Whether a fault is latched.
    #[must_use]
    pub fn faulted(&self) -> bool {
        match self {
            Self::Engine(runtime) => runtime.faulted(),
            Self::Compiled(program) => program.faulted(),
        }
    }

    /// The values of the globals `names` names.
    #[must_use]
    pub fn snapshot_globals(&self, names: &[SmolStr]) -> IndexMap<SmolStr, Value> {
        match self {
            Self::Engine(runtime) => runtime.snapshot_globals(names),
            Self::Compiled(program) => program.snapshot_globals(names),
        }
    }

    /// Writes each of `updates` into the global its name names.
    pub fn apply_mesh_updates(&mut self, updates: &IndexMap<SmolStr, Value>) {
        match self {
            Self::Engine(runtime) => runtime.apply_mesh_updates(updates),
            Self::Compiled(program) => program.apply_mesh_updates(updates),
        }
    }

    /// Changes the program online to the program of `bytes`. A program on the bytecode engine
    /// answers its new metadata; a compiled one answers none.
    ///
    /// # Errors
    ///
    /// The refusal of the change; the running program is then as it was.
    pub fn apply_online_change_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<Option<RuntimeMetadata>, RuntimeError> {
        match self {
            Self::Engine(runtime) => runtime.apply_online_change_bytes(bytes, None).map(Some),
            Self::Compiled(program) => program.apply_online_change_bytes(bytes).map(|()| None),
        }
    }
}
