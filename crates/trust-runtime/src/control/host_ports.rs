//! The ports of the shared control services over the host's `ControlState` (spec 34 ARCH-11).
//!
//! Each method reads or locks the same field, with the same poison handling, as the host code it
//! replaced: a poisoned value is reported unavailable, a poisoned control mode counts as
//! production, a poisoned restart slot or event store is left untouched, and the resource state
//! accessors keep their own poison panics.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;

use smol_str::SmolStr;
use trust_runtime_core::ports::WallClock;
use trust_runtime_services::access::{AccessRole, AuthFailure};
use trust_runtime_services::events::{store_event, RuntimeEvent};
use trust_runtime_services::metrics::RuntimeMetricsSnapshot;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch, RecordsPort, ResourcePort,
    StatusPort,
};
use trust_runtime_services::resource::commands::CommandSendError;
use trust_runtime_services::status::{IoDriverStatus, StatusSettings};

use crate::config::ControlMode;
use crate::debug::ControlAction;
use crate::error::RuntimeError;
use crate::scheduler::ResourceState;
use crate::RestartMode;

use super::types::debug_stop_reason_label;
use super::{
    audit, auth, comm_handlers, control_event_time_now, handlers, status_handlers, ControlRequest,
    ControlResponse, ControlState,
};

impl AccessPort for ControlState {
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        auth::resolve_request_role(request, self, client)
    }

    fn debug_enabled(&self) -> bool {
        self.debug_enabled.load(Ordering::Relaxed)
    }
}

impl AuditSink for ControlState {
    fn record_audit(&self, record: AuditRecord<'_>) -> Option<SmolStr> {
        audit::record_audit(self, record)
    }
}

impl EventSink for ControlState {
    /// Stores `event` in the project's event store, which keeps the newest
    /// `runtime.events.capacity` events (specification 11, the event store). A settings or event
    /// store that cannot be read drops the event, as the store being unwritable does.
    fn record_event(&self, event: RuntimeEvent) {
        let capacity = self
            .settings
            .lock()
            .ok()
            .map(|settings| settings.event_capacity);
        if let Some(capacity) = capacity
            && let Ok(mut events) = self.events.lock()
        {
            store_event(&mut events, event, capacity);
        }
    }
}

impl WallClock for ControlState {
    /// The host's clock is always set, so control events always carry their time; a clock before
    /// the epoch reads as the epoch, as before.
    fn utc_ms(&self) -> Option<u64> {
        Some(u64::try_from(control_event_time_now().as_millis()).unwrap_or(0))
    }
}

impl ResourcePort for ControlState {
    fn resource_state(&self) -> ResourceState {
        self.resource.state()
    }

    fn last_error(&self) -> Option<RuntimeError> {
        self.resource.last_error()
    }

    fn is_hung(&self) -> bool {
        self.resource.is_hung()
    }

    fn pause_resource(&self) -> Result<(), CommandSendError> {
        self.resource.pause()
    }

    fn resume_resource(&self) -> Result<(), CommandSendError> {
        self.resource.resume()
    }

    fn debug_control_mode(&self) -> bool {
        let mode = self
            .control_mode
            .lock()
            .map(|value| *value)
            .unwrap_or(ControlMode::Production);
        matches!(mode, ControlMode::Debug)
    }

    fn debugger_pause(&self) {
        let _ = self.debug.apply_action(ControlAction::Pause(None));
    }

    fn debugger_continue(&self) {
        let _ = self.debug.apply_action(ControlAction::Continue);
    }

    fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>)) {
        if let Ok(mut guard) = self.pending_restart.lock() {
            let pending: &mut Option<RestartMode> = &mut guard;
            update(pending);
        }
    }

    /// `shutdown` ends the trust-runtime process, so a stopped resource has no scan thread to take
    /// a restart (spec 34 STATE-08).
    fn starts_when_stopped(&self) -> bool {
        false
    }
}

impl StatusPort for ControlState {
    fn resource_name(&self) -> &str {
        self.resource_name.as_str()
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        let settings = self.settings.lock().ok()?;
        Some(StatusSettings {
            execution_backend: settings.execution_backend.as_str(),
            execution_backend_source: settings.execution_backend_source.as_str(),
            simulation_enabled: settings.simulation.enabled,
            simulation_time_scale: settings.simulation.time_scale,
            simulation_mode: settings.simulation.mode_label.clone(),
            simulation_warning: settings.simulation.warning.clone(),
        })
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        self.io_health.lock().ok().map(|drivers| drivers.clone())
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        let realtime = self.realtime_status.lock().ok()?;
        Some(status_handlers::realtime_status_json(&realtime))
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        let realtime = self.realtime_status.lock().ok()?;
        Some(!realtime.errors.is_empty())
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        status_handlers::openot_persistence_status_json(self).ok()
    }
}

impl RecordsPort for ControlState {
    fn debug_hold(&self) -> Option<&'static str> {
        self.debug.hold_reason().map(debug_stop_reason_label)
    }

    fn control_mode_label(&self) -> Option<String> {
        let mode = self.control_mode.lock().ok()?;
        Some(format!("{:?}", *mode).to_ascii_lowercase())
    }

    fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot> {
        self.metrics.lock().ok().map(|metrics| metrics.snapshot())
    }

    fn event_capacity(&self) -> Option<usize> {
        let settings = self.settings.lock().ok()?;
        Some(settings.event_capacity)
    }

    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R> {
        let events = self.events.lock().ok()?;
        let store: &VecDeque<RuntimeEvent> = &events;
        Some(read(store))
    }
}

impl ExtensionDispatch for ControlState {
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse> {
        handlers::dispatch(request, self)
    }

    fn audit_details(&self, request: &ControlRequest) -> Option<serde_json::Value> {
        comm_handlers::audit_details_for_comm_request(
            request.r#type.as_str(),
            request.params.as_ref(),
        )
    }
}
