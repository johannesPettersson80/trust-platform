//! Control CLI helpers.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use serde_json::{json, Value as JsonValue};
use trust_runtime::config::RuntimeConfig;
use trust_runtime::control::ControlEndpoint;

use crate::cli::ControlAction;

const CONTROL_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const CONTROL_IO_TIMEOUT: Duration = Duration::from_millis(750);
const MAX_CONTROL_RESPONSE_BYTES: usize = 1024 * 1024;

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
    let project_runtime = if let Some(project) = bundle.as_ref() {
        if endpoint.is_none() || auth_token.is_none() {
            Some(RuntimeConfig::load(project.join("runtime.toml"))?)
        } else {
            None
        }
    } else {
        None
    };
    if auth_token.is_none() {
        auth_token = project_runtime
            .as_ref()
            .and_then(|runtime| runtime.control_auth_token.as_ref())
            .map(ToString::to_string);
    }
    let endpoint_text = if let Some(endpoint) = endpoint {
        endpoint
    } else if let Some(runtime) = project_runtime {
        runtime.control_endpoint.to_string()
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
    ensure_control_response_ok(&response)?;
    print_control_response(action, &response)
}

pub(crate) fn ensure_control_response_ok(response: &JsonValue) -> anyhow::Result<()> {
    match response.get("ok").and_then(JsonValue::as_bool) {
        Some(true) => Ok(()),
        Some(false) => {
            let message = response
                .get("error")
                .and_then(JsonValue::as_str)
                .unwrap_or("control request failed");
            if let Some(code) = response.get("error_code").and_then(JsonValue::as_str) {
                anyhow::bail!("{message} ({code})");
            }
            anyhow::bail!("{message}");
        }
        None => anyhow::bail!("control response missing boolean 'ok'"),
    }
}

pub(crate) fn send_control_request_value(
    endpoint: &ControlEndpoint,
    request: &JsonValue,
) -> anyhow::Result<JsonValue> {
    match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let mut stream = std::net::TcpStream::connect_timeout(addr, CONTROL_CONNECT_TIMEOUT)
                .with_context(|| format!("connect control endpoint tcp://{addr}"))?;
            stream
                .set_read_timeout(Some(CONTROL_IO_TIMEOUT))
                .context("set control endpoint read timeout")?;
            stream
                .set_write_timeout(Some(CONTROL_IO_TIMEOUT))
                .context("set control endpoint write timeout")?;
            let mut reader = BufReader::new(stream.try_clone()?);
            exchange_control_request(&mut stream, &mut reader, request)
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let mut stream = std::os::unix::net::UnixStream::connect(path)
                .with_context(|| format!("connect control endpoint unix://{}", path.display()))?;
            stream
                .set_read_timeout(Some(CONTROL_IO_TIMEOUT))
                .context("set control endpoint read timeout")?;
            stream
                .set_write_timeout(Some(CONTROL_IO_TIMEOUT))
                .context("set control endpoint write timeout")?;
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
    let bytes = reader
        .take((MAX_CONTROL_RESPONSE_BYTES + 1) as u64)
        .read_line(&mut response)?;
    if bytes == 0 || response.trim().is_empty() {
        anyhow::bail!("empty control response");
    }
    if bytes > MAX_CONTROL_RESPONSE_BYTES {
        anyhow::bail!(
            "control response exceeded {MAX_CONTROL_RESPONSE_BYTES} bytes without a complete line"
        );
    }
    if !response.ends_with('\n') {
        anyhow::bail!("control response missing newline terminator");
    }
    let response =
        serde_json::from_str::<JsonValue>(response.trim_end()).context("parse control response")?;
    let request_id = request
        .get("id")
        .and_then(JsonValue::as_u64)
        .context("control request missing numeric 'id'")?;
    if response.get("id").and_then(JsonValue::as_u64) != Some(request_id) {
        anyhow::bail!("control response id is missing or does not match request id {request_id}");
    }
    Ok(response)
}

