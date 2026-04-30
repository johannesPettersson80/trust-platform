//! Config-UI route handlers for TOML/ST-first engineering workflows.

#![allow(missing_docs)]

use super::*;
use crate::harness::{CompileSession, SourceFile as HarnessSourceFile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

pub(super) struct ConfigUiRouteContext<'a> {
    pub mode: WebServerMode,
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub control_state: &'a Arc<ControlState>,
    pub bundle_root: &'a Option<PathBuf>,
}

pub(super) enum ConfigUiRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

#[derive(Debug, Clone)]
struct WorkspaceRuntime {
    runtime_id: String,
    root: PathBuf,
    runtime: RuntimeConfig,
}

#[derive(Debug, Clone)]
struct WorkspaceModel {
    root: PathBuf,
    runtimes: Vec<WorkspaceRuntime>,
}

#[derive(Debug, Clone, Serialize)]
struct FieldErrorItem {
    path: String,
    hint: String,
}

#[derive(Debug, Deserialize)]
struct ConfigTextWriteRequest {
    runtime_id: Option<String>,
    text: String,
    expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigStWriteRequest {
    runtime_id: Option<String>,
    path: String,
    text: String,
    expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigStValidateRequest {
    runtime_id: Option<String>,
    path: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigLiveConnectRequest {
    target: Option<String>,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigLiveTargetUpsertRequest {
    target: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigLiveTargetRemoveRequest {
    target: String,
}

#[derive(Debug, Deserialize)]
struct ConfigRuntimeLifecycleRequest {
    runtime_id: String,
    action: String,
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigRuntimeCreateRequest {
    runtime_id: String,
    host_group: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigRuntimeDeleteRequest {
    runtime_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigUiLiveTargetProfile {
    target: String,
    label: String,
}

#[derive(Debug, Default)]
struct ConfigUiLiveManagerState {
    profiles: BTreeMap<String, ConfigUiLiveTargetProfile>,
    active_target: Option<String>,
    active_token: Option<String>,
    connected: bool,
    last_error: Option<String>,
    last_runtime_cloud: Option<serde_json::Value>,
    updated_at_ns: u64,
}

struct ConfigUiManagedRuntimeProcess {
    listen: String,
    child: Child,
    started_at_ns: u64,
}

#[derive(Default)]
struct ConfigUiLifecycleManagerState {
    managed: BTreeMap<String, ConfigUiManagedRuntimeProcess>,
}

static CONFIG_UI_LIVE_MANAGER: OnceLock<Mutex<ConfigUiLiveManagerState>> = OnceLock::new();
static CONFIG_UI_LIFECYCLE_MANAGER: OnceLock<Mutex<ConfigUiLifecycleManagerState>> =
    OnceLock::new();

pub(super) fn handle_config_ui_route(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: ConfigUiRouteContext<'_>,
) -> ConfigUiRouteOutcome {
    if ctx.mode != WebServerMode::StandaloneIde {
        return ConfigUiRouteOutcome::NotHandled(request);
    }

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

fn json_response(status: u16, body: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn structured_error_response(
    status: u16,
    error_code: &str,
    message: &str,
    field_errors: Vec<FieldErrorItem>,
    conflict_version: Option<String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        status,
        json!({
            "ok": false,
            "error_code": error_code,
            "message": message,
            "field_errors": field_errors,
            "conflict_version": conflict_version,
        }),
    )
}

fn load_workspace_model(bundle_root: &Option<PathBuf>) -> Result<WorkspaceModel, RuntimeError> {
    let root = default_bundle_root(bundle_root);
    let mut runtime_roots = Vec::<PathBuf>::new();
    if root.join("runtime.toml").is_file() {
        runtime_roots.push(root.clone());
    }
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let candidate = entry.path();
            if candidate.join("runtime.toml").is_file() {
                runtime_roots.push(candidate);
            }
        }
    }
    runtime_roots.sort();
    runtime_roots.dedup();
    if runtime_roots.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "no runtime.toml found in '{}' or direct subdirectories",
                root.display()
            )
            .into(),
        ));
    }

    let mut runtimes = Vec::new();
    let mut seen_ids = BTreeSet::new();
    for runtime_root in runtime_roots {
        let runtime = RuntimeConfig::load(runtime_root.join("runtime.toml"))?;
        let runtime_id = runtime.resource_name.to_string();
        if !seen_ids.insert(runtime_id.clone()) {
            return Err(RuntimeError::InvalidConfig(
                format!("duplicate runtime.resource.name '{runtime_id}' in workspace").into(),
            ));
        }
        runtimes.push(WorkspaceRuntime {
            runtime_id,
            root: runtime_root,
            runtime,
        });
    }
    runtimes.sort_by(|left, right| left.runtime_id.cmp(&right.runtime_id));

    Ok(WorkspaceModel { root, runtimes })
}

fn resolve_runtime_target<'a>(
    workspace: &'a WorkspaceModel,
    requested_runtime_id: Option<&str>,
    control_state: &Arc<ControlState>,
) -> Result<&'a WorkspaceRuntime, RuntimeError> {
    if let Some(requested) = requested_runtime_id
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return resolve_runtime_by_id(workspace, requested);
    }

