use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

mod shm;
mod transport;

pub use transport::T0Transport;

/// Communication route class for runtime-to-runtime exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeRoute {
    /// Same-host deterministic HardRT route.
    T0HardRt,
    /// Non-deterministic network route (mesh/IP).
    MeshIp,
}

/// Canonical communication QoS tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QosTier {
    /// T0 HardRT traffic (same host, deterministic).
    T0HardRt,
    /// T1 fast mesh traffic (best effort).
    T1Fast,
    /// T2 operations mesh traffic (reliable ops/cfg/cmd).
    T2Ops,
    /// T3 diagnostics mesh traffic (best effort telemetry).
    T3Diag,
}

impl QosTier {
    /// Determine whether a concrete route is legal for this QoS tier.
    #[must_use]
    pub const fn route_is_legal(self, route: RealtimeRoute) -> bool {
        match self {
            Self::T0HardRt => matches!(route, RealtimeRoute::T0HardRt),
            Self::T1Fast | Self::T2Ops | Self::T3Diag => matches!(route, RealtimeRoute::MeshIp),
        }
    }
}

/// Deterministic error class for T0 communication operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0ErrorCode {
    /// Channel or bind is not configured.
    NotConfigured,
    /// Request violates a contract invariant.
    ContractViolation,
    /// Strict schema hash binding failed.
    SchemaMismatch,
    /// Fresh data is stale/unavailable within bounded policy.
    StaleData,
    /// Transport state is not ready for read/write.
    TransportFailure,
}

/// Canonical error code model for communication contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommsErrorCode {
    /// Channel/bind/configuration is missing.
    NotConfigured,
    /// HardRT contract was violated.
    RtContractViolation,
    /// Schema/hash compatibility check failed.
    SchemaMismatch,
    /// Caller identity is not authorized.
    PermissionDenied,
    /// Operation timed out before completion.
    Timeout,
    /// Target peer/runtime is unavailable.
    PeerNotAvailable,
    /// Transport readiness or integrity failure.
    TransportFailure,
    /// Data is stale beyond policy threshold.
    StaleData,
}

impl CommsErrorCode {
    /// Deterministic remediation hint surfaced to callers/UI.
    pub const fn remediation_hint(self) -> &'static str {
        match self {
            Self::NotConfigured => "Complete comms configuration and rebind handles.",
            Self::RtContractViolation => {
                "Use pre-bound T0 handles and fixed-layout payloads; generic IP mesh is non-HardRT."
            }
            Self::SchemaMismatch => "Refresh schema/catalog and rebind with matching schema hash.",
            Self::PermissionDenied => "Use an identity/role allowed by policy.",
            Self::Timeout => "Retry after verifying transport health and peer responsiveness.",
            Self::PeerNotAvailable => "Verify peer presence/liveliness before retrying.",
            Self::TransportFailure => "Verify channel readiness and runtime transport setup.",
            Self::StaleData => "Wait for fresh publisher updates or reduce stale thresholds.",
        }
    }
}

impl From<T0ErrorCode> for CommsErrorCode {
    fn from(value: T0ErrorCode) -> Self {
        match value {
            T0ErrorCode::NotConfigured => Self::NotConfigured,
            T0ErrorCode::ContractViolation => Self::RtContractViolation,
            T0ErrorCode::SchemaMismatch => Self::SchemaMismatch,
            T0ErrorCode::StaleData => Self::StaleData,
            T0ErrorCode::TransportFailure => Self::TransportFailure,
        }
    }
}

/// Structured error returned by T0 operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T0Error {
    /// Deterministic error code.
    pub code: T0ErrorCode,
    /// Human-readable error detail.
    pub message: String,
}

impl T0Error {
    fn new(code: T0ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for T0Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for T0Error {}

/// Bounded policy for a T0 channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct T0ChannelPolicy {
    /// Fixed slot size (bytes) for payload transfer.
    pub slot_size: usize,
    /// Read misses before stale data is surfaced.
    pub stale_after_reads: u8,
    /// Maximum bounded retries for unstable writer sequence.
    pub max_spin_retries: u8,
    /// Maximum bounded spin time for unstable writer sequence.
    pub max_spin_time_us: u64,
}

impl Default for T0ChannelPolicy {
    fn default() -> Self {
        Self {
            slot_size: 256,
            stale_after_reads: 2,
            max_spin_retries: 3,
            max_spin_time_us: 50,
        }
    }
}

/// Page-pinning policy for OS shared-memory channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0PinningMode {
    /// SHM page pinning is mandatory; startup fails if locking fails.
    Required,
    /// SHM page pinning is attempted, but startup continues if locking fails.
    BestEffort,
    /// SHM page pinning is disabled.
    Disabled,
}

/// Pinning backend selection for deterministic startup behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0PinningProvider {
    /// Use OS-backed page locking.
    Os,
    /// Disable OS-backed page locking.
    None,
}

/// Startup configuration for T0 SHM channel provisioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T0ShmConfig {
    /// Root directory for shared channel mappings.
    pub root_dir: PathBuf,
    /// Pinning requirement policy.
    pub pinning_mode: T0PinningMode,
    /// Pinning backend.
    pub pinning_provider: T0PinningProvider,
}

impl T0ShmConfig {
    /// Build a config that reuses a caller-provided root (e.g. multi-process tests).
    #[must_use]
    pub fn with_root(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            ..Self::default()
        }
    }
}

impl Default for T0ShmConfig {
    fn default() -> Self {
        static SHM_ROOT_NONCE: AtomicU64 = AtomicU64::new(0);
        let nonce = SHM_ROOT_NONCE.fetch_add(1, Ordering::Relaxed);
        let root_dir =
            std::env::temp_dir().join(format!("trust-runtime-t0-{}-{nonce}", std::process::id()));
        Self {
            root_dir,
            pinning_mode: T0PinningMode::Required,
            pinning_provider: T0PinningProvider::Os,
        }
    }
}

/// Channel ownership contract for SPSC T0 channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0ChannelOwnership {
    /// Publisher writes and subscriber reads.
    PublisherWrites,
}

impl T0ChannelOwnership {
    /// Stable text tag for readiness/audit payloads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublisherWrites => "publisher_writes",
        }
    }
}

/// Registration contract for a T0 channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T0ChannelSpec {
    /// Logical channel identifier.
    pub channel_id: String,
    /// Stable schema identifier.
    pub schema_id: String,
    /// Schema version.
    pub schema_version: u32,
    /// Concrete schema layout hash.
    pub schema_hash: String,
    /// Fixed T0 policy.
    pub policy: T0ChannelPolicy,
    /// Producer/consumer ownership contract.
    pub ownership: T0ChannelOwnership,
}

impl T0ChannelSpec {
    /// Build a default channel contract where schema id tracks channel id.
    #[must_use]
    pub fn new(
        channel_id: impl Into<String>,
        schema_hash: impl Into<String>,
        policy: T0ChannelPolicy,
    ) -> Self {
        let channel_id = channel_id.into();
        Self {
            schema_id: channel_id.clone(),
            channel_id,
            schema_version: 1,
            schema_hash: schema_hash.into(),
            policy,
            ownership: T0ChannelOwnership::PublisherWrites,
        }
    }
}

