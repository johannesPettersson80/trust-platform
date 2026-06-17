//! Auxiliary non-IDE/non-runtime-cloud route handlers for the web server loop.

#![allow(missing_docs)]

use super::*;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;

pub(super) struct AuxRouteContext<'a> {
    pub mode: WebServerMode,
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub web_tls_enabled: bool,
    pub control_state: &'a Arc<ControlState>,
    pub discovery: &'a Arc<DiscoveryState>,
    pub bundle_root: &'a Option<PathBuf>,
}

pub(super) enum AuxRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_aux_route(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: AuxRouteContext<'_>,
) -> AuxRouteOutcome {
    if *method == Method::Get && url == "/api/ui/mode" {
        let body = json!({
            "ok": true,
            "mode": ctx.mode.as_str(),
        })
        .to_string();
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }

    if *method == Method::Get
        && ctx
            .control_state
            .historian
            .as_ref()
            .and_then(|hist| hist.prometheus_path())
            == Some(url)
    {
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
                return AuxRouteOutcome::Handled;
            }
        };
        let metrics = ctx
            .control_state
            .metrics
            .lock()
            .ok()
            .map(|guard| guard.snapshot())
            .unwrap_or_default();
        let body = ctx
            .control_state
            .historian
            .as_ref()
            .map(|service| service.render_prometheus(&metrics))
            .unwrap_or_else(|| crate::historian::render_prometheus(&metrics, None));
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "text/plain; version=0.0.4").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Get && url.starts_with("/api/qr") {
        let text = url.split('?').nth(1).and_then(|query| {
            query.split('&').find_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                if parts.next()? == "text" {
                    Some(parts.next().unwrap_or("").to_string())
                } else {
                    None
                }
            })
        });
        if let Some(encoded) = text {
            let decoded = urlencoding::decode(&encoded).unwrap_or_else(|_| encoded.as_str().into());
            match render_qr_svg(decoded.as_ref()) {
                Ok(svg) => {
                    let response = Response::from_string(svg)
                        .with_header(Header::from_bytes("Content-Type", "image/svg+xml").unwrap());
                    let _ = request.respond(response);
                }
                Err(err) => {
                    let response =
                        Response::from_string(format!("error: {err}")).with_status_code(500);
                    let _ = request.respond(response);
                }
            }
        } else {
            let response = Response::from_string("missing text").with_status_code(400);
            let _ = request.respond(response);
        }
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/discovery" {
        let items = ctx
            .discovery
            .snapshot()
            .into_iter()
            .map(|entry| {
                json!({
                    "id": entry.id.as_str(),
                    "name": entry.name.as_str(),
                    "addresses": entry.addresses,
                    "web_port": entry.web_port,
                    "web_tls": entry.web_tls,
                    "mesh_port": entry.mesh_port,
                    "control": entry.control.as_ref().map(|v| v.as_str()),
                    "host_group": entry.host_group.as_ref().map(|v| v.as_str()),
                    "last_seen_ns": entry.last_seen_ns,
                })
            })
            .collect::<Vec<_>>();
        let body = json!({ "items": items }).to_string();
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Get && url.starts_with("/api/probe") {
        let target = query_value(url, "url");
        let target = match target {
            Some(value) if value.starts_with("http://") || value.starts_with("https://") => {
                value.trim_end_matches('/').to_string()
            }
            _ => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": "invalid url" }).to_string(),
                )
                .with_status_code(400);
                let _ = request.respond(response);
                return AuxRouteOutcome::Handled;
            }
        };
        let control_url = format!("{target}/api/control");
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_millis(500)))
            .timeout_recv_response(Some(Duration::from_millis(800)))
            .http_status_as_error(true)
            .build();
        let agent: ureq::Agent = config.into();
        let body = json!({ "id": 1u64, "type": "status" }).to_string();
        let mut probe_request = agent
            .post(&control_url)
            .header("Content-Type", "application/json");
        if let Some(value) = query_value(url, "username")
            .map(|entry| entry.trim().to_string())
            .filter(|entry| !entry.is_empty())
        {
            let password = query_value(url, "password").unwrap_or_default();
            let auth = STANDARD.encode(format!("{value}:{password}"));
            probe_request = probe_request.header("Authorization", &format!("Basic {auth}"));
        }
        let response_body = probe_request.send(body.as_str());
        let payload = match response_body {
            Ok(mut resp) => {
                let text = resp.body_mut().read_to_string().unwrap_or_default();
                parse_probe_response(&text)
            }
            Err(ureq::Error::StatusCode(401)) => {
                json!({ "ok": false, "error": "auth_required" })
            }
            Err(_) => json!({ "ok": false, "error": "unreachable" }),
        };
        let response = Response::from_string(payload.to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/control/proxy" {
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
                return AuxRouteOutcome::Handled;
            }
        };
        if let Err(response) = api_post_policy_check(&request, ctx.web_tls_enabled, true) {
            let _ = request.respond(response);
            return AuxRouteOutcome::Handled;
        }
        let payload: ControlProxyRequest =
            match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
                Ok(value) => value,
                Err(error) => {
                    let _ = request.respond(json_body_error_response(error));
                    return AuxRouteOutcome::Handled;
                }
            };
        if payload
            .control_request
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            let _ = request.respond(aux_json_response(
                400,
                json!({
                    "ok": false,
                    "error": "control_request.type is required",
                }),
            ));
            return AuxRouteOutcome::Handled;
        }
        let (status, response_body) = match proxy_remote_post(
            payload.target.as_str(),
            "/api/control",
            request_token.as_deref(),
            payload.auth_basic.as_ref(),
            &payload.control_request,
            500,
            2_500,
        ) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(aux_json_response(
                    503,
                    json!({
                        "ok": false,
                        "error": error.to_string(),
                    }),
                ));
                return AuxRouteOutcome::Handled;
            }
        };
        let _ = request.respond(aux_json_response(
            status,
            normalize_proxy_response(status, response_body),
        ));
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/deploy/proxy" {
        let request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Admin,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return AuxRouteOutcome::Handled;
            }
        };
        if let Err(response) = api_post_policy_check(&request, ctx.web_tls_enabled, true) {
            let _ = request.respond(response);
            return AuxRouteOutcome::Handled;
        }
        let payload: DeployProxyRequest = match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES)
        {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return AuxRouteOutcome::Handled;
            }
        };
        let (status, response_body) = match proxy_remote_post(
            payload.target.as_str(),
            "/api/deploy",
            request_token.as_deref(),
            payload.auth_basic.as_ref(),
            &payload.deploy_request,
            500,
            30_000,
        ) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(aux_json_response(
                    503,
                    json!({
                        "ok": false,
                        "error": error.to_string(),
                    }),
                ));
                return AuxRouteOutcome::Handled;
            }
        };
        let _ = request.respond(aux_json_response(
            status,
            normalize_proxy_response(status, response_body),
        ));
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/setup/defaults" {
        let defaults = setup_defaults(ctx.bundle_root);
        let body = serde_json::to_string(&defaults).unwrap_or_else(|_| "{}".to_string());
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/setup/apply" {
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
                return AuxRouteOutcome::Handled;
            }
        };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response = Response::from_string("invalid body").with_status_code(400);
            let _ = request.respond(response);
            return AuxRouteOutcome::Handled;
        }
        let payload: SetupApplyRequest = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string("invalid json").with_status_code(400);
                let _ = request.respond(response);
                return AuxRouteOutcome::Handled;
            }
        };
        let response_body = match apply_setup(ctx.bundle_root, payload) {
            Ok(message) => message,
            Err(err) => format!("error: {err}"),
        };
        let response = Response::from_string(response_body)
            .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/io/config" {
        let body = match load_io_config(ctx.bundle_root) {
            Ok(config) => serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string()),
            Err(err) => json!({ "error": err.to_string() }).to_string(),
        };
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
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
                return AuxRouteOutcome::Handled;
            }
        };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response = Response::from_string("invalid body").with_status_code(400);
            let _ = request.respond(response);
            return AuxRouteOutcome::Handled;
        }
        let payload: IoConfigRequest = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string("invalid json").with_status_code(400);
                let _ = request.respond(response);
                return AuxRouteOutcome::Handled;
            }
        };
        let response_body = match save_io_config(ctx.bundle_root, &payload) {
            Ok(message) => message,
            Err(err) => format!("error: {err}"),
        };
        let response = Response::from_string(response_body)
            .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/io/modbus-test" {
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
                return AuxRouteOutcome::Handled;
            }
        };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response = Response::from_string("invalid body").with_status_code(400);
            let _ = request.respond(response);
            return AuxRouteOutcome::Handled;
        }
        let payload: serde_json::Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string("invalid json").with_status_code(400);
                let _ = request.respond(response);
                return AuxRouteOutcome::Handled;
            }
        };
        let (status, body) = io_probe_legacy_body("modbus_tcp", payload);
        let response = Response::from_string(body)
            .with_status_code(status)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/io/mqtt-test" {
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
                return AuxRouteOutcome::Handled;
            }
        };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response = Response::from_string("invalid body").with_status_code(400);
            let _ = request.respond(response);
            return AuxRouteOutcome::Handled;
        }
        let payload: serde_json::Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string("invalid json").with_status_code(400);
                let _ = request.respond(response);
                return AuxRouteOutcome::Handled;
            }
        };
        let (status, body) = io_probe_legacy_body("mqtt", payload);
        let response = Response::from_string(body)
            .with_status_code(status)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return AuxRouteOutcome::Handled;
    }

    AuxRouteOutcome::NotHandled(request)
}

