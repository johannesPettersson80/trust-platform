//! Runtime-cloud I/O config proxy planning policy.

#![allow(missing_docs)]

use serde_json::{json, Value};

use crate::runtime_cloud::contracts::ReasonCode;
use crate::runtime_cloud::routing::RuntimeCloudActionRequest;

pub(crate) const RUNTIME_CLOUD_IO_PROXY_ACTOR: &str = "runtime-cloud-io-proxy";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeCloudIoProxyOperation {
    ReadConfig,
    WriteConfig,
}

impl RuntimeCloudIoProxyOperation {
    fn action_type(self) -> &'static str {
        match self {
            Self::ReadConfig => "status_read",
            Self::WriteConfig => "cfg_apply",
        }
    }

    fn action_payload(self) -> Value {
        match self {
            Self::ReadConfig => json!({}),
            Self::WriteConfig => json!({ "params": {} }),
        }
    }

    fn target_required_message(self) -> &'static str {
        match self {
            Self::ReadConfig => "target runtime is required",
            Self::WriteConfig => "target_runtime is required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCloudIoProxyPlanError {
    pub(crate) code: ReasonCode,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeCloudIoProxyPlan {
    pub(crate) target_runtime: String,
    pub(crate) action_type: String,
    pub(crate) action: RuntimeCloudActionRequest,
}

pub(crate) fn runtime_cloud_io_proxy_plan(
    operation: RuntimeCloudIoProxyOperation,
    api_version: &str,
    actor: &str,
    target_runtime: &str,
    connected_via: &str,
    now_ns: u64,
) -> Result<RuntimeCloudIoProxyPlan, RuntimeCloudIoProxyPlanError> {
    let target_runtime = target_runtime.trim();
    if target_runtime.is_empty() {
        return Err(RuntimeCloudIoProxyPlanError {
            code: ReasonCode::ContractViolation,
            message: operation.target_required_message().to_string(),
        });
    }

    let actor = actor.trim();
    if actor.is_empty() {
        return Err(RuntimeCloudIoProxyPlanError {
            code: ReasonCode::ContractViolation,
            message: "actor is required".to_string(),
        });
    }

    let action_type = operation.action_type();
    let action = RuntimeCloudActionRequest {
        api_version: api_version.to_string(),
        request_id: format!("io-proxy-{now_ns}"),
        connected_via: connected_via.to_string(),
        target_runtimes: vec![target_runtime.to_string()],
        actor: actor.to_string(),
        action_type: action_type.to_string(),
        query_budget_ms: Some(1_500),
        dry_run: false,
        payload: operation.action_payload(),
    };

    Ok(RuntimeCloudIoProxyPlan {
        target_runtime: target_runtime.to_string(),
        action_type: action_type.to_string(),
        action,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_plan_uses_status_read_action() {
        let plan = runtime_cloud_io_proxy_plan(
            RuntimeCloudIoProxyOperation::ReadConfig,
            "1.0",
            RUNTIME_CLOUD_IO_PROXY_ACTOR,
            " runtime-b ",
            "runtime-a",
            7,
        )
        .expect("io proxy read plan");

        assert_eq!(plan.target_runtime, "runtime-b");
        assert_eq!(plan.action_type, "status_read");
        assert_eq!(plan.action.request_id, "io-proxy-7");
        assert_eq!(plan.action.connected_via, "runtime-a");
        assert_eq!(plan.action.actor, RUNTIME_CLOUD_IO_PROXY_ACTOR);
        assert_eq!(plan.action.payload, json!({}));
    }

    #[test]
    fn write_plan_uses_cfg_apply_action() {
        let plan = runtime_cloud_io_proxy_plan(
            RuntimeCloudIoProxyOperation::WriteConfig,
            "1.0",
            " spiffe://trust/default-site/engineer-1 ",
            " runtime-b ",
            "runtime-a",
            9,
        )
        .expect("io proxy write plan");

        assert_eq!(plan.target_runtime, "runtime-b");
        assert_eq!(plan.action_type, "cfg_apply");
        assert_eq!(plan.action.request_id, "io-proxy-9");
        assert_eq!(plan.action.actor, "spiffe://trust/default-site/engineer-1");
        assert_eq!(plan.action.payload, json!({ "params": {} }));
    }

    #[test]
    fn read_plan_uses_legacy_target_error_text() {
        let error = runtime_cloud_io_proxy_plan(
            RuntimeCloudIoProxyOperation::ReadConfig,
            "1.0",
            RUNTIME_CLOUD_IO_PROXY_ACTOR,
            " ",
            "runtime-a",
            7,
        )
        .expect_err("empty target should fail");

        assert_eq!(error.code, ReasonCode::ContractViolation);
        assert_eq!(error.message, "target runtime is required");
    }

    #[test]
    fn write_plan_validates_target_before_actor() {
        let error = runtime_cloud_io_proxy_plan(
            RuntimeCloudIoProxyOperation::WriteConfig,
            "1.0",
            "",
            " ",
            "runtime-a",
            7,
        )
        .expect_err("empty target should fail first");

        assert_eq!(error.code, ReasonCode::ContractViolation);
        assert_eq!(error.message, "target_runtime is required");
    }

    #[test]
    fn write_plan_rejects_empty_actor() {
        let error = runtime_cloud_io_proxy_plan(
            RuntimeCloudIoProxyOperation::WriteConfig,
            "1.0",
            " ",
            "runtime-b",
            "runtime-a",
            7,
        )
        .expect_err("empty actor should fail");

        assert_eq!(error.code, ReasonCode::ContractViolation);
        assert_eq!(error.message, "actor is required");
    }
}
