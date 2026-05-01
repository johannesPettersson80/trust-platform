use super::super::*;

pub(in crate::web::config_ui_routes) fn handle_live_routes(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: &ConfigUiRouteContext<'_>,
) -> ConfigUiRouteOutcome {
    if *method == Method::Get && url == "/api/config-ui/live/targets" {
        let _request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Viewer,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let response = json_response(
            200,
            serde_json::to_value(config_ui_live_targets_snapshot()).unwrap_or_else(|_| json!({})),
        );
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/live/targets" {
        let _request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Engineer,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let payload: ConfigLiveTargetUpsertRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match config_ui_live_target_upsert(&payload) {
            Ok(snapshot) => json_response(
                200,
                serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "live_target_upsert_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/live/targets/remove" {
        let _request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Engineer,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let payload: ConfigLiveTargetRemoveRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match config_ui_live_target_remove(&payload) {
            Ok(snapshot) => json_response(
                200,
                serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "live_target_remove_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/live/connect" {
        let request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Engineer,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let payload: ConfigLiveConnectRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let token = payload.token.or(request_token);
        let response = match config_ui_live_connect(payload.target.as_deref(), token.as_deref()) {
            Ok(snapshot) => json_response(
                200,
                serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "live_connect_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url.starts_with("/api/config-ui/live/state") {
        let _request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Viewer,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let target = query_value(url, "target");
        let response = match config_ui_live_state(target.as_deref()) {
            Ok(snapshot) => json_response(
                200,
                serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "live_state_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    ConfigUiRouteOutcome::NotHandled(request)
}
