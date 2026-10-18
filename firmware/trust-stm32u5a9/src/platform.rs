//! The resource loop's platform on the STM32U5A9 (specification 34, ARCH-11 and §3.3): the values the
//! chip reports, the access rights of its USB control link, the Live Values the loop applies, its
//! hardware watchdog and the reset the boot reports. It is the ESP32-C6 platform's contract with this
//! chip's descriptor and watchdog.
//!
//! A USB client is local (specification 11 §6.9.5), so the link carries no transport label and no
//! control token is configured yet; a local client therefore acts as `Admin`. The chip has no wall
//! clock until slice T6, no audit store and no platform operation yet, so those ports answer as an
//! absent value does, not with an invented one.

use alloc::string::String;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_platform_memory::reset::{ResetCause, ResetRecord};
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

use crate::profile::DEVICE_DESCRIPTOR;
use crate::watchdog::Watchdog;

/// The STM32U5A9's platform for the single-thread resource loop, over its watchdog, the values the
/// installed application's record configures and the Live Values of the installed program.
pub struct Platform {
    watchdog: Watchdog,
    resource_name: SmolStr,
    /// The chip's stable runtime ID, its chip name and USB serial number (specification 11,
    /// `status`).
    runtime_id: SmolStr,
    settings: StatusSettings,
    debug_enabled: bool,
    /// `runtime.control.mode` of the installed application's deployment record (specification 34
    /// §5.1.1, section 7): the mode `status` reports and the path `pause` and `resume` take.
    control_mode: ControlMode,
    /// The Live Values the runner applies at its cycle phases and the `io.*`, `set` and `var.*`
    /// operations answer from (specification 11, "Live Values"; specification 34 §4.3.1).
    live: EngineLiveValues,
    /// The reset the boot reports, for the `capabilities` reply (specification 11).
    last_reset: LastReset,
}

impl Platform {
    /// A platform over `watchdog` that reports `resource_name`, the runtime `settings`, whether the
    /// debug surface is `debug_enabled`, the record's `control_mode`, the Live Values `live` of the
    /// installed program (or none, for a device without one) and the `last_reset` the boot reports.
    #[must_use]
    pub fn new(
        watchdog: Watchdog,
        resource_name: SmolStr,
        runtime_id: SmolStr,
        settings: StatusSettings,
        debug_enabled: bool,
        control_mode: ControlMode,
        live: EngineLiveValues,
        last_reset: LastReset,
    ) -> Self {
        Self {
            watchdog,
            resource_name,
            runtime_id,
            settings,
            debug_enabled,
            control_mode,
            live,
            last_reset,
        }
    }
}

/// The reset the boot reports (specification 11, the runtime device capabilities contract): the
/// record the previous boot left, or, when it left none, the watchdog or power-on reset its hardware
/// reason names. `boot_count` is the chip's boot counter, reported for every reset.
#[must_use]
pub fn boot_last_reset(watchdog: bool, record: Option<ResetRecord>, boot_count: u32) -> LastReset {
    match record {
        Some(record) => LastReset {
            cause: reset_cause(record.cause()),
            text: SmolStr::new(String::from_utf8_lossy(record.text()).as_ref()),
            boot_count,
        },
        None => LastReset {
            cause: if watchdog {
                ResetCauseName::Watchdog
            } else {
                ResetCauseName::PowerOn
            },
            text: SmolStr::default(),
            boot_count,
        },
    }
}

/// The wire name of the reset record's cause (specification 11).
fn reset_cause(cause: ResetCause) -> ResetCauseName {
    match cause {
        ResetCause::Panic => ResetCauseName::Panic,
        ResetCause::Watchdog => ResetCauseName::Watchdog,
        ResetCause::Requested => ResetCauseName::Requested,
    }
}

/// The runtime settings `status` reports for this chip: the VM backend, production control and no
/// simulation.
#[must_use]
pub fn status_settings() -> StatusSettings {
    StatusSettings {
        execution_backend: "vm",
        execution_backend_source: "default",
        simulation_enabled: false,
        simulation_time_scale: 1,
        simulation_mode: SmolStr::new_static("production"),
        simulation_warning: SmolStr::default(),
    }
}

impl AccessPort for Platform {
    /// No control token is configured, so a local client (no transport label) acts as `Admin`.
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

impl AuditSink for Platform {
    /// The chip has no audit store, so no record id is reported.
    fn record_audit(&self, _record: AuditRecord<'_>) -> Option<SmolStr> {
        None
    }
}

/// A device whose clock nobody has set yet (specification 34, TARGET-06).
impl WallClock for Platform {
    fn utc_ms(&self) -> Option<u64> {
        None
    }
}

impl StatusPort for Platform {
    fn resource_name(&self) -> &str {
        self.resource_name.as_str()
    }

    /// The chip's name (specification 11, `status`).
    fn platform(&self) -> &str {
        trust_platform_stm32u5::usb::PLATFORM
    }

    /// The chip's name and USB serial number (specification 11, `status`).
    fn runtime_id(&self) -> &str {
        self.runtime_id.as_str()
    }

    /// A chip keeps control after `shutdown`: the program takes the place of the process, as on
    /// the C6 (specification 11, `status`).
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

    /// The chip's descriptor, which admission reads (specification 34 §5.1.1, COMPAT-02, TARGET-03).
    fn device_descriptor(&self) -> DeviceDescriptor {
        DEVICE_DESCRIPTOR
    }

    /// The reset the boot reported (specification 11).
    fn last_reset(&self) -> Option<LastReset> {
        Some(self.last_reset.clone())
    }
}

/// The chip answers no platform operation yet.
impl ExtensionDispatch for Platform {
    fn dispatch_extension(&self, _request: &ControlRequest) -> Option<ControlResponse> {
        None
    }

    fn audit_details(&self, _request: &ControlRequest) -> Option<serde_json::Value> {
        None
    }
}

impl HardwareWatchdog for Platform {
    fn feed_interval(&self) -> TimeSpan {
        self.watchdog.feed_interval()
    }

    fn feed(&mut self) {
        self.watchdog.feed();
    }
}

impl RunnerPlatform for Platform {
    fn live_values(&self) -> &EngineLiveValues {
        &self.live
    }

    /// The installed application's record carries the mode (specification 34 §5.1.1, section 7).
    fn control_mode(&self) -> ControlMode {
        self.control_mode
    }
}

/// The platform forwards the `io.*`, `set` and `var.*` operations to the Live Values of the installed
/// program, as trust-runtime's `ControlState` forwards them.
impl IoQueuePort for Platform {
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

impl VariableQueuePort for Platform {
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

/// The chip's stable runtime ID: its chip name and the 24 digits of its unique ID, the serial number
/// its USB device reports (specification 11, `status`).
pub fn device_runtime_id() -> SmolStr {
    let text =
        trust_platform_stm32u5::usb::runtime_id_text(trust_platform_stm32u5::usb_link::read_uid());
    // The text is ASCII by construction: the chip name, a hyphen and hexadecimal digits.
    core::str::from_utf8(&text).map_or_else(
        |_| SmolStr::new_static(trust_platform_stm32u5::usb::PLATFORM),
        SmolStr::new,
    )
}
