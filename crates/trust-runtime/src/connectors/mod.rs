//! Shared connector status contracts and protocol adapters.

pub mod adapters;
pub mod contract;
pub mod mapping;
pub mod report;

pub use contract::{
    ConnectorHealth, ConnectorKind, ConnectorPointCounts, ConnectorPointMetadata,
    ConnectorPointStatus, ConnectorProtocol, ConnectorState, ConnectorStatusReport,
    DiscoveryConfidence, PointDirection, PointQuality, ReconnectPolicy,
    CONNECTOR_STATUS_SCHEMA_VERSION,
};
pub use mapping::{
    ads_connection_state_status, ads_connection_status_state, ethercat_status, io_driver_status,
    modbus_status, mqtt_session_status, opcua_client_status, opcua_server_snapshot_status,
    ConnectorStatusProjection, EthercatProjection, ModbusProjection, MqttSessionProjection,
    OpcUaServerSnapshotState,
};
pub use report::ConnectorStatusBuilder;
