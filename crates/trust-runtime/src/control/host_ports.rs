//! The ports of the shared control services over the host's `ControlState` (spec 34 ARCH-11).
//!
//! Each method reads or locks the same field, with the same poison handling, as the host code it
//! replaced: a poisoned value is reported unavailable, a poisoned control mode counts as
//! production, a poisoned restart slot or event store is left untouched, and the resource state
//! accessors keep their own poison panics.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;

use smol_str::SmolStr;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::WallClock;
use trust_runtime_services::access::{AccessRole, AuthFailure};
use trust_runtime_services::capabilities::LastReset;
use trust_runtime_services::events::{store_event, RuntimeEvent};
use trust_runtime_services::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use trust_runtime_services::live_values::io_snapshot::IoSnapshot as PortableIoSnapshot;
use trust_runtime_services::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, PendingVarTarget,
};
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::live_values::RequestQueueFull;
use trust_runtime_services::metrics::RuntimeMetricsSnapshot;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch, ProgramControl, RecordsPort,
    ResourcePort, StatusPort,
};
use trust_runtime_services::resource::commands::CommandSendError;
use trust_runtime_services::status::{IoDriverStatus, StatusSettings};

use crate::config::ControlMode;
use crate::debug::ControlAction;
use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::scheduler::ResourceState;
use crate::value::Value;
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

    /// The operating system and CPU architecture this binary is built for, for example
    /// `linux-x86_64`.
    fn platform(&self) -> &str {
        crate::runtime_id::platform()
    }

    /// The runtime's stable runtime ID (specification 11, `status` and runtime discovery): the
    /// same value the runtime announces by mDNS. It is resolved once, where the state is built: a
    /// project's `.trust-runtime/runtime_id`, or the resource name for a state without an
    /// installed project, which never creates a file.
    fn runtime_id(&self) -> &str {
        self.runtime_id.as_str()
    }

    /// `shutdown` ends the trust-runtime process, so a stopped runtime has no scan loop to take a
    /// `restart` (spec 34 STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::Process
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

    /// A computer's descriptor (specification 34, HOST-02); the `capabilities` operation reports
    /// it (specification 11, the runtime device capabilities contract).
    fn device_descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor::hosted()
    }

    /// A computer starts as a new process and has no last reset (specification 11, the runtime
    /// device capabilities contract).
    fn last_reset(&self) -> Option<LastReset> {
        None
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

impl IoQueuePort for ControlState {
    /// The published snapshot of the host's snapshot slot. A slot that cannot be read answers
    /// `I/O snapshot unavailable`; a slot that holds no snapshot yet answers `no snapshot
    /// available`, as the host's `io.list` and `io.read` answer today.
    fn io_snapshot(&self) -> IoSnapshotRead {
        match self.io_snapshot.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(snapshot) => IoSnapshotRead::Published(PortableIoSnapshot::from(snapshot)),
                None => IoSnapshotRead::Missing,
            },
            Err(_) => IoSnapshotRead::Unavailable,
        }
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.debug.try_enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.debug.try_force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.debug.release_io(address);
    }
}

impl VariableQueuePort for ControlState {
    fn generation(&self) -> Generation {
        self.debug.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        match target {
            PendingVarTarget::Global(name) => self.debug.try_enqueue_global_write(name, value),
            PendingVarTarget::Retain(name) => self.debug.try_enqueue_retain_write(name, value),
            PendingVarTarget::Instance(generation, instance, name) => self
                .debug
                .try_enqueue_instance_write(generation, instance, name, value),
            PendingVarTarget::Local(frame, name) => {
                self.debug.try_enqueue_local_write(frame, name, value)
            }
        }
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        match target {
            ForcedVarTarget::Global(name) => self.debug.try_force_global(name, value),
            ForcedVarTarget::Retain(name) => self.debug.try_force_retain(name, value),
            ForcedVarTarget::Instance(generation, instance, name) => self
                .debug
                .try_force_instance(generation, instance, name, value),
        }
    }

    fn release_global(&self, name: &str) {
        self.debug.release_global(name);
    }

    fn release_retain(&self, name: &str) {
        self.debug.release_retain(name);
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.debug.release_instance(instance, name);
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.debug.forced_snapshot()
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
