//! The platform of the resource loop's contract tests: access rights with an optional control
//! token, an audit sink that numbers its records, a wall clock that may be unset, fixed status
//! values, one platform operation and a hardware watchdog that counts its feeds.

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::time::Duration;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::ports::{
    CheckpointStorage, CheckpointStore, HardwareWatchdog, TimeSpan, WallClock,
};
use trust_runtime_core::retain::checkpoint::CheckpointFlash;

use super::runner_ports::{ms, TestWait};
use crate::access::{resolve_role, AccessRole, AuthFailure};
use crate::ports::{AccessPort, AuditRecord, AuditSink, ExtensionDispatch, StatusPort};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::runner::RunnerPlatform;
use crate::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

/// The longest wait between two feeds of the hardware watchdog, unless a test sets its own.
pub(crate) const FEED_INTERVAL_MS: u64 = 1_000;

/// The test platform.
pub(crate) struct TestPlatform {
    pub(crate) control_token: Option<&'static str>,
    pub(crate) audits: RefCell<Vec<(String, bool)>>,
    pub(crate) utc_ms: Option<u64>,
    pub(crate) idle_passes: Rc<Cell<u32>>,
    /// A wake that arrives while the loop does its idle work.
    pub(crate) wake_during_idle: Option<TestWait>,
    pub(crate) extension_calls: RefCell<Vec<String>>,
    pub(crate) feed_interval: Duration,
    /// The watchdog's feeds; a test keeps a clone to read them.
    pub(crate) feeds: Rc<Cell<u32>>,
    /// The checkpoint store the platform supplies, or `None` when it has none.
    pub(crate) checkpoint: Option<CheckpointStore>,
}

impl Default for TestPlatform {
    fn default() -> Self {
        Self {
            control_token: None,
            audits: RefCell::new(Vec::new()),
            utc_ms: None,
            idle_passes: Rc::new(Cell::new(0)),
            wake_during_idle: None,
            extension_calls: RefCell::new(Vec::new()),
            feed_interval: ms(FEED_INTERVAL_MS),
            feeds: Rc::new(Cell::new(0)),
            checkpoint: None,
        }
    }
}

impl TestPlatform {
    /// The platform's checkpoint store is `flash`, which one persister step may program up to
    /// `program_budget` bytes of (specification 34 §10).
    pub(crate) fn set_checkpoint(
        &mut self,
        flash: Box<dyn CheckpointFlash>,
        program_budget: usize,
    ) {
        self.checkpoint = Some(CheckpointStore {
            flash,
            program_budget,
        });
    }
}

impl AccessPort for TestPlatform {
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        resolve_role(
            request.auth.as_deref(),
            self.control_token,
            false,
            client,
            |_| None,
        )
    }

    fn debug_enabled(&self) -> bool {
        true
    }
}

impl AuditSink for TestPlatform {
    fn record_audit(&self, record: AuditRecord<'_>) -> Option<SmolStr> {
        let mut audits = self.audits.borrow_mut();
        audits.push((String::from(record.request_type.as_str()), record.ok));
        Some(SmolStr::new(format!("audit-{}", audits.len())))
    }
}

impl WallClock for TestPlatform {
    fn utc_ms(&self) -> Option<u64> {
        self.utc_ms
    }
}

impl StatusPort for TestPlatform {
    fn resource_name(&self) -> &'static str {
        "Controller"
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        Some(StatusSettings {
            execution_backend: "vm",
            execution_backend_source: "default",
            simulation_enabled: false,
            simulation_time_scale: 1,
            simulation_mode: SmolStr::new("production"),
            simulation_warning: SmolStr::default(),
        })
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        Some(Vec::new())
    }

    /// A device has no realtime profile, the block every runtime reports without one.
    fn realtime_status(&self) -> Option<serde_json::Value> {
        Some(absent_realtime_profile())
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        Some(false)
    }

    /// A device has no `OpenOT` persistence, the block every runtime reports without it.
    fn openot_persistence(&self) -> Option<serde_json::Value> {
        Some(disabled_openot_persistence())
    }
}

impl ExtensionDispatch for TestPlatform {
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse> {
        self.extension_calls
            .borrow_mut()
            .push(request.r#type.clone());
        (request.r#type == "config.get")
            .then(|| ControlResponse::ok(request.id, json!({"answered_by": "platform"})))
    }

    fn audit_details(&self, _request: &ControlRequest) -> Option<serde_json::Value> {
        None
    }
}

impl HardwareWatchdog for TestPlatform {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(self.feed_interval)
    }

    fn feed(&mut self) {
        self.feeds.set(self.feeds.get() + 1);
    }
}

impl CheckpointStorage for TestPlatform {
    fn take_checkpoint_store(&mut self) -> Option<CheckpointStore> {
        self.checkpoint.take()
    }
}

impl RunnerPlatform for TestPlatform {
    fn idle(&mut self) {
        self.idle_passes.set(self.idle_passes.get() + 1);
        if let Some(wait) = &self.wake_during_idle {
            wait.wake();
        }
    }
}
