//! The dispatcher's view of the resource loop: the platform's ports, and the resource state,
//! metrics and events that the loop keeps itself.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock, WallClock};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use super::state::{Records, ResourceCell};
use crate::access::{AccessRole, AuthFailure};
use crate::events::RuntimeEvent;
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
/// knows, the operations only the platform answers and the hardware watchdog. A request the
/// platform does not answer gets trust-runtime's `unsupported request`.
///
/// The loop hands each audit record to the platform's sink in the pass that serves the request.
/// trust-runtime reports `audit_dropped` when its audit logger thread has ended and the channel
/// to it refuses a record; a device has no audit thread and no channel, so it never reports
/// `audit_dropped`.
pub trait RunnerPlatform:
    AccessPort + AuditSink + WallClock + StatusPort + ExtensionDispatch + HardwareWatchdog
{
    /// Work between scans, for example servicing a communication driver; the loop runs it once
    /// per pass, and it must not wait. The default does nothing.
    fn idle(&mut self) {}
}

/// The ports the dispatcher reads and acts through while the loop serves a request.
pub(super) struct RunnerHost<'a, P, C> {
    pub(super) platform: &'a P,
    pub(super) resource: &'a ResourceCell,
    pub(super) records: &'a Records<C>,
}

impl<P: RunnerPlatform, C> AccessPort for RunnerHost<'_, P, C> {
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

impl<P: RunnerPlatform, C> AuditSink for RunnerHost<'_, P, C> {
    fn record_audit(&self, record: AuditRecord<'_>) -> Option<SmolStr> {
        self.platform.record_audit(record)
    }
}

impl<P, C: MonotonicClock> EventSink for RunnerHost<'_, P, C> {
    fn record_event(&self, event: RuntimeEvent) {
        self.records.push_event(event);
    }
}

impl<P: RunnerPlatform, C> WallClock for RunnerHost<'_, P, C> {
    fn utc_ms(&self) -> Option<u64> {
        self.platform.utc_ms()
    }
}

/// The loop runs in production control mode: a pause holds the scan between cycles. The debugger
/// hold of specification 13 is not part of the loop yet.
impl<P, C> ResourcePort for RunnerHost<'_, P, C> {
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
}

impl<P: RunnerPlatform, C> StatusPort for RunnerHost<'_, P, C> {
    fn resource_name(&self) -> &str {
        self.platform.resource_name()
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        self.platform.status_settings()
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        self.platform.io_drivers()
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

impl<P, C: MonotonicClock> RecordsPort for RunnerHost<'_, P, C> {
    fn debug_hold(&self) -> Option<&'static str> {
        None
    }

    fn control_mode_label(&self) -> Option<String> {
        Some(String::from("production"))
    }

    fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot> {
        self.records.metrics_snapshot()
    }

    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R> {
        self.records.read_events(read)
    }
}

impl<P: RunnerPlatform, C> ExtensionDispatch for RunnerHost<'_, P, C> {
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse> {
        self.platform.dispatch_extension(request)
    }

    fn audit_details(&self, request: &ControlRequest) -> Option<serde_json::Value> {
        self.platform.audit_details(request)
    }
}
