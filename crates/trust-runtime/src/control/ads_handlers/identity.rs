use crate::ads::onboarding::{
    derive_host_ads_identity, runtime_address_candidates_from_interfaces, IdentityRequest,
};

use super::super::{ControlResponse, ControlState};

pub(in crate::control) fn handle_ads_identity(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let request: IdentityRequest = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let candidates = runtime_address_candidates_from_interfaces().unwrap_or_default();
    let identity = match derive_host_ads_identity(&request, candidates) {
        Ok(identity) => identity,
        Err(error) => return ControlResponse::error(id, error.to_string()),
    };
    match serde_json::to_value(identity) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => {
            ControlResponse::error(id, format!("ADS identity serialization failed: {error}"))
        }
    }
}
