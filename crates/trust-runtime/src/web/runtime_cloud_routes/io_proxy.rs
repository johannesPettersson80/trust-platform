use super::*;
use crate::runtime_cloud::io_proxy_policy::{
    runtime_cloud_io_proxy_plan, RuntimeCloudIoProxyOperation, RuntimeCloudIoProxyPlan,
    RUNTIME_CLOUD_IO_PROXY_ACTOR,
};

fn json_response(status: u16, body: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn denied_preflight_response(
    preflight: RuntimeCloudActionPreflight,
) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        403,
        json!({
            "ok": false,
            "denial_code": preflight.denial_code,
            "denial_reason": preflight.denial_reason,
            "error": preflight.denial_reason.clone().unwrap_or_else(|| "runtime cloud preflight denied".to_string()),
            "preflight": preflight,
        }),
    )
}

fn runtime_cloud_io_preflight(
    ctx: &RuntimeCloudRouteContext<'_>,
    web_role: AccessRole,
    local_runtime: &str,
    action: &RuntimeCloudActionRequest,
) -> RuntimeCloudActionPreflight {
    let (preflight, _ha_request, _known_targets) = runtime_cloud_preflight_for_action(
        action,
        local_runtime,
        ctx.discovery.as_ref(),
        RuntimeCloudPreflightPolicy {
            role: web_role,
            local_supports_secure_transport: ctx.web_tls_enabled,
            profile: ctx.profile,
            wan_allow_write: ctx.wan_allow_write,
            auth_mode: ctx.auth_mode,
        },
        ctx.ha_state.as_ref(),
    );
    preflight
}

fn remote_io_config_url(
    ctx: &RuntimeCloudRouteContext<'_>,
    target_runtime: &str,
) -> Option<String> {
    runtime_cloud_target_web_base_url(
        ctx.discovery.as_ref(),
        target_runtime,
        ctx.profile.requires_secure_transport(),
    )
    .map(|base| format!("{base}/api/io/config"))
}

fn remote_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_millis(500)))
        .timeout_recv_response(Some(Duration::from_millis(1500)))
        .http_status_as_error(false)
        .build();
    config.into()
}

pub(super) fn handle_get_io_config(
    request: tiny_http::Request,
    url: &str,
    ctx: &RuntimeCloudRouteContext<'_>,
) {
    let (web_role, request_token) = match check_auth_with_role(
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

    let local_runtime = local_runtime_id(ctx);
    let target_runtime = query_value(url, "target").unwrap_or_else(|| local_runtime.clone());
    let plan = match runtime_cloud_io_proxy_plan(
        RuntimeCloudIoProxyOperation::ReadConfig,
        RUNTIME_CLOUD_API_VERSION,
        RUNTIME_CLOUD_IO_PROXY_ACTOR,
        target_runtime.as_str(),
        local_runtime.as_str(),
        now_ns(),
    ) {
        Ok(plan) => plan,
        Err(error) => {
            let _ = request.respond(json_response(
                400,
                json!({
                    "ok": false,
                    "denial_code": error.code,
                    "error": error.message,
                }),
            ));
            return;
        }
    };

    let RuntimeCloudIoProxyPlan {
        target_runtime,
        action_type,
        action,
    } = plan;
    let preflight = runtime_cloud_io_preflight(ctx, web_role, local_runtime.as_str(), &action);
    if !preflight.allowed {
        let _ = request.respond(denied_preflight_response(preflight));
        return;
    }

    if target_runtime == local_runtime {
        let response = match load_io_config(ctx.bundle_root) {
            Ok(config) => json_response(
                200,
                serde_json::to_value(config).unwrap_or_else(|_| json!({})),
            ),
            Err(error) => json_response(
                500,
                json!({
                    "ok": false,
                    "denial_code": ReasonCode::TransportFailure,
                    "error": error.to_string(),
                }),
            ),
        };
        let _ = request.respond(response);
        return;
    }

    let Some(remote_url) = remote_io_config_url(ctx, target_runtime.as_str()) else {
        let _ = request.respond(json_response(
            503,
            json!({
                "ok": false,
                "denial_code": ReasonCode::TargetUnreachable,
                "error": format!("target runtime '{}' is not reachable", target_runtime),
            }),
        ));
        return;
    };

    let mut remote = remote_agent().get(remote_url.as_str());
    if let Some(token) = request_token.as_deref() {
        remote = remote.header("X-Trust-Token", token);
    }
    let response = match remote.call() {
        Ok(mut remote_response) => {
            let status = remote_response.status().as_u16();
            let text = remote_response
                .body_mut()
                .read_to_string()
                .unwrap_or_default();
            if (200..300).contains(&status) {
                let value = serde_json::from_str::<serde_json::Value>(&text)
                    .unwrap_or_else(|_| json!({ "ok": false, "error": "invalid remote response" }));
                json_response(status, value)
            } else {
                let mut value =
                    serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|_| json!({}));
                if !value.is_object() {
                    value = json!({ "ok": false, "error": format!("http status {status}") });
                }
                value["ok"] = serde_json::Value::Bool(false);
                if value.get("denial_code").is_none() {
                    value["denial_code"] = serde_json::to_value(
                        runtime_cloud_map_remote_http_status(status, action_type.as_str()),
                    )
                    .unwrap_or(serde_json::Value::String("transport_failure".to_string()));
                }
                if value.get("error").is_none() {
                    value["error"] = serde_json::Value::String(format!("http status {status}"));
                }
                json_response(status, value)
            }
        }
        Err(error) => json_response(
            503,
            json!({
                "ok": false,
                "denial_code": ReasonCode::TargetUnreachable,
                "error": error.to_string(),
            }),
        ),
    };
    let _ = request.respond(response);
}

