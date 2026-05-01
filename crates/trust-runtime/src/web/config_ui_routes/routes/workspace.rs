use super::super::*;

pub(in crate::web::config_ui_routes) fn handle_workspace_routes(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: &ConfigUiRouteContext<'_>,
) -> ConfigUiRouteOutcome {
    if *method == Method::Get && url == "/api/config-ui/project/state" {
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
        let response = match load_workspace_model(ctx.bundle_root).and_then(config_project_state) {
            Ok(body) => json_response(200, body),
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

    if *method == Method::Post && url == "/api/config-ui/runtime/create" {
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
        let payload: ConfigRuntimeCreateRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            create_workspace_runtime(
                &workspace,
                payload.runtime_id.as_str(),
                payload.host_group.as_deref(),
            )
        }) {
            Ok(body) => json_response(200, body),
            Err(error) => structured_error_response(
                400,
                "runtime_create_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/runtime/delete" {
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
        let payload: ConfigRuntimeDeleteRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match load_workspace_model(ctx.bundle_root)
            .and_then(|workspace| delete_workspace_runtime(&workspace, payload.runtime_id.as_str()))
        {
            Ok(body) => json_response(200, body),
            Err(error) => structured_error_response(
                400,
                "runtime_delete_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url.starts_with("/api/config-ui/runtime/config") {
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
        let runtime_id = query_value(url, "runtime_id");
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime =
                resolve_runtime_target(&workspace, runtime_id.as_deref(), ctx.control_state)?;
            let runtime_path = runtime.root.join("runtime.toml");
            let text = fs::read_to_string(&runtime_path).map_err(|error| {
                RuntimeError::InvalidConfig(format!("failed to read runtime.toml: {error}").into())
            })?;
            Ok(json!({
                "ok": true,
                "runtime_id": runtime.runtime_id,
                "path": runtime_path.display().to_string(),
                "text": text,
                "revision": text_revision(text.as_str()),
            }))
        }) {
            Ok(body) => json_response(200, body),
            Err(error) => structured_error_response(
                400,
                "runtime_read_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/runtime/config" {
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
        let payload: ConfigTextWriteRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_target(
                &workspace,
                payload.runtime_id.as_deref(),
                ctx.control_state,
            )?;
            write_config_file(
                runtime.root.join("runtime.toml").as_path(),
                payload.text.as_str(),
                payload.expected_revision.as_deref(),
                crate::config::validate_runtime_toml_text,
            )
            .map(|revision| {
                json!({
                    "ok": true,
                    "runtime_id": runtime.runtime_id,
                    "revision": revision,
                    "message": "runtime.toml saved",
                })
            })
        }) {
            Ok(body) => json_response(200, body),
            Err(RuntimeError::ControlError(message)) if message.starts_with("conflict:") => {
                let conflict = message.trim_start_matches("conflict:").trim().to_string();
                structured_error_response(
                    409,
                    "conflict",
                    "stale write conflict",
                    vec![FieldErrorItem {
                        path: "expected_revision".to_string(),
                        hint: "refresh and retry".to_string(),
                    }],
                    Some(conflict),
                )
            }
            Err(error) => structured_error_response(
                400,
                "runtime_write_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url.starts_with("/api/config-ui/io/config") {
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
        let runtime_id = query_value(url, "runtime_id");
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime =
                resolve_runtime_target(&workspace, runtime_id.as_deref(), ctx.control_state)?;
            let path = runtime.root.join("io.toml");
            let text = if path.is_file() {
                fs::read_to_string(&path).map_err(|error| {
                    RuntimeError::InvalidConfig(format!("failed to read io.toml: {error}").into())
                })?
            } else {
                String::new()
            };
            Ok(json!({
                "ok": true,
                "runtime_id": runtime.runtime_id,
                "path": path.display().to_string(),
                "text": text,
                "revision": text_revision(text.as_str()),
            }))
        }) {
            Ok(body) => json_response(200, body),
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

    if *method == Method::Post && url == "/api/config-ui/io/config" {
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
        let payload: ConfigTextWriteRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_target(
                &workspace,
                payload.runtime_id.as_deref(),
                ctx.control_state,
            )?;
            write_config_file(
                runtime.root.join("io.toml").as_path(),
                payload.text.as_str(),
                payload.expected_revision.as_deref(),
                crate::config::validate_io_toml_text,
            )
            .map(|revision| {
                json!({
                    "ok": true,
                    "runtime_id": runtime.runtime_id,
                    "revision": revision,
                    "message": "io.toml saved",
                })
            })
        }) {
            Ok(body) => json_response(200, body),
            Err(RuntimeError::ControlError(message)) if message.starts_with("conflict:") => {
                let conflict = message.trim_start_matches("conflict:").trim().to_string();
                structured_error_response(
                    409,
                    "conflict",
                    "stale write conflict",
                    vec![FieldErrorItem {
                        path: "expected_revision".to_string(),
                        hint: "refresh and retry".to_string(),
                    }],
                    Some(conflict),
                )
            }
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

    if *method == Method::Get && url.starts_with("/api/config-ui/st/files") {
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
        let runtime_id = query_value(url, "runtime_id");
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime =
                resolve_runtime_target(&workspace, runtime_id.as_deref(), ctx.control_state)?;
            let files = list_sources(runtime.root.as_path())
                .into_iter()
                .map(|path| {
                    let text =
                        read_source_file(runtime.root.as_path(), path.as_str()).unwrap_or_default();
                    json!({
                        "path": path,
                        "revision": text_revision(text.as_str()),
                        "bytes": text.len(),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "ok": true,
                "runtime_id": runtime.runtime_id,
                "files": files,
            }))
        }) {
            Ok(body) => json_response(200, body),
            Err(error) => structured_error_response(
                400,
                "st_list_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url.starts_with("/api/config-ui/st/file") {
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
        let runtime_id = query_value(url, "runtime_id");
        let file_path = query_value(url, "path");
        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime =
                resolve_runtime_target(&workspace, runtime_id.as_deref(), ctx.control_state)?;
            let Some(path) = file_path.as_deref() else {
                return Err(RuntimeError::InvalidConfig("path is required".into()));
            };
            let text = read_source_file(runtime.root.as_path(), path)?;
            Ok(json!({
                "ok": true,
                "runtime_id": runtime.runtime_id,
                "path": path,
                "text": text,
                "revision": text_revision(text.as_str()),
            }))
        }) {
            Ok(body) => json_response(200, body),
            Err(error) => structured_error_response(
                400,
                "st_read_failed",
                error.to_string().as_str(),
                vec![FieldErrorItem {
                    path: "path".to_string(),
                    hint: "Provide a valid .st file path under src/".to_string(),
                }],
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/st/file" {
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
        let payload: ConfigStWriteRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };

        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_target(
                &workspace,
                payload.runtime_id.as_deref(),
                ctx.control_state,
            )?;
            let path = normalize_st_relative_path(payload.path.as_str())?;
            let absolute = runtime.root.join("src").join(&path);
            let current_text = fs::read_to_string(&absolute).unwrap_or_default();
            let current_revision = text_revision(current_text.as_str());
            if let Some(expected) = payload.expected_revision.as_deref() {
                if expected.trim() != current_revision {
                    return Err(RuntimeError::ControlError(
                        format!("conflict: {current_revision}").into(),
                    ));
                }
            }
            atomic_write_text(&absolute, payload.text.as_str())?;
            let revision = text_revision(payload.text.as_str());
            Ok(json!({
                "ok": true,
                "runtime_id": runtime.runtime_id,
                "path": path.display().to_string(),
                "revision": revision,
                "message": "source saved",
            }))
        }) {
            Ok(body) => json_response(200, body),
            Err(RuntimeError::ControlError(message)) if message.starts_with("conflict:") => {
                let conflict = message.trim_start_matches("conflict:").trim().to_string();
                structured_error_response(
                    409,
                    "conflict",
                    "stale write conflict",
                    vec![FieldErrorItem {
                        path: "expected_revision".to_string(),
                        hint: "refresh and retry".to_string(),
                    }],
                    Some(conflict),
                )
            }
            Err(error) => structured_error_response(
                400,
                "st_write_failed",
                error.to_string().as_str(),
                vec![FieldErrorItem {
                    path: "path".to_string(),
                    hint: "Path must stay under src/ and end with .st".to_string(),
                }],
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Post && url == "/api/config-ui/st/validate" {
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
        let payload: ConfigStValidateRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return ConfigUiRouteOutcome::Handled;
                }
            };

        let response = match load_workspace_model(ctx.bundle_root).and_then(|workspace| {
            let runtime = resolve_runtime_target(
                &workspace,
                payload.runtime_id.as_deref(),
                ctx.control_state,
            )?;
            validate_st_sources(
                runtime.root.as_path(),
                payload.path.as_deref(),
                payload.text.as_deref(),
            )
        }) {
            Ok(diagnostics) => {
                json_response(200, json!({ "ok": true, "diagnostics": diagnostics }))
            }
            Err(error) => structured_error_response(
                400,
                "st_validation_failed",
                error.to_string().as_str(),
                Vec::new(),
                None,
            ),
        };
        let _ = request.respond(response);
        return ConfigUiRouteOutcome::Handled;
    }

    if *method == Method::Get && url == "/api/config-ui/topology/projected" {
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
                "projection_failed",
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
