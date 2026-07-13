//! Read-only ADS live-values control request.

use super::super::{ControlResponse, ControlState};

pub(in crate::control) fn handle_ads_live_values(id: u64, state: &ControlState) -> ControlResponse {
    match state.resource.ads_live_values_snapshot() {
        Ok(snapshot) => match serde_json::to_value(snapshot) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => {
                ControlResponse::error(id, format!("ADS live-values serialization failed: {error}"))
            }
        },
        Err(error) => ControlResponse::error(id, format!("ADS live-values unavailable: {error}")),
    }
}
