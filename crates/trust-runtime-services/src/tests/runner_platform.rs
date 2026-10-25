//! The platform of the resource loop's contract tests: access rights with an optional control
//! token, an audit sink that numbers its records, a wall clock that may be unset, fixed status
//! values, one platform operation, a hardware watchdog that counts its feeds, the Live Values
//! queues the loop answers the `io.*`, `set` and `var.*` operations from, and the checkpoint
//! store the loop's persister step programs.

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
    CheckpointStorage, CheckpointStore, HardwareWatchdog, MonotonicClock, TimeSpan, WallClock,
};
use trust_runtime_core::value::Value;

use super::runner_ports::{ms, TestClock, TestWait};
use crate::access::{resolve_role, AccessRole, AuthFailure};
use crate::capabilities::LastReset;
use crate::chip::watchdog::watchdog_timeout_ms;
use crate::device_settings::{DeviceInterfaces, DeviceSettings};
use crate::firmware::FirmwareFacts;
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::ports::{
    AccessPort, AuditRecord, AuditSink, DevicePort, ExtensionDispatch, ProgramControl, StatusPort,
};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::runner::{DeviceLiveValues, RunnerPlatform};
use crate::settings::{RuntimeSettings, TokenChange};
use crate::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

/// The longest wait between two feeds of the hardware watchdog, unless a test sets its own.
pub(crate) const FEED_INTERVAL_MS: u64 = 1_000;

/// The feeds after which the test platform stops a loop that never waits: no test feeds the
/// watchdog so often, so a loop that does has spun.
const MAX_FEEDS: u32 = 100_000;

/// A test chip's stored device settings with their sequence.
pub(crate) type StoredSettings = RefCell<Option<(u32, DeviceSettings)>>;

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
    /// The times on the clock at which the watchdog was fed, once the fixture has given the
    /// platform its clock; a test keeps a clone to read them.
    pub(crate) feed_times: Rc<RefCell<Vec<Duration>>>,
    /// The clock that stamps the feeds.
    pub(crate) clock: Option<TestClock>,
    /// Whether the platform enables the debug surface (specification 11 §6.9).
    pub(crate) debug_enabled: bool,
    /// The Live Values the runner applies and the shared `io.*`, `set` and `var.*` operations
    /// answer from: the queues and the published snapshot. The fixture replaces it with the one
    /// bound to the board probe's image.
    pub(crate) live_values: DeviceLiveValues,
    /// The checkpoint store the platform supplies, or `None` when it has none.
    pub(crate) checkpoint: Option<CheckpointStore>,
    /// The control mode the platform's deployment record carries, `production` by default.
    pub(crate) control_mode: ControlMode,
    /// The device descriptor the `capabilities` operation reports; a computer's by default.
    pub(crate) device_descriptor: DeviceDescriptor,
    /// The reset the platform's boot read; `None` for a computer, which starts as a new process.
    pub(crate) last_reset: Option<LastReset>,
    /// What a chip knows of its firmware slots; `None` for a computer.
    pub(crate) firmware: Option<FirmwareFacts>,
    /// A chip's device settings: its interfaces and the stored record; `None` for a computer.
    pub(crate) device: Option<(DeviceInterfaces, StoredSettings)>,
    /// How many times the loop reset the device; a test keeps a clone to read them.
    pub(crate) resets: Rc<Cell<u32>>,
    /// How many times the loop dropped the program that ended (`unload_program`); a test keeps a
    /// clone to read them.
    pub(crate) unloads: Rc<Cell<u32>>,
    /// The settings of the program's record that the loop answers `config.get` and `config.set`
    /// from; `None`, the default, leaves both to the platform. When set, they give the control mode
    /// and the debug surface.
    pub(crate) runtime: Option<RuntimeSettings>,
    /// The control token a `config.set` gave the platform, which then replaces `control_token`.
    pub(crate) committed_token: Option<SmolStr>,
    /// The longest interval the platform's hardware watchdog takes, in milliseconds.
    pub(crate) max_watchdog_ms: u32,
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
            feed_times: Rc::new(RefCell::new(Vec::new())),
            clock: None,
            debug_enabled: true,
            live_values: DeviceLiveValues::empty(),
            checkpoint: None,
            control_mode: ControlMode::Production,
            device_descriptor: DeviceDescriptor::hosted(),
            last_reset: None,
            firmware: None,
            device: None,
            resets: Rc::new(Cell::new(0)),
            unloads: Rc::new(Cell::new(0)),
            runtime: None,
            committed_token: None,
            max_watchdog_ms: u32::MAX,
        }
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
            self.committed_token.as_deref().or(self.control_token),
            false,
            client,
            |_| None,
        )
    }

    fn debug_enabled(&self) -> bool {
        self.runtime
            .map_or(self.debug_enabled, |settings| settings.debug_enabled)
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

    /// The test platform stands for a chip, not for the computer the tests run on.
    fn platform(&self) -> &'static str {
        "esp32c6"
    }

    /// A device without an operating system has no project ID; the runner reports its platform's
    /// value (specification 11, `status`).
    fn runtime_id(&self) -> &'static str {
        "esp32c6"
    }

    /// A device without an operating system keeps control after `shutdown` (spec 34 STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
    }

    /// A chip's settings: compiled programs, production control and no simulation.
    fn status_settings(&self) -> Option<StatusSettings> {
        Some(crate::chip::platform::status_settings())
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

    fn firmware(&self) -> Option<FirmwareFacts> {
        self.firmware.clone()
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
        let feeds = self.feeds.get() + 1;
        assert!(
            feeds <= MAX_FEEDS,
            "the loop fed the watchdog {MAX_FEEDS} times: it spins instead of waiting"
        );
        self.feeds.set(feeds);
        if let Some(clock) = &self.clock {
            self.feed_times
                .borrow_mut()
                .push(clock.now().since_origin());
        }
    }
}

