//! Control CLI helpers.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::Context;
use serde_json::{json, Value as JsonValue};
use trust_runtime::config::RuntimeBundle;
use trust_runtime::control::ControlEndpoint;

use crate::cli::ControlAction;

pub(crate) struct ResolvedControlTarget {
    pub(crate) endpoint: ControlEndpoint,
    pub(crate) auth_token: Option<String>,
}

pub fn run_control(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    action: ControlAction,
) -> anyhow::Result<()> {
    let target = resolve_control_target(bundle, endpoint, token)?;
    send_control_request(&target, &action)
}

pub(crate) fn resolve_control_target(
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
        endpoint,
        auth_token,
    })
}

fn send_control_request(
    target: &ResolvedControlTarget,
    action: &ControlAction,
) -> anyhow::Result<()> {
    let request = build_request(action, target.auth_token.as_deref());
    let response = send_control_request_value(&target.endpoint, &request)?;
    let rendered = serde_json::to_string(&response)?;
    print_control_response(action, rendered.trim_end());
    Ok(())
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

fn print_control_response(action: &ControlAction, response: &str) {
    if response.trim().is_empty() {
        return;
    }
    if matches!(action, ControlAction::Status) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(response) {
            if let Some(result) = value.get("result") {
                if let Some(state) = result.get("state").and_then(|v| v.as_str()) {
                    let fault = result
                        .get("fault")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none");
                    let rt_profile = result
                        .get("realtime")
                        .and_then(|v| v.get("profile"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("disabled");
                    let rt_active = result
                        .get("realtime")
                        .and_then(|v| v.get("active"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    println!(
                        "state={state} fault={fault} rt_profile={rt_profile} rt_active={rt_active}"
                    );
                    return;
                }
            }
        }
    }
    if matches!(action, ControlAction::Health) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(response) {
            if let Some(result) = value.get("result") {
                if let Some(ok) = result.get("ok").and_then(|v| v.as_bool()) {
                    println!("ok={ok}");
                    return;
                }
            }
        }
    }
    if matches!(action, ControlAction::Stats) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(response) {
            if let Some(tasks) = value
                .get("result")
                .and_then(|result| result.get("tasks"))
                .and_then(|value| value.as_array())
            {
                if tasks.is_empty() {
                    println!("tasks=0");
                    return;
                }
                for task in tasks {
                    let name = task.get("name").and_then(|v| v.as_str()).unwrap_or("task");
                    let min = task.get("min_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let avg = task.get("avg_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let max = task.get("max_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let last = task.get("last_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let overruns = task.get("overruns").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!(
                        "task={name} min_ms={min:.3} avg_ms={avg:.3} max_ms={max:.3} last_ms={last:.3} overruns={overruns}"
                    );
                }
                return;
            }
        }
    }
    println!("{response}");
}

fn build_request(action: &ControlAction, auth_token: Option<&str>) -> serde_json::Value {
    let auth = auth_token.map(|value| value.to_string());
    match action {
        ControlAction::Status => json!({"id": 1, "type": "status", "auth": auth}),
        ControlAction::Health => json!({"id": 1, "type": "health", "auth": auth}),
        ControlAction::Stats => json!({"id": 1, "type": "tasks.stats", "auth": auth}),
        ControlAction::Pause => json!({"id": 1, "type": "pause", "auth": auth}),
        ControlAction::Resume => json!({"id": 1, "type": "resume", "auth": auth}),
        ControlAction::StepIn => json!({"id": 1, "type": "step_in", "auth": auth}),
        ControlAction::StepOver => json!({"id": 1, "type": "step_over", "auth": auth}),
        ControlAction::StepOut => json!({"id": 1, "type": "step_out", "auth": auth}),
        ControlAction::BreakpointsSet { source, lines } => json!({
            "id": 1,
            "type": "breakpoints.set",
            "auth": auth,
            "params": { "source": source, "lines": lines }
        }),
        ControlAction::BreakpointsClear { source } => json!({
            "id": 1,
            "type": "breakpoints.clear",
            "auth": auth,
            "params": { "source": source, "lines": [] }
        }),
        ControlAction::BreakpointsList => {
            json!({"id": 1, "type": "breakpoints.list", "auth": auth})
        }
        ControlAction::IoRead => json!({"id": 1, "type": "io.read", "auth": auth}),
        ControlAction::IoWrite { address, value } => json!({
            "id": 1,
            "type": "io.write",
            "auth": auth,
            "params": { "address": address, "value": value }
        }),
        ControlAction::IoForce { address, value } => json!({
            "id": 1,
            "type": "io.force",
            "auth": auth,
            "params": { "address": address, "value": value }
        }),
        ControlAction::IoUnforce { address } => json!({
            "id": 1,
            "type": "io.unforce",
            "auth": auth,
            "params": { "address": address }
        }),
        ControlAction::Eval { expr } => json!({
            "id": 1,
            "type": "eval",
            "auth": auth,
            "params": { "expr": expr }
        }),
        ControlAction::Set { target, value } => json!({
            "id": 1,
            "type": "set",
            "auth": auth,
            "params": { "target": target, "value": value }
        }),
        ControlAction::Restart { mode } => json!({
            "id": 1,
            "type": "restart",
            "auth": auth,
            "params": { "mode": mode }
        }),
        ControlAction::Shutdown => json!({"id": 1, "type": "shutdown", "auth": auth}),
        ControlAction::ConfigGet => json!({"id": 1, "type": "config.get", "auth": auth}),
        ControlAction::ConfigSet { key, value } => {
            let mut params = serde_json::Map::new();
            params.insert(key.clone(), parse_config_value(value));
            json!({
                "id": 1,
                "type": "config.set",
                "auth": auth,
                "params": params
            })
        }
    }
}

fn parse_config_value(value: &str) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return serde_json::Value::Null;
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if let Ok(number) = trimmed.parse::<i64>() {
        return serde_json::Value::Number(number.into());
    }
    serde_json::Value::String(trimmed.to_string())
}
