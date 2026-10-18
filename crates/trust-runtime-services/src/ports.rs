//! What a platform supplies to the shared control services (specification 34, ARCH-11 and §3.3).
//!
//! Each port is a narrow view of platform state or mechanism: the access credentials, the audit
//! and event sinks, the resource scheduler, the values the platform knows, the records the
//! running resource keeps and the operations that only the platform answers. The clocks are the
//! engine's platform contracts in [`trust_runtime_core::ports`]. The shared services read and act
//! through these ports and hold no platform state themselves.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::WallClock;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use crate::access::{AccessRole, AuthFailure};
use crate::capabilities::LastReset;
use crate::debug::Debugger;
use crate::events::RuntimeEvent;
use crate::installation::Admitted;
use crate::live_values::io_requests::IoQueuePort;
use crate::live_values::var_requests::VariableQueuePort;
use crate::metrics::RuntimeMetricsSnapshot;
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::commands::CommandSendError;
use crate::status::{IoDriverStatus, StatusSettings};
use crate::transfer::TransferSession;

/// Access rights of the control connection.
pub trait AccessPort {
    /// The role `request` acts with, decided from the platform's credentials with
    /// [`crate::access::resolve_role`].
    ///
    /// # Errors
    ///
    /// The [`AuthFailure`] that refuses the request before its role is known.
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure>;

    /// Whether the debug surface is enabled; its operations are refused otherwise.
    fn debug_enabled(&self) -> bool;
}

/// The audit record of one control request.
#[derive(Debug)]
pub struct AuditRecord<'a> {
    /// Request number, or 0 for a request that could not be read.
    pub request_id: u64,
    /// Operation name, or `invalid` for a request that could not be read.
    pub request_type: SmolStr,
    /// Caller's correlation id.
    pub correlation_id: Option<&'a str>,
    /// Whether the request succeeded.
    pub ok: bool,
    /// Error message of a refused or failed request.
    pub error: Option<SmolStr>,
    /// Whether the request carried a token.
    pub auth_present: bool,
    /// Transport label of the client.
    pub client: Option<&'a str>,
    /// Operation details with secrets removed.
    pub details: Option<serde_json::Value>,
}

/// Where audit records go.
pub trait AuditSink {
    /// Records `record` and returns the id of the stored record, if it has one.
    fn record_audit(&self, record: AuditRecord<'_>) -> Option<SmolStr>;
}

/// The runtime's event store. It keeps at most the store's configured
/// `runtime.events.capacity` events (1,000 by default on a computer), dropping the oldest;
/// [`store_event`](crate::events::store_event) applies that bound.
pub trait EventSink {
    /// Appends `event` as the newest event; a store that cannot be written drops it.
    fn record_event(&self, event: RuntimeEvent);
}

/// The resource scheduler of one runtime.
pub trait ResourcePort {
    /// Current resource state.
    fn resource_state(&self) -> ResourceState;

    /// The fault that stopped the resource, if any.
    fn last_error(&self) -> Option<RuntimeError>;

    /// Whether the scan is inside a call that did not return.
    fn is_hung(&self) -> bool;

    /// Queues a pause of the scan cycles without waiting.
    ///
    /// # Errors
    ///
    /// The [`CommandSendError`] of a command the scheduler did not take.
    fn pause_resource(&self) -> Result<(), CommandSendError>;

    /// Queues a resume of the scan cycles without waiting.
    ///
    /// # Errors
    ///
    /// The [`CommandSendError`] of a command the scheduler did not take.
    fn resume_resource(&self) -> Result<(), CommandSendError>;

    /// Whether the runtime runs in debug control mode; an unreadable mode counts as production.
    fn debug_control_mode(&self) -> bool;

    /// Whether the debugger cannot hold the scan because the program has no line table, so a
    /// debug-mode `pause` is refused before it asks the debugger to hold anything (specification
    /// 11 §6.13.3). The default is a runtime that can always debug.
    fn debugger_unavailable(&self) -> bool {
        false
    }

    /// Asks the debugger to hold the scan at its next statement.
    fn debugger_pause(&self);

    /// Releases a debugger hold, so the scan returns to cyclic execution.
    fn debugger_continue(&self);

