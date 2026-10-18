//! The dispatcher's view of the resource loop: the platform's ports, the installed program's
//! I/O drivers, and the resource state, metrics and events that the loop keeps itself.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock, WallClock};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use super::cycle::ResourceCycle;
use super::engine::EngineLiveValues;
use super::program::Program;
use super::state::{Records, ResourceCell};
use crate::access::{AccessRole, AuthFailure};
use crate::events::RuntimeEvent;
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::metrics::RuntimeMetricsSnapshot;
use crate::ports::{
    AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch, RecordsPort, ResourcePort,
    StatusPort,
};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::commands::{CommandClass, CommandSendError};
use crate::status::{IoDriverStatus, StatusSettings};

/// What the platform supplies to the resource loop besides the link, the wait, the clock and the
/// installed program: the access rights, the audit sink, the wall clock, the values the platform
/// knows, the operations only the platform answers, the hardware watchdog and the Live Values
/// queues. A request the platform does not answer gets trust-runtime's `unsupported request`.
///
/// The loop hands each audit record to the platform's sink in the pass that serves the request.
/// trust-runtime reports `audit_dropped` when its audit logger thread has ended and the channel
/// to it refuses a record; a device has no audit thread and no channel, so it never reports
/// `audit_dropped`.
pub trait RunnerPlatform:
    AccessPort
    + AuditSink
    + WallClock
    + StatusPort
    + ExtensionDispatch
    + HardwareWatchdog
    + IoQueuePort
    + VariableQueuePort
{
    /// The Live Values the runner applies at its cycle phases and the platform answers the
    /// `io.*`, `set` and `var.*` requests from (specification 11, "Live Values"; specification 34
    /// §4.3.1). The platform owns it and the runner borrows it for a scan.
    fn live_values(&self) -> &EngineLiveValues;

    /// Work between scans, for example servicing a communication driver; the loop runs it once
    /// per pass, and it must not wait. The default does nothing.
    fn idle(&mut self) {}
}

/// The ports the dispatcher reads and acts through while the loop serves a request.
pub(super) struct RunnerHost<'a, P, Y, C> {
    pub(super) platform: &'a P,
    pub(super) program: &'a Program<Y>,
    pub(super) resource: &'a ResourceCell,
    pub(super) records: &'a Records<C>,
}

impl<P: RunnerPlatform, Y, C> AccessPort for RunnerHost<'_, P, Y, C> {
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        self.platform.request_role(request, client)
    }

    fn debug_enabled(&self) -> bool {
        self.platform.debug_enabled()
    }
}

impl<P: RunnerPlatform, Y, C> AuditSink for RunnerHost<'_, P, Y, C> {
    fn record_audit(&self, record: AuditRecord<'_>) -> Option<SmolStr> {
        self.platform.record_audit(record)
    }
}

impl<P, Y, C: MonotonicClock> EventSink for RunnerHost<'_, P, Y, C> {
    fn record_event(&self, event: RuntimeEvent) {
        self.records.push_event(event);
    }
}

impl<P: RunnerPlatform, Y, C> WallClock for RunnerHost<'_, P, Y, C> {
    fn utc_ms(&self) -> Option<u64> {
        self.platform.utc_ms()
    }
}

/// The loop runs in production control mode: a pause holds the scan between cycles. The debugger
/// hold of specification 13 is not part of the loop yet.
impl<P, Y, C> ResourcePort for RunnerHost<'_, P, Y, C> {
    fn resource_state(&self) -> ResourceState {
        self.resource.state()
    }

    fn last_error(&self) -> Option<RuntimeError> {
        self.resource.last_error()
    }

    /// The loop itself runs the scan, so a request is never served while a scan has not returned.
    fn is_hung(&self) -> bool {
        false
    }

    fn pause_resource(&self) -> Result<(), CommandSendError> {
        self.resource.command(CommandClass::Pause)
    }

    fn resume_resource(&self) -> Result<(), CommandSendError> {
        self.resource.command(CommandClass::Resume)
    }

    fn debug_control_mode(&self) -> bool {
        false
    }

    fn debugger_pause(&self) {}

    fn debugger_continue(&self) {}

    fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>)) {
        self.resource.update_pending_restart(update);
    }

    /// The program takes the place of the process (spec 34 STATE-08).
    fn starts_when_stopped(&self) -> bool {
        true
    }

    fn missing_program(&self) -> Option<&str> {
        self.program.missing()
    }
}

impl<P: RunnerPlatform, Y: ResourceCycle, C> StatusPort for RunnerHost<'_, P, Y, C> {
    fn resource_name(&self) -> &str {
        self.platform.resource_name()
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        self.platform.status_settings()
    }

    /// The installed program's I/O drivers, then those the platform runs itself.
    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        let mut drivers = self.program.io_drivers();
        drivers.extend(self.platform.io_drivers()?);
        Some(drivers)
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        self.platform.realtime_status()
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        self.platform.realtime_has_errors()
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        self.platform.openot_persistence()
    }
}

impl<P, Y, C: MonotonicClock> RecordsPort for RunnerHost<'_, P, Y, C> {
    fn debug_hold(&self) -> Option<&'static str> {
        None
    }

    fn control_mode_label(&self) -> Option<String> {
        Some(String::from("production"))
    }

    fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot> {
        self.records.metrics_snapshot()
    }

    fn event_capacity(&self) -> Option<usize> {
        Some(self.records.event_capacity())
    }

    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R> {
        self.records.read_events(read)
    }
}

impl<P: RunnerPlatform, Y, C> ExtensionDispatch for RunnerHost<'_, P, Y, C> {
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse> {
        self.platform.dispatch_extension(request)
    }

    fn audit_details(&self, request: &ControlRequest) -> Option<serde_json::Value> {
        self.platform.audit_details(request)
    }
}

/// The platform owns the Live Values queues and the published snapshot, as `ControlState` does
/// for trust-runtime; the loop only passes them through.
impl<P: RunnerPlatform, Y, C> IoQueuePort for RunnerHost<'_, P, Y, C> {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.platform.io_snapshot()
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.platform.enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.platform.force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.platform.release_io(address);
    }
}

impl<P: RunnerPlatform, Y, C> VariableQueuePort for RunnerHost<'_, P, Y, C> {
    fn generation(&self) -> Generation {
        self.platform.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.platform.enqueue_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.platform.force_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.platform.release_global(name);
    }

    fn release_retain(&self, name: &str) {
        self.platform.release_retain(name);
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.platform.release_instance(instance, name);
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.platform.forced_snapshot()
    }
}
