//! Additive connector status schema shared by process-image and supervisory links.

use serde::{Deserialize, Serialize};

/// Current connector status schema version.
pub const CONNECTOR_STATUS_SCHEMA_VERSION: u32 = 1;

/// Connector execution tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    /// Scan-cycle process image driver.
    ProcessImage,
    /// Supervisory client that talks to another system outside the scan loop.
    SupervisoryClient,
    /// Supervisory server exposed by this runtime.
    SupervisoryServer,
}

/// Connector protocol family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorProtocol {
    /// Beckhoff ADS.
    Ads,
    /// OPC UA.
    Opcua,
    /// Modbus TCP.
    ModbusTcp,
    /// MQTT.
    Mqtt,
    /// EtherCAT.
    Ethercat,
    /// Local GPIO.
    Gpio,
    /// Simulated I/O.
    Simulated,
    /// Loopback I/O.
    Loopback,
    /// Unknown or custom process-image driver.
    Unknown,
}

/// Normalized connector lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorState {
    /// Connector is explicitly disabled.
    Disabled,
    /// Connector has valid configuration but is not started yet.
    Configured,
    /// Connector is starting or establishing first contact.
    Starting,
    /// Connector is ready and fresh enough for normal operation.
    Ready,
    /// Connector works with reduced quality or some degraded points.
    Degraded,
    /// Connector is attempting to reconnect.
    Reconnecting,
    /// Connector previously worked but is no longer fresh.
    Stale,
    /// Connector is running but missing prerequisite runtime state.
    NotReady,
    /// Connector is faulted.
    Faulted,
}

/// Normalized connector health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorHealth {
    /// Healthy.
    Ok,
    /// Degraded but not fully faulted.
    Degraded,
    /// Faulted.
    Faulted,
    /// Not enough evidence to classify.
    Unknown,
}

/// Discovery confidence shared by discovery and status surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryConfidence {
    /// Protocol-level proof succeeded.
    Confirmed,
    /// Strong evidence exists, but full protocol proof was not obtained.
    Likely,
    /// Only TCP or transport reachability was observed.
    PortReachable,
    /// No useful discovery evidence is available.
    Unavailable,
}

/// Per-point quality. `Stale` is intentionally distinct from connector-level
/// stale: one point can be stale while the connector still reports degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointQuality {
    /// Point is fresh and usable.
    Good,
    /// Point value is old.
    Stale,
    /// Point read/write failed.
    Bad,
    /// Protocol or device does not support this point.
    Unsupported,
    /// Point is temporarily unavailable.
    Unavailable,
    /// Write has been accepted locally but not confirmed.
    WritePending,
    /// Last write failed.
    WriteFailed,
}

/// Reconnect behavior projected from existing per-connector settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconnectPolicy {
    /// No reconnect behavior.
    Disabled,
    /// Fixed-delay reconnect.
    FixedDelay,
    /// Exponential backoff reconnect.
    ExponentialBackoff,
    /// Reconnect is owned outside this contract.
    ExternallyManaged,
}

/// Point direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointDirection {
    /// Read-only point.
    Read,
    /// Write-only point.
    Write,
    /// Read-write point.
    ReadWrite,
}

/// Connector point metadata without runtime-storage coupling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorPointMetadata {
    /// Runtime-facing point name.
    pub name: String,
    /// Protocol-native point address, symbol, topic, or node id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// IEC or protocol data type when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
    /// Read/write direction.
    pub direction: PointDirection,
}

/// Connector point status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorPointStatus {
    /// Static point metadata.
    pub metadata: ConnectorPointMetadata,
    /// Current normalized point quality.
    pub quality: PointQuality,
    /// Last point update timestamp in milliseconds when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update_ms: Option<u64>,
    /// Human-readable detail for diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Aggregated point counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorPointCounts {
    /// Total point count.
    pub total: usize,
    /// Good/fresh point count.
    pub good: usize,
    /// Degraded/stale/bad point count.
    pub degraded: usize,
    /// Unavailable or unsupported point count.
    pub unavailable: usize,
}

/// Additive connector status report for a single connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorStatusReport {
    /// Schema version for compatibility.
    pub schema_version: u32,
    /// Stable connector id.
    pub connector_id: String,
    /// Optional display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Protocol.
    pub protocol: ConnectorProtocol,
    /// Execution tier.
    pub kind: ConnectorKind,
    /// Endpoint, interface, route, or other connector target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Normalized lifecycle state.
    pub state: ConnectorState,
    /// Normalized health.
    pub health: ConnectorHealth,
    /// Discovery/evidence confidence.
    pub confidence: DiscoveryConfidence,
    /// Reconnect policy projected from existing settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnect_policy: Option<ReconnectPolicy>,
    /// Last error detail when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Last connector state transition timestamp in milliseconds when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_transition_ms: Option<u64>,
    /// Connector freshness in milliseconds when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_ms: Option<u64>,
    /// Aggregated point counts.
    pub point_counts: ConnectorPointCounts,
    /// Optional per-point details.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<ConnectorPointStatus>,
}
