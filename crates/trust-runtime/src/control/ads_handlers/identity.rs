use crate::ads::onboarding::{
    derive_runtime_identity_from_source, resolve_os_source_ip,
    runtime_address_candidates_from_interfaces, IdentityRequest,
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
    let chosen_ip = match resolve_os_source_ip(request.target_ip.as_str()) {
        Ok(ip) => ip,
        Err(error) => return ControlResponse::error(id, error.to_string()),
    };
    let candidates = runtime_address_candidates_from_interfaces().unwrap_or_default();
    let nic = candidates
        .iter()
        .find(|candidate| candidate.ip == chosen_ip)
        .and_then(|candidate| candidate.nic.clone());
    let identity =
        match derive_runtime_identity_from_source(&request, chosen_ip, None, nic, candidates) {
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
