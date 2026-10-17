use super::*;
use crate::control::endpoint::{project_control_endpoint_text, remove_endpoint_file_of};

struct ConfigUiManagedRuntimeProcess {
    listen: String,
    child: Child,
    started_at_ns: u64,
    /// The project folder, whose endpoint file the runtime cannot remove when it is killed.
    root: PathBuf,
}

#[derive(Default)]
struct ConfigUiLifecycleManagerState {
    managed: BTreeMap<String, ConfigUiManagedRuntimeProcess>,
}

static CONFIG_UI_LIFECYCLE_MANAGER: OnceLock<Mutex<ConfigUiLifecycleManagerState>> =
    OnceLock::new();

fn config_ui_lifecycle_manager() -> &'static Mutex<ConfigUiLifecycleManagerState> {
    CONFIG_UI_LIFECYCLE_MANAGER.get_or_init(|| Mutex::new(ConfigUiLifecycleManagerState::default()))
}

fn control_endpoint_online(endpoint: &str) -> bool {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(rest) = trimmed.strip_prefix("tcp://") {
        let mut socket_addrs = match rest.to_socket_addrs() {
            Ok(value) => value,
            Err(_) => return false,
        };
        if let Some(addr) = socket_addrs.next() {
            return std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok();
        }
        return false;
    }
    #[cfg(unix)]
    if let Some(path) = trimmed.strip_prefix("unix://") {
        if path.trim().is_empty() {
            return false;
        }
        return std::os::unix::net::UnixStream::connect(path).is_ok();
    }
    false
}

/// Whether the runtime's control endpoint answers. The endpoint is the one a client would use:
/// the endpoint file, else the configured endpoint, else the platform default (spec 11, 6.8.0a).
fn runtime_endpoint_online(runtime: &WorkspaceRuntime) -> bool {
    project_control_endpoint_text(&runtime.root, runtime.runtime.control_endpoint.as_deref())
        .is_some_and(|endpoint| control_endpoint_online(&endpoint))
}

fn prune_managed_runtime_processes(manager: &mut ConfigUiLifecycleManagerState) {
    manager
        .managed
        .retain(|_, process| match process.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        });
}

fn managed_runtime_pid(process: &ConfigUiManagedRuntimeProcess) -> u32 {
    process.child.id()
}

fn runtime_lifecycle_item(
    runtime: &WorkspaceRuntime,
    process: Option<&ConfigUiManagedRuntimeProcess>,
) -> serde_json::Value {
    let host_group = runtime
        .runtime
        .discovery
        .host_group
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let externally_running = runtime_endpoint_online(runtime);
    let (managed, managed_running, pid, started_at_ns, listen) = if let Some(process) = process {
        (
            true,
            true,
            Some(managed_runtime_pid(process)),
            Some(process.started_at_ns),
            process.listen.clone(),
        )
    } else {
        (
            false,
            false,
            None,
            None,
            runtime.runtime.web.listen.to_string(),
        )
    };
    let running = managed_running || externally_running;
    json!({
        "runtime_id": runtime.runtime_id,
        "runtime_root": runtime.root.display().to_string(),
        "host_group": host_group,
        "control_endpoint": project_control_endpoint_text(&runtime.root, runtime.runtime.control_endpoint.as_deref()),
        "web_listen": listen,
        "managed": managed,
        "running": running,
        "externally_running": externally_running,
        "pid": pid,
        "started_at_ns": started_at_ns,
    })
}

pub(super) fn config_ui_runtime_lifecycle_snapshot(
    workspace: &WorkspaceModel,
    _request_token: Option<&str>,
) -> Result<Vec<serde_json::Value>, RuntimeError> {
    let mut guard = config_ui_lifecycle_manager().lock().map_err(|_| {
        RuntimeError::ControlError("failed to lock config-ui lifecycle manager".into())
    })?;
    prune_managed_runtime_processes(&mut guard);
    let mut items = Vec::with_capacity(workspace.runtimes.len());
    for runtime in &workspace.runtimes {
        let process = guard.managed.get(runtime.runtime_id.as_str());
        items.push(runtime_lifecycle_item(runtime, process));
    }
    Ok(items)
}

