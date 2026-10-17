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
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::WallClock;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use crate::access::{AccessRole, AuthFailure};
use crate::events::RuntimeEvent;
use crate::metrics::RuntimeMetricsSnapshot;
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::commands::CommandSendError;
use crate::status::{IoDriverStatus, StatusSettings};

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

/// The runtime's event store. It keeps at most
/// [`EVENT_STORE_CAPACITY`](crate::events::EVENT_STORE_CAPACITY) events, dropping the oldest;
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

    /// Asks the debugger to hold the scan at its next statement.
    fn debugger_pause(&self);

    /// Releases a debugger hold, so the scan returns to cyclic execution.
    fn debugger_continue(&self);

    /// Updates the restart request that the scan takes at its next boundary or in the faulted
    /// wait. An unreadable request slot is left as it is.
    fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>));

    /// Whether `restart` starts a stopped resource again (specification 34, STATE-08).
    ///
    /// On a device without an operating system the program takes the place of the process: after
    /// `shutdown` the device stays reachable and `restart` starts the installed program again. On
    /// a computer `shutdown` ends the trust-runtime process, so a stopped resource refuses it.
    fn starts_when_stopped(&self) -> bool;
}

/// The values the platform knows that the status operations report: configuration, I/O driver
/// health and the platform's posture.
///
/// A method that returns `None` reports a value that cannot be read, and the operation answers
/// `<value> unavailable`.
pub trait StatusPort {
    /// Name of the resource.
    fn resource_name(&self) -> &str;

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
}

/// What the running resource records and the status operations report: the debugger hold, the
/// control mode, the metrics and the event store.
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

    /// Reads the event store, oldest event first.
    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R>;
}

/// Operations that only the platform answers.
pub trait ExtensionDispatch {
    /// Answers `request`, or `None` when the platform has no such operation.
    fn dispatch_extension(&self, request: &ControlRequest) -> Option<ControlResponse>;

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
        + ExtensionDispatch
{
}
