//! Runtime cloud HA lease authority, fencing, and replay protection.

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::contracts::ReasonCode;
use policy::{authority_decision, requires_active_authority, target_can_own_active_namespace};

mod policy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCloudHaProfile {
    #[default]
    SingleHost,
    DualHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCloudHaRole {
    #[default]
    Candidate,
    Standby,
    Active,
    DemotedSafe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeCloudHaTargetPolicy {
    #[serde(default)]
    pub role: RuntimeCloudHaRole,
    #[serde(default)]
    pub lease_consistent_authority: bool,
    #[serde(default)]
    pub lease_available: bool,
    #[serde(default)]
    pub lease_owner_runtime_id: Option<String>,
    #[serde(default)]
    pub fencing_token: Option<String>,
    #[serde(default)]
    pub fence_valid: bool,
    #[serde(default)]
    pub ambiguous_leadership: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeCloudHaRequest {
    #[serde(default)]
    pub profile: RuntimeCloudHaProfile,
    #[serde(default)]
    pub group_id: String,
    #[serde(default)]
    pub active_namespace: bool,
    #[serde(default)]
    pub command_seq: Option<u64>,
    #[serde(default)]
    pub targets: BTreeMap<String, RuntimeCloudHaTargetPolicy>,
}

impl RuntimeCloudHaRequest {
    #[must_use]
    pub fn split_brain_runtimes(&self, action_type: &str) -> BTreeSet<String> {
        if self.profile != RuntimeCloudHaProfile::DualHost
            || !requires_active_authority(action_type, self)
        {
            return BTreeSet::new();
        }

        let mut active_candidates = Vec::new();
        for (runtime_id, target) in &self.targets {
            if target_can_own_active_namespace(runtime_id, target) {
                active_candidates.push(runtime_id.to_string());
            }
        }
        if active_candidates.len() <= 1 {
            return BTreeSet::new();
        }
        active_candidates.into_iter().collect()
    }
}

pub fn parse_action_ha_request(payload: &Value) -> Result<Option<RuntimeCloudHaRequest>, String> {
    let Some(ha) = payload.get("ha") else {
        return Ok(None);
    };
    serde_json::from_value::<RuntimeCloudHaRequest>(ha.clone())
        .map(Some)
        .map_err(|error| format!("payload.ha is invalid: {error}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCloudHaDecision {
    pub allowed: bool,
    pub denial_code: Option<ReasonCode>,
    pub denial_reason: Option<String>,
}

impl RuntimeCloudHaDecision {
    #[must_use]
    pub fn allow() -> Self {
        Self {
            allowed: true,
            denial_code: None,
            denial_reason: None,
        }
    }

    #[must_use]
    pub fn deny(code: ReasonCode, reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            denial_code: Some(code),
            denial_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCloudHaDispatchRecord {
    pub ok: bool,
    pub denial_code: Option<ReasonCode>,
    pub denial_reason: Option<String>,
    pub audit_id: Option<String>,
    pub response: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCloudHaDispatchTicket {
    group_id: String,
    runtime_id: String,
    request_id: String,
    command_seq: u64,
}

type RuntimeCloudHaDispatchKey = (String, String, String);

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeCloudHaDispatchGate {
    Proceed(RuntimeCloudHaDispatchTicket),
    Denied(RuntimeCloudHaDecision),
    Deduplicated(RuntimeCloudHaDispatchRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeCloudHaCoordinator {
    groups: BTreeMap<String, RuntimeCloudHaGroupLedger>,
    #[serde(skip)]
    active_dispatches: BTreeSet<RuntimeCloudHaDispatchKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RuntimeCloudHaGroupLedger {
    runtimes: BTreeMap<String, RuntimeCloudHaRuntimeLedger>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RuntimeCloudHaRuntimeLedger {
    last_applied_command_seq: u64,
    requests: BTreeMap<String, RuntimeCloudHaRequestLedger>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RuntimeCloudHaRequestLedger {
    command_seq: u64,
    pending: bool,
    result: Option<RuntimeCloudHaDispatchRecord>,
}

impl RuntimeCloudHaCoordinator {
    #[must_use]
    pub fn preflight_decision(
        &self,
        action_type: &str,
        request_id: &str,
        runtime_id: &str,
        ha_request: &RuntimeCloudHaRequest,
    ) -> Option<RuntimeCloudHaDecision> {
        if ha_request.profile != RuntimeCloudHaProfile::DualHost {
            return None;
        }
        let authority = authority_decision(action_type, runtime_id, ha_request);
        if !authority.allowed {
            return Some(authority);
        }
        if action_type != "cmd_invoke" {
            return Some(RuntimeCloudHaDecision::allow());
        }
        let command_seq = ha_request.command_seq.unwrap_or_default();
        match self.replay_check(
            ha_request.group_id.as_str(),
            runtime_id,
            request_id,
            command_seq,
        ) {
            RuntimeCloudHaReplayCheck::Denied(decision) => Some(decision),
            RuntimeCloudHaReplayCheck::Proceed | RuntimeCloudHaReplayCheck::Deduplicated(_) => {
                Some(RuntimeCloudHaDecision::allow())
            }
        }
    }

    #[must_use]
    pub fn begin_dispatch(
        &mut self,
        action_type: &str,
        request_id: &str,
        runtime_id: &str,
        ha_request: &RuntimeCloudHaRequest,
    ) -> Option<RuntimeCloudHaDispatchGate> {
        if ha_request.profile != RuntimeCloudHaProfile::DualHost {
            return None;
        }
        let authority = authority_decision(action_type, runtime_id, ha_request);
        if !authority.allowed {
            return Some(RuntimeCloudHaDispatchGate::Denied(authority));
        }
        if action_type != "cmd_invoke" {
            return None;
        }
        let command_seq = ha_request.command_seq.unwrap_or_default();
        match self.replay_check(
            ha_request.group_id.as_str(),
            runtime_id,
            request_id,
            command_seq,
        ) {
            RuntimeCloudHaReplayCheck::Denied(decision) => {
                Some(RuntimeCloudHaDispatchGate::Denied(decision))
            }
            RuntimeCloudHaReplayCheck::Deduplicated(result) => {
                Some(RuntimeCloudHaDispatchGate::Deduplicated(result))
            }
            RuntimeCloudHaReplayCheck::Proceed => {
                {
                    let runtime = self.runtime_ledger_mut(ha_request.group_id.as_str(), runtime_id);
                    runtime.requests.insert(
                        request_id.to_string(),
                        RuntimeCloudHaRequestLedger {
                            command_seq,
                            pending: true,
                            result: None,
                        },
                    );
                }
                self.active_dispatches.insert(runtime_cloud_ha_dispatch_key(
                    ha_request.group_id.as_str(),
                    runtime_id,
                    request_id,
                ));
                Some(RuntimeCloudHaDispatchGate::Proceed(
                    RuntimeCloudHaDispatchTicket {
                        group_id: ha_request.group_id.clone(),
                        runtime_id: runtime_id.to_string(),
                        request_id: request_id.to_string(),
                        command_seq,
                    },
                ))
            }
        }
    }

    pub fn finish_dispatch(
        &mut self,
        ticket: RuntimeCloudHaDispatchTicket,
        result: RuntimeCloudHaDispatchRecord,
    ) {
        {
            let runtime =
                self.runtime_ledger_mut(ticket.group_id.as_str(), ticket.runtime_id.as_str());
            let Some(record) = runtime.requests.get_mut(ticket.request_id.as_str()) else {
                return;
            };
            if record.command_seq != ticket.command_seq {
                return;
            }
            record.pending = false;
            record.result = Some(result.clone());
            if result.ok {
                runtime.last_applied_command_seq =
                    runtime.last_applied_command_seq.max(ticket.command_seq);
            }
        }
        self.active_dispatches
            .remove(&runtime_cloud_ha_dispatch_key(
                ticket.group_id.as_str(),
                ticket.runtime_id.as_str(),
                ticket.request_id.as_str(),
            ));
    }

    #[must_use]
    pub fn last_applied_command_seq(&self, group_id: &str, runtime_id: &str) -> u64 {
        self.groups
            .get(group_id)
            .and_then(|group| group.runtimes.get(runtime_id))
            .map(|runtime| runtime.last_applied_command_seq)
            .unwrap_or(0)
    }

    fn runtime_ledger_mut(
        &mut self,
        group_id: &str,
        runtime_id: &str,
    ) -> &mut RuntimeCloudHaRuntimeLedger {
        self.groups
            .entry(group_id.to_string())
            .or_default()
            .runtimes
            .entry(runtime_id.to_string())
            .or_default()
    }

    fn replay_check(
        &self,
        group_id: &str,
        runtime_id: &str,
        request_id: &str,
        command_seq: u64,
    ) -> RuntimeCloudHaReplayCheck {
        if command_seq == 0 {
            return RuntimeCloudHaReplayCheck::Denied(RuntimeCloudHaDecision::deny(
                ReasonCode::ContractViolation,
                "command_seq must be greater than zero for cmd_invoke",
            ));
        }
        let runtime = self
            .groups
            .get(group_id)
            .and_then(|group| group.runtimes.get(runtime_id));
        let Some(runtime) = runtime else {
            return RuntimeCloudHaReplayCheck::Proceed;
        };

        if let Some(existing) = runtime.requests.get(request_id) {
            if existing.command_seq != command_seq {
                return RuntimeCloudHaReplayCheck::Denied(RuntimeCloudHaDecision::deny(
                    ReasonCode::Conflict,
                    format!(
                        "request_id '{}' already recorded with command_seq {} (received {})",
                        request_id, existing.command_seq, command_seq
                    ),
                ));
            }
            if existing.pending {
                if self
                    .active_dispatches
                    .contains(&runtime_cloud_ha_dispatch_key(
                        group_id, runtime_id, request_id,
                    ))
                {
                    return RuntimeCloudHaReplayCheck::Denied(RuntimeCloudHaDecision::deny(
                        ReasonCode::Conflict,
                        format!(
                            "request_id '{}' is already dispatching command_seq {} for group '{}' runtime '{}'",
                            request_id, command_seq, group_id, runtime_id
                        ),
                    ));
                }
                return RuntimeCloudHaReplayCheck::Proceed;
            }
            if let Some(result) = existing.result.clone() {
                return RuntimeCloudHaReplayCheck::Deduplicated(result);
            }
            return RuntimeCloudHaReplayCheck::Proceed;
        }

        if command_seq <= runtime.last_applied_command_seq {
            return RuntimeCloudHaReplayCheck::Denied(RuntimeCloudHaDecision::deny(
                ReasonCode::Conflict,
                format!(
                    "command_seq {} is stale for group '{}' runtime '{}'; last_applied={}",
                    command_seq, group_id, runtime_id, runtime.last_applied_command_seq
                ),
            ));
        }
        RuntimeCloudHaReplayCheck::Proceed
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RuntimeCloudHaReplayCheck {
    Proceed,
    Denied(RuntimeCloudHaDecision),
    Deduplicated(RuntimeCloudHaDispatchRecord),
}

fn runtime_cloud_ha_dispatch_key(
    group_id: &str,
    runtime_id: &str,
    request_id: &str,
) -> RuntimeCloudHaDispatchKey {
    (
        group_id.to_string(),
        runtime_id.to_string(),
        request_id.to_string(),
    )
}

#[cfg(test)]
mod tests;
