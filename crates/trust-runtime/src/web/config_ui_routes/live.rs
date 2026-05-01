use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigUiLiveTargetProfile {
    target: String,
    label: String,
}

#[derive(Debug, Default)]
struct ConfigUiLiveManagerState {
    profiles: BTreeMap<String, ConfigUiLiveTargetProfile>,
    active_target: Option<String>,
    active_token: Option<String>,
    connected: bool,
    last_error: Option<String>,
    last_runtime_cloud: Option<serde_json::Value>,
    updated_at_ns: u64,
}

static CONFIG_UI_LIVE_MANAGER: OnceLock<Mutex<ConfigUiLiveManagerState>> = OnceLock::new();

fn config_ui_live_manager() -> &'static Mutex<ConfigUiLiveManagerState> {
    CONFIG_UI_LIVE_MANAGER.get_or_init(|| Mutex::new(ConfigUiLiveManagerState::default()))
}

fn normalize_live_target(raw: &str) -> Result<String, RuntimeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig("target is required".into()));
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let normalized = with_scheme.trim_end_matches('/').to_string();
    if normalized == "http:" || normalized == "https:" || normalized.ends_with("://") {
        return Err(RuntimeError::InvalidConfig(
            "target must include host".into(),
        ));
    }
    Ok(normalized)
}

pub(super) fn config_ui_live_targets_snapshot() -> serde_json::Value {
    let manager = config_ui_live_manager()
        .lock()
        .ok()
        .map(|guard| config_ui_live_targets_snapshot_with_guard(&guard))
        .unwrap_or_else(|| {
            json!({
                "ok": false,
                "profiles": [],
                "active_target": null,
                "connected": false,
                "last_error": "live manager unavailable",
                "updated_at_ns": now_ns(),
            })
        });
    manager
}

fn config_ui_live_targets_snapshot_with_guard(
    guard: &ConfigUiLiveManagerState,
) -> serde_json::Value {
    let profiles = guard
        .profiles
        .values()
        .cloned()
        .collect::<Vec<ConfigUiLiveTargetProfile>>();
    json!({
        "ok": true,
        "profiles": profiles,
        "active_target": guard.active_target,
        "connected": guard.connected,
        "last_error": guard.last_error,
        "updated_at_ns": guard.updated_at_ns,
    })
}

pub(super) fn config_ui_live_target_upsert(
    payload: &ConfigLiveTargetUpsertRequest,
) -> Result<serde_json::Value, RuntimeError> {
    let target = normalize_live_target(payload.target.as_str())?;
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(target.as_str())
        .to_string();
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    guard.profiles.insert(
        target.clone(),
        ConfigUiLiveTargetProfile {
            target: target.clone(),
            label,
        },
    );
    guard.updated_at_ns = now_ns();
    let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
    Ok(json!({
        "ok": true,
        "target": target,
        "snapshot": snapshot,
    }))
}

pub(super) fn config_ui_live_target_remove(
    payload: &ConfigLiveTargetRemoveRequest,
) -> Result<serde_json::Value, RuntimeError> {
    let target = normalize_live_target(payload.target.as_str())?;
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    guard.profiles.remove(&target);
    if guard.active_target.as_deref() == Some(target.as_str()) {
        guard.active_target = None;
        guard.active_token = None;
        guard.connected = false;
        guard.last_error = None;
        guard.last_runtime_cloud = None;
    }
    guard.updated_at_ns = now_ns();
    let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
    Ok(json!({
        "ok": true,
        "target": target,
        "snapshot": snapshot,
    }))
}

fn fetch_runtime_cloud_state(
    target: &str,
    token: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let target = normalize_live_target(target)?;
    let state_url = format!("{target}/api/runtime-cloud/state");
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_millis(800)))
        .timeout_recv_response(Some(Duration::from_millis(1500)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();
    let request = token
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| agent.get(&state_url).header("X-Trust-Token", value))
        .unwrap_or_else(|| agent.get(&state_url));
    let mut response = request.call().map_err(|error| {
        RuntimeError::ControlError(format!("live connect request failed: {error}").into())
    })?;
    let status = response.status().as_u16();
    let body_text = response.body_mut().read_to_string().unwrap_or_default();
    let body: serde_json::Value = serde_json::from_str(&body_text).unwrap_or_else(|_| json!({}));
    if status >= 400 {
        let detail = body
            .get("error")
            .and_then(serde_json::Value::as_str)
            .or_else(|| body.get("message").and_then(serde_json::Value::as_str))
            .unwrap_or("remote runtime-cloud request failed");
        return Err(RuntimeError::ControlError(
            format!("live connect failed ({status}): {detail}").into(),
        ));
    }
    if body.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        let detail = body
            .get("error")
            .and_then(serde_json::Value::as_str)
            .or_else(|| body.get("message").and_then(serde_json::Value::as_str))
            .unwrap_or("remote runtime-cloud returned error");
        return Err(RuntimeError::ControlError(
            format!("live connect failed: {detail}").into(),
        ));
    }
    Ok(body)
}