    let connected_via =
        crate::control::runtime_resource_name_port(control_state.as_ref()).to_string();
    if !connected_via.is_empty() {
        if let Ok(runtime) = resolve_runtime_by_id(workspace, connected_via.as_str()) {
            return Ok(runtime);
        }
    }

    workspace
        .runtimes
        .first()
        .ok_or_else(|| RuntimeError::InvalidConfig("workspace has no runtimes".into()))
}

fn resolve_runtime_by_id<'a>(
    workspace: &'a WorkspaceModel,
    runtime_id: &str,
) -> Result<&'a WorkspaceRuntime, RuntimeError> {
    workspace
        .runtimes
        .iter()
        .find(|runtime| runtime.runtime_id == runtime_id)
        .ok_or_else(|| {
            RuntimeError::InvalidConfig(format!("unknown runtime_id '{runtime_id}'").into())
        })
}

fn normalize_runtime_id(raw: &str) -> Result<String, RuntimeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig("runtime_id is required".into()));
    }
    if trimmed
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime_id may only contain [a-zA-Z0-9-_]".into(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_host_group(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn render_new_runtime_toml(runtime_id: &str, host_group: Option<&str>) -> String {
    let normalized_group = host_group
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default-host");
    format!(
        r#"[bundle]
version = 1

[resource]
name = "{runtime_id}"
cycle_interval_ms = 20

[runtime.control]
endpoint = "unix:///tmp/{runtime_id}.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "127.0.0.1:0"
auth = "local"
tls = false

[runtime.discovery]
enabled = true
service_name = "{runtime_id}"
advertise = true
interfaces = ["lo"]
host_group = "{normalized_group}"

[runtime.mesh]
enabled = true
role = "peer"
listen = "127.0.0.1:0"
connect = []
tls = false
publish = []
subscribe = {{}}

[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []

[runtime.cloud.links]
transports = []
"#
    )
}

fn create_workspace_runtime(
    workspace: &WorkspaceModel,
    runtime_id: &str,
    host_group: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let runtime_id = normalize_runtime_id(runtime_id)?;
    if workspace
        .runtimes
        .iter()
        .any(|runtime| runtime.runtime_id == runtime_id)
    {
        return Err(RuntimeError::InvalidConfig(
            format!("runtime '{runtime_id}' already exists").into(),
        ));
    }

    let runtime_root = workspace.root.join(runtime_id.as_str());
    if runtime_root.exists() {
        return Err(RuntimeError::InvalidConfig(
            format!("runtime folder '{}' already exists", runtime_root.display()).into(),
        ));
    }

    let normalized_group = normalize_host_group(host_group);
    let runtime_text = render_new_runtime_toml(runtime_id.as_str(), normalized_group.as_deref());
    crate::config::validate_runtime_toml_text(runtime_text.as_str())?;

    let io_template = crate::bundle_template::build_io_config_auto("simulated")
        .map_err(|error| RuntimeError::InvalidConfig(error.to_string().into()))?;
    let io_text = crate::bundle_template::render_io_toml(&io_template);
    crate::config::validate_io_toml_text(io_text.as_str())?;

    let main_st_text = "PROGRAM Main\nVAR\n  x : INT := 0;\nEND_VAR\nEND_PROGRAM\n";

    atomic_write_text(
        runtime_root.join("runtime.toml").as_path(),
        runtime_text.as_str(),
    )?;
    atomic_write_text(runtime_root.join("io.toml").as_path(), io_text.as_str())?;
    atomic_write_text(runtime_root.join("src/main.st").as_path(), main_st_text)?;

    Ok(json!({
        "ok": true,
        "runtime_id": runtime_id,
        "runtime_root": runtime_root.display().to_string(),
        "host_group": normalized_group,
        "runtime_revision": text_revision(runtime_text.as_str()),
        "io_revision": text_revision(io_text.as_str()),
        "st_revision": text_revision(main_st_text),
        "message": "runtime created",
    }))
}

