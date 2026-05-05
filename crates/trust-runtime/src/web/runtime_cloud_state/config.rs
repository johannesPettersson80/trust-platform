use super::*;
use crate::runtime_cloud::config_policy::{self, RuntimeCloudConfigReconcileAction};

pub(in crate::web) fn runtime_cloud_config_state_path(
    bundle_root: Option<&PathBuf>,
) -> Option<PathBuf> {
    let root = bundle_root?;
    Some(
        root.join(".trust")
            .join("runtime-cloud")
            .join("config-agent-state.json"),
    )
}

pub(in crate::web) fn runtime_cloud_config_load_state(
    path: Option<&Path>,
) -> RuntimeCloudConfigAgentState {
    let Some(path) = path else {
        return runtime_cloud_config_initial_state();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return runtime_cloud_config_initial_state();
    };
    match serde_json::from_str::<RuntimeCloudConfigAgentState>(&text) {
        Ok(state) => state,
        Err(err) => runtime_cloud_corrupt_config_state(path, &err),
    }
}

fn runtime_cloud_corrupt_config_state(
    path: &Path,
    err: &serde_json::Error,
) -> RuntimeCloudConfigAgentState {
    let mut state = runtime_cloud_config_initial_state();
    state.status.state = crate::runtime_cloud::contracts::ConfigState::Error;
    state.status.blocked_reason = Some(ReasonCode::ContractViolation);
    state.status.required_action = Some("repair_runtime_cloud_state".to_string());
    state.status.errors.push(format!(
        "corrupt persisted runtime-cloud config state '{}': {err}",
        path.display()
    ));
    state
}

pub(in crate::web) fn runtime_cloud_config_store_state(
    path: Option<&Path>,
    state: &RuntimeCloudConfigAgentState,
) {
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, text);
    }
}

pub(in crate::web) fn runtime_cloud_config_initial_state() -> RuntimeCloudConfigAgentState {
    config_policy::runtime_cloud_config_initial_state(now_ns())
}

pub(in crate::web) fn runtime_cloud_config_snapshot(
    state: &Mutex<RuntimeCloudConfigAgentState>,
    runtime_id: &str,
) -> RuntimeCloudConfigSnapshot {
    let current = state
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| runtime_cloud_config_initial_state());
    config_policy::runtime_cloud_config_snapshot(current, runtime_id)
}

pub(in crate::web) fn runtime_cloud_config_write_desired(
    state: &Mutex<RuntimeCloudConfigAgentState>,
    payload: &RuntimeCloudDesiredWriteRequest,
    persist_path: Option<&Path>,
) -> Result<RuntimeCloudConfigAgentState, RuntimeCloudConfigWriteError> {
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => {
            let snapshot = runtime_cloud_config_initial_state();
            return Err(RuntimeCloudConfigWriteError {
                code: ReasonCode::TransportFailure,
                message: "config agent state unavailable".to_string(),
                snapshot: Box::new(snapshot),
            });
        }
    };
    let result = config_policy::runtime_cloud_config_write_desired(
        &mut guard,
        payload.actor.as_str(),
        &payload.desired,
        payload.expected_revision,
        payload.expected_etag.as_deref(),
        now_ns(),
    );
    if result.is_ok()
        || matches!(result.as_ref(), Err(error) if error.code == ReasonCode::RevisionConflict)
    {
        runtime_cloud_config_store_state(persist_path, &guard);
    }
    result
}

pub(in crate::web) fn runtime_cloud_config_reconcile_once(
    state: &Mutex<RuntimeCloudConfigAgentState>,
    control_state: &ControlState,
    persist_path: Option<&Path>,
) {
    let apply_request = {
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        match config_policy::runtime_cloud_config_prepare_reconcile(&mut guard) {
            RuntimeCloudConfigReconcileAction::Idle { should_store } => {
                if should_store {
                    runtime_cloud_config_store_state(persist_path, &guard);
                }
                return;
            }
            RuntimeCloudConfigReconcileAction::Apply(request) => request,
        }
    };

    let control_payload = json!({
        "id": 1_u64,
        "type": "config.set",
        "request_id": format!("cfg-agent-{}", apply_request.desired_revision),
        "params": apply_request.desired,
    });
    let control_response = dispatch_control_request(
        control_payload,
        control_state,
        Some("runtime-cloud-config-agent"),
        None,
    );
    let response_value = serde_json::to_value(control_response).unwrap_or_else(
        |_| json!({ "ok": false, "error": "config apply response serialization failed" }),
    );
    let ok = response_value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if ok {
        config_policy::runtime_cloud_config_apply_success(
            &mut guard,
            apply_request.desired_revision,
            apply_request.desired_etag,
            now_ns(),
        );
        runtime_cloud_config_store_state(persist_path, &guard);
        return;
    }

    let error_text = response_value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("config apply failed")
        .to_string();
    config_policy::runtime_cloud_config_apply_failure(&mut guard, error_text);
    runtime_cloud_config_store_state(persist_path, &guard);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_state_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("trust-runtime-cloud-{name}-{stamp}.json"))
    }

    #[test]
    #[ignore = "red test for runtime-safety fail-closed Phase 8"]
    fn runtime_cloud_corrupt_config_state_does_not_reset_to_default() {
        let path = temp_state_path("corrupt-config");
        std::fs::write(&path, "{not valid json").expect("write corrupt state");

        let state = runtime_cloud_config_load_state(Some(path.as_path()));

        assert_eq!(
            state.status.state,
            crate::runtime_cloud::contracts::ConfigState::Error,
            "corrupt persisted config state must become an explicit error state, not default InSync"
        );
        assert!(
            state
                .status
                .errors
                .iter()
                .any(|error| error.contains("corrupt")),
            "corrupt persisted config state must keep parse-failure evidence: {:?}",
            state.status.errors
        );

        let _ = std::fs::remove_file(path);
    }
}
