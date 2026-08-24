use std::collections::VecDeque;
use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use serde::Serialize;
use toml_edit::{DocumentMut, Item};
use trust_runtime::bundle_builder::build_program_stbc;
use trust_runtime::control::ControlEndpoint;

use super::{
    load_manifest, FleetManifestRuntime, FLEET_MANIFEST_FILE, FLEET_MANIFEST_SCHEMA_VERSION,
};

const MAX_CONTROL_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub(super) struct FleetRuntimeLifecycleResponse {
    pub(super) schema_version: u32,
    pub(super) name: String,
    pub(super) path: String,
    pub(super) control_endpoint: String,
    pub(super) status: String,
    pub(super) pid: Option<u32>,
    pub(super) log_path: String,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) control_status: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetRuntimeLogsResponse {
    pub(super) schema_version: u32,
    pub(super) name: String,
    pub(super) log_path: String,
    pub(super) lines: Vec<String>,
}

pub(super) fn start_runtime(
    fleet_root: &Path,
    name: &str,
) -> anyhow::Result<FleetRuntimeLifecycleResponse> {
    let (runtime, project_path) = resolve_manifest_runtime(fleet_root, name)?;
    let current = status_runtime_entry(fleet_root, &runtime, &project_path)?;
    if current.status == "running" {
        return Ok(FleetRuntimeLifecycleResponse {
            message: "Runtime is already running.".to_string(),
            ..current
        });
    }

    build_program_stbc(&project_path, None)
        .with_context(|| format!("failed to build {}", project_path.display()))?;

    let paths = lifecycle_paths(fleet_root, runtime.name.as_str());
    fs::create_dir_all(&paths.dir)
        .with_context(|| format!("failed to create {}", paths.dir.display()))?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .with_context(|| format!("failed to open {}", paths.log.display()))?;
    let stdout = log
        .try_clone()
        .with_context(|| format!("failed to clone {}", paths.log.display()))?;
    let mut child =
        Command::new(std::env::current_exe().context("failed to locate trust-runtime")?)
            .arg("run")
            .arg("--project")
            .arg(&project_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(log))
            .spawn()
            .with_context(|| format!("failed to start runtime '{}'", runtime.name))?;
    if let Err(error) = fs::write(&paths.pid, child.id().to_string()) {
        terminate_started_child(&mut child, &paths.pid);
        return Err(error).with_context(|| format!("failed to write {}", paths.pid.display()));
    }

    let mut last = current;
    for _ in 0..30 {
        let exit_status = match child.try_wait() {
            Ok(exit_status) => exit_status,
            Err(error) => {
                terminate_started_child(&mut child, &paths.pid);
                return Err(error)
                    .with_context(|| format!("failed to inspect runtime '{}'", runtime.name));
            }
        };
        if let Some(exit_status) = exit_status {
            remove_pid_file(fleet_root, runtime.name.as_str());
            anyhow::bail!(
                "runtime '{}' exited before its control endpoint became ready: {exit_status}",
                runtime.name
            );
        }
        last = match status_runtime_entry(fleet_root, &runtime, &project_path) {
            Ok(status) => status,
            Err(error) => {
                terminate_started_child(&mut child, &paths.pid);
                return Err(error).with_context(|| {
                    format!("failed to verify runtime '{}' startup", runtime.name)
                });
            }
        };
        if last.status == "running" {
            return Ok(FleetRuntimeLifecycleResponse {
                message: "Runtime started.".to_string(),
                ..last
            });
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(FleetRuntimeLifecycleResponse {
        status: "starting".to_string(),
        message: "Runtime process started, but the control endpoint is not reachable yet."
            .to_string(),
        ..last
    })
}

fn terminate_started_child(child: &mut Child, pid_path: &Path) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(pid_path);
}

pub(super) fn stop_runtime(
    fleet_root: &Path,
    name: &str,
) -> anyhow::Result<FleetRuntimeLifecycleResponse> {
    let (runtime, project_path) = resolve_manifest_runtime(fleet_root, name)?;
    let current = status_runtime_entry(fleet_root, &runtime, &project_path)?;
    if current.status != "running" {
        remove_pid_file(fleet_root, runtime.name.as_str());
        return Ok(FleetRuntimeLifecycleResponse {
            status: "stopped".to_string(),
            message: "Runtime was not running.".to_string(),
            ..current
        });
    }

    let auth = read_control_auth_token(&project_path)?;
    request_control(
        runtime.control_endpoint.as_str(),
        auth.as_deref(),
        "shutdown",
    )
    .with_context(|| format!("failed to stop runtime '{}'", runtime.name))?;

    let mut last = status_runtime_entry(fleet_root, &runtime, &project_path)?;
    for _ in 0..30 {
        if last.status != "running" {
            remove_pid_file(fleet_root, runtime.name.as_str());
            return Ok(FleetRuntimeLifecycleResponse {
                status: "stopped".to_string(),
                message: "Runtime stopped.".to_string(),
                ..last
            });
        }
        thread::sleep(Duration::from_millis(200));
        last = status_runtime_entry(fleet_root, &runtime, &project_path)?;
    }

    Ok(FleetRuntimeLifecycleResponse {
        status: "stopping".to_string(),
        message: "Stop was requested, but the control endpoint is still reachable.".to_string(),
        ..last
    })
}

pub(super) fn status_runtime(
    fleet_root: &Path,
    name: &str,
) -> anyhow::Result<FleetRuntimeLifecycleResponse> {
    let (runtime, project_path) = resolve_manifest_runtime(fleet_root, name)?;
    status_runtime_entry(fleet_root, &runtime, &project_path)
}

pub(super) fn runtime_logs(
    fleet_root: &Path,
    name: &str,
    lines: usize,
) -> anyhow::Result<FleetRuntimeLogsResponse> {
    let (runtime, _project_path) = resolve_manifest_runtime(fleet_root, name)?;
    let paths = lifecycle_paths(fleet_root, runtime.name.as_str());
    if lines == 0 {
        return Ok(FleetRuntimeLogsResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name,
            log_path: paths.log.display().to_string(),
            lines: Vec::new(),
        });
    }
    let file = match fs::File::open(&paths.log) {
        Ok(file) => Some(file),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", paths.log.display()))
        }
    };
    let mut tail = VecDeque::new();
    if let Some(file) = file {
        for line in BufReader::new(file).lines() {
            let line = line.with_context(|| format!("failed to read {}", paths.log.display()))?;
            if tail.len() == lines {
                tail.pop_front();
            }
            tail.push_back(line);
        }
    }
    Ok(FleetRuntimeLogsResponse {
        schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
        name: runtime.name,
        log_path: paths.log.display().to_string(),
        lines: tail.into_iter().collect(),
    })
}

