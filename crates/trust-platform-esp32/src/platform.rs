//! The resource loop's platform on the C6 (specification 34, ARCH-11 and §3.3): the values the
//! chip reports, the access rights of its USB control link, the Live Values the loop applies, and
//! its hardware watchdog.
//!
//! A USB client is local (specification 11 §6.9.5), so the link carries no transport label and
//! no control token is configured yet (TARGET-02 keeps secrets off the project); a local client
//! therefore acts as `Admin`, exactly as trust-runtime's local socket without a configured token.
//! The chip has no wall clock until slice T6, no audit store and no platform operation yet, so
//! those ports answer as an absent value does, not with an invented one. The chip answers the
//! `io.*`, `set` and `var.*` operations from its own Live Values, the queues the runner applies
//! and the snapshot it publishes, as trust-runtime's `ControlState` answers them
//! (specification 11, "Live Values"; specification 34 §4.3.1).

use alloc::string::String;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::{ControlMode, DeviceDescriptor};
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{HardwareWatchdog, TimeSpan, WallClock};
use trust_runtime_core::value::Value;
use trust_runtime_services::access::{resolve_role, AccessRole, AuthFailure};
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use trust_runtime_services::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, PendingVarTarget,
};
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::live_values::RequestQueueFull;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, ExtensionDispatch, ProgramControl, StatusPort,
};
use trust_runtime_services::protocol::{ControlRequest, ControlResponse};
use trust_runtime_services::runner::{EngineLiveValues, RunnerPlatform};
use trust_runtime_services::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

use crate::profile::{DEVICE_DESCRIPTOR, PLATFORM, RESET_WATCHDOG};
use crate::{ResetCause, ResetRecord};

/// The C6's platform for the single-thread resource loop, over the watchdog `W` the adapter
/// supplies, the values the installed application's record configures and the Live Values of the
/// installed program.
pub struct Platform<W> {
    watchdog: W,
    resource_name: SmolStr,
    settings: StatusSettings,
    debug_enabled: bool,
    /// `runtime.control.mode` of the installed application's deployment record (specification 34
    /// §5.1.1, section 7): the mode `status` reports and the path `pause` and `resume` take.
    control_mode: ControlMode,
    /// The Live Values the runner applies at its cycle phases and the `io.*`, `set` and `var.*`
    /// operations answer from (specification 11, "Live Values"; specification 34 §4.3.1).
    live: EngineLiveValues,
    /// The reset the boot reports, for the `capabilities` reply (specification 11, the runtime
    /// device capabilities contract), built by [`Platform::boot_last_reset`].
    last_reset: LastReset,
    /// The chip's stable runtime ID that `status` reports, from [`crate::profile::runtime_id`].
    runtime_id: SmolStr,
}

impl<W> Platform<W> {
    /// A platform over `watchdog` that reports `resource_name`, the runtime `settings`, whether
    /// the debug surface is `debug_enabled` and the record's `control_mode`, with the Live Values
    /// `live` of the installed program, or [`EngineLiveValues::empty`] for a chip without one, and
    /// the `last_reset` the boot reports, under the chip's stable `runtime_id`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        watchdog: W,
        resource_name: SmolStr,
        settings: StatusSettings,
        debug_enabled: bool,
        control_mode: ControlMode,
        live: EngineLiveValues,
        last_reset: LastReset,
        runtime_id: SmolStr,
    ) -> Self {
        Self {
            watchdog,
            resource_name,
            settings,
            debug_enabled,
            control_mode,
            live,
            last_reset,
            runtime_id,
        }
    }

    /// The reset the boot reports (specification 11, the runtime device capabilities contract).
    /// Its cause and text are the record the previous boot left. When it left none, the boot's
    /// hardware reset reason `reason` decides: `RESET_WATCHDOG` is a watchdog reset, which runs no
    /// code and so writes no record (specification 34, the C6's supervision), and any other reason
    /// is `power_on`. `boot_count` is the chip's boot counter for this boot, reported for every
    /// reset, record or not.
    #[must_use]
    pub fn boot_last_reset(reason: u8, record: Option<ResetRecord>, boot_count: u32) -> LastReset {
        match record {
            Some(record) => LastReset {
                cause: reset_cause(record.cause()),
                text: SmolStr::new(String::from_utf8_lossy(record.text()).as_ref()),
                boot_count,
            },
            None => LastReset {
                cause: if reason == RESET_WATCHDOG {
                    ResetCauseName::Watchdog
                } else {
                    ResetCauseName::PowerOn
                },
                text: SmolStr::default(),
                boot_count,
            },
        }
    }
}

