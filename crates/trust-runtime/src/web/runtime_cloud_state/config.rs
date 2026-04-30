use super::*;

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
    serde_json::from_str::<RuntimeCloudConfigAgentState>(&text)
        .unwrap_or_else(|_| runtime_cloud_config_initial_state())
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
    let desired = json!({});
    let etag = runtime_cloud_hash_json(&desired);
    RuntimeCloudConfigAgentState {
        desired: desired.clone(),
        reported: desired,
        meta: ConfigMeta {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            desired_revision: 0,
            reported_revision: 0,
            desired_etag: etag.clone(),
            reported_etag: etag,
            last_writer: "bootstrap".to_string(),
            apply_policy: "explicit".to_string(),
            updated_at_ns: now_ns(),
        },
        status: ConfigStatus {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            state: RuntimeCloudConfigState::InSync,
            applied_revision: 0,
            pending_revision: None,
            required_action: None,
            blocked_reason: None,
            errors: Vec::new(),
        },
    }
}

pub(in crate::web) fn runtime_cloud_config_snapshot(
    state: &Mutex<RuntimeCloudConfigAgentState>,
    runtime_id: &str,
) -> RuntimeCloudConfigSnapshot {
    let current = state
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| runtime_cloud_config_initial_state());
    RuntimeCloudConfigSnapshot {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        runtime_id: runtime_id.to_string(),
        desired: current.desired,
        reported: current.reported,
        meta: current.meta,
        status: current.status,
    }
}

pub(in crate::web) fn runtime_cloud_config_write_desired(
    state: &Mutex<RuntimeCloudConfigAgentState>,
    payload: &RuntimeCloudDesiredWriteRequest,
    persist_path: Option<&Path>,
) -> Result<RuntimeCloudConfigAgentState, RuntimeCloudConfigWriteError> {
    if payload.actor.trim().is_empty() {
        let snapshot = state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| runtime_cloud_config_initial_state());
        return Err(RuntimeCloudConfigWriteError {
            code: ReasonCode::ContractViolation,
            message: "actor must not be empty".to_string(),
            snapshot: Box::new(snapshot),
        });
    }
    if !payload.desired.is_object() {
        let snapshot = state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| runtime_cloud_config_initial_state());
        return Err(RuntimeCloudConfigWriteError {
            code: ReasonCode::ContractViolation,
            message: "desired must be an object".to_string(),
            snapshot: Box::new(snapshot),
        });
    }
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
    if let Some(expected_revision) = payload.expected_revision {
        if expected_revision != guard.meta.desired_revision {
            let message = format!(
                "expected_revision {} does not match current desired_revision {}",
                expected_revision, guard.meta.desired_revision
            );
            guard.status = ConfigStatus {
                api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
                state: RuntimeCloudConfigState::Blocked,
                applied_revision: guard.meta.reported_revision,
                pending_revision: Some(guard.meta.desired_revision),
                required_action: Some("rebase_required".to_string()),
                blocked_reason: Some(ReasonCode::RevisionConflict),
                errors: vec![message.clone()],
            };
            runtime_cloud_config_store_state(persist_path, &guard);
            return Err(RuntimeCloudConfigWriteError {
                code: ReasonCode::RevisionConflict,
                message,
                snapshot: Box::new(guard.clone()),
            });
        }
    }
    if let Some(expected_etag) = payload.expected_etag.as_deref() {
        if expected_etag != guard.meta.desired_etag {
            let message = "expected_etag does not match current desired_etag".to_string();
            guard.status = ConfigStatus {
                api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
                state: RuntimeCloudConfigState::Blocked,
                applied_revision: guard.meta.reported_revision,
                pending_revision: Some(guard.meta.desired_revision),
                required_action: Some("rebase_required".to_string()),
                blocked_reason: Some(ReasonCode::RevisionConflict),
                errors: vec![message.clone()],
            };
            runtime_cloud_config_store_state(persist_path, &guard);
            return Err(RuntimeCloudConfigWriteError {
                code: ReasonCode::RevisionConflict,
                message,
                snapshot: Box::new(guard.clone()),
            });
        }
    }

    runtime_cloud_merge_json(&mut guard.desired, &payload.desired);
    guard.meta.desired_revision = guard.meta.desired_revision.saturating_add(1);
    guard.meta.desired_etag = runtime_cloud_hash_json(&guard.desired);
    guard.meta.last_writer = payload.actor.clone();
    guard.meta.updated_at_ns = now_ns();
    guard.status = ConfigStatus {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        state: RuntimeCloudConfigState::Pending,
        applied_revision: guard.meta.reported_revision,
        pending_revision: Some(guard.meta.desired_revision),
        required_action: None,
        blocked_reason: None,
        errors: Vec::new(),
    };
    runtime_cloud_config_store_state(persist_path, &guard);
    Ok(guard.clone())
}