pub(super) fn config_ui_live_connect(
    target: Option<&str>,
    token: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    let chosen_target = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_live_target)
        .transpose()?
        .or_else(|| guard.active_target.clone());

    let Some(chosen_target) = chosen_target else {
        guard.active_target = None;
        guard.connected = false;
        guard.last_error = None;
        guard.last_runtime_cloud = None;
        guard.updated_at_ns = now_ns();
        let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
        return Ok(json!({
            "ok": true,
            "connected": false,
            "active_target": null,
            "snapshot": snapshot,
        }));
    };

    if !guard.profiles.contains_key(&chosen_target) {
        guard.profiles.insert(
            chosen_target.clone(),
            ConfigUiLiveTargetProfile {
                target: chosen_target.clone(),
                label: chosen_target.clone(),
            },
        );
    }

    guard.active_target = Some(chosen_target.clone());
    guard.active_token = token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| guard.active_token.clone());

    match fetch_runtime_cloud_state(&chosen_target, guard.active_token.as_deref()) {
        Ok(value) => {
            guard.connected = true;
            guard.last_error = None;
            guard.last_runtime_cloud = Some(value.clone());
            guard.updated_at_ns = now_ns();
            let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
            Ok(json!({
                "ok": true,
                "connected": true,
                "active_target": chosen_target,
                "runtime_cloud": value,
                "snapshot": snapshot,
            }))
        }
        Err(error) => {
            guard.connected = false;
            guard.last_error = Some(error.to_string());
            guard.last_runtime_cloud = None;
            guard.updated_at_ns = now_ns();
            let snapshot = config_ui_live_targets_snapshot_with_guard(&guard);
            Ok(json!({
                "ok": true,
                "connected": false,
                "active_target": chosen_target,
                "last_error": guard.last_error,
                "snapshot": snapshot,
            }))
        }
    }
}

pub(super) fn config_ui_live_state(
    target: Option<&str>,
) -> Result<serde_json::Value, RuntimeError> {
    let target = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_live_target)
        .transpose()?;
    let mut guard = config_ui_live_manager()
        .lock()
        .map_err(|_| RuntimeError::ControlError("failed to lock config-ui live manager".into()))?;
    let chosen = target.or_else(|| guard.active_target.clone());
    let Some(chosen) = chosen else {
        return Ok(json!({
            "ok": true,
            "connected": false,
            "active_target": null,
            "runtime_cloud": null,
            "last_error": guard.last_error,
            "updated_at_ns": guard.updated_at_ns,
        }));
    };

    match fetch_runtime_cloud_state(&chosen, guard.active_token.as_deref()) {
        Ok(value) => {
            guard.active_target = Some(chosen.clone());
            guard.connected = true;
            guard.last_error = None;
            guard.last_runtime_cloud = Some(value.clone());
            guard.updated_at_ns = now_ns();
            Ok(json!({
                "ok": true,
                "connected": true,
                "active_target": chosen,
                "runtime_cloud": value,
                "last_error": null,
                "updated_at_ns": guard.updated_at_ns,
            }))
        }
        Err(error) => {
            guard.active_target = Some(chosen.clone());
            guard.connected = false;
            guard.last_error = Some(error.to_string());
            guard.last_runtime_cloud = None;
            guard.updated_at_ns = now_ns();
            Ok(json!({
                "ok": true,
                "connected": false,
                "active_target": chosen,
                "runtime_cloud": null,
                "last_error": guard.last_error,
                "updated_at_ns": guard.updated_at_ns,
            }))
        }
    }
}

pub(super) fn config_ui_live_runtime_cloud_overlay() -> Option<RuntimeCloudUiState> {
    config_ui_live_manager()
        .lock()
        .ok()
        .and_then(|guard| {
            if guard.connected {
                guard.last_runtime_cloud.clone()
            } else {
                None
            }
        })
        .and_then(|value| serde_json::from_value::<RuntimeCloudUiState>(value).ok())
}