fn delete_workspace_runtime(
    workspace: &WorkspaceModel,
    runtime_id: &str,
) -> Result<serde_json::Value, RuntimeError> {
    if workspace.runtimes.len() <= 1 {
        return Err(RuntimeError::InvalidConfig(
            "cannot delete the last runtime in workspace".into(),
        ));
    }
    let runtime_id = normalize_runtime_id(runtime_id)?;
    let runtime = resolve_runtime_by_id(workspace, runtime_id.as_str())?;
    if runtime.root == workspace.root {
        return Err(RuntimeError::InvalidConfig(
            "refusing to delete workspace root runtime; move project to multi-runtime layout first"
                .into(),
        ));
    }
    if !runtime.root.starts_with(&workspace.root) {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "refusing to delete runtime outside workspace root: '{}'",
                runtime.root.display()
            )
            .into(),
        ));
    }
    fs::remove_dir_all(&runtime.root).map_err(|error| {
        RuntimeError::InvalidConfig(
            format!(
                "failed to delete runtime '{}': {error}",
                runtime.root.display()
            )
            .into(),
        )
    })?;
    Ok(json!({
        "ok": true,
        "runtime_id": runtime_id,
        "message": "runtime deleted",
    }))
}

fn text_revision(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

fn atomic_write_text(path: &Path, text: &str) -> Result<(), RuntimeError> {
    let parent = path.parent().ok_or_else(|| {
        RuntimeError::InvalidConfig(
            format!("invalid destination '{}': missing parent", path.display()).into(),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        RuntimeError::InvalidConfig(
            format!("failed to create directory '{}': {error}", parent.display()).into(),
        )
    })?;
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config"),
        std::process::id(),
        now_ns()
    ));
    fs::write(&temp_path, text).map_err(|error| {
        RuntimeError::InvalidConfig(
            format!(
                "failed to write temp file '{}': {error}",
                temp_path.display()
            )
            .into(),
        )
    })?;
    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        RuntimeError::InvalidConfig(
            format!("failed to replace '{}': {error}", path.display()).into(),
        )
    })
}

fn write_config_file(
    path: &Path,
    text: &str,
    expected_revision: Option<&str>,
    validator: fn(&str) -> Result<(), RuntimeError>,
) -> Result<String, RuntimeError> {
    let current_text = fs::read_to_string(path).unwrap_or_default();
    let current_revision = text_revision(current_text.as_str());
    if let Some(expected) = expected_revision
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if expected != current_revision {
            return Err(RuntimeError::ControlError(
                format!("conflict: {current_revision}").into(),
            ));
        }
    }
    validator(text)?;
    atomic_write_text(path, text)?;
    Ok(text_revision(text))
}

fn normalize_st_relative_path(path: &str) -> Result<PathBuf, RuntimeError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig("path is required".into()));
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return Err(RuntimeError::InvalidConfig(
            "path must be relative to src/".into(),
        ));
    }
    for component in candidate.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RuntimeError::InvalidConfig(
                    "path must stay under src/".into(),
                ));
            }
            _ => {}
        }
    }
    let extension = candidate
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "st" {
        return Err(RuntimeError::InvalidConfig(
            "path must reference a .st file".into(),
        ));
    }
    Ok(candidate)
}

fn load_project_io_config_response(runtime_root: &Path) -> Result<IoConfigResponse, RuntimeError> {
    let io_path = runtime_root.join("io.toml");
    if io_path.is_file() {
        let config = IoConfig::load(&io_path)?;
        return Ok(io_config_to_response(config, "project", false));
    }
    Ok(IoConfigResponse {
        driver: "loopback".to_string(),
        params: json!({}),
        drivers: Vec::new(),
        safe_state: Vec::new(),
        supported_drivers: IoDriverRegistry::default_registry().canonical_driver_names(),
        source: "default".to_string(),
        use_system_io: false,
    })
}

fn host_groups_from_workspace(workspace: &WorkspaceModel) -> Vec<Vec<String>> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for runtime in &workspace.runtimes {
        let group_key = runtime
            .runtime
            .discovery
            .host_group
            .as_ref()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("host-{}", runtime.runtime_id));
        grouped
            .entry(group_key)
            .or_default()
            .push(runtime.runtime_id.clone());
    }
    grouped
        .into_values()
        .map(|mut ids| {
            ids.sort();
            ids
        })
        .collect()
}

