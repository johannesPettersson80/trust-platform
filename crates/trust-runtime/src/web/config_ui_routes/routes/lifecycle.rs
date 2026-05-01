use super::super::*;

pub(in crate::web::config_ui_routes) fn handle_lifecycle_routes(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: &ConfigUiRouteContext<'_>,
) -> ConfigUiRouteOutcome {
    if *method == Method::Get && url.starts_with("/api/config-ui/runtime/lifecycle") {
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
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            config_ui_runtime_lifecycle_snapshot(&workspace, request_token.as_deref())
        }) {
            Ok(items) => json_response(200, json!({ "ok": true, "items": items })),
            Err(error) => structured_error_response(
                400,
                "lifecycle_read_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/runtime/lifecycle" {
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
        let payload: ConfigRuntimeLifecycleRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            config_ui_runtime_lifecycle_apply(&workspace, &payload, request_token.as_deref())
        }) {
            Ok(result) => json_response(
                200,
                serde_json::to_value(result).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "lifecycle_write_failed",
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
