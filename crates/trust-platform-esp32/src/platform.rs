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

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{HardwareWatchdog, TimeSpan, WallClock};
use trust_runtime_core::value::Value;
use trust_runtime_services::access::{resolve_role, AccessRole, AuthFailure};
use trust_runtime_services::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use trust_runtime_services::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, PendingVarTarget,
};
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::live_values::RequestQueueFull;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, ExtensionDispatch, StatusPort,
};
use trust_runtime_services::protocol::{ControlRequest, ControlResponse};
use trust_runtime_services::runner::{EngineLiveValues, RunnerPlatform};
use trust_runtime_services::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

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
}

impl<W> Platform<W> {
    /// A platform over `watchdog` that reports `resource_name`, the runtime `settings`, whether
    /// the debug surface is `debug_enabled` and the record's `control_mode`, with the Live Values
    /// `live` of the installed program, or [`EngineLiveValues::empty`] for a chip without one.
    #[must_use]
    pub fn new(
        watchdog: W,
        resource_name: SmolStr,
        settings: StatusSettings,
        debug_enabled: bool,
        control_mode: ControlMode,
        live: EngineLiveValues,
    ) -> Self {
        Self {
            watchdog,
            resource_name,
            settings,
            debug_enabled,
            control_mode,
            live,
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
