//! Agent-facing control request helpers.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::Context;
use serde_json::{json, Value as JsonValue};
use trust_runtime::config::RuntimeBundle;
use trust_runtime::control::ControlEndpoint;

pub(crate) fn call_control_request(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    request_type: &str,
    params: Option<JsonValue>,
) -> anyhow::Result<ControlCallResult> {
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

fn resolve_control_target(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<ResolvedControlTarget> {
    let mut auth_token = token.or_else(|| std::env::var("TRUST_CTL_TOKEN").ok());
    let endpoint_text = if let Some(endpoint) = endpoint {
        endpoint
    } else if let Some(bundle_path) = bundle {
        let bundle = RuntimeBundle::load(bundle_path)?;
        if auth_token.is_none() {
            auth_token = bundle
                .runtime
                .control_auth_token
                .as_ref()
                .map(|value| value.to_string());
        }
        bundle.runtime.control_endpoint.to_string()
    } else {
        anyhow::bail!("--endpoint or --project required");
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
