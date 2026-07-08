use std::sync::mpsc;
use std::time::Duration;

use crate::ads::diagnostics::AdsStatusReport;
use crate::scheduler::ResourceCommand;

use super::super::{ControlResponse, ControlState};

const ADS_STATUS_TIMEOUT: Duration = Duration::from_millis(250);

pub(in crate::control) fn handle_ads_status(id: u64, state: &ControlState) -> ControlResponse {
    match ads_status_report_from_state(state) {
        Ok(report) => match serde_json::to_value(report) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => {
                ControlResponse::error(id, format!("ADS status serialization failed: {error}"))
            }
        },
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(in crate::control) fn ads_status_report_from_state(
    state: &ControlState,
) -> Result<AdsStatusReport, String> {
    let (tx, rx) = mpsc::channel();
    if let Err(error) = state
        .resource
        .send_command(ResourceCommand::AdsStatus { respond_to: tx })
    {
        return Err(format!("ADS status request failed: {error}"));
    }
    match rx.recv_timeout(ADS_STATUS_TIMEOUT) {
        Ok(report) => Ok(report),
        Err(error) => Err(format!("ADS status request timed out: {error}")),
    }
}
