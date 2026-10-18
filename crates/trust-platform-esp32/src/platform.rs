//! The resource loop's platform on the C6 (specification 34, ARCH-11 and §3.3): the values the
//! chip reports, the access rights of its USB control link, and its hardware watchdog.
//!
//! A USB client is local (specification 11 §6.9.5), so the link carries no transport label and
//! no control token is configured yet (TARGET-02 keeps secrets off the project); a local client
//! therefore acts as `Admin`, exactly as trust-runtime's local socket without a configured token.
//! The chip has no wall clock until slice T6, no audit store and no platform operation yet, so
//! those ports answer as an absent value does, not with an invented one.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::ports::{HardwareWatchdog, TimeSpan, WallClock};
use trust_runtime_services::access::{resolve_role, AccessRole, AuthFailure};
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, ExtensionDispatch, StatusPort,
};
use trust_runtime_services::protocol::{ControlRequest, ControlResponse};
use trust_runtime_services::runner::RunnerPlatform;
use trust_runtime_services::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

/// The C6's platform for the single-thread resource loop, over the watchdog `W` the adapter
/// supplies and the values the installed application's record configures.
pub struct Platform<W> {
    watchdog: W,
    resource_name: SmolStr,
    settings: StatusSettings,
    debug_enabled: bool,
}

impl<W> Platform<W> {
    /// A platform over `watchdog` that reports `resource_name`, the runtime `settings` and
    /// whether the debug surface is `debug_enabled`.
    #[must_use]
    pub fn new(
        watchdog: W,
        resource_name: SmolStr,
        settings: StatusSettings,
        debug_enabled: bool,
    ) -> Self {
        Self {
            watchdog,
            resource_name,
            settings,
            debug_enabled,
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

impl<W: HardwareWatchdog> RunnerPlatform for Platform<W> {}
