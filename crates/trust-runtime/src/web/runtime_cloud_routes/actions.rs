use super::*;

pub(super) fn handle_post_preflight(
    mut request: tiny_http::Request,
    ctx: &RuntimeCloudRouteContext<'_>,
) {
    let (web_role, _request_token) = match check_auth_with_role(
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
    let action: RuntimeCloudActionRequest =
        match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return;
            }
        };
    let local_runtime = local_runtime_id(ctx);
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
    let response = Response::from_string(
        serde_json::to_string(&preflight).unwrap_or_else(|_| "{}".to_string()),
    )
    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}

pub(super) fn handle_post_dispatch(
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
    let action: RuntimeCloudActionRequest =
        match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return;
            }
        };
    let local_runtime = local_runtime_id(ctx);
    let (preflight, ha_request, _known_targets) = runtime_cloud_preflight_for_action(
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

    if !preflight.allowed || action.dry_run {
        let report = RuntimeCloudDispatchResponse {
            api_version: preflight.api_version.clone(),
            request_id: preflight.request_id.clone(),
            connected_via: preflight.connected_via.clone(),
            acting_on: preflight.acting_on.clone(),
            dry_run: action.dry_run,
            ok: preflight.allowed && action.dry_run,
            preflight: preflight.clone(),
            results: runtime_cloud_denied_results(&preflight),
        };
        let response = Response::from_string(
            serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string()),
        )
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }

    let control_payload = match map_action_to_control_request(&action) {
        Ok(payload) => payload,
        Err((denial_code, denial_reason)) => {
            let results = preflight
                .acting_on
                .iter()
                .map(|runtime_id| RuntimeCloudDispatchTargetResult {
                    runtime_id: runtime_id.clone(),
                    ok: false,
                    denial_code: Some(denial_code),
                    denial_reason: Some(denial_reason.clone()),
                    audit_id: None,
                    response: None,
                })
                .collect::<Vec<_>>();
            let report = RuntimeCloudDispatchResponse {
                api_version: preflight.api_version.clone(),
                request_id: preflight.request_id.clone(),
                connected_via: preflight.connected_via.clone(),
                acting_on: preflight.acting_on.clone(),
                dry_run: action.dry_run,
                ok: false,
                preflight: preflight.clone(),
                results,
            };
            let response = Response::from_string(
                serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string()),
            )
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return;
        }
    };

    let mut results = Vec::new();
    let payload_text = control_payload.to_string();
    let dispatch_budget_ms = action.query_budget_ms.unwrap_or(1_500).min(10_000);
    let dispatch_deadline = std::time::Instant::now() + Duration::from_millis(dispatch_budget_ms);
    let agent_config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_millis(500)))
        .timeout_recv_response(Some(Duration::from_millis(1500)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = agent_config.into();
    for decision in &preflight.decisions {
        if !decision.allowed {
            results.push(RuntimeCloudDispatchTargetResult {
                runtime_id: decision.runtime_id.clone(),
                ok: false,
                denial_code: decision.denial_code,
                denial_reason: decision.denial_reason.clone(),
                audit_id: None,
                response: None,
            });
            continue;
        }
        if std::time::Instant::now() >= dispatch_deadline {
            results.push(RuntimeCloudDispatchTargetResult {
                runtime_id: decision.runtime_id.clone(),
                ok: false,
                denial_code: Some(ReasonCode::Timeout),
                denial_reason: Some(format!(
                    "dispatch query budget {} ms exhausted; remaining targets cancelled",
                    dispatch_budget_ms
                )),
                audit_id: None,
                response: None,
            });
            continue;
        }

        let mut ha_ticket: Option<RuntimeCloudHaDispatchTicket> = None;
        if let Some(ha_request) = ha_request.as_ref() {
            let gate = match ctx.ha_state.lock() {
                Ok(mut coordinator) => coordinator.begin_dispatch(
                    action.action_type.as_str(),
                    action.request_id.as_str(),
                    decision.runtime_id.as_str(),
                    ha_request,
                ),
                Err(_) => Some(RuntimeCloudHaDispatchGate::Denied(
                    RuntimeCloudHaDecision::deny(
                        ReasonCode::TransportFailure,
                        "runtime cloud HA state is unavailable",
                    ),
                )),
            };
            match gate {
                Some(RuntimeCloudHaDispatchGate::Denied(denial)) => {
                    results.push(RuntimeCloudDispatchTargetResult {
                        runtime_id: decision.runtime_id.clone(),
                        ok: false,
                        denial_code: denial.denial_code,
                        denial_reason: denial.denial_reason,
                        audit_id: None,
                        response: None,
                    });
                    continue;
                }
                Some(RuntimeCloudHaDispatchGate::Deduplicated(record)) => {
                    results.push(RuntimeCloudDispatchTargetResult {
                        runtime_id: decision.runtime_id.clone(),
                        ok: record.ok,
                        denial_code: record.denial_code,
                        denial_reason: record.denial_reason,
                        audit_id: record.audit_id,
                        response: record.response,
                    });
                    continue;
                }
                Some(RuntimeCloudHaDispatchGate::Proceed(ticket)) => {
                    ha_ticket = Some(ticket);
                }
                None => {}
            }
        }

        let target_result = if decision.runtime_id == local_runtime {
            let control_response = dispatch_control_request(
                control_payload.clone(),
                ctx.control_state,
                Some("runtime-cloud"),
                request_token.as_deref(),
            );
            let response_value = serde_json::to_value(&control_response)
                .unwrap_or_else(|_| json!({ "ok": false, "error": "serialize error" }));
            let ok = response_value
                .get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let (denial_code, denial_reason) = if ok {
                (None, None)
            } else {
                let reason = response_value
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("dispatch failed")
                    .to_string();
                (
                    Some(runtime_cloud_map_control_error(
                        reason.as_str(),
                        action.action_type.as_str(),
                    )),
                    Some(reason),
                )
            };
            let audit_id = runtime_cloud_extract_audit_id(&response_value);
            RuntimeCloudDispatchTargetResult {
                runtime_id: decision.runtime_id.clone(),
                ok,
                denial_code,
                denial_reason,
                audit_id,
                response: Some(response_value),
            }
        } else if let Some(url) = runtime_cloud_target_control_url(
            ctx.discovery.as_ref(),
            decision.runtime_id.as_str(),
            ctx.profile.requires_secure_transport(),
        ) {
            let mut remote = agent.post(&url).header("Content-Type", "application/json");
            if let Some(token) = request_token.as_deref() {
                remote = remote.header("X-Trust-Token", token);
            }
            match remote.send(payload_text.as_str()) {
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    let text = response.body_mut().read_to_string().unwrap_or_default();
                    let value = serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(
                        |_| json!({ "ok": false, "error": "invalid remote response" }),
                    );
                    if (200..300).contains(&status) {
                        let ok = value
                            .get("ok")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        let (denial_code, denial_reason) = if ok {
                            (None, None)
                        } else {
                            let reason = value
                                .get("error")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("remote dispatch failed")
                                .to_string();
                            (
                                Some(runtime_cloud_map_control_error(
                                    reason.as_str(),
                                    action.action_type.as_str(),
                                )),
                                Some(reason),
                            )
                        };
                        let audit_id = runtime_cloud_extract_audit_id(&value);
                        RuntimeCloudDispatchTargetResult {
                            runtime_id: decision.runtime_id.clone(),
                            ok,
                            denial_code,
                            denial_reason,
                            audit_id,
                            response: Some(value),
                        }
                    } else {
                        let error = value
                            .get("error")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("http status {status}"));
                        RuntimeCloudDispatchTargetResult {
                            runtime_id: decision.runtime_id.clone(),
                            ok: false,
                            denial_code: Some(runtime_cloud_map_remote_http_status(
                                status,
                                action.action_type.as_str(),
                            )),
                            denial_reason: Some(error),
                            audit_id: runtime_cloud_extract_audit_id(&value),
                            response: Some(value),
                        }
                    }
                }
                Err(error) => RuntimeCloudDispatchTargetResult {
                    runtime_id: decision.runtime_id.clone(),
                    ok: false,
                    denial_code: Some(
                        crate::runtime_cloud::contracts::ReasonCode::TargetUnreachable,
                    ),
                    denial_reason: Some(error.to_string()),
                    audit_id: None,
                    response: None,
                },
            }
        } else {
            RuntimeCloudDispatchTargetResult {
                runtime_id: decision.runtime_id.clone(),
                ok: false,
                denial_code: Some(crate::runtime_cloud::contracts::ReasonCode::TargetUnreachable),
                denial_reason: Some(format!(
                    "target runtime '{}' is not reachable",
                    decision.runtime_id
                )),
                audit_id: None,
                response: None,
            }
        };

        if let Some(ticket) = ha_ticket {
            if let Ok(mut coordinator) = ctx.ha_state.lock() {
                coordinator
                    .finish_dispatch(ticket, runtime_cloud_ha_record_from_result(&target_result));
            }
        }
        results.push(target_result);
    }

    let ok = results.iter().all(|result| result.ok);
    let report = RuntimeCloudDispatchResponse {
        api_version: preflight.api_version.clone(),
        request_id: preflight.request_id.clone(),
        connected_via: preflight.connected_via.clone(),
        acting_on: preflight.acting_on.clone(),
        dry_run: action.dry_run,
        ok,
        preflight,
        results,
    };
    let response =
        Response::from_string(serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string()))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}