struct RuntimeLifecyclePaths {
    dir: PathBuf,
    pid: PathBuf,
    log: PathBuf,
}

fn lifecycle_paths(fleet_root: &Path, name: &str) -> RuntimeLifecyclePaths {
    let dir = fleet_root.join(".trust-runtime");
    RuntimeLifecyclePaths {
        dir: dir.clone(),
        pid: dir.join(format!("{name}.pid")),
        log: dir.join(format!("{name}.log")),
    }
}

fn resolve_manifest_runtime(
    fleet_root: &Path,
    name: &str,
) -> anyhow::Result<(FleetManifestRuntime, PathBuf)> {
    let manifest = load_manifest(&fleet_root.join(FLEET_MANIFEST_FILE))?;
    let runtime = manifest
        .runtime
        .into_iter()
        .find(|runtime| runtime.name == name)
        .with_context(|| format!("fleet runtime '{name}' is not registered"))?;
    let path = runtime_project_path(fleet_root, runtime.path.as_str());
    Ok((runtime, path))
}

fn runtime_project_path(fleet_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        fleet_root.join(path)
    }
}

fn status_runtime_entry(
    fleet_root: &Path,
    runtime: &FleetManifestRuntime,
    project_path: &Path,
) -> anyhow::Result<FleetRuntimeLifecycleResponse> {
    let paths = lifecycle_paths(fleet_root, runtime.name.as_str());
    let pid = read_pid(&paths.pid).ok();
    let log_path = paths.log.display().to_string();
    let auth = read_control_auth_token(project_path)?;
    match exchange_control(runtime.control_endpoint.as_str(), auth.as_deref(), "status")? {
        ControlExchange::Response(result) => Ok(FleetRuntimeLifecycleResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name.clone(),
            path: project_path.display().to_string(),
            control_endpoint: runtime.control_endpoint.clone(),
            status: "running".to_string(),
            pid,
            log_path,
            message: "Runtime is reachable.".to_string(),
            control_status: Some(result),
        }),
        ControlExchange::Unreachable(error) => Ok(FleetRuntimeLifecycleResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name.clone(),
            path: project_path.display().to_string(),
            control_endpoint: runtime.control_endpoint.clone(),
            status: "stopped".to_string(),
            pid,
            log_path,
            message: format!("Runtime is not reachable: {error}"),
            control_status: None,
        }),
    }
}

