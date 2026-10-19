// The scheduler's policy set, which `config.set` changes as one command, is shared runtime code
// (spec 34 ARCH-11; specification 11 section 6.8.1).
pub use trust_runtime_services::resource::commands::SchedulerPolicies;

/// Commands applied to a running resource.
#[derive(Debug, Clone)]
pub enum ResourceCommand {
    Pause,
    Resume,
    UpdateSchedulerPolicies(SchedulerPolicies),
    UpdateIoSafeState(crate::io::IoSafeState),
    ReloadBytecode {
        bytes: Vec<u8>,
        /// Shared with the waiting request, which withdraws the reload if it times out first.
        claim: ReloadClaim,
        respond_to: std::sync::mpsc::Sender<Result<RuntimeMetadata, RuntimeError>>,
    },
    MeshSnapshot {
        names: Vec<SmolStr>,
        respond_to: std::sync::mpsc::Sender<IndexMap<SmolStr, Value>>,
    },
    MeshApply {
        updates: IndexMap<SmolStr, Value>,
        source: Option<SmolStr>,
        sequence: Option<u64>,
    },
    Snapshot {
        respond_to: std::sync::mpsc::Sender<crate::debug::DebugSnapshot>,
    },
    AdsStatus {
        respond_to: std::sync::mpsc::Sender<crate::ads::diagnostics::AdsStatusReport>,
    },
    OpcUaClientStatus {
        respond_to: std::sync::mpsc::Sender<crate::opcua::OpcUaClientStatusReport>,
    },
    ActiveAdsDevice {
        target: crate::ads::diagnostics::TargetIdentity,
        local: Option<crate::ads::diagnostics::LocalIdentity>,
        respond_to:
            std::sync::mpsc::Sender<Option<crate::ads::onboarding::ActiveAdsDeviceSnapshot>>,
    },
}

/// What a program start leaves to the scan thread: the scheduling settings of the program's own
/// configuration, which the thread takes over (specification 34, STATE-08).
pub struct StartedProgram {
    /// The cycle interval of the program's configuration.
    pub cycle_interval: Duration,
    /// The simulation time scale of the program's configuration, at least 1.
    pub time_scale: u32,
    /// The simulation of the program's configuration, when it has one.
    pub simulation: Option<crate::simulation::SimulationController>,
    /// What hosted hang supervision does after it records a hung scan thread.
    pub hang_action: crate::watchdog::HangAction,
}

/// Loads the installed program into the runtime slot and starts it in the given mode, as a new
/// process loads and starts its program (specification 34, STATE-08).
///
/// The loader reads the program first and changes the slot only once that has succeeded. It drops
/// the program that stopped, with its I/O drivers, before it builds the drivers of the new one.
/// When it fails the slot holds either the program that stopped or a new program that did not
/// start, so the next `restart` loads the program again.
pub type ProgramLoader =
    Box<dyn FnMut(&mut Runtime, crate::RestartMode) -> Result<StartedProgram, RuntimeError> + Send>;

/// The kind of work `command` asks for; the shared supervisor rules decide what each kind does
/// while running and while faulted.
fn command_class(command: &ResourceCommand) -> CommandClass {
    match command {
        ResourceCommand::Pause => CommandClass::Pause,
        ResourceCommand::Resume => CommandClass::Resume,
        ResourceCommand::ReloadBytecode { .. } => CommandClass::Reload,
        ResourceCommand::MeshApply { .. } => CommandClass::VariableWrite,
        ResourceCommand::UpdateSchedulerPolicies(_)
        | ResourceCommand::UpdateIoSafeState(_)
        | ResourceCommand::MeshSnapshot { .. }
        | ResourceCommand::Snapshot { .. }
        | ResourceCommand::AdsStatus { .. }
        | ResourceCommand::OpcUaClientStatus { .. }
        | ResourceCommand::ActiveAdsDevice { .. } => CommandClass::Service,
    }
}

/// Gate that blocks resource execution until opened.
#[derive(Debug, Default)]
pub struct StartGate {
    open: Mutex<bool>,
    cvar: Condvar,
}

impl StartGate {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: Mutex::new(false),
            cvar: Condvar::new(),
        }
    }

    pub fn open(&self) {
        let mut guard = self.open.lock().expect("start gate lock poisoned");
        *guard = true;
        self.cvar.notify_all();
    }

    fn wait_open(&self, stop: &AtomicBool) -> bool {
        let mut guard = self.open.lock().expect("start gate lock poisoned");
        while !*guard {
            if stop.load(Ordering::SeqCst) {
                return false;
            }
            let (next, _) = self
                .cvar
                .wait_timeout(guard, std::time::Duration::from_millis(50))
                .expect("start gate wait poisoned");
            guard = next;
        }
        true
    }
}
