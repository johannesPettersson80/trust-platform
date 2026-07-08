//! Pure state-mapping helpers for connector status adapters.

use crate::ads::diagnostics::AdsConnectionStatusState;
use crate::ads::AdsConnectionState;
use crate::io::{IoDriverErrorPolicy, IoDriverHealth};
use crate::opcua::OpcUaClientConnectionState;

use super::contract::{ConnectorHealth, ConnectorState};

/// Normalized state/health projection plus optional detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorStatusProjection {
    /// Normalized lifecycle state.
    pub state: ConnectorState,
    /// Normalized health.
    pub health: ConnectorHealth,
    /// Optional detail from the source status.
    pub detail: Option<String>,
}

impl ConnectorStatusProjection {
    /// Construct a projection without detail.
    #[must_use]
    pub const fn new(state: ConnectorState, health: ConnectorHealth) -> Self {
        Self {
            state,
            health,
            detail: None,
        }
    }

    /// Add detail to a projection.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// OPC UA server snapshot availability used by the server adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaServerSnapshotState {
    /// Server disabled.
    Disabled,
    /// Listener is starting.
    Starting,
    /// Listener is up but no runtime snapshot is available.
    NoSnapshot,
    /// Listener has live runtime snapshot data.
    SnapshotReady,
    /// Server faulted.
    Faulted,
}

/// MQTT session/freshness projection input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqttSessionProjection {
    /// Driver disabled.
    Disabled,
    /// No session is connected.
    Disconnected,
    /// Session is connecting.
    Connecting,
    /// Connected with fresh data.
    ConnectedFresh,
    /// Connected but data freshness is stale.
    ConnectedStale,
    /// Session faulted.
    Faulted,
}

/// Modbus runtime projection input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusProjection {
    /// Driver disabled.
    Disabled,
    /// Driver is ready.
    Ready,
    /// Read/write timed out.
    Timeout,
    /// Protocol-level error.
    ProtocolError,
    /// Driver faulted.
    Faulted,
}

/// EtherCAT bus projection input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthercatProjection {
    /// Driver disabled.
    Disabled,
    /// Bus is operational.
    Operational,
    /// Bus is degraded.
    Degraded,
    /// Bus is reconnecting.
    Reconnecting,
    /// Bus faulted.
    Faulted,
}

/// Project process-image driver health into the connector contract.
#[must_use]
pub fn io_driver_status(
    health: &IoDriverHealth,
    error_policy: IoDriverErrorPolicy,
) -> ConnectorStatusProjection {
    match health {
        IoDriverHealth::Ok => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        IoDriverHealth::Degraded { error } => {
            ConnectorStatusProjection::new(ConnectorState::Degraded, ConnectorHealth::Degraded)
                .with_detail(error.as_str())
        }
        IoDriverHealth::Faulted { error } => {
            let projection = match error_policy {
                IoDriverErrorPolicy::Fault => ConnectorStatusProjection::new(
                    ConnectorState::Faulted,
                    ConnectorHealth::Faulted,
                ),
                IoDriverErrorPolicy::Warn | IoDriverErrorPolicy::Ignore => {
                    ConnectorStatusProjection::new(
                        ConnectorState::Degraded,
                        ConnectorHealth::Degraded,
                    )
                }
            };
            projection.with_detail(error.as_str())
        }
    }
}

/// Project low-level ADS connection state into the connector contract.
#[must_use]
pub fn ads_connection_state_status(state: AdsConnectionState) -> ConnectorStatusProjection {
    match state {
        AdsConnectionState::Disconnected => {
            ConnectorStatusProjection::new(ConnectorState::Stale, ConnectorHealth::Degraded)
        }
        AdsConnectionState::Connecting => {
            ConnectorStatusProjection::new(ConnectorState::Starting, ConnectorHealth::Unknown)
        }
        AdsConnectionState::Connected => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        AdsConnectionState::Reconnecting => {
            ConnectorStatusProjection::new(ConnectorState::Reconnecting, ConnectorHealth::Degraded)
        }
        AdsConnectionState::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
    }
}

