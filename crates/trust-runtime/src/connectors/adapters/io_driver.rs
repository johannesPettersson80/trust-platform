//! Process-image `IoDriver` status adapter.

use crate::io::{IoDriverErrorPolicy, IoDriverHealth, IoDriverStatus};

use super::super::contract::{
    ConnectorKind, ConnectorProtocol, ConnectorStatusReport, DiscoveryConfidence,
};
use super::super::mapping::{io_driver_status, ConnectorStatusProjection};
use super::super::report::ConnectorStatusBuilder;

/// Project process-image driver health into the connector status contract.
#[must_use]
pub fn project_io_driver_health(
    health: &IoDriverHealth,
    error_policy: IoDriverErrorPolicy,
) -> ConnectorStatusProjection {
    io_driver_status(health, error_policy)
}

/// Project a process-image driver status into a connector report.
#[must_use]
pub fn project_io_driver_status(
    status: &IoDriverStatus,
    error_policy: IoDriverErrorPolicy,
) -> ConnectorStatusReport {
    let projection = project_io_driver_health(&status.health, error_policy);
    let mut builder = ConnectorStatusBuilder::new(
        format!("io:{}", status.name.as_str()),
        protocol_from_io_driver_name(status.name.as_str()),
        ConnectorKind::ProcessImage,
        projection.state,
        projection.health,
    )
    .display_name(status.name.as_str())
    .confidence(DiscoveryConfidence::Confirmed);
    if let Some(detail) = projection.detail {
        builder = builder.last_error(detail);
    }
    builder.build()
}

/// Map canonical runtime I/O driver names to connector protocol families.
#[must_use]
pub fn protocol_from_io_driver_name(name: &str) -> ConnectorProtocol {
    match name.trim().to_ascii_lowercase().as_str() {
        "modbus-tcp" | "modbus_tcp" => ConnectorProtocol::ModbusTcp,
        "mqtt" | "mqtt-tcp" => ConnectorProtocol::Mqtt,
        "ethercat" | "ether-cat" | "ecat" => ConnectorProtocol::Ethercat,
        "gpio" => ConnectorProtocol::Gpio,
        "simulated" | "sim" | "noop" => ConnectorProtocol::Simulated,
        "loopback" => ConnectorProtocol::Loopback,
        _ => ConnectorProtocol::Unknown,
    }
}
