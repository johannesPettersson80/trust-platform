use super::*;

pub(super) fn handle_get_config(request: tiny_http::Request, ctx: &RuntimeCloudRouteContext<'_>) {
    let (_web_role, _request_token) = match check_auth_with_role(
        &request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = request.respond(auth_error_response(error));
            return;
        }
    };
    runtime_cloud_config_reconcile_once(
        ctx.config_state.as_ref(),
        ctx.control_state.as_ref(),
        ctx.config_path,
    );
    let snapshot =
        runtime_cloud_config_snapshot(ctx.config_state.as_ref(), local_runtime_id(ctx).as_str());
    let response = Response::from_string(
        serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string()),
    )
    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}

pub(super) fn handle_post_config_desired(
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
    let payload: RuntimeCloudDesiredWriteRequest =
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
    match runtime_cloud_config_write_desired(ctx.config_state.as_ref(), &payload, ctx.config_path) {
        Ok(snapshot) => {
            let response = Response::from_string(
                json!({
                    "ok": true,
                    "meta": snapshot.meta,
                    "status": snapshot.status,
                })
                .to_string(),
            )
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
        }
        Err(error) => {
            let status = if error.code == ReasonCode::RevisionConflict {
                StatusCode(409)
            } else {
                StatusCode(400)
            };
            let response = Response::from_string(
                json!({
                    "ok": false,
                    "denial_code": error.code,
                    "error": error.message,
                    "desired": error.snapshot.desired,
                    "reported": error.snapshot.reported,
                    "meta": error.snapshot.meta,
                    "status": error.snapshot.status,
                })
                .to_string(),
            )
            .with_status_code(status)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
        }
    }
}
