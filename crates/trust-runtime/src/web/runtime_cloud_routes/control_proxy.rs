use super::*;
use crate::control::required_role_for_control_request;

pub(super) fn handle_post_control_proxy(
    mut request: tiny_http::Request,
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
    if let Err(response) = api_post_policy_check(&request, ctx.web_tls_enabled, true) {
        let _ = request.respond(response);
        return;
    }
    let payload: RuntimeCloudControlProxyRequest =
        match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return;
            }
        };

    if let Some(response) = validate_proxy_payload(&payload) {
        let _ = request.respond(response);
        return;
    }

    let kind = payload.control_request.r#type.trim();
    let params = payload.control_request.params.clone();
    let required_role = required_role_for_control_request(kind, params.as_ref());
    if !web_role.allows(required_role) {
        let denial_code = if kind == "config.set" {
            ReasonCode::AclDeniedCfgWrite
        } else {
            ReasonCode::PermissionDenied
        };
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": denial_code,
                "error": format!(
                    "role '{}' does not satisfy required role '{}' for '{}'",
                    web_role.as_str(),
                    required_role.as_str(),
                    kind,
                ),
            })
            .to_string(),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    let local_runtime = local_runtime_id(ctx);
    let target_runtime = payload.target_runtime.trim().to_string();
    let action_type = proxy_action_type(kind, required_role);
    let action = RuntimeCloudActionRequest {
        api_version: payload.api_version.clone(),
        request_id: payload
            .control_request
            .request_id
            .clone()
            .unwrap_or_else(|| format!("proxy-{}", now_ns())),
        connected_via: local_runtime.clone(),
        target_runtimes: vec![target_runtime.clone()],
        actor: payload.actor.trim().to_string(),
        action_type: action_type.to_string(),
        query_budget_ms: Some(1_500),
        dry_run: false,
        payload: proxy_action_payload(action_type, kind, params.as_ref()),
    };
    let (preflight, _ha_request, _known_targets) = runtime_cloud_preflight_for_action(
        &action,
        local_runtime.as_str(),
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
    if !preflight.allowed {
        let (denial_code, denial_reason) = preflight
            .decisions
            .iter()
            .find(|decision| decision.runtime_id == target_runtime)
            .map(|decision| (decision.denial_code, decision.denial_reason.clone()))
            .unwrap_or((preflight.denial_code, preflight.denial_reason.clone()));
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": denial_code,
                "denial_reason": denial_reason,
                "error": denial_reason.clone().unwrap_or_else(|| "control proxy preflight denied".to_string()),
            })
            .to_string(),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    let control_payload = proxy_control_payload(kind, params.as_ref(), action.request_id.as_str());
    let response = if target_runtime == local_runtime {
        let control_response = dispatch_control_request(
            control_payload,
            ctx.control_state,
            Some("runtime-cloud-proxy"),
            request_token.as_deref(),
        );
        let mut value = serde_json::to_value(&control_response)
            .unwrap_or_else(|_| json!({ "ok": false, "error": "serialize error" }));
        let ok = value
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !ok && value.get("denial_code").is_none() {
            value["denial_code"] = serde_json::to_value(runtime_cloud_map_control_error(
                value
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("control proxy failed"),
                action_type,
            ))
            .unwrap_or(serde_json::Value::String("transport_failure".to_string()));
        }
        Response::from_string(value.to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
    } else if let Some(url) = runtime_cloud_target_control_url(
        ctx.discovery.as_ref(),
        target_runtime.as_str(),
        ctx.profile.requires_secure_transport(),
    ) {
        let agent_config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_millis(500)))
            .timeout_recv_response(Some(Duration::from_millis(1500)))
            .http_status_as_error(false)
            .build();
        let agent: ureq::Agent = agent_config.into();
        let mut remote = agent
            .post(url.as_str())
            .header("Content-Type", "application/json");
        if let Some(token) = request_token.as_deref() {
            remote = remote.header("X-Trust-Token", token);
        }
        match remote.send(control_payload.to_string()) {
            Ok(mut remote_response) => {
                let status = remote_response.status().as_u16();
                let text = remote_response
                    .body_mut()
                    .read_to_string()
                    .unwrap_or_default();
                let value = serde_json::from_str::<serde_json::Value>(&text)
                    .unwrap_or_else(|_| json!({ "ok": false, "error": "invalid remote response" }));
                if (200..300).contains(&status) {
                    Response::from_string(value.to_string()).with_header(
                        Header::from_bytes("Content-Type", "application/json").unwrap(),
                    )
                } else {
                    let mut body = value;
                    body["ok"] = serde_json::Value::Bool(false);
                    if body.get("denial_code").is_none() {
                        body["denial_code"] = serde_json::to_value(
                            runtime_cloud_map_remote_http_status(status, action_type),
                        )
                        .unwrap_or(serde_json::Value::String("transport_failure".to_string()));
                    }
                    if body.get("error").is_none() {
                        body["error"] = serde_json::Value::String(format!("http status {status}"));
                    }
                    Response::from_string(body.to_string())
                        .with_status_code(StatusCode(status))
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        )
                }
            }
            Err(error) => Response::from_string(
                json!({
                    "ok": false,
                    "denial_code": ReasonCode::TargetUnreachable,
                    "error": error.to_string(),
                })
                .to_string(),
            )
            .with_status_code(StatusCode(503))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
        }
    } else {
        Response::from_string(
            json!({
                "ok": false,
                "denial_code": ReasonCode::TargetUnreachable,
                "error": format!("target runtime '{}' is not reachable", target_runtime),
            })
            .to_string(),
        )
        .with_status_code(StatusCode(503))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
    };

    let _ = request.respond(response);
}