fn io_probe_legacy_body(protocol: &str, mut payload: serde_json::Value) -> (u16, String) {
    if protocol == "modbus_tcp" {
        if let Some(address) = payload
            .get("address")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|address| !address.is_empty() && !address.contains(':'))
            .map(ToOwned::to_owned)
        {
            let port = payload
                .get("port")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(502);
            payload["address"] = json!(format!("{address}:{port}"));
        }
    }

    let probe = crate::control::communication_probe_value_port(protocol, payload);
    let ok = probe
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if ok {
        return (200, json!({ "ok": true, "probe": probe }).to_string());
    }
    let status = if probe_has_input_errors(&probe) {
        400
    } else {
        200
    };
    (
        status,
        json!({
        "ok": false,
        "error": probe_error_message(&probe),
        "probe": probe,
        })
        .to_string(),
    )
}

fn probe_has_input_errors(probe: &serde_json::Value) -> bool {
    let has_field_errors = probe
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| !errors.is_empty());
    let has_runtime_evidence = probe.get("evidence").is_some_and(|value| !value.is_null());
    has_field_errors && !has_runtime_evidence
}

fn probe_error_message(probe: &serde_json::Value) -> String {
    probe
        .get("field_errors")
        .and_then(serde_json::Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| probe.get("detail").and_then(serde_json::Value::as_str))
        .unwrap_or("Connection test failed.")
        .to_string()
}