impl<W> AccessPort for Platform<W> {
    /// A chip has no configured control token yet, so a local client (no transport label) acts as
    /// `Admin`; a network label would be a `Viewer`, as on trust-runtime.
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        resolve_role(request.auth.as_deref(), None, false, client, |_| None)
    }

    fn debug_enabled(&self) -> bool {
        self.debug_enabled
    }
}

impl<W> AuditSink for Platform<W> {
    /// A chip has no audit store, so no record id is reported.
    fn record_audit(&self, _record: AuditRecord<'_>) -> Option<SmolStr> {
        None
    }
}

/// A device whose clock nobody has set yet (specification 34, TARGET-06).
impl<W> WallClock for Platform<W> {
    fn utc_ms(&self) -> Option<u64> {
        None
    }
}

impl<W> StatusPort for Platform<W> {
    fn resource_name(&self) -> &str {
        self.resource_name.as_str()
    }

    /// The chip's name (specification 11, `status`).
    fn platform(&self) -> &str {
        PLATFORM
    }

    /// The chip's name and USB serial number (specification 11, `status`).
    fn runtime_id(&self) -> &str {
        self.runtime_id.as_str()
    }

    /// A chip keeps control after `shutdown`: the program takes the place of the process
    /// (specification 34, STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        Some(self.settings.clone())
    }

    /// The installed program's own drivers are reported by the cycle; the platform runs none.
    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        Some(Vec::new())
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        Some(absent_realtime_profile())
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        Some(false)
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        Some(disabled_openot_persistence())
    }

    /// The C6's descriptor, the same admission reads (specification 34 §5.1.1, COMPAT-02 and
    /// TARGET-03).
    fn device_descriptor(&self) -> DeviceDescriptor {
        DEVICE_DESCRIPTOR
    }

    /// The reset the boot reported (specification 11, the runtime device capabilities contract).
    fn last_reset(&self) -> Option<LastReset> {
        Some(self.last_reset.clone())
    }
}

/// The wire name of the reset record's cause (specification 11, the runtime device capabilities
/// contract).
fn reset_cause(cause: ResetCause) -> ResetCauseName {
    match cause {
        ResetCause::Panic => ResetCauseName::Panic,
        ResetCause::Watchdog => ResetCauseName::Watchdog,
        ResetCause::Requested => ResetCauseName::Requested,
    }
}

/// The chip answers no platform operation yet.
impl<W> ExtensionDispatch for Platform<W> {
    fn dispatch_extension(&self, _request: &ControlRequest) -> Option<ControlResponse> {
        None
    }

    fn audit_details(&self, _request: &ControlRequest) -> Option<serde_json::Value> {
        None
    }
}

impl<W: HardwareWatchdog> HardwareWatchdog for Platform<W> {
    fn feed_interval(&self) -> TimeSpan {
        self.watchdog.feed_interval()
    }

    fn feed(&mut self) {
        self.watchdog.feed();
    }
}

impl<W: HardwareWatchdog> RunnerPlatform for Platform<W> {
    fn live_values(&self) -> &EngineLiveValues {
        &self.live
    }

    /// The installed application's record carries the mode (specification 34 §5.1.1, section 7).
    fn control_mode(&self) -> ControlMode {
        self.control_mode
    }
}

/// The platform forwards the `io.*`, `set` and `var.*` operations to the Live Values of the
/// installed program, as trust-runtime's `ControlState` forwards them.
impl<W> IoQueuePort for Platform<W> {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.live.io_snapshot()
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live.enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live.force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.live.release_io(address);
    }
}

impl<W> VariableQueuePort for Platform<W> {
    fn generation(&self) -> Generation {
        self.live.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.live.enqueue_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.live.force_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.live.release_global(name);
    }

    fn release_retain(&self, name: &str) {
        self.live.release_retain(name);
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.live.release_instance(instance, name);
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.live.forced_snapshot()
    }
}
