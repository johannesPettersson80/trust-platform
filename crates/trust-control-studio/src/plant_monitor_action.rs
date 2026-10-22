//! The method `controlStudio/plantMonitor` (specification 35, §0.1a; specification 34, §20.4): the plant summary of a plant handoff.
//! Every decision is trust-control's (`trust_control::plant_monitor`). This module makes the one read that decision asks for, the
//! runtime's `plant.status`, bounded by a short timeout so that a silent endpoint does not hold the engine for long.

use std::time::Duration;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::plant_monitor::{self, PlantMonitorRequest, PlantMonitorResult};
use trust_control::runtime::RuntimeConnection;

use crate::runtime_client::RuntimeClient;

/// How long the plant status read may wait. The panel refreshes the summary on each runtime change, so a silent endpoint is given
/// two seconds, as the extension gave it before the engine made the read.
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// Answers `controlStudio/plantMonitor`: the plant summary the request names, read from the runtime when the request's runtime state
/// allows the read.
#[must_use]
pub fn answer(request: &PlantMonitorRequest) -> PlantMonitorResult {
    plant_monitor::answer(request, |connection| {
        read_plant_status(connection, &request.project_id)
    })
}

/// The runtime's `plant.status` answer, or the text of why the read failed: the refusal code and the failure's own words.
fn read_plant_status(connection: &RuntimeConnection, project_id: &str) -> Result<Value, String> {
    let mut client = RuntimeClient::connect_within(connection, READ_TIMEOUT)
        .map_err(|failure| failure_text(&failure, None))?;
    client
        .plant_status(project_id)
        .map_err(|call| failure_text(&call.failure, call.refusal_code.as_deref()))
}

/// The text of a failed read, from which trust-control takes the reason: the refusal code, then the failure's sentence and detail.
fn failure_text(failure: &Failure, code: Option<&str>) -> String {
    let code = code.unwrap_or_default();
    let message = &failure.message;
    let detail = failure.detail.as_deref().unwrap_or_default();
    format!("{code} {message} {detail}")
}

#[cfg(test)]
#[path = "plant_monitor_action_tests.rs"]
mod tests;
