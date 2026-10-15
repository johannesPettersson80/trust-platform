/// The scheduler's copies of the watchdog, fault and retain-save policies, which `config.set`
/// changes. They travel in one command, so a full queue refuses the whole set and the scan thread
/// applies the whole set as one step (specification 11 section 6.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerPolicies {
    pub watchdog: crate::watchdog::WatchdogPolicy,
    pub fault_policy: crate::watchdog::FaultPolicy,
    pub retain_save_interval: Option<Duration>,
}

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