fn config_mode_runtime_cloud_state(workspace: &WorkspaceModel) -> RuntimeCloudUiState {
    let now = now_ns();
    let connected_via = workspace
        .runtimes
        .first()
        .map(|runtime| runtime.runtime_id.clone())
        .unwrap_or_else(|| "runtime-1".to_string());
    let acting_on = workspace
        .runtimes
        .iter()
        .map(|runtime| runtime.runtime_id.clone())
        .collect::<Vec<_>>();
    let peers = workspace
        .runtimes
        .iter()
        .filter(|runtime| runtime.runtime_id != connected_via)
        .map(|runtime| RuntimePresenceRecord {
            runtime_id: runtime.runtime_id.clone(),
            site: "local".to_string(),
            display_name: runtime.runtime_id.clone(),
            mesh_reachable: true,
            last_seen_ns: now,
            stale: false,
            partitioned: false,
        })
        .collect::<Vec<_>>();

    let mut state = project_runtime_cloud_state(
        UiContext {
            connected_via: connected_via.clone(),
            acting_on,
            site_scope: vec!["local".to_string()],
            identity: "config://local-engineering".to_string(),
            role: "engineer".to_string(),
            mode: UiMode::Edit,
        },
        connected_via.as_str(),
        "local",
        now,
        &peers,
    );
    state.topology.host_groups = host_groups_from_workspace(workspace);
    state.feature_flags = runtime_cloud_topology_feature_flags(RuntimeCloudProfile::Dev);
    state.feature_flags.insert("edit_mode".to_string(), true);

    let mut existing = BTreeSet::<(String, String)>::new();
    for edge in &state.topology.edges {
        existing.insert((edge.source.clone(), edge.target.clone()));
    }

    let known = workspace
        .runtimes
        .iter()
        .map(|runtime| runtime.runtime_id.clone())
        .collect::<BTreeSet<_>>();

    for runtime in &workspace.runtimes {
        for preference in &runtime.runtime.runtime_cloud_link_preferences {
            let source = preference.source.to_string();
            let target = preference.target.to_string();
            if source == target || !known.contains(&source) || !known.contains(&target) {
                continue;
            }
            let channel = match preference.transport {
                crate::config::RuntimeCloudPreferredTransport::Realtime => ChannelType::T0HardRt,
                crate::config::RuntimeCloudPreferredTransport::Zenoh => ChannelType::MeshT2Ops,
                crate::config::RuntimeCloudPreferredTransport::Mesh => ChannelType::MeshT1Fast,
                crate::config::RuntimeCloudPreferredTransport::Discovery => ChannelType::MeshT3Diag,
                crate::config::RuntimeCloudPreferredTransport::Mqtt
                | crate::config::RuntimeCloudPreferredTransport::ModbusTcp
                | crate::config::RuntimeCloudPreferredTransport::OpcUa
                | crate::config::RuntimeCloudPreferredTransport::Web => {
                    ChannelType::FederationBridge
                }
            };
            if let Some(edge) = state
                .topology
                .edges
                .iter_mut()
                .find(|edge| edge.source == source && edge.target == target)
            {
                edge.channel_type = channel;
                continue;
            }
            if existing.insert((source.clone(), target.clone())) {
                state.topology.edges.push(FleetEdge {
                    source,
                    target,
                    channel_type: channel,
                    state: ChannelState::Healthy,
                    latency_ms_p95: Some(2.0),
                    loss_pct: Some(0.0),
                    stale: false,
                    last_ok_ns: now,
                });
            }
        }
    }

    apply_config_mode_offline_projection(&mut state);
    apply_config_mode_live_overlay(&mut state);
    state
}

fn apply_config_mode_offline_projection(state: &mut RuntimeCloudUiState) {
    // Config UI can run without live runtimes. Render topology as planned/offline by default.
    for node in &mut state.topology.nodes {
        node.lifecycle_state = crate::runtime_cloud::projection::LifecycleState::Offline;
        node.health_state = crate::runtime_cloud::projection::HealthState::Degraded;
        node.config_state = crate::runtime_cloud::projection::ConfigState::Pending;
        node.last_seen_ns = 0;
    }
    for edge in &mut state.topology.edges {
        edge.state = ChannelState::Failed;
        edge.stale = true;
        edge.latency_ms_p95 = None;
        edge.loss_pct = None;
        edge.last_ok_ns = 0;
    }
    state.timeline.clear();
}

