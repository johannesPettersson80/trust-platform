use crate::ads::onboarding::{discover_targets, DiscoveryRequest};

use super::super::{ControlResponse, ControlState};

pub(in crate::control) fn handle_ads_discover(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let request: DiscoveryRequest = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };

    #[cfg(feature = "ads-wire")]
    {
        let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::default();
        match discover_targets(&mut wire, &request) {
            Ok(results) => serialize_results(id, results),
            Err(error) => ControlResponse::error(id, error.to_string()),
        }
    }

    #[cfg(not(feature = "ads-wire"))]
    {
        if request.target_ams_net_id.is_none() || request.include_broadcast {
            return ControlResponse::error(
                id,
                "ADS discovery needs an ads-wire build unless target_ams_net_id is supplied"
                    .to_string(),
            );
        }
        let mut wire = crate::ads::onboarding::MockAdsOnboardingWire::default();
        match discover_targets(&mut wire, &request) {
            Ok(results) => serialize_results(id, results),
            Err(error) => ControlResponse::error(id, error.to_string()),
        }
    }
}

fn serialize_results(
    id: u64,
    results: Vec<crate::ads::onboarding::DiscoveryResult>,
) -> ControlResponse {
    match serde_json::to_value(results) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => {
            ControlResponse::error(id, format!("ADS discovery serialization failed: {error}"))
        }
    }
}