    /// Updates the restart request that the scan takes at its next boundary or in the faulted
    /// wait. An unreadable request slot is left as it is.
    fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>));

    /// Whether `restart` starts a stopped resource again (specification 34, STATE-08).
    ///
    /// On every runtime the program takes the place of the process: after `shutdown` the runtime
    /// stays reachable and `restart` starts the installed program again. A platform whose
    /// `shutdown` ended the process would have no scan to take the restart, so its stopped
    /// resource would refuse it.
    fn starts_when_stopped(&self) -> bool;

    /// Why the resource has no installed program, when it has none: a device whose application
    /// region holds no program, or one that the device refused (specification 34, STATE-08).
    /// `restart` is then refused with the reason. A runtime with a program has none, the
    /// default.
    fn missing_program(&self) -> Option<&str> {
        None
    }
}

/// How the runtime's program control behaves across `shutdown`: whether the runtime still answers
/// once its program is stopped, and what `restart` then does (specification 11, `status`;
/// specification 34, STATE-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramControl {
    /// The runtime still answers after `shutdown`, and `restart` starts the program again: the
    /// program takes the place of the process, as on a device without an operating system and,
    /// since the owner's decision of 18 October 2026, as on trust-runtime on a computer
    /// (spec 34 STATE-08).
    KeepsControl,
    /// `shutdown` ends the runtime's process, so a stopped runtime has no scan loop to take a
    /// `restart`, as an earlier trust-runtime on a computer did (spec 34 STATE-08).
    Process,
}

impl ProgramControl {
    /// The wire name of the posture that `status` reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepsControl => "keeps_control",
            Self::Process => "process",
        }
    }
}

/// The values the platform knows that the status and `capabilities` operations report:
/// configuration, I/O driver health, the platform's posture, the device descriptor admission
/// reads and the reset a device without an operating system read at boot.
///
/// A method that returns `None` reports a value that cannot be read, and the operation answers
/// `<value> unavailable`. The descriptor and last-reset methods default to a computer's values,
/// which a platform that composes the hosted runtime keeps.
pub trait StatusPort {
    /// Name of the resource.
    fn resource_name(&self) -> &str;

    /// The platform the runtime runs on: `<os>-<arch>` on a computer, for example `linux-x86_64`,
    /// and the board name on a device, for example `stm32f407`.
    fn platform(&self) -> &str;

    /// The runtime's stable runtime ID (specification 11, `status` and runtime discovery): the
    /// same value the runtime announces by mDNS, and the same across restarts.
    fn runtime_id(&self) -> &str;

    /// Whether the runtime's program control survives `shutdown` (spec 34 STATE-08).
    fn program_control(&self) -> ProgramControl;

    /// The runtime settings that `status` reports.
    fn status_settings(&self) -> Option<StatusSettings>;

    /// Health of each I/O driver, in registration order.
    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>>;

    /// The platform's realtime posture block of `status`.
    fn realtime_status(&self) -> Option<serde_json::Value>;

    /// Whether the realtime posture reports an error.
    fn realtime_has_errors(&self) -> Option<bool>;

    /// The `openot_persistence` block of `status`: the state of the `OpenOT` document
    /// persistence.
    fn openot_persistence(&self) -> Option<serde_json::Value>;

    /// The device descriptor admission reads (specification 34 §5.1.1, COMPAT-02); the
    /// `capabilities` operation serves it. The default is a computer's descriptor
    /// (specification 34, HOST-02).
    fn device_descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor::hosted()
    }

    /// The reset a device without an operating system read at boot, or `None` on a computer, which
    /// starts as a new process (specification 11, the runtime device capabilities contract). The
    /// default is no last reset.
    fn last_reset(&self) -> Option<LastReset> {
        None
    }
}

/// One installed program, as `program.active` reports it: its install sequence, the SHA-256 of the
/// program and its length in bytes (specification 34 §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramIdentity {
    /// The install sequence the store gave the program.
    pub sequence: u64,
    /// The SHA-256 of the program.
    pub program_sha256: [u8; 32],
    /// The program's length in bytes.
    pub bytes: usize,
}

/// The program the store selects and the program before it, as `program.active` reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramIdentities {
    /// The selected program, when the store holds one.
    pub active: Option<ProgramIdentity>,
    /// The program before the selected one, when the store holds one.
    pub previous: Option<ProgramIdentity>,
}

