use super::super::*;

pub(in crate::web::config_ui_routes) fn handle_runtime_cloud_routes(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: &ConfigUiRouteContext<'_>,
) -> ConfigUiRouteOutcome {
    if *method == Method::Get && url == "/api/runtime-cloud/state" {
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
        let response = match load_workspace_model(ctx.bundle_root)
            .map(|workspace| config_mode_runtime_cloud_state(&workspace))
        {
            Ok(state) => json_response(
                200,
                serde_json::to_value(state).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "invalid_project",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url == "/api/runtime-cloud/config" {
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
        let response = match load_workspace_model(ctx.bundle_root)
            .map(|workspace| config_mode_runtime_cloud_config_snapshot(&workspace))
        {
            Ok(snapshot) => json_response(
                200,
                serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => structured_error_response(
                400,
                "invalid_project",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url == "/api/runtime-cloud/rollouts" {
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
            json!({
                "api_version": RUNTIME_CLOUD_API_VERSION,
                "items": [],
            }),
        );
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url.starts_with("/api/runtime-cloud/io/config") {
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
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let target = query_value(url, "target");
            let runtime = resolve_runtime_target(&workspace, target.as_deref(), ctx.control_state)?;
            load_project_io_config_response(runtime.root.as_path())
        }) {
            Ok(io) => json_response(200, serde_json::to_value(io).unwrap_or_else(|_| json!({}))),
            Err(error) => structured_error_response(
                400,
                "io_read_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/runtime-cloud/io/config" {
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
        if let Err(response) = api_post_policy_check(&request, false, true) {
            let _ = request.respond(response);
            return ConfigUiRouteOutcome::Handled;
        }
        let payload: RuntimeCloudIoConfigProxyRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let target_runtime = payload.target_runtime.trim();
        if target_runtime.is_empty() {
            let response = structured_error_response(
                400,
                "contract_violation",
                "target_runtime is required",
                vec![FieldErrorItem {
                    path: "target_runtime".to_string(),
                    hint: "Provide target runtime id".to_string(),
                }],
                None,
            );
            let _ = request.respond(response);
            return ConfigUiRouteOutcome::Handled;
        }
        let io_request = payload.to_io_config_request();
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_by_id(&workspace, target_runtime)?;
            save_io_config(&Some(runtime.root.clone()), &io_request)
        }) {
            Ok(message) => json_response(200, json!({ "ok": true, "message": message })),
            Err(error) => structured_error_response(
                400,
                "io_write_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url == "/api/io/config" {
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
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_target(&workspace, None, ctx.control_state)?;
            load_project_io_config_response(runtime.root.as_path())
        }) {
            Ok(io) => json_response(200, serde_json::to_value(io).unwrap_or_else(|_| json!({}))),
            Err(error) => structured_error_response(
                400,
                "io_read_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/io/config" {
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
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let _ = request.respond(structured_error_response(
                400,
                "invalid_body",
                "invalid body",
                Vec::new(),
                None,
            ));
            return ConfigUiRouteOutcome::Handled;
        }
        let payload: IoConfigRequest = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(structured_error_response(
                    400,
                    "invalid_json",
                    format!("invalid json: {error}").as_str(),
                    Vec::new(),
                    None,
                ));
                return ConfigUiRouteOutcome::Handled;
            }
        };
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_target(&workspace, None, ctx.control_state)?;
            save_io_config(&Some(runtime.root.clone()), &payload)
        }) {
            Ok(message) => Response::from_string(message)
                .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap()),
            Err(error) => Response::from_string(format!("error: {error}"))
                .with_status_code(StatusCode(400))
                .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap()),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    ConfigUiRouteOutcome::NotHandled(request)
}
