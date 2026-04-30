//! Authentication, role resolution, and control dispatch helper functions.

#![allow(missing_docs)]

use super::*;

pub(super) fn check_auth(
    request: &tiny_http::Request,
    auth_mode: WebAuthMode,
    token: &Arc<Mutex<Option<smol_str::SmolStr>>>,
    pairing: Option<&PairingStore>,
    required_role: AccessRole,
) -> Result<Option<String>, &'static str> {
    check_auth_with_role(request, auth_mode, token, pairing, required_role)
        .map(|(_role, request_token)| request_token)
}

pub(super) fn check_auth_with_role(
    request: &tiny_http::Request,
    auth_mode: WebAuthMode,
    token: &Arc<Mutex<Option<smol_str::SmolStr>>>,
    pairing: Option<&PairingStore>,
    required_role: AccessRole,
) -> Result<(AccessRole, Option<String>), &'static str> {
    let Some((role, request_token)) = resolve_web_role(request, auth_mode, token, pairing) else {
        return Err("unauthorized");
    };
    if !role.allows(required_role) {
        return Err("forbidden");
    }
    Ok((role, request_token))
}

fn resolve_web_role(
    request: &tiny_http::Request,
    auth_mode: WebAuthMode,
    token: &Arc<Mutex<Option<smol_str::SmolStr>>>,
    pairing: Option<&PairingStore>,
) -> Option<(AccessRole, Option<String>)> {
    if matches!(auth_mode, WebAuthMode::Local) {
        return Some((AccessRole::Admin, None));
    }
    let expected = token.lock().ok().and_then(|guard| guard.as_ref().cloned());
    let header = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Trust-Token"))
        .map(|header| header.value.as_str().to_string());
    if let Some(expected) = expected {
        if let Some(provided) = header.as_deref() {
            if constant_time_eq(expected.as_str(), provided) {
                return Some((AccessRole::Admin, header));
            }
        }
    }
    let header = header?;
    pairing
        .as_ref()
        .and_then(|store| store.validate_with_role(header.as_str()))
        .map(|role| (role, Some(header)))
}

pub(super) fn auth_error_response(error: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let status = if error == "forbidden" {
        StatusCode(403)
    } else {
        StatusCode(401)
    };
    Response::from_string(json!({ "ok": false, "error": error }).to_string())
        .with_status_code(status)
}

pub(super) fn dispatch_control_request(
    payload: serde_json::Value,
    control_state: &ControlState,
    client: Option<&str>,
    request_token: Option<&str>,
) -> crate::control::ControlResponse {
    crate::control::dispatch_web_control_request_port(payload, control_state, client, request_token)
}

pub(super) fn ide_session_token(request: &tiny_http::Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Trust-Ide-Session"))
        .map(|header| header.value.as_str().to_string())
}

pub(super) fn ide_write_enabled(_control_state: &ControlState) -> bool {
    true
}

pub(super) fn ide_error_response(error: IdeError) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut payload = json!({ "ok": false, "error": error.to_string() });
    if let Some(version) = error.current_version() {
        payload["current_version"] = json!(version);
    }
    Response::from_string(payload.to_string())
        .with_status_code(StatusCode(error.status_code()))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}