fn launch_runtime_for_workspace_runtime(
    runtime: &WorkspaceRuntime,
) -> Result<ConfigUiManagedRuntimeProcess, RuntimeError> {
    let exe = std::env::current_exe().map_err(|error| {
        RuntimeError::ControlError(
            format!("failed to resolve trust-runtime executable path: {error}").into(),
        )
    })?;
    let mut command = Command::new(exe);
    command
        .arg("play")
        .arg("--project")
        .arg(runtime.root.as_os_str())
        .arg("--no-console")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn().map_err(|error| {
        RuntimeError::ControlError(
            format!(
                "failed to start runtime '{}' from config-ui: {error}",
                runtime.runtime_id
            )
            .into(),
        )
    })?;
    Ok(ConfigUiManagedRuntimeProcess {
        listen: runtime.runtime.web.listen.to_string(),
        child,
        started_at_ns: now_ns(),
        root: runtime.root.clone(),
    })
}

fn stop_managed_runtime_process(
    process: &mut ConfigUiManagedRuntimeProcess,
) -> Result<(), RuntimeError> {
    process.child.kill().map_err(|error| {
        RuntimeError::ControlError(
            format!("failed to stop managed runtime process: {error}").into(),
        )
    })?;
    let _ = process.child.wait();
    // A killed runtime cannot remove its endpoint file, so the file that names the killed process
    // is removed here and no client finds a runtime that is gone (spec 11, section 6.8.0a).
    remove_endpoint_file_of(&process.root, process.child.id());
    Ok(())
}

pub(super) fn config_ui_runtime_lifecycle_apply(
    workspace: &WorkspaceModel,
    payload: &ConfigRuntimeLifecycleRequest,
    _request_token: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let runtime_id = normalize_runtime_id(payload.runtime_id.as_str())?;
    let runtime = resolve_runtime_by_id(workspace, runtime_id.as_str())?;
    let action = payload.action.trim().to_ascii_lowercase();
    if action.is_empty() {
        return Err(RuntimeError::InvalidConfig("action is required".into()));
    }
    let mut guard = config_ui_lifecycle_manager().lock().map_err(|_| {
        RuntimeError::ControlError("failed to lock config-ui lifecycle manager".into())
    })?;
    prune_managed_runtime_processes(&mut guard);

    let externally_running = runtime_endpoint_online(runtime);
    let managed_present = guard.managed.contains_key(runtime_id.as_str());
    let result = match action.as_str() {
        "start" => {
            if managed_present || externally_running {
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "already_running",
                })
            } else {
                let process = launch_runtime_for_workspace_runtime(runtime)?;
                let pid = managed_runtime_pid(&process);
                guard.managed.insert(runtime_id.clone(), process);
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "started",
                    "pid": pid,
                })
            }
        }
        "stop" => {
            if let Some(mut process) = guard.managed.remove(runtime_id.as_str()) {
                stop_managed_runtime_process(&mut process)?;
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "stopped",
                })
            } else if externally_running {
                return Err(RuntimeError::InvalidConfig(
                    format!(
                        "runtime '{runtime_id}' is running but not managed by config-ui; stop it via runtime control endpoint"
                    )
                    .into(),
                ));
            } else {
                json!({
                    "ok": true,
                    "runtime_id": runtime_id,
                    "action": action,
                    "result": "already_stopped",
                })
            }
        }
        "restart" => {
            if let Some(mut process) = guard.managed.remove(runtime_id.as_str()) {
                let _ = stop_managed_runtime_process(&mut process);
            } else if externally_running {
                return Err(RuntimeError::InvalidConfig(
                    format!(
                        "runtime '{runtime_id}' is running but not managed by config-ui; restart it via runtime control endpoint"
                    )
                    .into(),
                ));
            }
            let process = launch_runtime_for_workspace_runtime(runtime)?;
            let pid = managed_runtime_pid(&process);
            guard.managed.insert(runtime_id.clone(), process);
            json!({
                "ok": true,
                "runtime_id": runtime_id,
                "action": action,
                "result": "restarted",
                "pid": pid,
            })
        }
        "status" | "probe" => json!({
            "ok": true,
            "runtime_id": runtime_id,
            "action": action,
            "result": if managed_present || externally_running { "running" } else { "stopped" },
        }),
        _ => {
            return Err(RuntimeError::InvalidConfig(
                format!("unsupported lifecycle action '{}'", payload.action).into(),
            ))
        }
    };

    prune_managed_runtime_processes(&mut guard);
    let process = guard.managed.get(runtime_id.as_str());
    Ok(json!({
        "ok": true,
        "result": result,
        "item": runtime_lifecycle_item(runtime, process),
        "requested_mode": payload.mode.as_deref().unwrap_or(""),
    }))
}

#[cfg(test)]
#[path = "lifecycle/contract_tests.rs"]
mod contract_tests;
