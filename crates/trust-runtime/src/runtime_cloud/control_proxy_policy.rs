//! Runtime-cloud control-proxy request planning policy.

#![allow(missing_docs)]

use serde_json::{json, Value};

use crate::runtime_cloud::contracts::{
    evaluate_compatibility, ContractCompatibility, ReasonCode, RUNTIME_CLOUD_API_VERSION,
};
use crate::runtime_cloud::routing::RuntimeCloudActionRequest;
use crate::security::AccessRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCloudControlProxyPlanError {
    pub(crate) code: ReasonCode,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCloudControlProxyRoleDenial {
    pub(crate) code: ReasonCode,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeCloudControlProxyPlan {
    pub(crate) target_runtime: String,
    pub(crate) action_type: String,
    pub(crate) action: RuntimeCloudActionRequest,
    pub(crate) control_payload: Value,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_cloud_control_proxy_plan(
    api_version: &str,
    actor: &str,
    target_runtime: &str,
    control_kind: &str,
    control_params: Option<&Value>,
    control_request_id: Option<&str>,
    connected_via: &str,
    required_role: AccessRole,
    now_ns: u64,
) -> Result<RuntimeCloudControlProxyPlan, RuntimeCloudControlProxyPlanError> {
    match evaluate_compatibility(api_version, RUNTIME_CLOUD_API_VERSION) {
        Ok(ContractCompatibility::Exact | ContractCompatibility::AdditiveWithinMajor) => {}
        Ok(ContractCompatibility::BreakingMajor) => {
            return Err(RuntimeCloudControlProxyPlanError {
                code: ReasonCode::ContractViolation,
                message: format!(
                    "unsupported api_version '{api_version}' for runtime cloud {RUNTIME_CLOUD_API_VERSION}"
                ),
            });
        }
        Err(error) => {
            return Err(RuntimeCloudControlProxyPlanError {
                code: ReasonCode::ContractViolation,
                message: error.to_string(),
            });
        }
    }

    let actor = actor.trim();
    if actor.is_empty() {
        return Err(RuntimeCloudControlProxyPlanError {
            code: ReasonCode::ContractViolation,
            message: "actor must not be empty".to_string(),
        });
    }

    let target_runtime = target_runtime.trim();
    if target_runtime.is_empty() {
        return Err(RuntimeCloudControlProxyPlanError {
            code: ReasonCode::ContractViolation,
            message: "target_runtime must not be empty".to_string(),
        });
    }

    let kind = control_kind.trim();
    if kind.is_empty() {
        return Err(RuntimeCloudControlProxyPlanError {
            code: ReasonCode::ContractViolation,
            message: "control_request.type must not be empty".to_string(),
        });
    }

    let request_id = control_request_id
        .map(str::to_string)
        .unwrap_or_else(|| format!("proxy-{now_ns}"));
    let action_type = runtime_cloud_control_proxy_action_type(kind, required_role);
    let action = RuntimeCloudActionRequest {
        api_version: api_version.to_string(),
        request_id: request_id.clone(),
        connected_via: connected_via.to_string(),
        target_runtimes: vec![target_runtime.to_string()],
        actor: actor.to_string(),
        action_type: action_type.to_string(),
        query_budget_ms: Some(1_500),
        dry_run: false,
        payload: runtime_cloud_control_proxy_action_payload(action_type, kind, control_params),
    };
    let control_payload =
        runtime_cloud_control_proxy_payload(kind, control_params, request_id.as_str());

    Ok(RuntimeCloudControlProxyPlan {
        target_runtime: target_runtime.to_string(),
        action_type: action_type.to_string(),
        action,
        control_payload,
    })
}

pub(crate) fn runtime_cloud_control_proxy_role_denial(
    actual_role: AccessRole,
    required_role: AccessRole,
    kind: &str,
) -> RuntimeCloudControlProxyRoleDenial {
    let denial_code = if kind == "config.set" {
        ReasonCode::AclDeniedCfgWrite
    } else {
        ReasonCode::PermissionDenied
    };
    RuntimeCloudControlProxyRoleDenial {
        code: denial_code,
        message: format!(
            "role '{}' does not satisfy required role '{}' for '{}'",
            actual_role.as_str(),
            required_role.as_str(),
            kind,
        ),
    }
}

fn runtime_cloud_control_proxy_action_type(kind: &str, required_role: AccessRole) -> &'static str {
    if kind == "config.set" {
        return "cfg_apply";
    }
    if required_role == AccessRole::Viewer {
        return "status_read";
    }
    "cmd_invoke"
}

fn runtime_cloud_control_proxy_action_payload(
    action_type: &str,
    kind: &str,
    params: Option<&Value>,
) -> Value {
    if action_type == "cfg_apply" {
        let config_params = params.cloned().unwrap_or_else(|| json!({}));
        return json!({ "params": config_params });
    }
    if action_type == "status_read" {
        return json!({});
    }
    let mut payload = json!({
        "command": kind,
    });
    if let Some(params) = params {
        payload["params"] = params.clone();
    }
    payload
}

fn runtime_cloud_control_proxy_payload(
    kind: &str,
    params: Option<&Value>,
    request_id: &str,
) -> Value {
    let mut payload = json!({
        "id": 1_u64,
        "type": kind,
        "request_id": request_id,
    });
    if let Some(params) = params {
        payload["params"] = params.clone();
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_plan_uses_status_read_for_viewer_control_request() {
        let plan = runtime_cloud_control_proxy_plan(
            "1.0",
            "spiffe://trust/default-site/operator-1",
            "runtime-b",
            "status",
            None,
            Some("proxy-1"),
            "runtime-a",
            AccessRole::Viewer,
            99,
        )
        .expect("control proxy plan");

        assert_eq!(plan.action_type, "status_read");
        assert_eq!(plan.action.action_type, "status_read");
        assert_eq!(plan.action.request_id, "proxy-1");
        assert_eq!(plan.action.target_runtimes, vec!["runtime-b"]);
        assert_eq!(plan.control_payload["type"], json!("status"));
        assert_eq!(plan.control_payload["request_id"], json!("proxy-1"));
    }

    #[test]
    fn proxy_plan_uses_cfg_apply_for_config_set() {
        let params = json!({ "log.level": "debug" });
        let plan = runtime_cloud_control_proxy_plan(
            "1.0",
            "spiffe://trust/default-site/engineer-1",
            "runtime-b",
            "config.set",
            Some(&params),
            Some("proxy-2"),
            "runtime-a",
            AccessRole::Engineer,
            99,
        )
        .expect("control proxy plan");

        assert_eq!(plan.action_type, "cfg_apply");
        assert_eq!(plan.action.payload["params"]["log.level"], json!("debug"));
        assert_eq!(plan.control_payload["type"], json!("config.set"));
        assert_eq!(plan.control_payload["params"]["log.level"], json!("debug"));
    }

    #[test]
    fn proxy_plan_uses_generated_request_id_when_missing() {
        let plan = runtime_cloud_control_proxy_plan(
            "1.0",
            "spiffe://trust/default-site/operator-1",
            "runtime-b",
            "restart",
            None,
            None,
            "runtime-a",
            AccessRole::Operator,
            42,
        )
        .expect("control proxy plan");

        assert_eq!(plan.action_type, "cmd_invoke");
        assert_eq!(plan.action.request_id, "proxy-42");
        assert_eq!(plan.control_payload["request_id"], json!("proxy-42"));
    }

    #[test]
    fn proxy_plan_rejects_breaking_api_version() {
        let error = runtime_cloud_control_proxy_plan(
            "2.0",
            "spiffe://trust/default-site/operator-1",
            "runtime-b",
            "status",
            None,
            Some("proxy-1"),
            "runtime-a",
            AccessRole::Viewer,
            99,
        )
        .expect_err("breaking api version should fail");

        assert_eq!(error.code, ReasonCode::ContractViolation);
        assert!(error.message.contains("unsupported api_version '2.0'"));
    }

    #[test]
    fn proxy_role_denial_uses_cfg_write_code_for_config_set() {
        let denial = runtime_cloud_control_proxy_role_denial(
            AccessRole::Viewer,
            AccessRole::Engineer,
            "config.set",
        );

        assert_eq!(denial.code, ReasonCode::AclDeniedCfgWrite);
        assert!(denial.message.contains("role 'viewer'"));
    }
}