fn apply_config_mode_live_overlay(state: &mut RuntimeCloudUiState) {
    let remote = config_ui_live_manager()
        .lock()
        .ok()
        .and_then(|guard| {
            if guard.connected {
                guard.last_runtime_cloud.clone()
            } else {
                None
            }
        })
        .and_then(|value| serde_json::from_value::<RuntimeCloudUiState>(value).ok());
    let Some(remote) = remote else {
        return;
    };

    let remote_nodes = remote
        .topology
        .nodes
        .into_iter()
        .map(|node| (node.runtime_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut overlay_count = 0usize;
    for node in &mut state.topology.nodes {
        if let Some(remote_node) = remote_nodes.get(node.runtime_id.as_str()) {
            node.lifecycle_state = remote_node.lifecycle_state;
            node.health_state = remote_node.health_state;
            node.config_state = remote_node.config_state;
            node.last_seen_ns = remote_node.last_seen_ns;
            overlay_count += 1;
        }
    }

    for edge in &mut state.topology.edges {
        if let Some(remote_edge) = remote
            .topology
            .edges
            .iter()
            .find(|candidate| {
                candidate.source == edge.source
                    && candidate.target == edge.target
                    && candidate.channel_type == edge.channel_type
            })
            .or_else(|| {
                remote.topology.edges.iter().find(|candidate| {
                    candidate.source == edge.source && candidate.target == edge.target
                })
            })
        {
            edge.state = remote_edge.state;
            edge.latency_ms_p95 = remote_edge.latency_ms_p95;
            edge.loss_pct = remote_edge.loss_pct;
            edge.stale = remote_edge.stale;
            edge.last_ok_ns = remote_edge.last_ok_ns;
        }
    }
    if overlay_count > 0 {
        state
            .feature_flags
            .insert("config_live_overlay".to_string(), true);
    }
}

fn config_mode_runtime_cloud_config_snapshot(
    workspace: &WorkspaceModel,
) -> RuntimeCloudConfigSnapshot {
    let runtime_id = workspace
        .runtimes
        .first()
        .map(|runtime| runtime.runtime_id.clone())
        .unwrap_or_else(|| "runtime-1".to_string());
    let state = runtime_cloud_config_initial_state();
    RuntimeCloudConfigSnapshot {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        runtime_id,
        desired: state.desired,
        reported: state.reported,
        meta: state.meta,
        status: state.status,
    }
}

fn config_project_state(workspace: WorkspaceModel) -> Result<serde_json::Value, RuntimeError> {
    let mut hasher = Sha256::new();
    let mut runtimes = Vec::new();

    for runtime in workspace.runtimes {
        let runtime_toml_path = runtime.root.join("runtime.toml");
        let io_toml_path = runtime.root.join("io.toml");
        let runtime_text = fs::read_to_string(&runtime_toml_path).map_err(|error| {
            RuntimeError::InvalidConfig(format!("failed to read runtime.toml: {error}").into())
        })?;
        let io_text = fs::read_to_string(&io_toml_path).unwrap_or_default();
        let st_files = list_sources(runtime.root.as_path());

        hasher.update(runtime.runtime_id.as_bytes());
        hasher.update(runtime_text.as_bytes());
        hasher.update(io_text.as_bytes());
        for file in &st_files {
            hasher.update(file.as_bytes());
        }

        runtimes.push(json!({
            "runtime_id": runtime.runtime_id,
            "project_path": runtime.root.display().to_string(),
            "host_group": runtime.runtime.discovery.host_group.map(|v| v.to_string()),
            "runtime_revision": text_revision(runtime_text.as_str()),
            "io_revision": text_revision(io_text.as_str()),
            "st_files": st_files,
        }));
    }

    Ok(json!({
        "ok": true,
        "api_version": RUNTIME_CLOUD_API_VERSION,
        "mode": "config",
        "project_root": workspace.root.display().to_string(),
        "revision": format!("{:x}", hasher.finalize()),
        "runtimes": runtimes,
    }))
}

fn validate_st_sources(
    runtime_root: &Path,
    override_path: Option<&str>,
    override_text: Option<&str>,
) -> Result<Vec<serde_json::Value>, RuntimeError> {
    let mut source_map = BTreeMap::<String, String>::new();
    for file in list_sources(runtime_root) {
        let text = read_source_file(runtime_root, file.as_str())?;
        source_map.insert(file, text);
    }

    if let Some(path) = override_path {
        let normalized = normalize_st_relative_path(path)?;
        let text = if let Some(override_text) = override_text {
            override_text.to_string()
        } else {
            read_source_file(runtime_root, normalized.to_string_lossy().as_ref())?
        };
        source_map.insert(normalized.to_string_lossy().to_string(), text);
    }

    if source_map.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "no ST sources found under src/".into(),
        ));
    }

    let sources = source_map
        .iter()
        .map(|(path, text)| HarnessSourceFile::with_path(path.clone(), text.clone()))
        .collect::<Vec<_>>();

    match CompileSession::from_sources(sources).build_bytecode_module() {
        Ok(_) => Ok(Vec::new()),
        Err(error) => Err(RuntimeError::InvalidConfig(
            format!("ST validation failed: {error}").into(),
        )),
    }
}

fn config_ui_live_manager() -> &'static Mutex<ConfigUiLiveManagerState> {
    CONFIG_UI_LIVE_MANAGER.get_or_init(|| Mutex::new(ConfigUiLiveManagerState::default()))
}

fn config_ui_lifecycle_manager() -> &'static Mutex<ConfigUiLifecycleManagerState> {
    CONFIG_UI_LIFECYCLE_MANAGER.get_or_init(|| Mutex::new(ConfigUiLifecycleManagerState::default()))
}

