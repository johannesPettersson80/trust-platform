//! Beckhoff ADS setup/onboarding HTTP routes.

#![allow(missing_docs)]

use super::*;

use crate::ads::diagnostics::CredentialChannelClassification;
use crate::control::control_request_required_role_port;

pub(super) struct AdsRouteContext<'a> {
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub web_tls_enabled: bool,
    pub control_state: &'a Arc<ControlState>,
}

pub(super) enum AdsRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_ads_route(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: AdsRouteContext<'_>,
) -> AdsRouteOutcome {
    let path = url.split('?').next().unwrap_or(url);

    if *method == Method::Get && path == "/api/ads/status" {
        return handle_ads_get_control(request, "ads.status", None, ctx);
    }

    if *method == Method::Get && path == "/api/ads/server/status" {
        return handle_ads_get_control(request, "ads.server.status", None, ctx);
    }

    if *method == Method::Get && path == "/api/ads/server/symbols" {
        return handle_ads_get_control(request, "ads.server.symbols", None, ctx);
    }

    if *method == Method::Get && path == "/api/ads/doctor/status" {
        let Some(job_id) = query_value(url, "job_id").or_else(|| query_value(url, "id")) else {
            respond_json(
                request,
                StatusCode(400),
                json!({ "ok": false, "error": "missing job_id" }),
            );
            return AdsRouteOutcome::Handled;
        };
        return handle_ads_get_control(
            request,
            "ads.doctor.status",
            Some(json!({ "job_id": job_id })),
            ctx,
        );
    }

    if *method == Method::Get && path == "/api/ads/server/doctor/status" {
        let Some(job_id) = query_value(url, "job_id").or_else(|| query_value(url, "id")) else {
            respond_json(
                request,
                StatusCode(400),
                json!({ "ok": false, "error": "missing job_id" }),
            );
            return AdsRouteOutcome::Handled;
        };
        return handle_ads_get_control(
            request,
            "ads.server.doctor.status",
            Some(json!({ "job_id": job_id })),
            ctx,
        );
    }

    let Some(kind) = post_route_kind(path) else {
        return AdsRouteOutcome::NotHandled(request);
    };
    if *method != Method::Post {
        return AdsRouteOutcome::NotHandled(request);
    }
    if let Err(response) = api_post_policy_check(&request, ctx.web_tls_enabled, true) {
        let _ = request.respond(response);
        return AdsRouteOutcome::Handled;
    }
    let mut params: serde_json::Value = match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
        Ok(value) => value,
        Err(error) => {
            let _ = request.respond(json_body_error_response(error));
            return AdsRouteOutcome::Handled;
        }
    };
    let required_role = control_request_required_role_port(kind, Some(&params));
    let (role, request_token) = match check_auth_with_role(
        &request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        required_role,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = request.respond(auth_error_response(error));
            return AdsRouteOutcome::Handled;
        }
    };
    if kind == "ads.route_add" {
        apply_route_add_channel(&mut params, classify_setup_channel(&request, &ctx, role));
    }
    respond_control(
        request,
        kind,
        Some(params),
        ctx.control_state,
        request_token.as_deref(),
    );
    AdsRouteOutcome::Handled
}

fn handle_ads_get_control(
    request: tiny_http::Request,
    kind: &str,
    params: Option<serde_json::Value>,
    ctx: AdsRouteContext<'_>,
) -> AdsRouteOutcome {
    let request_token = match check_auth(
        &request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    ) {
        Ok(token) => token,
        Err(error) => {
            let _ = request.respond(auth_error_response(error));
            return AdsRouteOutcome::Handled;
        }
    };
    respond_control(
        request,
        kind,
        params,
        ctx.control_state,
        request_token.as_deref(),
    );
    AdsRouteOutcome::Handled
}

fn post_route_kind(path: &str) -> Option<&'static str> {
    match path {
        "/api/ads/discover" => Some("ads.discover"),
        "/api/ads/identity" => Some("ads.identity"),
        "/api/ads/doctor" => Some("ads.doctor"),
        "/api/ads/doctor/start" => Some("ads.doctor.start"),
        "/api/ads/route-plan" => Some("ads.route_plan"),
        "/api/ads/route-add" => Some("ads.route_add"),
        "/api/ads/route-remove" => Some("ads.route_remove"),
        "/api/ads/import-symbols" => Some("ads.import_symbols"),
        "/api/ads/server/doctor" => Some("ads.server.doctor"),
        "/api/ads/server/doctor/start" => Some("ads.server.doctor.start"),
        "/api/ads/server/route-plan" => Some("ads.server.route_plan"),
        _ => None,
    }
}

fn classify_setup_channel(
    request: &tiny_http::Request,
    ctx: &AdsRouteContext<'_>,
    role: AccessRole,
) -> CredentialChannelClassification {
    classify_setup_channel_from_parts(
        ctx.auth_mode,
        ctx.web_tls_enabled,
        role,
        request
            .remote_addr()
            .is_some_and(|addr| addr.ip().is_loopback()),
    )
}

