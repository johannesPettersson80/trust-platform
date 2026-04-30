//! Runtime-cloud rollout state machine and policy.

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::runtime_cloud::contracts::{
    evaluate_compatibility, ConfigState as RuntimeCloudConfigState, ContractCompatibility,
    ReasonCode, RUNTIME_CLOUD_API_VERSION,
};

pub(crate) const RUNTIME_CLOUD_ROLLOUT_APPLY_TIMEOUT_NS: u64 = 30_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeCloudRolloutState {
    Queued,
    Staging,
    Staged,
    Applying,
    Applied,
    Verifying,
    Verified,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeCloudRolloutTargetState {
    Queued,
    Staging,
    Staged,
    Applying,
    Applied,
    Verifying,
    Verified,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeCloudRolloutTargetRecord {
    pub(crate) runtime_id: String,
    pub(crate) state: RuntimeCloudRolloutTargetState,
    pub(crate) verification: Option<String>,
    pub(crate) blocked_reason: Option<ReasonCode>,
    pub(crate) error: Option<String>,
    pub(crate) updated_at_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeCloudRolloutRecord {
    pub(crate) api_version: String,
    pub(crate) rollout_id: String,
    pub(crate) actor: String,
    pub(crate) desired_revision: u64,
    pub(crate) state: RuntimeCloudRolloutState,
    pub(crate) paused: bool,
    pub(crate) created_at_ns: u64,
    pub(crate) updated_at_ns: u64,
    pub(crate) targets: Vec<RuntimeCloudRolloutTargetRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeCloudRolloutManagerState {
    pub(crate) next_id: u64,
    pub(crate) rollouts: BTreeMap<String, RuntimeCloudRolloutRecord>,
}

impl Default for RuntimeCloudRolloutManagerState {
    fn default() -> Self {
        Self {
            next_id: 1,
            rollouts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeCloudRolloutActionResponse {
    pub(crate) ok: bool,
    pub(crate) action: String,
    pub(crate) denial_code: Option<ReasonCode>,
    pub(crate) error: Option<String>,
    pub(crate) rollout: Option<RuntimeCloudRolloutRecord>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeCloudRolloutConfigView {
    pub(crate) reported_revision: u64,
    pub(crate) state: RuntimeCloudConfigState,
    pub(crate) blocked_reason: Option<ReasonCode>,
    pub(crate) first_error: Option<String>,
}

pub(crate) fn runtime_cloud_rollouts_snapshot(
    state: &RuntimeCloudRolloutManagerState,
) -> Vec<RuntimeCloudRolloutRecord> {
    state.rollouts.values().cloned().collect::<Vec<_>>()
}

pub(crate) fn runtime_cloud_rollout_create(
    state: &mut RuntimeCloudRolloutManagerState,
    api_version: &str,
    actor: &str,
    target_runtimes: &[String],
    desired_revision: Option<u64>,
    current_desired_revision: u64,
    now_ns: u64,
) -> Result<RuntimeCloudRolloutRecord, (ReasonCode, String)> {
    match evaluate_compatibility(api_version, RUNTIME_CLOUD_API_VERSION) {
        Ok(ContractCompatibility::Exact | ContractCompatibility::AdditiveWithinMajor) => {}
        Ok(ContractCompatibility::BreakingMajor) => {
            return Err((
                ReasonCode::ContractViolation,
                format!(
                    "unsupported api_version '{api_version}' for runtime cloud {RUNTIME_CLOUD_API_VERSION}",
                ),
            ));
        }
        Err(error) => {
            return Err((ReasonCode::ContractViolation, error.to_string()));
        }
    }
    if actor.trim().is_empty() {
        return Err((
            ReasonCode::ContractViolation,
            "actor must not be empty".to_string(),
        ));
    }
    if target_runtimes.is_empty() {
        return Err((
            ReasonCode::ContractViolation,
            "target_runtimes must include at least one runtime".to_string(),
        ));
    }

    let desired_revision = desired_revision.unwrap_or(current_desired_revision);
    if desired_revision > current_desired_revision {
        return Err((
            ReasonCode::RevisionConflict,
            format!(
                "desired_revision {desired_revision} is newer than current desired_revision {current_desired_revision}",
            ),
        ));
    }

    let rollout_id = format!("rollout-{}", state.next_id);
    state.next_id = state.next_id.saturating_add(1);

    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    for runtime_id in target_runtimes {
        if !seen.insert(runtime_id.as_str()) {
            continue;
        }
        targets.push(RuntimeCloudRolloutTargetRecord {
            runtime_id: runtime_id.clone(),
            state: RuntimeCloudRolloutTargetState::Queued,
            verification: None,
            blocked_reason: None,
            error: None,
            updated_at_ns: now_ns,
        });
    }
    if targets.is_empty() {
        return Err((
            ReasonCode::ContractViolation,
            "target_runtimes must include at least one unique runtime".to_string(),
        ));
    }

    let rollout = RuntimeCloudRolloutRecord {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        rollout_id: rollout_id.clone(),
        actor: actor.to_string(),
        desired_revision,
        state: RuntimeCloudRolloutState::Queued,
        paused: false,
        created_at_ns: now_ns,
        updated_at_ns: now_ns,
        targets,
    };
    state.rollouts.insert(rollout_id, rollout.clone());
    Ok(rollout)
}

pub(crate) fn runtime_cloud_rollout_apply_action(
    state: &mut RuntimeCloudRolloutManagerState,
    rollout_id: &str,
    action: &str,
    now_ns: u64,
) -> RuntimeCloudRolloutActionResponse {
    let Some(rollout) = state.rollouts.get_mut(rollout_id) else {
        return RuntimeCloudRolloutActionResponse {
            ok: false,
            action: action.to_string(),
            denial_code: Some(ReasonCode::PeerNotAvailable),
            error: Some(format!("unknown rollout_id '{rollout_id}'")),
            rollout: None,
        };
    };

    match action {
        "pause" => {
            if runtime_cloud_rollout_is_terminal(rollout.state) {
                RuntimeCloudRolloutActionResponse {
                    ok: false,
                    action: "pause".to_string(),
                    denial_code: Some(ReasonCode::Conflict),
                    error: Some("cannot pause terminal rollout".to_string()),
                    rollout: Some(rollout.clone()),
                }
            } else if rollout.paused {
                RuntimeCloudRolloutActionResponse {
                    ok: true,
                    action: "noop".to_string(),
                    denial_code: None,
                    error: None,
                    rollout: Some(rollout.clone()),
                }
            } else {
                rollout.paused = true;
                rollout.updated_at_ns = now_ns;
                RuntimeCloudRolloutActionResponse {
                    ok: true,
                    action: "paused".to_string(),
                    denial_code: None,
                    error: None,
                    rollout: Some(rollout.clone()),
                }
            }
        }
        "resume" => {
            if runtime_cloud_rollout_is_terminal(rollout.state) {
                RuntimeCloudRolloutActionResponse {
                    ok: false,
                    action: "resume".to_string(),
                    denial_code: Some(ReasonCode::Conflict),
                    error: Some("cannot resume terminal rollout".to_string()),
                    rollout: Some(rollout.clone()),
                }
            } else if !rollout.paused {
                RuntimeCloudRolloutActionResponse {
                    ok: true,
                    action: "noop".to_string(),
                    denial_code: None,
                    error: None,
                    rollout: Some(rollout.clone()),
                }
            } else {
                rollout.paused = false;
                rollout.updated_at_ns = now_ns;
                RuntimeCloudRolloutActionResponse {
                    ok: true,
                    action: "resumed".to_string(),
                    denial_code: None,
                    error: None,
                    rollout: Some(rollout.clone()),
                }
            }
        }
        "abort" => {
            if runtime_cloud_rollout_is_terminal(rollout.state) {
                RuntimeCloudRolloutActionResponse {
                    ok: false,
                    action: "abort".to_string(),
                    denial_code: Some(ReasonCode::Conflict),
                    error: Some("cannot abort terminal rollout".to_string()),
                    rollout: Some(rollout.clone()),
                }
            } else {
                rollout.paused = false;
                rollout.state = RuntimeCloudRolloutState::Aborted;
                rollout.updated_at_ns = now_ns;
                for target in &mut rollout.targets {
                    target.state = RuntimeCloudRolloutTargetState::Aborted;
                    target.error = Some("operator aborted rollout".to_string());
                    target.updated_at_ns = now_ns;
                }
                RuntimeCloudRolloutActionResponse {
                    ok: true,
                    action: "aborted".to_string(),
                    denial_code: None,
                    error: None,
                    rollout: Some(rollout.clone()),
                }
            }
        }
        _ => RuntimeCloudRolloutActionResponse {
            ok: false,
            action: action.to_string(),
            denial_code: Some(ReasonCode::ContractViolation),
            error: Some(format!("unsupported rollout action '{action}'")),
            rollout: Some(rollout.clone()),
        },
    }
}

pub(crate) fn runtime_cloud_rollouts_reconcile_once(
    state: &mut RuntimeCloudRolloutManagerState,
    config: &RuntimeCloudRolloutConfigView,
    now_ns: u64,
) -> bool {
    let mut changed = false;
    for rollout in state.rollouts.values_mut() {
        if rollout.paused || runtime_cloud_rollout_is_terminal(rollout.state) {
            continue;
        }
        if runtime_cloud_rollout_advance(rollout, config, now_ns) {
            changed = true;
        }
    }
    changed
}

pub(crate) fn runtime_cloud_rollout_advance(
    rollout: &mut RuntimeCloudRolloutRecord,
    config: &RuntimeCloudRolloutConfigView,
    now_ns: u64,
) -> bool {
    let before = rollout.state;
    let next = match rollout.state {
        RuntimeCloudRolloutState::Queued => Some(RuntimeCloudRolloutState::Staging),
        RuntimeCloudRolloutState::Staging => Some(RuntimeCloudRolloutState::Staged),
        RuntimeCloudRolloutState::Staged => Some(RuntimeCloudRolloutState::Applying),
        RuntimeCloudRolloutState::Applying => {
            let elapsed = now_ns.saturating_sub(rollout.updated_at_ns);
            if elapsed >= RUNTIME_CLOUD_ROLLOUT_APPLY_TIMEOUT_NS {
                runtime_cloud_rollout_fail(
                    rollout,
                    now_ns,
                    Some(ReasonCode::Timeout),
                    Some("rollout applying timed out".to_string()),
                );
                return true;
            }
            if matches!(
                config.state,
                RuntimeCloudConfigState::Blocked | RuntimeCloudConfigState::Error
            ) {
                runtime_cloud_rollout_fail(
                    rollout,
                    now_ns,
                    config.blocked_reason,
                    config.first_error.clone(),
                );
                return true;
            }
            if config.reported_revision >= rollout.desired_revision {
                Some(RuntimeCloudRolloutState::Applied)
            } else {
                None
            }
        }
        RuntimeCloudRolloutState::Applied => Some(RuntimeCloudRolloutState::Verifying),
        RuntimeCloudRolloutState::Verifying => {
            if matches!(
                config.state,
                RuntimeCloudConfigState::Blocked | RuntimeCloudConfigState::Error
            ) {
                runtime_cloud_rollout_fail(
                    rollout,
                    now_ns,
                    config.blocked_reason,
                    config.first_error.clone(),
                );
                return true;
            }
            if config.reported_revision >= rollout.desired_revision
                && config.state == RuntimeCloudConfigState::InSync
            {
                Some(RuntimeCloudRolloutState::Verified)
            } else {
                None
            }
        }
        RuntimeCloudRolloutState::Verified => Some(RuntimeCloudRolloutState::Completed),
        RuntimeCloudRolloutState::Completed
        | RuntimeCloudRolloutState::Failed
        | RuntimeCloudRolloutState::Aborted => None,
    };

    let Some(next) = next else {
        return false;
    };
    rollout.state = next;
    rollout.updated_at_ns = now_ns;
    for target in &mut rollout.targets {
        target.state = match next {
            RuntimeCloudRolloutState::Queued => RuntimeCloudRolloutTargetState::Queued,
            RuntimeCloudRolloutState::Staging => RuntimeCloudRolloutTargetState::Staging,
            RuntimeCloudRolloutState::Staged => RuntimeCloudRolloutTargetState::Staged,
            RuntimeCloudRolloutState::Applying => RuntimeCloudRolloutTargetState::Applying,
            RuntimeCloudRolloutState::Applied => RuntimeCloudRolloutTargetState::Applied,
            RuntimeCloudRolloutState::Verifying => RuntimeCloudRolloutTargetState::Verifying,
            RuntimeCloudRolloutState::Verified => {
                target.verification = Some(format!(
                    "reported_revision={} status=in_sync",
                    config.reported_revision
                ));
                RuntimeCloudRolloutTargetState::Verified
            }
            RuntimeCloudRolloutState::Completed => target.state,
            RuntimeCloudRolloutState::Failed => RuntimeCloudRolloutTargetState::Failed,
            RuntimeCloudRolloutState::Aborted => RuntimeCloudRolloutTargetState::Aborted,
        };
        target.updated_at_ns = now_ns;
    }
    before != rollout.state
}

pub(crate) fn runtime_cloud_rollout_is_terminal(state: RuntimeCloudRolloutState) -> bool {
    matches!(
        state,
        RuntimeCloudRolloutState::Completed
            | RuntimeCloudRolloutState::Failed
            | RuntimeCloudRolloutState::Aborted
    )
}

fn runtime_cloud_rollout_fail(
    rollout: &mut RuntimeCloudRolloutRecord,
    now_ns: u64,
    reason: Option<ReasonCode>,
    error: Option<String>,
) {
    rollout.state = RuntimeCloudRolloutState::Failed;
    rollout.updated_at_ns = now_ns;
    for target in &mut rollout.targets {
        target.state = RuntimeCloudRolloutTargetState::Failed;
        target.blocked_reason = reason;
        target.error = error.clone();
        target.updated_at_ns = now_ns;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_cloud_rollout_applying_timeout_transitions_to_failed() {
        let now = 42_000_000_000;
        let mut rollout = RuntimeCloudRolloutRecord {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            rollout_id: "rollout-timeout-1".to_string(),
            actor: "spiffe://trust/default-site/operator-1".to_string(),
            desired_revision: 5,
            state: RuntimeCloudRolloutState::Applying,
            paused: false,
            created_at_ns: now,
            updated_at_ns: now.saturating_sub(RUNTIME_CLOUD_ROLLOUT_APPLY_TIMEOUT_NS + 1),
            targets: vec![RuntimeCloudRolloutTargetRecord {
                runtime_id: "runtime-a".to_string(),
                state: RuntimeCloudRolloutTargetState::Applying,
                verification: None,
                blocked_reason: None,
                error: None,
                updated_at_ns: now,
            }],
        };
        let config = RuntimeCloudRolloutConfigView {
            reported_revision: 0,
            state: RuntimeCloudConfigState::InSync,
            blocked_reason: None,
            first_error: None,
        };

        let changed = runtime_cloud_rollout_advance(&mut rollout, &config, now);
        assert!(changed);
        assert_eq!(rollout.state, RuntimeCloudRolloutState::Failed);
        assert_eq!(
            rollout.targets[0].state,
            RuntimeCloudRolloutTargetState::Failed
        );
        assert_eq!(rollout.targets[0].blocked_reason, Some(ReasonCode::Timeout));
    }
}