fn print_control_response(action: &ControlAction, response: &JsonValue) -> anyhow::Result<()> {
    match action {
        ControlAction::Status => {
            let result = response
                .get("result")
                .and_then(JsonValue::as_object)
                .context("status response missing object 'result'")?;
            let state = result
                .get("state")
                .and_then(JsonValue::as_str)
                .context("status result missing string 'state'")?;
            let fault = match result.get("fault") {
                Some(value) if value.is_null() => "none",
                Some(value) => value
                    .as_str()
                    .context("status result 'fault' must be a string")?,
                None => "none",
            };
            let (rt_profile, rt_active) = match result.get("realtime") {
                Some(value) => {
                    let realtime = value
                        .as_object()
                        .context("status result 'realtime' must be an object")?;
                    let profile = match realtime.get("profile") {
                        Some(value) => value
                            .as_str()
                            .context("status realtime 'profile' must be a string")?,
                        None => "disabled",
                    };
                    let active = match realtime.get("active") {
                        Some(value) => value
                            .as_bool()
                            .context("status realtime 'active' must be a boolean")?,
                        None => false,
                    };
                    (profile, active)
                }
                None => ("disabled", false),
            };
            println!("state={state} fault={fault} rt_profile={rt_profile} rt_active={rt_active}");
        }
        ControlAction::Health => {
            let ok = response
                .get("result")
                .and_then(|result| result.get("ok"))
                .and_then(JsonValue::as_bool)
                .context("health result missing boolean 'ok'")?;
            println!("ok={ok}");
        }
        ControlAction::Stats => {
            let tasks = response
                .get("result")
                .and_then(|result| result.get("tasks"))
                .and_then(JsonValue::as_array)
                .context("stats result missing array 'tasks'")?;
            if tasks.is_empty() {
                println!("tasks=0");
                return Ok(());
            }
            for task in tasks {
                let name = task
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .context("task result missing string 'name'")?;
                let min = task
                    .get("min_ms")
                    .and_then(JsonValue::as_f64)
                    .context("task result missing numeric 'min_ms'")?;
                let avg = task
                    .get("avg_ms")
                    .and_then(JsonValue::as_f64)
                    .context("task result missing numeric 'avg_ms'")?;
                let max = task
                    .get("max_ms")
                    .and_then(JsonValue::as_f64)
                    .context("task result missing numeric 'max_ms'")?;
                let last = task
                    .get("last_ms")
                    .and_then(JsonValue::as_f64)
                    .context("task result missing numeric 'last_ms'")?;
                let overruns = task
                    .get("overruns")
                    .and_then(JsonValue::as_u64)
                    .context("task result missing non-negative integer 'overruns'")?;
                println!(
                    "task={name} min_ms={min:.3} avg_ms={avg:.3} max_ms={max:.3} last_ms={last:.3} overruns={overruns}"
                );
            }
        }
        _ => println!("{}", serde_json::to_string(response)?),
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_actions_map_to_the_registered_wire_contract() {
        let cases = vec![
            (
                ControlAction::Status,
                json!({"id": 1, "type": "status", "auth": "token"}),
            ),
            (
                ControlAction::Health,
                json!({"id": 1, "type": "health", "auth": "token"}),
            ),
            (
                ControlAction::Stats,
                json!({"id": 1, "type": "tasks.stats", "auth": "token"}),
            ),
            (
                ControlAction::Pause,
                json!({"id": 1, "type": "pause", "auth": "token"}),
            ),
            (
                ControlAction::Resume,
                json!({"id": 1, "type": "resume", "auth": "token"}),
            ),
            (
                ControlAction::StepIn,
                json!({"id": 1, "type": "step_in", "auth": "token"}),
            ),
            (
                ControlAction::StepOver,
                json!({"id": 1, "type": "step_over", "auth": "token"}),
            ),
            (
                ControlAction::StepOut,
                json!({"id": 1, "type": "step_out", "auth": "token"}),
            ),
            (
                ControlAction::BreakpointsSet {
                    source: "main.st".into(),
                    lines: vec![3, 8],
                },
                json!({
                    "id": 1,
                    "type": "breakpoints.set",
                    "auth": "token",
                    "params": { "source": "main.st", "lines": [3, 8] }
                }),
            ),
            (
                ControlAction::BreakpointsClear {
                    source: "main.st".into(),
                },
                json!({
                    "id": 1,
                    "type": "breakpoints.clear",
                    "auth": "token",
                    "params": { "source": "main.st", "lines": [] }
                }),
            ),
            (
                ControlAction::BreakpointsList,
                json!({"id": 1, "type": "breakpoints.list", "auth": "token"}),
            ),
            (
                ControlAction::IoRead,
                json!({"id": 1, "type": "io.read", "auth": "token"}),
            ),
            (
                ControlAction::IoWrite {
                    address: "%QX0.0".into(),
                    value: "TRUE".into(),
                },
                json!({
                    "id": 1,
                    "type": "io.write",
                    "auth": "token",
                    "params": { "address": "%QX0.0", "value": "TRUE" }
                }),
            ),
            (
                ControlAction::IoForce {
                    address: "%QX0.1".into(),
                    value: "FALSE".into(),
                },
                json!({
                    "id": 1,
                    "type": "io.force",
                    "auth": "token",
                    "params": { "address": "%QX0.1", "value": "FALSE" }
                }),
            ),
            (
                ControlAction::IoUnforce {
                    address: "%QX0.1".into(),
                },
                json!({
                    "id": 1,
                    "type": "io.unforce",
                    "auth": "token",
                    "params": { "address": "%QX0.1" }
                }),
            ),
            (
                ControlAction::Eval {
                    expr: "Counter + 1".into(),
                },
                json!({
                    "id": 1,
                    "type": "eval",
                    "auth": "token",
                    "params": { "expr": "Counter + 1" }
                }),
            ),
            (
                ControlAction::Set {
                    target: "Counter".into(),
                    value: "7".into(),
                },
                json!({
                    "id": 1,
                    "type": "set",
                    "auth": "token",
                    "params": { "target": "Counter", "value": "7" }
                }),
            ),
            (
                ControlAction::Restart {
                    mode: "warm".into(),
                },
                json!({
                    "id": 1,
                    "type": "restart",
                    "auth": "token",
                    "params": { "mode": "warm" }
                }),
            ),
            (
                ControlAction::Shutdown,
                json!({"id": 1, "type": "shutdown", "auth": "token"}),
            ),
            (
                ControlAction::ConfigGet,
                json!({"id": 1, "type": "config.get", "auth": "token"}),
            ),
            (
                ControlAction::ConfigSet {
                    key: "discovery.enabled".into(),
                    value: "true".into(),
                },
                json!({
                    "id": 1,
                    "type": "config.set",
                    "auth": "token",
                    "params": { "discovery.enabled": true }
                }),
            ),
        ];

        for (action, expected) in cases {
            assert_eq!(build_request(&action, Some("token")), expected);
        }
        assert_eq!(
            build_request(&ControlAction::Status, None),
            json!({"id": 1, "type": "status", "auth": null})
        );
    }

    #[test]
    fn control_exchange_uses_one_json_line_and_rejects_invalid_responses() {
        let request = json!({"id": 1, "type": "status", "auth": null});
        let mut output = Vec::new();
        let mut response = std::io::Cursor::new(b"{\"id\":1,\"ok\":true}\n");
        let parsed = exchange_control_request(&mut output, &mut response, &request)
            .expect("valid control exchange");
        assert_eq!(parsed, json!({"id": 1, "ok": true}));
        assert_eq!(
            String::from_utf8(output).expect("request is UTF-8"),
            "{\"auth\":null,\"id\":1,\"type\":\"status\"}\n"
        );

        let mut output = Vec::new();
        let mut empty = std::io::Cursor::new(b"");
        let error = exchange_control_request(&mut output, &mut empty, &request)
            .expect_err("empty response must fail");
        assert!(error.to_string().contains("empty control response"));

        let mut output = Vec::new();
        let mut malformed = std::io::Cursor::new(b"not-json\n");
        let error = exchange_control_request(&mut output, &mut malformed, &request)
            .expect_err("malformed response must fail");
        assert!(error.to_string().contains("parse control response"));

        let mut output = Vec::new();
        let mut unterminated = std::io::Cursor::new(b"{\"id\":1,\"ok\":true}");
        let error = exchange_control_request(&mut output, &mut unterminated, &request)
            .expect_err("unterminated response must fail");
        assert!(
            error.to_string().contains("newline terminator"),
            "{error:#}"
        );

        for response in [
            b"{\"ok\":true}\n".as_slice(),
            b"{\"id\":2,\"ok\":true}\n".as_slice(),
        ] {
            let mut output = Vec::new();
            let mut response = std::io::Cursor::new(response);
            let error = exchange_control_request(&mut output, &mut response, &request)
                .expect_err("missing or mismatched response ID must fail");
            assert!(error.to_string().contains("response id"), "{error:#}");
        }

        let mut output = Vec::new();
        let oversized = vec![b'x'; 1024 * 1024 + 1];
        let mut response = std::io::Cursor::new(oversized);
        let error = exchange_control_request(&mut output, &mut response, &request)
            .expect_err("oversized response must fail");
        assert!(error.to_string().contains("exceeded"), "{error:#}");
    }

    #[test]
    fn config_values_and_control_results_cover_closed_scalar_partitions() {
        let cases = [
            (" null ", JsonValue::Null),
            ("TRUE", JsonValue::Bool(true)),
            (" false ", JsonValue::Bool(false)),
            ("-17", json!(-17)),
            ("1.5", json!("1.5")),
            (" value ", json!("value")),
        ];
        for (raw, expected) in cases {
            assert_eq!(parse_config_value(raw), expected, "{raw}");
        }

        assert!(ensure_control_response_ok(&json!({"ok": true})).is_ok());
        let rejected = ensure_control_response_ok(&json!({
            "ok": false,
            "error": "denied",
            "error_code": "insufficient_role"
        }))
        .expect_err("rejected response must fail");
        assert_eq!(rejected.to_string(), "denied (insufficient_role)");
        assert_eq!(
            ensure_control_response_ok(&json!({"ok": false}))
                .expect_err("default rejection must fail")
                .to_string(),
            "control request failed"
        );
        assert!(ensure_control_response_ok(&json!({"ok": "true"}))
            .expect_err("non-boolean ok must fail")
            .to_string()
            .contains("missing boolean 'ok'"));
    }

    #[cfg(unix)]
    #[test]
    fn control_value_exchange_supports_the_unix_endpoint_variant() {
        use std::os::unix::net::UnixListener;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root =
            std::path::PathBuf::from("/tmp").join(format!("trt-{}-{unique}", std::process::id()));
        std::fs::create_dir(&root).expect("create Unix control test directory");
        let socket = root.join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind Unix control socket");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept Unix control request");
            let mut request = String::new();
            BufReader::new(stream.try_clone().expect("clone Unix control stream"))
                .read_line(&mut request)
                .expect("read Unix control request");
            assert_eq!(
                serde_json::from_str::<JsonValue>(request.trim_end()).expect("request JSON"),
                json!({"id": 1, "type": "health", "auth": null})
            );
            writeln!(
                stream,
                "{}",
                json!({"id": 1, "ok": true, "result": {"ok": true}})
            )
            .expect("write Unix control response");
        });

        let response = send_control_request_value(
            &ControlEndpoint::Unix(socket.clone()),
            &json!({"id": 1, "type": "health", "auth": null}),
        )
        .expect("Unix control exchange");
        server.join().expect("Unix control server");
        assert_eq!(
            response,
            json!({"id": 1, "ok": true, "result": {"ok": true}})
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn control_value_exchange_enforces_the_established_read_timeout() {
        use std::net::TcpListener;
        use std::sync::mpsc;
        use std::time::Instant;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind timeout control listener");
        let endpoint =
            ControlEndpoint::Tcp(listener.local_addr().expect("timeout listener address"));
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept timeout control request");
            let mut request = String::new();
            BufReader::new(stream.try_clone().expect("clone timeout control stream"))
                .read_line(&mut request)
                .expect("read timeout control request");
            release_rx
                .recv_timeout(Duration::from_secs(3))
                .expect("client must release the held response connection");
        });

        let started = Instant::now();
        let error = send_control_request_value(
            &endpoint,
            &json!({"id": 1, "type": "status", "auth": null}),
        )
        .expect_err("missing response must hit the read timeout");
        let elapsed = started.elapsed();
        release_tx.send(()).expect("release timeout server");
        server.join().expect("timeout control server");

        assert!(
            elapsed >= Duration::from_millis(500) && elapsed < Duration::from_secs(2),
            "control read timeout elapsed {elapsed:?}: {error:#}"
        );
    }
}
