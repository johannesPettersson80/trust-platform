//! The dispatcher's view of the resource loop: the platform's ports, the installed program's
//! I/O drivers, and the resource state, metrics and events that the loop keeps itself.

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::{ControlMode, DeviceDescriptor};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock, WallClock};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use super::cycle::DebugView;
use super::debugger::ChipDebug;
use super::engine::EngineLiveValues;
use super::install::InstallState;
use super::online_change::ChangeState;
use super::state::{Records, ResourceCell};
use super::stepping::StepState;
use crate::access::{AccessRole, AuthFailure};
use crate::capabilities::LastReset;
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use crate::debug::Debugger;
use crate::events::RuntimeEvent;
use crate::installation::Admitted;
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::loading::install::Checked;
use crate::loading::reload;
use crate::metrics::RuntimeMetricsSnapshot;
use crate::ports::{
    part_in_memory, AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch,
    ProgramControl, ProgramIdentities, ProgramIdentity, RecordsPort, ResourcePort, StatusPort,
    TransferSessionError,
};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::commands::{CommandClass, CommandSendError};
use crate::resource::replies::no_installed_program;
use crate::status::{IoDriverStatus, StatusSettings};
use crate::transfer::{StoredUpload, TransferId, TransferSession};

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
    /// §4.3.1). The platform owns it, and the runner takes a handle on the same state when it
    /// starts ([`EngineLiveValues`] is a handle), so a platform returns the same one for as long as
    /// its program lives. A device that loads a program in place gives the platform the Live Values
    /// of each program it loads (`unload_program`, then its loader), and the loop takes a handle on
    /// them as that program starts (specification 34 STATE-08).
    fn live_values(&self) -> &EngineLiveValues;

    /// Work between scans, for example servicing a communication driver; the loop runs it once
    /// per pass, and so does the pass of a debugger hold on a scan, which keeps communication
    /// drivers running as trust-runtime's threads keep running (specification 34, STATE-07). It
    /// must not wait. The default does nothing.
    fn idle(&mut self) {}

    /// The control mode the platform's deployment record carries (specification 34 §5.1.1,
    /// section 7; specification 11 §6.9): `runtime.control.mode` from the record's configuration.
    /// A platform without a deployment record reports `production`, the record's default.
    fn control_mode(&self) -> ControlMode {
        ControlMode::Production
    }

    /// The transfer session the platform keeps for the transfer operations (specification 34,
    /// PORT-05). A platform without one keeps the default, and the transfer operations are then
    /// unsupported on it, unless the device gave its loop a session of its own
    /// ([`Runner::with_transfers`](crate::runner::Runner::with_transfers)), which the loop uses first.
    ///
    /// # Errors
    ///
    /// [`TransferSessionError::Unsupported`] when the platform keeps no session, and
    /// [`TransferSessionError::Unavailable`] when the session cannot be locked.
    fn with_transfers<R>(
        &self,
        _use_session: impl FnOnce(&mut TransferSession) -> R,
    ) -> Result<R, TransferSessionError> {
        Err(TransferSessionError::Unsupported)
    }

    /// The selected program and the program before it, which `program.active` reports (specification
    /// 34 §11.2). A platform without a program store keeps the default. A loop that was given a
    /// program store answers `program.active` from that store instead
    /// ([`Runner::with_program_store`](crate::runner::Runner::with_program_store)).
    fn program_identities(&self) -> Option<ProgramIdentities> {
        None
    }

    /// The program that ended is dropped: the platform lets go of the values of its record, which a
    /// device that loads a program in place replaces when it loads the next (specification 34
    /// STATE-08). The Live Values of the program that ended hold memory the next program needs, so
    /// they are emptied here. The default does nothing, for a platform that never loads in place.
    fn unload_program(&mut self) {}
}

/// What the dispatcher reads of the installed program while the loop serves a request: whether
/// there is one, and the health of its I/O drivers.
///
/// Between scans that is the program itself. While a debugger holds a scan the program is inside
/// its scan and cannot be asked, so the view is the health its cycle published before the program
/// ran (specification 34, STATE-07).
pub(super) trait ProgramView {
    /// Why no program is installed, when none is.
    fn missing_program(&self) -> Option<&str>;