fn classify_setup_channel_from_parts(
    _auth_mode: WebAuthMode,
    web_tls_enabled: bool,
    role: AccessRole,
    remote_is_loopback: bool,
) -> CredentialChannelClassification {
    if remote_is_loopback {
        return CredentialChannelClassification::TrustedSameHost;
    }
    if web_tls_enabled && role.allows(AccessRole::Admin) {
        return CredentialChannelClassification::TrustedHttpsAdmin;
    }
    CredentialChannelClassification::UntrustedPlainHttpNetwork
}

fn apply_route_add_channel(
    params: &mut serde_json::Value,
    channel: CredentialChannelClassification,
) {
    if !params.is_object() {
        return;
    }
    params["channel"] = serde_json::to_value(channel)
        .unwrap_or_else(|_| serde_json::Value::String("untrusted_plain_http_network".to_string()));
}

fn respond_control(
    request: tiny_http::Request,
    kind: &str,
    params: Option<serde_json::Value>,
    control_state: &ControlState,
    request_token: Option<&str>,
) {
    let mut payload = json!({
        "id": 1u64,
        "type": kind,
    });
    if let Some(params) = params {
        payload["params"] = params;
    }
    let response = dispatch_control_request(payload, control_state, Some("web-ads"), request_token);
    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    let response = Response::from_string(body)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}

fn respond_json(request: tiny_http::Request, status: StatusCode, body: serde_json::Value) {
    let response = Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ads::diagnostics::{LocalIdentity, LocalNetworkClassification, TargetIdentity};
    use crate::ads::onboarding::errors::OnboardingWireErrorKind;
    use crate::ads::onboarding::route::{
        add_route_with_channel_policy, RouteAddRequest, RouteCredentials,
    };
    use crate::ads::onboarding::wire::{MockAdsOnboardingScenario, MockAdsOnboardingWire};

    #[test]
    fn setup_channel_classification_matches_security_matrix() {
        assert_eq!(
            classify_setup_channel_from_parts(WebAuthMode::Local, false, AccessRole::Admin, false),
            CredentialChannelClassification::UntrustedPlainHttpNetwork
        );
        assert_eq!(
            classify_setup_channel_from_parts(WebAuthMode::Token, false, AccessRole::Admin, true),
            CredentialChannelClassification::TrustedSameHost
        );
        assert_eq!(
            classify_setup_channel_from_parts(WebAuthMode::Token, true, AccessRole::Admin, false),
            CredentialChannelClassification::TrustedHttpsAdmin
        );
        assert_eq!(
            classify_setup_channel_from_parts(WebAuthMode::Token, false, AccessRole::Admin, false),
            CredentialChannelClassification::UntrustedPlainHttpNetwork
        );
        assert_eq!(
            classify_setup_channel_from_parts(WebAuthMode::Token, true, AccessRole::Viewer, false),
            CredentialChannelClassification::UntrustedPlainHttpNetwork
        );
    }

    #[test]
    fn route_add_channel_is_derived_server_side_and_overwrites_client_claim() {
        let mut params = json!({
            "route_name": "trust-runtime-line-controller-1",
            "channel": "trusted_https_admin"
        });

        apply_route_add_channel(
            &mut params,
            CredentialChannelClassification::UntrustedPlainHttpNetwork,
        );

        assert_eq!(
            params.get("channel").and_then(serde_json::Value::as_str),
            Some("untrusted_plain_http_network")
        );
    }

    #[test]
    fn route_add_with_server_derived_untrusted_channel_is_rejected() {
        let mut params = json!({
            "route_name": "trust-runtime-line-controller-1",
            "target": target_identity(),
            "local": local_identity(),
            "credentials": {
                "username": "Administrator",
                "password": "not-persisted"
            },
            "channel": "trusted_same_host"
        });
        apply_route_add_channel(
            &mut params,
            CredentialChannelClassification::UntrustedPlainHttpNetwork,
        );
        let channel: CredentialChannelClassification =
            serde_json::from_value(params["channel"].clone()).expect("valid channel");
        let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::Healthy);

        let error =
            add_route_with_channel_policy(&mut wire, &route_add_request(), channel).unwrap_err();

        assert_eq!(error.kind, OnboardingWireErrorKind::UnsupportedOperation);
        assert!(error.detail.contains("disabled"));
    }

    fn target_identity() -> TargetIdentity {
        TargetIdentity {
            name: Some("CX-1234".to_string()),
            ip: "192.168.10.5".to_string(),
            ams_net_id: "5.23.91.12.1.1".to_string(),
            ams_port: 851,
            tc_version: Some("3.1.4024".to_string()),
        }
    }

    fn local_identity() -> LocalIdentity {
        LocalIdentity {
            host_name: Some("line-controller-1".to_string()),
            chosen_ip: "192.168.10.20".to_string(),
            ams_net_id: "192.168.10.20.1.1".to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: LocalNetworkClassification::Lan,
        }
    }

    fn route_add_request() -> RouteAddRequest {
        RouteAddRequest {
            route_name: "trust-runtime-line-controller-1".to_string(),
            target: target_identity(),
            local: local_identity(),
            credentials: RouteCredentials {
                username: "Administrator".to_string(),
                password: "not-persisted".to_string(),
            },
        }
    }
}