fn read_pid(path: &Path) -> anyhow::Result<u32> {
    let text = fs::read_to_string(path)?;
    text.trim()
        .parse::<u32>()
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn remove_pid_file(fleet_root: &Path, name: &str) {
    let paths = lifecycle_paths(fleet_root, name);
    let _ = fs::remove_file(paths.pid);
}

fn read_control_auth_token(project_path: &Path) -> anyhow::Result<Option<String>> {
    let runtime_toml = project_path.join("runtime.toml");
    let text = fs::read_to_string(&runtime_toml)
        .with_context(|| format!("failed to read {}", runtime_toml.display()))?;
    let doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", runtime_toml.display()))?;
    let token = doc
        .get("runtime")
        .and_then(Item::as_table)
        .and_then(|runtime| runtime.get("control"))
        .and_then(Item::as_table)
        .and_then(|control| control.get("auth_token"))
        .and_then(Item::as_value);
    let token = match token {
        Some(value) => value
            .as_str()
            .map(|token| Some(token.to_owned()))
            .with_context(|| {
                format!(
                    "runtime.control.auth_token must be a string in {}",
                    runtime_toml.display()
                )
            }),
        None => Ok(None),
    }?;
    trust_runtime::config::validate_runtime_toml_text(text.as_str())
        .map_err(anyhow::Error::msg)
        .with_context(|| {
            format!(
                "invalid runtime configuration in {}",
                runtime_toml.display()
            )
        })?;
    Ok(token.map(|token| token.trim().to_owned()))
}

enum ControlExchange {
    Response(serde_json::Value),
    Unreachable(String),
}

fn request_control(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
) -> anyhow::Result<serde_json::Value> {
    match exchange_control(endpoint, auth_token, request_type)? {
        ControlExchange::Response(response) => Ok(response),
        ControlExchange::Unreachable(error) => anyhow::bail!("{error}"),
    }
}

fn exchange_control(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
) -> anyhow::Result<ControlExchange> {
    let endpoint = ControlEndpoint::parse(endpoint).map_err(anyhow::Error::msg)?;
    let payload = serde_json::json!({
        "id": 1,
        "type": request_type,
        "auth": auth_token,
    });
    let response = match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
                Ok(stream) => stream,
                Err(error) => {
                    return Ok(ControlExchange::Unreachable(format!(
                        "failed to connect to tcp://{addr}: {error}"
                    )))
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_millis(750)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(750)))
                .ok();
            writeln!(stream, "{payload}").context("failed to send control request")?;
            let mut reader = BufReader::new(stream);
            read_control_response(&mut reader)?
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let mut stream = match std::os::unix::net::UnixStream::connect(&path) {
                Ok(stream) => stream,
                Err(error) => {
                    return Ok(ControlExchange::Unreachable(format!(
                        "failed to connect to unix://{}: {error}",
                        path.display()
                    )))
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_millis(750)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(750)))
                .ok();
            writeln!(stream, "{payload}").context("failed to send control request")?;
            let mut reader = BufReader::new(stream);
            read_control_response(&mut reader)?
        }
    };
    let response: serde_json::Value =
        serde_json::from_str(response.trim()).context("control response was not JSON")?;
    if response.get("id").and_then(serde_json::Value::as_u64) != Some(1) {
        anyhow::bail!("control response id is missing or does not match request id 1");
    }
    if response
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(ControlExchange::Response(
            response
                .get("result")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        ));
    }
    let message = response
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("control request failed");
    anyhow::bail!("{message}");
}

