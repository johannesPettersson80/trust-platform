//! A platform for the contract tests: every port over plain fields, recording what the shared
//! services ask of it.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::WallClock;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use crate::access::{resolve_role, AccessRole, AuthFailure};
use crate::capabilities::LastReset;
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use crate::events::{RuntimeEvent, EVENT_STORE_CAPACITY};
use crate::installation::Admitted;
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, LiveValueQueues, PendingVarTarget,
};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::metrics::RuntimeMetricsSnapshot;
use crate::ports::{
    AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch, ProgramControl,
    ProgramIdentities, ProgramIdentity, RecordsPort, ResourcePort, StatusPort,
    TransferSessionError,
};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::commands::CommandSendError;
use crate::status::{IoDriverStatus, StatusSettings};
use crate::transfer::TransferSession;

/// The wall time the fake platform reports, in Unix milliseconds.
pub(crate) const WALL_TIME_MS: u64 = 1_792_108_800_000;

/// One audit record as the fake platform stored it.
#[derive(Debug, Clone)]
pub(crate) struct StoredAudit {
    pub(crate) request_id: u64,
    pub(crate) request_type: String,
    pub(crate) correlation_id: Option<String>,
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
    pub(crate) auth_present: bool,
    pub(crate) client: Option<String>,
    pub(crate) details: Option<serde_json::Value>,
}

/// What the platform was asked to do to the scan, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    SchedulerPause,
    SchedulerResume,
    DebuggerPause,
    DebuggerContinue,
}

#[allow(clippy::struct_excessive_bools)] // one flag per platform condition under test
pub(crate) struct FakeHost {
    pub(crate) resource_name: &'static str,
    /// The platform the fake host reports, standing for the computer the tests run on.
    pub(crate) platform: &'static str,
    /// The stable runtime ID the fake host reports.
    pub(crate) runtime_id: &'static str,
    pub(crate) control_token: Option<&'static str>,
    pub(crate) requires_auth: bool,
    pub(crate) pairing: Option<(&'static str, AccessRole)>,
    pub(crate) debug_enabled: bool,
    pub(crate) audits: RefCell<Vec<StoredAudit>>,
    pub(crate) events: RefCell<VecDeque<RuntimeEvent>>,
    pub(crate) state: Cell<ResourceState>,
    pub(crate) last_error: Option<RuntimeError>,
    pub(crate) hung: bool,
    /// Refuses every scheduler command with this error and then reports this state.
    pub(crate) command_refusal: Option<(CommandSendError, ResourceState)>,
    pub(crate) debug_mode: bool,
    pub(crate) actions: RefCell<Vec<Action>>,
    pub(crate) pending_restart: RefCell<Option<RestartMode>>,
    /// Values that cannot be read: `settings`, `io`, `metrics`, `realtime`, `control_mode`,
    /// `openot`, `event_capacity`, `events` or `restart_slot`.
    pub(crate) unreadable: Vec<&'static str>,
    pub(crate) debug_hold: Option<&'static str>,
    pub(crate) io_drivers: Vec<IoDriverStatus>,
    pub(crate) metrics: RuntimeMetricsSnapshot,
    /// The newest events the fake platform's event store keeps.
    pub(crate) event_capacity: usize,
    pub(crate) realtime_errors: bool,
    pub(crate) extension_calls: RefCell<Vec<String>>,
    /// The wall clock: [`WALL_TIME_MS`], or `None` while the time is not set.
    pub(crate) utc_ms: Option<u64>,
    /// Whether `restart` starts a stopped resource (a device without an operating system).
    pub(crate) starts_when_stopped: bool,
    /// The Live Values queues: I/O writes, variable writes and active forces.
    pub(crate) live_values: RefCell<LiveValueQueues<()>>,
    /// The published I/O snapshot the shared `io.*` operations answer from.
    pub(crate) io_snapshot: RefCell<IoSnapshotRead>,
    /// The installation generation an instance-targeted request is resolved in.
    pub(crate) generation: Generation,
    /// Why a device has no installed program, when it has none.
    pub(crate) missing_program: Option<&'static str>,
    /// The device descriptor the `capabilities` operation reports; a computer's by default.
    pub(crate) device_descriptor: DeviceDescriptor,
    /// The reset the platform's boot read; `None` for a computer, which starts as a new process.
    pub(crate) last_reset: Option<LastReset>,
    /// The program-control posture the fake host reports.
    pub(crate) program_control: ProgramControl,
    /// The transfer session the transfer operations use; `None` for a platform without one.
    pub(crate) transfers: Option<RefCell<TransferSession>>,
    /// The identities `program.active` reports; `None` for a platform without a program store.
    pub(crate) program_identities: Option<ProgramIdentities>,
    /// Whether the platform keeps a program store that `program.install` commits to.
    pub(crate) has_program_store: bool,
    /// The retained-value check's refusal text, when the check refuses.
    pub(crate) retained_refusal: Option<&'static str>,
    /// The containers the commit was asked to store, in order.
    pub(crate) commits: RefCell<Vec<Vec<u8>>>,
    /// The identity a commit gives the program; `None` refuses every commit.
    pub(crate) commit_identity: Option<ProgramIdentity>,
    /// The steps the platform was asked to run, in order.
    pub(crate) steps: RefCell<Vec<StepRequest>>,
    /// What every step answers; `None` for a platform without stepped scans.
    pub(crate) step_answer: Option<Result<StepOutcome, StepRefusal>>,
}

impl FakeHost {
    /// A running runtime in production mode without access control.
    pub(crate) fn running() -> Self {
        Self {
            resource_name: "RESOURCE",
            platform: "test-platform",
            runtime_id: "test-runtime-id",
            control_token: None,
            requires_auth: false,
            pairing: None,
            debug_enabled: true,
            audits: RefCell::new(Vec::new()),
            events: RefCell::new(VecDeque::new()),
            state: Cell::new(ResourceState::Running),
            last_error: None,
            hung: false,
            command_refusal: None,
            debug_mode: false,
            actions: RefCell::new(Vec::new()),
            pending_restart: RefCell::new(None),
            unreadable: Vec::new(),
            debug_hold: None,
            io_drivers: Vec::new(),
            metrics: RuntimeMetricsSnapshot::default(),
            event_capacity: EVENT_STORE_CAPACITY,
            realtime_errors: false,
            extension_calls: RefCell::new(Vec::new()),
            utc_ms: Some(WALL_TIME_MS),
            starts_when_stopped: false,
            live_values: RefCell::new(LiveValueQueues::new()),
            io_snapshot: RefCell::new(IoSnapshotRead::Missing),
            generation: Generation::FIRST,
            missing_program: None,
            device_descriptor: DeviceDescriptor::hosted(),
            last_reset: None,
            program_control: ProgramControl::Process,
            transfers: None,
            program_identities: None,
            has_program_store: false,
            retained_refusal: None,
            commits: RefCell::new(Vec::new()),
            commit_identity: None,
            steps: RefCell::new(Vec::new()),
            step_answer: None,
        }
    }

