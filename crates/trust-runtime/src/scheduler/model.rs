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
