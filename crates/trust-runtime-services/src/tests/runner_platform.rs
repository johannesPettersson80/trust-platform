//! The platform of the resource loop's contract tests: access rights with an optional control
//! token, an audit sink that numbers its records, a wall clock that may be unset, fixed status
//! values, one platform operation, a hardware watchdog that counts its feeds, the Live Values
//! queues the loop answers the `io.*`, `set` and `var.*` operations from, and the checkpoint
//! store the loop's persister step programs.

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::time::Duration;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::deployment::{ControlMode, DeviceDescriptor};
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{
    CheckpointStorage, CheckpointStore, HardwareWatchdog, TimeSpan, WallClock,
};
use trust_runtime_core::retain::checkpoint::CheckpointFlash;
use trust_runtime_core::value::Value;

use super::runner_ports::{ms, TestWait};
use crate::access::{resolve_role, AccessRole, AuthFailure};
use crate::capabilities::LastReset;
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::ports::{
    AccessPort, AuditRecord, AuditSink, ExtensionDispatch, ProgramControl, StatusPort,
};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::runner::{EngineLiveValues, RunnerPlatform};
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
    /// The Live Values the runner applies and the shared `io.*`, `set` and `var.*` operations
    /// answer from: the queues and the published snapshot. The fixture replaces it with the one
    /// built from the prepared board probe.
    pub(crate) live_values: Rc<EngineLiveValues>,
    /// The checkpoint store the platform supplies, or `None` when it has none.
    pub(crate) checkpoint: Option<CheckpointStore>,
    /// The control mode the platform's deployment record carries, `production` by default.
    pub(crate) control_mode: ControlMode,
    /// The device descriptor the `capabilities` operation reports; a computer's by default.
    pub(crate) device_descriptor: DeviceDescriptor,
    /// The reset the platform's boot read; `None` for a computer, which starts as a new process.
    pub(crate) last_reset: Option<LastReset>,
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
            live_values: Rc::new(EngineLiveValues::empty()),
            checkpoint: None,
            control_mode: ControlMode::Production,
            device_descriptor: DeviceDescriptor::hosted(),
            last_reset: None,
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

    /// The test platform stands for the board fixture, not for the computer the tests run on.
    fn platform(&self) -> &'static str {
        "stm32f407"
    }

    /// A device without an operating system has no project ID; the runner reports its platform's
    /// value (specification 11, `status`).
    fn runtime_id(&self) -> &'static str {
        "stm32f407"
    }

    /// A device without an operating system keeps control after `shutdown` (spec 34 STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
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

    fn device_descriptor(&self) -> DeviceDescriptor {
        self.device_descriptor.clone()
    }

    fn last_reset(&self) -> Option<LastReset> {
        self.last_reset.clone()
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
    fn live_values(&self) -> &EngineLiveValues {
        &self.live_values
    }

    fn control_mode(&self) -> ControlMode {
        self.control_mode
    }

    fn idle(&mut self) {
        self.idle_passes.set(self.idle_passes.get() + 1);
        if let Some(wait) = &self.wake_during_idle {
            wait.wake();
        }
    }
}

impl IoQueuePort for TestPlatform {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.live_values.io_snapshot()
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live_values.enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live_values.force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.live_values.release_io(address);
    }
}

impl VariableQueuePort for TestPlatform {
    fn generation(&self) -> Generation {
        self.live_values.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.live_values.enqueue_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.live_values.force_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.live_values.release_global(name);
    }

    fn release_retain(&self, name: &str) {
        self.live_values.release_retain(name);
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.live_values.release_instance(instance, name);
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.live_values.forced_snapshot()
    }
}