fn normalize_live_target(raw: &str) -> Result<String, RuntimeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig("target is required".into()));
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let normalized = with_scheme.trim_end_matches('/').to_string();
    if normalized == "http:" || normalized == "https:" || normalized.ends_with("://") {
        return Err(RuntimeError::InvalidConfig(
            "target must include host".into(),
        ));
    }
    Ok(normalized)
}

fn config_ui_live_targets_snapshot() -> serde_json::Value {
    let manager = config_ui_live_manager()
        .lock()
        .ok()
        .map(|guard| config_ui_live_targets_snapshot_with_guard(&guard))
        .unwrap_or_else(|| {
            json!({
                "ok": false,
                "profiles": [],
                "active_target": null,
                "connected": false,
                "last_error": "live manager unavailable",
                "updated_at_ns": now_ns(),
            })
        });
    manager
}

fn config_ui_live_targets_snapshot_with_guard(
    guard: &ConfigUiLiveManagerState,
) -> serde_json::Value {
    let profiles = guard
        .profiles
        .values()
        .cloned()
        .collect::<Vec<ConfigUiLiveTargetProfile>>();
    json!({
        "ok": true,
        "profiles": profiles,
        "active_target": guard.active_target,
        "connected": guard.connected,
        "last_error": guard.last_error,
        "updated_at_ns": guard.updated_at_ns,
    })
}

fn config_ui_live_target_upsert(
    payload: &ConfigLiveTargetUpsertRequest,
) -> Result<serde_json::Value, RuntimeError> {
    let target = normalize_live_target(payload.target.as_str())?;
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(target.as_str())
        .to_string();
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    guard.profiles.insert(
        target.clone(),
        ConfigUiLiveTargetProfile {
            target: target.clone(),
            label,
        },
    );
    guard.updated_at_ns = now_ns();
    let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
    Ok(json!({
        "ok": true,
        "target": target,
        "snapshot": snapshot,
    }))
}

fn config_ui_live_target_remove(
    payload: &ConfigLiveTargetRemoveRequest,
) -> Result<serde_json::Value, RuntimeError> {
    let target = normalize_live_target(payload.target.as_str())?;
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    guard.profiles.remove(&target);
    if guard.active_target.as_deref() == Some(target.as_str()) {
        guard.active_target = None;
        guard.active_token = None;
        guard.connected = false;
        guard.last_error = None;
        guard.last_runtime_cloud = None;
    }
    guard.updated_at_ns = now_ns();
    let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
    Ok(json!({
        "ok": true,
        "target": target,
        "snapshot": snapshot,
    }))
}

fn fetch_runtime_cloud_state(
    target: &str,
    token: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let target = normalize_live_target(target)?;
    let state_url = format!("{target}/api/runtime-cloud/state");
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_millis(800)))
        .timeout_recv_response(Some(Duration::from_millis(1500)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();
    let request = token
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| agent.get(&state_url).header("X-Trust-Token", value))
        .unwrap_or_else(|| agent.get(&state_url));
    let mut response = request.call().map_err(|error| {
        RuntimeError::ControlError(format!("live connect request failed: {error}").into())
    })?;
    let status = response.status().as_u16();
    let body_text = response.body_mut().read_to_string().unwrap_or_default();
    let body: serde_json::Value = serde_json::from_str(&body_text).unwrap_or_else(|_| json!({}));
    if status >= 400 {
        let detail = body
            .get("error")
            .and_then(serde_json::Value::as_str)
            .or_else(|| body.get("message").and_then(serde_json::Value::as_str))
            .unwrap_or("remote runtime-cloud request failed");
        return Err(RuntimeError::ControlError(
            format!("live connect failed ({status}): {detail}").into(),
        ));
    }
    if body.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        let detail = body
            .get("error")
            .and_then(serde_json::Value::as_str)
            .or_else(|| body.get("message").and_then(serde_json::Value::as_str))
            .unwrap_or("remote runtime-cloud returned error");
        return Err(RuntimeError::ControlError(
            format!("live connect failed: {detail}").into(),
        ));
    }
    Ok(body)
}

