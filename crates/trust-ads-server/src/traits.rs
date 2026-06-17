use std::sync::Arc;

use trust_ads_core::{PointQuality, SymbolDescriptor, SymbolSnapshot};

use crate::{AdsServerAuditEvent, AdsServerError, ClientId};

/// Supplies the ADS symbol table owned by the server integration.
pub trait SymbolSource {
    /// Returns the current symbol snapshot.
    fn snapshot(&self) -> Arc<SymbolSnapshot>;

    /// Returns the current ADS symbol version.
    fn version(&self) -> u32;
}

/// Reads encoded ADS bytes for a resolved symbol.
pub trait ValueIo {
    /// Reads the current value bytes and point quality for `symbol`.
    ///
    /// Implementations return already encoded ADS bytes. The runtime integration
    /// owns IEC value conversion through `trust-ads-core`.
    ///
    /// # Errors
    ///
    /// Returns an ADS server error when the symbol cannot be read or encoded.
    fn read(&self, symbol: &SymbolDescriptor) -> Result<(Vec<u8>, PointQuality), AdsServerError>;
}

/// Applies a write requested by an external ADS client.
pub trait RuntimeWritePort {
    /// Writes raw ADS bytes for `symbol` on behalf of `client`.
    ///
    /// The runtime integration decodes, gates, enqueues at the scan boundary, and
    /// audits the request. The protocol crate never sees runtime values.
    ///
    /// # Errors
    ///
    /// Returns an ADS server error when policy, decoding, or enqueue fails.
    fn write(
        &self,
        symbol: &SymbolDescriptor,
        bytes: &[u8],
        client: &ClientId,
    ) -> Result<(), AdsServerError>;
}

/// Enforces inbound ADS client trust.
pub trait ClientPolicy {
    /// Returns whether this client identity and source are permitted.
    fn permits(&self, client: &ClientId) -> bool;
}

/// Records ADS server security and write events.
pub trait AuditSink {
    /// Records an audit event.
    fn record(&self, event: &AdsServerAuditEvent);
}

/// Supplies monotonic/runtime time to the ADS server.
pub trait Clock {
    /// Returns current time in milliseconds.
    fn now_ms(&self) -> u64;
}
