use super::*;
use crate::control::control_request_required_role_port;
use crate::runtime_cloud::control_proxy_policy::{
    runtime_cloud_control_proxy_plan, runtime_cloud_control_proxy_role_denial,
    RuntimeCloudControlProxyPlan,
};

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

    let kind = payload.control_request.r#type.trim();
    let params = payload.control_request.params.clone();
    let required_role = control_request_required_role_port(kind, params.as_ref());
    let local_runtime = local_runtime_id(ctx);
    let plan = match runtime_cloud_control_proxy_plan(
        payload.api_version.as_str(),
        payload.actor.as_str(),
        payload.target_runtime.as_str(),
        kind,
        params.as_ref(),
        payload.control_request.request_id.as_deref(),
        local_runtime.as_str(),
        required_role,
        now_ns(),
    ) {
        Ok(plan) => plan,
        Err(error) => {
            let response = Response::from_string(
                json!({
                    "ok": false,
                    "denial_code": error.code,
                    "error": error.message,
                })
                .to_string(),
            )
            .with_status_code(StatusCode(400))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return;
        }
    };
    if !web_role.allows(required_role) {
        let denial = runtime_cloud_control_proxy_role_denial(web_role, required_role, kind);
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": denial.code,
                "error": denial.message,
            })
            .to_string(),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    let RuntimeCloudControlProxyPlan {
        target_runtime,
        action_type,
        action,
        control_payload,
    } = plan;
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

    let response = if target_runtime == local_runtime {
        let control_response = dispatch_control_request(
            control_payload,
            ctx.control_state,
            Some("runtime-cloud-proxy"),
            request_token.as_deref(),
        );
        let mut value = match serde_json::to_value(&control_response) {
            Ok(value) => value,
            Err(err) => {
                let response = Response::from_string(
                    json!({
                        "ok": false,
                        "denial_code": ReasonCode::TransportFailure,
                        "error": format!("control proxy serialization failed: {err}"),
                    })
                    .to_string(),
                )
                .with_status_code(StatusCode(502))
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                return;
            }
        };
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
                action_type.as_str(),
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
                            runtime_cloud_map_remote_http_status(status, action_type.as_str()),
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
