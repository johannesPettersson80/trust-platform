//! EtherCAT status adapter home.

use super::super::mapping::{ethercat_status, ConnectorStatusProjection, EthercatProjection};

/// Project EtherCAT bus status.
#[must_use]
pub fn project_ethercat_status(state: EthercatProjection) -> ConnectorStatusProjection {
    ethercat_status(state)
}
