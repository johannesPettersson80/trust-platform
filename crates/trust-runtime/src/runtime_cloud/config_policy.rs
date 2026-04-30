//! Runtime-cloud config-agent state and policy.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::runtime_cloud::contracts::{
    ConfigMeta, ConfigState as RuntimeCloudConfigState, ConfigStatus, ReasonCode,
    RUNTIME_CLOUD_API_VERSION,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeCloudConfigSnapshot {
    pub(crate) api_version: String,
    pub(crate) runtime_id: String,
    pub(crate) desired: Value,
    pub(crate) reported: Value,
    pub(crate) meta: ConfigMeta,
    pub(crate) status: ConfigStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeCloudConfigAgentState {
    pub(crate) desired: Value,
    pub(crate) reported: Value,
    pub(crate) meta: ConfigMeta,
    pub(crate) status: ConfigStatus,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeCloudConfigWriteError {
    pub(crate) code: ReasonCode,
    pub(crate) message: String,
    pub(crate) snapshot: Box<RuntimeCloudConfigAgentState>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeCloudConfigApplyRequest {
    pub(crate) desired: Value,
    pub(crate) desired_revision: u64,
    pub(crate) desired_etag: String,
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeCloudConfigReconcileAction {
    Idle { should_store: bool },
    Apply(RuntimeCloudConfigApplyRequest),
}

pub(crate) fn runtime_cloud_config_initial_state(
    updated_at_ns: u64,
) -> RuntimeCloudConfigAgentState {
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
            updated_at_ns,
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

pub(crate) fn runtime_cloud_config_snapshot(
    state: RuntimeCloudConfigAgentState,
    runtime_id: &str,
) -> RuntimeCloudConfigSnapshot {
    RuntimeCloudConfigSnapshot {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        runtime_id: runtime_id.to_string(),
        desired: state.desired,
        reported: state.reported,
        meta: state.meta,
        status: state.status,
    }
}

pub(crate) fn runtime_cloud_config_write_desired(
    state: &mut RuntimeCloudConfigAgentState,
    actor: &str,
    desired: &Value,
    expected_revision: Option<u64>,
    expected_etag: Option<&str>,
    updated_at_ns: u64,
) -> Result<RuntimeCloudConfigAgentState, RuntimeCloudConfigWriteError> {
    if actor.trim().is_empty() {
        return Err(RuntimeCloudConfigWriteError {
            code: ReasonCode::ContractViolation,
            message: "actor must not be empty".to_string(),
            snapshot: Box::new(state.clone()),
        });
    }
    if !desired.is_object() {
        return Err(RuntimeCloudConfigWriteError {
            code: ReasonCode::ContractViolation,
            message: "desired must be an object".to_string(),
            snapshot: Box::new(state.clone()),
        });
    }

    if let Some(expected_revision) = expected_revision {
        if expected_revision != state.meta.desired_revision {
            let message = format!(
                "expected_revision {} does not match current desired_revision {}",
                expected_revision, state.meta.desired_revision
            );
            runtime_cloud_config_mark_revision_conflict(state, message.clone());
            return Err(RuntimeCloudConfigWriteError {
                code: ReasonCode::RevisionConflict,
                message,
                snapshot: Box::new(state.clone()),
            });
        }
    }
    if let Some(expected_etag) = expected_etag {
        if expected_etag != state.meta.desired_etag {
            let message = "expected_etag does not match current desired_etag".to_string();
            runtime_cloud_config_mark_revision_conflict(state, message.clone());
            return Err(RuntimeCloudConfigWriteError {
                code: ReasonCode::RevisionConflict,
                message,
                snapshot: Box::new(state.clone()),
            });
        }
    }

    runtime_cloud_merge_json(&mut state.desired, desired);
    state.meta.desired_revision = state.meta.desired_revision.saturating_add(1);
    state.meta.desired_etag = runtime_cloud_hash_json(&state.desired);
    state.meta.last_writer = actor.to_string();
    state.meta.updated_at_ns = updated_at_ns;
    state.status = ConfigStatus {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        state: RuntimeCloudConfigState::Pending,
        applied_revision: state.meta.reported_revision,
        pending_revision: Some(state.meta.desired_revision),
        required_action: None,
        blocked_reason: None,
        errors: Vec::new(),
    };
    Ok(state.clone())
}

pub(crate) fn runtime_cloud_config_prepare_reconcile(
    state: &mut RuntimeCloudConfigAgentState,
) -> RuntimeCloudConfigReconcileAction {
    if state.meta.desired_revision == state.meta.reported_revision {
        runtime_cloud_config_mark_in_sync(state);
        return RuntimeCloudConfigReconcileAction::Idle { should_store: true };
    }
    if state.status.state != RuntimeCloudConfigState::Pending {
        return RuntimeCloudConfigReconcileAction::Idle {
            should_store: false,
        };
    }
    RuntimeCloudConfigReconcileAction::Apply(RuntimeCloudConfigApplyRequest {
        desired: state.desired.clone(),
        desired_revision: state.meta.desired_revision,
        desired_etag: state.meta.desired_etag.clone(),
    })
}

pub(crate) fn runtime_cloud_config_apply_success(
    state: &mut RuntimeCloudConfigAgentState,
    desired_revision: u64,
    desired_etag: String,
    updated_at_ns: u64,
) {
    state.reported = state.desired.clone();
    state.meta.reported_revision = desired_revision;
    state.meta.reported_etag = desired_etag;
    state.meta.updated_at_ns = updated_at_ns;
    runtime_cloud_config_mark_in_sync(state);
}

pub(crate) fn runtime_cloud_config_apply_failure(
    state: &mut RuntimeCloudConfigAgentState,
    error_text: String,
) {
    let (state_value, blocked_reason, required_action) =
        runtime_cloud_config_error_semantics(error_text.as_str());
    state.status = ConfigStatus {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        state: state_value,
        applied_revision: state.meta.reported_revision,
        pending_revision: Some(state.meta.desired_revision),
        required_action,
        blocked_reason,
        errors: vec![error_text],
    };
}

pub(crate) fn runtime_cloud_merge_json(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_map), Value::Object(patch_map)) => {
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

pub(crate) fn runtime_cloud_hash_json(value: &Value) -> String {
    let payload = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    let digest = Sha256::digest(payload);
    format!("sha256:{digest:x}")
}

pub(crate) fn runtime_cloud_config_error_semantics(
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

fn runtime_cloud_config_mark_revision_conflict(
    state: &mut RuntimeCloudConfigAgentState,
    message: String,
) {
    state.status = ConfigStatus {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        state: RuntimeCloudConfigState::Blocked,
        applied_revision: state.meta.reported_revision,
        pending_revision: Some(state.meta.desired_revision),
        required_action: Some("rebase_required".to_string()),
        blocked_reason: Some(ReasonCode::RevisionConflict),
        errors: vec![message],
    };
}

fn runtime_cloud_config_mark_in_sync(state: &mut RuntimeCloudConfigAgentState) {
    state.status = ConfigStatus {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        state: RuntimeCloudConfigState::InSync,
        applied_revision: state.meta.reported_revision,
        pending_revision: None,
        required_action: None,
        blocked_reason: None,
        errors: Vec::new(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_write_merges_json_and_sets_pending_status() {
        let mut state = runtime_cloud_config_initial_state(1);
        let desired = json!({
            "runtime": {
                "cycle_ms": 20
            }
        });

        let updated =
            runtime_cloud_config_write_desired(&mut state, "operator", &desired, None, None, 2)
                .expect("write desired config");

        assert_eq!(updated.meta.desired_revision, 1);
        assert_eq!(updated.meta.last_writer, "operator");
        assert_eq!(updated.status.state, RuntimeCloudConfigState::Pending);
        assert_eq!(updated.desired["runtime"]["cycle_ms"], 20);
    }
}
