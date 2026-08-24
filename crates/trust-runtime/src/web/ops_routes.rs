//! Operational HTTP route handlers for pairing/deploy/control endpoints.

#![allow(missing_docs)]

use super::*;

pub(super) struct OpsRouteContext<'a> {
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub web_url: &'a str,
    pub control_state: &'a Arc<ControlState>,
    pub bundle_root: &'a Option<PathBuf>,
    pub web_tls_enabled: bool,
}

pub(super) enum OpsRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_ops_route(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: OpsRouteContext<'_>,
) -> OpsRouteOutcome {
    let auth = ctx.auth_mode;
    let auth_token = ctx.auth_token;
    let pairing = ctx.pairing;
    let web_url = ctx.web_url;
    let control_state = ctx.control_state;
    let bundle_root = ctx.bundle_root;
    let web_tls_enabled = ctx.web_tls_enabled;
    if *method == Method::Get && url == "/api/pairings" {
        let _request_token =
            match check_auth(&request, auth, auth_token, pairing, AccessRole::Admin) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return OpsRouteOutcome::Handled;
                }
            };
        let list = pairing
            .as_ref()
            .map(|store| store.list())
            .unwrap_or_default();
        let body = json!({ "items": list }).to_string();
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/pair/start" {
        let _request_token =
            match check_auth(&request, auth, auth_token, pairing, AccessRole::Admin) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return OpsRouteOutcome::Handled;
                }
            };
        let body = if let Some(store) = pairing.as_ref() {
            let code = store.start_pairing();
            json!({
                "code": code.code,
                "expires_at": code.expires_at,
            })
        } else {
            json!({ "error": "pairing unavailable" })
        };
        let response = Response::from_string(body.to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/pair/claim" {
        let payload: serde_json::Value = match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES)
        {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return OpsRouteOutcome::Handled;
            }
        };
        let code = payload.get("code").and_then(|value| value.as_str());
        let requested_role = payload
            .get("role")
            .and_then(|value| value.as_str())
            .and_then(AccessRole::parse);
        let token = code.and_then(|value| {
            pairing
                .as_ref()
                .and_then(|store| store.claim(value, requested_role))
        });
        let body = if let Some(token) = token {
            json!({ "token": token })
        } else {
            json!({ "error": "invalid code" })
        };
        let response = Response::from_string(body.to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/invite" {
        let _request_token =
            match check_auth(&request, auth, auth_token, pairing, AccessRole::Admin) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return OpsRouteOutcome::Handled;
                }
            };
        let token = auth_token
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|value| value.to_string()));
        let body = json!({
            "endpoint": web_url,
            "token": token,
        })
        .to_string();
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Get && url.starts_with("/api/events") {
        let request_token =
            match check_auth(&request, auth, auth_token, pairing, AccessRole::Viewer) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return OpsRouteOutcome::Handled;
                }
            };
        let limit = parse_limit(url).unwrap_or(50);
        let response = dispatch_control_request(
            json!({ "id": 1, "type": "events.tail", "params": { "limit": limit } }),
            control_state,
            Some("web"),
            request_token.as_deref(),
        );
        let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Get && url.starts_with("/api/faults") {
        let request_token =
            match check_auth(&request, auth, auth_token, pairing, AccessRole::Viewer) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return OpsRouteOutcome::Handled;
                }
            };
        let limit = parse_limit(url).unwrap_or(50);
        let response = dispatch_control_request(
            json!({ "id": 1, "type": "faults", "params": { "limit": limit } }),
            control_state,
            Some("web"),
            request_token.as_deref(),
        );
        let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
        let response = Response::from_string(body)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/deploy" {
        let request_token = match check_auth(&request, auth, auth_token, pairing, AccessRole::Admin)
        {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return OpsRouteOutcome::Handled;
            }
        };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response =
                Response::from_string(json!({ "ok": false, "error": "invalid body" }).to_string())
                    .with_status_code(StatusCode(400));
            let _ = request.respond(response);
            return OpsRouteOutcome::Handled;
        }
        let payload: DeployRequest = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": "invalid json" }).to_string(),
                )
                .with_status_code(StatusCode(400));
                let _ = request.respond(response);
                return OpsRouteOutcome::Handled;
            }
        };
        let Some(bundle_root) = bundle_root.as_ref() else {
            let response = Response::from_string(
                json!({ "ok": false, "error": "project folder unavailable" }).to_string(),
            )
            .with_status_code(StatusCode(400));
            let _ = request.respond(response);
            return OpsRouteOutcome::Handled;
        };
        let result = apply_deploy(bundle_root, payload);
        let body = match result {
            Ok(result) => {
                if let Some(restart) = result.restart.as_ref() {
                    let _ = dispatch_control_request(
                        json!({ "id": 1, "type": "restart", "params": { "mode": restart } }),
                        control_state,
                        Some("web"),
                        request_token.as_deref(),
                    );
                }
                json!({ "ok": true, "written": result.written, "restart": result.restart })
            }
            Err(err) => json!({ "ok": false, "error": err.to_string() }),
        };
        let response = Response::from_string(body.to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/rollback" {
        let request_token = match check_auth(&request, auth, auth_token, pairing, AccessRole::Admin)
        {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return OpsRouteOutcome::Handled;
            }
        };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response =
                Response::from_string(json!({ "ok": false, "error": "invalid body" }).to_string())
                    .with_status_code(StatusCode(400));
            let _ = request.respond(response);
            return OpsRouteOutcome::Handled;
        }
        let payload: RollbackRequest =
            serde_json::from_str(&body).unwrap_or(RollbackRequest { restart: None });
        let Some(bundle_root) = bundle_root.as_ref() else {
            let response = Response::from_string(
                json!({ "ok": false, "error": "project folder unavailable" }).to_string(),
            )
            .with_status_code(StatusCode(400));
            let _ = request.respond(response);
            return OpsRouteOutcome::Handled;
        };
        let root = bundle_root
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| bundle_root.clone());
        let result = apply_rollback(&root);
        let body = match result {
            Ok(result) => {
                if let Some(restart) = payload.restart.as_ref() {
                    let _ = dispatch_control_request(
                        json!({ "id": 1, "type": "restart", "params": { "mode": restart } }),
                        control_state,
                        Some("web"),
                        request_token.as_deref(),
                    );
                }
                json!({
                    "ok": true,
                    "current": result.current.display().to_string(),
                    "previous": result.previous.display().to_string(),
                })
            }
            Err(err) => json!({ "ok": false, "error": err.to_string() }),
        };
        let response = Response::from_string(body.to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return OpsRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/control" {
        let request_token =
            match check_auth(&request, auth, auth_token, pairing, AccessRole::Viewer) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return OpsRouteOutcome::Handled;
                }
            };
        if let Err(response) = api_post_policy_check(&request, web_tls_enabled, true) {
            let _ = request.respond(response);
            return OpsRouteOutcome::Handled;
        }
        let payload: serde_json::Value = match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES)
        {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return OpsRouteOutcome::Handled;
            }
        };
        let control_response = prepare_control_request(
            payload,
            control_state,
            Some("web"),
            request_token.as_deref(),
        );
        let mut control_responses = [control_response];
        let _ = write_then_complete_control_requests(
            &mut control_responses,
            control_state,
            |control_responses| {
                let body =
                    serde_json::to_string(&control_responses[0]).unwrap_or_else(|_| "{}".into());
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                request.respond(response)
            },
        );
        return OpsRouteOutcome::Handled;
    }
    OpsRouteOutcome::NotHandled(request)
}
