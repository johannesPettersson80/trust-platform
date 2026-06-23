use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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
    let current = status_runtime_entry(fleet_root, &runtime, &project_path);
    if current.status == "running" {
        return Ok(FleetRuntimeLifecycleResponse {
            message: "Runtime is already running.".to_string(),
            ..current
        });
    }

    build_program_stbc(&project_path, None)
        .with_context(|| format!("failed to build {}", project_path.display()))?;

    let paths = lifecycle_paths(fleet_root, runtime.name.as_str())?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .with_context(|| format!("failed to open {}", paths.log.display()))?;
    let stdout = log
        .try_clone()
        .with_context(|| format!("failed to clone {}", paths.log.display()))?;
    let child = Command::new(std::env::current_exe().context("failed to locate trust-runtime")?)
        .arg("run")
        .arg("--project")
        .arg(&project_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log))
        .spawn()
        .with_context(|| format!("failed to start runtime '{}'", runtime.name))?;
    fs::write(&paths.pid, child.id().to_string())
        .with_context(|| format!("failed to write {}", paths.pid.display()))?;

    let mut last = status_runtime_entry(fleet_root, &runtime, &project_path);
    for _ in 0..30 {
        if last.status == "running" {
            return Ok(FleetRuntimeLifecycleResponse {
                message: "Runtime started.".to_string(),
                ..last
            });
        }
        thread::sleep(Duration::from_millis(200));
        last = status_runtime_entry(fleet_root, &runtime, &project_path);
    }
    Ok(FleetRuntimeLifecycleResponse {
        status: "starting".to_string(),
        message: "Runtime process started, but the control endpoint is not reachable yet."
            .to_string(),
        ..last
    })
}

pub(super) fn stop_runtime(
    fleet_root: &Path,
    name: &str,
) -> anyhow::Result<FleetRuntimeLifecycleResponse> {
    let (runtime, project_path) = resolve_manifest_runtime(fleet_root, name)?;
    let current = status_runtime_entry(fleet_root, &runtime, &project_path);
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

    let mut last = status_runtime_entry(fleet_root, &runtime, &project_path);
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
        last = status_runtime_entry(fleet_root, &runtime, &project_path);
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
    Ok(status_runtime_entry(fleet_root, &runtime, &project_path))
}

pub(super) fn runtime_logs(
    fleet_root: &Path,
    name: &str,
    lines: usize,
) -> anyhow::Result<FleetRuntimeLogsResponse> {
    let (runtime, _project_path) = resolve_manifest_runtime(fleet_root, name)?;
    let paths = lifecycle_paths(fleet_root, runtime.name.as_str())?;
    let text = match fs::read_to_string(&paths.log) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", paths.log.display()))
        }
    };
    let all = text.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let keep = lines.min(all.len());
    let start = all.len().saturating_sub(keep);
    Ok(FleetRuntimeLogsResponse {
        schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
        name: runtime.name,
        log_path: paths.log.display().to_string(),
        lines: all[start..].to_vec(),
    })
}

struct RuntimeLifecyclePaths {
    pid: PathBuf,
    log: PathBuf,
}

fn lifecycle_paths(fleet_root: &Path, name: &str) -> anyhow::Result<RuntimeLifecyclePaths> {
    let dir = fleet_root.join(".trust-runtime");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    Ok(RuntimeLifecyclePaths {
        pid: dir.join(format!("{name}.pid")),
        log: dir.join(format!("{name}.log")),
    })
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
) -> FleetRuntimeLifecycleResponse {
    let paths = lifecycle_paths(fleet_root, runtime.name.as_str()).ok();
    let pid = paths.as_ref().and_then(|paths| read_pid(&paths.pid).ok());
    let log_path = paths
        .as_ref()
        .map(|paths| paths.log.display().to_string())
        .unwrap_or_default();
    let auth = read_control_auth_token(project_path).ok().flatten();
    match request_control(runtime.control_endpoint.as_str(), auth.as_deref(), "status") {
        Ok(result) => FleetRuntimeLifecycleResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name.clone(),
            path: project_path.display().to_string(),
            control_endpoint: runtime.control_endpoint.clone(),
            status: "running".to_string(),
            pid,
            log_path,
            message: "Runtime is reachable.".to_string(),
            control_status: Some(result),
        },
        Err(error) => FleetRuntimeLifecycleResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name.clone(),
            path: project_path.display().to_string(),
            control_endpoint: runtime.control_endpoint.clone(),
            status: "stopped".to_string(),
            pid,
            log_path,
            message: format!("Runtime is not reachable: {error}"),
            control_status: None,
        },
    }
}

fn read_pid(path: &Path) -> anyhow::Result<u32> {
    let text = fs::read_to_string(path)?;
    text.trim()
        .parse::<u32>()
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn remove_pid_file(fleet_root: &Path, name: &str) {
    if let Ok(paths) = lifecycle_paths(fleet_root, name) {
        let _ = fs::remove_file(paths.pid);
    }
}

fn read_control_auth_token(project_path: &Path) -> anyhow::Result<Option<String>> {
    let runtime_toml = project_path.join("runtime.toml");
    let text = fs::read_to_string(&runtime_toml)
        .with_context(|| format!("failed to read {}", runtime_toml.display()))?;
    let doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", runtime_toml.display()))?;
    Ok(doc
        .get("runtime")
        .and_then(Item::as_table)
        .and_then(|runtime| runtime.get("control"))
        .and_then(Item::as_table)
        .and_then(|control| control.get("auth_token"))
        .and_then(Item::as_value)
        .and_then(toml_edit::Value::as_str)
        .map(ToOwned::to_owned))
}

fn request_control(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
) -> anyhow::Result<serde_json::Value> {
    let endpoint = ControlEndpoint::parse(endpoint).map_err(anyhow::Error::msg)?;
    let payload = serde_json::json!({
        "id": 1,
        "type": request_type,
        "auth": auth_token,
    });
    let response = match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500))
                .with_context(|| format!("failed to connect to tcp://{addr}"))?;
            stream
                .set_read_timeout(Some(Duration::from_millis(750)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(750)))
                .ok();
            writeln!(stream, "{payload}").context("failed to send control request")?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .context("failed to read control response")?;
            line
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let mut stream = std::os::unix::net::UnixStream::connect(&path)
                .with_context(|| format!("failed to connect to unix://{}", path.display()))?;
            stream
                .set_read_timeout(Some(Duration::from_millis(750)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(750)))
                .ok();
            writeln!(stream, "{payload}").context("failed to send control request")?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .context("failed to read control response")?;
            line
        }
    };
    let response: serde_json::Value =
        serde_json::from_str(response.trim()).context("control response was not JSON")?;
    if response
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(response
            .get("result")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})));
    }
    let message = response
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("control request failed");
    anyhow::bail!("{message}");
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

        fs::remove_dir_all(root).ok();
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
        let paths = lifecycle_paths(&root, "cell").expect("lifecycle paths");
        fs::write(&paths.log, "one\ntwo\nthree\n").expect("write log");

        let logs = runtime_logs(&root, "cell", 2).expect("runtime logs");
        assert_eq!(logs.lines, vec!["two".to_string(), "three".to_string()]);

        fs::remove_dir_all(root).ok();
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
