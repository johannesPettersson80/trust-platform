//! JSON-line deterministic harness executor for agents, CI, and docs examples.

use std::env;
use std::io::{self, BufRead, Write};

use anyhow::{anyhow, Context};
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};
use trust_runtime::harness::{
    decode_json_value, encode_json_value, BoundaryEntry, BoundaryError, HarnessAutomation,
    HarnessAutomationError,
};
use trust_runtime::RestartMode;

#[derive(Debug, Deserialize)]
struct Request {
    cmd: String,
    source: Option<String>,
    sources: Option<Vec<String>>,
    count: Option<u32>,
    dt_ms: Option<i64>,
    duration_ms: Option<i64>,
    watch: Option<Vec<String>>,
    name: Option<String>,
    value: Option<JsonValue>,
    equals: Option<JsonValue>,
    max_cycles: Option<u64>,
    address: Option<String>,
    mode: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolVersion {
    V1,
    V2,
}

impl ProtocolVersion {
    fn number(self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let protocol = parse_protocol_version()?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut harness = HarnessAutomation::new();

    for line in stdin.lock().lines() {
        let line = line.context("read stdin line")?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => handle_request(request, &mut harness, protocol),
            Err(err) => with_protocol(
                error_response("invalid_request", format!("invalid request: {err}"), None),
                protocol,
            ),
        };

        writeln!(out, "{}", serde_json::to_string(&response)?)?;
        out.flush()?;
    }

    Ok(())
}

fn handle_request(
    request: Request,
    harness: &mut HarnessAutomation,
    protocol: ProtocolVersion,
) -> JsonValue {
    let response = match dispatch_request(request, harness, protocol) {
        Ok(data) => json!({
            "ok": true,
            "protocol_version": protocol.number(),
            "data": data,
        }),
        Err(err) => match err.downcast::<HarnessAutomationError>() {
            Ok(protocol_error) => automation_error_response(protocol_error),
            Err(other) => error_response("invalid_request", other.to_string(), None),
        },
    };
    with_protocol(response, protocol)
}

fn dispatch_request(
    request: Request,
    harness: &mut HarnessAutomation,
    protocol: ProtocolVersion,
) -> anyhow::Result<JsonValue> {
    match request.cmd.as_str() {
        "load" => handle_load(request, harness),
        "reload" => handle_reload(request, harness),
        "cycle" => handle_cycle(request, harness, protocol),
        "set_input" => handle_set_input(request, harness),
        "get_output" => handle_get_output(request, harness),
        "set_access" => handle_set_access(request, harness),
        "get_access" => handle_get_access(request, harness),
        "bind_direct" => handle_bind_direct(request, harness),
        "set_direct_input" => handle_set_direct_input(request, harness),
        "get_direct_output" => handle_get_direct_output(request, harness),
        "advance_time" => handle_advance_time(request, harness),
        "run_until" => handle_run_until(request, harness, protocol),
        "restart" => handle_restart(request, harness),
        "snapshot" => handle_snapshot(request, harness, protocol),
        other => Err(anyhow!("unsupported command '{other}'")),
    }
}

fn handle_load(request: Request, harness: &mut HarnessAutomation) -> anyhow::Result<JsonValue> {
    let summary = harness
        .load_sources(&source_list(&request)?)
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "source_count": summary.source_count,
        "cycle_count": summary.cycle_count,
        "elapsed_ms": summary.elapsed_ms,
    }))
}

fn handle_reload(request: Request, harness: &mut HarnessAutomation) -> anyhow::Result<JsonValue> {
    let summary = harness
        .reload_sources(&source_list(&request)?)
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "source_count": summary.source_count,
        "cycle_count": summary.cycle_count,
        "elapsed_ms": summary.elapsed_ms,
    }))
}

fn handle_cycle(
    request: Request,
    harness: &mut HarnessAutomation,
    protocol: ProtocolVersion,
) -> anyhow::Result<JsonValue> {
    let snapshot = harness
        .cycle(
            request.count.unwrap_or(1),
            request.dt_ms.unwrap_or(0),
            request.watch.as_deref().unwrap_or(&[]),
        )
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "cycle_count": snapshot.cycle_count,
        "elapsed_ms": snapshot.elapsed_ms,
        "values": encode_watch_values(snapshot.values, protocol)?,
    }))
}

fn handle_set_input(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let name = required_string(request.name, "set_input requires 'name'")?;
    let value = request
        .value
        .as_ref()
        .ok_or_else(|| anyhow!("set_input requires 'value'"))?;
    harness
        .set_input(
            name.as_str(),
            decode_json_value(value).map_err(anyhow::Error::from)?,
        )
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "name": name,
        "status": "ok",
    }))
}