impl CheckpointStorage for TestPlatform {
    fn take_checkpoint_store(&mut self) -> Option<CheckpointStore> {
        self.checkpoint.take()
    }
}

impl DevicePort for TestPlatform {
    fn device_interfaces(&self) -> Option<DeviceInterfaces> {
        self.device.as_ref().map(|(interfaces, _)| *interfaces)
    }

    fn device_settings(&self) -> Result<Option<(u32, DeviceSettings)>, String> {
        Ok(self
            .device
            .as_ref()
            .and_then(|(_, stored)| stored.borrow().clone()))
    }

    fn store_device_settings(&self, settings: &DeviceSettings) -> Result<u32, String> {
        let (_, stored) = self
            .device
            .as_ref()
            .ok_or_else(|| String::from("unsupported request"))?;
        let sequence = stored
            .borrow()
            .as_ref()
            .map_or(1, |(sequence, _)| sequence + 1);
        *stored.borrow_mut() = Some((sequence, settings.clone()));
        Ok(sequence)
    }
}

impl RunnerPlatform for TestPlatform {
    fn live_values(&self) -> &DeviceLiveValues {
        &self.live_values
    }

    fn reset_device(&mut self) {
        self.resets.set(self.resets.get() + 1);
    }

    fn control_mode(&self) -> ControlMode {
        self.runtime
            .map_or(self.control_mode, |settings| settings.control_mode)
    }

    fn runtime_settings(&self) -> Option<RuntimeSettings> {
        self.runtime
    }

    fn control_token_length(&self) -> Option<usize> {
        self.committed_token
            .as_ref()
            .map(SmolStr::len)
            .or(self.control_token.map(str::len))
    }

    fn takes_cycle_timeout_ms(&self, cycle_timeout_ms: u32) -> bool {
        watchdog_timeout_ms(cycle_timeout_ms) <= self.max_watchdog_ms
    }

    fn commit_settings(&mut self, settings: RuntimeSettings, token: TokenChange) {
        self.runtime = Some(settings);
        token.apply_to(&mut self.committed_token);
    }

    fn idle(&mut self) {
        self.idle_passes.set(self.idle_passes.get() + 1);
        if let Some(wait) = &self.wake_during_idle {
            wait.wake();
        }
    }

    fn unload_program(&mut self) {
        self.unloads.set(self.unloads.get() + 1);
        self.live_values = DeviceLiveValues::empty();
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
