use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::ads::diagnostics::{AdsConnectionStatusState, AdsStatusOverall, AdsStatusReport};
use crate::scheduler::ResourceCommand;

use super::super::resource_queue::ResourceReadError;
use super::super::{ControlResponse, ControlState};

const ADS_STATUS_TIMEOUT: Duration = Duration::from_millis(250);

pub(in crate::control) fn handle_ads_status(id: u64, state: &ControlState) -> ControlResponse {
    match ads_status_report_from_state(state) {
        Ok(report) => match status_value(&report) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => {
                ControlResponse::error(id, format!("ADS status serialization failed: {error}"))
            }
        },
        Err(error) => error.into_response(id),
    }
}

pub(in crate::control) fn ads_status_report_from_state(
    state: &ControlState,
) -> Result<AdsStatusReport, ResourceReadError> {
    let (tx, rx) = mpsc::channel();
    if let Err(error) = state
        .resource
        .send_command(ResourceCommand::AdsStatus { respond_to: tx })
    {
        return Err(ResourceReadError::from_send(
            "ADS status request failed",
            error,
        ));
    }
    match rx.recv_timeout(ADS_STATUS_TIMEOUT) {
        Ok(report) => Ok(report),
        Err(error) => Err(ResourceReadError::failed(format!(
            "ADS status request timed out: {error}"
        ))),
    }
}

/// The `ads.status` answer: the report, and the summary that the tools show of it as `view`
/// (specification 11, "ADS status summary").
fn status_value(report: &AdsStatusReport) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(report)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("view".to_owned(), status_view(report));
    }
    Ok(value)
}

/// The summary of an ADS status report (specification 11, "ADS status summary"): how many ADS
/// devices it has and how many of them are degraded, in one sentence; the report's own summary
/// while it has none, else `ADS is not configured.`; and its overall state. A device is degraded
/// when it has a degraded point or is neither connected nor disabled.
pub(super) fn status_view(report: &AdsStatusReport) -> Value {
    let devices = report.connections.len();
    let degraded = report
        .connections
        .iter()
        .filter(|connection| {
            connection.degraded_points > 0
                || !matches!(
                    connection.state,
                    AdsConnectionStatusState::Connected | AdsConnectionStatusState::Disabled
                )
        })
        .count();
    let text = if devices == 0 {
        if report.summary.is_empty() {
            "ADS is not configured.".to_owned()
        } else {
            report.summary.clone()
        }
    } else {
        format!(
            "ADS: {devices} device{} · {degraded} degraded",
            if devices == 1 { "" } else { "s" }
        )
    };
    json!({
        "text": text,
        "overall": overall_word(report.overall),
        "device_count": devices,
        "degraded_count": if devices == 0 { 0 } else { degraded },
    })
}

/// The word of an overall ADS state, as the report spells it.
fn overall_word(overall: AdsStatusOverall) -> &'static str {
    match overall {
        AdsStatusOverall::Healthy => "healthy",
        AdsStatusOverall::Degraded => "degraded",
        AdsStatusOverall::NotReady => "not_ready",
        AdsStatusOverall::Faulted => "faulted",
        AdsStatusOverall::Disabled => "disabled",
        AdsStatusOverall::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::status_view;
    use crate::ads::diagnostics::AdsStatusReport;

    /// A report of `connections` (each its name, state, point count and degraded points) with the
    /// overall state `overall` and the summary `summary`, as `ads.status` reads it.
    fn report(
        overall: &str,
        summary: &str,
        connections: &[(&str, &str, u64, u64)],
    ) -> AdsStatusReport {
        serde_json::from_value(json!({
            "schema_version": 1,
            "role": "client",
            "overall": overall,
            "summary": summary,
            "connections": connections
                .iter()
                .map(|(name, state, points, degraded)| json!({
                    "name": name, "state": state, "point_count": points,
                    "degraded_points": degraded, "summary": "",
                }))
                .collect::<Vec<_>>(),
        }))
        .expect("an ADS status report")
    }

    #[test]
    fn status_view_counts_devices_and_degraded_devices() {
        // The summary of the ADS status (specification 11, "ADS status summary"). Two devices:
        // `line1` is connected with no degraded point, `line2` reconnects with one degraded point,
        // so one device of two is degraded. A wrong implementation counts degraded points instead
        // of devices (2), counts the reconnecting device as healthy (0), or drops the plural.
        let view = status_view(&report(
            "degraded",
            "2 ADS devices, 1 degraded.",
            &[
                ("line1", "connected", 4, 0),
                ("line2", "reconnecting", 3, 1),
            ],
        ));
        assert_eq!(
            view,
            json!({
                "text": "ADS: 2 devices · 1 degraded",
                "overall": "degraded",
                "device_count": 2,
                "degraded_count": 1,
            })
        );
    }

    #[test]
    fn status_view_without_devices_keeps_the_reports_summary() {
        // A report without devices says what the report says, else that ADS is not configured.
        // A wrong implementation says `ADS: 0 devices` or drops the report's own summary.
        let configured_off = status_view(&report("disabled", "ADS is not configured.", &[]));
        assert_eq!(configured_off["text"], "ADS is not configured.");
        assert_eq!(configured_off["overall"], "disabled");
        assert_eq!(configured_off["device_count"], 0);
        let silent = status_view(&report("unknown", "", &[]));
        assert_eq!(silent["text"], "ADS is not configured.");
    }

    #[test]
    fn status_view_counts_a_disabled_device_as_healthy_and_a_stale_one_as_degraded() {
        // A disabled device is not degraded; a stale one is, even without a degraded point. One
        // device of three is degraded. A wrong implementation counts the disabled device too (2),
        // or reads only the degraded points (0).
        let view = status_view(&report(
            "degraded",
            "",
            &[
                ("a", "disabled", 1, 0),
                ("b", "stale", 2, 0),
                ("c", "connected", 3, 0),
            ],
        ));
        assert_eq!(view["text"], "ADS: 3 devices · 1 degraded");
        assert_eq!(view["degraded_count"], 1);
    }
}
