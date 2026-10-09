//! Shared POST request policy/body helpers for web routes.

#![allow(missing_docs)]

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JsonBodyError {
    InvalidBody,
    TooLarge,
    InvalidJson,
}

pub(super) fn read_json_body<T: DeserializeOwned>(
    request: &mut tiny_http::Request,
    max_bytes: usize,
) -> Result<T, JsonBodyError> {
    let read_limit = u64::try_from(max_bytes.saturating_add(1)).unwrap_or(u64::MAX);
    let mut limited = request.as_reader().take(read_limit);
    let mut body = Vec::new();
    if limited.read_to_end(&mut body).is_err() {
        return Err(JsonBodyError::InvalidBody);
    }
    if body.len() > max_bytes {
        return Err(JsonBodyError::TooLarge);
    }
    serde_json::from_slice(&body).map_err(|_| JsonBodyError::InvalidJson)
}

pub(super) fn json_body_error_response(error: JsonBodyError) -> Response<std::io::Cursor<Vec<u8>>> {
    let (status, denial_code, text) = match error {
        JsonBodyError::InvalidBody => (StatusCode(400), "contract_violation", "invalid body"),
        JsonBodyError::TooLarge => (
            StatusCode(413),
            "contract_violation",
            "request body exceeds maximum size",
        ),
        JsonBodyError::InvalidJson => (StatusCode(400), "contract_violation", "invalid json"),
    };
    Response::from_string(
        json!({
            "ok": false,
            "denial_code": denial_code,
            "error": text,
        })
        .to_string(),
    )
    .with_status_code(status)
    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

/// What a write route allows when a request carries no `Origin` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingOrigin {
    /// Only a loopback client, as the runtime's general write routes require.
    LoopbackOnly,
    /// Any client. A browser sends `Origin` with every POST, so a request without one comes from
    /// a program such as the editor's Live mode or Deploy (specification 39, §21.5 item 3).
    Allowed,
}

pub(super) fn api_post_policy_check(
    request: &tiny_http::Request,
    web_tls_enabled: bool,
    require_json_content_type: bool,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    post_policy_check(
        request,
        web_tls_enabled,
        require_json_content_type,
        MissingOrigin::LoopbackOnly,
    )
}

/// The cross-site check of every HMI write route (specification 39, §21.5 item 3): the policy of
/// the other write routes, on the connection's own scheme, requiring a JSON body and, when the
/// request carries `Origin`, the runtime's own origin. Every refusal answers 403.
pub(super) fn hmi_post_policy_check(
    request: &tiny_http::Request,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    post_policy_check(request, request.secure(), true, MissingOrigin::Allowed)
        .map_err(|refusal| refusal.with_status_code(StatusCode(403)))
}

/// Runs an HMI write route behind [`hmi_post_policy_check`]: a refused request is answered at
/// once and never reaches the route, so it has no effect.
pub(super) fn hmi_write_route(request: tiny_http::Request, route: impl FnOnce(tiny_http::Request)) {
    match hmi_post_policy_check(&request) {
        Ok(()) => route(request),
        Err(refusal) => {
            let _ = request.respond(refusal);
        }
    }
}

fn post_policy_check(
    request: &tiny_http::Request,
    web_tls_enabled: bool,
    require_json_content_type: bool,
    missing_origin: MissingOrigin,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    if require_json_content_type {
        let content_type = header_value(request, "Content-Type").unwrap_or_default();
        let media_type = content_type.split(';').next().unwrap_or_default().trim();
        if !media_type.eq_ignore_ascii_case("application/json") {
            return Err(Response::from_string(
                json!({
                    "ok": false,
                    "denial_code": "contract_violation",
                    "error": "Content-Type must be application/json",
                })
                .to_string(),
            )
            .with_status_code(StatusCode(415))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()));
        }
    }

    let Some(origin_raw) = header_value(request, "Origin") else {
        if missing_origin == MissingOrigin::Allowed
            || request
                .remote_addr()
                .is_some_and(|addr| addr.ip().is_loopback())
        {
            return Ok(());
        }
        return Err(Response::from_string(
            json!({
                "ok": false,
                "denial_code": "permission_denied",
                "error": "missing Origin header",
            })
            .to_string(),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()));
    };
    let origin = origin_raw.trim().trim_end_matches('/').to_ascii_lowercase();
    if origin == "null" {
        return Err(Response::from_string(
            json!({
                "ok": false,
                "denial_code": "permission_denied",
                "error": "origin is not allowed",
            })
            .to_string(),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()));
    }
    let Some(host_raw) = header_value(request, "Host") else {
        return Err(Response::from_string(
            json!({
                "ok": false,
                "denial_code": "contract_violation",
                "error": "missing Host header",
            })
            .to_string(),
        )
        .with_status_code(StatusCode(400))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()));
    };
    let scheme = if web_tls_enabled { "https" } else { "http" };
    let expected_origin = format!(
        "{}://{}",
        scheme,
        host_raw.trim().trim_end_matches('/').to_ascii_lowercase()
    );
    if origin != expected_origin {
        return Err(Response::from_string(
            json!({
                "ok": false,
                "denial_code": "permission_denied",
                "error": format!("origin '{}' is not allowed", origin_raw),
            })
            .to_string(),
        )
        .with_status_code(StatusCode(403))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "http_post_policy/contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "http_post_policy/hmi_contract_tests.rs"]
mod hmi_contract_tests;