    /// The health of the program's I/O drivers, in `io.toml` order.
    fn driver_health(&self) -> Vec<IoDriverStatus>;

    /// Runs one stepped scan of the stopped program (`cycle.step`, specification 11 §6.8.1).
    ///
    /// # Errors
    ///
    /// The [`StepRefusal`] of a step that ran no scan or whose values cannot be read.
    fn step(&self, request: &StepRequest) -> Result<StepOutcome, StepRefusal>;

    /// What a debug read sees of the program (specification 11 §6.13.1): between scans the state of
    /// the last scan that ended, and while a debugger holds the scan its halted state. None when no
    /// debugger is attached to the program.
    fn debug_state(&self) -> Option<DebugView<'_>>;

    /// What a debug read sees of a compiled program (specification 44 §13.2): its memory between
    /// scans, and its held scan's memory and frames while the debugger holds it. None for the
    /// interpreter's program and without a debugger.
    #[cfg(feature = "native")]
    fn compiled_state(&self) -> Option<super::cycle::CompiledView<'_>> {
        None
    }
}

/// The ports the dispatcher reads and acts through while the loop serves a request.
pub(super) struct RunnerHost<'a, P, C> {
    pub(super) platform: &'a P,
    pub(super) program: &'a dyn ProgramView,
    pub(super) resource: &'a ResourceCell,
    pub(super) records: &'a Records<C>,
    pub(super) debug: &'a ChipDebug,
    /// The transfers, the program store and the install the loop keeps.
    pub(super) install: &'a InstallState,
    /// The online change that waits for the program's next boundary.
    pub(super) change: &'a ChangeState,
    /// The step deferred to the loop.
    pub(super) step: &'a StepState,
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

/// The resource loop's own resource. The control mode comes from the platform's deployment
/// record, so `pause` and `resume` take trust-runtime's path for that mode (specification 11
/// §6.9): the scheduler between cycles in production, the debugger in debug. The debugger holds
/// the scan at a statement start (specification 13, "Pause"), and `resume` releases the hold in
/// either mode.
impl<P: RunnerPlatform, C> ResourcePort for RunnerHost<'_, P, C> {
    fn resource_state(&self) -> ResourceState {
        self.resource.state()
    }

    fn last_error(&self) -> Option<RuntimeError> {
        self.resource.last_error()
    }

    /// The loop itself runs the scan, so a request is served only between scans, or while a
    /// debugger holds a scan, which waits by design and is no hang (specification 11 §6.9).
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
        matches!(self.platform.control_mode(), ControlMode::Debug)
    }

    /// A program with a debugger but without a line table has no statement start to hold at
    /// (specification 11 §6.13.3). A program that no debugger is attached to is not asked.
    fn debugger_unavailable(&self) -> bool {
        self.program
            .debug_state()
            .is_some_and(|view| view.sources.is_none())
    }

    fn undebuggable(&self) -> Option<&'static str> {
        #[cfg(feature = "native")]
        if self
            .program
            .compiled_state()
            .is_some_and(|view| view.release)
        {
            return Some(crate::debug::replies::NO_DEBUG_HOOKS);
        }
        None
    }

    fn debugger_pause(&self) {
        self.debug.pause();
    }

    fn debugger_continue(&self) {
        self.debug.resume();
    }

    fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>)) {
        self.resource.update_pending_restart(update);
    }

    /// The program takes the place of the process (spec 34 STATE-08).
    fn starts_when_stopped(&self) -> bool {
        true
    }

    /// Why the resource has no program, unless the store holds a container newer than the one the loop
    /// refused or lacked: a `restart` then loads it in place (specification 34 STATE-08). A store that
    /// cannot be ordered keeps the refusal.
    fn missing_program(&self) -> Option<&str> {
        let reason = self.program.missing_program()?;
        if self.install.newer_container().unwrap_or(false) {
            None
        } else {
            Some(reason)
        }
    }

    /// The loop lends its program between scans; a scan that a debugger holds cannot step. A cold
    /// restart that finds a program installed since the program was loaded loads it first, as
    /// `restart` loads it (specification 34 STATE-08), which only the loop can do: the step is
    /// deferred to the loop.
    fn step_cycle(&self, request: &StepRequest) -> Result<StepOutcome, StepRefusal> {
        if request.restart == Some(RestartMode::Cold)
            && self.install.newer_container().unwrap_or(false)
        {
            self.step.defer(request.clone());
            return Err(StepRefusal::Deferred);
        }
        self.program.step(request)
    }
}

