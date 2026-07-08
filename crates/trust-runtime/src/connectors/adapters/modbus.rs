//! Modbus TCP status adapter home.

use super::super::mapping::{modbus_status, ConnectorStatusProjection, ModbusProjection};

/// Project Modbus driver status.
#[must_use]
pub fn project_modbus_status(state: ModbusProjection) -> ConnectorStatusProjection {
    modbus_status(state)
}
