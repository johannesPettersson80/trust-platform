//! Runtime cloud cross-runtime action routing contracts and preflight ACL policy.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::security::AccessRole;

use super::contracts::{
    evaluate_compatibility, ContractCompatibility, ReasonCode, RUNTIME_CLOUD_API_VERSION,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCloudActionRequest {
    pub api_version: String,
    pub request_id: String,
    pub connected_via: String,
    pub target_runtimes: Vec<String>,
    pub actor: String,
    pub action_type: String,
    #[serde(default)]
    pub query_budget_ms: Option<u64>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCloudTargetStatus {
    pub reachable: bool,
    pub stale: bool,
    pub supports_secure_transport: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCloudTargetDecision {
    pub runtime_id: String,
    pub allowed: bool,
    pub denial_code: Option<ReasonCode>,
    pub denial_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCloudActionPreflight {
    pub api_version: String,
    pub request_id: String,
    pub connected_via: String,
    pub acting_on: Vec<String>,
    pub allowed: bool,
    pub denial_code: Option<ReasonCode>,
    pub denial_reason: Option<String>,
    pub decisions: Vec<RuntimeCloudTargetDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCloudPreflightContext<'a> {
    pub local_runtime_id: &'a str,
    pub role: AccessRole,
}

pub fn preflight_action(
    request: &RuntimeCloudActionRequest,
    context: RuntimeCloudPreflightContext<'_>,
    targets: &BTreeMap<String, RuntimeCloudTargetStatus>,
) -> RuntimeCloudActionPreflight {
    let mut global_denial: Option<(ReasonCode, String)> = None;

    if request.request_id.trim().is_empty() {
        global_denial = Some((
            ReasonCode::ContractViolation,
            "request_id must not be empty".to_string(),
        ));
    }
    if global_denial.is_none() && request.actor.trim().is_empty() {
        global_denial = Some((
            ReasonCode::ContractViolation,
            "actor must not be empty".to_string(),
        ));
    }
    if global_denial.is_none() {
        match evaluate_compatibility(request.api_version.as_str(), RUNTIME_CLOUD_API_VERSION) {
            Ok(ContractCompatibility::BreakingMajor) => {
                global_denial = Some((
                    ReasonCode::ContractViolation,
                    format!(
                        "unsupported api_version '{}' for runtime cloud {}",
                        request.api_version, RUNTIME_CLOUD_API_VERSION
                    ),
                ));
            }
            Ok(ContractCompatibility::Exact | ContractCompatibility::AdditiveWithinMajor) => {}
            Err(error) => {
                global_denial = Some((ReasonCode::ContractViolation, error.to_string()));
            }
        }
    }
    if global_denial.is_none() && request.connected_via != context.local_runtime_id {
        global_denial = Some((
            ReasonCode::ContractViolation,
            format!(
                "connected_via '{}' must match local runtime '{}'",
                request.connected_via, context.local_runtime_id
            ),
        ));
    }
    if global_denial.is_none() && request.target_runtimes.is_empty() {
        global_denial = Some((
            ReasonCode::ContractViolation,
            "target_runtimes must include at least one runtime".to_string(),
        ));
    }

    let mut required_role = AccessRole::Viewer;
    if global_denial.is_none() {
        match required_role_for_action(request) {
            Ok(role) => required_role = role,
            Err(error) => global_denial = Some(error),
        }
    }

    let decisions = request
        .target_runtimes
        .iter()
        .map(|runtime_id| {
            if let Some((code, reason)) = global_denial.as_ref() {
                return RuntimeCloudTargetDecision {
                    runtime_id: runtime_id.clone(),
                    allowed: false,
                    denial_code: Some(*code),
                    denial_reason: Some(reason.clone()),
                };
            }
            if !context.role.allows(required_role) {
                let denial_code = denial_for_role(context.role, required_role, request);
                return RuntimeCloudTargetDecision {
                    runtime_id: runtime_id.clone(),
                    allowed: false,
                    denial_code: Some(denial_code),
                    denial_reason: Some(format!(
                        "role '{}' does not satisfy required role '{}' for action '{}'",
                        context.role.as_str(),
                        required_role.as_str(),
                        request.action_type
                    )),
                };
            }
            if runtime_id == context.local_runtime_id {
                return RuntimeCloudTargetDecision {
                    runtime_id: runtime_id.clone(),
                    allowed: true,
                    denial_code: None,
                    denial_reason: None,
                };
            }
            let status = targets.get(runtime_id);
            match status {
                Some(RuntimeCloudTargetStatus { stale: true, .. }) => RuntimeCloudTargetDecision {
                    runtime_id: runtime_id.clone(),
                    allowed: false,
                    denial_code: Some(ReasonCode::StaleData),
                    denial_reason: Some(format!(
                        "target runtime '{}' is stale; wait for fresh liveliness before dispatch",
                        runtime_id
                    )),
                },
                None
                | Some(RuntimeCloudTargetStatus {
                    reachable: false, ..
                }) => RuntimeCloudTargetDecision {
                    runtime_id: runtime_id.clone(),
                    allowed: false,
                    denial_code: Some(ReasonCode::TargetUnreachable),
                    denial_reason: Some(format!(
                        "target runtime '{}' is not reachable from connected_via '{}'",
                        runtime_id, request.connected_via
                    )),
                },
                Some(RuntimeCloudTargetStatus {
                    reachable: true,
                    stale: false,
                    ..
                }) => RuntimeCloudTargetDecision {
                    runtime_id: runtime_id.clone(),
                    allowed: true,
                    denial_code: None,
                    denial_reason: None,
                },
            }
        })
        .collect::<Vec<_>>();

    let allowed = decisions.iter().all(|decision| decision.allowed);
    let (denial_code, denial_reason) = decisions
        .iter()
        .find(|decision| !decision.allowed)
        .map(|decision| (decision.denial_code, decision.denial_reason.clone()))
        .unwrap_or((None, None));

    RuntimeCloudActionPreflight {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        request_id: request.request_id.clone(),
        connected_via: request.connected_via.clone(),
        acting_on: request.target_runtimes.clone(),
        allowed,
        denial_code,
        denial_reason,
        decisions,
    }
}

pub fn map_action_to_control_request(
    request: &RuntimeCloudActionRequest,
) -> Result<serde_json::Value, (ReasonCode, String)> {
    match request.action_type.as_str() {
        "cfg_apply" => {
            let params = request
                .payload
                .get("params")
                .cloned()
                .unwrap_or_else(|| request.payload.clone());
            if !params.is_object() {
                return Err((
                    ReasonCode::ContractViolation,
                    "cfg_apply requires object payload.params".to_string(),
                ));
            }
            Ok(serde_json::json!({
                "id": 1_u64,
                "type": "config.set",
                "request_id": request.request_id,
                "params": params
            }))
        }
        "status_read" => Ok(serde_json::json!({
            "id": 1_u64,
            "type": "status",
            "request_id": request.request_id
        })),
        "cmd_invoke" => {
            let command = request
                .payload
                .get("command")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    (
                        ReasonCode::ContractViolation,
                        "cmd_invoke requires payload.command".to_string(),
                    )
                })?;
            let mut control = serde_json::json!({
                "id": 1_u64,
                "type": command,
                "request_id": request.request_id
            });
            if let Some(params) = request.payload.get("params") {
                control["params"] = params.clone();
            }
            Ok(control)
        }
        _ => Err((
            ReasonCode::ContractViolation,
            format!("unsupported action_type '{}'", request.action_type),
        )),
    }
}

fn required_role_for_action(
    request: &RuntimeCloudActionRequest,
) -> Result<AccessRole, (ReasonCode, String)> {
    match request.action_type.as_str() {
        "cfg_apply" => Ok(cfg_apply_required_role(&request.payload)),
        "status_read" => Ok(AccessRole::Viewer),
        "cmd_invoke" => Ok(AccessRole::Operator),
        _ => Err((
            ReasonCode::ContractViolation,
            format!("unsupported action_type '{}'", request.action_type),
        )),
    }
}

fn cfg_apply_required_role(payload: &serde_json::Value) -> AccessRole {
    let has_protected_key = |object: Option<&serde_json::Map<String, serde_json::Value>>| {
        object.is_some_and(|params| {
            params.keys().any(|key| {
                matches!(
                    key.as_str(),
                    "control.auth_token"
                        | "mesh.auth_token"
                        | "control.mode"
                        | "web.auth"
                        | "runtime_cloud.profile"
                        | "runtime_cloud.wan.allow_write"
                        | "runtime_cloud.links.transports"
                )
            })
        })
    };
    let protected_keys =
        has_protected_key(payload.get("params").and_then(serde_json::Value::as_object))
            || has_protected_key(payload.as_object());
    if protected_keys {
        AccessRole::Admin
    } else {
        AccessRole::Engineer
    }
}

fn denial_for_role(
    _actual_role: AccessRole,
    _required_role: AccessRole,
    request: &RuntimeCloudActionRequest,
) -> ReasonCode {
    match request.action_type.as_str() {
        "cfg_apply" => ReasonCode::AclDeniedCfgWrite,
        _ => ReasonCode::PermissionDenied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_fixture() -> RuntimeCloudActionRequest {
        RuntimeCloudActionRequest {
            api_version: "1.0".to_string(),
            request_id: "req-1".to_string(),
            connected_via: "runtime-a".to_string(),
            target_runtimes: vec!["runtime-b".to_string()],
            actor: "spiffe://trust/site-a/operator".to_string(),
            action_type: "cfg_apply".to_string(),
            query_budget_ms: None,
            dry_run: true,
            payload: serde_json::json!({ "params": { "log.level": "debug" } }),
        }
    }

    #[test]
    fn preflight_applies_acl_denial_for_cfg_write() {
        let request = request_fixture();
        let mut targets = BTreeMap::new();
        targets.insert(
            "runtime-b".to_string(),
            RuntimeCloudTargetStatus {
                reachable: true,
                stale: false,
                supports_secure_transport: true,
            },
        );

        let report = preflight_action(
            &request,
            RuntimeCloudPreflightContext {
                local_runtime_id: "runtime-a",
                role: AccessRole::Operator,
            },
            &targets,
        );
        assert!(!report.allowed);
        assert_eq!(report.denial_code, Some(ReasonCode::AclDeniedCfgWrite));
        assert_eq!(report.decisions.len(), 1);
        assert_eq!(
            report.decisions[0].denial_code,
            Some(ReasonCode::AclDeniedCfgWrite)
        );
    }

    #[test]
    fn preflight_connected_via_mismatch_has_deterministic_precedence() {
        let mut request = request_fixture();
        request.connected_via = "runtime-z".to_string();
        let targets = BTreeMap::new();

        let report = preflight_action(
            &request,
            RuntimeCloudPreflightContext {
                local_runtime_id: "runtime-a",
                role: AccessRole::Admin,
            },
            &targets,
        );
        assert!(!report.allowed);
        assert_eq!(report.denial_code, Some(ReasonCode::ContractViolation));
        assert_eq!(
            report.decisions[0].denial_code,
            Some(ReasonCode::ContractViolation)
        );
    }

    #[test]
    fn preflight_denies_unreachable_target_with_reason_code() {
        let request = request_fixture();
        let targets = BTreeMap::new();

        let report = preflight_action(
            &request,
            RuntimeCloudPreflightContext {
                local_runtime_id: "runtime-a",
                role: AccessRole::Admin,
            },
            &targets,
        );
        assert!(!report.allowed);
        assert_eq!(report.denial_code, Some(ReasonCode::TargetUnreachable));
        assert_eq!(
            report.decisions[0].denial_code,
            Some(ReasonCode::TargetUnreachable)
        );
    }

    #[test]
    fn cfg_apply_root_level_protected_key_requires_admin_role() {
        let mut request = request_fixture();
        request.payload = serde_json::json!({
            "runtime_cloud.profile": "wan"
        });
        let mut targets = BTreeMap::new();
        targets.insert(
            "runtime-b".to_string(),
            RuntimeCloudTargetStatus {
                reachable: true,
                stale: false,
                supports_secure_transport: true,
            },
        );

        let report = preflight_action(
            &request,
            RuntimeCloudPreflightContext {
                local_runtime_id: "runtime-a",
                role: AccessRole::Engineer,
            },
            &targets,
        );
        assert!(!report.allowed);
        assert_eq!(report.denial_code, Some(ReasonCode::AclDeniedCfgWrite));
    }

    #[test]
    fn cfg_apply_link_transport_protected_key_requires_admin_role() {
        let mut request = request_fixture();
        request.payload = serde_json::json!({
            "runtime_cloud.links.transports": [
                { "source": "runtime-a", "target": "runtime-b", "transport": "realtime" }
            ]
        });
        let mut targets = BTreeMap::new();
        targets.insert(
            "runtime-b".to_string(),
            RuntimeCloudTargetStatus {
                reachable: true,
                stale: false,
                supports_secure_transport: true,
            },
        );

        let report = preflight_action(
            &request,
            RuntimeCloudPreflightContext {
                local_runtime_id: "runtime-a",
                role: AccessRole::Engineer,
            },
            &targets,
        );
        assert!(!report.allowed);
        assert_eq!(report.denial_code, Some(ReasonCode::AclDeniedCfgWrite));
    }

    #[test]
    fn map_action_cfg_apply_emits_config_set_control_request() {
        let request = request_fixture();
        let mapped = map_action_to_control_request(&request).expect("mapped action");
        assert_eq!(
            mapped.get("type").and_then(serde_json::Value::as_str),
            Some("config.set")
        );
        assert_eq!(
            mapped.get("request_id").and_then(serde_json::Value::as_str),
            Some("req-1")
        );
        assert!(
            mapped.get("params").is_some(),
            "config.set mapping must include params"
        );
    }

    #[test]
    fn map_action_status_read_emits_status_control_request() {
        let mut request = request_fixture();
        request.action_type = "status_read".to_string();
        request.payload = serde_json::json!({});
        let mapped = map_action_to_control_request(&request).expect("mapped status_read");
        assert_eq!(
            mapped.get("type").and_then(serde_json::Value::as_str),
            Some("status")
        );
        assert_eq!(
            mapped.get("request_id").and_then(serde_json::Value::as_str),
            Some("req-1")
        );
    }

    #[test]
    fn runtime_cloud_api_payload_fuzz_smoke_budget() {
        fn next(state: &mut u64) -> u64 {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *state
        }

        let iterations = std::env::var("TRUST_COMMS_FUZZ_ITERS")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(512);
        let mut state = 0xBEEF_BAAD_0102_0304_u64;

        for idx in 0..iterations {
            let action_type = match next(&mut state) % 4 {
                0 => "cfg_apply",
                1 => "status_read",
                2 => "cmd_invoke",
                _ => "unsupported",
            };
            let payload = match next(&mut state) % 4 {
                0 => serde_json::json!({}),
                1 => serde_json::json!({"params": {"k": next(&mut state)}}),
                2 => serde_json::json!({"command": "status", "params": {"blob": next(&mut state)}}),
                _ => serde_json::json!([next(&mut state), next(&mut state)]),
            };
            let target = format!("runtime-{}", (next(&mut state) % 3) + 1);
            let request = RuntimeCloudActionRequest {
                api_version: if next(&mut state).is_multiple_of(5) {
                    "2.0".to_string()
                } else {
                    "1.0".to_string()
                },
                request_id: if next(&mut state).is_multiple_of(7) {
                    String::new()
                } else {
                    format!("req-{idx}")
                },
                connected_via: if next(&mut state).is_multiple_of(9) {
                    "runtime-z".to_string()
                } else {
                    "runtime-a".to_string()
                },
                target_runtimes: vec![target.clone()],
                actor: "spiffe://trust/site-a/operator".to_string(),
                action_type: action_type.to_string(),
                query_budget_ms: Some((next(&mut state) % 4_000) + 1),
                dry_run: next(&mut state).is_multiple_of(2),
                payload,
            };

            let mut targets = BTreeMap::new();
            targets.insert(
                target,
                RuntimeCloudTargetStatus {
                    reachable: next(&mut state).is_multiple_of(2),
                    stale: next(&mut state).is_multiple_of(3),
                    supports_secure_transport: next(&mut state).is_multiple_of(2),
                },
            );

            let report = preflight_action(
                &request,
                RuntimeCloudPreflightContext {
                    local_runtime_id: "runtime-a",
                    role: AccessRole::Admin,
                },
                &targets,
            );
            assert_eq!(report.decisions.len(), request.target_runtimes.len());

            let _ = map_action_to_control_request(&request);
        }
    }
}