impl<P: RunnerPlatform, C> StatusPort for RunnerHost<'_, P, C> {
    fn resource_name(&self) -> &str {
        self.platform.resource_name()
    }

    fn platform(&self) -> &str {
        self.platform.platform()
    }

    /// The runner's runtime ID is its platform's, which the device without an operating system
    /// knows on its own (specification 11, `status`).
    fn runtime_id(&self) -> &str {
        self.platform.runtime_id()
    }

    /// A device without an operating system keeps control after `shutdown`: its program takes the
    /// place of the process, so it still answers and `restart` starts the program again
    /// (spec 34 STATE-08). The device never asks the platform, which cannot answer otherwise.
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        self.platform.status_settings()
    }

    /// The installed program's I/O drivers, then those the platform runs itself.
    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        let mut drivers = self.program.driver_health();
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

    /// The platform's descriptor, so a device answers its own and a hosted runtime the computer's.
    fn device_descriptor(&self) -> DeviceDescriptor {
        self.platform.device_descriptor()
    }

    /// The reset the platform's boot read, `None` on a computer.
    fn last_reset(&self) -> Option<LastReset> {
        self.platform.last_reset()
    }
}

impl<P: RunnerPlatform, C: MonotonicClock> RecordsPort for RunnerHost<'_, P, C> {
    /// The reason a debugger holds the scan, as trust-runtime's `status` reports it (specification
    /// 11 §6.9).
    fn debug_hold(&self) -> Option<&'static str> {
        self.debug.hold_label()
    }

    fn control_mode_label(&self) -> Option<String> {
        let label = match self.platform.control_mode() {
            ControlMode::Production => "production",
            ControlMode::Debug => "debug",
        };
        Some(String::from(label))
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

    /// The loop's own session when the device gave it one, else the platform's.
    fn with_transfers<R>(
        &self,
        use_session: impl FnOnce(&mut TransferSession) -> R,
    ) -> Result<R, TransferSessionError> {
        if self.install.keeps_transfers() {
            self.install.with_transfers(use_session)
        } else {
            self.platform.with_transfers(use_session)
        }
    }

    /// An upload in the loop's own session, written into its program store while the resource is
    /// Stopped (specification 34 §5.1), else in the platform's session.
    fn begin_upload(
        &self,
        bytes: u64,
        digest: [u8; 32],
    ) -> Result<Result<TransferId, String>, TransferSessionError> {
        if self.install.keeps_transfers() {
            let stopped = matches!(self.resource.state(), ResourceState::Stopped);
            self.install.begin_upload(bytes, digest, stopped)
        } else {
            self.platform.with_transfers(|session| {
                session
                    .begin(bytes, digest)
                    .map_err(|error| error.to_string())
            })
        }
    }

    /// A part of an upload in the loop's own session, which a store upload writes into the store
    /// (specification 34 §5.1), else in the platform's session.
    fn receive_part(
        &self,
        transfer: TransferId,
        offset: u64,
        encoded: &str,
        crc: u32,
    ) -> Result<Result<usize, String>, TransferSessionError> {
        if self.install.keeps_transfers() {
            self.install.receive_part(transfer, offset, encoded, crc)
        } else {
            self.platform
                .with_transfers(|session| part_in_memory(session, transfer, offset, encoded, crc))
        }
    }

    /// The bytes of an upload the loop wrote into its store, where they lie in its slot.
    fn stored_upload(&self, upload: &StoredUpload) -> Option<&'static [u8]> {
        self.install.stored_region(upload)
    }

    /// Begins the stepped commit of an upload the loop wrote into its store: its header and magic
    /// (specification 34 §5.1, §11.2).
    fn commit_stored(
        &self,
        upload: StoredUpload,
        checked: Checked,
    ) -> Result<ProgramIdentity, String> {
        self.install.begin_stored(upload, checked)
    }

    fn program_identities(&self) -> Option<ProgramIdentities> {
        self.platform.program_identities()
    }

    /// The identities of the loop's own program store when it has one, read in bounded reads, else the
    /// platform's (specification 34 §11.2).
    fn read_program_identities(&self) -> Result<Option<ProgramIdentities>, String> {
        match self.install.identities() {
            Some(identities) => identities.map(Some).map_err(|error| error.to_string()),
            None => Ok(self.platform.program_identities()),
        }
    }

    /// Whether the device gave its loop a program store (specification 34 §11.2).
    fn has_program_store(&self) -> bool {
        self.install.has_store()
    }

    /// The retained-value check of an install, read from the newest checkpoint (decision G13).
    fn check_retained_values(&self, admitted: &Admitted<'_>) -> Result<(), RuntimeError> {
        self.install.check_retained(admitted)
    }

    /// Begins the stepped commit of the checked container; the loop writes the `installed` reply
    /// when the last step is done (specification 34 §11.2, PORT-05).
    fn commit_program(&self, container: Vec<u8>) -> Result<ProgramIdentity, String> {
        self.install.begin(container)
    }
}