/// Project ADS report connection state into the connector contract.
#[must_use]
pub fn ads_connection_status_state(
    state: AdsConnectionStatusState,
    degraded_points: usize,
) -> ConnectorStatusProjection {
    match state {
        AdsConnectionStatusState::Connected if degraded_points == 0 => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        AdsConnectionStatusState::Connected => {
            ConnectorStatusProjection::new(ConnectorState::Degraded, ConnectorHealth::Degraded)
        }
        AdsConnectionStatusState::Reconnecting => {
            ConnectorStatusProjection::new(ConnectorState::Reconnecting, ConnectorHealth::Degraded)
        }
        AdsConnectionStatusState::NotReady => {
            ConnectorStatusProjection::new(ConnectorState::NotReady, ConnectorHealth::Unknown)
        }
        AdsConnectionStatusState::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
        AdsConnectionStatusState::Stale => {
            ConnectorStatusProjection::new(ConnectorState::Stale, ConnectorHealth::Degraded)
        }
        AdsConnectionStatusState::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        AdsConnectionStatusState::Unknown => {
            ConnectorStatusProjection::new(ConnectorState::NotReady, ConnectorHealth::Unknown)
        }
    }
}

/// Project OPC UA client connection state into the connector contract.
#[must_use]
pub fn opcua_client_status(
    state: OpcUaClientConnectionState,
    degraded_points: usize,
) -> ConnectorStatusProjection {
    match state {
        OpcUaClientConnectionState::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        OpcUaClientConnectionState::Configured => {
            ConnectorStatusProjection::new(ConnectorState::Configured, ConnectorHealth::Unknown)
        }
        OpcUaClientConnectionState::Connecting => {
            ConnectorStatusProjection::new(ConnectorState::Reconnecting, ConnectorHealth::Degraded)
        }
        OpcUaClientConnectionState::Connected if degraded_points == 0 => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        OpcUaClientConnectionState::Connected => {
            ConnectorStatusProjection::new(ConnectorState::Degraded, ConnectorHealth::Degraded)
        }
        OpcUaClientConnectionState::Reconnecting => {
            ConnectorStatusProjection::new(ConnectorState::Reconnecting, ConnectorHealth::Degraded)
        }
        OpcUaClientConnectionState::Stale => {
            ConnectorStatusProjection::new(ConnectorState::Stale, ConnectorHealth::Degraded)
        }
        OpcUaClientConnectionState::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
    }
}

/// Project OPC UA server snapshot availability into the connector contract.
#[must_use]
pub fn opcua_server_snapshot_status(state: OpcUaServerSnapshotState) -> ConnectorStatusProjection {
    match state {
        OpcUaServerSnapshotState::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        OpcUaServerSnapshotState::Starting => {
            ConnectorStatusProjection::new(ConnectorState::Starting, ConnectorHealth::Unknown)
        }
        OpcUaServerSnapshotState::NoSnapshot => {
            ConnectorStatusProjection::new(ConnectorState::NotReady, ConnectorHealth::Unknown)
        }
        OpcUaServerSnapshotState::SnapshotReady => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        OpcUaServerSnapshotState::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
    }
}

/// Project MQTT session/freshness into the connector contract.
#[must_use]
pub fn mqtt_session_status(state: MqttSessionProjection) -> ConnectorStatusProjection {
    match state {
        MqttSessionProjection::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        MqttSessionProjection::Disconnected => {
            ConnectorStatusProjection::new(ConnectorState::Stale, ConnectorHealth::Degraded)
        }
        MqttSessionProjection::Connecting => {
            ConnectorStatusProjection::new(ConnectorState::Starting, ConnectorHealth::Unknown)
        }
        MqttSessionProjection::ConnectedFresh => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        MqttSessionProjection::ConnectedStale => {
            ConnectorStatusProjection::new(ConnectorState::Stale, ConnectorHealth::Degraded)
        }
        MqttSessionProjection::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
    }
}

/// Project Modbus runtime status into the connector contract.
#[must_use]
pub fn modbus_status(state: ModbusProjection) -> ConnectorStatusProjection {
    match state {
        ModbusProjection::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        ModbusProjection::Ready => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        ModbusProjection::Timeout | ModbusProjection::ProtocolError => {
            ConnectorStatusProjection::new(ConnectorState::Degraded, ConnectorHealth::Degraded)
        }
        ModbusProjection::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
    }
}

/// Project EtherCAT bus status into the connector contract.
#[must_use]
pub fn ethercat_status(state: EthercatProjection) -> ConnectorStatusProjection {
    match state {
        EthercatProjection::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        EthercatProjection::Operational => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        EthercatProjection::Degraded => {
            ConnectorStatusProjection::new(ConnectorState::Degraded, ConnectorHealth::Degraded)
        }
        EthercatProjection::Reconnecting => {
            ConnectorStatusProjection::new(ConnectorState::Reconnecting, ConnectorHealth::Degraded)
        }
        EthercatProjection::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
    }
}