fn read_control_response(reader: &mut impl BufRead) -> anyhow::Result<String> {
    let mut line = String::new();
    let bytes = reader
        .take((MAX_CONTROL_RESPONSE_BYTES + 1) as u64)
        .read_line(&mut line)
        .context("failed to read control response")?;
    if bytes == 0 {
        anyhow::bail!("control response was empty");
    }
    if bytes > MAX_CONTROL_RESPONSE_BYTES {
        anyhow::bail!(
            "control response exceeded {MAX_CONTROL_RESPONSE_BYTES} bytes without a complete line"
        );
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::cli::FleetRuntimeTemplateArg;

    use super::super::add_runtime;
    use super::*;

    #[test]
    fn fleet_runtime_status_reports_stopped_when_endpoint_unreachable() {
        let root = temp_dir("fleet-runtime-status-stopped");
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(19916),
            Some(19986),
        )
        .expect("add runtime");

        let status = status_runtime(&root, "cell").expect("status runtime");
        assert_eq!(status.name, "cell");
        assert_eq!(status.status, "stopped");
        assert!(status.control_status.is_none());
        assert!(
            status.message.contains("not reachable"),
            "status should explain stopped/unreachable state: {}",
            status.message
        );
        assert!(
            !root.join(".trust-runtime").exists(),
            "read-only status must not create lifecycle state"
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_status_rejects_malformed_runtime_auth_config() {
        let root = temp_dir("fleet-runtime-status-invalid-auth");
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(19918),
            Some(19988),
        )
        .expect("add runtime");
        let valid_runtime_text =
            fs::read_to_string(root.join("cell/runtime.toml")).expect("read valid runtime config");
        fs::write(root.join("cell/runtime.toml"), "not valid TOML = [")
            .expect("corrupt runtime config");

        let error = status_runtime(&root, "cell")
            .expect_err("invalid runtime auth config must not report stopped");

        assert!(
            error.to_string().contains("failed to parse"),
            "unexpected error: {error:#}"
        );

        fs::write(
            root.join("cell/runtime.toml"),
            "[runtime.control]\nauth_token = 42\n",
        )
        .expect("write non-string runtime auth config");
        let error = status_runtime(&root, "cell")
            .expect_err("non-string runtime auth token must not report stopped");
        assert!(
            error
                .to_string()
                .contains("runtime.control.auth_token must be a string"),
            "unexpected error: {error:#}"
        );

        let mut blank_token = valid_runtime_text
            .parse::<DocumentMut>()
            .expect("parse valid runtime config");
        blank_token["runtime"]["control"]["auth_token"] = toml_edit::value("  ");
        fs::write(root.join("cell/runtime.toml"), blank_token.to_string())
            .expect("write blank runtime auth token");
        let error = status_runtime(&root, "cell")
            .expect_err("blank runtime auth token must not report stopped");
        assert!(
            format!("{error:#}").contains("auth_token"),
            "unexpected error: {error:#}"
        );

        let mut missing_token = valid_runtime_text
            .parse::<DocumentMut>()
            .expect("parse valid runtime config");
        missing_token["runtime"]["control"]
            .as_table_mut()
            .expect("runtime control table")
            .remove("auth_token");
        fs::write(root.join("cell/runtime.toml"), missing_token.to_string())
            .expect("write missing runtime auth token");
        let error = status_runtime(&root, "cell")
            .expect_err("missing TCP auth token must not report stopped");
        assert!(
            format!("{error:#}").contains("auth_token"),
            "unexpected error: {error:#}"
        );

        fs::remove_file(root.join("cell/runtime.toml")).expect("remove runtime auth config");
        let error = status_runtime(&root, "cell")
            .expect_err("missing runtime auth config must not report stopped");
        assert!(
            error.to_string().contains("failed to read"),
            "unexpected error: {error:#}"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_auth_reader_matches_runtime_token_normalization() {
        let root = temp_dir("fleet-runtime-auth-normalization");
        let control_port = free_loopback_port();
        let web_port = distinct_free_loopback_port(control_port);
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(control_port),
            Some(web_port),
        )
        .expect("add runtime");
        let runtime_path = root.join("cell/runtime.toml");
        let mut runtime = fs::read_to_string(&runtime_path)
            .expect("read runtime config")
            .parse::<DocumentMut>()
            .expect("parse runtime config");
        runtime["runtime"]["control"]["auth_token"] = toml_edit::value("  padded-token  ");
        fs::write(&runtime_path, runtime.to_string()).expect("write padded auth token");

        let token =
            read_control_auth_token(&root.join("cell")).expect("read normalized auth token");

        assert_eq!(token.as_deref(), Some("padded-token"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_status_rejects_a_connected_control_error() {
        let root = temp_dir("fleet-runtime-status-control-error");
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve control port");
        let port = probe.local_addr().expect("control listener address").port();
        drop(probe);
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(port),
            Some(19989),
        )
        .expect("add runtime");
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", port)).expect("bind fake control endpoint");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept status request");
            let mut request = String::new();
            BufReader::new(stream.try_clone().expect("clone fake stream"))
                .read_line(&mut request)
                .expect("read status request");
            writeln!(
                stream,
                "{}",
                serde_json::json!({"id": 1, "ok": false, "error": "unauthorized"})
            )
            .expect("write rejected response");
        });

        let error = status_runtime(&root, "cell")
            .expect_err("connected rejection must not be flattened to stopped");
        server.join().expect("join fake control server");

        assert!(
            error.to_string().contains("unauthorized"),
            "unexpected error: {error:#}"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn control_request_rejects_a_mismatched_response_id() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind control listener");
        let address = listener.local_addr().expect("control listener address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control request");
            let mut request = String::new();
            BufReader::new(stream.try_clone().expect("clone fake stream"))
                .read_line(&mut request)
                .expect("read control request");
            writeln!(
                stream,
                "{}",
                serde_json::json!({"id": 99, "ok": true, "result": {}})
            )
            .expect("write mismatched response");
        });

        let error = request_control(format!("tcp://{address}").as_str(), Some("token"), "status")
            .expect_err("mismatched response id must fail");
        server.join().expect("join fake control server");

        assert!(
            error.to_string().contains("response id"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn control_request_rejects_malformed_or_ambiguous_response_envelopes() {
        for response in [
            "not-json",
            r#"{"id":1}"#,
            r#"{"id":1,"ok":"true","result":{}}"#,
        ] {
            let error = control_error_for_response(response);
            assert!(
                error.contains("not JSON") || error.contains("control request failed"),
                "unexpected error for {response:?}: {error}"
            );
        }
    }

    #[test]
    fn control_response_reader_rejects_an_oversized_line() {
        let bytes = vec![b'x'; MAX_CONTROL_RESPONSE_BYTES + 1];
        let mut reader = std::io::Cursor::new(bytes);

        let error = read_control_response(&mut reader)
            .expect_err("oversized control response must be rejected");

        assert!(
            error.to_string().contains("exceeded"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn fleet_runtime_logs_return_requested_tail() {
        let root = temp_dir("fleet-runtime-logs");
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(19917),
            Some(19987),
        )
        .expect("add runtime");
        let paths = lifecycle_paths(&root, "cell");
        fs::create_dir_all(&paths.dir).expect("create lifecycle state directory");
        fs::write(&paths.log, "one\ntwo\nthree\n").expect("write log");

        let logs = runtime_logs(&root, "cell", 2).expect("runtime logs");
        assert_eq!(logs.lines, vec!["two".to_string(), "three".to_string()]);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_runtime_logs_are_empty_without_creating_state() {
        let root = temp_dir("fleet-runtime-logs-missing");
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(19919),
            Some(19990),
        )
        .expect("add runtime");

        let logs = runtime_logs(&root, "cell", 10).expect("read absent logs");

        assert!(logs.lines.is_empty());
        assert!(
            !root.join(".trust-runtime").exists(),
            "read-only logs must not create lifecycle state"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_status_uses_endpoint_evidence_not_a_stale_pid() {
        let root = temp_dir("fleet-runtime-status-stale-pid");
        let control_port = free_loopback_port();
        let web_port = distinct_free_loopback_port(control_port);
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(control_port),
            Some(web_port),
        )
        .expect("add runtime");
        let paths = lifecycle_paths(&root, "cell");
        fs::create_dir_all(&paths.dir).expect("create lifecycle directory");
        fs::write(&paths.pid, "4242").expect("write stale PID");

        let status = status_runtime(&root, "cell").expect("status runtime");

        assert_eq!(status.status, "stopped");
        assert_eq!(status.pid, Some(4242));
        assert!(status.control_status.is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn managed_runtime_project_paths_resolve_relative_and_preserve_absolute_paths() {
        let root = Path::new("fleet-root");
        assert_eq!(
            runtime_project_path(root, "cell"),
            root.join("cell"),
            "relative manifest paths resolve beneath the fleet root"
        );

        let absolute = std::env::current_dir()
            .expect("current directory")
            .join("external-managed-runtime");
        assert_eq!(
            runtime_project_path(root, absolute.to_string_lossy().as_ref()),
            absolute,
            "absolute hand-edited manifest paths remain absolute"
        );
    }

    #[test]
    fn fleet_runtime_start_removes_pid_when_child_exits_before_ready() {
        let root = temp_dir("fleet-runtime-start-early-exit");
        let control_port = free_loopback_port();
        let web_port = distinct_free_loopback_port(control_port);
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(control_port),
            Some(web_port),
        )
        .expect("add runtime");

        let error =
            start_runtime(&root, "cell").expect_err("test-harness child must exit before ready");

        assert!(
            error
                .to_string()
                .contains("exited before its control endpoint became ready"),
            "unexpected error: {error:#}"
        );
        assert!(
            !lifecycle_paths(&root, "cell").pid.exists(),
            "early child exit must remove the PID file"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_start_aborts_when_pid_persistence_fails() {
        let root = temp_dir("fleet-runtime-start-pid-failure");
        let control_port = free_loopback_port();
        let web_port = distinct_free_loopback_port(control_port);
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            Some(control_port),
            Some(web_port),
        )
        .expect("add runtime");
        let paths = lifecycle_paths(&root, "cell");
        fs::create_dir_all(&paths.pid).expect("make PID path unwritable as a file");

        let error =
            start_runtime(&root, "cell").expect_err("PID persistence failure must abort start");

        assert!(
            error.to_string().contains("failed to write"),
            "unexpected error: {error:#}"
        );
        fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn terminate_started_child_kills_and_reaps_a_live_process() {
        let root = temp_dir("fleet-runtime-terminate-child");
        let pid_path = root.join("child.pid");
        fs::write(&pid_path, "advisory").expect("write advisory PID file");
        let mut child = Command::new("sh")
            .args(["-c", "while :; do sleep 1; done"])
            .spawn()
            .expect("spawn live child");
        std::thread::sleep(Duration::from_millis(50));
        assert!(
            child.try_wait().expect("inspect live child").is_none(),
            "fixture child must still be live before cleanup"
        );

        terminate_started_child(&mut child, &pid_path);

        assert!(
            child.try_wait().expect("inspect reaped child").is_some(),
            "cleanup must leave the child terminated and reaped"
        );
        assert!(!pid_path.exists(), "cleanup must remove advisory PID state");
        fs::remove_dir_all(root).ok();
    }

    fn control_error_for_response(response: &str) -> String {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("bind fake control listener");
        let address = listener.local_addr().expect("read fake control address");
        let response = response.to_owned();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control request");
            let mut request = String::new();
            BufReader::new(stream.try_clone().expect("clone fake stream"))
                .read_line(&mut request)
                .expect("read control request");
            writeln!(stream, "{response}").expect("write fake response");
        });

        let error = request_control(format!("tcp://{address}").as_str(), Some("token"), "status")
            .expect_err("invalid response must fail");
        server.join().expect("join fake control server");
        format!("{error:#}")
    }

    fn free_loopback_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral loopback port")
            .local_addr()
            .expect("read ephemeral loopback port")
            .port()
    }

    fn distinct_free_loopback_port(other: u16) -> u16 {
        loop {
            let port = free_loopback_port();
            if port != other {
                return port;
            }
        }
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }
}
