use serde::Deserialize;

use crate::ads::diagnostics::{CredentialChannelClassification, LocalIdentity, TargetIdentity};
use crate::ads::onboarding::{
    build_route_plan, build_route_remove_artifact, RouteAddRequest, RouteCredentials,
    RoutePlanRequest, RouteRemoveRequest,
};

use super::super::{ControlResponse, ControlState};

pub(in crate::control) fn handle_ads_route_plan(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let params: RoutePlanRequest = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    match serde_json::to_value(build_route_plan(params)) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => {
            ControlResponse::error(id, format!("ADS route plan serialization failed: {error}"))
        }
    }
}

pub(in crate::control) fn handle_ads_route_add(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let params: RouteAddControlParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    if !params.channel.permits_credentials() {
        return ControlResponse::error_with_code(
            id,
            "automatic ADS route-add is disabled for this credential channel".to_string(),
            "untrusted_credential_channel",
        );
    }
    let request = RouteAddRequest {
        route_name: params.route_name,
        target: params.target,
        local: params.local,
        credentials: RouteCredentials {
            username: params.credentials.username,
            password: params.credentials.password,
        },
    };

    #[cfg(feature = "ads-wire")]
    {
        let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::default();
        match crate::ads::onboarding::add_route_with_channel_policy(
            &mut wire,
            &request,
            params.channel,
        ) {
            Ok(()) => ControlResponse::ok(id, serde_json::json!({ "status": "route_added" })),
            Err(error) => ControlResponse::error(id, error.to_string()),
        }
    }

    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = request;
        ControlResponse::error(
            id,
            "automatic ADS route-add needs an ads-wire build".to_string(),
        )
    }
}

pub(in crate::control) fn handle_ads_route_remove(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let params: RouteRemoveRequest = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let artifact = build_route_remove_artifact(params.route_name.as_str());
    ControlResponse::ok(
        id,
        serde_json::json!({
            "status": "artifact",
            "route_name": params.route_name,
            "target": params.target,
            "artifact": artifact,
        }),
    )
}

#[derive(Deserialize)]
struct RouteAddControlParams {
    route_name: String,
    target: TargetIdentity,
    local: LocalIdentity,
    channel: CredentialChannelClassification,
    credentials: RouteAddCredentialsParams,
}

#[derive(Deserialize)]
struct RouteAddCredentialsParams {
    username: String,
    password: String,
}
