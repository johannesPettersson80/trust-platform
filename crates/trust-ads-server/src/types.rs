use trust_ads_core::AmsNetId;

use crate::AdsServerError;

/// ADS client identity observed by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientId {
    /// Client AMS Net ID asserted in the AMS header.
    pub ams_net_id: AmsNetId,
    /// Source IP observed by the transport, when available.
    pub source_ip: Option<String>,
}

impl ClientId {
    /// Creates a client identity with no source IP.
    #[must_use]
    pub fn new(ams_net_id: AmsNetId) -> Self {
        Self {
            ams_net_id,
            source_ip: None,
        }
    }

    /// Returns this identity with a source IP attached.
    #[must_use]
    pub fn with_source_ip(mut self, source_ip: impl Into<String>) -> Self {
        self.source_ip = Some(source_ip.into());
        self
    }
}

/// Kind of ADS server audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsServerAuditKind {
    /// An ADS write was accepted and enqueued.
    WriteAccepted,
    /// An ADS write was rejected.
    WriteRejected,
    /// A client was rejected by policy before a command was served.
    PolicyRejected,
}

/// Audit event emitted by the ADS server integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsServerAuditEvent {
    /// Event kind.
    pub kind: AdsServerAuditKind,
    /// Client identity.
    pub client: ClientId,
    /// Optional ADS symbol name.
    pub symbol: Option<String>,
    /// Optional ADS/IEC source type name.
    pub value_type: Option<String>,
    /// Result of the audited operation.
    pub result: Result<(), AdsServerError>,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
}

impl AdsServerAuditEvent {
    /// Creates an ADS server audit event.
    #[must_use]
    pub fn new(
        kind: AdsServerAuditKind,
        client: ClientId,
        symbol: Option<String>,
        result: Result<(), AdsServerError>,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            kind,
            client,
            symbol,
            value_type: None,
            result,
            timestamp_ms,
        }
    }

    /// Attaches a value type to this audit event.
    #[must_use]
    pub fn with_value_type(mut self, value_type: impl Into<String>) -> Self {
        self.value_type = Some(value_type.into());
        self
    }
}
