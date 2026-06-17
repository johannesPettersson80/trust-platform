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
    let mut limited = request.as_reader().take((max_bytes + 1) as u64);
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

pub(super) fn api_post_policy_check(
    request: &tiny_http::Request,
    web_tls_enabled: bool,
    require_json_content_type: bool,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    if require_json_content_type {
        let content_type = header_value(request, "Content-Type")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !content_type.starts_with("application/json") {
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
        if request
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
