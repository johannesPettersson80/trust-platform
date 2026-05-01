/// Commands applied to a running resource.
#[derive(Debug, Clone)]
pub enum ResourceCommand {
    Pause,
    Resume,
    UpdateWatchdog(crate::watchdog::WatchdogPolicy),
    UpdateFaultPolicy(crate::watchdog::FaultPolicy),
    UpdateRetainSaveInterval(Option<Duration>),
    UpdateIoSafeState(crate::io::IoSafeState),
    ReloadBytecode {
        bytes: Vec<u8>,
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