fn config_ui_live_connect(
    target: Option<&str>,
    token: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    let chosen_target = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_live_target)
        .transpose()?
        .or_else(|| guard.active_target.clone());

    let Some(chosen_target) = chosen_target else {
        guard.active_target = None;
        guard.connected = false;
        guard.last_error = None;
        guard.last_runtime_cloud = None;
        guard.updated_at_ns = now_ns();
        let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
        return Ok(json!({
            "ok": true,
            "connected": false,
            "active_target": null,
            "snapshot": snapshot,
        }));
    };

    if !guard.profiles.contains_key(&chosen_target) {
        guard.profiles.insert(
            chosen_target.clone(),
            ConfigUiLiveTargetProfile {
                target: chosen_target.clone(),
                label: chosen_target.clone(),
            },
        );
    }

    guard.active_target = Some(chosen_target.clone());
    guard.active_token = token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| guard.active_token.clone());

    match fetch_runtime_cloud_state(&chosen_target, guard.active_token.as_deref()) {
        Ok(value) => {
            guard.connected = true;
            guard.last_error = None;
            guard.last_runtime_cloud = Some(value.clone());
            guard.updated_at_ns = now_ns();
            let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
            Ok(json!({
                "ok": true,
                "connected": true,
                "active_target": chosen_target,
                "runtime_cloud": value,
                "snapshot": snapshot,
            }))
        }
        Err(error) => {
            guard.connected = false;
            guard.last_error = Some(error.to_string());
            guard.last_runtime_cloud = None;
            guard.updated_at_ns = now_ns();
            let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
            Ok(json!({
                "ok": true,
                "connected": false,
                "active_target": chosen_target,
                "last_error": guard.last_error,
                "snapshot": snapshot,
            }))
        }
    }
}

fn config_ui_live_state(target: Option<&str>) -> Result<serde_json::Value, RuntimeError> {
    let target = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_live_target)
        .transpose()?;
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    let chosen = target.or_else(|| guard.active_target.clone());
    let Some(chosen) = chosen else {
        return Ok(json!({
            "ok": true,
            "connected": false,
            "active_target": null,
            "runtime_cloud": null,
            "last_error": guard.last_error,
            "updated_at_ns": guard.updated_at_ns,
        }));
    };

    match fetch_runtime_cloud_state(&chosen, guard.active_token.as_deref()) {
        Ok(value) => {
            guard.active_target = Some(chosen.clone());
            guard.connected = true;
            guard.last_error = None;
            guard.last_runtime_cloud = Some(value.clone());
            guard.updated_at_ns = now_ns();
            Ok(json!({
                "ok": true,
                "connected": true,
                "active_target": chosen,
                "runtime_cloud": value,
                "last_error": null,
                "updated_at_ns": guard.updated_at_ns,
            }))
        }
        Err(error) => {
            guard.active_target = Some(chosen.clone());
            guard.connected = false;
            guard.last_error = Some(error.to_string());
            guard.last_runtime_cloud = None;
            guard.updated_at_ns = now_ns();
            Ok(json!({
                "ok": true,
                "connected": false,
                "active_target": chosen,
                "runtime_cloud": null,
                "last_error": guard.last_error,
                "updated_at_ns": guard.updated_at_ns,
            }))
        }
    }
}

fn control_endpoint_online(endpoint: &str) -> bool {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(rest) = trimmed.strip_prefix("tcp://") {
        let mut socket_addrs = match rest.to_socket_addrs() {
            Ok(value) => value,
            Err(_) => return false,
        };
        if let Some(addr) = socket_addrs.next() {
            return std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok();
        }
        return false;
    }
    #[cfg(unix)]
    if let Some(path) = trimmed.strip_prefix("unix://") {
        if path.trim().is_empty() {
            return false;
        }
        return std::os::unix::net::UnixStream::connect(path).is_ok();
    }
    false
}

fn prune_managed_runtime_processes(manager: &mut ConfigUiLifecycleManagerState) {
    manager
        .managed
        .retain(|_, process| match process.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        });
}

fn managed_runtime_pid(process: &ConfigUiManagedRuntimeProcess) -> u32 {
    process.child.id()
}

fn runtime_lifecycle_item(
    runtime: &WorkspaceRuntime,
    process: Option<&ConfigUiManagedRuntimeProcess>,
) -> serde_json::Value {
    let host_group = runtime
        .runtime
        .discovery
        .host_group
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let externally_running = control_endpoint_online(runtime.runtime.control_endpoint.as_str());
    let (managed, managed_running, pid, started_at_ns, listen) = if let Some(process) = process {
        (
            true,
            true,
            Some(managed_runtime_pid(process)),
            Some(process.started_at_ns),
            process.listen.clone(),
        )
    } else {
        (
            false,
            false,
            None,
            None,
            runtime.runtime.web.listen.to_string(),
        )
    };
    let running = managed_running || externally_running;
    json!({
        "runtime_id": runtime.runtime_id,
        "runtime_root": runtime.root.display().to_string(),
        "host_group": host_group,
        "control_endpoint": runtime.runtime.control_endpoint.to_string(),
        "web_listen": listen,
        "managed": managed,
        "running": running,
        "externally_running": externally_running,
        "pid": pid,
        "started_at_ns": started_at_ns,
    })
}