pub(in crate::web) fn runtime_cloud_config_reconcile_once(
    state: &Mutex<RuntimeCloudConfigAgentState>,
    control_state: &ControlState,
    persist_path: Option<&Path>,
) {
    let (desired, desired_revision, desired_etag) = {
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if guard.meta.desired_revision == guard.meta.reported_revision {
            guard.status = ConfigStatus {
                api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
                state: RuntimeCloudConfigState::InSync,
                applied_revision: guard.meta.reported_revision,
                pending_revision: None,
                required_action: None,
                blocked_reason: None,
                errors: Vec::new(),
            };
            runtime_cloud_config_store_state(persist_path, &guard);
            return;
        }
        if guard.status.state != RuntimeCloudConfigState::Pending {
            return;
        }
        (
            guard.desired.clone(),
            guard.meta.desired_revision,
            guard.meta.desired_etag.clone(),
        )
    };

    let control_payload = json!({
        "id": 1_u64,
        "type": "config.set",
        "request_id": format!("cfg-agent-{desired_revision}"),
        "params": desired,
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
        guard.reported = guard.desired.clone();
        guard.meta.reported_revision = desired_revision;
        guard.meta.reported_etag = desired_etag;
        guard.meta.updated_at_ns = now_ns();
        guard.status = ConfigStatus {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            state: RuntimeCloudConfigState::InSync,
            applied_revision: guard.meta.reported_revision,
            pending_revision: None,
            required_action: None,
            blocked_reason: None,
            errors: Vec::new(),
        };
        runtime_cloud_config_store_state(persist_path, &guard);
        return;
    }

    let error_text = response_value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("config apply failed")
        .to_string();
    let (state_value, blocked_reason, required_action) =
        runtime_cloud_config_error_semantics(error_text.as_str());
    guard.status = ConfigStatus {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        state: state_value,
        applied_revision: guard.meta.reported_revision,
        pending_revision: Some(guard.meta.desired_revision),
        required_action,
        blocked_reason,
        errors: vec![error_text],
    };
    runtime_cloud_config_store_state(persist_path, &guard);
}

pub(in crate::web) fn runtime_cloud_merge_json(
    target: &mut serde_json::Value,
    patch: &serde_json::Value,
) {
    match (target, patch) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(patch_map)) => {
            for (key, patch_value) in patch_map {
                match target_map.get_mut(key) {
                    Some(target_value) => runtime_cloud_merge_json(target_value, patch_value),
                    None => {
                        target_map.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        (target_value, patch_value) => {
            *target_value = patch_value.clone();
        }
    }
}

pub(in crate::web) fn runtime_cloud_hash_json(value: &serde_json::Value) -> String {
    let payload = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    let digest = Sha256::digest(payload);
    format!("sha256:{digest:x}")
}

pub(in crate::web) fn runtime_cloud_config_error_semantics(
    error: &str,
) -> (RuntimeCloudConfigState, Option<ReasonCode>, Option<String>) {
    let lower = error.to_ascii_lowercase();
    if lower.contains("forbidden") || lower.contains("permission") {
        return (
            RuntimeCloudConfigState::Blocked,
            Some(ReasonCode::PermissionDenied),
            Some("privileged_write_required".to_string()),
        );
    }
    if lower.contains("conflict") || lower.contains("etag") || lower.contains("revision") {
        return (
            RuntimeCloudConfigState::Blocked,
            Some(ReasonCode::RevisionConflict),
            Some("rebase_required".to_string()),
        );
    }
    if lower.contains("schema") {
        return (
            RuntimeCloudConfigState::Error,
            Some(ReasonCode::SchemaMismatch),
            None,
        );
    }
    (RuntimeCloudConfigState::Error, None, None)
}
