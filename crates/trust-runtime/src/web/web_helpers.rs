//! Generic helper functions used across web route modules.

#![allow(missing_docs)]

use super::*;

pub(super) fn header_value(request: &tiny_http::Request, key: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(key))
        .map(|header| header.value.as_str().trim().to_string())
}

pub(super) fn format_web_url(listen: &str, tls: bool) -> String {
    let (host, port) = if listen.starts_with('[') {
        listen
            .rfind("]:")
            .map(|index| (&listen[..=index], &listen[index + 2..]))
            .unwrap_or((listen, "8080"))
    } else {
        listen.rsplit_once(':').unwrap_or((listen, "8080"))
    };
    let host = if host == "0.0.0.0" { "localhost" } else { host };
    let scheme = if tls { "https" } else { "http" };
    format!("{scheme}://{host}:{port}")
}

pub(super) fn render_qr_svg(text: &str) -> Result<String, RuntimeError> {
    let code = QrCode::new(text.as_bytes())
        .map_err(|err| RuntimeError::ControlError(format!("qr: {err}").into()))?;
    let svg = code.render::<svg::Color>().min_dimensions(120, 120).build();
    Ok(svg)
}

pub(super) fn parse_limit(url: &str) -> Option<u64> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == "limit" {
            return parts.next().and_then(|value| {
                (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
                    .then(|| value.parse().ok())
                    .flatten()
            });
        }
    }
    None
}

pub(super) fn query_value(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            let raw = parts.next().unwrap_or_default();
            return Some(decode_url_component(raw));
        }
    }
    None
}

pub(super) fn parse_runtime_cloud_rollout_action(url: &str) -> Option<(String, String)> {
    let prefix = "/api/runtime-cloud/rollouts/";
    let rest = url.strip_prefix(prefix)?;
    let mut parts = rest.split('/');
    let rollout_id = parts.next()?.trim();
    let action = parts.next()?.trim();
    if rollout_id.is_empty() || action.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((rollout_id.to_string(), action.to_string()))
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub(super) fn decode_url_component(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let input = input.as_bytes();
    let mut index = 0;
    while index < input.len() {
        let byte = input[index];
        match byte {
            b'%' if index + 2 < input.len() => {
                let hex = &input[index + 1..index + 3];
                if hex.iter().all(|byte| byte.is_ascii_hexdigit()) {
                    let text = std::str::from_utf8(hex).expect("ASCII hex is UTF-8");
                    bytes.push(u8::from_str_radix(text, 16).expect("validated hex octet"));
                    index += 3;
                    continue;
                }
                bytes.push(byte);
            }
            b'+' => bytes.push(b' '),
            _ => bytes.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub(super) fn parse_probe_response(text: &str) -> Value {
    let value: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({}));
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if !ok {
        let error = value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unreachable");
        return json!({ "ok": false, "error": error });
    }
    let result = value.get("result").cloned().unwrap_or_else(|| json!({}));
    let name = result
        .get("plc_name")
        .or_else(|| result.get("resource"))
        .and_then(|v| v.as_str())
        .unwrap_or("PLC");
    let state = result
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("online");
    json!({ "ok": true, "name": name, "state": state })
}

#[cfg(test)]
#[path = "web_helpers/contract_tests.rs"]
mod contract_tests;