fn validate_proxy_payload(
    payload: &RuntimeCloudControlProxyRequest,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
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
            return Some(response);
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
            return Some(response);
        }
    }
    if payload.actor.trim().is_empty() {
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": ReasonCode::ContractViolation,
                "error": "actor must not be empty",
            })
            .to_string(),
        )
        .with_status_code(StatusCode(400))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        return Some(response);
    }
    if payload.target_runtime.trim().is_empty() {
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": ReasonCode::ContractViolation,
                "error": "target_runtime must not be empty",
            })
            .to_string(),
        )
        .with_status_code(StatusCode(400))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        return Some(response);
    }
    if payload.control_request.r#type.trim().is_empty() {
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": ReasonCode::ContractViolation,
                "error": "control_request.type must not be empty",
            })
            .to_string(),
        )
        .with_status_code(StatusCode(400))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        return Some(response);
    }
    None
}

fn proxy_action_type(kind: &str, required_role: AccessRole) -> &'static str {
    if kind == "config.set" {
        return "cfg_apply";
    }
    if required_role == AccessRole::Viewer {
        return "status_read";
    }
    "cmd_invoke"
}

fn proxy_action_payload(
    action_type: &str,
    kind: &str,
    params: Option<&serde_json::Value>,
) -> serde_json::Value {
    if action_type == "cfg_apply" {
        let config_params = params.cloned().unwrap_or_else(|| json!({}));
        return json!({ "params": config_params });
    }
    if action_type == "status_read" {
        return json!({});
    }
    let mut payload = json!({
        "command": kind,
    });
    if let Some(params) = params {
        payload["params"] = params.clone();
    }
    payload
}

fn proxy_control_payload(
    kind: &str,
    params: Option<&serde_json::Value>,
    request_id: &str,
) -> serde_json::Value {
    let mut payload = json!({
        "id": 1_u64,
        "type": kind,
        "request_id": request_id,
    });
    if let Some(params) = params {
        payload["params"] = params.clone();
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_action_type_uses_status_read_for_viewer() {
        assert_eq!(
            proxy_action_type("status", AccessRole::Viewer),
            "status_read"
        );
    }

    #[test]
    fn proxy_action_type_uses_cfg_apply_for_config_set() {
        assert_eq!(
            proxy_action_type("config.set", AccessRole::Engineer),
            "cfg_apply"
        );
    }

    #[test]
    fn proxy_control_payload_keeps_request_shape() {
        let payload = proxy_control_payload("events", Some(&json!({ "limit": 20 })), "proxy-1");
        assert_eq!(payload["type"], json!("events"));
        assert_eq!(payload["request_id"], json!("proxy-1"));
        assert_eq!(payload["params"]["limit"], json!(20));
    }
}