fn handle_get_output(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let name = required_string(request.name, "get_output requires 'name'")?;
    let snapshot = harness
        .get_output(name.as_str())
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "name": snapshot.name,
        "value": encode_json_value(&snapshot.value),
    }))
}

fn handle_set_access(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let name = required_string(request.name, "set_access requires 'name'")?;
    let value = request
        .value
        .as_ref()
        .ok_or_else(|| anyhow!("set_access requires 'value'"))?;
    harness
        .set_access(
            name.as_str(),
            decode_json_value(value).map_err(anyhow::Error::from)?,
        )
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "name": name,
        "status": "ok",
    }))
}

fn handle_get_access(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let name = required_string(request.name, "get_access requires 'name'")?;
    let snapshot = harness
        .get_access(name.as_str())
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "name": snapshot.name,
        "value": encode_json_value(&snapshot.value),
    }))
}

fn handle_bind_direct(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let name = required_string(request.name, "bind_direct requires 'name'")?;
    let address = required_string(request.address, "bind_direct requires 'address'")?;
    harness
        .bind_direct(name.as_str(), address.as_str())
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "name": name,
        "address": address,
        "status": "ok",
    }))
}

fn handle_set_direct_input(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let address = required_string(request.address, "set_direct_input requires 'address'")?;
    let value = request
        .value
        .as_ref()
        .ok_or_else(|| anyhow!("set_direct_input requires 'value'"))?;
    harness
        .set_direct_input(
            address.as_str(),
            decode_json_value(value).map_err(anyhow::Error::from)?,
        )
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "address": address,
        "status": "ok",
    }))
}

fn handle_get_direct_output(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let address = required_string(request.address, "get_direct_output requires 'address'")?;
    let snapshot = harness
        .get_direct_output(address.as_str())
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "address": address,
        "value": encode_json_value(&snapshot.value),
    }))
}

fn handle_advance_time(
    request: Request,
    harness: &mut HarnessAutomation,
) -> anyhow::Result<JsonValue> {
    let duration_ms = request.duration_ms.or(request.dt_ms).unwrap_or(0);
    let summary = harness
        .advance_time(duration_ms)
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "cycle_count": summary.cycle_count,
        "elapsed_ms": summary.elapsed_ms,
    }))
}

fn handle_run_until(
    request: Request,
    harness: &mut HarnessAutomation,
    protocol: ProtocolVersion,
) -> anyhow::Result<JsonValue> {
    let name = required_string(request.name, "run_until requires 'name'")?;
    let equals = request
        .equals
        .as_ref()
        .ok_or_else(|| anyhow!("run_until requires 'equals'"))?;
    let summary = harness
        .run_until(
            name.as_str(),
            decode_json_value(equals).map_err(anyhow::Error::from)?,
            request.dt_ms.unwrap_or(0),
            request.max_cycles.unwrap_or(10_000),
            request.watch.as_deref().unwrap_or(&[]),
        )
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "name": summary.name,
        "cycles_ran": summary.cycles_ran,
        "cycle_count": summary.cycle_count,
        "elapsed_ms": summary.elapsed_ms,
        "matched_value": encode_json_value(&summary.matched_value),
        "values": encode_watch_values(summary.values, protocol)?,
    }))
}

fn handle_restart(request: Request, harness: &mut HarnessAutomation) -> anyhow::Result<JsonValue> {
    let mode = parse_restart_mode(request.mode.as_deref().unwrap_or("cold"))?;
    let summary = harness.restart(mode).map_err(anyhow::Error::from)?;
    Ok(json!({
        "mode": match mode {
            RestartMode::Cold => "cold",
            RestartMode::Warm => "warm",
        },
        "cycle_count": summary.cycle_count,
        "elapsed_ms": summary.elapsed_ms,
    }))
}

fn handle_snapshot(
    request: Request,
    harness: &mut HarnessAutomation,
    protocol: ProtocolVersion,
) -> anyhow::Result<JsonValue> {
    let snapshot = harness
        .snapshot(request.watch.as_deref().unwrap_or(&[]))
        .map_err(anyhow::Error::from)?;
    Ok(json!({
        "cycle_count": snapshot.cycle_count,
        "elapsed_ms": snapshot.elapsed_ms,
        "values": encode_watch_values(snapshot.values, protocol)?,
    }))
}

fn source_list(request: &Request) -> anyhow::Result<Vec<String>> {
    if let Some(sources) = request.sources.as_ref() {
        if sources.is_empty() {
            return Err(anyhow!("'sources' must not be empty"));
        }
        return Ok(sources.clone());
    }
    if let Some(source) = request.source.as_ref() {
        return Ok(vec![source.clone()]);
    }
    Err(anyhow!("request requires 'source' or 'sources'"))
}

