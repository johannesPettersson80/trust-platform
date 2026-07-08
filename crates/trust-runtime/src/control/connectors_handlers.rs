use crate::connectors::adapters::ads::{
    project_ads_status_report, project_ads_status_report_with_default_endpoint,
};
use crate::connectors::adapters::io_driver::project_io_driver_status;
use crate::connectors::adapters::opcua::project_opcua_client_status_report;
use crate::connectors::CONNECTOR_STATUS_SCHEMA_VERSION;
use crate::io::IoDriverErrorPolicy;
use crate::scheduler::ResourceCommand;
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;

use super::ads_handlers::{
    ads_server_connector_endpoint_from_state, ads_server_status_report_from_state,
    ads_status_report_from_state,
};
use super::{ControlResponse, ControlState};

const CONNECTOR_STATUS_TIMEOUT: Duration = Duration::from_millis(250);

pub(super) fn handle_connectors_status(id: u64, state: &ControlState) -> ControlResponse {
    let mut warnings = Vec::new();
    let mut connectors = state
        .io_health
        .lock()
        .ok()
        .map(|guard| {
            guard
                .iter()
                .map(|status| project_io_driver_status(status, IoDriverErrorPolicy::Fault))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match ads_status_report_from_state(state) {
        Ok(report) => connectors.extend(project_ads_status_report(&report)),
        Err(error) => warnings.push(error),
    }
    match ads_server_status_report_from_state(state) {
        Ok(report) => {
            let endpoint = ads_server_connector_endpoint_from_state(state);
            connectors.extend(project_ads_status_report_with_default_endpoint(
                &report,
                Some(endpoint.as_str()),
            ));
        }
        Err(error) => warnings.push(error),
    }
    match opcua_client_status_report_from_state(state) {
        Some(report) => connectors.extend(project_opcua_client_status_report(&report)),
        None => warnings.push("OPC UA client status is unavailable.".to_string()),
    }
    let mut result = json!({
        "schema_version": CONNECTOR_STATUS_SCHEMA_VERSION,
        "connectors": connectors,
    });
    if !warnings.is_empty() {
        result["warnings"] = json!(warnings);
    }
    ControlResponse::ok(id, result)
}

fn opcua_client_status_report_from_state(
    state: &ControlState,
) -> Option<crate::opcua::OpcUaClientStatusReport> {
    let (tx, rx) = mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::OpcUaClientStatus { respond_to: tx })
        .ok()?;
    rx.recv_timeout(CONNECTOR_STATUS_TIMEOUT).ok()
}