impl<P: RunnerPlatform, C: MonotonicClock> RunnerHost<'_, P, C> {
    /// `bytecode.reload` on a loop that can change its program online (specification 34 §11.3): the
    /// shared rule admits the request ([`reload::admit`]), and the change is queued for the program's
    /// next boundary. The reply built here is `reloaded`, which the loop holds and writes when the
    /// change is applied; a failure replaces it (the loop's `finish_change`). A loop that has no
    /// program to change refuses with the reason it has none.
    fn queue_online_change(&self, request: &ControlRequest) -> ControlResponse {
        let id = request.id;
        let program = match reload::admit(id, request.params.clone(), self) {
            Ok(program) => program,
            Err(refusal) => return refusal,
        };
        if let Some(reason) = self.program.missing_program() {
            return no_installed_program(id, reason);
        }
        match self.change.begin(program) {
            Ok(()) => ControlResponse::ok(id, serde_json::json!({"status": "reloaded"})),
            Err(refusal) => ControlResponse::error(id, refusal),
        }
    }
}

impl<P: RunnerPlatform, C: MonotonicClock> ExtensionDispatch for RunnerHost<'_, P, C> {
    /// The loop answers `bytecode.reload` itself when it was given a loader that builds candidates;
    /// every other operation, and `bytecode.reload` on a loop without one, goes to the platform.
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse> {
        if request.r#type == reload::OPERATION && self.change.enabled() {
            return Some(self.queue_online_change(request));
        }
        self.platform.dispatch_extension(request)
    }

    /// The loop's own debugger answers the debug operations when a debugger is attached to the
    /// program, the interpreter's or a compiled program's (specification 44 §13.2); a program
    /// without one leaves them to the platform, which has none.
    fn debugger(&self) -> Option<&dyn Debugger> {
        let attached = self.program.debug_state().is_some();
        #[cfg(feature = "native")]
        let attached = attached || self.program.compiled_state().is_some();
        attached.then_some(self as &dyn Debugger)
    }

    fn audit_details(&self, request: &ControlRequest) -> Option<serde_json::Value> {
        self.platform.audit_details(request)
    }
}

/// The platform owns the Live Values queues and the published snapshot, as `ControlState` does
/// for trust-runtime; the loop only passes them through.
impl<P: RunnerPlatform, C> IoQueuePort for RunnerHost<'_, P, C> {
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

impl<P: RunnerPlatform, C> VariableQueuePort for RunnerHost<'_, P, C> {
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
