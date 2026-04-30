use super::*;

pub(super) fn handle_post_link_transport(
    mut request: tiny_http::Request,
    ctx: &RuntimeCloudRouteContext<'_>,
) {
    let (_web_role, _request_token) = match check_auth_with_role(
        &request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Engineer,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = request.respond(auth_error_response(error));
            return;
        }
    };
    if let Err(response) = api_post_policy_check(&request, ctx.web_tls_enabled, true) {
        let _ = request.respond(response);
        return;
    }
    let payload: RuntimeCloudLinkTransportSetRequest =
        match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return;
            }
        };
    let compatibility =
        evaluate_compatibility(payload.api_version.as_str(), RUNTIME_CLOUD_API_VERSION);
    match compatibility {
        Ok(ContractCompatibility::Exact | ContractCompatibility::AdditiveWithinMajor) => {}
        Ok(ContractCompatibility::BreakingMajor) => {
            let response = Response::from_string(
                json!({
                    "ok": false,
                    "denial_code": "contract_violation",
                    "error": format!(
                        "unsupported api_version '{}' for runtime cloud {}",
                        payload.api_version, RUNTIME_CLOUD_API_VERSION
                    ),
                })
                .to_string(),
            )
            .with_status_code(StatusCode(400))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return;
        }
        Err(error) => {
            let response = Response::from_string(
                json!({
                    "ok": false,
                    "denial_code": "contract_violation",
                    "error": error.to_string(),
                })
                .to_string(),
            )
            .with_status_code(StatusCode(400))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return;
        }
    }

    let source = payload.source.trim();
    let target = payload.target.trim();
    if source.is_empty() || target.is_empty() {
        let response = Response::from_string(
            serde_json::to_string(&RuntimeCloudLinkTransportSetResponse {
                ok: false,
                preference: None,
                denial_code: Some(ReasonCode::ContractViolation),
                error: Some("source and target must not be empty".to_string()),
            })
            .unwrap_or_else(|_| "{}".to_string()),
        )
        .with_status_code(StatusCode(400))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    let local_runtime = local_runtime_id(ctx);
    if source != local_runtime {
        let response = Response::from_string(
            serde_json::to_string(&RuntimeCloudLinkTransportSetResponse {
                ok: false,
                preference: None,
                denial_code: Some(ReasonCode::PermissionDenied),
                error: Some(format!(
                    "source runtime must be '{}' when writing from this dashboard session",
                    local_runtime
                )),
            })
            .unwrap_or_else(|_| "{}".to_string()),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    if payload.transport == RuntimeCloudLinkTransport::Realtime
        && !runtime_cloud_link_is_same_host(ctx.discovery.as_ref(), source, target)
    {
        let response = Response::from_string(
            serde_json::to_string(&RuntimeCloudLinkTransportSetResponse {
                ok: false,
                preference: None,
                denial_code: Some(ReasonCode::ContractViolation),
                error: Some(
                    "realtime transport requires source and target runtimes to resolve on the same host"
                        .to_string(),
                ),
            })
            .unwrap_or_else(|_| "{}".to_string()),
        )
        .with_status_code(StatusCode(400))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    match runtime_cloud_link_set_transport(
        ctx.link_transport_state.as_ref(),
        source,
        target,
        payload.transport,
        payload.actor.as_str(),
        ctx.link_transport_path,
    ) {
        Ok(preference) => {
            let response = Response::from_string(
                serde_json::to_string(&RuntimeCloudLinkTransportSetResponse {
                    ok: true,
                    preference: Some(preference),
                    denial_code: None,
                    error: None,
                })
                .unwrap_or_else(|_| "{}".to_string()),
            )
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
        }
        Err(code) => {
            let status = if code == ReasonCode::TransportFailure {
                StatusCode(503)
            } else {
                StatusCode(400)
            };
            let response = Response::from_string(
                serde_json::to_string(&RuntimeCloudLinkTransportSetResponse {
                    ok: false,
                    preference: None,
                    denial_code: Some(code),
                    error: Some(code.remediation_hint().to_string()),
                })
                .unwrap_or_else(|_| "{}".to_string()),
            )
            .with_status_code(status)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
        }
    }
}