#[derive(Debug, Deserialize)]
struct ControlProxyRequest {
    target: String,
    control_request: serde_json::Value,
    #[serde(default)]
    auth_basic: Option<ProxyBasicAuth>,
}

#[derive(Debug, Deserialize)]
struct DeployProxyRequest {
    target: String,
    deploy_request: serde_json::Value,
    #[serde(default)]
    auth_basic: Option<ProxyBasicAuth>,
}

#[derive(Debug, Deserialize)]
struct ProxyBasicAuth {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

impl ProxyBasicAuth {
    fn authorization_header(&self) -> Option<String> {
        let username = self.username.trim();
        if username.is_empty() {
            return None;
        }
        let encoded = STANDARD.encode(format!("{username}:{}", self.password));
        Some(format!("Basic {encoded}"))
    }
}

fn normalize_proxy_target(raw: &str) -> Result<String, RuntimeError> {
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

fn proxy_remote_post(
    target: &str,
    path: &str,
    request_token: Option<&str>,
    auth_basic: Option<&ProxyBasicAuth>,
    payload: &serde_json::Value,
    connect_timeout_ms: u64,
    recv_timeout_ms: u64,
) -> Result<(u16, serde_json::Value), RuntimeError> {
    let target = normalize_proxy_target(target)?;
    let url = format!("{target}{path}");
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_millis(connect_timeout_ms)))
        .timeout_recv_response(Some(Duration::from_millis(recv_timeout_ms)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.post(&url).header("Content-Type", "application/json");
    if let Some(token) = request_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header("X-Trust-Token", token);
    }
    if let Some(header_value) = auth_basic.and_then(ProxyBasicAuth::authorization_header) {
        request = request.header("Authorization", &header_value);
    }
    let mut response = request.send(payload.to_string()).map_err(|error| {
        RuntimeError::ControlError(format!("proxy request failed: {error}").into())
    })?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    let body = serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|_| {
        if text.trim().is_empty() {
            json!({})
        } else {
            json!({
                "ok": false,
                "error": "invalid remote response",
            })
        }
    });
    Ok((status, body))
}

fn normalize_proxy_response(status: u16, body: serde_json::Value) -> serde_json::Value {
    let success = (200..300).contains(&status);
    if let serde_json::Value::Object(mut object) = body {
        let has_ok = object
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .is_some();
        if success {
            if has_ok {
                return serde_json::Value::Object(object);
            }
            return json!({
                "ok": true,
                "result": serde_json::Value::Object(object),
            });
        }
        object.insert("ok".to_string(), serde_json::Value::Bool(false));
        if !object.contains_key("error") {
            object.insert(
                "error".to_string(),
                serde_json::Value::String(format!("http status {status}")),
            );
        }
        return serde_json::Value::Object(object);
    }
    if success {
        json!({
            "ok": true,
            "result": body,
        })
    } else {
        json!({
            "ok": false,
            "error": format!("http status {status}"),
        })
    }
}

fn aux_json_response(status: u16, body: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}
