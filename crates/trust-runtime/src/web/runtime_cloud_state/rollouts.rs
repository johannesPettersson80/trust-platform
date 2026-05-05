use super::*;
use crate::runtime_cloud::rollout_policy;

pub(in crate::web) fn runtime_cloud_rollouts_state_path(
    bundle_root: Option<&PathBuf>,
) -> Option<PathBuf> {
    let root = bundle_root?;
    Some(
        root.join(".trust")
            .join("runtime-cloud")
            .join("rollouts-state.json"),
    )
}

pub(in crate::web) fn runtime_cloud_rollouts_load_state(
    path: Option<&Path>,
) -> Result<RuntimeCloudRolloutManagerState, RuntimeError> {
    let Some(path) = path else {
        return Ok(RuntimeCloudRolloutManagerState::default());
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RuntimeCloudRolloutManagerState::default());
        }
        Err(err) => {
            return Err(RuntimeError::ControlError(
                format!(
                    "load runtime-cloud rollouts state '{}': {err}",
                    path.display()
                )
                .into(),
            ));
        }
    };
    serde_json::from_str::<RuntimeCloudRolloutManagerState>(&text).map_err(|err| {
        RuntimeError::ControlError(
            format!(
                "corrupt persisted runtime-cloud rollouts state '{}': {err}",
                path.display()
            )
            .into(),
        )
    })
}

pub(in crate::web) fn runtime_cloud_rollouts_store_state(
    path: Option<&Path>,
    state: &RuntimeCloudRolloutManagerState,
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

pub(in crate::web) fn runtime_cloud_rollouts_snapshot(
    rollouts: &Mutex<RuntimeCloudRolloutManagerState>,
) -> Vec<RuntimeCloudRolloutRecord> {
    rollouts
        .lock()
        .map(|guard| rollout_policy::runtime_cloud_rollouts_snapshot(&guard))
        .unwrap_or_default()
}

pub(in crate::web) fn runtime_cloud_rollout_create(
    rollouts: &Mutex<RuntimeCloudRolloutManagerState>,
    config: &Mutex<RuntimeCloudConfigAgentState>,
    payload: &RuntimeCloudRolloutCreateRequest,
    persist_path: Option<&Path>,
) -> Result<RuntimeCloudRolloutRecord, (ReasonCode, String)> {
    let config_snapshot = config
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| runtime_cloud_config_initial_state());
    let mut guard = rollouts.lock().map_err(|_| {
        (
            ReasonCode::TransportFailure,
            "rollout state unavailable".to_string(),
        )
    })?;
    let rollout = rollout_policy::runtime_cloud_rollout_create(
        &mut guard,
        payload.api_version.as_str(),
        payload.actor.as_str(),
        payload.target_runtimes.as_slice(),
        payload.desired_revision,
        config_snapshot.meta.desired_revision,
        now_ns(),
    )?;
    runtime_cloud_rollouts_store_state(persist_path, &guard);
    Ok(rollout)
}

pub(in crate::web) fn runtime_cloud_rollout_apply_action(
    rollouts: &Mutex<RuntimeCloudRolloutManagerState>,
    rollout_id: &str,
    action: &str,
    persist_path: Option<&Path>,
) -> RuntimeCloudRolloutActionResponse {
    let mut guard = match rollouts.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return RuntimeCloudRolloutActionResponse {
                ok: false,
                action: action.to_string(),
                denial_code: Some(ReasonCode::TransportFailure),
                error: Some("rollout state unavailable".to_string()),
                rollout: None,
            };
        }
    };
    let response = rollout_policy::runtime_cloud_rollout_apply_action(
        &mut guard,
        rollout_id,
        action,
        now_ns(),
    );
    runtime_cloud_rollouts_store_state(persist_path, &guard);
    response
}

pub(in crate::web) fn runtime_cloud_rollouts_reconcile_once(
    rollouts: &Mutex<RuntimeCloudRolloutManagerState>,
    config: &Mutex<RuntimeCloudConfigAgentState>,
    persist_path: Option<&Path>,
) {
    let config_snapshot = config
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| runtime_cloud_config_initial_state());
    let config_view = runtime_cloud_rollout_config_view(&config_snapshot);
    let mut guard = match rollouts.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if rollout_policy::runtime_cloud_rollouts_reconcile_once(&mut guard, &config_view, now_ns()) {
        runtime_cloud_rollouts_store_state(persist_path, &guard);
    }
}

fn runtime_cloud_rollout_config_view(
    config: &RuntimeCloudConfigAgentState,
) -> RuntimeCloudRolloutConfigView {
    RuntimeCloudRolloutConfigView {
        reported_revision: config.meta.reported_revision,
        state: config.status.state,
        blocked_reason: config.status.blocked_reason,
        first_error: config.status.errors.first().cloned(),
    }
}
