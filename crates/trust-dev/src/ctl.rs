//! Agent-facing control request helpers.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::Context;
use serde_json::{json, Value as JsonValue};
use trust_runtime::config::RuntimeConfig;
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
    let token = token.or_else(|| std::env::var("TRUST_CTL_TOKEN").ok());
    let target = resolve_control_target(bundle, endpoint, token)?;
    let request = json!({
        "id": 1,
        "type": request_type,
        "auth": target.auth_token.as_deref(),
        "params": params,
    });
    let response = send_control_request_value(&target.endpoint, &request)?;
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

struct ResolvedControlTarget {
    endpoint_text: String,
    endpoint: ControlEndpoint,
    auth_token: Option<String>,
}

/// Resolves the endpoint and token the way `trust-runtime ctl` does (spec 11 section 6.8.1).
///
/// The explicit or environment token comes first. The project's `runtime.toml` supplies the
/// endpoint when none is given and the token when none is known. It is read even with
/// `--endpoint`, so the project token stays eligible when only the endpoint is overridden.
fn resolve_control_target(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<ResolvedControlTarget> {
    let mut auth_token = token;
    let project_runtime = match bundle {
        Some(project) if endpoint.is_none() || auth_token.is_none() => {
            Some(RuntimeConfig::load(project.join("runtime.toml"))?)
        }
        _ => None,
    };
    if auth_token.is_none() {
        auth_token = project_runtime
            .as_ref()
            .and_then(|runtime| runtime.control_auth_token.as_ref())
            .map(ToString::to_string);
    }
    let endpoint_text = match (endpoint, project_runtime) {
        (Some(endpoint), _) => endpoint,
        (None, Some(runtime)) => runtime.control_endpoint.to_string(),
        (None, None) => anyhow::bail!("--endpoint or --project required"),
    };
    let endpoint = ControlEndpoint::parse(&endpoint_text)?;
    Ok(ResolvedControlTarget {
        endpoint_text,
        endpoint,
        auth_token,
    })
}

fn send_control_request_value(
    endpoint: &ControlEndpoint,
    request: &JsonValue,
) -> anyhow::Result<JsonValue> {
    match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let mut stream = std::net::TcpStream::connect(addr)
                .with_context(|| format!("connect control endpoint tcp://{addr}"))?;
            let mut reader = BufReader::new(stream.try_clone()?);
            exchange_control_request(&mut stream, &mut reader, request)
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let mut stream = std::os::unix::net::UnixStream::connect(path)
                .with_context(|| format!("connect control endpoint unix://{}", path.display()))?;
            let mut reader = BufReader::new(stream.try_clone()?);
            exchange_control_request(&mut stream, &mut reader, request)
        }
    }
}

fn exchange_control_request<S: Write, R: BufRead>(
    stream: &mut S,
    reader: &mut R,
    request: &JsonValue,
) -> anyhow::Result<JsonValue> {
    let line = serde_json::to_string(request)?;
    writeln!(stream, "{line}")?;
    stream.flush()?;
    let mut response = String::new();
    reader.read_line(&mut response)?;
    if response.trim().is_empty() {
        anyhow::bail!("empty control response");
    }
    serde_json::from_str::<JsonValue>(response.trim_end()).context("parse control response")
}