fn required_string(value: Option<String>, message: &str) -> anyhow::Result<String> {
    value.ok_or_else(|| anyhow!(message.to_string()))
}

fn parse_restart_mode(mode: &str) -> anyhow::Result<RestartMode> {
    match mode.to_ascii_lowercase().as_str() {
        "cold" => Ok(RestartMode::Cold),
        "warm" => Ok(RestartMode::Warm),
        other => Err(anyhow!("unsupported restart mode '{other}'")),
    }
}

fn parse_protocol_version() -> anyhow::Result<ProtocolVersion> {
    let mut value = env::var("TRUST_HARNESS_PROTOCOL_VERSION").ok();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--protocol-version" {
            value = Some(
                args.next()
                    .ok_or_else(|| anyhow!("--protocol-version requires 1 or 2"))?,
            );
        } else if let Some(version) = arg.strip_prefix("--protocol-version=") {
            value = Some(version.to_string());
        } else {
            return Err(anyhow!("unsupported argument '{arg}'"));
        }
    }

    match value.as_deref().unwrap_or("2") {
        "1" => Ok(ProtocolVersion::V1),
        "2" => Ok(ProtocolVersion::V2),
        other => Err(anyhow!("unsupported protocol version '{other}'")),
    }
}

fn encode_watch_values(
    values: std::collections::BTreeMap<String, BoundaryEntry>,
    protocol: ProtocolVersion,
) -> anyhow::Result<JsonValue> {
    let mut object = Map::new();
    for (name, entry) in values {
        match protocol {
            ProtocolVersion::V2 => {
                object.insert(name, encode_boundary_entry(&entry));
            }
            ProtocolVersion::V1 => {
                if let Some(error) = entry.error {
                    return Err(anyhow::Error::from(HarnessAutomationError::Boundary(error)));
                }
                let value = entry.value.ok_or_else(|| {
                    anyhow::Error::from(HarnessAutomationError::Boundary(
                        BoundaryError::InternalFailure {
                            context: "watch entry missing value",
                        },
                    ))
                })?;
                object.insert(name, encode_json_value(&value));
            }
        }
    }
    Ok(JsonValue::Object(object))
}

fn encode_boundary_entry(entry: &BoundaryEntry) -> JsonValue {
    if let Some(value) = entry.value.as_ref() {
        return json!({
            "status": "ok",
            "value": encode_json_value(value),
        });
    }
    let Some(error) = entry.error.as_ref() else {
        return json!({
            "status": "error",
            "code": "internal_failure",
            "message": "watch entry missing value and error",
        });
    };
    json!({
        "status": "error",
        "code": error.code(),
        "message": error.to_string(),
        "path": error.path(),
        "candidates": error.candidates().iter().map(|candidate| candidate.as_str()).collect::<Vec<_>>(),
    })
}

fn automation_error_response(error: HarnessAutomationError) -> JsonValue {
    match error {
        HarnessAutomationError::NotLoaded => error_response("not_loaded", error.to_string(), None),
        HarnessAutomationError::InvalidArgument(message) => {
            error_response("invalid_argument", message, None)
        }
        HarnessAutomationError::Compile(message) => error_response("compile_error", message, None),
        HarnessAutomationError::Runtime(message) => error_response("runtime_error", message, None),
        HarnessAutomationError::RuntimeCycle { message, errors } => error_response(
            "runtime_cycle_error",
            message,
            Some(json!({ "errors": errors })),
        ),
        HarnessAutomationError::Boundary(error) => boundary_error_response(error),
        HarnessAutomationError::RunUntilTimeout {
            name,
            max_cycles,
            expected,
        } => error_response(
            "run_until_timeout",
            format!(
                "run_until exceeded {max_cycles} cycles before '{name}' matched the expected value"
            ),
            Some(json!({
                "name": name,
                "max_cycles": max_cycles,
                "expected": encode_json_value(&expected),
            })),
        ),
    }
}

fn boundary_error_response(error: BoundaryError) -> JsonValue {
    error_response(
        error.code(),
        error.to_string(),
        Some(json!({
            "path": error.path(),
            "candidates": error.candidates().iter().map(|candidate| candidate.as_str()).collect::<Vec<_>>(),
        })),
    )
}

fn error_response(kind: &str, message: String, data: Option<JsonValue>) -> JsonValue {
    json!({
        "ok": false,
        "error": {
            "kind": kind,
            "message": message,
            "data": data,
        },
    })
}

fn with_protocol(mut response: JsonValue, protocol: ProtocolVersion) -> JsonValue {
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "protocol_version".to_string(),
            JsonValue::from(protocol.number()),
        );
    }
    response
}