fn config_ui_runtime_lifecycle_snapshot(
    workspace: &WorkspaceModel,
    _request_token: Option<&str>,
) -> Result<Vec<serde_json::Value>, RuntimeError> {
    let mut guard = config_ui_lifecycle_manager().lock().map_err(|_| {
        RuntimeError::ControlError("failed to lock config-ui lifecycle manager".into())
    })?;
    prune_managed_runtime_processes(&mut guard);
    let mut items = Vec::with_capacity(workspace.runtimes.len());
    for runtime in &workspace.runtimes {
        let process = guard.managed.get(runtime.runtime_id.as_str());
        items.push(runtime_lifecycle_item(runtime, process));
    }
    Ok(items)
}

fn launch_runtime_for_workspace_runtime(
    runtime: &WorkspaceRuntime,
) -> Result<ConfigUiManagedRuntimeProcess, RuntimeError> {
    let exe = std::env::current_exe().map_err(|error| {
        RuntimeError::ControlError(
            format!("failed to resolve trust-runtime executable path: {error}").into(),
        )
    })?;
    let mut command = Command::new(exe);
    command
        .arg("play")
        .arg("--project")
        .arg(runtime.root.as_os_str())
        .arg("--no-console")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn().map_err(|error| {
        RuntimeError::ControlError(
            format!(
                "failed to start runtime '{}' from config-ui: {error}",
                runtime.runtime_id
            )
            .into(),
        )
    })?;
    Ok(ConfigUiManagedRuntimeProcess {
        listen: runtime.runtime.web.listen.to_string(),
        child,
        started_at_ns: now_ns(),
    })
}

fn stop_managed_runtime_process(
    process: &mut ConfigUiManagedRuntimeProcess,
) -> Result<(), RuntimeError> {
    process.child.kill().map_err(|error| {
        RuntimeError::ControlError(
            format!("failed to stop managed runtime process: {error}").into(),
        )
    })?;
    let _ = process.child.wait();
    Ok(())
}

fn config_ui_runtime_lifecycle_apply(
    workspace: &WorkspaceModel,
    payload: &ConfigRuntimeLifecycleRequest,
    _request_token: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let runtime_id = normalize_runtime_id(payload.runtime_id.as_str())?;
    let runtime = resolve_runtime_by_id(workspace, runtime_id.as_str())?;
    let action = payload.action.trim().to_ascii_lowercase();
    if action.is_empty() {
        return Err(RuntimeError::InvalidConfig("action is required".into()));
    }
    let mut guard = config_ui_lifecycle_manager().lock().map_err(|_| {
        RuntimeError::ControlError("failed to lock config-ui lifecycle manager".into())
    })?;
    prune_managed_runtime_processes(&mut guard);

    let externally_running = control_endpoint_online(runtime.runtime.control_endpoint.as_str());
    let managed_present = guard.managed.contains_key(runtime_id.as_str());
    let result = match action.as_str() {
        "start" => {
            if managed_present || externally_running {
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "already_running",
                })
            } else {
                let process = launch_runtime_for_workspace_runtime(runtime)?;
                let pid = managed_runtime_pid(&process);
                guard.managed.insert(runtime_id.clone(), process);
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "started",
                    "pid": pid,
                })
            }
        }
        "stop" => {
            if let Some(mut process) = guard.managed.remove(runtime_id.as_str()) {
                stop_managed_runtime_process(&mut process)?;
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "stopped",
                })
            } else if externally_running {
                return Err(RuntimeError::InvalidConfig(
                    format!(
                        "runtime '{runtime_id}' is running but not managed by config-ui; stop it via runtime control endpoint"
                    )
                    .into(),
                ));
            } else {
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "already_stopped",
                })
            }
        }
        "restart" => {
            if let Some(mut process) = guard.managed.remove(runtime_id.as_str()) {
                let _ = stop_managed_runtime_process(&mut process);
            } else if externally_running {
                return Err(RuntimeError::InvalidConfig(
                    format!(
                        "runtime '{runtime_id}' is running but not managed by config-ui; restart it via runtime control endpoint"
                    )
                    .into(),
                ));
            }
            let process = launch_runtime_for_workspace_runtime(runtime)?;
            let pid = managed_runtime_pid(&process);
            guard.managed.insert(runtime_id.clone(), process);
            json!({
                "ok": true,
                "runtime_id": runtime_id,
                "action": action,
                "result": "restarted",
                "pid": pid,
            })
        }
        "status" | "probe" => json!({
            "ok": true,
            "runtime_id": runtime_id,
            "action": action,
            "result": if managed_present || externally_running { "running" } else { "stopped" },
        }),
        _ => {
            return Err(RuntimeError::InvalidConfig(
                format!("unsupported lifecycle action '{}'", payload.action).into(),
            ))
        }
    };

    prune_managed_runtime_processes(&mut guard);
    let process = guard.managed.get(runtime_id.as_str());
    Ok(json!({
        "ok": true,
        "result": result,
        "item": runtime_lifecycle_item(runtime, process),
        "requested_mode": payload.mode.as_deref().unwrap_or(""),
    }))
}