pub(super) fn handle_post_io_config(
    mut request: tiny_http::Request,
    ctx: &RuntimeCloudRouteContext<'_>,
) {
    let (web_role, request_token) = match check_auth_with_role(
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
    let payload: RuntimeCloudIoConfigProxyRequest =
        match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return;
            }
        };

    let local_runtime = local_runtime_id(ctx);
    let plan = match runtime_cloud_io_proxy_plan(
        RuntimeCloudIoProxyOperation::WriteConfig,
        payload.api_version.as_str(),
        payload.actor.as_str(),
        payload.target_runtime.as_str(),
        local_runtime.as_str(),
        now_ns(),
    ) {
        Ok(plan) => plan,
        Err(error) => {
            let _ = request.respond(json_response(
                400,
                json!({
                    "ok": false,
                    "denial_code": error.code,
                    "error": error.message,
                }),
            ));
            return;
        }
    };

    let RuntimeCloudIoProxyPlan {
        target_runtime,
        action_type,
        action,
    } = plan;
    let preflight = runtime_cloud_io_preflight(ctx, web_role, local_runtime.as_str(), &action);
    if !preflight.allowed {
        let _ = request.respond(denied_preflight_response(preflight));
        return;
    }

    let io_request = payload.to_io_config_request();
    if target_runtime == local_runtime {
        let response = match save_io_config(ctx.bundle_root, &io_request) {
            Ok(message) => json_response(200, json!({ "ok": true, "message": message })),
            Err(error) => {
                let error_text = error.to_string();
                json_response(
                    400,
                    json!({
                        "ok": false,
                        "denial_code": runtime_cloud_map_control_error(error_text.as_str(), action_type.as_str()),
                        "error": error_text,
                    }),
                )
            }
        };
        let _ = request.respond(response);
        return;
    }

    let Some(remote_url) = remote_io_config_url(ctx, target_runtime.as_str()) else {
        let _ = request.respond(json_response(
            503,
            json!({
                "ok": false,
                "denial_code": ReasonCode::TargetUnreachable,
                "error": format!("target runtime '{}' is not reachable", target_runtime),
            }),
        ));
        return;
    };

    let body = match serde_json::to_string(&io_request) {
        Ok(text) => text,
        Err(error) => {
            let _ = request.respond(json_response(
                400,
                json!({
                    "ok": false,
                    "denial_code": ReasonCode::ContractViolation,
                    "error": error.to_string(),
                }),
            ));
            return;
        }
    };

    let mut remote = remote_agent()
        .post(remote_url.as_str())
        .header("Content-Type", "application/json");
    if let Some(token) = request_token.as_deref() {
        remote = remote.header("X-Trust-Token", token);
    }
    let response = match remote.send(body) {
        Ok(mut remote_response) => {
            let status = remote_response.status().as_u16();
            let text = remote_response
                .body_mut()
                .read_to_string()
                .unwrap_or_default();
            if !(200..300).contains(&status) {
                json_response(
                    status,
                    json!({
                        "ok": false,
                        "denial_code": runtime_cloud_map_remote_http_status(status, action_type.as_str()),
                        "error": if text.trim().is_empty() { format!("http status {status}") } else { text },
                    }),
                )
            } else if text.trim().to_ascii_lowercase().starts_with("error:") {
                json_response(
                    400,
                    json!({
                        "ok": false,
                        "denial_code": ReasonCode::ContractViolation,
                        "error": text.trim(),
                    }),
                )
            } else {
                json_response(
                    status,
                    json!({
                        "ok": true,
                        "message": if text.trim().is_empty() { "I/O config saved." } else { text.trim() },
                    }),
                )
            }
        }
        Err(error) => json_response(
            503,
            json!({
                "ok": false,
                "denial_code": ReasonCode::TargetUnreachable,
                "error": error.to_string(),
            }),
        ),
    };
    let _ = request.respond(response);
}