/// Why the platform gives out no transfer session (specification 34, PORT-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferSessionError {
    /// The platform keeps no transfer session, so the transfer operations are unsupported.
    Unsupported,
    /// The platform's session cannot be locked now.
    Unavailable,
}

impl TransferSessionError {
    /// The refusal to request `id` when the platform gives out no session.
    #[must_use]
    pub fn refusal(self, id: u64) -> ControlResponse {
        let text = match self {
            Self::Unsupported => "unsupported request",
            Self::Unavailable => "transfer session unavailable",
        };
        ControlResponse::error(id, text.into())
    }
}

/// What the running resource records and the status operations report: the debugger hold, the
/// control mode, the metrics and the event store. The transfer session and the installed program's
/// identities are held the same way, and a platform that keeps neither keeps the defaults.
///
/// A method that returns `None` reports a value that cannot be read, and the operation answers
/// `<value> unavailable`.
pub trait RecordsPort {
    /// Wire label of the reason a debugger hold keeps the scan paused, if one is active.
    fn debug_hold(&self) -> Option<&'static str>;

    /// Lowercase name of the control mode: `production` or `debug`.
    fn control_mode_label(&self) -> Option<String>;

    /// A snapshot of the runtime metrics.
    fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot>;

    /// The newest events the event store keeps (`runtime.events.capacity`, specification 11, the
    /// event store). `None` reports a capacity that cannot be read.
    fn event_capacity(&self) -> Option<usize>;

    /// Reads the event store, oldest event first.
    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R>;

    /// Runs `use_session` on the platform's transfer session, which holds the transfers of its boot
    /// (specification 34, PORT-05). The default is no session, so the transfer operations answer
    /// `unsupported request`.
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
    /// 34 §11.2). The default is no program store, so `program.active` answers `unsupported request`.
    fn program_identities(&self) -> Option<ProgramIdentities> {
        None
    }

    /// Whether the platform keeps a program store that `program.install` commits to (specification
    /// 34 §11.2). The default is none: `program.install` then answers `unsupported request` after its
    /// state check, and takes nothing.
    fn has_program_store(&self) -> bool {
        false
    }

    /// The retained-value check of specification 11 §6.7.1 (decision G13), run before any write: the
    /// warm start of the admitted program with the retained values of the newest checkpoint on a
    /// chip, or of the retain file on a computer. It is asked only of a platform that has a program
    /// store. The default refuses, so a store cannot skip the check by leaving it out.
    ///
    /// # Errors
    ///
    /// The warm start's own error, which names the retained value and its reason.
    fn check_retained_values(&self, _admitted: &Admitted<'_>) -> Result<(), RuntimeError> {
        Err(RuntimeError::ControlError(
            "the platform does not check retained values".into(),
        ))
    }

    /// Commits the complete container `container`, already checked, to the platform's program store
    /// (specification 34 §11.2), and returns the identity the store gives the program. The default
    /// refuses.
    ///
    /// # Errors
    ///
    /// The store's refusal or failure, as plain text.
    fn commit_program(&self, _container: &[u8]) -> Result<ProgramIdentity, String> {
        Err("the platform does not store programs".into())
    }
}

/// Operations that only the platform answers.
pub trait ExtensionDispatch {
    /// Answers `request`, or `None` when the platform has no such operation.
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse>;

    /// The platform's debugger, from which the shared pipeline answers the debug operations
    /// (specification 11 §6.9 and §6.13). A platform that returns none answers them itself through
    /// [`Self::dispatch_extension`], as trust-runtime does with its own debugger.
    fn debugger(&self) -> Option<&dyn Debugger> {
        None
    }

    /// Audit details of `request` with secrets removed, for operations that have details.
    fn audit_details(&self, request: &ControlRequest) -> Option<serde_json::Value>;
}

/// Everything the control request pipeline needs from a platform.
pub trait ControlHost:
    AccessPort
    + AuditSink
    + EventSink
    + WallClock
    + ResourcePort
    + StatusPort
    + RecordsPort
    + IoQueuePort
    + VariableQueuePort
    + ExtensionDispatch
{
}

impl<T> ControlHost for T where
    T: AccessPort
        + AuditSink
        + EventSink
        + WallClock
        + ResourcePort
        + StatusPort
        + RecordsPort
        + IoQueuePort
        + VariableQueuePort
        + ExtensionDispatch
{
}