    /// The actions taken so far.
    pub(crate) fn actions(&self) -> Vec<Action> {
        self.actions.borrow().clone()
    }

    /// The audit records written so far.
    pub(crate) fn audits(&self) -> Vec<StoredAudit> {
        self.audits.borrow().clone()
    }

    fn readable(&self, value: &str) -> bool {
        !self.unreadable.contains(&value)
    }

    fn send(&self, action: Action) -> Result<(), CommandSendError> {
        if let Some((error, state)) = self.command_refusal {
            self.state.set(state);
            return Err(error);
        }
        self.actions.borrow_mut().push(action);
        Ok(())
    }
}

impl AccessPort for FakeHost {
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        resolve_role(
            request.auth.as_deref(),
            self.control_token,
            self.requires_auth,
            client,
            |token| {
                self.pairing
                    .filter(|(paired, _)| *paired == token)
                    .map(|(_, role)| role)
            },
        )
    }

    fn debug_enabled(&self) -> bool {
        self.debug_enabled
    }
}

impl AuditSink for FakeHost {
    fn record_audit(&self, record: AuditRecord<'_>) -> Option<SmolStr> {
        let mut audits = self.audits.borrow_mut();
        audits.push(StoredAudit {
            request_id: record.request_id,
            request_type: String::from(record.request_type.as_str()),
            correlation_id: record.correlation_id.map(String::from),
            ok: record.ok,
            error: record.error.as_deref().map(String::from),
            auth_present: record.auth_present,
            client: record.client.map(String::from),
            details: record.details,
        });
        Some(SmolStr::new(format!("audit-{}", audits.len())))
    }
}

impl EventSink for FakeHost {
    fn record_event(&self, event: RuntimeEvent) {
        self.events.borrow_mut().push_back(event);
    }
}

impl WallClock for FakeHost {
    fn utc_ms(&self) -> Option<u64> {
        self.utc_ms
    }
}

impl ResourcePort for FakeHost {
    fn resource_state(&self) -> ResourceState {
        self.state.get()
    }

    fn last_error(&self) -> Option<RuntimeError> {
        self.last_error.clone()
    }

    fn is_hung(&self) -> bool {
        self.hung
    }

    fn pause_resource(&self) -> Result<(), CommandSendError> {
        self.send(Action::SchedulerPause)
    }

    fn resume_resource(&self) -> Result<(), CommandSendError> {
        self.send(Action::SchedulerResume)
    }

    fn debug_control_mode(&self) -> bool {
        self.debug_mode
    }

    fn debugger_pause(&self) {
        self.actions.borrow_mut().push(Action::DebuggerPause);
    }

    fn debugger_continue(&self) {
        self.actions.borrow_mut().push(Action::DebuggerContinue);
    }

    fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>)) {
        if self.readable("restart_slot") {
            let mut slot = self.pending_restart.borrow_mut();
            let pending: &mut Option<RestartMode> = &mut slot;
            update(pending);
        }
    }

    fn starts_when_stopped(&self) -> bool {
        self.starts_when_stopped
    }

    fn missing_program(&self) -> Option<&str> {
        self.missing_program
    }

    fn step_cycle(&self, request: &StepRequest) -> Result<StepOutcome, StepRefusal> {
        self.steps.borrow_mut().push(request.clone());
        self.step_answer
            .clone()
            .unwrap_or(Err(StepRefusal::Unsupported))
    }
}

impl StatusPort for FakeHost {
    fn resource_name(&self) -> &str {
        self.resource_name
    }

    fn platform(&self) -> &str {
        self.platform
    }

    fn runtime_id(&self) -> &str {
        self.runtime_id
    }

    fn program_control(&self) -> ProgramControl {
        self.program_control
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        self.readable("settings").then(|| StatusSettings {
            execution_backend: "native",
            execution_backend_source: "default",
            simulation_enabled: false,
            simulation_time_scale: 1,
            simulation_mode: SmolStr::new("production"),
            simulation_warning: SmolStr::default(),
        })
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        self.readable("io").then(|| self.io_drivers.clone())
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        self.readable("realtime")
            .then(|| json!({"profile": "disabled", "errors": []}))
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        self.readable("realtime").then_some(self.realtime_errors)
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        self.readable("openot")
            .then(|| json!({"state": "disabled"}))
    }

    fn device_descriptor(&self) -> DeviceDescriptor {
        self.device_descriptor.clone()
    }

    fn last_reset(&self) -> Option<LastReset> {
        self.last_reset.clone()
    }
}

impl RecordsPort for FakeHost {
    fn debug_hold(&self) -> Option<&'static str> {
        self.debug_hold
    }

    fn control_mode_label(&self) -> Option<String> {
        let label = if self.debug_mode {
            "debug"
        } else {
            "production"
        };
        self.readable("control_mode").then(|| String::from(label))
    }

    fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot> {
        self.readable("metrics").then(|| self.metrics.clone())
    }

    fn event_capacity(&self) -> Option<usize> {
        self.readable("event_capacity")
            .then_some(self.event_capacity)
    }

    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R> {
        if !self.readable("events") {
            return None;
        }
        let events = self.events.borrow();
        let store: &VecDeque<RuntimeEvent> = &events;
        Some(read(store))
    }

    fn with_transfers<R>(
        &self,
        use_session: impl FnOnce(&mut TransferSession) -> R,
    ) -> Result<R, TransferSessionError> {
        let Some(session) = &self.transfers else {
            return Err(TransferSessionError::Unsupported);
        };
        match session.try_borrow_mut() {
            Ok(mut session) => Ok(use_session(&mut session)),
            Err(_) => Err(TransferSessionError::Unavailable),
        }
    }

    fn program_identities(&self) -> Option<ProgramIdentities> {
        self.program_identities
    }

    fn has_program_store(&self) -> bool {
        self.has_program_store
    }

    fn check_retained_values(&self, _admitted: &Admitted<'_>) -> Result<(), RuntimeError> {
        match self.retained_refusal {
            Some(text) => Err(RuntimeError::RetainMigration(text.into())),
            None => Ok(()),
        }
    }

    fn commit_program(&self, container: Vec<u8>) -> Result<ProgramIdentity, String> {
        self.commits.borrow_mut().push(container);
        self.commit_identity
            .ok_or_else(|| String::from("the fake store refuses the commit"))
    }
}

impl ExtensionDispatch for FakeHost {
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse> {
        self.extension_calls
            .borrow_mut()
            .push(request.r#type.clone());
        (request.r#type == "config.get")
            .then(|| ControlResponse::ok(request.id, json!({"answered_by": "platform"})))
    }

    fn audit_details(&self, request: &ControlRequest) -> Option<serde_json::Value> {
        request
            .r#type
            .starts_with("comm.")
            .then(|| json!({"params": request.params}))
    }
}

impl IoQueuePort for FakeHost {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.io_snapshot.borrow().clone()
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live_values
            .borrow_mut()
            .try_enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live_values
            .borrow_mut()
            .try_upsert_forced_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.live_values.borrow_mut().release_io(address);
    }
}

impl VariableQueuePort for FakeHost {
    fn generation(&self) -> Generation {
        self.generation
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.live_values
            .borrow_mut()
            .try_upsert_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.live_values
            .borrow_mut()
            .try_upsert_forced_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.live_values.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Global(current) if current.as_str() == name),
        );
    }

    fn release_retain(&self, name: &str) {
        self.live_values.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Retain(current) if current.as_str() == name),
        );
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.live_values.borrow_mut().clear_forced_vars(|target| {
            matches!(target, ForcedVarTarget::Instance(_, current_id, current_name) if *current_id == instance && current_name.as_str() == name)
        });
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.live_values.borrow().forced_snapshot()
    }
}
