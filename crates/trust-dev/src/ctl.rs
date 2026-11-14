//! Agent-facing control request helpers.

use std::path::PathBuf;

use serde_json::{json, Value as JsonValue};
use trust_runtime::control::endpoint::socket_client::{self, ConnectFailed};
use trust_runtime::control::endpoint::{control_token_from_env, ControlTarget};
use trust_runtime::control::ControlEndpoint;

#[cfg(test)]
mod tests;

pub(crate) fn call_control_request(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    request_type: &str,
    params: Option<JsonValue>,
) -> anyhow::Result<ControlCallResult> {
    let token = token.or_else(control_token_from_env);
    let target = resolve_control_target(bundle, endpoint, token)?;
    let request = json!({
        "type": request_type,
        "auth": target.auth_token.as_deref(),
        "params": params,
    });
    let response = send_control_request_value(&target, &request)?;
    if response
        .get("ok")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false)
    {
        Ok(ControlCallResult {
            endpoint: target.endpoint_text,
            result: response.get("result").cloned().unwrap_or(JsonValue::Null),
            raw_response: response,
        })
    } else {
        let message = response
            .get("error")
            .and_then(JsonValue::as_str)
            .unwrap_or("control request failed");
        anyhow::bail!("{message}");
    }
}

pub(crate) struct ControlCallResult {
    pub(crate) endpoint: String,
    pub(crate) result: JsonValue,
    pub(crate) raw_response: JsonValue,
}

/// Resolves the endpoint and token the way `trust-runtime ctl` does (spec 11, sections 6.8.0a and
/// 6.8.1). The explicit or environment token comes first, and the project supplies what is
/// missing, so the shared resolver is the only definition of the order.
fn resolve_control_target(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<ControlTarget> {
    let explicit = endpoint
        .as_deref()
        .map(ControlEndpoint::parse)
        .transpose()?;
    Ok(trust_runtime::control::endpoint::resolve_control_target(
        bundle.as_deref(),
        explicit,
        token.as_deref(),
    )?)
}

/// Sends one request through the shared control client of a runtime's socket, with the waits,
/// the response limit and the checks that `trust-runtime ctl` has (spec 11, section 6.8.1). A
/// failed connection is reported through the target, so nothing listening at the project's own
/// endpoint reads as a runtime that is not running (spec 11, section 6.8.0a).
fn send_control_request_value(
    target: &ControlTarget,
    request: &JsonValue,
) -> anyhow::Result<JsonValue> {
    socket_client::request(&target.endpoint, request).map_err(|error| {
        match error.downcast_ref::<ConnectFailed>() {
            Some(failed) => anyhow::anyhow!(target.connect_error(&failed.error)),
            None => error,
        }
    })
}
